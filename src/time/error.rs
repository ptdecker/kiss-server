//! Time module custom errors

use super::*;

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    InvalidMonth,
    SystemTime(std::time::SystemTimeError),
}

impl From<std::time::SystemTimeError> for Error {
    fn from(e: std::time::SystemTimeError) -> Self {
        Error::SystemTime(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::InvalidMonth => write!(f, "invalid month value"),
            Error::SystemTime(e) => write!(f, "system time error: {e}"),
        }
    }
}

impl std::error::Error for Error {}
