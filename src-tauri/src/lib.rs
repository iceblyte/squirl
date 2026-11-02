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
            tracing::info!(target: "db_record", path = %data_dir.display(), "数据库已打开");
            emit_recovery(&handle, event);

            app.manage(db);
            app.manage(crate::scanner::SharedSession::default());
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
        ])
        .run(tauri::generate_context!())
        .expect("Squirl 启动失败");
}
