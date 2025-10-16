use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "Assignment 5: config printer", long_about = None)]
struct Cli {
    #[arg(short, long)]
    debug: bool,

    #[arg(short, long, env = "CONF_FILE", default_value = "config.toml")]
    conf: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Mode {
    #[serde(default)]
    debug: bool,
}

#[derive(Debug, Deserialize)]
struct Server {
    #[serde(default = "default_external_url")]
    external_url: String,

    #[serde(default = "default_http_port")]
    http_port: u16,

    #[serde(default = "default_grpc_port")]
    grpc_port: u16,

    #[serde(default = "default_healthz_port")]
    healthz_port: u16,

    #[serde(default = "default_metrics_port")]
    metrics_port: u16,
}

fn default_external_url() -> String { "http://127.0.0.1".to_string() }
fn default_http_port() -> u16 { 8081 }
fn default_grpc_port() -> u16 { 8082 }
fn default_healthz_port() -> u16 { 10025 }
fn default_metrics_port() -> u16 { 9199 }

#[derive(Debug, Deserialize)]
struct LogApp {
    #[serde(default = "default_log_level")]
    level: String,
}

fn default_log_level() -> String { "info".to_string() }

#[derive(Debug, Deserialize)]
struct BackgroundWatchdog {
    #[serde(default = "default_period")]
    period: String,

    #[serde(default = "default_limit")]
    limit: u32,

    #[serde(default = "default_lock_timeout")]
    lock_timeout: String,
}

fn default_period() -> String { "5s".to_string() }
fn default_limit() -> u32 { 10 }
fn default_lock_timeout() -> String { "4s".to_string() }

#[derive(Debug, Deserialize)]
struct Conf {
    #[serde(default)]
    mode: Option<Mode>,

    #[serde(default = "default_server")]
    server: Server,

    #[serde(default = "default_logapp")]
    log: LogApp,

    #[serde(default = "default_watchdog")]
    background: BackgroundWatchdog,
}

fn default_server() -> Server {
    Server {
        external_url: default_external_url(),
        http_port: default_http_port(),
        grpc_port: default_grpc_port(),
        healthz_port: default_healthz_port(),
        metrics_port: default_metrics_port(),
    }
}

fn default_logapp() -> LogApp {
    LogApp { level: default_log_level() }
}

fn default_watchdog() -> BackgroundWatchdog {
    BackgroundWatchdog { period: default_period(), limit: default_limit(), lock_timeout: default_lock_timeout() }
}

fn env_source() -> config::Environment {
    // read env variables with CONF_ prefix; use double underscore as separator for nesting
    config::Environment::with_prefix("CONF").separator("__")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // start with defaults from types by serializing default Conf
    let mut builder = config::Config::builder()
        .set_default("mode.debug", cli.debug)?
        .set_default("server.external_url", default_external_url())?
        .set_default("server.http_port", default_http_port())?
        .set_default("server.grpc_port", default_grpc_port())?
        .set_default("server.healthz_port", default_healthz_port())?
        .set_default("server.metrics_port", default_metrics_port())?
        .set_default("log.level", default_log_level())?
        .set_default("background.period", default_period())?
        .set_default("background.limit", default_limit())?
        .set_default("background.lock_timeout", default_lock_timeout())?;

    // merge from file (TOML)
    if cli.conf.exists() {
        builder = builder.add_source(config::File::from(cli.conf.as_path()));
    }

    // merge from env CONF_* (highest priority)
    let settings = builder.add_source(env_source()).build()?;

    // deserialize into typed struct
    let conf: Conf = settings.try_deserialize()?;

    // print configuration to stdout
    println!("Configuration:\n{:#?}", conf);

    Ok(())
}
