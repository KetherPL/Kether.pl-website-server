// SPDX-License-Identifier: GPL-3.0-only

const DEFAULT_MINUTES: u64 = 45;
const DEFAULT_HOURS_MINUTES: u64 = 60;
const DEFAULT_DAYS_MINUTES: u64 = 1440;

fn parse_hour_value(value: &str) -> Result<u64, String> {
    if let Some((hours_str, minutes_str)) = value.split_once(':').or_else(|| value.split_once('.')) {
        let hours: u64 = hours_str
            .parse()
            .map_err(|_| format!("Invalid hours value: {}", hours_str))?;
        let minutes: u64 = minutes_str
            .parse()
            .map_err(|_| format!("Invalid minutes value: {}", minutes_str))?;
        if minutes >= 60 {
            return Err(format!("Minutes must be less than 60 in hour format: {}", value));
        }
        return Ok(hours * 60 + minutes);
    }

    let hours: u64 = value
        .parse()
        .map_err(|_| format!("Invalid hours value: {}", value))?;
    Ok(hours * 60)
}

fn parse_day_value(value: &str) -> Result<u64, String> {
    let days: f64 = value
        .parse()
        .map_err(|_| format!("Invalid days value: {}", value))?;
    if days <= 0.0 {
        return Err("Days must be greater than zero.".to_string());
    }
    Ok(days.mul_add(1440.0, 0.5) as u64)
}

pub(super) fn parse_mute_duration(tokens: &[&str]) -> Result<u64, String> {
    if tokens.is_empty() {
        return Ok(DEFAULT_MINUTES);
    }

    match tokens[0].to_ascii_lowercase().as_str() {
        "h" => {
            if tokens.len() > 2 {
                return Err("Too many arguments after 'h'. Usage: !mute <user> [minutes] | !mute <user> h [H | H:MM]".to_string());
            }
            match tokens.get(1) {
                None => Ok(DEFAULT_HOURS_MINUTES),
                Some(value) => parse_hour_value(value),
            }
        }
        "d" => {
            if tokens.len() > 2 {
                return Err("Too many arguments after 'd'. Usage: !mute <user> d [days]".to_string());
            }
            match tokens.get(1) {
                None => Ok(DEFAULT_DAYS_MINUTES),
                Some(value) => parse_day_value(value),
            }
        }
        value => {
            if tokens.len() > 1 {
                return Err("Too many arguments. Usage: !mute <user> [minutes] | !mute <user> h [H | H:MM] | !mute <user> d [days]".to_string());
            }
            let minutes: u64 = value
                .parse()
                .map_err(|_| format!("Invalid minutes value: {}", value))?;
            if minutes == 0 {
                return Err("Mute duration must be greater than zero.".to_string());
            }
            Ok(minutes)
        }
    }
}

pub(super) fn format_duration_minutes(minutes: u64) -> String {
    if minutes % 1440 == 0 && minutes >= 1440 {
        let days = minutes / 1440;
        return format!("{}d", days);
    }

    let hours = minutes / 60;
    let mins = minutes % 60;
    match (hours, mins) {
        (0, 0) => "0m".to_string(),
        (0, m) => format!("{}m", m),
        (h, 0) => format!("{}h", h),
        (h, m) => format!("{}h {}m", h, m),
    }
}

pub(super) fn format_remaining_seconds(seconds: i64) -> String {
    if seconds <= 0 {
        return "0m".to_string();
    }
    format_duration_minutes(((seconds + 59) / 60) as u64)
}

#[cfg(test)]
mod tests {
    use super::{format_duration_minutes, parse_mute_duration};

    #[test]
    fn default_minutes_when_empty() {
        assert_eq!(parse_mute_duration(&[]).unwrap(), 45);
    }

    #[test]
    fn parse_plain_minutes() {
        assert_eq!(parse_mute_duration(&["120"]).unwrap(), 120);
    }

    #[test]
    fn parse_hour_default() {
        assert_eq!(parse_mute_duration(&["h"]).unwrap(), 60);
    }

    #[test]
    fn parse_hour_with_minutes_colon() {
        assert_eq!(parse_mute_duration(&["h", "1:30"]).unwrap(), 90);
    }

    #[test]
    fn parse_hour_with_minutes_dot() {
        assert_eq!(parse_mute_duration(&["h", "1.30"]).unwrap(), 90);
    }

    #[test]
    fn parse_day_default() {
        assert_eq!(parse_mute_duration(&["d"]).unwrap(), 1440);
    }

    #[test]
    fn parse_decimal_days() {
        assert_eq!(parse_mute_duration(&["d", "1.5"]).unwrap(), 2160);
    }

    #[test]
    fn rejects_zero_minutes() {
        assert!(parse_mute_duration(&["0"]).is_err());
    }

    #[test]
    fn format_duration_examples() {
        assert_eq!(format_duration_minutes(45), "45m");
        assert_eq!(format_duration_minutes(90), "1h 30m");
        assert_eq!(format_duration_minutes(1440), "1d");
    }
}
