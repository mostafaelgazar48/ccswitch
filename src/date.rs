//! Just enough date handling for ccswitch, without a date library.

use std::time::{Duration, SystemTime};

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
