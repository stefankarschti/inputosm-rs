//! Configure diagnostics for each reader.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Info,
    Error,
    Disabled,
}
pub type LogCallback = std::sync::Arc<dyn Fn(LogLevel, &str) + Send + Sync>;
