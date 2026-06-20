//! Integration tests verifying cross-database join operations.
//!
//! These tests exercise the in-memory join engine that fetches data from
//! different database adapters and joins them locally.

mod common;

use common::{
    assert_no_nulls, assert_row_count, assert_row_count_gte, execute_river, TestContext,
};
use river::adapters::Value;

// ── Helper ───────────────────────────────────────────────────────────────────

/// Extract a numeric value from a Value, regardless of whether it was returned
/// as Int, Float, or String containing a number.
fn value_as_f64(val: &Value) -> Option<f64> {
    match val {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

// ── Cross-DB Inner Joins ─────────────────────────────────────────────────────

#[tokio::test]
async fn pg_mysql_inner_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u join orders@mysql as o on u.id = o.user_id limit 100"#,
    )
    .await
    .expect("pg-mysql inner join failed");

    assert_row_count(&result, 100);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");
}

#[tokio::test]
async fn pg_mongo_inner_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u join orders@mongo as o on u.id = o.user_id limit 100"#,
    )
    .await
    .expect("pg-mongo inner join failed");

    assert_row_count(&result, 100);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");
}

#[tokio::test]
async fn mysql_sqlite_inner_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@mysql as u join orders@sqlite as o on u.id = o.user_id limit 50"#,
    )
    .await
    .expect("mysql-sqlite inner join failed");

    assert_row_count(&result, 50);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");
}

// ── Full join without limit ──────────────────────────────────────────────────

#[tokio::test]
async fn pg_mysql_join_no_limit() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u join orders@mysql as o on u.id = o.user_id"#,
    )
    .await
    .expect("pg-mysql join without limit failed");

    // With 10k users and 10k orders, user_id = (i*7+1) % 10000 + 1,
    // most users get at least one order — expect well over 1000 matches.
    assert_row_count_gte(&result, 1000);
}

// ── Left Join ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn pg_mysql_left_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u left join orders@mysql as o on u.id = o.user_id limit 200"#,
    )
    .await
    .expect("pg-mysql left join failed");

    assert_row_count(&result, 200);
    // In a LEFT JOIN, the left side (users) is always present — name should never be null.
    assert_no_nulls(&result, "name");
}

// ── Join with WHERE filter ───────────────────────────────────────────────────

#[tokio::test]
async fn cross_db_join_with_filter() {
    let ctx = TestContext::new().await;
    // Use `total > 100` as filter since "total" is unambiguous (only in orders table).
    // Note: "status" exists in both users and orders, so qualified "o.status" would
    // resolve to the first matching column in the merged result (a known limitation).
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u join orders@mysql as o on u.id = o.user_id where total > 100 limit 50"#,
    )
    .await
    .expect("cross-db join with filter failed");

    // The WHERE filters after the join; many orders have total > 100
    assert!(
        result.rows.len() <= 50,
        "Expected at most 50 rows, got {}",
        result.rows.len()
    );
    assert!(
        !result.rows.is_empty(),
        "Expected at least some rows with total > 100"
    );
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");

    // Verify the filter actually worked: all totals should be > 100
    let col_idx = result
        .columns
        .iter()
        .position(|c| c == "total")
        .expect("'total' column not found");
    for row in &result.rows {
        let total = match &row[col_idx] {
            Value::Float(f) => *f,
            Value::Int(i) => *i as f64,
            other => panic!("Unexpected total value: {:?}", other),
        };
        assert!(
            total > 100.0,
            "Expected total > 100, got {}",
            total
        );
    }
}

// ── Projection (selecting only specific columns) ─────────────────────────────

#[tokio::test]
async fn cross_db_join_projection() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name] from users@pg as u join orders@mysql as o on u.id = o.user_id limit 20"#,
    )
    .await
    .expect("cross-db join projection failed");

    assert_row_count(&result, 20);
    // Only one column should be projected
    assert_eq!(
        result.columns.len(),
        1,
        "Expected 1 projected column, got {:?}",
        result.columns
    );
    assert_no_nulls(&result, "name");
}

// ── Join with ORDER BY ───────────────────────────────────────────────────────

#[tokio::test]
async fn cross_db_join_with_order() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u join orders@mysql as o on u.id = o.user_id order by total desc limit 20"#,
    )
    .await
    .expect("cross-db join with order failed");

    assert_row_count(&result, 20);
    assert_no_nulls(&result, "total");

    // Verify descending order on "total" — values may be Int, Float, or String
    let col_idx = result
        .columns
        .iter()
        .position(|c| c == "total")
        .expect("'total' column not found");

    for window in result.rows.windows(2) {
        let a = value_as_f64(&window[0][col_idx]);
        let b = value_as_f64(&window[1][col_idx]);
        if let (Some(va), Some(vb)) = (a, b) {
            assert!(
                va >= vb,
                "Order violation: {} should be >= {} (desc order)",
                va,
                vb
            );
        }
    }
}

// ── Same table across different databases ────────────────────────────────────

#[tokio::test]
async fn same_table_different_dbs() {
    let ctx = TestContext::new().await;
    // Join users@pg with users@mysql on matching id — since both DBs are seeded
    // identically, the same id should yield the same name.
    // Use projected columns to keep memory manageable (full * on both 10k tables is heavy).
    let result = execute_river(
        &ctx,
        r#"find [u.name, m.name] from users@pg as u join users@mysql as m on u.id = m.id limit 10"#,
    )
    .await
    .expect("same table different dbs join failed");

    assert_row_count(&result, 10);

    // Both columns should be "name" — the projection selects u.name and m.name
    // which both resolve to the "name" field. After projection, we have two columns.
    assert_eq!(
        result.columns.len(),
        2,
        "Expected 2 columns, got {:?}",
        result.columns
    );

    // Verify names match across databases (same seed data)
    for row in &result.rows {
        assert_eq!(
            row[0], row[1],
            "Name mismatch between pg and mysql for same id: {:?} vs {:?}",
            row[0], row[1]
        );
    }
}

// ── Large cross-DB join (stress test) ────────────────────────────────────────

#[tokio::test]
async fn large_cross_db_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u join orders@sqlite as o on u.id = o.user_id"#,
    )
    .await
    .expect("large cross-db join failed (possible crash)");

    // With 10k users and 10k orders where user_id = (i*7+1) % 10000 + 1,
    // each order maps to exactly one user, and with the hash distribution,
    // we should see a large number of matches.
    assert_row_count_gte(&result, 5000);
}
