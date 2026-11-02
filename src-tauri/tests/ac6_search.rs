//! AC6：检索性能——100 项目规模、≤3 字符定位、< 200ms（Windows 实机）。

use std::time::Instant;

use squirl::db::repo::{insert_command, register_project, search_projects};
use squirl::rusqlite::Connection;

#[test]
fn ac6_search_100_projects_under_200ms() {
    let mut conn = Connection::open_in_memory().unwrap();
    squirl::db::migrate::schema_v1_migrations()
        .to_latest(&mut conn)
        .unwrap();

    // 100 个真实登记项目（走 register_project 全链路，含规范化）
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..100u32 {
        let dir = tmp.path().join(format!("project-{i:03}"));
        std::fs::create_dir_all(&dir).unwrap();
        let card = register_project(&conn, &dir, Some(&format!("备注 {i}"))).unwrap();
        if i % 3 == 0 {
            insert_command(&conn, squirl::uuid::Uuid::parse_str(&card.id).unwrap(), "dev", "npm run dev")
                .unwrap();
        }
    }

    // ≤3 字符查询（涵盖 name / note / 命令名三个命中面），各测 5 次取最大
    let queries = ["pro", "00", "dev", "备"];
    let mut worst = std::time::Duration::ZERO;
    for q in queries {
        for _ in 0..5 {
            let t0 = Instant::now();
            let hits = search_projects(&conn, q).unwrap();
            worst = worst.max(t0.elapsed());
            assert!(!hits.is_empty(), "查询 {q:?} 应有命中");
        }
    }
    assert!(
        worst.as_millis() < 200,
        "AC6：100 项目最慢查询 {worst:?} ≥ 200ms"
    );
    // 语义抽查：3 字符前缀命中全部
    assert_eq!(search_projects(&conn, "pro").unwrap().len(), 100);
    assert_eq!(search_projects(&conn, "dev").unwrap().len(), 34);
}
