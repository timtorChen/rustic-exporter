use clap::Parser;

/// Rustic exporter
#[derive(Parser)]
#[command(version, about)]
pub(crate) struct Args {
    /// Metrics collection frequency in seconds
    #[arg(long, short, value_name = "INTERVAL", default_value = "300")]
    pub(crate) interval: u64,

    /// Log level: debug, info, warn, error
    #[arg(long, value_name = "LOG_LEVEL", default_value = "info")]
    pub(crate) log_level: String,

    /// Show logs of all dependents
    #[arg(long, short, value_name = "VERBOSE")]
    pub(crate) verbose: bool,

    /// Path to the configuration file
    #[arg(long, short, long = "config", value_name = "CONFIG")]
    pub(crate) config_path: String,

    /// Server host
    #[arg(long, value_name = "HOST", default_value = "0.0.0.0")]
    pub(crate) host: String,

    /// Server port
    #[arg(long, value_name = "PORT", default_value = "8080")]
    pub(crate) port: u16,
}
