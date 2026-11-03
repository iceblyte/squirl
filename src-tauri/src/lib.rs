//! Squirl 库入口：本地项目台账与一键启动中枢。
//!
//! 模块布局与设计文档 §3.2 一致；M0（Phase 0）已落危险链路生产模块
//! （engine / db / error / paths），S1 起装配 Tauri（见 docs/进度与验证记录.md ADR-1）。

pub mod clock;
pub mod commands;
pub mod db;
pub mod engine;
pub mod error;
pub mod models;
pub mod paths;
pub mod scanner;

// 依赖类型再导出：集成测试与上层（Tauri commands）共用同一版本
pub use rusqlite;
pub use rusqlite_migration;
pub use uuid;

use crate::db::RecoveryEvent;
use crate::error::AppError;
use tauri::{Emitter, Manager};

/// 单实例回调：二次启动唤起已有主窗口（需求 A 节「单实例运行」）。
fn focus_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 恢复事件 → 前端 toast（契约 `app://db-restored`，AC9）。
fn emit_recovery(app: &tauri::AppHandle, event: RecoveryEvent) {
    if let RecoveryEvent::None = event {
        return;
    }
    let kind = match event {
        RecoveryEvent::RestoredFromBackup => "restored_from_backup",
        RecoveryEvent::RebuiltEmpty => "rebuilt_empty",
        RecoveryEvent::None => return,
    };
    if let Err(e) = app.emit("app://db-restored", serde_json::json!({ "kind": kind })) {
        tracing::warn!(target: "db_record", error = %e, "发送恢复事件失败");
    }
}

/// Tauri Builder 装配（设计 §3.1）。
pub fn run() {
    tauri::Builder::default()
        // 单实例必须最先注册
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app);
        }))
        .plugin(
            // 应用自身诊断日志：滚动写数据目录（与命令日志「不落盘」语义区分，§5.8）
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("squirl".into()),
                    }),
                    #[cfg(debug_assertions)]
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                ])
                .max_file_size(2 * 1024 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = handle.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("squirl.db");

            // DR-6：DB 调用走阻塞上下文；启动期同步打开（仅 settings+schema，快）
            let (db, event) = {
                let db_path = db_path.clone();
                tauri::async_runtime::block_on(async move {
                    tokio::task::spawn_blocking(move || crate::db::Db::open(&db_path))
                        .await
                        .map_err(|e| AppError::Engine(format!("DB 线程失败: {e}")))?
                })
            }?;
            // ADR-4：清扫上次运行残留的 pwsh 启动脚本（数据目录 tmp）
            crate::engine::adapter::sweep_script_dir(&data_dir.join("tmp"));
            tracing::info!(target: "db_record", path = %data_dir.display(), "数据库已打开");
            emit_recovery(&handle, event);

            app.manage(db.clone());
            app.manage(crate::scanner::SharedSession::default());
            // S6：内置模式进程监管（事件直达前端，ADR-2 生产消费端）；
            // 持 DB 句柄做终态实时写回（DR-16②）
            let sink = std::sync::Arc::new(crate::commands::sink::TauriEventSink::new(handle.clone()));
            app.manage(crate::engine::supervisor::Supervisor::with_db(sink, db));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::roots::list_roots,
            commands::roots::add_root,
            commands::roots::remove_root,
            commands::roots::set_root_enabled,
            commands::scan::start_scan,
            commands::scan::resolve_scan_pause,
            commands::scan::get_discovery,
            commands::projects::register_project,
            commands::projects::list_projects,
            commands::projects::search_projects,
            commands::projects::get_project,
            commands::projects::update_project,
            commands::projects::relocate_project,
            commands::projects::delete_project,
            commands::projects::get_project_command_count,
            commands::command_crud::add_command,
            commands::command_crud::update_command,
            commands::command_crud::delete_command,
            commands::command_crud::list_commands,
            commands::run::get_effective_execution_config,
            commands::run::start_command,
            commands::run::start_temp_command,
            commands::run::stop_run,
            commands::run::list_active_runs,
            commands::run::get_run_log,
        ])
        .build(tauri::generate_context!())
        .expect("Squirl 启动失败")
        .run(|app, event| {
            // DR-14 退出四步串行（MVP 无托盘：关窗 = 退出，ExitRequested 即退出请求）：
            // ① 回收全部内置 run（reap_all：并发 stop + 等终态）
            // ② 终态写回——watcher 实时路径已在各终态提交（record_builtin_terminal），
            //    此处无需重复；语义等价性：备份必然包含全部终态（单连接 Mutex 串行）
            // ③ VACUUM INTO 常规备份（失败仅记日志，不阻断退出）
            // ④ 回调返回 = 放行退出
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let started = std::time::Instant::now();
                let db = app.state::<crate::db::Db>().inner().clone();
                let sup = app
                    .try_state::<crate::engine::supervisor::Supervisor>()
                    .map(|s| s.inner().clone());
                tauri::async_runtime::block_on(async move {
                    if let Some(sup) = sup {
                        sup.reap_all().await; // ①
                    }
                    // ② 已由 watcher 实时完成（见上注）
                    let backup = crate::db::backup::snapshot_regular(&db.conn()); // ③
                    match backup {
                        Ok(()) => {
                            tracing::info!(target: "db_record",
                                elapsed_ms = started.elapsed().as_millis() as u64,
                                "退出四步完成（回收→写回→备份→放行）")
                        }
                        Err(e) => {
                            tracing::warn!(target: "db_record", error = %e,
                                "退出备份失败（不阻断退出）")
                        }
                    }
                }); // ④
            }
        });
}
