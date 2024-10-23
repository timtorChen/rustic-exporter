use clap::Parser;

/// Rustic exporter
#[derive(Parser)]
#[command(version, about)]
pub(crate) struct Args {
    /// Metrics collection frequency in seconds
    #[arg(long, value_name = "INTERVAL", default_value = "300")]
    pub(crate) interval: u64,

    /// Path to the configuration file
    #[arg(long, value_name = "CONFIG")]
    pub(crate) config: String,

    /// Server host
    #[arg(long, value_name = "HOST", default_value = "0.0.0.0")]
    pub(crate) host: String,

    /// Server port
    #[arg(long, value_name = "PORT", default_value = "8080")]
    pub(crate) port: u16,
}
