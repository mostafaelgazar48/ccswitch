//! Just enough date handling for ccswitch, without a date library.

use std::time::{Duration, SystemTime};

const DAY: u64 = 86_400;

/// A UTC calendar date as `YYYY-MM-DD`, the same prefix Claude's log timestamps start with.
pub fn day(t: SystemTime) -> String {
    let days = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / DAY;
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Parses `--since`: `7d` (today and the 6 days before, UTC), `YYYY-MM-DD`, or `all`. Returns the
/// first day to include and the moment it starts.
pub fn since(text: &str, now: SystemTime) -> Option<(String, SystemTime)> {
    let start_days = if text == "all" {
        0
    } else if let Some(n) = text.strip_suffix('d') {
        let n: u64 = n.parse().ok().filter(|n| *n > 0)?;
        let today = now.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs() / DAY;
        today.saturating_sub(n - 1)
    } else {
        let mut parts = text.splitn(3, '-').map(str::parse::<u32>);
        let (y, m, d) = (
            parts.next()?.ok()?,
            parts.next()?.ok()?,
            parts.next()?.ok()?,
        );
        let valid =
            text.len() == 10 && (1..=12).contains(&m) && (1..=days_in_month(y, m)).contains(&d);
        u64::try_from(days_from_civil(y as i64, m, d))
            .ok()
            .filter(|_| valid)?
    };
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(start_days * DAY);
    Some((day(start), start))
}

fn days_in_month(y: u32, m: u32) -> u32 {
    match m {
        2 if (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

// Howard Hinnant's algorithms for the proleptic Gregorian calendar.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from((m + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "just now", "5m ago", "3h ago", "2d ago".
pub fn ago(then: SystemTime, now: SystemTime) -> String {
    let secs = now.duration_since(then).unwrap_or(Duration::ZERO).as_secs();
    match secs {
        0..=59 => "just now".to_owned(),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86_399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn formats_utc_days() {
        assert_eq!(day(at(0)), "1970-01-01");
        assert_eq!(day(at(951_782_400)), "2000-02-29");
        assert_eq!(day(at(1_790_632_633)), "2026-09-28");
    }

    #[test]
    fn parses_since() {
        let now = at(1_790_632_633); // 2026-09-28
        assert_eq!(since("1d", now).unwrap().0, "2026-09-28");
        assert_eq!(since("7d", now).unwrap().0, "2026-09-22");
        assert_eq!(since("30d", now).unwrap().0, "2026-08-30");
        let (d, t) = since("2024-02-29", now).unwrap();
        assert_eq!(d, "2024-02-29");
        assert_eq!(day(t), d);
        assert_eq!(since("all", now).unwrap().0, "1970-01-01");
        for bad in [
            "0d",
            "xd",
            "2023-02-29",
            "2024-13-01",
            "2024-1-1",
            "yesterday",
            "",
        ] {
            assert!(since(bad, now).is_none(), "{bad}");
        }
    }

    #[test]
    fn ago_rounds_down() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let before = |s| now - Duration::from_secs(s);
        assert_eq!(ago(before(5), now), "just now");
        assert_eq!(ago(before(125), now), "2m ago");
        assert_eq!(ago(before(7200), now), "2h ago");
        assert_eq!(ago(before(3 * 86_400 + 5), now), "3d ago");
        assert_eq!(ago(now + Duration::from_secs(9), now), "just now");
    }
}
