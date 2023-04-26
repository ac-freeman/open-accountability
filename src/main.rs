#[macro_use]
extern crate rocket;

mod auth;
mod monitoring;
mod requests;

use signal_hook::consts::SIGTERM;
use signal_hook::iterator::Signals;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use file_rotate::{
    compression::Compression, suffix::AppendCount, ContentLimit, FileRotate, TimeFrequency,
};

use image::io::Reader as ImageReader;

use log::{error, info};

use std::error::Error;
use std::io::{Read, Write};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum OpenAccError {
    #[error("Program stopped with SIGTERM")]
    SigTerm,

    #[error("Invalid startup state")]
    InvalidStartupState,

    #[error("Image error")]
    ImageError(#[from] image::ImageError),

    #[error("Text error")]
    TextError(#[from] std::str::Utf8Error),

    #[error("io error")]
    IoError(#[from] std::io::Error),

    #[error("reqwest error")]
    ReqwestError(#[from] reqwest::Error),

    // put all other errors into one
    #[error("Other error")]
    OtherError(#[from] Box<dyn Error>),
}

const SERVICE_NAME: &str = "open-accountability";

#[macro_use]
extern crate dotenv_codegen;

const MIN_SLEEP_SECONDS: u64 = 60 * 2;
const MAX_SLEEP_SECONDS: u64 = 60 * 5;

// Automatically rotate log files when they reach this size (in number of lines)
const LOG_FILE_LINE_COUNT_LIMIT: usize = 10000;

const LOG_PATH: &str = "output.log";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    log::info!("Starting up...");

    // Setup up rotating loggers. Each log file will encompass at most one day, and there
    // will be up to three days of logs stored.
    let log = Box::new(FileRotate::new(
        LOG_PATH.clone(),
        AppendCount::new(3),
        ContentLimit::Time(TimeFrequency::Daily),
        Compression::None,
        #[cfg(unix)]
        None,
    ));

    // Configure logger at runtime
    fern::Dispatch::new()
        // Perform allocation-free log formatting
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                humantime::format_rfc3339(std::time::SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        // Add blanket level filter -
        .level(log::LevelFilter::Debug)
        // - and per-module overrides
        .level_for("hyper", log::LevelFilter::Info)
        // Output to stdout, files, and other Dispatch configurations
        .chain(std::io::stdout())
        .chain(std::io::stderr())
        .chain(log as Box<dyn Write + Send>)
        // Apply globally
        .apply()?;

    let lt = leptess::LepTess::new(None, "eng").unwrap();

    let client = reqwest::Client::new();

    // Authenticate the device
    let mut auth = Auth::new(client).await?;
    auth.check_service_file().await?;

    // panic!("test");
    info!("Authenticated. About to start.");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        info!("Received SIGINT signal. Exiting...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // Register a handler for the SIGTERM signal
    let mut signals = Signals::new([SIGTERM]).unwrap();
    let r2 = running.clone();
    std::thread::spawn(move || {
        for _ in signals.forever() {
            info!("Received SIGTERM signal. Exiting...");

            // Handle the SIGTERM signal here
            // Perform any cleanup tasks before exiting
            r2.store(false, Ordering::SeqCst);
            break;
        }
    });

    match monitor(running, lt, &mut auth).await {
        Ok(_) => {
            auth.exit_program(false).await?;
        }
        Err(OpenAccError::SigTerm) => {
            auth.exit_program(is_shutdown_in_progress()).await?;
        }
        Err(_) => {}
    };

    Ok(())
}

use crate::auth::Auth;
// use crate::monitoring::monitor;

use crate::monitoring::monitor;
use std::process::Command;
use std::string::ToString;

fn is_shutdown_in_progress() -> bool {
    let output = Command::new("runlevel")
        .output()
        .expect("Failed to execute command");
    let output_str = String::from_utf8_lossy(&output.stdout);

    output_str.starts_with("N 0")
}
