//! Simple logger implementation
//!
//! Provides an implementation of the ubiquitous Rust [log](https://crates.io/crates/log) facade.
//!
//! This logger implementation borrows some code from [simple_logger](https://crates.io/crates/simple_logger)
//! but is a very bare-bones logger implementation that makes assumptions where other full-featured
//! loggers would provide configuration options. It is included here in keeping with the intent of
//! this site to minimize to ideally none, the usage of any third-party crates.
//!
//! Log entries are output to stderr
//!

use std::{
    env::{VarError, var},
    str::FromStr,
};

use log::{LevelFilter, Log, Metadata, Record, set_boxed_logger, set_max_level};

use crate::time::DateTime;

use super::*;

/// Logging level environment variable name
const LOG_ENV_VAR_NAME: &str = "RUST_LOG";

/// Simple logger
pub struct SimpleLogger {
    log_level: LevelFilter,
}

impl SimpleLogger {
    /// Initialize the logger
    pub fn init() -> Result<()> {
        let log_level = match var(LOG_ENV_VAR_NAME) {
            Ok(result) => Ok(result),
            Err(VarError::NotPresent) => Ok("trace".to_string()),
            Err(e) => Err(e),
        }?;
        let logger = Self {
            log_level: LevelFilter::from_str(log_level.as_str())?,
        };
        set_max_level(logger.log_level);
        set_boxed_logger(Box::new(logger))?;
        Ok(())
    }
}

impl Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.log_level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let timestamp = DateTime::now()
                .map(|dt| dt.to_string())
                .unwrap_or_else(|_| "[unknown]".to_string());
            eprintln!(
                "{}: {}: {}: {}",
                timestamp,
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}
