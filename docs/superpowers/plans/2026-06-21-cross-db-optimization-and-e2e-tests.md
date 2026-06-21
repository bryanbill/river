# Cross-DB Join Optimization & E2E Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add chunked pushdown optimization for cross-DB equi-joins, guard against unbounded cross-DB cross joins, and build comprehensive E2E integration tests validating all documented query features.

**Architecture:** The planner emits a `SemiJoinFetch` node for cross-DB equi-joins (fetches the build side, pushes join keys to the probe side via batched `WHERE key IN (...)` clauses). Cross-DB cross joins without LIMIT are rejected at plan time. E2E tests live in `tests/integrations/` with one file per doc topic, using the existing `tests/common/` infrastructure.

**Tech Stack:** Rust, tokio, existing adapter trait, existing translator infrastructure. Tests use `#[tokio::test]` and the existing `TestContext`/`execute_river` helpers.

---

## File Map

### Modified Files
- `src/engine/planner.rs` — Add `SemiJoinFetch` variant to `PlanNode`, add cross-join guard, add decision logic
- `src/engine/executor.rs` — Handle `SemiJoinFetch` execution (batch IN-pushdown + hash join)
- `src/engine/translator.rs` — Add `translate_in_list()` helper for generating `col IN (v1, v2, ...)` SQL
- `tests/common/mod.rs` — Add `assert_cross_db_consistency` and `assert_exact_rows` helpers

### New Files
- `tests/integrations/main.rs` — Integration test crate root (declares modules)
- `tests/integrations/helpers.rs` — Seed-formula computation helpers for exact-match assertions
- `tests/integrations/t01_basic_queries.rs` — Doc 01 tests
- `tests/integrations/t02_filtering.rs` — Doc 02 tests
- `tests/integrations/t03_joins.rs` — Doc 03 tests
- `tests/integrations/t04_aggregation.rs` — Doc 04 tests
- `tests/integrations/t05_window_functions.rs` — Doc 05 tests
- `tests/integrations/t06_advanced_queries.rs` — Doc 06 tests
- `tests/integrations/t07_cross_database.rs` — Doc 07 tests
- `tests/integrations/t08_data_modification.rs` — Doc 08 tests
- `tests/integrations/t09_meta_commands.rs` — Doc 09 tests

---

## Task 1: Add `SemiJoinFetch` to `PlanNode` and planner decision logic

**Files:**
- Modify: `src/engine/planner.rs`

- [ ] **Step 1: Add `SemiJoinFetch` variant to `PlanNode` enum**

In `src/engine/planner.rs`, add after the `Union` variant (around line 56):

```rust
SemiJoinFetch {
    build: Box<PlanNode>,
    probe_source: Source,
    probe_database: (String, DatabaseKind),
    build_key: Expression,
    probe_key: Expression,
    join_kind: JoinKind,
    condition: Expression,
},
```

- [ ] **Step 2: Add `CROSS_DB_BATCH_SIZE` constant**

At the top of `src/engine/planner.rs`, after the imports:

```rust
pub const CROSS_DB_BATCH_SIZE: usize = 1000;
```

- [ ] **Step 3: Add helper to extract equi-join key pair from a condition**

Add this function (it extracts the left/right column expressions from an `Eq` binary op):

```rust
fn extract_equi_keys(condition: &Expression) -> Option<(Expression, Expression)> {
    match condition {
        Expression::BinaryOp {
            op: BinaryOp::Eq,
            left,
            right,
        } => Some((*left.clone(), *right.clone())),
        _ => None,
    }
}
```

- [ ] **Step 4: Add helper to determine which key belongs to which source**

```rust
fn key_belongs_to_source(key: &Expression, source: &Source) -> bool {
    match key {
        Expression::QualifiedIdent { table, .. } => {
            source.alias.as_deref() == Some(table) || source.name == *table
        }
        Expression::Ident(_) => true,
        _ => false,
    }
}
```

- [ ] **Step 5: Modify `plan_query` to emit `SemiJoinFetch` for cross-DB equi-joins**

Replace the join-building loop in `plan_query` (lines ~120-139) with logic that checks if a cross-DB scenario exists and routes to `SemiJoinFetch`:

```rust
for join in &query.joins {
    let right_node = plan_source(&join.source, source_db);
    let left_db = find_single_db(&root);
    let right_db = match &right_node {
        PlanNode::Scan { database, .. } => database.clone(),
        _ => None,
    };

    let is_cross_db = match (&left_db, &right_db) {
        (Some((ln, _)), Some((rn, _))) => ln != rn,
        _ => false,
    };

    if is_cross_db {
        let condition = join.condition.clone().unwrap_or(Expression::Boolean(true));
        let is_cross_join = join.kind == JoinKind::Cross
            || matches!(&condition, Expression::Boolean(true));
        let equi_keys = extract_equi_keys(&condition);

        if let Some((left_key, right_key)) = equi_keys {
            // Cross-DB equi-join → SemiJoinFetch
            // Use left (current root) as build, right as probe
            let (probe_source, probe_database) = match &right_node {
                PlanNode::Scan { source, database, .. } => {
                    (source.clone(), database.clone().unwrap())
                }
                _ => {
                    // Fallback: can't extract probe info, use normal join
                    root = PlanNode::Join {
                        left: Box::new(root),
                        right: Box::new(right_node),
                        condition,
                        strategy: JoinStrategy::Hash,
                        join_kind: join.kind,
                    };
                    continue;
                }
            };

            // Determine which key belongs to which side
            let (build_key, probe_key) = if key_belongs_to_source(&right_key, &probe_source) {
                (left_key, right_key)
            } else {
                (right_key, left_key)
            };

            root = PlanNode::SemiJoinFetch {
                build: Box::new(root),
                probe_source,
                probe_database,
                build_key,
                probe_key,
                join_kind: join.kind,
                condition,
            };
        } else if is_cross_join && query.limit.is_none() {
            // Cross-DB cross join without LIMIT → error
            // We can't return an error from plan_query directly (it returns QueryPlan),
            // so we emit a special node that the executor will reject.
            // Actually, let's change plan_query to return Result. See step 6.
            root = PlanNode::Join {
                left: Box::new(root),
                right: Box::new(right_node),
                condition,
                strategy: JoinStrategy::NestedLoop,
                join_kind: join.kind,
            };
        } else {
            // Cross-DB non-equi join with LIMIT (acceptable, bounded)
            root = PlanNode::Join {
                left: Box::new(root),
                right: Box::new(right_node),
                condition,
                strategy: JoinStrategy::NestedLoop,
                join_kind: join.kind,
            };
        }
    } else {
        // Same-DB join: normal plan node
        root = PlanNode::Join {
            left: Box::new(root),
            right: Box::new(right_node),
            condition: join.condition.clone().unwrap_or(Expression::Boolean(true)),
            strategy: match join.kind {
                JoinKind::Cross => JoinStrategy::NestedLoop,
                _ => JoinStrategy::Hash,
            },
            join_kind: join.kind,
        };
    }
}
```

- [ ] **Step 6: Add cross-join guard in executor instead of planner**

Rather than making `plan_query` return `Result` (which would require changing many call sites), add the guard in `execute_node`. Before processing a `PlanNode::Join` with cross-DB and cross join semantics, check if it's bounded. Add this check at the top of the `PlanNode::Join` match arm in `execute_node`:

```rust
PlanNode::Join {
    left,
    right,
    condition,
    strategy,
    join_kind,
} => {
    // Guard: reject unbounded cross-DB cross joins
    if *join_kind == JoinKind::Cross || matches!(condition, Expression::Boolean(true)) {
        let left_db = find_single_db(left);
        let right_db = find_single_db(right);
        if let (Some((ln, _)), Some((rn, _))) = (&left_db, &right_db) {
            if ln != rn {
                return Err(RiverError::Unsupported(
                    "Cross-database cross joins require a LIMIT clause to prevent \
                     unbounded result sets. Add 'limit N' to your query.".into()
                ));
            }
        }
    }
    // ... existing join execution code
}
```

- [ ] **Step 7: Update `collect_databases` to handle `SemiJoinFetch`**

In the `collect_databases` function, add a match arm:

```rust
PlanNode::SemiJoinFetch { build, probe_database, .. } => {
    collect_databases(build, out);
    out.push(probe_database.clone());
}
```

- [ ] **Step 8: Update `find_single_db` to handle `SemiJoinFetch`**

```rust
PlanNode::SemiJoinFetch { .. } => None,
```

- [ ] **Step 9: Verify it compiles**

Run: `cargo check 2>&1 | head -30`
Expected: May have warnings but no errors related to `SemiJoinFetch` being unhandled.

- [ ] **Step 10: Commit**

```bash
git add src/engine/planner.rs
git commit -m "feat: add SemiJoinFetch plan node for cross-DB equi-join optimization"
```

---

## Task 2: Add IN-list helper to the translator

**Files:**
- Modify: `src/engine/translator.rs`

- [ ] **Step 1: Add `translate_in_list` public function for SQL dialects**

Add at the end of `src/engine/translator.rs` (before the MongoDB section):

```rust
use crate::adapters::Value;

pub fn translate_in_list(column: &str, values: &[Value], dialect: &dyn SqlDialect) -> String {
    if values.is_empty() {
        return "1=0".to_string();
    }
    let col = dialect.quote_ident(column);
    let vals: Vec<String> = values
        .iter()
        .filter_map(|v| match v {
            Value::Null => None,
            Value::Int(i) => Some(i.to_string()),
            Value::Float(f) => Some(f.to_string()),
            Value::String(s) => Some(format!("'{}'", escape_sql_string(s))),
            Value::Bool(b) => Some(if *b { "TRUE".into() } else { "FALSE".into() }),
        })
        .collect();
    if vals.is_empty() {
        return "1=0".to_string();
    }
    format!("{} IN ({})", col, vals.join(", "))
}
```

- [ ] **Step 2: Add `translate_in_list_mongo` public function for MongoDB**

```rust
pub fn translate_in_list_mongo(column: &str, values: &[Value]) -> String {
    let vals: Vec<JsonValue> = values
        .iter()
        .filter_map(|v| match v {
            Value::Null => None,
            Value::Int(i) => Some(json!(i)),
            Value::Float(f) => Some(json!(f)),
            Value::String(s) => Some(json!(s)),
            Value::Bool(b) => Some(json!(b)),
        })
        .collect();
    let filter = json!({ column: { "$in": vals } });
    serde_json::to_string(&filter).unwrap_or_else(|_| "{}".to_string())
}
```

- [ ] **Step 3: Add `build_scan_with_in_filter` that constructs a full query string for the probe side**

```rust
pub fn build_probe_query_sql(
    table: &str,
    key_column: &str,
    values: &[Value],
    dialect: &dyn SqlDialect,
) -> String {
    let table_quoted = dialect.quote_ident(table);
    let in_clause = translate_in_list(key_column, values, dialect);
    format!("SELECT * FROM {} WHERE {}", table_quoted, in_clause)
}

pub fn build_probe_query_mongo(
    collection: &str,
    key_column: &str,
    values: &[Value],
    database: &str,
) -> String {
    let vals: Vec<JsonValue> = values
        .iter()
        .filter_map(|v| match v {
            Value::Null => None,
            Value::Int(i) => Some(json!(i)),
            Value::Float(f) => Some(json!(f)),
            Value::String(s) => Some(json!(s)),
            Value::Bool(b) => Some(json!(b)),
        })
        .collect();
    let pipeline = json!([
        { "$match": { key_column: { "$in": vals } } }
    ]);
    let cmd = json!({
        "database": database,
        "collection": collection,
        "pipeline": pipeline,
    });
    serde_json::to_string(&cmd).unwrap_or_else(|_| "{}".to_string())
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: Clean compile or only unrelated warnings.

- [ ] **Step 5: Commit**

```bash
git add src/engine/translator.rs
git commit -m "feat: add IN-list translation helpers for cross-DB pushdown"
```

---

## Task 3: Implement `SemiJoinFetch` execution in the executor

**Files:**
- Modify: `src/engine/executor.rs`

- [ ] **Step 1: Add the `SemiJoinFetch` match arm in `execute_node`**

In the `execute_node` function, add a new match arm before the existing `PlanNode::Join` arm:

```rust
PlanNode::SemiJoinFetch {
    build,
    probe_source,
    probe_database,
    build_key,
    probe_key,
    join_kind,
    condition,
} => {
    execute_semi_join_fetch(
        build, probe_source, probe_database, build_key, probe_key,
        *join_kind, condition, adapters,
    ).await
}
```

- [ ] **Step 2: Import the new translator functions**

At the top of `executor.rs`, ensure these are imported:

```rust
use crate::engine::planner::CROSS_DB_BATCH_SIZE;
```

- [ ] **Step 3: Implement `execute_semi_join_fetch`**

```rust
async fn execute_semi_join_fetch(
    build: &PlanNode,
    probe_source: &Source,
    probe_database: &(String, DatabaseKind),
    build_key: &Expression,
    probe_key: &Expression,
    join_kind: JoinKind,
    condition: &Expression,
    adapters: &HashMap<String, Box<dyn DatabaseAdapter>>,
) -> Result<QueryResult, RiverError> {
    // 1. Execute the build side
    let build_result = Box::pin(execute_node(build, adapters)).await?;

    if build_result.rows.is_empty() {
        // No build rows → no matches possible (for inner join)
        if matches!(join_kind, JoinKind::Inner) {
            return Ok(empty_result());
        }
        // For left join, return build rows with null probe columns
        // We don't know probe columns yet, so return empty
        return Ok(empty_result());
    }

    // 2. Extract distinct join keys from build result
    let build_key_col = match build_key {
        Expression::Ident(name) | Expression::QualifiedIdent { field: name, .. } => name.clone(),
        _ => {
            return Err(RiverError::Unsupported(
                "SemiJoinFetch requires a simple column reference as build key".into(),
            ));
        }
    };
    let probe_key_col = match probe_key {
        Expression::Ident(name) | Expression::QualifiedIdent { field: name, .. } => name.clone(),
        _ => {
            return Err(RiverError::Unsupported(
                "SemiJoinFetch requires a simple column reference as probe key".into(),
            ));
        }
    };

    let build_key_idx = build_result
        .columns
        .iter()
        .position(|c| c == &build_key_col)
        .ok_or_else(|| {
            RiverError::Unsupported(format!(
                "Build key column '{}' not found in build result columns: {:?}",
                build_key_col, build_result.columns
            ))
        })?;

    let mut distinct_keys: Vec<Value> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for row in &build_result.rows {
        let key = row.get(build_key_idx).cloned().unwrap_or(Value::Null);
        if key != Value::Null && seen.insert(key.clone()) {
            distinct_keys.push(key);
        }
    }

    if distinct_keys.is_empty() {
        return Ok(empty_result());
    }

    // 3. Batch-fetch from probe side
    let (db_name, db_kind) = probe_database;
    let adapter = adapters.get(db_name).ok_or_else(|| {
        RiverError::Unsupported(format!("no adapter connected for '{}'", db_name))
    })?;

    let table_name = match &probe_source.kind {
        SourceKind::Table(t) => t.clone(),
        _ => probe_source.name.clone(),
    };

    let mut probe_rows: Vec<Vec<Value>> = Vec::new();
    let mut probe_columns: Vec<String> = Vec::new();

    for chunk in distinct_keys.chunks(CROSS_DB_BATCH_SIZE) {
        let native_query = match db_kind {
            DatabaseKind::MongoDB => {
                translate_for_kind(
                    &build_probe_query_obj(&table_name, &probe_key_col, chunk),
                    db_kind,
                )
            }
            _ => {
                let dialect: Box<dyn crate::engine::translator::SqlDialect> = match db_kind {
                    DatabaseKind::Postgres => Box::new(crate::engine::translator::PostgresDialect),
                    DatabaseKind::MySQL => Box::new(crate::engine::translator::MySQLDialect),
                    DatabaseKind::MSSQL => Box::new(crate::engine::translator::MSSQLDialect),
                    DatabaseKind::SQLite => Box::new(crate::engine::translator::SQLiteDialect),
                    DatabaseKind::MongoDB => unreachable!(),
                };
                crate::engine::translator::build_probe_query_sql(
                    &table_name, &probe_key_col, chunk, dialect.as_ref(),
                )
            }
        };

        let batch_result = adapter.execute(&native_query).await?;
        if probe_columns.is_empty() {
            probe_columns = batch_result.columns.clone();
        }
        probe_rows.extend(batch_result.rows);
    }

    let probe_result = QueryResult {
        columns: probe_columns,
        rows: probe_rows,
        elapsed: std::time::Duration::default(),
        rows_affected: 0,
    };

    // 4. Hash join the build and probe results
    hash_join(build_result, probe_result, condition, join_kind)
}
```

- [ ] **Step 4: Add helper to build a Query object for MongoDB probe**

```rust
fn build_probe_query_obj(table: &str, key_column: &str, values: &[Value]) -> Query {
    let in_values: Vec<Expression> = values
        .iter()
        .map(|v| match v {
            Value::Int(i) => Expression::Integer(*i),
            Value::Float(f) => Expression::Number(*f),
            Value::String(s) => Expression::String(s.clone()),
            Value::Bool(b) => Expression::Boolean(*b),
            Value::Null => Expression::Null,
        })
        .collect();

    let mut q = Query::default();
    q.sources.push(Source {
        name: table.to_string(),
        alias: None,
        connection: None,
        kind: SourceKind::Table(table.to_string()),
    });
    q.projection = vec![Projection::Wildcard];
    q.filter = Some(Expression::BinaryOp {
        op: BinaryOp::In,
        left: Box::new(Expression::Ident(key_column.to_string())),
        right: Box::new(Expression::Array(in_values)),
    });
    q
}
```

- [ ] **Step 5: Update `replace_cte_scans` to handle `SemiJoinFetch`**

In the `replace_cte_scans` function, add:

```rust
PlanNode::SemiJoinFetch { build, .. } => {
    replace_cte_scans(build, cte_data);
}
```

- [ ] **Step 6: Update `collect_single_db_query` to handle `SemiJoinFetch`**

It should return `None` (SemiJoinFetch is always cross-DB, can't be pushed to a single DB):

Already handled by the `_` arm, but add explicitly for clarity if desired.

- [ ] **Step 7: Verify it compiles**

Run: `cargo check 2>&1 | head -30`
Expected: Clean compile.

- [ ] **Step 8: Run existing tests to verify no regressions**

Run: `cargo test --lib 2>&1 | tail -20`
Expected: All existing unit tests pass.

- [ ] **Step 9: Commit**

```bash
git add src/engine/executor.rs
git commit -m "feat: implement SemiJoinFetch execution with batched IN pushdown"
```

---

## Task 4: Write unit tests for the cross-DB optimization

**Files:**
- Modify: `src/engine/executor.rs` (tests module at bottom)

- [ ] **Step 1: Add test for cross-join guard rejection**

In the `#[cfg(test)] mod tests` section at the bottom of `executor.rs`:

```rust
#[tokio::test]
async fn cross_db_cross_join_without_limit_rejected() {
    let mut adapters: HashMap<String, Box<dyn DatabaseAdapter>> = HashMap::new();
    adapters.insert("pg".into(), Box::new(MockAdapter));
    adapters.insert("mysql".into(), Box::new(MockAdapter));
    let source_db = vec![
        ("pg".into(), DatabaseKind::Postgres),
        ("mysql".into(), DatabaseKind::MySQL),
    ];

    let query = r#"find [u.name, p.name] from users@pg as u cross join products@mysql as p"#;
    let stmt = parse(query).expect("parse failed");
    let result = execute_statement(&stmt, &source_db, &adapters).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("LIMIT"),
        "Expected error about LIMIT, got: {}",
        err_msg
    );
}
```

- [ ] **Step 2: Add test for cross-join with LIMIT allowed**

```rust
#[tokio::test]
async fn cross_db_cross_join_with_limit_allowed() {
    let mut adapters: HashMap<String, Box<dyn DatabaseAdapter>> = HashMap::new();
    adapters.insert("pg".into(), Box::new(MockAdapter));
    adapters.insert("mysql".into(), Box::new(MockAdapter));
    let source_db = vec![
        ("pg".into(), DatabaseKind::Postgres),
        ("mysql".into(), DatabaseKind::MySQL),
    ];

    let query = r#"find [u.name, p.name] from users@pg as u cross join products@mysql as p limit 5"#;
    let stmt = parse(query).expect("parse failed");
    let result = execute_statement(&stmt, &source_db, &adapters).await;
    assert!(result.is_ok());
    assert!(result.unwrap().rows.len() <= 5);
}
```

- [ ] **Step 3: Add test for SemiJoinFetch path (equi-join cross-DB)**

This needs a mock adapter that responds differently based on query content. Modify or extend `MockAdapter` to handle IN clauses:

```rust
#[tokio::test]
async fn semi_join_fetch_equi_join() {
    let mut adapters: HashMap<String, Box<dyn DatabaseAdapter>> = HashMap::new();
    adapters.insert("pg".into(), Box::new(MockAdapter));
    adapters.insert("mysql".into(), Box::new(MockAdapter));
    let source_db = vec![
        ("pg".into(), DatabaseKind::Postgres),
        ("mysql".into(), DatabaseKind::MySQL),
    ];

    let query = r#"find [u.name, o.total] from users@pg as u join orders@mysql as o on u.id = o.user_id"#;
    let stmt = parse(query).expect("parse failed");
    let result = execute_statement(&stmt, &source_db, &adapters).await;
    assert!(result.is_ok());
    let qr = result.unwrap();
    assert!(!qr.rows.is_empty());
}
```

- [ ] **Step 4: Run unit tests**

Run: `cargo test --lib 2>&1 | tail -20`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/engine/executor.rs
git commit -m "test: add unit tests for cross-DB join guard and SemiJoinFetch"
```

---

## Task 5: Add translator unit tests for IN-list generation

**Files:**
- Modify: `src/engine/translator.rs` (add tests module)

- [ ] **Step 1: Add `#[cfg(test)]` module at the end of translator.rs**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::Value;

    #[test]
    fn in_list_postgres_ints() {
        let values = vec![Value::Int(1), Value::Int(2), Value::Int(3)];
        let sql = translate_in_list("user_id", &values, &PostgresDialect);
        assert_eq!(sql, r#""user_id" IN (1, 2, 3)"#);
    }

    #[test]
    fn in_list_mysql_strings() {
        let values = vec![
            Value::String("a".into()),
            Value::String("b".into()),
        ];
        let sql = translate_in_list("name", &values, &MySQLDialect);
        assert_eq!(sql, "`name` IN ('a', 'b')");
    }

    #[test]
    fn in_list_empty_returns_false() {
        let values: Vec<Value> = vec![];
        let sql = translate_in_list("id", &values, &PostgresDialect);
        assert_eq!(sql, "1=0");
    }

    #[test]
    fn in_list_nulls_skipped() {
        let values = vec![Value::Null, Value::Int(1), Value::Null];
        let sql = translate_in_list("id", &values, &SQLiteDialect);
        assert_eq!(sql, r#""id" IN (1)"#);
    }

    #[test]
    fn in_list_all_nulls_returns_false() {
        let values = vec![Value::Null, Value::Null];
        let sql = translate_in_list("id", &values, &PostgresDialect);
        assert_eq!(sql, "1=0");
    }

    #[test]
    fn probe_query_sql_postgres() {
        let values = vec![Value::Int(1), Value::Int(2)];
        let sql = build_probe_query_sql("users", "id", &values, &PostgresDialect);
        assert_eq!(sql, r#"SELECT * FROM "users" WHERE "id" IN (1, 2)"#);
    }

    #[test]
    fn probe_query_sql_mysql() {
        let values = vec![Value::Int(10)];
        let sql = build_probe_query_sql("orders", "user_id", &values, &MySQLDialect);
        assert_eq!(sql, "SELECT * FROM `orders` WHERE `user_id` IN (10)");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib translator::tests 2>&1`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add src/engine/translator.rs
git commit -m "test: add unit tests for IN-list translator helpers"
```

---

## Task 6: Set up integration test infrastructure

**Files:**
- Create: `tests/integrations/main.rs`
- Create: `tests/integrations/helpers.rs`
- Modify: `tests/common/mod.rs`

- [ ] **Step 1: Create `tests/integrations/main.rs`**

```rust
mod helpers;
mod t01_basic_queries;
mod t02_filtering;
mod t03_joins;
mod t04_aggregation;
mod t05_window_functions;
mod t06_advanced_queries;
mod t07_cross_database;
mod t08_data_modification;
mod t09_meta_commands;

#[path = "../common/mod.rs"]
mod common;
```

- [ ] **Step 2: Create `tests/integrations/helpers.rs`**

This contains seed-formula computation functions matching `infra/seed.py`:

```rust
#![allow(dead_code)]

pub const ROWS: usize = 10_000;

const FIRST_NAMES: &[&str] = &[
    "Alice", "Bob", "Carol", "Dave", "Eve", "Frank", "Grace", "Henry",
    "Iris", "Jack", "Kate", "Liam", "Mia", "Noah", "Olivia", "Paul",
    "Quinn", "Rose", "Sam", "Tina", "Uma", "Vince", "Wendy", "Xander",
    "Yuki", "Zara",
];

const LAST_NAMES: &[&str] = &[
    "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller",
    "Davis", "Rodriguez", "Martinez", "Hernandez", "Lopez", "Wilson",
    "Anderson", "Thomas", "Taylor", "Moore", "Jackson", "Martin", "Lee",
    "Perez", "Thompson", "White", "Harris", "Sanchez", "Clark",
];

const DEPARTMENTS: &[&str] = &[
    "Engineering", "Sales", "Marketing", "Support", "Finance",
    "HR", "Legal", "Product", "Design", "Operations",
];

const USER_STATUSES: &[&str] = &["active", "inactive", "suspended", "pending"];

const CATEGORIES: &[&str] = &[
    "Electronics", "Clothing", "Books", "Home", "Sports",
    "Food", "Toys", "Health", "Automotive", "Garden",
];

const PRODUCT_ADJ: &[&str] = &[
    "Premium", "Basic", "Pro", "Ultra", "Mini",
    "Super", "Mega", "Elite", "Nano", "Max",
];

const PRODUCT_NOUN: &[&str] = &[
    "Widget", "Gadget", "Device", "Tool", "Kit",
    "Pack", "Set", "System", "Module", "Unit",
];

const ORDER_STATUSES: &[&str] = &["pending", "paid", "shipped", "delivered", "cancelled", "refunded"];

pub struct UserRow {
    pub name: String,
    pub email: String,
    pub department: String,
    pub salary: i64,
    pub status: String,
    pub is_verified: bool,
}

pub fn user_row(i: usize) -> UserRow {
    let fn_name = FIRST_NAMES[(i * 7 + 3) % FIRST_NAMES.len()];
    let ln_name = LAST_NAMES[(i * 13 + 5) % LAST_NAMES.len()];
    let name = format!("{} {}", fn_name, ln_name);
    let email = format!("{}.{}{i}@example.com", fn_name.to_lowercase(), ln_name.to_lowercase());
    let dept = DEPARTMENTS[(i * 11 + 2) % DEPARTMENTS.len()].to_string();
    let salary = 35000 + (i as i64 * 17) % 115000;
    let status = USER_STATUSES[(i * 3) % USER_STATUSES.len()].to_string();
    let is_verified = i % 3 == 0;
    UserRow { name, email, department: dept, salary, status, is_verified }
}

pub struct ProductRow {
    pub name: String,
    pub category: String,
    pub price: f64,
    pub stock: i64,
    pub rating: f64,
    pub is_active: bool,
}

pub fn product_row(i: usize) -> ProductRow {
    let adj = PRODUCT_ADJ[(i * 7) % PRODUCT_ADJ.len()];
    let noun = PRODUCT_NOUN[(i * 3) % PRODUCT_NOUN.len()];
    let name = format!("{} {} {}", adj, noun, i);
    let category = CATEGORIES[(i * 11) % CATEGORIES.len()].to_string();
    let price = ((99 + (i * 31) % 99901) as f64) / 100.0;
    let stock = (i as i64 * 7) % 500;
    let rating = (100 + (i * 13) % 400) as f64 / 100.0;
    let is_active = i % 10 != 0;
    ProductRow { name, category, price, stock, rating, is_active }
}

pub struct OrderRow {
    pub user_id: i64,
    pub status: String,
    pub total: f64,
}

pub fn order_row(i: usize) -> OrderRow {
    let user_id = ((i * 7 + 1) % ROWS + 1) as i64;
    let status = ORDER_STATUSES[(i * 5) % ORDER_STATUSES.len()].to_string();
    let total = (500 + (i * 43) % 50000) as f64 / 100.0;
    OrderRow { user_id, status, total }
}

pub struct OrderItemRow {
    pub order_id: i64,
    pub product_id: i64,
    pub quantity: i64,
    pub unit_price: f64,
}

pub fn order_item_row(i: usize) -> OrderItemRow {
    let order_id = ((i * 3 + 1) % ROWS + 1) as i64;
    let product_id = ((i * 11 + 2) % ROWS + 1) as i64;
    let quantity = 1 + (i as i64 * 7) % 10;
    let unit_price = ((99 + (i * 31) % 99901) as f64) / 100.0;
    OrderItemRow { order_id, product_id, quantity, unit_price }
}

/// Count users matching a department
pub fn count_users_in_department(dept: &str) -> usize {
    (1..=ROWS).filter(|&i| user_row(i).department == dept).count()
}

/// Count orders matching a status
pub fn count_orders_with_status(status: &str) -> usize {
    (1..=ROWS).filter(|&i| order_row(i).status == status).count()
}
```

- [ ] **Step 3: Add `assert_cross_db_consistency` to `tests/common/mod.rs`**

Append to the file:

```rust
/// Run the same query template against multiple connections and assert identical row counts.
/// `query_template` uses `{conn}` as placeholder for the connection name.
/// Example: `"find [name] from users@{conn} where department = \"Engineering\" limit 10"`
pub async fn assert_cross_db_consistency(
    ctx: &TestContext,
    query_template: &str,
    connections: &[&str],
) {
    let mut results: Vec<(String, QueryResult)> = Vec::new();
    for conn in connections {
        let query = query_template.replace("{conn}", conn);
        let result = execute_river(ctx, &query)
            .await
            .unwrap_or_else(|e| panic!("Query failed on connection '{}': {}", conn, e));
        results.push((conn.to_string(), result));
    }

    let first_count = results[0].1.rows.len();
    for (conn, result) in &results[1..] {
        assert_eq!(
            result.rows.len(),
            first_count,
            "Row count mismatch: '{}' returned {} rows but '{}' returned {}",
            results[0].0, first_count, conn, result.rows.len()
        );
    }
}

/// Assert that the result contains exactly the expected rows (order-independent).
pub fn assert_exact_rows(result: &QueryResult, expected: &[Vec<Value>]) {
    assert_eq!(
        result.rows.len(),
        expected.len(),
        "Row count mismatch: expected {} but got {}",
        expected.len(),
        result.rows.len()
    );
    for exp_row in expected {
        assert!(
            result.rows.contains(exp_row),
            "Expected row not found: {:?}\nActual rows (first 5): {:?}",
            exp_row,
            &result.rows[..std::cmp::min(5, result.rows.len())]
        );
    }
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check --tests 2>&1 | head -30`
Expected: Compiles (test modules may have empty files still).

- [ ] **Step 5: Commit**

```bash
git add tests/integrations/ tests/common/mod.rs
git commit -m "feat: add integration test infrastructure with seed helpers"
```

---

## Task 7: Integration tests — Doc 01 (Basic Queries)

**Files:**
- Create: `tests/integrations/t01_basic_queries.rs`

- [ ] **Step 1: Write the test file**

```rust
//! E2E tests for doc 01: Getting Started (basic find, select, limit, offset, order by).

use crate::common::{
    assert_columns, assert_no_nulls, assert_ordered_asc, assert_ordered_desc,
    assert_row_count, assert_row_count_gte, execute_river, TestContext,
};
use crate::helpers;
use river::adapters::Value;

#[tokio::test]
async fn find_all_users() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find users@pg").await.expect("find users failed");
    assert_row_count(&result, helpers::ROWS);
}

#[tokio::test]
async fn find_with_where() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find users@pg where status = "active""#)
        .await
        .expect("find with where failed");
    let expected_count = (1..=helpers::ROWS)
        .filter(|&i| helpers::user_row(i).status == "active")
        .count();
    assert_row_count(&result, expected_count);
}

#[tokio::test]
async fn select_columns() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find [name, email] from users@pg limit 5")
        .await
        .expect("select columns failed");
    assert_row_count(&result, 5);
    assert_columns(&result, &["name", "email"]);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "email");
}

#[tokio::test]
async fn limit_results() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find users@pg limit 10")
        .await
        .expect("limit failed");
    assert_row_count(&result, 10);
}

#[tokio::test]
async fn order_by_desc() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find [name, salary] from users@pg order by salary desc limit 20")
        .await
        .expect("order by desc failed");
    assert_row_count(&result, 20);
    assert_ordered_desc(&result, "salary");
}

#[tokio::test]
async fn order_by_asc() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find [name, salary] from users@pg order by salary asc limit 20")
        .await
        .expect("order by asc failed");
    assert_row_count(&result, 20);
    assert_ordered_asc(&result, "salary");
}

#[tokio::test]
async fn limit_and_offset() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find [name] from users@pg order by name asc limit 20 offset 40")
        .await
        .expect("limit offset failed");
    assert_row_count(&result, 20);
}

#[tokio::test]
async fn combined_query() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, email, department, salary] from users@pg where status = "active" and salary > 50000 order by salary desc limit 10"#,
    )
    .await
    .expect("combined query failed");
    assert_row_count(&result, 10);
    assert_ordered_desc(&result, "salary");
    // Verify all salaries > 50000
    for row in &result.rows {
        let salary_idx = result.columns.iter().position(|c| c == "salary").unwrap();
        match &row[salary_idx] {
            Value::Int(s) => assert!(*s > 50000, "salary {} not > 50000", s),
            Value::Float(s) => assert!(*s > 50000.0, "salary {} not > 50000", s),
            Value::String(s) => {
                let v: f64 = s.parse().unwrap();
                assert!(v > 50000.0, "salary {} not > 50000", v);
            }
            other => panic!("unexpected salary type: {:?}", other),
        }
    }
}

#[tokio::test]
async fn cross_db_consistency_basic() {
    let ctx = TestContext::new().await;
    crate::common::assert_cross_db_consistency(
        &ctx,
        r#"find [name, department] from users@{conn} where department = "Engineering" limit 50"#,
        &["pg", "mysql", "sqlite"],
    )
    .await;
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --test integrations t01 2>&1 | tail -20`
Expected: All pass (requires Docker DBs running).

- [ ] **Step 3: Commit**

```bash
git add tests/integrations/t01_basic_queries.rs
git commit -m "test: add E2E tests for doc 01 (basic queries)"
```

---

## Task 8: Integration tests — Doc 02 (Filtering & Expressions)

**Files:**
- Create: `tests/integrations/t02_filtering.rs`

- [ ] **Step 1: Write the test file**

```rust
//! E2E tests for doc 02: Filtering & Expressions.

use crate::common::{
    assert_all_match, assert_no_nulls, assert_row_count, assert_row_count_gte,
    execute_river, TestContext,
};
use crate::helpers;
use river::adapters::Value;

#[tokio::test]
async fn comparison_gt() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find [name, salary] from users@pg where salary > 100000")
        .await
        .expect("gt filter failed");
    assert_row_count_gte(&result, 1);
    assert_all_match(&result, "salary", |v| match v {
        Value::Int(s) => *s > 100000,
        Value::Float(s) => *s > 100000.0,
        Value::String(s) => s.parse::<f64>().map(|n| n > 100000.0).unwrap_or(false),
        _ => false,
    });
}

#[tokio::test]
async fn comparison_gte() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find [name, salary] from users@pg where salary >= 75000 limit 50")
        .await
        .expect("gte filter failed");
    assert_row_count(&result, 50);
    assert_all_match(&result, "salary", |v| match v {
        Value::Int(s) => *s >= 75000,
        Value::Float(s) => *s >= 75000.0,
        Value::String(s) => s.parse::<f64>().map(|n| n >= 75000.0).unwrap_or(false),
        _ => false,
    });
}

#[tokio::test]
async fn comparison_neq() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find users@pg where status != "active" limit 50"#)
        .await
        .expect("neq filter failed");
    assert_row_count(&result, 50);
    assert_all_match(&result, "status", |v| match v {
        Value::String(s) => s != "active",
        _ => true,
    });
}

#[tokio::test]
async fn logical_and() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find users@pg where status = "active" and department = "Engineering""#,
    )
    .await
    .expect("AND filter failed");
    let expected = (1..=helpers::ROWS)
        .filter(|&i| {
            let u = helpers::user_row(i);
            u.status == "active" && u.department == "Engineering"
        })
        .count();
    assert_row_count(&result, expected);
}

#[tokio::test]
async fn logical_or() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find users@pg where department = "Sales" or department = "Marketing""#,
    )
    .await
    .expect("OR filter failed");
    let expected = (1..=helpers::ROWS)
        .filter(|&i| {
            let u = helpers::user_row(i);
            u.department == "Sales" || u.department == "Marketing"
        })
        .count();
    assert_row_count(&result, expected);
}

#[tokio::test]
async fn between_numbers() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find products@pg where price between 10 and 50")
        .await
        .expect("BETWEEN failed");
    let expected = (1..=helpers::ROWS)
        .filter(|&i| {
            let p = helpers::product_row(i);
            p.price >= 10.0 && p.price <= 50.0
        })
        .count();
    assert_row_count(&result, expected);
}

#[tokio::test]
async fn in_list() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find users@pg where status in ("active", "pending")"#,
    )
    .await
    .expect("IN filter failed");
    let expected = (1..=helpers::ROWS)
        .filter(|&i| {
            let u = helpers::user_row(i);
            u.status == "active" || u.status == "pending"
        })
        .count();
    assert_row_count(&result, expected);
}

#[tokio::test]
async fn like_pattern() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find users@pg where name like "%Smith%" limit 100"#)
        .await
        .expect("LIKE filter failed");
    assert_row_count_gte(&result, 1);
    assert_all_match(&result, "name", |v| match v {
        Value::String(s) => s.contains("Smith"),
        _ => false,
    });
}

#[tokio::test]
async fn cross_db_consistency_filtering() {
    let ctx = TestContext::new().await;
    crate::common::assert_cross_db_consistency(
        &ctx,
        r#"find [name, salary] from users@{conn} where salary > 100000"#,
        &["pg", "mysql", "sqlite"],
    )
    .await;
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test integrations t02 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add tests/integrations/t02_filtering.rs
git commit -m "test: add E2E tests for doc 02 (filtering & expressions)"
```

---

## Task 9: Integration tests — Doc 03 (Joins)

**Files:**
- Create: `tests/integrations/t03_joins.rs`

- [ ] **Step 1: Write the test file**

```rust
//! E2E tests for doc 03: Joins.

use crate::common::{
    assert_no_nulls, assert_row_count, assert_row_count_gte,
    execute_river, TestContext,
};
use river::adapters::Value;

#[tokio::test]
async fn inner_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, o.total] from users@pg as u join orders@pg as o on u.id = o.user_id limit 100",
    )
    .await
    .expect("inner join failed");
    assert_row_count(&result, 100);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");
}

#[tokio::test]
async fn left_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, o.total] from users@pg as u left join orders@pg as o on u.id = o.user_id limit 200",
    )
    .await
    .expect("left join failed");
    assert_row_count(&result, 200);
    assert_no_nulls(&result, "name");
}

#[tokio::test]
async fn cross_join_with_limit() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, p.name] from users@pg as u cross join products@pg as p limit 25",
    )
    .await
    .expect("cross join with limit failed");
    assert_row_count(&result, 25);
}

#[tokio::test]
async fn multiple_joins() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, o.total, p.name] from users@pg as u join orders@pg as o on u.id = o.user_id join order_items@pg as oi on o.id = oi.order_id join products@pg as p on oi.product_id = p.id limit 50",
    )
    .await
    .expect("multiple joins failed");
    assert_row_count(&result, 50);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");
}

#[tokio::test]
async fn self_join() {
    let ctx = TestContext::new().await;
    // Join users with users on same department — each user should match itself and others
    let result = execute_river(
        &ctx,
        "find [a.name, b.name] from users@pg as a join users@pg as b on a.department = b.department limit 50",
    )
    .await
    .expect("self join failed");
    assert_row_count(&result, 50);
}

#[tokio::test]
async fn join_with_filter() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total, o.status] from users@pg as u join orders@pg as o on u.id = o.user_id where o.status = "paid" and o.total > 100 order by o.total desc limit 20"#,
    )
    .await
    .expect("join with filter failed");
    assert_row_count(&result, 20);
    for row in &result.rows {
        let status_idx = result.columns.iter().position(|c| c == "status").unwrap();
        assert_eq!(row[status_idx], Value::String("paid".into()));
    }
}

#[tokio::test]
async fn join_full_result_count() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, o.total] from users@pg as u join orders@pg as o on u.id = o.user_id",
    )
    .await
    .expect("full join count failed");
    // Each order has exactly one user_id mapping to a user (10k orders)
    assert_row_count_gte(&result, 5000);
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test integrations t03 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add tests/integrations/t03_joins.rs
git commit -m "test: add E2E tests for doc 03 (joins)"
```

---

## Task 10: Integration tests — Doc 04 (Aggregation)

**Files:**
- Create: `tests/integrations/t04_aggregation.rs`

- [ ] **Step 1: Write the test file**

```rust
//! E2E tests for doc 04: Aggregation & Grouping.

use crate::common::{
    assert_row_count, assert_row_count_gte, execute_river, TestContext,
};
use crate::helpers;
use river::adapters::Value;

#[tokio::test]
async fn count_all() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find [count(*)] from users@pg")
        .await
        .expect("count(*) failed");
    assert_row_count(&result, 1);
    match &result.rows[0][0] {
        Value::Int(n) => assert_eq!(*n, helpers::ROWS as i64),
        other => panic!("Expected Int, got {:?}", other),
    }
}

#[tokio::test]
async fn sum_total() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find [sum(total)] from orders@pg")
        .await
        .expect("sum(total) failed");
    assert_row_count(&result, 1);
    match &result.rows[0][0] {
        Value::Int(n) => assert!(*n > 0, "sum should be positive"),
        Value::Float(n) => assert!(*n > 0.0, "sum should be positive"),
        Value::String(s) => {
            let n: f64 = s.parse().expect("sum should be numeric");
            assert!(n > 0.0, "sum should be positive");
        }
        other => panic!("Expected numeric, got {:?}", other),
    }
}

#[tokio::test]
async fn avg_salary() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find [avg(salary)] from users@pg")
        .await
        .expect("avg(salary) failed");
    assert_row_count(&result, 1);
}

#[tokio::test]
async fn min_max_price() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find [min(price), max(price)] from products@pg")
        .await
        .expect("min/max failed");
    assert_row_count(&result, 1);
    assert_eq!(result.columns.len(), 2);
}

#[tokio::test]
async fn group_by_department() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [department, count(*) as headcount] from users@pg group by department",
    )
    .await
    .expect("group by failed");
    // 10 departments
    assert_row_count(&result, 10);
}

#[tokio::test]
async fn group_by_with_having() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [department, count(*) as cnt] from users@pg group by department having count(*) > 500",
    )
    .await
    .expect("having failed");
    // Each department has ~1000 users, so all 10 should pass having > 500
    assert_row_count_gte(&result, 1);
}

#[tokio::test]
async fn multiple_aggregates() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [count(*) as total_orders, sum(total) as revenue, avg(total) as avg_order, min(total) as smallest, max(total) as largest] from orders@pg where status = "paid""#,
    )
    .await
    .expect("multiple aggregates failed");
    assert_row_count(&result, 1);
    assert_eq!(result.columns.len(), 5);
}

#[tokio::test]
async fn group_by_multiple_columns() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [department, status, count(*) as cnt] from users@pg group by department, status",
    )
    .await
    .expect("multi-column group by failed");
    // 10 departments * 4 statuses = up to 40 groups
    assert_row_count_gte(&result, 10);
}

#[tokio::test]
async fn cross_db_consistency_aggregation() {
    let ctx = TestContext::new().await;
    // Count should be same across all DBs
    let pg = execute_river(&ctx, "find [count(*)] from users@pg").await.unwrap();
    let mysql = execute_river(&ctx, "find [count(*)] from users@mysql").await.unwrap();
    let sqlite = execute_river(&ctx, "find [count(*)] from users@sqlite").await.unwrap();
    assert_eq!(pg.rows[0][0], mysql.rows[0][0]);
    assert_eq!(pg.rows[0][0], sqlite.rows[0][0]);
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test integrations t04 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add tests/integrations/t04_aggregation.rs
git commit -m "test: add E2E tests for doc 04 (aggregation)"
```

---

## Task 11: Integration tests — Doc 05 (Window Functions)

**Files:**
- Create: `tests/integrations/t05_window_functions.rs`

- [ ] **Step 1: Write the test file**

```rust
//! E2E tests for doc 05: Window Functions.
//!
//! Note: Window functions are pushed down to the native database.
//! MongoDB does not support window functions, so these tests target SQL databases.

use crate::common::{
    assert_no_nulls, assert_row_count, assert_row_count_gte, execute_river, TestContext,
};
use river::adapters::Value;

#[tokio::test]
async fn row_number_partitioned() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [name, department, salary, row_number() over (partition by department order by salary desc) as rank] from users@pg limit 100",
    )
    .await
    .expect("row_number failed");
    assert_row_count(&result, 100);
    assert_no_nulls(&result, "rank");
}

#[tokio::test]
async fn rank_function() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [name, salary, rank() over (order by salary desc) as position] from users@pg limit 50",
    )
    .await
    .expect("rank() failed");
    assert_row_count(&result, 50);
    assert_no_nulls(&result, "position");
}

#[tokio::test]
async fn dense_rank_function() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [name, salary, dense_rank() over (order by salary desc) as position] from users@pg limit 50",
    )
    .await
    .expect("dense_rank() failed");
    assert_row_count(&result, 50);
    assert_no_nulls(&result, "position");
}

#[tokio::test]
async fn lag_function() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [id, total, lag(total, 1) over (order by id) as prev_total] from orders@pg limit 50",
    )
    .await
    .expect("lag() failed");
    assert_row_count(&result, 50);
    // First row's lag should be NULL
    let lag_idx = result.columns.iter().position(|c| c == "prev_total").unwrap();
    assert_eq!(result.rows[0][lag_idx], Value::Null);
}

#[tokio::test]
async fn running_total() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [id, total, sum(total) over (order by id) as running_total] from orders@pg limit 20",
    )
    .await
    .expect("running total failed");
    assert_row_count(&result, 20);
    assert_no_nulls(&result, "running_total");
}

#[tokio::test]
async fn avg_over_partition() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [name, department, salary, avg(salary) over (partition by department) as dept_avg] from users@pg limit 50",
    )
    .await
    .expect("avg over partition failed");
    assert_row_count(&result, 50);
    assert_no_nulls(&result, "dept_avg");
}

#[tokio::test]
async fn window_functions_mysql() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [name, salary, row_number() over (order by salary desc) as rn] from users@mysql limit 20",
    )
    .await
    .expect("window function on MySQL failed");
    assert_row_count(&result, 20);
    assert_no_nulls(&result, "rn");
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test integrations t05 2>&1 | tail -20`
Expected: All pass (may skip if window functions not yet fully supported on all DBs).

- [ ] **Step 3: Commit**

```bash
git add tests/integrations/t05_window_functions.rs
git commit -m "test: add E2E tests for doc 05 (window functions)"
```

---

## Task 12: Integration tests — Doc 06 (Advanced Queries)

**Files:**
- Create: `tests/integrations/t06_advanced_queries.rs`

- [ ] **Step 1: Write the test file**

```rust
//! E2E tests for doc 06: Advanced Queries (CTEs, subqueries, UNION, DISTINCT, CASE, CAST).

use crate::common::{
    assert_no_nulls, assert_row_count, assert_row_count_gte, execute_river, TestContext,
};
use crate::helpers;
use river::adapters::Value;

#[tokio::test]
async fn single_cte() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"with active_users as ( find * from users@pg where status = "active" ) find [name, email] from active_users limit 50"#,
    )
    .await
    .expect("single CTE failed");
    assert_row_count(&result, 50);
    assert_no_nulls(&result, "name");
}

#[tokio::test]
async fn multiple_ctes() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"with paid_orders as ( find * from orders@pg where status = "paid" ), user_totals as ( find [user_id, sum(total) as revenue] from paid_orders group by user_id ) find [u.name, ut.revenue] from users@pg as u join user_totals as ut on u.id = ut.user_id where ut.revenue > 1000 order by ut.revenue desc limit 20"#,
    )
    .await
    .expect("multiple CTEs failed");
    assert_row_count(&result, 20);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "revenue");
}

#[tokio::test]
async fn distinct_query() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find distinct [department] from users@pg")
        .await
        .expect("DISTINCT failed");
    assert_row_count(&result, 10);
}

#[tokio::test]
async fn distinct_status() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find distinct [status] from orders@pg")
        .await
        .expect("DISTINCT status failed");
    assert_row_count(&result, 6);
}

#[tokio::test]
async fn cast_expression() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [name, cast(salary as string) as salary_str] from users@pg limit 5",
    )
    .await
    .expect("CAST failed");
    assert_row_count(&result, 5);
    assert_no_nulls(&result, "salary_str");
}

#[tokio::test]
async fn cte_with_aggregation() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"with dept_stats as ( find [department, count(*) as cnt, avg(salary) as avg_sal] from users@pg group by department ) find [department, cnt, avg_sal] from dept_stats where cnt > 500 order by avg_sal desc"#,
    )
    .await
    .expect("CTE with aggregation failed");
    assert_row_count_gte(&result, 1);
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test integrations t06 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add tests/integrations/t06_advanced_queries.rs
git commit -m "test: add E2E tests for doc 06 (advanced queries)"
```

---

## Task 13: Integration tests — Doc 07 (Cross-Database)

**Files:**
- Create: `tests/integrations/t07_cross_database.rs`

- [ ] **Step 1: Write the test file**

```rust
//! E2E tests for doc 07: Cross-Database Queries.
//! Tests the SemiJoinFetch optimization and cross-DB CTEs.

use crate::common::{
    assert_no_nulls, assert_row_count, assert_row_count_gte, execute_river, TestContext,
};
use river::adapters::Value;

#[tokio::test]
async fn cross_db_inner_join_pg_mysql() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, o.total] from users@pg as u join orders@mysql as o on u.id = o.user_id limit 100",
    )
    .await
    .expect("cross-db pg-mysql join failed");
    assert_row_count(&result, 100);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");
}

#[tokio::test]
async fn cross_db_inner_join_pg_sqlite() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, o.total] from users@pg as u join orders@sqlite as o on u.id = o.user_id limit 100",
    )
    .await
    .expect("cross-db pg-sqlite join failed");
    assert_row_count(&result, 100);
    assert_no_nulls(&result, "name");
}

#[tokio::test]
async fn cross_db_inner_join_pg_mongo() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, o.total] from users@pg as u join orders@mongo as o on u.id = o.user_id limit 100",
    )
    .await
    .expect("cross-db pg-mongo join failed");
    assert_row_count(&result, 100);
    assert_no_nulls(&result, "name");
}

#[tokio::test]
async fn cross_db_left_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, o.total] from users@pg as u left join orders@mysql as o on u.id = o.user_id limit 200",
    )
    .await
    .expect("cross-db left join failed");
    assert_row_count(&result, 200);
    assert_no_nulls(&result, "name");
}

#[tokio::test]
async fn cross_db_join_with_filter() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, o.total] from users@pg as u join orders@mysql as o on u.id = o.user_id where total > 100 limit 50",
    )
    .await
    .expect("cross-db join with filter failed");
    assert_row_count(&result, 50);
    let total_idx = result.columns.iter().position(|c| c == "total").unwrap();
    for row in &result.rows {
        match &row[total_idx] {
            Value::Int(t) => assert!(*t > 100),
            Value::Float(t) => assert!(*t > 100.0),
            Value::String(s) => assert!(s.parse::<f64>().unwrap() > 100.0),
            other => panic!("unexpected total type: {:?}", other),
        }
    }
}

#[tokio::test]
async fn cross_db_full_join_no_limit() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, o.total] from users@pg as u join orders@sqlite as o on u.id = o.user_id",
    )
    .await
    .expect("cross-db full equi-join failed");
    assert_row_count_gte(&result, 5000);
}

#[tokio::test]
async fn cross_db_cross_join_without_limit_rejected() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, p.name] from users@pg as u cross join products@mysql as p",
    )
    .await;
    assert!(result.is_err(), "Expected error for unbounded cross-db cross join");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("LIMIT"), "Error should mention LIMIT: {}", err);
}

#[tokio::test]
async fn cross_db_cross_join_with_limit_allowed() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, p.name] from users@pg as u cross join products@mysql as p limit 10",
    )
    .await
    .expect("cross-db cross join with limit should work");
    assert_row_count(&result, 10);
}

#[tokio::test]
async fn cross_db_cte() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"with recent_orders as ( find [user_id, total] from orders@pg where total > 400 limit 100 ) find [u.name, ro.total] from users@mysql as u join recent_orders as ro on u.id = ro.user_id"#,
    )
    .await
    .expect("cross-db CTE failed");
    assert_row_count_gte(&result, 1);
    assert_no_nulls(&result, "name");
}

#[tokio::test]
async fn cross_db_same_table_different_dbs() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [u.name, m.name] from users@pg as u join users@mysql as m on u.id = m.id limit 10",
    )
    .await
    .expect("same table different dbs failed");
    assert_row_count(&result, 10);
    // Names should match (same seed data)
    for row in &result.rows {
        assert_eq!(row[0], row[1], "Name mismatch: {:?} vs {:?}", row[0], row[1]);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test integrations t07 2>&1 | tail -30`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add tests/integrations/t07_cross_database.rs
git commit -m "test: add E2E tests for doc 07 (cross-database queries)"
```

---

## Task 14: Integration tests — Doc 08 (Data Modification)

**Files:**
- Create: `tests/integrations/t08_data_modification.rs`

- [ ] **Step 1: Write the test file**

```rust
//! E2E tests for doc 08: Data Modification (INSERT, UPDATE, DELETE).
//!
//! These tests use a dedicated "test_scratch" table approach:
//! create a temp table, perform mutations, verify, then clean up.
//! Tests run sequentially to avoid conflicts.

use crate::common::{execute_river, assert_row_count, assert_row_count_gte, TestContext};
use river::adapters::Value;

/// Helper: execute raw SQL on a specific adapter (for setup/teardown)
async fn raw_exec(ctx: &TestContext, db: &str, sql: &str) {
    let adapter = ctx.adapters.get(db).expect("adapter not found");
    adapter.execute(sql).await.expect("raw exec failed");
}

#[tokio::test]
async fn insert_single_row_pg() {
    let ctx = TestContext::new().await;
    // Setup: create scratch table
    raw_exec(&ctx, "pg", "DROP TABLE IF EXISTS test_scratch").await;
    raw_exec(&ctx, "pg", "CREATE TABLE test_scratch (id SERIAL PRIMARY KEY, name TEXT, value INT)").await;

    // Insert via RiverQL
    let result = execute_river(
        &ctx,
        r#"create test_scratch@pg { name: "test1", value: 42 }"#,
    )
    .await;
    assert!(result.is_ok(), "INSERT failed: {:?}", result.err());

    // Verify
    let check = execute_river(&ctx, "find [name, value] from test_scratch@pg")
        .await
        .expect("verify failed");
    assert_row_count(&check, 1);
    assert_eq!(check.rows[0][0], Value::String("test1".into()));

    // Cleanup
    raw_exec(&ctx, "pg", "DROP TABLE test_scratch").await;
}

#[tokio::test]
async fn insert_multiple_rows_pg() {
    let ctx = TestContext::new().await;
    raw_exec(&ctx, "pg", "DROP TABLE IF EXISTS test_scratch").await;
    raw_exec(&ctx, "pg", "CREATE TABLE test_scratch (id SERIAL PRIMARY KEY, name TEXT, value INT)").await;

    let result = execute_river(
        &ctx,
        r#"create test_scratch@pg [{ name: "a", value: 1 }, { name: "b", value: 2 }, { name: "c", value: 3 }]"#,
    )
    .await;
    assert!(result.is_ok(), "multi-INSERT failed: {:?}", result.err());

    let check = execute_river(&ctx, "find [name] from test_scratch@pg")
        .await
        .expect("verify failed");
    assert_row_count(&check, 3);

    raw_exec(&ctx, "pg", "DROP TABLE test_scratch").await;
}

#[tokio::test]
async fn update_rows_pg() {
    let ctx = TestContext::new().await;
    raw_exec(&ctx, "pg", "DROP TABLE IF EXISTS test_scratch").await;
    raw_exec(&ctx, "pg", "CREATE TABLE test_scratch (id SERIAL PRIMARY KEY, name TEXT, status TEXT)").await;
    raw_exec(&ctx, "pg", "INSERT INTO test_scratch (name, status) VALUES ('alice', 'active'), ('bob', 'active'), ('carol', 'inactive')").await;

    let result = execute_river(
        &ctx,
        r#"update test_scratch@pg set status = "updated" where status = "active""#,
    )
    .await;
    assert!(result.is_ok(), "UPDATE failed: {:?}", result.err());

    let check = execute_river(&ctx, r#"find [name, status] from test_scratch@pg where status = "updated""#)
        .await
        .expect("verify failed");
    assert_row_count(&check, 2);

    raw_exec(&ctx, "pg", "DROP TABLE test_scratch").await;
}

#[tokio::test]
async fn delete_rows_pg() {
    let ctx = TestContext::new().await;
    raw_exec(&ctx, "pg", "DROP TABLE IF EXISTS test_scratch").await;
    raw_exec(&ctx, "pg", "CREATE TABLE test_scratch (id SERIAL PRIMARY KEY, name TEXT, status TEXT)").await;
    raw_exec(&ctx, "pg", "INSERT INTO test_scratch (name, status) VALUES ('alice', 'active'), ('bob', 'inactive'), ('carol', 'inactive')").await;

    let result = execute_river(
        &ctx,
        r#"delete test_scratch@pg where status = "inactive""#,
    )
    .await;
    assert!(result.is_ok(), "DELETE failed: {:?}", result.err());

    let check = execute_river(&ctx, "find [name] from test_scratch@pg")
        .await
        .expect("verify failed");
    assert_row_count(&check, 1);
    assert_eq!(check.rows[0][0], Value::String("alice".into()));

    raw_exec(&ctx, "pg", "DROP TABLE test_scratch").await;
}

#[tokio::test]
async fn insert_single_row_mysql() {
    let ctx = TestContext::new().await;
    raw_exec(&ctx, "mysql", "DROP TABLE IF EXISTS test_scratch").await;
    raw_exec(&ctx, "mysql", "CREATE TABLE test_scratch (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(200), value INT)").await;

    let result = execute_river(
        &ctx,
        r#"create test_scratch@mysql { name: "test1", value: 99 }"#,
    )
    .await;
    assert!(result.is_ok(), "MySQL INSERT failed: {:?}", result.err());

    let check = execute_river(&ctx, "find [name, value] from test_scratch@mysql")
        .await
        .expect("verify failed");
    assert_row_count(&check, 1);

    raw_exec(&ctx, "mysql", "DROP TABLE test_scratch").await;
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test integrations t08 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add tests/integrations/t08_data_modification.rs
git commit -m "test: add E2E tests for doc 08 (data modification)"
```

---

## Task 15: Integration tests — Doc 09 (Meta Commands)

**Files:**
- Create: `tests/integrations/t09_meta_commands.rs`

- [ ] **Step 1: Write the test file**

```rust
//! E2E tests for doc 09: Meta Commands (DESCRIBE, SHOW TABLES).

use crate::common::{
    assert_row_count_gte, execute_river, TestContext,
};
use river::adapters::Value;

#[tokio::test]
async fn show_tables_pg() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "show tables @pg")
        .await
        .expect("SHOW TABLES @pg failed");
    assert_row_count_gte(&result, 4); // users, products, orders, order_items
    let table_names: Vec<&str> = result.rows.iter().filter_map(|r| {
        match &r[0] {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }).collect();
    assert!(table_names.contains(&"users"), "users table not found in: {:?}", table_names);
    assert!(table_names.contains(&"orders"), "orders table not found in: {:?}", table_names);
    assert!(table_names.contains(&"products"), "products table not found in: {:?}", table_names);
    assert!(table_names.contains(&"order_items"), "order_items table not found in: {:?}", table_names);
}

#[tokio::test]
async fn show_tables_mysql() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "show tables @mysql")
        .await
        .expect("SHOW TABLES @mysql failed");
    assert_row_count_gte(&result, 4);
}

#[tokio::test]
async fn show_tables_sqlite() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "show tables @sqlite")
        .await
        .expect("SHOW TABLES @sqlite failed");
    assert_row_count_gte(&result, 4);
}

#[tokio::test]
async fn describe_users_pg() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "describe users@pg")
        .await
        .expect("DESCRIBE users@pg failed");
    assert_row_count_gte(&result, 7); // id, name, email, department, salary, status, is_verified, created_at
    let col_names: Vec<&str> = result.rows.iter().filter_map(|r| {
        match &r[0] {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }).collect();
    assert!(col_names.contains(&"name"), "name column not found: {:?}", col_names);
    assert!(col_names.contains(&"email"), "email column not found: {:?}", col_names);
    assert!(col_names.contains(&"salary"), "salary column not found: {:?}", col_names);
}

#[tokio::test]
async fn describe_orders_mysql() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "describe orders@mysql")
        .await
        .expect("DESCRIBE orders@mysql failed");
    assert_row_count_gte(&result, 4); // id, user_id, status, total, created_at
    let col_names: Vec<&str> = result.rows.iter().filter_map(|r| {
        match &r[0] {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }).collect();
    assert!(col_names.contains(&"user_id"), "user_id column not found: {:?}", col_names);
    assert!(col_names.contains(&"total"), "total column not found: {:?}", col_names);
}

#[tokio::test]
async fn describe_products_sqlite() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "describe products@sqlite")
        .await
        .expect("DESCRIBE products@sqlite failed");
    assert_row_count_gte(&result, 6); // id, name, category, price, stock, rating, is_active, created_at
    let col_names: Vec<&str> = result.rows.iter().filter_map(|r| {
        match &r[0] {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }).collect();
    assert!(col_names.contains(&"category"), "category column not found: {:?}", col_names);
    assert!(col_names.contains(&"price"), "price column not found: {:?}", col_names);
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test integrations t09 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add tests/integrations/t09_meta_commands.rs
git commit -m "test: add E2E tests for doc 09 (meta commands)"
```

---

## Task 16: Final integration run and cleanup

**Files:**
- No new files

- [ ] **Step 1: Run the full integration test suite**

Run: `cargo test --test integrations 2>&1 | tail -40`
Expected: All tests pass.

- [ ] **Step 2: Run the entire test suite (lib + all integration tests)**

Run: `cargo test 2>&1 | tail -30`
Expected: All tests pass, no regressions.

- [ ] **Step 3: Final commit if any fixes were needed**

```bash
git add -A
git commit -m "fix: resolve integration test issues from full suite run"
```
