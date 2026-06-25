use crate::common::{assert_columns, assert_row_count, drop_table_if_exists, execute_river, TestContext};
use river::adapters::Value;

// ── CREATE TABLE (explicit DDL) ─────────────────────────────────────────────

#[tokio::test]
async fn create_table_roundtrip_pg() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_ct_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!(
            "create table if not exists {}@pg (id int primary key, name string not null, amount float)",
            table_name
        ),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!(r#"create {}@pg {{ id: 1, name: "test_roundtrip", amount: 99.50 }}"#, table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [id, name, amount] from {}@pg where id = 1", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::Int(1));
    assert_eq!(result.rows[0][1], Value::String("test_roundtrip".into()));

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;
}

#[tokio::test]
async fn create_table_roundtrip_mysql() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_ct_mysql";
    let _ = execute_river(&ctx, &format!("remove {}@mysql where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!(
            "create table if not exists {}@mysql (id int primary key, name string not null, amount float)",
            table_name
        ),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!(r#"create {}@mysql {{ id: 1, name: "test_roundtrip_mysql", amount: 42.0 }}"#, table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [id, name] from {}@mysql where id = 1", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::Int(1));

    let _ = execute_river(&ctx, &format!("remove {}@mysql where id > 0", table_name)).await;
}

#[tokio::test]
async fn create_table_roundtrip_sqlite() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_ct_sqlite";
    let _ = execute_river(&ctx, &format!("remove {}@sqlite where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!(
            "create table if not exists {}@sqlite (id int primary key, name string not null, amount float)",
            table_name
        ),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!(r#"create {}@sqlite {{ id: 1, name: "test_roundtrip_sqlite", amount: 12.34 }}"#, table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [id, name] from {}@sqlite where id = 1", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::Int(1));

    let _ = execute_river(&ctx, &format!("remove {}@sqlite where id > 0", table_name)).await;
}

#[tokio::test]
async fn create_table_roundtrip_mongo() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_ct_mongo";
    let _ = execute_river(&ctx, &format!("remove {}@mongo where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!("create table {}@mongo (id int, name string, amount float)", table_name),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!(r#"create {}@mongo {{ id: 1, name: "test_roundtrip_mongo", amount: 77.7 }}"#, table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [id, name, amount] from {}@mongo where id = 1", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::Int(1));
    assert_eq!(result.rows[0][1], Value::String("test_roundtrip_mongo".into()));

    let _ = execute_river(&ctx, &format!("remove {}@mongo where id > 0", table_name)).await;
}

// ── CREATE TABLE if not exists (idempotent) ─────────────────────────────────

#[tokio::test]
async fn create_table_if_not_exists_pg() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_ifne_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!("create table if not exists {}@pg (id int, name string)", table_name),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!("create table if not exists {}@pg (id int, name string)", table_name),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!(r#"create {}@pg {{ id: 1, name: "ifne_test" }}"#, table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [name] from {}@pg where id = 1", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::String("ifne_test".into()));

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;
}

#[tokio::test]
async fn create_table_if_not_exists_mysql() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_ifne_mysql";
    let _ = execute_river(&ctx, &format!("remove {}@mysql where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!("create table if not exists {}@mysql (id int, name string)", table_name),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!("create table if not exists {}@mysql (id int, name string)", table_name),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!(r#"create {}@mysql {{ id: 1, name: "ifne_mysql_test" }}"#, table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [name] from {}@mysql where id = 1", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::String("ifne_mysql_test".into()));

    let _ = execute_river(&ctx, &format!("remove {}@mysql where id > 0", table_name)).await;
}

// ── Persist Query Results (>>) ──────────────────────────────────────────────

#[tokio::test]
async fn persist_query_basic_pg() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_persist_basic_pg";

    // Drop any leftover table from previous runs
    drop_table_if_exists(&ctx, table_name, "pg").await;

    execute_river(
        &ctx,
        &format!("find * from users@pg limit 10 >> {}@pg", table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [count(*)] from {}@pg", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::Int(10));

    drop_table_if_exists(&ctx, table_name, "pg").await;
}

#[tokio::test]
async fn persist_query_basic_mysql() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_persist_basic_mysql";
    let _ = execute_river(&ctx, &format!("remove {}@mysql where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!("find * from users@mysql limit 5 >> {}@mysql", table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [count(*)] from {}@mysql", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::Int(5));

    let _ = execute_river(&ctx, &format!("remove {}@mysql where id > 0", table_name)).await;
}

#[tokio::test]
async fn persist_query_basic_sqlite() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_persist_basic_sqlite";
    let _ = execute_river(&ctx, &format!("remove {}@sqlite where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!("find * from users@sqlite limit 7 >> {}@sqlite", table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [count(*)] from {}@sqlite", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::Int(7));

    let _ = execute_river(&ctx, &format!("remove {}@sqlite where id > 0", table_name)).await;
}

#[tokio::test]
async fn persist_query_basic_mongo() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_persist_basic_mongo";
    let _ = execute_river(&ctx, &format!("remove {}@mongo where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!("find * from users@pg limit 8 >> {}@mongo", table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [id] from {}@mongo", table_name),
    ).await.unwrap();

    assert_row_count(&result, 8);

    let _ = execute_river(&ctx, &format!("remove {}@mongo where id > 0", table_name)).await;
}

// ── Persist Query with Filter ───────────────────────────────────────────────

#[tokio::test]
async fn persist_query_with_filter_pg() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_persist_filter_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where name = name", table_name)).await;

    execute_river(
        &ctx,
        &format!(
            r#"find [name, email] from users@pg where status = "active" limit 15 >> {}@pg"#,
            table_name
        ),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [name, email] from {}@pg", table_name),
    ).await.unwrap();

    assert_columns(&result, &["name", "email"]);
    for row in &result.rows {
        assert!(matches!(row[0], Value::String(_)));
        assert!(matches!(row[1], Value::String(_)));
    }

    let _ = execute_river(&ctx, &format!("remove {}@pg where name = name", table_name)).await;
}

#[tokio::test]
async fn persist_query_with_filter_mongo() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_persist_filter_mongo";
    let _ = execute_river(&ctx, &format!("remove {}@mongo where name = name", table_name)).await;

    execute_river(
        &ctx,
        &format!(
            r#"find [name, email] from users@pg where status = "active" limit 12 >> {}@mongo"#,
            table_name
        ),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [name, email] from {}@mongo", table_name),
    ).await.unwrap();

    assert_columns(&result, &["name", "email"]);
    assert_row_count(&result, 12);

    let _ = execute_river(&ctx, &format!("remove {}@mongo where name = name", table_name)).await;
}

// ── Persist Query with Conflict Handling (requires pre-existing PK) ─────────
// NOTE: The >> operator auto-creates tables without a PRIMARY KEY constraint.
// ON CONFLICT requires a unique constraint, which the auto-created table lacks.
// This test verifies the basic >> behavior without conflict handling.

#[tokio::test]
async fn persist_query_multiple_inserts_pg() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_multi_insert_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;

    // First >> creates the table and inserts one row
    execute_river(
        &ctx,
        &format!("find [id, name, email] from users@pg where id = 1 >> {}@pg", table_name),
    ).await.unwrap();

    // Second >> appends a row to the existing table
    execute_river(
        &ctx,
        &format!("find [id, name, email] from users@pg where id = 1 >> {}@pg", table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [count(*)] from {}@pg", table_name),
    ).await.unwrap();
    // Without a PK, each >> appends (not dedup)
    assert_eq!(result.rows[0][0], Value::Int(2));

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;
}

// ── Cross-DB Persist ───────────────────────────────────────────────────────

#[tokio::test]
async fn persist_cross_db_pg_to_mongo() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_cross_pg2mongo";
    let _ = execute_river(&ctx, &format!("remove {}@mongo where name = name", table_name)).await;

    execute_river(
        &ctx,
        &format!(
            "find [name, email, department] from users@pg limit 20 >> {}@mongo",
            table_name
        ),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [name] from {}@mongo", table_name),
    ).await.unwrap();

    assert_row_count(&result, 20);

    let _ = execute_river(&ctx, &format!("remove {}@mongo where name = name", table_name)).await;
}

#[tokio::test]
async fn persist_cross_db_mongo_to_pg() {
    let ctx = TestContext::new().await;
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let table_name = format!("t11cm2p_{}", id);
    let mongo_tmp = format!("t11ctmp_{}", id);

    execute_river(
        &ctx,
        &format!("find [id, name, email] from users@pg limit 5 >> {}@mongo", mongo_tmp),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!("find [id, name, email] from {}@mongo >> {}@pg", mongo_tmp, table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [count(*)] from {}@pg", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::Int(5));

    let _ = execute_river(&ctx, &format!("remove {}@mongo where id > 0", mongo_tmp)).await;
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;
}

// ── Table with constraints and defaults ─────────────────────────────────────

#[tokio::test]
async fn create_table_not_null_pg() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_notnull_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!(
            "create table if not exists {}@pg (id int primary key, required_name string not null, optional_desc string)",
            table_name
        ),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!(r#"create {}@pg {{ id: 1, required_name: "present", optional_desc: "extra" }}"#, table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [required_name, optional_desc] from {}@pg where id = 1", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::String("present".into()));
    assert_eq!(result.rows[0][1], Value::String("extra".into()));

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;
}

#[tokio::test]
async fn create_table_with_defaults_pg() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_defaults_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!(
            r#"create table if not exists {}@pg (id int primary key, name string not null, status string default "active")"#,
            table_name
        ),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!(r#"create {}@pg {{ id: 1, name: "default_test" }}"#, table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [name, status] from {}@pg where id = 1", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::String("default_test".into()));
    assert_eq!(result.rows[0][1], Value::String("active".into()));

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;
}

#[tokio::test]
async fn create_table_with_primary_key_pg() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_pk_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!("create table if not exists {}@pg (id int primary key, value string)", table_name),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!(r#"create {}@pg {{ id: 1, value: "first" }}"#, table_name),
    ).await.unwrap();
    execute_river(
        &ctx,
        &format!(r#"create {}@pg {{ id: 2, value: "second" }}"#, table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [id, value] from {}@pg order by id asc", table_name),
    ).await.unwrap();

    assert_row_count(&result, 2);
    assert_eq!(result.rows[0][0], Value::Int(1));
    assert_eq!(result.rows[1][0], Value::Int(2));

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_name)).await;
}

// ── Persist with projections and aggregations ───────────────────────────────

#[tokio::test]
async fn persist_query_with_projection_pg() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_proj_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where full_name = full_name", table_name)).await;

    execute_river(
        &ctx,
        &format!(
            r#"find [name as full_name, email as contact] from users@pg limit 5 >> {}@pg"#,
            table_name
        ),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [full_name, contact] from {}@pg", table_name),
    ).await.unwrap();

    assert_columns(&result, &["full_name", "contact"]);
    assert_row_count(&result, 5);

    let _ = execute_river(&ctx, &format!("remove {}@pg where full_name = full_name", table_name)).await;
}

#[tokio::test]
async fn persist_aggregated_query_pg() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_agg_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where status = status", table_name)).await;

    execute_river(
        &ctx,
        &format!(
            "find [status, count(*) as cnt] from users@pg group by status >> {}@pg",
            table_name
        ),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [status, cnt] from {}@pg order by status asc", table_name),
    ).await.unwrap();

    assert_row_count(&result, 4);
    assert_columns(&result, &["status", "cnt"]);

    let _ = execute_river(&ctx, &format!("remove {}@pg where status = status", table_name)).await;
}

// ── Create table with schema ────────────────────────────────────────────────

#[tokio::test]
async fn create_table_with_schema_pg() {
    let ctx = TestContext::new().await;
    let table_name = "t11_test_schema_pg";
    let _ = execute_river(&ctx, &format!("remove public.{}@pg where id > 0", table_name)).await;

    execute_river(
        &ctx,
        &format!(
            "create table if not exists public.{}@pg (id int primary key, note string)",
            table_name
        ),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!(r#"create public.{}@pg {{ id: 1, note: "schema test" }}"#, table_name),
    ).await.unwrap();

    let result = execute_river(
        &ctx,
        &format!("find [id, note] from public.{}@pg where id = 1", table_name),
    ).await.unwrap();

    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][1], Value::String("schema test".into()));

    let _ = execute_river(&ctx, &format!("remove public.{}@pg where id > 0", table_name)).await;
}

// ── Multiple tables per statement ───────────────────────────────────────────
// NOTE: parse() returns only the first statement. Use separate calls for multi-statement.

#[tokio::test]
async fn create_multiple_tables_pg() {
    let ctx = TestContext::new().await;
    let table_a = "t11_test_multi_a_pg";
    let table_b = "t11_test_multi_b_pg";

    // Pre-cleanup
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_a)).await;
    let _ = execute_river(&ctx, &format!("remove {}@pg where ref_id > 0", table_b)).await;

    execute_river(
        &ctx,
        &format!(
            "create table if not exists {}@pg (id int primary key, name string)",
            table_a
        ),
    ).await.unwrap();
    execute_river(
        &ctx,
        &format!(
            "create table if not exists {}@pg (id int primary key, ref_id int, value float)",
            table_b
        ),
    ).await.unwrap();

    execute_river(
        &ctx,
        &format!(r#"create {}@pg {{ id: 1, name: "multi" }}"#, table_a),
    ).await.unwrap();
    execute_river(
        &ctx,
        &format!(r#"create {}@pg {{ id: 1, ref_id: 1, value: 3.14 }}"#, table_b),
    ).await.unwrap();

    let result_a = execute_river(
        &ctx,
        &format!("find [count(*)] from {}@pg", table_a),
    ).await.unwrap();
    assert_eq!(result_a.rows[0][0], Value::Int(1));

    let result_b = execute_river(
        &ctx,
        &format!("find [count(*)] from {}@pg", table_b),
    ).await.unwrap();
    assert_eq!(result_b.rows[0][0], Value::Int(1));

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", table_a)).await;
    let _ = execute_river(&ctx, &format!("remove {}@pg where ref_id > 0", table_b)).await;
}
