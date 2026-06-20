#![allow(dead_code)]

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RiverError {
    #[error("connection failed: {0}")]
    Connection(#[from] sqlx::Error),

    #[error("mongodb error: {0}")]
    MongoDB(#[from] mongodb::error::Error),

    #[error("tiberius error: {0}")]
    Tiberius(#[from] tiberius::error::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("parse error at line {line}: {msg}")]
    Parse { line: usize, msg: String },

    #[error("unsupported operation: {0}")]
    Unsupported(String),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}
