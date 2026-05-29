use std::path;

use crate::{Error, Result, engines::KvsEngine};

pub struct SledStore(sled::Db);

impl SledStore {
    pub fn open<P: AsRef<path::Path>>(path: P) -> Result<Self> {
        Ok(Self(sled::open(path)?))
    }
}

impl KvsEngine for SledStore {
    fn set(&mut self, key: String, value: String) -> Result<()> {
        self.0.insert(key, value.as_str())?;
        Ok(())
    }

    fn remove(&mut self, key: String) -> Result<()> {
        self.0.remove(key)?.ok_or(Error::KeyNotFound)?;
        self.0.flush()?;
        Ok(())
    }

    fn get(&mut self, key: String) -> Result<Option<String>> {
        Ok(self.0.get(key)?.map(|ivec| String::from_utf8(ivec.to_vec())).transpose()?)
    }
}
