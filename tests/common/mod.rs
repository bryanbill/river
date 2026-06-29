#![allow(dead_code)]

use std::collections::HashMap;

use river::adapters::{create_adapter, DatabaseAdapter, QueryResult, Value};
use river::ai::AiClient;
use river::connection::{AiConfig, ConnectionConfig, DatabaseKind};
use river::engine::executor::execute_statement;
use river::error::RiverError;
use river::lang::parse;

/// Shared test context that holds connected adapters and source database metadata.
pub struct TestContext {
    pub adapters: HashMap<String, Box<dyn DatabaseAdapter>>,
    pub source_db: Vec<(String, DatabaseKind)>,
    pub ai_configs: HashMap<String, AiConfig>,
    pub ai_client: AiClient,
}

impl TestContext {
    /// Create a new TestContext by reading `infra/river.yaml` and connecting all adapters.
    ///
    /// Panics with a helpful message if any connection fails (e.g., Docker containers not running).
    pub async fn new() -> Self {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let config_path = format!("{}/infra/river.yaml", manifest_dir);

        let yaml_content = std::fs::read_to_string(&config_path).unwrap_or_else(|e| {
            panic!(
                "Failed to read config at {}: {}\n\
                 Hint: make sure infra/river.yaml exists with your database connections.",
                config_path, e
            )
        });

        let configs: Vec<ConnectionConfig> =
            serde_yaml::from_str(&yaml_content).unwrap_or_else(|e| {
                panic!(
                    "Failed to parse {}: {}\n\
                     Hint: the file should be a YAML array of {{name, kind, uri}} objects.",
                    config_path, e
                )
            });

        let mut adapters: HashMap<String, Box<dyn DatabaseAdapter>> = HashMap::new();
        let mut source_db: Vec<(String, DatabaseKind)> = Vec::new();

        for config in &configs {
            if matches!(config.kind, DatabaseKind::AI) {
                continue;
            }
            let adapter = create_adapter(config).await.unwrap_or_else(|e| {
                panic!(
                    "Failed to connect to '{}' ({:?}) at {}: {}\n\
                     Hint: run `docker-compose up -d` in the infra/ directory to start databases.",
                    config.name, config.kind, config.uri, e
                )
            });
            source_db.push((config.name.clone(), config.kind.clone()));
            adapters.insert(config.name.clone(), adapter);
        }

        let ai_configs_vec = river::connection::config::load_ai_configs(&config_path)
            .unwrap_or_else(|e| panic!("Failed to load AI configs from {}: {}", config_path, e));
        let mut ai_configs: HashMap<String, AiConfig> = HashMap::new();
        for cfg in ai_configs_vec {
            ai_configs.insert(cfg.name.clone(), cfg);
        }

        Self {
            adapters,
            source_db,
            ai_configs,
            ai_client: AiClient::new(),
        }
    }
}

/// Parse a RiverQL query, plan it, and execute it against the test context's adapters.
pub async fn execute_river(ctx: &TestContext, query: &str) -> Result<QueryResult, RiverError> {
    let stmt = parse(query)?;
    execute_statement(&stmt, &ctx.source_db, &ctx.adapters, &ctx.ai_configs, &ctx.ai_client).await
}

/// Execute raw SQL/JSON directly against an adapter (for cleanup operations like DROP TABLE).
pub async fn execute_raw(ctx: &TestContext, connection: &str, query: &str) -> Result<QueryResult, RiverError> {
    let adapter = ctx.adapters.get(connection).ok_or_else(|| {
        RiverError::Unsupported(format!("no adapter connected for '{}'", connection))
    })?;
    adapter.execute(query).await
}

/// Drop a table if it exists using raw SQL (for test cleanup).
pub async fn drop_table_if_exists(ctx: &TestContext, table: &str, connection: &str) {
    let sql = format!("DROP TABLE IF EXISTS \"{}\"", table);
    let _ = execute_raw(ctx, connection, &sql).await;
}

// ── Assertion Helpers ─────────────────────────────────────────────────────────

/// Assert that the result has exactly the expected column names (in order).
pub fn assert_columns(result: &QueryResult, expected: &[&str]) {
    let actual: Vec<&str> = result.columns.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        actual, expected,
        "Column mismatch.\n  Expected: {:?}\n  Actual:   {:?}",
        expected, actual
    );
}

/// Assert that the result contains exactly `expected` rows.
pub fn assert_row_count(result: &QueryResult, expected: usize) {
    assert_eq!(
        result.rows.len(),
        expected,
        "Row count mismatch: expected {} rows but got {}",
        expected,
        result.rows.len()
    );
}

/// Assert that the result contains at least `min` rows.
pub fn assert_row_count_gte(result: &QueryResult, min: usize) {
    assert!(
        result.rows.len() >= min,
        "Expected at least {} rows but got {}",
        min,
        result.rows.len()
    );
}

/// Assert that the row count is between `min` and `max` (inclusive).
pub fn assert_row_count_between(result: &QueryResult, min: usize, max: usize) {
    let count = result.rows.len();
    assert!(
        count >= min && count <= max,
        "Expected between {} and {} rows but got {}",
        min, max, count
    );
}

/// Assert that a specific value exists somewhere in the result set.
pub fn assert_contains_value(result: &QueryResult, column: &str, value: &Value) {
    let col_idx = result
        .columns
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("Column '{}' not found in result columns: {:?}", column, result.columns));

    let found = result.rows.iter().any(|row| &row[col_idx] == value);
    assert!(
        found,
        "Value {:?} not found in column '{}'. First 5 values: {:?}",
        value,
        column,
        result.rows.iter().take(5).map(|r| &r[col_idx]).collect::<Vec<_>>()
    );
}

/// Assert that all rows satisfy a predicate applied to a given column.
pub fn assert_all_match<F>(result: &QueryResult, column: &str, predicate: F)
where
    F: Fn(&Value) -> bool,
{
    let col_idx = result
        .columns
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("Column '{}' not found in result columns: {:?}", column, result.columns));

    for (i, row) in result.rows.iter().enumerate() {
        assert!(
            predicate(&row[col_idx]),
            "Row {} failed predicate on column '{}': value = {:?}",
            i, column, row[col_idx]
        );
    }
}

/// Assert that no NULL values appear in a given column.
pub fn assert_no_nulls(result: &QueryResult, column: &str) {
    assert_all_match(result, column, |v| !matches!(v, Value::Null));
}

/// Assert that a column's values are in ascending order (non-decreasing).
/// Compares Int and Float values numerically; String values lexicographically.
/// Null values are skipped.
pub fn assert_ordered_asc(result: &QueryResult, column: &str) {
    let col_idx = result
        .columns
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("Column '{}' not found in result columns: {:?}", column, result.columns));

    let values: Vec<&Value> = result.rows.iter().map(|r| &r[col_idx]).collect();

    for window in values.windows(2) {
        let (a, b) = (window[0], window[1]);
        if matches!(a, Value::Null) || matches!(b, Value::Null) {
            continue;
        }
        let ordered = match (a, b) {
            (Value::Int(x), Value::Int(y)) => x <= y,
            (Value::Float(x), Value::Float(y)) => x <= y,
            (Value::String(x), Value::String(y)) => x <= y,
            (Value::Bool(x), Value::Bool(y)) => !x || *y, // false <= true
            _ => true, // mixed types — skip comparison
        };
        assert!(
            ordered,
            "Column '{}' is not in ascending order: {:?} > {:?}",
            column, a, b
        );
    }
}

/// Assert that a column's values are in descending order (non-increasing).
/// Compares Int and Float values numerically; String values lexicographically.
/// Null values are skipped.
pub fn assert_ordered_desc(result: &QueryResult, column: &str) {
    let col_idx = result
        .columns
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("Column '{}' not found in result columns: {:?}", column, result.columns));

    let values: Vec<&Value> = result.rows.iter().map(|r| &r[col_idx]).collect();

    for window in values.windows(2) {
        let (a, b) = (window[0], window[1]);
        if matches!(a, Value::Null) || matches!(b, Value::Null) {
            continue;
        }
        let ordered = match (a, b) {
            (Value::Int(x), Value::Int(y)) => x >= y,
            (Value::Float(x), Value::Float(y)) => x >= y,
            (Value::String(x), Value::String(y)) => x >= y,
            (Value::Bool(x), Value::Bool(y)) => *x || !y, // true >= false
            _ => true, // mixed types — skip comparison
        };
        assert!(
            ordered,
            "Column '{}' is not in descending order: {:?} < {:?}",
            column, a, b
        );
    }
}
