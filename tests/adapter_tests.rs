//! Integration tests verifying each database adapter (PostgreSQL, MySQL, MongoDB, SQLite)
//! can connect and execute RiverQL queries correctly.

mod common;

use common::{
    assert_all_match, assert_columns, assert_no_nulls, assert_row_count, assert_row_count_gte,
    execute_river, TestContext,
};
use river::adapters::Value;

// ── Helper ───────────────────────────────────────────────────────────────────

/// Check whether a Value equals the given string, regardless of whether the
/// adapter returned it as Value::String or Value::Int/Float/Bool.
fn value_matches_str(val: &Value, expected: &str) -> bool {
    match val {
        Value::String(s) => s == expected,
        Value::Int(i) => i.to_string() == expected,
        Value::Float(f) => f.to_string() == expected,
        Value::Bool(b) => b.to_string() == expected,
        Value::Null => false,
    }
}

// ── PostgreSQL ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn pg_select_by_id() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [name, email] from users@pg where id = 1"#)
        .await
        .expect("query failed");

    assert_columns(&result, &["name", "email"]);
    assert_row_count(&result, 1);

    let row = &result.rows[0];
    assert!(
        value_matches_str(&row[0], "Kate Martin"),
        "Expected name 'Kate Martin', got {:?}",
        row[0]
    );
    assert!(
        value_matches_str(&row[1], "kate.martin1@example.com"),
        "Expected email 'kate.martin1@example.com', got {:?}",
        row[1]
    );
}

#[tokio::test]
async fn pg_select_with_filter() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, department] from users@pg where department = "Engineering""#,
    )
    .await
    .expect("query failed");

    assert_columns(&result, &["name", "department"]);
    assert_row_count_gte(&result, 1);
    assert_all_match(&result, "department", |v| {
        value_matches_str(v, "Engineering")
    });
}

#[tokio::test]
async fn pg_select_with_limit() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [name] from users@pg limit 5"#)
        .await
        .expect("query failed");

    assert_columns(&result, &["name"]);
    assert_row_count(&result, 5);
    assert_no_nulls(&result, "name");
}

#[tokio::test]
async fn pg_count_all_users() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find * from users@pg"#)
        .await
        .expect("query failed");

    assert_row_count(&result, 10000);
}

// ── MySQL ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn mysql_select_by_id() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [name, email] from users@mysql where id = 1"#)
        .await
        .expect("query failed");

    assert_columns(&result, &["name", "email"]);
    assert_row_count(&result, 1);

    let row = &result.rows[0];
    assert!(
        value_matches_str(&row[0], "Kate Martin"),
        "Expected name 'Kate Martin', got {:?}",
        row[0]
    );
    assert!(
        value_matches_str(&row[1], "kate.martin1@example.com"),
        "Expected email 'kate.martin1@example.com', got {:?}",
        row[1]
    );
}

#[tokio::test]
async fn mysql_select_with_filter() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, status] from users@mysql where status = "active""#,
    )
    .await
    .expect("query failed");

    assert_columns(&result, &["name", "status"]);
    assert_row_count_gte(&result, 1);
    assert_all_match(&result, "status", |v| value_matches_str(v, "active"));
}

#[tokio::test]
async fn mysql_select_with_limit() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [name] from users@mysql limit 5"#)
        .await
        .expect("query failed");

    assert_columns(&result, &["name"]);
    assert_row_count(&result, 5);
    assert_no_nulls(&result, "name");
}

// ── MongoDB ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn mongo_select_by_id() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [name, email] from users@mongo where _id = 1"#)
        .await
        .expect("query failed");

    assert_columns(&result, &["name", "email"]);
    assert_row_count(&result, 1);

    let row = &result.rows[0];
    assert!(
        value_matches_str(&row[0], "Kate Martin"),
        "Expected name 'Kate Martin', got {:?}",
        row[0]
    );
    assert!(
        value_matches_str(&row[1], "kate.martin1@example.com"),
        "Expected email 'kate.martin1@example.com', got {:?}",
        row[1]
    );
}

#[tokio::test]
async fn mongo_select_with_filter() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, department] from users@mongo where department = "Engineering""#,
    )
    .await
    .expect("query failed");

    assert_columns(&result, &["name", "department"]);
    assert_row_count_gte(&result, 1);
    assert_all_match(&result, "department", |v| {
        value_matches_str(v, "Engineering")
    });
}

#[tokio::test]
async fn mongo_select_with_limit() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [name] from users@mongo limit 5"#)
        .await
        .expect("query failed");

    assert_columns(&result, &["name"]);
    assert_row_count(&result, 5);
    assert_no_nulls(&result, "name");
}

// ── SQLite ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn sqlite_select_by_id() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [name, email] from users@sqlite where id = 1"#)
        .await
        .expect("query failed");

    assert_columns(&result, &["name", "email"]);
    assert_row_count(&result, 1);

    let row = &result.rows[0];
    assert!(
        value_matches_str(&row[0], "Kate Martin"),
        "Expected name 'Kate Martin', got {:?}",
        row[0]
    );
    assert!(
        value_matches_str(&row[1], "kate.martin1@example.com"),
        "Expected email 'kate.martin1@example.com', got {:?}",
        row[1]
    );
}

#[tokio::test]
async fn sqlite_count_all_users() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find * from users@sqlite"#)
        .await
        .expect("query failed");

    assert_row_count(&result, 10000);
}

// ── Cross-adapter consistency ────────────────────────────────────────────────

#[tokio::test]
async fn same_data_across_sql_adapters() {
    let ctx = TestContext::new().await;

    let pg_result = execute_river(&ctx, r#"find [name] from users@pg where id = 100"#)
        .await
        .expect("pg query failed");
    let mysql_result = execute_river(&ctx, r#"find [name] from users@mysql where id = 100"#)
        .await
        .expect("mysql query failed");
    let sqlite_result = execute_river(&ctx, r#"find [name] from users@sqlite where id = 100"#)
        .await
        .expect("sqlite query failed");

    assert_row_count(&pg_result, 1);
    assert_row_count(&mysql_result, 1);
    assert_row_count(&sqlite_result, 1);

    let pg_name = &pg_result.rows[0][0];
    let mysql_name = &mysql_result.rows[0][0];
    let sqlite_name = &sqlite_result.rows[0][0];

    // Extract name as string regardless of Value variant
    let extract = |v: &Value| -> String {
        match v {
            Value::String(s) => s.clone(),
            other => panic!("Expected string name, got {:?}", other),
        }
    };

    let pg_str = extract(pg_name);
    let mysql_str = extract(mysql_name);
    let sqlite_str = extract(sqlite_name);

    assert_eq!(
        pg_str, mysql_str,
        "PG and MySQL returned different names for id=100: '{}' vs '{}'",
        pg_str, mysql_str
    );
    assert_eq!(
        pg_str, sqlite_str,
        "PG and SQLite returned different names for id=100: '{}' vs '{}'",
        pg_str, sqlite_str
    );
}
