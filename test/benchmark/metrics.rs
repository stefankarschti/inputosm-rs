//! Read optional Linux process counters without an additional crate.
use std::{fs, process::Command};

pub fn ticks_per_second() -> Option<f64> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let output = Command::new("getconf").arg("CLK_TCK").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let ticks: f64 = std::str::from_utf8(&output.stdout)
        .ok()?
        .trim()
        .parse()
        .ok()?;
    (ticks > 0.0).then_some(ticks)
}
fn field(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let (name, rest) = line.split_once(':')?;
        (name == key)
            .then(|| rest.split_whitespace().next()?.parse().ok())
            .flatten()
    })
}
#[derive(Default)]
pub struct Usage {
    ticks: Option<u64>,
    minor: Option<u64>,
    major: Option<u64>,
    pub max_rss: Option<u64>,
}
impl Usage {
    pub fn read() -> Self {
        let stat = fs::read_to_string("/proc/self/stat").unwrap_or_default();
        let fields: Vec<_> = stat
            .rsplit_once(") ")
            .map(|(_, rest)| rest.split_whitespace().collect())
            .unwrap_or_default();
        let number = |i: usize| fields.get(i)?.parse::<u64>().ok();
        Self {
            ticks: number(11).zip(number(12)).map(|(u, s)| u + s),
            minor: number(7),
            major: number(9),
            max_rss: fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|s| field(&s, "VmHWM")),
        }
    }
    pub fn differences(&self, before: &Self, ticks_per_second: Option<f64>) -> [String; 3] {
        let delta = |a: Option<u64>, b: Option<u64>| a.zip(b).and_then(|(a, b)| a.checked_sub(b));
        [
            delta(self.ticks, before.ticks)
                .zip(ticks_per_second)
                .map(|(n, hz)| format!("{:.6}", n as f64 / hz))
                .unwrap_or_default(),
            display(delta(self.minor, before.minor)),
            display(delta(self.major, before.major)),
        ]
    }
}
#[derive(Default)]
pub struct Memory {
    pub rss: Option<u64>,
    pub pss: Option<u64>,
    pub anonymous: Option<u64>,
    pub page_tables: Option<u64>,
}
impl Memory {
    pub fn read() -> Self {
        let rollup = fs::read_to_string("/proc/self/smaps_rollup").unwrap_or_default();
        let status = fs::read_to_string("/proc/self/status").unwrap_or_default();
        Self {
            rss: field(&rollup, "Rss"),
            pss: field(&rollup, "Pss"),
            anonymous: field(&rollup, "Pss_Anon"),
            page_tables: field(&status, "VmPTE"),
        }
    }
}
pub fn display(value: Option<u64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_counters_are_empty_and_deltas_are_checked() {
        assert_eq!(
            Usage::default().differences(&Usage::default(), Some(100.0)),
            ["", "", ""]
        );
        assert_eq!(field("Rss: 12 kB\nPss_Anon: 9 kB\n", "Pss_Anon"), Some(9));
        let before = Usage {
            ticks: Some(10),
            minor: Some(2),
            major: Some(0),
            max_rss: None,
        };
        let after = Usage {
            ticks: Some(15),
            minor: Some(5),
            major: Some(0),
            max_rss: None,
        };
        assert_eq!(
            after.differences(&before, Some(100.0)),
            ["0.050000", "3", "0"]
        );
    }
}
