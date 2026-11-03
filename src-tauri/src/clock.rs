//! 仅依赖 std 的时钟工具：RFC3339(UTC) 时间戳。
//! （不引入 chrono；UUID v7 自带毫秒时间戳，此处做 civil 转换。）

/// 当前 Unix 毫秒（DR-8 输出时钟用）。
pub fn now_unix_millis() -> u64 {
    let ts = uuid::Uuid::now_v7()
        .get_timestamp()
        .expect("v7 UUID 必带时间戳");
    let (secs, nanos) = ts.to_unix();
    secs * 1000 + u64::from(nanos) / 1_000_000
}

/// 当前时间的 RFC3339 / UTC 字符串（秒级精度 + Z 后缀）。
pub fn now_rfc3339() -> String {
    let ts = uuid::Uuid::now_v7()
        .get_timestamp()
        .expect("v7 UUID 必带时间戳");
    let (secs, _) = ts.to_unix();
    rfc3339_from_unix(secs as i64)
}

pub fn rfc3339_from_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Howard Hinnant civil_from_days：Unix 天数 → (年, 月, 日)。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_epochs() {
        assert_eq!(rfc3339_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_from_unix(1_700_000_000), "2023-11-14T22:13:20Z");
        // 闰年边界：2024-02-29T00:00:00Z = 1709164800
        assert_eq!(rfc3339_from_unix(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn now_is_well_formed() {
        let s = now_rfc3339();
        assert_eq!(s.len(), 20);
        assert!(s.ends_with('Z'));
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[10..11], "T");
    }
}
