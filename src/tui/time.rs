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
    fn fifty_nine_seconds_is_seconds() {
        assert_eq!(format_relative(NOW - 59, NOW), "59s ago");
    }

    #[test]
    fn sixty_seconds_is_one_minute() {
        assert_eq!(format_relative(NOW - MINUTE, NOW), "1m ago");
    }

    #[test]
    fn three_minutes() {
        assert_eq!(format_relative(NOW - 3 * MINUTE, NOW), "3m ago");
    }

    #[test]
    fn fifty_nine_minutes() {
        assert_eq!(format_relative(NOW - 59 * MINUTE, NOW), "59m ago");
    }

    #[test]
    fn one_hour() {
        assert_eq!(format_relative(NOW - HOUR, NOW), "1h ago");
    }

    #[test]
    fn two_hours() {
        assert_eq!(format_relative(NOW - 2 * HOUR, NOW), "2h ago");
    }

    #[test]
    fn twenty_three_hours() {
        assert_eq!(format_relative(NOW - 23 * HOUR, NOW), "23h ago");
    }

    #[test]
    fn one_day() {
        assert_eq!(format_relative(NOW - DAY, NOW), "1d ago");
    }

    #[test]
    fn five_days() {
        assert_eq!(format_relative(NOW - 5 * DAY, NOW), "5d ago");
    }

    #[test]
    fn six_days() {
        assert_eq!(format_relative(NOW - 6 * DAY, NOW), "6d ago");
    }

    #[test]
    fn one_week() {
        assert_eq!(format_relative(NOW - WEEK, NOW), "1w ago");
    }

    #[test]
    fn three_weeks() {
        assert_eq!(format_relative(NOW - 3 * WEEK, NOW), "3w ago");
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
    fn six_months() {
        assert_eq!(format_relative(NOW - 6 * MONTH, NOW), "6mo ago");
    }

    #[test]
    fn eleven_months() {
        assert_eq!(format_relative(NOW - 11 * MONTH, NOW), "11mo ago");
    }

    #[test]
    fn one_year() {
        assert_eq!(format_relative(NOW - YEAR, NOW), "1y ago");
    }

    #[test]
    fn two_years() {
        assert_eq!(format_relative(NOW - 2 * YEAR, NOW), "2y ago");
    }
}
