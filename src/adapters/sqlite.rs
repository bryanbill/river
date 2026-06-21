#![allow(dead_code)]

use std::time::Instant;

use async_trait::async_trait;
use sqlx::sqlite::{SqlitePoolOptions, SqliteRow};
use sqlx::AssertSqlSafe;
use sqlx::Column;
use sqlx::Row;

use super::{ColumnInfo, DatabaseAdapter, QueryResult, TableInfo, TableSchema, Value};
use crate::connection::{ConnectionConfig, DatabaseKind};
use crate::error::RiverError;

pub struct SQLiteAdapter {
    pool: sqlx::SqlitePool,
}

fn row_to_values(row: &SqliteRow) -> Vec<Value> {
    let n = row.columns().len();
    let mut values = Vec::with_capacity(n);
    for i in 0..n {
        let val = row
            .try_get::<Option<String>, _>(i)
            .ok()
            .flatten()
            .map(Value::String)
            .or_else(|| {
                row.try_get::<Option<i64>, _>(i)
                    .ok()
                    .flatten()
                    .map(Value::Int)
            })
            .or_else(|| {
                row.try_get::<Option<f64>, _>(i)
                    .ok()
                    .flatten()
                    .map(Value::Float)
            })
            .or_else(|| {
                row.try_get::<Option<bool>, _>(i)
                    .ok()
                    .flatten()
                    .map(Value::Bool)
            })
            .or_else(|| {
                row.try_get::<Option<time::PrimitiveDateTime>, _>(i)
                    .ok()
                    .flatten()
                    .map(|dt| Value::String(super::format_primitive_dt(dt)))
            })
            .or_else(|| {
                row.try_get::<Option<time::OffsetDateTime>, _>(i)
                    .ok()
                    .flatten()
                    .map(|dt| Value::String(super::format_offset_dt(dt)))
            })
            .or_else(|| {
                row.try_get::<Option<time::Date>, _>(i)
                    .ok()
                    .flatten()
                    .map(|d| Value::String(super::format_date(d)))
            })
            .or_else(|| {
                row.try_get::<Option<time::Time>, _>(i)
                    .ok()
                    .flatten()
                    .map(|t| Value::String(super::format_time(t)))
            })
            .unwrap_or(Value::Null);
        values.push(val);
    }
    values
}

#[async_trait]
impl DatabaseAdapter for SQLiteAdapter {
    async fn connect(config: &ConnectionConfig) -> Result<Self, RiverError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&config.uri)
            .await?;
        Ok(Self { pool })
    }

    fn dialect(&self) -> DatabaseKind {
        DatabaseKind::SQLite
    }

    async fn execute(&self, query: &str) -> Result<QueryResult, RiverError> {
        let start = Instant::now();
        let rows = sqlx::query(AssertSqlSafe(query)).fetch_all(&self.pool).await?;
        let elapsed = start.elapsed();

        let columns = if rows.is_empty() {
            vec![]
        } else {
            rows[0]
                .columns()
                .iter()
                .map(|c| c.name().to_string())
                .collect()
        };

        let data: Vec<Vec<Value>> = rows.iter().map(|r| row_to_values(r)).collect();

        Ok(QueryResult {
            columns,
            rows: data,
            elapsed,
            rows_affected: rows.len() as u64,
        })
    }

    async fn list_tables(&self) -> Result<Vec<TableInfo>, RiverError> {
        let rows = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(name,)| TableInfo { name, schema: None })
            .collect())
    }

    async fn describe_table(&self, table: &str) -> Result<TableSchema, RiverError> {
        let query = format!("PRAGMA table_info('{}')", table);
        let rows = sqlx::query_as::<_, (i32, String, String, i32, String, i32)>(
            AssertSqlSafe(query),
        )
        .fetch_all(&self.pool)
        .await?;

        let columns = rows
            .into_iter()
            .map(|(_, name, data_type, not_null, _, pk)| ColumnInfo {
                name,
                data_type,
                nullable: not_null == 0,
                is_primary_key: pk != 0,
            })
            .collect();

        Ok(TableSchema {
            name: table.to_string(),
            columns,
        })
    }
}
