//! 日志管道（设计 §5.3.3）：stdout/stderr 读任务 → mpsc 汇聚 →
//! 「30ms 或 64KB 先到先发」聚合 → EventSink 批量推送 + 环形缓冲落账。
//!
//! ADR-2：M0 阶段消费端以 `EventSink` trait 注入（测试消费者），
//! Tauri emit 实装归 S7；管道协议（LogBatch 载荷）与契约 §5.4.2 对齐。

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::mpsc;

use crate::engine::logbuf::{is_marker_line, normalize_eol, RingBuffer, Stream};
use crate::error::Result;

/// 批次间隔（30ms，需求 F5）。
pub const FLUSH_INTERVAL: Duration = Duration::from_millis(30);
/// 字节先发阈值（64KB，设计 §5.3.3）。
pub const FLUSH_BYTES: usize = 64 * 1024;

/// run://log 事件载荷（§5.4.2）。`seq` = 批内首行 seq；
/// `trimmed_lines` = 推送时刻环形缓冲累计截断行数（前端按 seq 对齐裁剪镜像）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogBatch {
    pub run_id: String,
    pub seq: u64,
    pub trimmed_lines: u64,
    pub lines: Vec<LogLineOut>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LogLineOut {
    pub text: String,
    pub stream: Stream,
}

/// run://state 事件载荷（§5.4.2）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RunStateEvent {
    pub run_id: String,
    pub command_id: Option<String>,
    /// idle/starting/running/exited/failed/killed
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub ts: String,
}

/// 事件消费端抽象（ADR-2）。生产实现（S7）= Tauri emit。
pub trait EventSink: Send + Sync {
    fn on_log_batch(&self, batch: LogBatch);
    fn on_state(&self, event: RunStateEvent);
    /// DR-8：疑似等待输入（running 且 30s 无输出；只提示不干预）。
    fn on_input_suspected(&self, _run_id: String) {}
}

struct LineMsg {
    stream: Stream,
    text: String,
}

/// 管道句柄：marker 槽（聚合中即时写入）+ 完成信号。
pub struct PipeHandle {
    /// marker 行 seq（首次出现即写入；无 marker = None）。
    pub marker: Arc<std::sync::Mutex<Option<u64>>>,
    /// 读端全部关闭、缓冲终发后的完成信号。
    pub done: tokio::task::JoinHandle<()>,
}

/// 为一个 run 装配读管道：两个读任务 + 一个聚合任务（后台运行，不阻塞调用方）。
pub fn spawn_pipe(
    run_id: String,
    stdout: impl AsyncRead + Unpin + Send + 'static,
    stderr: impl AsyncRead + Unpin + Send + 'static,
    ring: Arc<std::sync::Mutex<RingBuffer>>,
    sink: Arc<dyn EventSink>,
) -> Result<PipeHandle> {
    spawn_pipe_with_clock(run_id, stdout, stderr, ring, sink, None)
}

/// 同上，附加 DR-8 输出时钟（聚合器每次 flush 刷新；毫秒自 spawn 起算由调用方解释）。
pub fn spawn_pipe_with_clock(
    run_id: String,
    stdout: impl AsyncRead + Unpin + Send + 'static,
    stderr: impl AsyncRead + Unpin + Send + 'static,
    ring: Arc<std::sync::Mutex<RingBuffer>>,
    sink: Arc<dyn EventSink>,
    output_clock: Option<Arc<std::sync::atomic::AtomicU64>>,
) -> Result<PipeHandle> {
    let (tx, rx) = mpsc::channel::<LineMsg>(4096);
    tokio::spawn(reader_task(run_id.clone(), Stream::Stdout, stdout, tx.clone()));
    tokio::spawn(reader_task(run_id.clone(), Stream::Stderr, stderr, tx));
    let marker_slot = Arc::new(std::sync::Mutex::new(None));
    let done = tokio::spawn(aggregator_task(
        run_id, rx, ring, sink, marker_slot.clone(), output_clock,
    ));
    Ok(PipeHandle { marker: marker_slot, done })
}

async fn reader_task(
    run_id: String,
    stream: Stream,
    mut reader: impl AsyncRead + Unpin,
    tx: mpsc::Sender<LineMsg>,
) {
    let _ = run_id; // 诊断用途预留
    let mut buf = [0u8; 8192];
    let mut carry: Vec<u8> = Vec::new();
    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                carry.extend_from_slice(&buf[..n]);
                // 按 \n 切行；残段留给下一轮
                while let Some(pos) = carry.iter().position(|&b| b == b'\n') {
                    let mut line: Vec<u8> = carry.drain(..=pos).collect();
                    line.pop(); // 去掉换行符本身
                    if line.last() == Some(&b'\r') {
                        line.pop(); // CRLF：先去 \r 再归一内部残留
                    }
                    let text = decode_line(&line);
                    if tx.send(LineMsg { stream, text }).await.is_err() {
                        return;
                    }
                }
            }
            Err(_) => break,
        }
    }
    if !carry.is_empty() {
        let text = decode_line(&carry);
        let _ = tx.send(LineMsg { stream, text }).await;
    }
}

/// UTF-8 有损兜底 + CRLF 归一。
fn decode_line(raw: &[u8]) -> String {
    normalize_eol(&String::from_utf8_lossy(raw))
}

/// 刷新最后输出时刻（DR-8 时钟；Unix 毫秒）。
fn touch_clock(clock: &Option<Arc<std::sync::atomic::AtomicU64>>) {
    if let Some(c) = clock {
        c.store(crate::clock::now_unix_millis(), std::sync::atomic::Ordering::SeqCst);
    }
}

#[allow(clippy::too_many_arguments)]
async fn aggregator_task(
    run_id: String,
    mut rx: mpsc::Receiver<LineMsg>,
    ring: Arc<std::sync::Mutex<RingBuffer>>,
    sink: Arc<dyn EventSink>,
    marker_slot: Arc<std::sync::Mutex<Option<u64>>>,
    output_clock: Option<Arc<std::sync::atomic::AtomicU64>>,
) {
    let mut marker_seq: Option<u64> = None;
    let mut pending: Vec<LineMsg> = Vec::new();
    let mut pending_bytes = 0usize;
    loop {
        let msg = if pending.is_empty() {
            // 空批：阻塞等数据（None = 全部读端关闭）
            rx.recv().await
        } else {
            // 已有积压：30ms 内的后续消息并入同批，到点先发
            tokio::select! {
                m = rx.recv() => m,
                _ = tokio::time::sleep(FLUSH_INTERVAL) => None,
            }
        };
        match msg {
            Some(m) => {
                pending_bytes += m.text.len();
                pending.push(m);
                if pending_bytes >= FLUSH_BYTES {
                    flush(&run_id, &mut pending, &ring, &sink, &mut marker_seq, &marker_slot);
                    touch_clock(&output_clock);
                    pending_bytes = 0;
                }
            }
            None => {
                // 通道关闭，或 30ms 到点。积压非空则终发；
                // 关闭且已排空时下一轮 recv 仍为 None → 退出。
                if pending.is_empty() {
                    break;
                }
                flush(&run_id, &mut pending, &ring, &sink, &mut marker_seq, &marker_slot);
                touch_clock(&output_clock);
                pending_bytes = 0;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn flush(
    run_id: &str,
    pending: &mut Vec<LineMsg>,
    ring: &Arc<std::sync::Mutex<RingBuffer>>,
    sink: &Arc<dyn EventSink>,
    marker_seq: &mut Option<u64>,
    marker_slot: &Arc<std::sync::Mutex<Option<u64>>>,
) {
    if pending.is_empty() {
        return;
    }
    let mut base_seq = 0u64;
    let mut lines_out = Vec::with_capacity(pending.len());
    let trimmed;
    {
        let mut r = ring.lock().unwrap();
        for m in pending.drain(..) {
            let text = m.text;
            let seq = r.push(m.stream, text.clone());
            if base_seq == 0 {
                base_seq = seq;
            }
            if marker_seq.is_none() && is_marker_line(&text) {
                *marker_seq = Some(seq);
                *marker_slot.lock().unwrap() = Some(seq);
            }
            lines_out.push(LogLineOut { text, stream: m.stream });
        }
        trimmed = r.trimmed_lines();
    }
    sink.on_log_batch(LogBatch {
        run_id: run_id.to_string(),
        seq: base_seq,
        trimmed_lines: trimmed,
        lines: lines_out,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::logbuf::LogLine;
    use std::sync::Mutex as StdMutex;

    #[derive(Default)]
    struct CollectSink {
        batches: StdMutex<Vec<LogBatch>>,
        states: StdMutex<Vec<RunStateEvent>>,
    }

    impl EventSink for CollectSink {
        fn on_log_batch(&self, batch: LogBatch) {
            self.batches.lock().unwrap().push(batch);
        }
        fn on_state(&self, event: RunStateEvent) {
            self.states.lock().unwrap().push(event);
        }
    }

    fn cursor(data: &'static [u8]) -> impl AsyncRead + Unpin {
        std::io::Cursor::new(data.to_vec())
    }

    #[tokio::test]
    async fn pipe_splits_lines_and_marks_marker() {
        let ring = Arc::new(std::sync::Mutex::new(RingBuffer::new()));
        let sink = Arc::new(CollectSink::default());
        let out = b"profile noise\n---- squirl command output ----\ncmd output\n";
        let pipe = spawn_pipe("r1".into(), cursor(out), cursor(b""), ring.clone(), sink.clone())
            .unwrap();
        pipe.done.await.unwrap();
        let (lines, trimmed) = ring.lock().unwrap().snapshot();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text, "profile noise");
        assert_eq!(lines[2].text, "cmd output");
        assert_eq!(trimmed, 0);
        // marker 在第 2 行（seq=2），sink 收到同一批
        let batches = sink.batches.lock().unwrap();
        assert_eq!(batches.len(), 1, "小数据量应并入单批");
        assert_eq!(batches[0].seq, 1);
        assert_eq!(batches[0].lines.len(), 3);
        assert_eq!(batches[0].lines[1].text, "---- squirl command output ----");
        assert_eq!(*pipe.marker.lock().unwrap(), Some(2));
    }

    #[tokio::test]
    async fn pipe_normalizes_crlf_and_streams_are_tagged() {
        let ring = Arc::new(std::sync::Mutex::new(RingBuffer::new()));
        let sink = Arc::new(CollectSink::default());
        let pipe = spawn_pipe(
            "r2".into(),
            cursor(b"out-1\r\nout-2\r\n"),
            cursor(b"err-1\r\n"),
            ring.clone(),
            sink.clone(),
        )
        .unwrap();
        pipe.done.await.unwrap();
        let (lines, _) = ring.lock().unwrap().snapshot();
        assert_eq!(lines[0], LogLine { seq: 1, stream: Stream::Stdout, text: "out-1".into() });
        assert_eq!(lines[2], LogLine { seq: 3, stream: Stream::Stderr, text: "err-1".into() });
    }

    #[tokio::test]
    async fn pipe_no_final_newline_still_emits() {
        let ring = Arc::new(std::sync::Mutex::new(RingBuffer::new()));
        let sink = Arc::new(CollectSink::default());
        let pipe = spawn_pipe("r3".into(), cursor(b"no-newline-tail"), cursor(b""), ring.clone(), sink.clone())
            .unwrap();
        pipe.done.await.unwrap();
        let (lines, _) = ring.lock().unwrap().snapshot();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "no-newline-tail");
    }

    /// 回归：30ms 空闲间隙 flush 后，后续消息不得丢失
    /// （曾因 None 分支 recv 丢弃消息，导致 marker / 慢启动命令输出消失）。
    #[tokio::test]
    async fn pipe_does_not_drop_after_idle_gap() {
        let ring = Arc::new(std::sync::Mutex::new(RingBuffer::new()));
        let sink = Arc::new(CollectSink::default());
        let (mut w, r) = tokio::io::duplex(64);
        let pipe = spawn_pipe("r4".into(), r, cursor(b""), ring.clone(), sink.clone()).unwrap();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let _ = w.write_all(b"burst-one-1\nburst-one-2\n").await;
            tokio::time::sleep(Duration::from_millis(120)).await; // > 30ms 窗口
            let _ = w.write_all(b"after-gap-1\nafter-gap-2\n").await;
            tokio::time::sleep(Duration::from_millis(120)).await;
            let _ = w.write_all(b"final\n").await;
        });
        pipe.done.await.unwrap();
        let (lines, _) = ring.lock().unwrap().snapshot();
        let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["burst-one-1", "burst-one-2", "after-gap-1", "after-gap-2", "final"],
            "空闲间隙 flush 后的消息必须全部送达"
        );
    }
}
