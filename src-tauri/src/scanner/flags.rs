//! 标志文件探测（设计 §5.2：命中即高亮，不做过滤——「识别是猜测，记录是事实」）。

use std::path::Path;

/// 需求 F1 列举的标志文件。
pub const FLAG_FILES: &[&str] = &[
    "package.json",
    "pyproject.toml",
    "Cargo.toml",
    "go.mod",
    "pom.xml",
    "Makefile",
    ".git",
];

/// 返回命中的标志文件名列表（探测存在性，命中即带 flags 字段）。
pub fn detect_flags(dir: &Path) -> Vec<String> {
    FLAG_FILES
        .iter()
        .filter(|f| dir.join(f).exists())
        .map(|f| (*f).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_present_flags_only() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("package.json"), "{}").unwrap();
        std::fs::write(tmp.path().join("Makefile"), "").unwrap();
        let flags = detect_flags(tmp.path());
        assert_eq!(flags, vec!["package.json", "Makefile"]);
    }

    #[test]
    fn no_flags_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(detect_flags(tmp.path()).is_empty());
    }
}
