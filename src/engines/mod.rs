use crate::Result;

mod kvs;
mod sled;

pub use kvs::KvStore;
pub use sled::SledStore;

/// Defines the storage interface.
pub trait KvsEngine {
    /// Set the value for a key.
    fn set(&mut self, key: String, value: String) -> Result<()>;
    /// Remove a given key.
    fn remove(&mut self, key: String) -> Result<()>;
    /// Get the value for a key.
    fn get(&mut self, key: String) -> Result<Option<String>>;
}
