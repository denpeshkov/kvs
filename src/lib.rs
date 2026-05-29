//! A simple key/value store.

use std::{io, result, string};

pub mod engines;
pub mod proto;

pub type Result<T> = result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// IO error.
    #[error("{0}")]
    Io(#[from] io::Error),

    /// Serialization or deserialization error.
    #[error("{0}")]
    Serde(#[from] postcard::Error),

    /// Removing non-existent key error.
    #[error("Key not found")]
    KeyNotFound,

    /// Unexpected command type error.
    /// It indicated a corrupted log or a program bug.
    #[error("Unexpected command type")]
    UnexpectedCommandType,

    #[error("Malformed metadata")]
    MalformedMetadata(String),

    /// Key or value is invalid UTF-8 sequence
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] string::FromUtf8Error),

    #[error("{0}")]
    Sled(#[from] sled::Error),

    #[error("Server error: {0}")]
    Server(String),
}
