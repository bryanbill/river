#![allow(dead_code)]

use super::config::ConnectionConfig;

use crate::error::RiverError;

pub enum ConnectionPool {
    Postgres(sqlx::PgPool),
    MySQL(sqlx::MySqlPool),
    SQLite(sqlx::SqlitePool),
    MongoDB(mongodb::Client),
}

impl ConnectionPool {
    pub async fn connect(config: &ConnectionConfig) -> Result<Self, RiverError> {
        match config.kind {
            super::config::DatabaseKind::Postgres => {
                let pool = sqlx::PgPool::connect(&config.uri).await?;
                Ok(Self::Postgres(pool))
            }
            super::config::DatabaseKind::MySQL => {
                let pool = sqlx::MySqlPool::connect(&config.uri).await?;
                Ok(Self::MySQL(pool))
            }
            super::config::DatabaseKind::SQLite => {
                let pool = sqlx::SqlitePool::connect(&config.uri).await?;
                Ok(Self::SQLite(pool))
            }
            super::config::DatabaseKind::MongoDB => {
                let client = mongodb::Client::with_uri_str(&config.uri).await?;
                Ok(Self::MongoDB(client))
            }
            super::config::DatabaseKind::MSSQL => Err(RiverError::Unsupported(
                "MSSQL uses tiberius directly; use the MssqlAdapter".into(),
            )),
        }
    }
}
