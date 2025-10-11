mod cli;
mod collector;
mod config;

use config::Config;

use axum::{
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};

use clap::Parser;
use core::panic;
use prometheus_client::{encoding::text::encode, registry::Registry};
use regex::Regex;
use std::{
    env, fs,
    sync::{Arc, Mutex},
};
use tokio::signal;
use tracing::{error, info};

async fn metrics_handler(State(state): State<Arc<Mutex<Registry>>>) -> impl IntoResponse {
    let registry = state.lock().unwrap();
    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
        .body(Body::from(buffer))
        .unwrap()
}

fn replace_with_env_vars(input: &str) -> String {
    let re = Regex::new(r"\$\{(.*)\}").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        let var_name = caps[1].to_string();
        env::var(var_name).unwrap_or_default()
    })
    .to_string()
}

#[tokio::main]
async fn main() {
    let args = cli::Args::parse();

    // log level
    if args.verbose {
        tracing_subscriber::fmt().init();
    } else {
        let filter =
            tracing_subscriber::EnvFilter::new(format!("rustic_exporter={}", args.log_level));
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    let config_path = args.config_path;
    let mut file_content = match fs::read_to_string(config_path.clone()) {
        Ok(c) => c,
        Err(e) => {
            error!("Unable to read the configuration file");
            panic!("Error: {}", e);
        }
    };
    info!("Using configuration file: {}", config_path);

    file_content = replace_with_env_vars(&file_content);
    let config: Config = match toml::from_str(&file_content) {
        Ok(c) => c,
        Err(e) => {
            error!("Invaid toml file");
            panic!("Error: {}", e);
        }
    };

    let mut registry = Registry::default();
    for backup in config.backups {
        info!("Registering repositroy: {}", backup.name);
        let collector = collector::RusticCollector::new(backup, args.interval);
        registry.register_collector(Box::new(collector));
    }
    let addr = format!("{}:{}", args.host, args.port);
    let listener = match tokio::net::TcpListener::bind(addr.clone()).await {
        Ok(c) => c,
        Err(e) => {
            error!("Cannot listen on {}", addr);
            panic!("Error: {}", e);
        }
    };
    let registry_state = Arc::new(Mutex::new(registry));
    let router = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(registry_state);

    info!("Start server on http://{addr}");
    let server = axum::serve(listener, router);
    let server_result = if cfg!(debug_assertions) {
        server.await
    } else {
        server.with_graceful_shutdown(shutdown_signal()).await
    };

    match server_result {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to start server");
            panic!("Error: {}", e);
        }
    };
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            println!("Ctrl+C triggered")
        },
        _ = terminate => {
            println!("signal SIGTERM triggered")
        },
    }
}
