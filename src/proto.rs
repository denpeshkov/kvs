use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Request {
    Set { key: String, value: String },
    Rm { key: String },
    Get { key: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Set(Result<()>),
    Rm(Result<()>),
    Get(Result<Option<String>>),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Result<T>(pub std::result::Result<T, String>);
