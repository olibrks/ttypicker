#[cfg(not(target_os = "linux"))]
compile_error!(concat!(
    env!("CARGO_PKG_NAME"),
    " crate only supports Linux"
));
mod config;
mod context;
mod error;
mod filechooser;
mod options;
mod state;
mod token;
mod utils;

use std::{error::Error, process::ExitCode};
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use zbus::connection;

use filechooser::FileChooser;

const TTYPICKER: &str = "ttypicker";
const PREFIX_TEMP: &str = "ttypicker-";

#[tokio::main]
async fn main() -> ExitCode {
    // CLI --check-config
    if std::env::args().any(|a| a == "--check-config") {
        match FileChooser::try_new() {
            Ok(_) => {
                println!("Config check succeeded");
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!("Config check failed: {e}");
                return ExitCode::from(e.exit_code() as u8);
            }
        }
    }

    // journald setup
    let journald = match tracing_journald::layer() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Fatal: could not connect to journald: {e}");
            return ExitCode::from(72); // EX_OSFILE
        }
    };

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(journald)
        .init();

    let filechooser = match FileChooser::try_new() {
        Ok(fc) => fc,
        Err(e) => {
            tracing::error!("Initialization failed: {e}");
            return ExitCode::from(e.exit_code() as u8);
        }
    };

    if let Err(e) = run(filechooser).await {
        tracing::error!("Fatal service error: {e}");
        return ExitCode::FAILURE;
    }

    tracing::info!("Service shutting down...");
    ExitCode::SUCCESS
}

async fn run(filechooser: FileChooser) -> Result<(), Box<dyn Error>> {
    let well_known_name = format!("org.freedesktop.impl.portal.desktop.{}", TTYPICKER);

    let _conn = connection::Builder::session()?
        .name(well_known_name)?
        .serve_at("/org/freedesktop/portal/desktop", filechooser)?
        .build()
        .await?;

    tracing::info!("Service running");

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    tokio::select! {
        _ = sigterm.recv() => tracing::debug!("Received SIGTERM"),
        _ = sigint.recv() => tracing::debug!("Received SIGINT"),
    }
    Ok(())
}
