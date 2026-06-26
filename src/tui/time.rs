const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
const WEEK: i64 = 7 * DAY;
const MONTH: i64 = 30 * DAY;
const YEAR: i64 = 365 * DAY;

pub fn format_relative(epoch: i64, now: i64) -> String {
    let delta = now.saturating_sub(epoch);
    if delta < 10 {
        return String::from("just now");
    }
    if delta < MINUTE {
        return format!("{}s ago", delta);
    }
    if delta < HOUR {
        return format!("{}m ago", delta / MINUTE);
    }
    if delta < DAY {
        return format!("{}h ago", delta / HOUR);
    }
    if delta < WEEK {
        return format!("{}d ago", delta / DAY);
    }
    if delta < MONTH {
        return format!("{}w ago", delta / WEEK);
    }
    if delta < YEAR {
        return format!("{}mo ago", delta / MONTH);
    }
    format!("{}y ago", delta / YEAR)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000_000_000;

    #[test]
    fn future_clamps_to_just_now() {
        assert_eq!(format_relative(NOW + 100, NOW), "just now");
    }

    #[test]
    fn zero_delta_is_just_now() {
        assert_eq!(format_relative(NOW, NOW), "just now");
    }

    #[test]
    fn nine_seconds_is_just_now() {
        assert_eq!(format_relative(NOW - 9, NOW), "just now");
    }

    #[test]
    fn ten_seconds_is_seconds() {
        assert_eq!(format_relative(NOW - 10, NOW), "10s ago");
    }

    #[test]
    fn sixty_seconds_is_one_minute() {
        assert_eq!(format_relative(NOW - MINUTE, NOW), "1m ago");
    }

    #[test]
    fn one_hour() {
        assert_eq!(format_relative(NOW - HOUR, NOW), "1h ago");
    }

    #[test]
    fn one_day() {
        assert_eq!(format_relative(NOW - DAY, NOW), "1d ago");
    }

    #[test]
    fn one_week() {
        assert_eq!(format_relative(NOW - WEEK, NOW), "1w ago");
    }

    #[test]
    fn one_day_before_month_boundary() {
        assert_eq!(format_relative(NOW - (MONTH - DAY), NOW), "4w ago");
    }

    #[test]
    fn one_month() {
        assert_eq!(format_relative(NOW - MONTH, NOW), "1mo ago");
    }

    #[test]
    fn one_year() {
        assert_eq!(format_relative(NOW - YEAR, NOW), "1y ago");
    }

}
