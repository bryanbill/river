use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseKind {
    Postgres,
    MySQL,
    MSSQL,
    SQLite,
    MongoDB,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub name: String,
    pub kind: DatabaseKind,
    pub uri: String,
    #[serde(default)]
    pub schema: Option<String>,
}
