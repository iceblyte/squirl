//! 路径层（设计 §5.1.3，INV-4）。
//!
//! INV-4：所有路径入库前必须经 `canonical` + `normalize_key`：
//! - `canonical`：绝对路径 + 平台原生分隔符（Windows 用 dunce 避免无谓 `\\?\` 前缀）。
//! - `normalize_key`：canonical 基础上做平台大小写折叠（Windows / macOS 折叠小写、
//!   Linux 保留大小写），作为 `path_normalized` 唯一约束的键。
//! - Windows：长度 > 260 的 fs 访问加 `\\?\` 前缀（`ensure_long`）。
//!
//! 已知限制（设计 §5.1.3 ⚠️）：macOS NFD 与 Windows NFC 的 Unicode 差异
//! 理论上影响折叠后的等价判断；MVP 按「同机自登记」场景处理，不做 Unicode 正规化。

use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};

/// Windows 长路径阈值。
#[cfg(windows)]
const MAX_PATH: usize = 260;

/// 规范化为绝对路径 + 原生分隔符。
///
/// 路径不存在时（登记先于落盘 / 已被移动）退化为词法绝对化：
/// 相对路径按当前目录拼装、清理 `.`/`..`，不做 fs 解析。
/// 这保证 `relocate_project`、失效条目等场景仍能入库唯一键。
pub fn canonical(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(AppError::Path("空路径".into()));
    }
    match dunce::canonicalize(path) {
        Ok(p) => Ok(p),
        Err(_) => {
            // 不存在或不可解析：词法绝对化
            if path.is_absolute() {
                Ok(lexically_clean(path))
            } else {
                let cwd = std::env::current_dir()
                    .map_err(|e| AppError::Path(format!("无法获取当前目录: {e}")))?;
                Ok(lexically_clean(&cwd.join(path)))
            }
        }
    }
}

/// 规范化唯一键（入库 `path_normalized` 列）：canonical + 平台大小写折叠。
pub fn normalize_key(path: &Path) -> Result<String> {
    let canon = canonical(path)?;
    let s = canon.to_string_lossy();
    Ok(match std::env::consts::OS {
        "windows" | "macos" => s.to_lowercase(),
        _ => s.to_string(),
    })
}

/// 归属派生（设计 §5.1.4，不落库）：
/// 归属根 = `path_norm` 命中的最长根前缀；前缀匹配必须落在分隔符边界
/// （防 `D:\GitHub` 误吞 `D:\GitHub2`）；无命中 → None（无归属）。
pub fn attribution<'a>(path_norm: &str, roots: &'a [(String, &'a str)]) -> Option<&'a str> {
    roots
        .iter()
        .filter(|(root_norm, _)| {
            path_norm == root_norm.as_str()
                || path_norm.starts_with(&format!("{root_norm}{}", std::path::MAIN_SEPARATOR))
                // 键已折叠为小写（Win/macOS）但分隔符仍为平台原生；
                // 兼容键内残留的正斜杠形态
                || (std::path::MAIN_SEPARATOR != '/'
                    && path_norm.starts_with(&format!("{root_norm}/")))
        })
        .max_by_key(|(root_norm, _)| root_norm.len())
        .map(|(_, root_id)| *root_id)
}

/// Windows：fs 访问用——长度 > 260 加 `\\?\` 前缀（已带前缀或 UNC 时不动）。
#[cfg(windows)]
pub fn ensure_long(path: &Path) -> PathBuf {
    let s = path.as_os_str().to_string_lossy();
    if s.len() > MAX_PATH && !s.starts_with(r"\\?\") {
        if let Some(rest) = s.strip_prefix(r"\\") {
            // UNC：\\?\UNC\server\share\...
            return PathBuf::from(format!(r"\\?\UNC\{rest}"));
        }
        return PathBuf::from(format!(r"\\?\{s}"));
    }
    path.to_path_buf()
}

/// 词法清理 `.` / `..`（不触 fs）。
fn lexically_clean(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_becomes_absolute_native() {
        let p = canonical(Path::new("docs")).unwrap();
        assert!(p.is_absolute());
        assert!(p.ends_with("docs"));
    }

    #[test]
    fn empty_path_is_error() {
        assert!(canonical(Path::new("")).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn case_insensitive_key_on_windows() {
        let tmp = tempfile::tempdir().unwrap();
        let mixed = tmp.path().join("MixedCase Dir");
        std::fs::create_dir_all(&mixed).unwrap();
        let lower = tmp.path().join("mixedcase dir");

        let k1 = normalize_key(&mixed).unwrap();
        let k2 = normalize_key(&lower).unwrap();
        assert_eq!(k1, k2, "Windows 键必须折叠小写");
        assert_eq!(k1, k1.to_lowercase());
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_prefix_is_stripped() {
        let tmp = tempfile::tempdir().unwrap();
        let verbatim = PathBuf::from(format!(r"\\?\{}", tmp.path().display()));
        let p = canonical(&verbatim).unwrap();
        assert!(
            !p.to_string_lossy().starts_with(r"\\?\"),
            "dunce 应剥掉无谓的 \\\\?\\ 前缀: {p:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn nonexistent_path_falls_back_lexical() {
        let p = canonical(Path::new(r"C:\surely\does\not\exist\SquirlM0")).unwrap();
        assert_eq!(
            normalize_key(&p).unwrap(),
            r"c:\surely\does\not\exist\squirlm0"
        );
    }

    #[cfg(windows)]
    #[test]
    fn long_path_gets_verbatim_prefix() {
        let deep = PathBuf::from(format!(r"C:\{}", "a\\".repeat(140)));
        let p = ensure_long(&deep);
        assert!(p.as_os_str().to_string_lossy().starts_with(r"\\?\"));
        // 已带前缀的不再重复加
        assert_eq!(ensure_long(&p), p);
    }

    #[test]
    fn attribution_longest_root_wins() {
        let roots = vec![
            (r"c:\github".to_string(), "root-1"),
            (r"c:\github\deep".to_string(), "root-2"),
        ];
        assert_eq!(
            attribution(r"c:\github\deep\project", &roots),
            Some("root-2"),
            "嵌套根归更深者"
        );
        assert_eq!(attribution(r"c:\github\other", &roots), Some("root-1"));
        // 边界：前缀必须落在分隔符上，防 D:\GitHub 误吞 D:\GitHub2
        assert_eq!(attribution(r"c:\github2\x", &roots), None);
        assert_eq!(attribution(r"d:\elsewhere", &roots), None);
    }
}
