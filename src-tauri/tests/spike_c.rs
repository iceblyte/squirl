//! Spike C —— 存储链路集成测试（三平台 CI 可跑；本机 Windows 实测）。
//!
//! 运行方式：`cargo test --test spike_c`
//!
//! C1 WAL + 双桶 VACUUM INTO 快照滚动；C2 注入损坏 → 备份回填 → 再校验
//! （AC9 预演）；C3 迁移失败双层防御（SQL 失败回滚 / 语义错误 premigrate
//! 回滚 + 阻断启动，DR-13）；C4 退出四步串行（DR-14：备份包含最终写回）。

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use squirl::db::backup;
use squirl::db::migrate::SCHEMA_V1_SQL;
use squirl::db::repo::{insert_command, insert_project, RunFinal, RunOutcome};
use squirl::db::{Db, RecoveryEvent};
use uuid::Uuid;

fn temp_db(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("squirl-c-{label}-{}", Uuid::now_v7().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("squirl.db")
}

fn bucket(db: &Path, suffix: &str, i: u32) -> PathBuf {
    let file = db.file_name().unwrap().to_string_lossy().into_owned();
    db.with_file_name(format!("{file}{suffix}{i}"))
}

fn open_snapshot(path: &Path) -> Connection {
    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

// ---------- C1：WAL + 双桶快照滚动 ----------

#[test]
fn c1_open_produces_backup_and_wal() {
    let path = temp_db("c1");
    let (db, event) = Db::open(&path).unwrap();
    assert_eq!(event, RecoveryEvent::None);
    let conn = db.conn();
    let mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
    assert_eq!(mode.to_lowercase(), "wal");
    // 首次打开触发迁移 → 常规备份一份
    assert!(bucket(&path, backup::REGULAR_SUFFIX, 1).exists());
    // premigrate 桶：空库首迁也留了一份（独立轮换）
    assert!(bucket(&path, backup::PREMIGRATE_SUFFIX, 1).exists());
}

#[test]
fn c1_premigrate_bucket_survives_daily_rotation() {
    let path = temp_db("c1b");
    let (db, _) = Db::open(&path).unwrap();
    let pre = bucket(&path, backup::PREMIGRATE_SUFFIX, 1);
    let pre_time = std::fs::metadata(&pre).unwrap().modified().unwrap();
    // 再来 5 轮常规备份（模拟多次启动/退出）
    {
        let conn = db.conn();
        for _ in 0..5 {
            backup::snapshot_regular(&conn).unwrap();
        }
    }
    assert!(
        bucket(&path, backup::REGULAR_SUFFIX, 3).exists(),
        "常规桶滚到 3 份"
    );
    let pre_time_after = std::fs::metadata(&pre).unwrap().modified().unwrap();
    assert_eq!(
        pre_time, pre_time_after,
        "premigrate 快照不得被日常备份轮换淘汰（DR-13）"
    );
}

// ---------- C2：注入损坏 → 回填 → 再校验（AC9 预演） ----------

#[test]
fn c2_corruption_restores_from_latest_backup() {
    let path = temp_db("c2");
    let pid;
    {
        let (db, _) = Db::open(&path).unwrap();
        pid = insert_project(&db.conn(), Path::new("D:/proj/demo"), "demo").unwrap();
        insert_command(&db.conn(), pid, "dev", "npm run dev").unwrap();
        // 模拟正常退出备份（备份点 = 迁移成功 + 正常退出，设计 §5.1.2）
        backup::snapshot_regular(&db.conn()).unwrap();
    } // 连接关闭
      // 注入损坏：破坏文件头
    std::fs::write(&path, b"SQUIRL-CORRUPTED-HEADER-GARBAGE!!").unwrap();

    let (db2, event) = Db::open(&path).unwrap();
    assert_eq!(event, RecoveryEvent::RestoredFromBackup, "必须回填备份并提示");
    let name: String = db2
        .conn()
        .query_row("SELECT name FROM projects WHERE id=?1", rusqlite::params![pid.to_string()], |r| r.get(0))
        .unwrap();
    assert_eq!(name, "demo", "数据回到最近快照");
}

#[test]
fn c2_total_loss_rebuilds_empty_and_reports() {
    let path = temp_db("c2b");
    {
        let (db, _) = Db::open(&path).unwrap();
        insert_project(&db.conn(), Path::new("D:/proj/demo"), "demo").unwrap();
        backup::snapshot_regular(&db.conn()).unwrap();
    }
    // 主库 + 全部常规备份一起破坏
    std::fs::write(&path, b"corrupt").unwrap();
    for i in 1..=backup::REGULAR_BUCKETS {
        let b = bucket(&path, backup::REGULAR_SUFFIX, i);
        if b.exists() {
            std::fs::write(&b, b"corrupt").unwrap();
        }
    }
    let (db, event) = Db::open(&path).unwrap();
    assert_eq!(event, RecoveryEvent::RebuiltEmpty, "全部备份不可用 → 重建空库");
    let n: i64 = db.conn().query_row("SELECT count(*) FROM projects", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0);
    // 应用仍可启动使用
    insert_project(&db.conn(), Path::new("D:/proj/new"), "new").unwrap();
}

// ---------- C3：迁移失败双层防御（DR-13） ----------

/// C3a：SQL 失败的迁移 → 事务自动回滚、user_version 不动、阻断启动。
#[test]
fn c3a_sql_failure_rolls_back_and_blocks() {
    let path = temp_db("c3a");
    {
        let (db, _) = Db::open(&path).unwrap();
        insert_project(&db.conn(), Path::new("D:/proj/x"), "x").unwrap();
    }
    let bad = squirl::rusqlite_migration::Migrations::new(vec![
        squirl::rusqlite_migration::M::up(SCHEMA_V1_SQL),
        squirl::rusqlite_migration::M::up("CREATE TABLE broken ("), // 语法错误
    ]);
    let err = Db::open_with_migrations(&path, &bad).unwrap_err();
    assert!(matches!(err, squirl::error::AppError::MigrationBlocked(_)));

    // 库保持迁移前状态：数据完好、user_version=1
    let conn = Connection::open(&path).unwrap();
    assert_eq!(squirl::db::migrate::user_version(&conn), 1, "user_version 不动");
    let n: i64 = conn.query_row("SELECT count(*) FROM projects", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "「迁一半」被事务回滚挡住");
}

/// C3b：「成功但语义错误」的迁移 → premigrate 快照回滚 + 阻断启动。
#[test]
fn c3b_semantic_failure_restores_premigrate_and_blocks() {
    let path = temp_db("c3b");
    {
        let (db, _) = Db::open(&path).unwrap();
        let pid = insert_project(&db.conn(), Path::new("D:/proj/x"), "x").unwrap();
        insert_command(&db.conn(), pid, "dev", "npm run dev").unwrap();
    }
    // SQL 成功但语义破坏：删掉 commands 表
    let bad = squirl::rusqlite_migration::Migrations::new(vec![
        squirl::rusqlite_migration::M::up(SCHEMA_V1_SQL),
        squirl::rusqlite_migration::M::up("DROP TABLE commands;"),
    ]);
    let err = Db::open_with_migrations(&path, &bad).unwrap_err();
    match err {
        squirl::error::AppError::MigrationBlocked(detail) => {
            assert!(detail.contains("回滚"), "阻断信息须明示降级路径");
        }
        other => panic!("应为 MigrationBlocked，实际 {other:?}"),
    }
    // 磁盘库已回到迁移前：commands 表在、user_version=1、数据无损
    let conn = Connection::open(&path).unwrap();
    assert_eq!(squirl::db::migrate::user_version(&conn), 1);
    let n: i64 = conn.query_row("SELECT count(*) FROM commands", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "premigrate 快照回滚保住数据");
    // premigrate 桶确有快照
    assert!(bucket(&path, backup::PREMIGRATE_SUFFIX, 1).exists());
}

/// C3c：premigrate 快照生成失败 → 中止启动，绝不带险迁移。
#[test]
fn c3c_premigrate_failure_blocks_startup() {
    let path = temp_db("c3c");
    {
        let (_db, _) = Db::open(&path).unwrap();
    }
    // 把 premigrate 目标位变成只读文件 → VACUUM INTO 目标已存在 → 失败
    // （rotate 会先删 bak-1 再 rename——这里让 rename 失败：目录只读）
    let two_mig = squirl::rusqlite_migration::Migrations::new(vec![
        squirl::rusqlite_migration::M::up(SCHEMA_V1_SQL),
        squirl::rusqlite_migration::M::up("CREATE TABLE extra (v TEXT);"),
    ]);
    // 占位 premigrate-2 为目录：轮换时 remove_file 必失败 → premigrate 快照失败
    std::fs::create_dir_all(bucket(&path, backup::PREMIGRATE_SUFFIX, 2)).unwrap();

    let err = Db::open_with_migrations(&path, &two_mig).unwrap_err();
    assert!(
        matches!(err, squirl::error::AppError::Backup(_)),
        "premigrate 失败必须中止启动（绝不带险迁移），实际 {err:?}"
    );
    // 主库未被动过迁移
    let conn = Connection::open(&path).unwrap();
    assert_eq!(squirl::db::migrate::user_version(&conn), 1);
}

// ---------- C4：退出四步串行（DR-14） ----------

#[test]
fn c4_shutdown_sequence_backup_contains_final_write() {
    let path = temp_db("c4");
    let (db, _) = Db::open(&path).unwrap();
    let pid = insert_project(&db.conn(), Path::new("D:/proj/c4"), "c4").unwrap();
    let c1 = insert_command(&db.conn(), pid, "dev", "npm run dev").unwrap();
    let c2 = insert_command(&db.conn(), pid, "test", "npm test").unwrap();
    let c3 = insert_command(&db.conn(), pid, "open", "wt.exe").unwrap();
    // c2 预置历史退出码：killed 不得覆盖
    db.conn()
        .execute(
            "UPDATE commands SET last_exit_code=9 WHERE id=?1",
            rusqlite::params![c2.to_string()],
        )
        .unwrap();

    let mut reaped = false;
    let finals = vec![
        RunFinal { command_id: c1, project_id: pid, outcome: RunOutcome::Exited(0) },
        RunFinal { command_id: c2, project_id: pid, outcome: RunOutcome::Killed },
        RunFinal { command_id: c3, project_id: pid, outcome: RunOutcome::LaunchedExternal },
    ];
    squirl::db::shutdown_sequence(&db, &finals, || reaped = true).unwrap();
    assert!(reaped, "第①步回收必须先于写回/备份执行");

    // 主库终态（DR-16）
    let conn = db.conn();
    let q_int = |id: Uuid| -> Option<i32> {
        conn.query_row(
            "SELECT last_exit_code FROM commands WHERE id=?1",
            rusqlite::params![id.to_string()],
            |r| r.get(0),
        )
        .unwrap()
    };
    let q_text = |id: Uuid| -> Option<String> {
        conn.query_row(
            "SELECT last_run_at FROM commands WHERE id=?1",
            rusqlite::params![id.to_string()],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(q_int(c1), Some(0));
    assert_eq!(q_int(c2), Some(9), "killed 保留上次值");
    assert_eq!(q_int(c3), None, "外部模式恒 NULL");
    for c in [c1, c2, c3] {
        assert!(q_text(c).is_some());
    }
    let rc: i64 = conn
        .query_row("SELECT run_count FROM projects WHERE id=?1", rusqlite::params![pid.to_string()], |r| r.get(0))
        .unwrap();
    assert_eq!(rc, 3, "双模式均累计（DR-16①）");
    drop(conn);

    // 第③步产物：备份包含最终写回（DR-14 的核心语义）
    let snap = bucket(&path, backup::REGULAR_SUFFIX, 1);
    let b = open_snapshot(&snap);
    let code1: Option<i32> = b
        .query_row(
            "SELECT last_exit_code FROM commands WHERE id=?1",
            rusqlite::params![c1.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(code1, Some(0), "备份必须包含第②步的终态写回");
    let code2: Option<i32> = b
        .query_row(
            "SELECT last_exit_code FROM commands WHERE id=?1",
            rusqlite::params![c2.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(code2, Some(9));
}

/// C4 补充：备份失败不阻断退出（设计 §5.1.2）。
#[test]
fn c4_backup_failure_does_not_block_exit() {
    let path = temp_db("c4b");
    let (db, _) = Db::open(&path).unwrap();
    let pid = insert_project(&db.conn(), Path::new("D:/proj/c4b"), "c4b").unwrap();
    let c = insert_command(&db.conn(), pid, "dev", "npm run dev").unwrap();
    // 让常规桶 1 变成目录 → rename/vacuum 失败
    let target = bucket(&path, backup::REGULAR_SUFFIX, 1);
    let _ = std::fs::remove_file(&target);
    std::fs::create_dir_all(&target).unwrap();

    // 不应报错：备份失败仅记日志，退出继续
    squirl::db::shutdown_sequence(
        &db,
        &[RunFinal { command_id: c, project_id: pid, outcome: RunOutcome::Exited(0) }],
        || {},
    )
    .unwrap();
    // 终态写回仍已生效
    let code: Option<i32> = db
        .conn()
        .query_row(
            "SELECT last_exit_code FROM commands WHERE id=?1",
            rusqlite::params![c.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(code, Some(0));
}
