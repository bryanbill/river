#![allow(dead_code)]

use std::time::Instant;

use async_trait::async_trait;
use futures::TryStreamExt;
use mongodb::bson::{doc, Document};
use serde_json::Value as JsonValue;

use super::{ColumnInfo, DatabaseAdapter, QueryResult, TableInfo, TableSchema, Value};
use crate::connection::{ConnectionConfig, DatabaseKind};
use crate::error::RiverError;

pub struct MongoAdapter {
    client: mongodb::Client,
    default_db: String,
}

fn json_to_value(jv: &JsonValue) -> Value {
    match jv {
        JsonValue::Null => Value::Null,
        JsonValue::Bool(b) => Value::Bool(*b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else {
                Value::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        JsonValue::String(s) => Value::String(s.clone()),
        JsonValue::Array(arr) => Value::String(serde_json::to_string(arr).unwrap_or_default()),
        JsonValue::Object(obj) => Value::String(serde_json::to_string(obj).unwrap_or_default()),
    }
}

fn infer_columns_from_doc(doc: &Document) -> Vec<ColumnInfo> {
    doc.iter()
        .map(|(key, val)| {
            let data_type = match val.element_type() {
                mongodb::bson::spec::ElementType::String => "string".to_string(),
                mongodb::bson::spec::ElementType::Int32 | mongodb::bson::spec::ElementType::Int64 => {
                    "int".to_string()
                }
                mongodb::bson::spec::ElementType::Double => "float".to_string(),
                mongodb::bson::spec::ElementType::Boolean => "bool".to_string(),
                mongodb::bson::spec::ElementType::Null => "null".to_string(),
                mongodb::bson::spec::ElementType::Array => "array".to_string(),
                mongodb::bson::spec::ElementType::EmbeddedDocument => "object".to_string(),
                mongodb::bson::spec::ElementType::ObjectId => "objectid".to_string(),
                mongodb::bson::spec::ElementType::DateTime => "datetime".to_string(),
                _ => "unknown".to_string(),
            };
            ColumnInfo {
                name: key.to_string(),
                data_type,
                nullable: true,
                is_primary_key: key == "_id",
            }
        })
        .collect()
}

#[async_trait]
impl DatabaseAdapter for MongoAdapter {
    async fn connect(config: &ConnectionConfig) -> Result<Self, RiverError> {
        let client = mongodb::Client::with_uri_str(&config.uri).await?;
        let default_db = client
            .default_database()
            .map(|db| db.name().to_string())
            .unwrap_or_else(|| "test".to_string());
        Ok(Self { client, default_db })
    }

    fn dialect(&self) -> DatabaseKind {
        DatabaseKind::MongoDB
    }

    async fn execute(&self, query: &str) -> Result<QueryResult, RiverError> {
        let start = Instant::now();

        let parsed: JsonValue =
            serde_json::from_str(query).map_err(|e| RiverError::Unsupported(e.to_string()))?;

        let db_name = parsed["database"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.default_db);
        let coll_name = parsed["collection"]
            .as_str()
            .ok_or_else(|| RiverError::Unsupported("missing 'collection' field".into()))?;

        let db = self.client.database(db_name);
        let collection = db.collection::<Document>(coll_name);

        let pipeline = parsed["pipeline"].as_array().cloned().unwrap_or_default();

        let docs: Vec<Document> = if pipeline.is_empty() {
            let mut cursor = collection.find(doc! {}).await?;
            let mut results = Vec::new();
            while let Some(doc) = cursor.try_next().await? {
                results.push(doc);
            }
            results
        } else {
            let docs_as_bson: Vec<Document> = pipeline
                .iter()
                .map(|v| mongodb::bson::to_document(v).unwrap_or_default())
                .collect();
            let mut cursor = collection.aggregate(docs_as_bson).await?;
            let mut results = Vec::new();
            while let Some(doc) = cursor.try_next().await? {
                results.push(doc);
            }
            results
        };

        let elapsed = start.elapsed();
        let rows_affected = docs.len() as u64;

        let columns = if docs.is_empty() {
            vec![]
        } else {
            docs[0].keys().map(|k| k.to_string()).collect()
        };

        let rows: Vec<Vec<Value>> = docs
            .iter()
            .map(|doc| {
                let bson_doc =
                    mongodb::bson::to_document(doc).unwrap_or_default();
                let json: JsonValue =
                    serde_json::to_value(&bson_doc).unwrap_or(JsonValue::Null);
                let obj = json.as_object();
                columns
                    .iter()
                    .map(|col| {
                        obj.and_then(|o| o.get(col))
                            .map(json_to_value)
                            .unwrap_or(Value::Null)
                    })
                    .collect()
            })
            .collect();

        Ok(QueryResult {
            columns,
            rows,
            elapsed,
            rows_affected,
        })
    }

    async fn list_tables(&self) -> Result<Vec<TableInfo>, RiverError> {
        let db = self.client.database(&self.default_db);
        let names = db.list_collection_names().await?;
        Ok(names
            .into_iter()
            .map(|name| TableInfo { name, schema: None })
            .collect())
    }

    async fn describe_table(&self, table: &str) -> Result<TableSchema, RiverError> {
        let db = self.client.database(&self.default_db);
        let collection = db.collection::<Document>(table);
        let sample = collection.find_one(doc! {}).await?;

        let columns = match sample {
            Some(doc) => infer_columns_from_doc(&doc),
            None => vec![],
        };

        Ok(TableSchema {
            name: table.to_string(),
            columns,
        })
    }
}
