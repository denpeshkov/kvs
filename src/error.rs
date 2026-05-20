use std::{fmt::Debug, io};

pub type Result<T> = std::result::Result<T, KvsError>;

#[derive(thiserror::Error, Debug)]
pub enum KvsError {
    /// IO error.
    #[error("{0}")]
    Io(#[from] io::Error),

    /// Serialization or deserialization error.
    #[error("{0}")]
    Serde(#[from] postcard::Error),

    /// Removing non-existent key error.
    #[error("key not found")]
    KeyNotFound,

    /// Unexpected command type error.
    /// It indicated a corrupted log or a program bug.
    #[error("unexpected command type")]
    UnexpectedCommandType,
}
