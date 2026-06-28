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

#[derive(Deserialize)]
struct ConfigFile {
    #[serde(default)]
    connections: Vec<ConnectionConfig>,
}

pub fn load_config(path: &str) -> anyhow::Result<Vec<ConnectionConfig>> {
    if !std::path::Path::new(path).exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(path)?;

    if let Ok(cfg) = serde_yaml::from_str::<ConfigFile>(&content)
        && !cfg.connections.is_empty()
    {
        return Ok(cfg.connections);
    }

    let connections = serde_yaml::from_str::<Vec<ConnectionConfig>>(&content)?;
    Ok(connections)
}
