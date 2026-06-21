#![allow(dead_code)]

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use futures::TryStreamExt;
use tiberius::{Client, Config};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use super::{ColumnInfo, DatabaseAdapter, QueryResult, TableInfo, TableSchema, Value};
use crate::connection::{ConnectionConfig, DatabaseKind};
use crate::error::RiverError;

pub struct MssqlAdapter {
    client: Arc<Mutex<Client<Compat<TcpStream>>>>,
}

async fn connect_client(uri: &str) -> Result<Client<Compat<TcpStream>>, RiverError> {
    let config = Config::from_ado_string(uri)?;

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let client = Client::connect(config, tcp.compat_write()).await?;
    Ok(client)
}

fn row_to_values(row: &tiberius::Row, col_count: usize) -> Vec<Value> {
    let mut values = Vec::with_capacity(col_count);
    for i in 0..col_count {
        let val = row
            .try_get::<&str, _>(i)
            .ok()
            .flatten()
            .map(|s: &str| Value::String(s.to_string()))
            .or_else(|| {
                row.try_get::<i32, _>(i)
                    .ok()
                    .flatten()
                    .map(|v| Value::Int(v as i64))
            })
            .or_else(|| {
                row.try_get::<i64, _>(i)
                    .ok()
                    .flatten()
                    .map(Value::Int)
            })
            .or_else(|| {
                row.try_get::<f64, _>(i)
                    .ok()
                    .flatten()
                    .map(Value::Float)
            })
            .or_else(|| {
                row.try_get::<bool, _>(i)
                    .ok()
                    .flatten()
                    .map(Value::Bool)
            })
            .or_else(|| {
                row.try_get::<time::OffsetDateTime, _>(i)
                    .ok()
                    .flatten()
                    .map(|dt| Value::String(super::format_offset_dt(dt)))
            })
            .or_else(|| {
                row.try_get::<time::PrimitiveDateTime, _>(i)
                    .ok()
                    .flatten()
                    .map(|dt| Value::String(super::format_primitive_dt(dt)))
            })
            .or_else(|| {
                row.try_get::<time::Date, _>(i)
                    .ok()
                    .flatten()
                    .map(|d| Value::String(super::format_date(d)))
            })
            .or_else(|| {
                row.try_get::<time::Time, _>(i)
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
impl DatabaseAdapter for MssqlAdapter {
    async fn connect(config: &ConnectionConfig) -> Result<Self, RiverError> {
        let client = connect_client(&config.uri).await?;
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    fn dialect(&self) -> DatabaseKind {
        DatabaseKind::MSSQL
    }

    async fn execute(&self, query: &str) -> Result<QueryResult, RiverError> {
        let start = Instant::now();
        let mut client = self.client.lock().await;

        let mut stream = client.query(query, &[]).await?;
        let cols = stream.columns().await?;
        let columns: Vec<String> = cols
            .map(|c| c.iter().map(|col| col.name().to_string()).collect())
            .unwrap_or_default();
        let col_count = columns.len();

        let row_stream = stream.into_row_stream();
        futures::pin_mut!(row_stream);

        let mut rows = Vec::new();
        while let Some(row) = row_stream.try_next().await? {
            rows.push(row_to_values(&row, col_count));
        }

        let elapsed = start.elapsed();

        Ok(QueryResult {
            columns,
            rows_affected: rows.len() as u64,
            rows,
            elapsed,
        })
    }

    async fn list_tables(&self) -> Result<Vec<TableInfo>, RiverError> {
        let mut client = self.client.lock().await;
        let stream = client.query(
            "SELECT TABLE_SCHEMA, TABLE_NAME FROM INFORMATION_SCHEMA.TABLES \
             WHERE TABLE_TYPE = 'BASE TABLE' \
             ORDER BY TABLE_SCHEMA, TABLE_NAME",
            &[],
        ).await?;

        let row_stream = stream.into_row_stream();
        futures::pin_mut!(row_stream);

        let mut tables = Vec::new();
        while let Some(row) = row_stream.try_next().await? {
            let values = row_to_values(&row, 2);
            let schema = match values.first() {
                Some(Value::String(s)) => Some(s.clone()),
                _ => None,
            };
            let name = match values.get(1) {
                Some(Value::String(n)) => n.clone(),
                _ => continue,
            };
            tables.push(TableInfo { name, schema });
        }

        Ok(tables)
    }

    async fn describe_table(&self, table: &str) -> Result<TableSchema, RiverError> {
        let mut client = self.client.lock().await;
        let query = format!(
            "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE \
             FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_NAME = '{}' ORDER BY ORDINAL_POSITION",
            table.replace('\'', "''")
        );
        let stream = client.query(&query, &[]).await?;

        let row_stream = stream.into_row_stream();
        futures::pin_mut!(row_stream);

        let mut columns = Vec::new();
        while let Some(row) = row_stream.try_next().await? {
            let values = row_to_values(&row, 3);
            let name = match values.first() {
                Some(Value::String(n)) => n.clone(),
                _ => continue,
            };
            let data_type = match values.get(1) {
                Some(Value::String(t)) => t.clone(),
                _ => String::new(),
            };
            let nullable = match values.get(2) {
                Some(Value::String(n)) => n == "YES",
                _ => true,
            };
            columns.push(ColumnInfo {
                name,
                data_type,
                nullable,
                is_primary_key: false,
            });
        }

        Ok(TableSchema {
            name: table.to_string(),
            columns,
        })
    }
}
