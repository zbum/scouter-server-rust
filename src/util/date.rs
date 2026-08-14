use chrono::{Datelike, Local, NaiveDate, TimeZone, Timelike};

/// Convert epoch milliseconds to "YYYYMMDD" string in local timezone.
pub fn yyyymmdd(millis: i64) -> String {
    let dt = Local
        .timestamp_millis_opt(millis)
        .single()
        .unwrap_or_else(|| Local::now());
    format!("{:04}{:02}{:02}", dt.year(), dt.month(), dt.day())
}

/// Get today's date as "YYYYMMDD".
pub fn yyyymmdd_today() -> String {
    let now = Local::now();
    format!("{:04}{:02}{:02}", now.year(), now.month(), now.day())
}

/// Convert "YYYYMMDD" string to epoch milliseconds at midnight local time.
pub fn date_millis(yyyymmdd: &str) -> i64 {
    if yyyymmdd.len() != 8 {
        return 0;
    }
    let year: i32 = yyyymmdd[0..4].parse().unwrap_or(0);
    let month: u32 = yyyymmdd[4..6].parse().unwrap_or(0);
    let day: u32 = yyyymmdd[6..8].parse().unwrap_or(0);

    if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
        if let Some(dt) = Local.from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap()).single() {
            return dt.timestamp_millis();
        }
    }
    0
}

/// Get milliseconds elapsed since midnight of the day for the given epoch ms.
/// This matches Java's DateUtil.getDateMillis(long).
pub fn get_date_millis(time_ms: i64) -> i32 {
    let dt = Local
        .timestamp_millis_opt(time_ms)
        .single()
        .unwrap_or_else(|| Local::now());
    let midnight = Local
        .from_local_datetime(
            &NaiveDate::from_ymd_opt(dt.year(), dt.month(), dt.day())
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
        )
        .single()
        .unwrap();
    (dt.timestamp_millis() - midnight.timestamp_millis()) as i32
}

/// Convert epoch ms to HHMM value (0~2359).
pub fn hhmm(millis: i64) -> i32 {
    let dt = Local
        .timestamp_millis_opt(millis)
        .single()
        .unwrap_or_else(|| Local::now());
    (dt.hour() * 100 + dt.minute()) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yyyymmdd_roundtrip() {
        let today = yyyymmdd_today();
        assert_eq!(today.len(), 8);

        let millis = date_millis(&today);
        assert!(millis > 0);

        let back = yyyymmdd(millis);
        assert_eq!(back, today);
    }

    #[test]
    fn test_get_date_millis() {
        // Midnight should return 0
        let today = yyyymmdd_today();
        let midnight = date_millis(&today);
        assert_eq!(get_date_millis(midnight), 0);

        // 1 hour after midnight should return 3_600_000
        assert_eq!(get_date_millis(midnight + 3_600_000), 3_600_000);
    }

    #[test]
    fn test_hhmm() {
        let today = yyyymmdd_today();
        let midnight = date_millis(&today);
        // midnight + 1h30m = 0130
        let t = midnight + 3_600_000 + 30 * 60_000;
        assert_eq!(hhmm(t), 130);
    }
}
