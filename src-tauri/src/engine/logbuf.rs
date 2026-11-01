//! 命令日志环形缓冲（设计 §5.3.3，DR-7）。
//!
//! 统一口径：**2MB 字节硬顶 + 5000 行软约束**——超限时从头部按字节/行
//! 丢弃最早内容（一次可能丢多行）；`trimmed_lines` 为累计截断行数，
//! 保留区头行 seq 恒等于 `trimmed_lines + 1`，前端据此对齐本地镜像。
//! 行文本为 UTF-8（有损兜底）+ CRLF 归一 LF；ANSI 原文透传不清洗。

use std::collections::VecDeque;

/// 单行归属流。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogLine {
    pub seq: u64,
    pub stream: Stream,
    pub text: String,
}

/// 序列化形态（IPC：`LogLine = { seq, text, stream }`，设计 §5.4.1）。
#[derive(Debug, serde::Serialize)]
pub struct LogLineDto {
    pub seq: u64,
    pub text: String,
    pub stream: Stream,
}

const BYTE_CAP: usize = 2 * 1024 * 1024; // 2MB 字节硬顶
const LINE_CAP_SOFT: usize = 5000; // 5000 行软约束

#[derive(Debug, Default)]
pub struct RingBuffer {
    lines: VecDeque<LogLine>,
    bytes: usize,
    /// 累计从头部截断的行数。
    trimmed_lines: u64,
    /// 已分配的最大 seq。
    next_seq: u64,
}

impl RingBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    fn line_bytes(line: &LogLine) -> usize {
        line.text.len() + 16 // 文本 + 结构近似开销
    }

    /// 追加一行（含超限裁剪）。返回该行 seq。
    pub fn push(&mut self, stream: Stream, text: String) -> u64 {
        let seq = self.next_seq + 1;
        self.next_seq = seq;
        let line = LogLine { seq, stream, text };
        self.bytes += Self::line_bytes(&line);
        self.lines.push_back(line);
        self.trim();
        seq
    }

    fn trim(&mut self) {
        while self.bytes > BYTE_CAP {
            match self.lines.pop_front() {
                Some(l) => {
                    self.bytes -= Self::line_bytes(&l);
                    self.trimmed_lines += 1;
                }
                None => break,
            }
        }
        while self.lines.len() > LINE_CAP_SOFT {
            match self.lines.pop_front() {
                Some(l) => {
                    self.bytes -= Self::line_bytes(&l);
                    self.trimmed_lines += 1;
                }
                None => break,
            }
        }
    }

    pub fn trimmed_lines(&self) -> u64 {
        self.trimmed_lines
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// 快照（面板挂载补发用）：保留行 + 当前累计截断数。
    pub fn snapshot(&self) -> (Vec<LogLine>, u64) {
        (self.lines.iter().cloned().collect(), self.trimmed_lines)
    }

    /// snapshot 的 DTO 形态（契约 `get_run_log → LogLine[]`）。
    pub fn snapshot_dto(&self) -> (Vec<LogLineDto>, u64) {
        let (lines, trimmed) = self.snapshot();
        (
            lines
                .into_iter()
                .map(|l| LogLineDto { seq: l.seq, text: l.text, stream: l.stream })
                .collect(),
            trimmed,
        )
    }
}

/// 文本入缓冲前的归一：CRLF → LF，UTF-8 有损兜底由调用方处理（字节层）。
pub fn normalize_eol(s: &str) -> String {
    if s.contains('\r') {
        s.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        s.to_string()
    }
}

/// marker 行识别（F4「profile 噪音与命令输出分离」的分隔线）。
pub fn is_marker_line(text: &str) -> bool {
    text.contains("squirl command output")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big_line(n: usize) -> String {
        "x".repeat(n)
    }

    #[test]
    fn seq_monotonic_and_trim_alignment() {
        let mut rb = RingBuffer::new();
        for i in 1..=6000u64 {
            rb.push(Stream::Stdout, format!("line-{i}"));
        }
        // 5000 行软约束触发裁剪
        let (lines, trimmed) = rb.snapshot();
        assert_eq!(lines.len(), 5000);
        assert_eq!(trimmed, 1000, "裁剪 1000 行");
        assert_eq!(lines[0].seq, 1001, "头行 seq = trimmed_lines + 1");
        assert_eq!(lines.last().unwrap().seq, 6000);
    }

    #[test]
    fn byte_cap_is_hard() {
        let mut rb = RingBuffer::new();
        // 单行 64KB × 40 = 2.56MB > 2MB 硬顶
        for _ in 0..40 {
            rb.push(Stream::Stdout, big_line(64 * 1024));
        }
        let (lines, trimmed) = rb.snapshot();
        assert!(lines.len() < 40, "必须发生字节裁剪");
        // 剩余内容字节量必须低于硬顶（近似：文本长 + 开销）
        let total: usize = lines.iter().map(|l| l.text.len() + 16).sum();
        assert!(total <= BYTE_CAP, "字节硬顶必须恒成立");
        assert_eq!(lines[0].seq, trimmed + 1);
    }

    #[test]
    fn normalize_eol_variants() {
        assert_eq!(normalize_eol("a\r\nb\rc\n"), "a\nb\nc\n");
        assert_eq!(normalize_eol("plain"), "plain");
    }

    #[test]
    fn marker_detection() {
        assert!(is_marker_line(&format!("Write-Output '{}'", super::super::quoting::MARKER_TEXT)));
        assert!(is_marker_line("---- squirl command output ----"));
        assert!(!is_marker_line("normal output"));
    }
}
