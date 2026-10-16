//! Install a diagnostic callback on a PBF reader.
mod common;
use inputosm::{Result, inputosmlog::LogLevel, timeutil};
use std::sync::Arc;

fn main() -> Result<()> {
    let args = common::arguments(1, 1, "Usage: custom_log <file.osm.pbf>")?;
    let mut reader = common::reader(&args[0])?;
    reader.set_log_level(LogLevel::Trace);
    reader.set_log_callback(Arc::new(|level, message| {
        let micros = timeutil::now_us();
        let label = match level {
            LogLevel::Trace => "TRC",
            LogLevel::Info => "INF",
            LogLevel::Error => "ERR",
            LogLevel::Disabled => "NON",
        };
        eprintln!(
            "{}.{:06} UTC [{label}]: {message}",
            timeutil::timestamp_to_string((micros / 1_000_000) as i64),
            micros % 1_000_000
        );
    }));
    reader.read_entity_views(true, |_, _| Ok(()))
}
