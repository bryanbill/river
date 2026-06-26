use crate::common::{execute_river, TestContext};
use river::adapters::Value;

// ── CREATE DATABASE: parsing and planning ──────────────────────────────

#[tokio::test]
async fn create_database_missing_connection_error() {
    let ctx = TestContext::new().await;

    let result = execute_river(&ctx, "create database mydb@nonexistent").await;
    assert!(result.is_err(), "should error on nonexistent connection");
}

#[tokio::test]
async fn create_database_if_not_exists_idempotent_pg() {
    let ctx = TestContext::new().await;
    let dbn = "t15_cd_test_pg";

    // Drop first to clean up any leftovers
    let _ = execute_river(&ctx, &format!("drop database if exists {}@pg", dbn)).await;

    // First creation should succeed
    let r1 = execute_river(&ctx, &format!("create database if not exists {}@pg", dbn)).await;
    assert!(r1.is_ok(), "first create should succeed: {:?}", r1.err());

    // Second creation should be idempotent
    let r2 = execute_river(&ctx, &format!("create database if not exists {}@pg", dbn)).await;
    assert!(r2.is_ok(), "second create should be idempotent: {:?}", r2.err());

    // Clean up
    let _ = execute_river(&ctx, &format!("drop database if exists {}@pg", dbn)).await;
}

#[tokio::test]
async fn create_database_name_clash_pg() {
    let ctx = TestContext::new().await;
    let dbn = "t15_cd_clash_pg";

    // Drop first
    let _ = execute_river(&ctx, &format!("drop database if exists {}@pg", dbn)).await;

    // Create database
    let r1 = execute_river(&ctx, &format!("create database {}@pg", dbn)).await;
    assert!(r1.is_ok(), "first create should succeed: {:?}", r1.err());

    // Without IF NOT EXISTS, second create should error
    let r2 = execute_river(&ctx, &format!("create database {}@pg", dbn)).await;
    assert!(r2.is_err(), "create without if not exists should error on existing db");

    // Clean up
    let _ = execute_river(&ctx, &format!("drop database if exists {}@pg", dbn)).await;
}

// ── DROP DATABASE ──────────────────────────────────────────────────────

#[tokio::test]
async fn drop_database_basic_pg() {
    let ctx = TestContext::new().await;
    let dbn = "t15_drop_tst_pg";

    // Clean up first
    let _ = execute_river(&ctx, &format!("drop database if exists {}@pg", dbn)).await;

    // Create then drop
    execute_river(&ctx, &format!("create database {}@pg", dbn))
        .await
        .unwrap();

    let r = execute_river(&ctx, &format!("drop database {}@pg", dbn)).await;
    assert!(r.is_ok(), "drop should succeed: {:?}", r.err());
}

#[tokio::test]
async fn drop_database_if_exists_missing() {
    let ctx = TestContext::new().await;

    // Dropping a non-existent database with IF EXISTS should succeed silently
    let r = execute_river(&ctx, "drop database if exists definitely_does_not_exist_xyz123@pg").await;
    assert!(r.is_ok(), "drop if exists on missing db should not error: {:?}", r.err());
}

#[tokio::test]
async fn drop_database_missing_error() {
    let ctx = TestContext::new().await;

    // Without IF EXISTS, dropping nonexistent db should error
    let r = execute_river(&ctx, "drop database definitely_does_not_exist_xyz456@pg").await;
    assert!(
        r.is_err(),
        "drop without if exists on missing db should error"
    );
}

// ── MySQL ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_drop_database_mysql() {
    let ctx = TestContext::new().await;
    let dbn = "t15_cd_mysql_tst";

    // Clean up
    let _ = execute_river(&ctx, &format!("drop database if exists {}@mysql", dbn)).await;

    // Create
    let r1 = execute_river(&ctx, &format!("create database if not exists {}@mysql", dbn)).await;
    assert!(r1.is_ok(), "create should succeed: {:?}", r1.err());

    // Drop
    let r2 = execute_river(&ctx, &format!("drop database {}@mysql", dbn)).await;
    assert!(r2.is_ok(), "drop should succeed: {:?}", r2.err());
}

// ── SQLite error path ──────────────────────────────────────────────────

#[tokio::test]
async fn create_database_sqlite_error() {
    let ctx = TestContext::new().await;

    let r = execute_river(&ctx, "create database mydb@sqlite").await;
    assert!(r.is_err(), "create database on SQLite should error");
}

#[tokio::test]
async fn drop_database_sqlite_error() {
    let ctx = TestContext::new().await;

    let r = execute_river(&ctx, "drop database mydb@sqlite").await;
    assert!(r.is_err(), "drop database on SQLite should error");
}

// ── MongoDB behavior ───────────────────────────────────────────────────

#[tokio::test]
async fn create_database_mongodb_noop() {
    let ctx = TestContext::new().await;

    let r = execute_river(&ctx, "create database if not exists new_mongo_db@mongo").await;
    assert!(r.is_ok(), "create database on MongoDB should succeed (no-op): {:?}", r.err());

    // Should return a helpful message
    let msg = r.unwrap();
    let has_message = msg.rows.iter().any(|row| {
        row.first()
            .map(|v| matches!(v, Value::String(s) if s.contains("MongoDB")))
            .unwrap_or(false)
    });
    assert!(has_message, "should return informative MongoDB message");
}

#[tokio::test]
async fn drop_database_mongodb() {
    let ctx = TestContext::new().await;

    // Dropping a nonexistent MongoDB database — MongoDB treats this as no-op
    let r = execute_river(&ctx, "drop database if exists definitely_not_there_mongo@mongo").await;
    // MongoDB dropDatabase on a nonexistent db typically succeeds
    assert!(r.is_ok(), "drop database if exists on missing MongoDB db should succeed: {:?}", r.err());
}
