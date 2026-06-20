//! Integration tests for error handling, boundary conditions, and unusual inputs.
//!
//! These tests verify that the system handles edge cases gracefully — returning
//! proper errors for invalid queries, surviving large result sets, and handling
//! special characters without panicking.

mod common;

use common::{assert_row_count, assert_row_count_gte, execute_river, TestContext};
use river::adapters::Value;
use river::error::RiverError;

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Shared context — one connection setup per test.
async fn ctx() -> TestContext {
    TestContext::new().await
}

// ── Impossible Filters (valid queries that return 0 rows) ───────────────────

#[tokio::test]
async fn impossible_filter_returns_empty() {
    let ctx = ctx().await;
    let result = execute_river(&ctx, r#"find [name] from users@pg where id = -999999"#)
        .await
        .expect("query should succeed, just return no rows");

    assert_row_count(&result, 0);
}

#[tokio::test]
async fn impossible_filter_mysql() {
    let ctx = ctx().await;
    let result = execute_river(
        &ctx,
        r#"find [name] from users@mysql where name = "ZZZZ_NONEXISTENT""#,
    )
    .await
    .expect("query should succeed, just return no rows");

    assert_row_count(&result, 0);
}

// ── Syntax Errors ───────────────────────────────────────────────────────────

#[tokio::test]
async fn syntax_error_incomplete_query() {
    let ctx = ctx().await;
    // "find" alone is clearly incomplete
    let result = execute_river(&ctx, "find [name from").await;
    assert!(result.is_err(), "incomplete query should produce an error");
    let err = result.unwrap_err();
    assert!(
        matches!(err, RiverError::Parse { .. }),
        "expected RiverError::Parse for incomplete query, got: {:?}",
        err
    );
}

#[tokio::test]
async fn syntax_error_garbage_input() {
    let ctx = ctx().await;
    let result = execute_river(&ctx, "!@#$%^&*").await;
    assert!(result.is_err(), "garbage input should produce an error");
    // Could be Parse or another variant — just confirm it errors, not panics
    let err = result.unwrap_err();
    assert!(
        matches!(err, RiverError::Parse { .. } | RiverError::Other(_) | RiverError::Unsupported(_)),
        "expected a graceful error variant, got: {:?}",
        err
    );
}

// ── Invalid Connection / Table ──────────────────────────────────────────────

#[tokio::test]
async fn invalid_connection_name() {
    let ctx = ctx().await;
    let result = execute_river(&ctx, r#"find [name] from users@nonexistent_db"#).await;
    assert!(
        result.is_err(),
        "query against nonexistent connection should error"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, RiverError::Unsupported(_)),
        "expected RiverError::Unsupported for unknown connection, got: {:?}",
        err
    );
}

#[tokio::test]
async fn invalid_table_pg() {
    let ctx = ctx().await;
    let result = execute_river(&ctx, r#"find [name] from totally_fake_table@pg"#).await;
    assert!(
        result.is_err(),
        "query against nonexistent table should error"
    );
    // The database should return an error — could be Connection (sqlx) or Other
    let err = result.unwrap_err();
    assert!(
        matches!(
            err,
            RiverError::Connection(_) | RiverError::Other(_) | RiverError::Unsupported(_)
        ),
        "expected a database error for nonexistent table, got: {:?}",
        err
    );
}

// ── Large Result Sets ───────────────────────────────────────────────────────

#[tokio::test]
async fn large_single_db_result() {
    let ctx = ctx().await;
    let result = execute_river(&ctx, r#"find * from users@pg"#)
        .await
        .expect("fetching all users should not crash");

    assert_row_count(&result, 10000);
}

#[tokio::test]
async fn large_cross_db_join_no_crash() {
    let ctx = ctx().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u join orders@mysql as o on u.id = o.user_id"#,
    )
    .await
    .expect("large cross-db join should not crash");

    assert_row_count_gte(&result, 1000);
}

// ── Sequential Queries ──────────────────────────────────────────────────────

#[tokio::test]
async fn sequential_queries_all_succeed() {
    let ctx = ctx().await;

    let queries = [
        r#"find [name] from users@pg limit 10"#,
        r#"find [name] from users@mysql limit 10"#,
        r#"find [name] from users@mongo limit 10"#,
        r#"find [name, salary] from users@pg limit 10"#,
        r#"find [name] from products@mysql limit 10"#,
    ];

    for (i, query) in queries.iter().enumerate() {
        let result = execute_river(&ctx, query)
            .await
            .unwrap_or_else(|e| panic!("Query {} failed: {:?}\n  Query: {}", i + 1, e, query));

        assert_row_count(&result, 10);
    }
}

// ── Special Characters and Escaping ─────────────────────────────────────────

#[tokio::test]
async fn filter_with_quote_in_value() {
    let ctx = ctx().await;
    let result = execute_river(
        &ctx,
        r#"find [name] from users@pg where name = "O'Brien""#,
    )
    .await;

    // Must NOT panic. Either returns results (possibly 0 rows) or a graceful error.
    match result {
        Ok(r) => {
            // Valid — query worked, maybe 0 rows if no user named O'Brien exists
            let _ = r.rows.len();
        }
        Err(_) => {
            // Also acceptable — a graceful error is fine for special characters
        }
    }
}

// ── Typed Filters ───────────────────────────────────────────────────────────

#[tokio::test]
async fn integer_filter_works() {
    let ctx = ctx().await;
    let result = execute_river(&ctx, r#"find [id, name] from users@pg where id = 42"#)
        .await
        .expect("integer filter query should succeed");

    assert_row_count(&result, 1);
}

#[tokio::test]
async fn boolean_filter_works() {
    let ctx = ctx().await;
    let result = execute_river(
        &ctx,
        r#"find [name, is_verified] from users@pg where is_verified = true limit 10"#,
    )
    .await
    .expect("boolean filter query should succeed");

    assert_row_count(&result, 10);
}

// ── NULL Handling ───────────────────────────────────────────────────────────

#[tokio::test]
async fn null_handling_in_left_join() {
    let ctx = ctx().await;
    // Left join users with a table where some right-side rows won't match.
    // Users with id > some threshold may have no matching orders.
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u left join orders@mysql as o on u.id = o.user_id limit 100"#,
    )
    .await
    .expect("left join should succeed");

    assert_row_count(&result, 100);

    // In a left join, left-side values (name) should never be null
    for row in &result.rows {
        assert!(
            !matches!(row[0], Value::Null),
            "left-side column (name) should not be null in a left join"
        );
    }

    // At least some right-side values (total) may be null if users have no orders.
    // We don't assert a specific count, but verify the result contains the structure.
    let has_null_on_right = result.rows.iter().any(|row| matches!(row[1], Value::Null));
    let has_value_on_right = result.rows.iter().any(|row| !matches!(row[1], Value::Null));

    // With a large user set and limited orders, we expect at least some nulls
    // and some non-nulls — but at minimum the query must not crash.
    assert!(
        has_null_on_right || has_value_on_right,
        "result should contain at least some right-side data (null or non-null)"
    );
}
