//! Convert UTC timestamps without a date library.

use crate::{Error, Result};

pub fn parse_timestamp(text: &str) -> Result<i64> {
    let b = text.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return Err(Error::Invalid("Invalid UTC timestamp"));
    }
    let number = |start: usize, end: usize| -> Result<i64> {
        b[start..end].iter().try_fold(0i64, |v, c| {
            if c.is_ascii_digit() {
                Ok(v * 10 + i64::from(c - b'0'))
            } else {
                Err(Error::Invalid("Invalid timestamp digit"))
            }
        })
    };
    let (mut y, m, d) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (h, min, s) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&m) || d < 1 || d > days[(m - 1) as usize] || h > 23 || min > 59 || s > 59
    {
        return Err(Error::Invalid("Timestamp component out of range"));
    }
    y -= i64::from(m <= 2);
    let era = y.div_euclid(400);
    let year = y - era * 400;
    let month = m + if m > 2 { -3 } else { 9 };
    let day = year * 365 + year / 4 - year / 100 + (153 * month + 2) / 5 + d - 1;
    Ok((era * 146097 + day - 719468) * 86400 + h * 3600 + min * 60 + s)
}

pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
pub fn now_us() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros()
}
pub fn parse_osmstate_timestamp(text: &str) -> Result<i64> {
    parse_timestamp(&text.replace("\\:", ":"))
}
pub fn timestamp_to_string(timestamp: i64) -> String {
    let days = timestamp.div_euclid(86400);
    let seconds = timestamp.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let day = z - era * 146097;
    let year = (day - day / 1460 + day / 36524 - day / 146096) / 365;
    let mut y = year + era * 400;
    let doy = day - (365 * year + year / 4 - year / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}
pub fn duration_to_string(nanoseconds: u64) -> String {
    match nanoseconds {
        n if n < 1000 => format!("{n} ns"),
        n if n < 1_000_000 => format!("{:.3} μs", n as f64 / 1000.),
        n if n < 1_000_000_000 => format!("{:.3} ms", n as f64 / 1_000_000.),
        n if n < 60_000_000_000 => format!("{:.3} s", n as f64 / 1_000_000_000.),
        n => {
            let s = n / 1_000_000_000;
            if s >= 3600 {
                format!(
                    "{} hours {} minutes {} seconds",
                    s / 3600,
                    s / 60 % 60,
                    s % 60
                )
            } else {
                format!("{} minutes {} seconds", s / 60, s % 60)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates() {
        assert_eq!(parse_timestamp("1970-01-01T00:00:00Z").unwrap(), 0);
        assert_eq!(parse_timestamp("2021-01-02T03:04:05Z").unwrap(), 1609556645);
        assert_eq!(parse_timestamp("1969-12-31T23:59:59Z").unwrap(), -1);
        assert!(parse_timestamp("1900-02-29T00:00:00Z").is_err());
        assert!(parse_timestamp("2000-02-29T00:00:00Z").is_ok());
        assert!(parse_timestamp("2020-01-01T24:00:00Z").is_err());
    }
}
