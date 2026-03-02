//! Provides the backend implementation for the ptodd.org website.

use log::{debug, info, warn};

use logger::SimpleLogger;
use server::Server;

mod handlers;
mod logger;
mod server;
mod time;
mod url;

const DEFAULT_ADDR: &str = "localhost:6502";

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;

fn main() -> Result<()> {
    SimpleLogger::init()?;
    Server::new(DEFAULT_ADDR)?.run()?;
    Ok(())
}
