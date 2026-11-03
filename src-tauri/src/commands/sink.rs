//! Tauri 事件汇：M0 管道协议（ADR-2）的生产消费端实现。
//! `run://log`（30ms 批）与 `run://state` 直达前端（契约 §5.4.2）。

use tauri::{Emitter, Manager};

use crate::engine::pipeline::{EventSink, LogBatch, RunStateEvent};

pub struct TauriEventSink {
    app: tauri::AppHandle,
}

impl TauriEventSink {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl EventSink for TauriEventSink {
    fn on_log_batch(&self, batch: LogBatch) {
        if let Err(e) = self.app.emit("run://log", &batch) {
            tracing::warn!(target: "run", error = %e, "发送 run://log 失败");
        }
    }

    fn on_state(&self, event: RunStateEvent) {
        if let Err(e) = self.app.emit("run://state", &event) {
            tracing::warn!(target: "run", error = %e, "发送 run://state 失败");
        }
    }

    fn on_input_suspected(&self, run_id: String) {
        // DR-8：疑似等待输入（只提示不干预；前端 toast + 强制停止 action）
        if let Err(e) = self
            .app
            .emit("run://input-suspected", serde_json::json!({ "runId": run_id }))
        {
            tracing::warn!(target: "run", error = %e, "发送 run://input-suspected 失败");
        }
    }
}

/// 应用退出时回收全部内置 run（DR-14 第①步的钩子挂载点在 S9；
/// 此处提供注册表访问助手）。
pub fn active_run_count(app: &tauri::AppHandle) -> usize {
    app.state::<crate::engine::supervisor::Supervisor>()
        .registry
        .list()
        .len()
}
