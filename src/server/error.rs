//! Server module custom errors

use super::*;

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    InvalidRequest(String),
    RequestTooLarge,
    Channel(String),
    Io(std::io::Error),
}

impl<T> From<mpsc::SendError<T>> for Error {
    fn from(err: mpsc::SendError<T>) -> Self {
        Error::Channel(err.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::InvalidRequest(e) => write!(f, "invalid request: {e}"),
            Error::RequestTooLarge => {
                write!(f, "request too large: exceeded maximum header line limit")
            }
            Error::Channel(s) => write!(f, "channel: {s}"),
            Error::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for Error {}
