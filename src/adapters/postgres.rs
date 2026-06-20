#![allow(dead_code)]

use std::time::Instant;

use async_trait::async_trait;
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::AssertSqlSafe;
use sqlx::Column;
use sqlx::Row;

use rust_decimal::Decimal;

use super::{ColumnInfo, DatabaseAdapter, QueryResult, TableInfo, TableSchema, Value};
use crate::connection::{ConnectionConfig, DatabaseKind};
use crate::error::RiverError;

pub struct PostgresAdapter {
    pool: sqlx::PgPool,
}

fn row_to_values(row: &PgRow) -> Vec<Value> {
    let n = row.columns().len();
    let mut values = Vec::with_capacity(n);
    for i in 0..n {
        // Try integer types first (i32 for SERIAL/INT4, i64 for BIGINT/INT8)
        // before String, because PG's sqlx won't implicitly decode integers as strings.
        let val = row
            .try_get::<Option<i32>, _>(i)
            .ok()
            .flatten()
            .map(|v| Value::Int(v as i64))
            .or_else(|| {
                row.try_get::<Option<i64>, _>(i)
                    .ok()
                    .flatten()
                    .map(Value::Int)
            })
            .or_else(|| {
                row.try_get::<Option<Decimal>, _>(i)
                    .ok()
                    .flatten()
                    .map(|d| {
                        // Try to represent as Int if no fractional part, otherwise Float
                        if d.scale() == 0 {
                            Value::Int(d.mantissa() as i64)
                        } else {
                            use rust_decimal::prelude::ToPrimitive;
                            Value::Float(d.to_f64().unwrap_or(0.0))
                        }
                    })
            })
            .or_else(|| {
                row.try_get::<Option<f64>, _>(i)
                    .ok()
                    .flatten()
                    .map(Value::Float)
            })
            .or_else(|| {
                row.try_get::<Option<f32>, _>(i)
                    .ok()
                    .flatten()
                    .map(|v| Value::Float(v as f64))
            })
            .or_else(|| {
                row.try_get::<Option<bool>, _>(i)
                    .ok()
                    .flatten()
                    .map(Value::Bool)
            })
            .or_else(|| {
                row.try_get::<Option<String>, _>(i)
                    .ok()
                    .flatten()
                    .map(Value::String)
            })
            .unwrap_or(Value::Null);
        values.push(val);
    }
    values
}

#[async_trait]
impl DatabaseAdapter for PostgresAdapter {
    async fn connect(config: &ConnectionConfig) -> Result<Self, RiverError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.uri)
            .await?;
        Ok(Self { pool })
    }

    fn dialect(&self) -> DatabaseKind {
        DatabaseKind::Postgres
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
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT table_schema, table_name FROM information_schema.tables \
             WHERE table_type = 'BASE TABLE' AND table_schema NOT IN ('information_schema', 'pg_catalog') \
             ORDER BY table_schema, table_name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(schema, name)| TableInfo {
                name,
                schema: Some(schema),
            })
            .collect())
    }

    async fn describe_table(&self, table: &str) -> Result<TableSchema, RiverError> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT column_name, data_type, is_nullable \
             FROM information_schema.columns \
             WHERE table_name = $1 ORDER BY ordinal_position",
        )
        .bind(table)
        .fetch_all(&self.pool)
        .await?;

        let columns = rows
            .into_iter()
            .map(|(name, data_type, nullable)| ColumnInfo {
                name,
                data_type,
                nullable: nullable == "YES",
                is_primary_key: false,
            })
            .collect();

        Ok(TableSchema {
            name: table.to_string(),
            columns,
        })
    }
}
