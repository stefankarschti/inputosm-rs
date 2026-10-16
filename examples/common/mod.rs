#![allow(dead_code)]

use inputosm::{Error, PbfReader, Result};
use std::sync::Mutex;

pub fn arguments(min: usize, max: usize, usage: &'static str) -> Result<Vec<String>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(min..=max).contains(&args.len()) {
        return Err(Error::Invalid(usage));
    }
    Ok(args)
}

pub fn parallelism() -> usize {
    std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(256)
}

pub fn reader(path: &str) -> Result<PbfReader> {
    let mut reader = PbfReader::open(path)?;
    reader.set_max_thread_count();
    Ok(reader)
}

#[repr(align(64))]
pub struct Worker<T>(pub Mutex<T>);

pub fn workers<T>(count: usize, mut init: impl FnMut() -> T) -> Vec<Worker<T>> {
    (0..count).map(|_| Worker(Mutex::new(init()))).collect()
}

pub fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut result = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(digit);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimal_groups_are_locale_independent() {
        for (value, expected) in [
            (0, "0"),
            (999, "999"),
            (1000, "1,000"),
            (17_005, "17,005"),
            (u64::MAX, "18,446,744,073,709,551,615"),
        ] {
            assert_eq!(grouped(value), expected);
        }
    }
}
