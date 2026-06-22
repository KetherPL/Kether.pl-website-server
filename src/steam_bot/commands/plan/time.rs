// SPDX-License-Identifier: GPL-3.0-only

use chrono::{NaiveDateTime, NaiveTime, TimeZone};
use chrono_tz::Europe::Warsaw;

const MAX_HOURS: u8 = 23;
const MAX_MINUTES: u8 = 59;

/// Validates hours and minutes are within valid ranges
fn validate_time_components(hours: u8, minutes: u8) -> Result<(), String> {
    if hours > MAX_HOURS {
        return Err(format!("Hours must be between 0 and {}", MAX_HOURS));
    }
    if minutes > MAX_MINUTES {
        return Err(format!("Minutes must be between 0 and {}", MAX_MINUTES));
    }
    Ok(())
}

/// Parses time string in colon format (HH:MM)
fn parse_colon_format(trimmed: &str) -> Result<Option<(u8, u8)>, String> {
    let colon_pos = match trimmed.find(':') {
        Some(pos) => pos,
        None => return Ok(None),
    };

    let hours_str = &trimmed[..colon_pos];
    let after_colon = &trimmed[colon_pos + 1..];
    // Take only the first two parts (HH:MM), ignore everything after the minutes (e.g. seconds, milliseconds, etc.).
    let minutes_str = if let Some(second_colon_pos) = after_colon.find(':') {
        &after_colon[..second_colon_pos]
    } else {
        after_colon
    };

    let hours: u8 = hours_str
        .parse()
        .map_err(|_| format!("Invalid hours: {}", hours_str))?;
    let minutes: u8 = minutes_str
        .parse()
        .map_err(|_| format!("Invalid minutes: {}", minutes_str))?;

    validate_time_components(hours, minutes)?;
    Ok(Some((hours, minutes)))
}

/// Parses time string in dot format (HH.MM)
fn parse_dot_format(trimmed: &str) -> Result<Option<(u8, u8)>, String> {
    let dot_pos = match trimmed.find('.') {
        Some(pos) => pos,
        None => return Ok(None),
    };

    let hours_str = &trimmed[..dot_pos];
    let after_dot = &trimmed[dot_pos + 1..];
    // Take only the first two parts (HH.MM), ignore everything after the minutes (e.g. seconds, milliseconds, etc.).
    let minutes_str = if let Some(second_dot_pos) = after_dot.find('.') {
        &after_dot[..second_dot_pos]
    } else {
        after_dot
    };

    let hours: u8 = hours_str
        .parse()
        .map_err(|_| format!("Invalid hours: {}", hours_str))?;
    let minutes: u8 = minutes_str
        .parse()
        .map_err(|_| format!("Invalid minutes: {}", minutes_str))?;

    validate_time_components(hours, minutes)?;
    Ok(Some((hours, minutes)))
}

/// Parses time string in hour-only format (HH)
fn parse_hour_only(trimmed: &str) -> Result<(u8, u8), String> {
    let hours: u8 = trimmed.parse().map_err(|_| {
        "Invalid time format. Use HH:MM, HH.MM, or HH (e.g., 19:00, 18.30, or 16)".to_string()
    })?;

    validate_time_components(hours, 0)?;
    Ok((hours, 0))
}

/// Parses a time string in multiple formats (HH:MM, HH.MM, or HH)
pub(super) fn parse_time_string(time_str: &str) -> Result<(u8, u8), String> {
    let trimmed = time_str.trim();

    if trimmed.is_empty() {
        return Err("Time string cannot be empty".to_string());
    }

    // Try colon format (HH:MM)
    if let Some(result) = parse_colon_format(trimmed)? {
        return Ok(result);
    }

    // Try dot format (HH.MM)
    if let Some(result) = parse_dot_format(trimmed)? {
        return Ok(result);
    }

    // Try hour-only format (HH)
    parse_hour_only(trimmed)
}

/// Converts Unix timestamp to CET/CEST time format (HH:MM)
pub(super) fn unix_timestamp_to_cet_time(timestamp: i64) -> String {
    let dt_utc = chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0)
        .expect("Invalid timestamp");

    let dt_cet = dt_utc.with_timezone(&Warsaw);
    dt_cet.format("%H:%M").to_string()
}

/// Converts hours and minutes to Unix timestamp using CET/CEST timezone
pub(super) fn time_to_unix_timestamp(hours: u8, minutes: u8) -> Result<i64, String> {
    let now_utc = chrono::Utc::now();
    let now_cet = now_utc.with_timezone(&Warsaw);
    let current_date = now_cet.date_naive();

    let time = NaiveTime::from_hms_opt(hours as u32, minutes as u32, 0)
        .ok_or_else(|| "Failed to create time from hours and minutes".to_string())?;

    let naive_dt = NaiveDateTime::new(current_date, time);

    let dt_cet = match Warsaw.from_local_datetime(&naive_dt) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(_dt1, dt2) => dt2,
        chrono::LocalResult::None => {
            return Err(
                "Time does not exist in CET/CEST timezone (invalid DST transition)".to_string(),
            );
        }
    };

    Ok(dt_cet.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_string_colon() {
        assert_eq!(parse_time_string("19:00").unwrap(), (19, 0));
        assert_eq!(parse_time_string("08:30").unwrap(), (8, 30));
        assert_eq!(parse_time_string("0:0").unwrap(), (0, 0));
        assert_eq!(parse_time_string("23:59").unwrap(), (23, 59));
    }

    #[test]
    fn test_parse_time_string_dot() {
        assert_eq!(parse_time_string("18.30").unwrap(), (18, 30));
        assert_eq!(parse_time_string("7.45").unwrap(), (7, 45));
    }

    #[test]
    fn test_parse_time_string_hour_only() {
        assert_eq!(parse_time_string("16").unwrap(), (16, 0));
        assert_eq!(parse_time_string("9").unwrap(), (9, 0));
    }

    #[test]
    fn test_parse_time_string_invalid() {
        assert!(parse_time_string("24:00").is_err());
        assert!(parse_time_string("12:60").is_err());
        assert!(parse_time_string("abc").is_err());
        assert!(parse_time_string("").is_err());
    }

    #[test]
    fn test_parse_time_string_ignores_suffix_after_minutes() {
        assert_eq!(parse_time_string("18:30:30").unwrap(), (18, 30));
        assert_eq!(parse_time_string("18:30:21:37:666:2137").unwrap(), (18, 30));
        assert_eq!(parse_time_string("18:30:*6").unwrap(), (18, 30));
        assert_eq!(parse_time_string("18.30.30").unwrap(), (18, 30));
    }
}
