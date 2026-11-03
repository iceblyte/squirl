//! 配置快填引擎（P2-⑧，DR-26）：解析 .env / TOML / YAML 顶层键值，
//! 保格式原子重写（不重排、不删行、保留注释与引号风格），README 命令提取。
//!
//! 硬承诺：`apply_config` 后文件里除目标键的值与新增占位行外，其余字节
//! 原样保留（注释/缩进/键序/空行）。原子性：临时文件 + rename。

use std::path::Path;

use crate::error::{AppError, Result};

/// 解析出的键（含原文件行号，0 起）。
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigKey {
    pub key: String,
    pub value: String,
    /// 首个定义行号（供 UI 定位）。
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigDialect {
    Env,
    Toml,
    Yaml,
}

/// 按扩展名判方言。
pub fn dialect_of(path: &Path) -> Option<ConfigDialect> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name.starts_with(".env") || name.ends_with(".env") {
        return Some(ConfigDialect::Env);
    }
    match path.extension()?.to_string_lossy().to_lowercase().as_str() {
        "toml" => Some(ConfigDialect::Toml),
        "yml" | "yaml" => Some(ConfigDialect::Yaml),
        _ => None,
    }
}

/// 剥引号（保留原引号风格信息由调用方按需处理）。
fn unquote(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 2
        && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')))
    {
        v[1..v.len() - 1].to_string()
    } else {
        v.to_string()
    }
}

/// 解析键值（逐行扫描，只取顶层/扁平键；键序 = 出现序）。
pub fn parse_config(content: &str, dialect: ConfigDialect) -> Vec<ConfigKey> {
    let mut out = Vec::new();
    let mut in_toml_section = false;
    for (i, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match dialect {
            ConfigDialect::Env => {
                if let Some(eq) = line.find('=') {
                    let key = line[..eq].trim();
                    if !key.is_empty() {
                        out.push(ConfigKey {
                            key: key.to_string(),
                            value: unquote(&line[eq + 1..]),
                            line: i,
                        });
                    }
                }
            }
            ConfigDialect::Toml | ConfigDialect::Yaml => {
                // 顶层键：非缩进行 + key = value (TOML) / key: value (YAML)
                if raw.starts_with(' ') || raw.starts_with('\t') {
                    continue;
                }
                // TOML 段状态机：[段] 之后直至下一段的键均属段内，不入顶层
                if dialect == ConfigDialect::Toml {
                    if line.starts_with('[') {
                        in_toml_section = true;
                        continue;
                    }
                    if in_toml_section {
                        continue;
                    }
                }
                let sep = if dialect == ConfigDialect::Toml { '=' } else { ':' };
                if let Some(eq) = line.find(sep) {
                    let key = line[..eq].trim().trim_matches('"');
                    let value = line[eq + 1..]
                        .split('#')
                        .next()
                        .unwrap_or("")
                        .trim();
                    if !key.is_empty() && !key.contains(' ') {
                        out.push(ConfigKey {
                            key: key.to_string(),
                            value: unquote(value),
                            line: i,
                        });
                    }
                }
            }
        }
    }
    out
}

/// 行内值替换：仅替换首个 `key = value` / `key: value` 的值部分，
/// 保留键、等号/冒号周围空格、引号风格（若原值带引号则新值也包引号）与行尾注释。
fn replace_value_in_line(line: &str, dialect: ConfigDialect, new_value: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let sep = match dialect {
        ConfigDialect::Toml | ConfigDialect::Env => '=',
        ConfigDialect::Yaml => ':',
    };
    let eq = line.find(sep)?;
    let key_part = &line[..=eq];
    let key = line[..eq].trim().trim_matches('"');
    if key.is_empty() || key.contains(' ') {
        return None;
    }
    let rest = &line[eq + 1..];
    // 行尾注释（YAML/TOML 的 #；.env 无行尾注释语义，保守不动）
    let (value_part, comment) = if dialect == ConfigDialect::Env {
        (rest, "")
    } else if let Some(hash) = rest.find(" #") {
        (&rest[..hash], &rest[hash..])
    } else {
        (rest, "")
    };
    let v = value_part.trim();
    let quoted = (v.starts_with('"') && v.ends_with('"') && v.len() >= 2)
        || (v.starts_with('\'') && v.ends_with('\'') && v.len() >= 2);
    let needs_quote = new_value.contains(' ') || new_value.contains('#');
    let new_val = if quoted || needs_quote {
        format!("\"{new_value}\"")
    } else {
        new_value.to_string()
    };
    // 保留等号后的前导空格风格
    let lead = value_part.len() - value_part.trim_start().len();
    let lead_ws = &value_part[..lead];
    Some(format!("{key_part}{lead_ws}{new_val}{comment}"))
}

/// 保格式应用键值：已有键原位更新；缺失键追加占位行（`# key=` 风格注释 + 赋值行）
/// 到文件尾。返回新内容；`changed` = 是否有实际修改。
pub fn apply_config(
    content: &str,
    dialect: ConfigDialect,
    values: &[(String, String)],
) -> (String, bool) {
    let parsed = parse_config(content, dialect);
    let existing: std::collections::HashSet<String> =
        parsed.iter().map(|k| k.key.clone()).collect();
    // 键 → 首个定义行号（parse_config 已提供，避免逐行二次解析）
    let mut key_line: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for k in &parsed {
        key_line.entry(k.key.as_str()).or_insert(k.line);
    }
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    let mut changed = false;

    for (key, value) in values {
        if let Some(&ln) = key_line.get(key.as_str()) {
            let new_opt = replace_value_in_line(&lines[ln], dialect, value);
            if let Some(new_line) = new_opt
                && new_line != lines[ln]
            {
                lines[ln] = new_line;
                changed = true;
            }
        } else if !existing.contains(key) {
            let sep = if dialect == ConfigDialect::Toml {
                " = "
            } else if dialect == ConfigDialect::Yaml {
                ": "
            } else {
                "="
            };
            lines.push(format!("# {key}={value}"));
            lines.push(format!("{key}{sep}{value}"));
            changed = true;
        }
    }
    let mut out = lines.join("\n");
    if content.ends_with('\n') {
        out.push('\n');
    }
    (out, changed)
}

/// 原子写回：写临时文件 + rename；改前备份 `.<name>.squirl-bak`
/// （INV-1 例外：用户明确操作的目标配置文件）。
pub fn atomic_write(path: &Path, new_content: &str) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| AppError::Path("配置文件无父目录".into()))?;
    let name = path
        .file_name()
        .ok_or_else(|| AppError::Path("配置文件名解析失败".into()))?;
    let bak = dir.join(format!("{}.squirl-bak", name.to_string_lossy()));
    if path.exists() {
        std::fs::copy(path, &bak)
            .map_err(|e| AppError::Path(format!("写备份失败: {e}")))?;
    }
    let tmp = dir.join(format!("{}.squirl-tmp", name.to_string_lossy()));
    std::fs::write(&tmp, new_content)
        .map_err(|e| AppError::Path(format!("写临时文件失败: {e}")))?;
    std::fs::rename(&tmp, path).map_err(|e| AppError::Path(format!("原子替换失败: {e}")))?;
    Ok(())
}

/// README 提取 fenced code blocks 中的命令行（bash/sh/无标注），去重保序。
pub fn scan_readme_commands(readme: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut in_block = false;
    let mut lang_ok = false;
    for line in readme.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            if in_block {
                in_block = false;
            } else {
                in_block = true;
                let lang = t.trim_start_matches('`').trim();
                lang_ok = lang.is_empty() || lang == "bash" || lang == "sh" || lang == "shell";
            }
            continue;
        }
        if in_block
            && lang_ok
            && !t.is_empty()
            && !t.starts_with('#')
            && !t.starts_with('<')
        {
            let cmd = t.strip_prefix("$ ").unwrap_or(t).trim();
            if !cmd.is_empty() && !out.iter().any(|c| c == cmd) {
                out.push(cmd.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENV_SAMPLE: &str = "# 数据库配置\nDB_HOST=localhost\nDB_PORT=5432\n\n# 应用\nAPP_NAME=\"my app\"\n";

    #[test]
    fn parse_env_keys_in_order() {
        let keys = parse_config(ENV_SAMPLE, ConfigDialect::Env);
        let names: Vec<&str> = keys.iter().map(|k| k.key.as_str()).collect();
        assert_eq!(names, vec!["DB_HOST", "DB_PORT", "APP_NAME"]);
        assert_eq!(keys[2].value, "my app", "引号剥离");
        assert_eq!(keys[0].line, 1, "行号定位");
    }

    #[test]
    fn apply_updates_in_place_preserving_format() {
        let (out, changed) =
            apply_config(ENV_SAMPLE, ConfigDialect::Env, &[("DB_PORT".into(), "6543".into())]);
        assert!(changed);
        assert!(out.contains("DB_HOST=localhost"), "未触及键原样");
        assert!(out.contains("DB_PORT=6543"), "目标键原位更新");
        assert!(out.contains("# 数据库配置"), "注释保留");
        assert!(out.starts_with("# 数据库配置\n"), "键序不变");
        assert!(out.ends_with("APP_NAME=\"my app\"\n"), "引号风格保留");
    }

    #[test]
    fn apply_appends_missing_keys() {
        let (out, changed) =
            apply_config(ENV_SAMPLE, ConfigDialect::Env, &[("NEW_KEY".into(), "v1".into())]);
        assert!(changed);
        assert!(out.contains("# NEW_KEY=v1") && out.contains("NEW_KEY=v1"), "占位 + 赋值");
        assert!(out.contains("DB_HOST=localhost"), "原文不动");
    }

    #[test]
    fn apply_no_change_is_reported() {
        let (_, changed) =
            apply_config(ENV_SAMPLE, ConfigDialect::Env, &[("DB_HOST".into(), "localhost".into())]);
        assert!(!changed, "同值写入不算变更");
    }

    #[test]
    fn toml_and_yaml_top_level() {
        let toml = parse_config("name = \"app\"\n[dependencies]\nserde = \"1\"\n", ConfigDialect::Toml);
        assert_eq!(toml.len(), 1, "段内键不入顶层");
        assert_eq!(toml[0].key, "name");

        let yaml = parse_config("host: localhost\n  child: 1\nport: 5432 # 注释\n", ConfigDialect::Yaml);
        assert_eq!(yaml.len(), 2, "缩进行跳过");
        assert_eq!(yaml[1].value, "5432", "行尾注释剥离");
    }

    #[test]
    fn yaml_value_update_preserves_comment() {
        let src = "port: 5432 # 数据库端口\n";
        let (out, _) = apply_config(src, ConfigDialect::Yaml, &[("port".into(), "6543".into())]);
        assert_eq!(out, "port: 6543 # 数据库端口\n", "行尾注释保留");
    }

    #[test]
    fn atomic_write_backs_up_and_replaces() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join(".env");
        std::fs::write(&p, "A=1\n").unwrap();
        atomic_write(&p, "A=2\n").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "A=2\n");
        let bak = tmp.path().join(".env.squirl-bak");
        assert_eq!(std::fs::read_to_string(&bak).unwrap(), "A=1\n", "备份留存");
        assert!(
            !tmp.path().join(".env.squirl-tmp").exists(),
            "临时文件不残留"
        );
    }

    #[test]
    fn readme_command_extraction() {
        let md = "# Demo\n\n```bash\nnpm install\nnpm run dev\n```\n\n```python\nprint(1)\n```\n\n```\n$ cargo run\n```\n";
        let cmds = scan_readme_commands(md);
        assert_eq!(cmds, vec!["npm install", "npm run dev", "cargo run"]);
    }
}
