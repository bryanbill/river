//! Integration tests for INSERT/UPDATE/DELETE operations.
//!
//! Because River's planner returns `PlanNode::Empty` for DML statements (mutations
//! don't yet execute through the RiverQL pipeline), these tests use the adapter's
//! `execute()` method directly with native SQL to perform mutations, then verify
//! results via `execute_river()` (the RiverQL pipeline).
//!
//! Important: all test IDs start at 99901+ to avoid collisions with the 10,000-row
//! seed data. Each test cleans up after itself.

mod common;

use common::{assert_row_count, execute_river, TestContext};
use river::adapters::Value;

// ── Helper ──────────────────────────────────────────────────────────────────────

async fn ctx() -> TestContext {
    TestContext::new().await
}

/// Extract a string from a Value, handling both String and Int representations.
fn value_as_string(val: &Value) -> Option<String> {
    match val {
        Value::String(s) => Some(s.clone()),
        Value::Int(i) => Some(i.to_string()),
        Value::Float(f) => Some(f.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
    }
}

// ── INSERT Tests ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn insert_and_select_pg() {
    let ctx = ctx().await;
    let adapter = ctx.adapters.get("pg").expect("pg adapter not found");

    // Insert a test user
    adapter
        .execute(
            "INSERT INTO users (id, name, email, department, salary, status, is_verified) \
             VALUES (99901, 'Test User PG', 'test99901@example.com', 'QA', 99999, 'active', TRUE)",
        )
        .await
        .expect("insert into pg failed");

    // Verify via RiverQL
    let result = execute_river(
        &ctx,
        r#"find [name, email, department] from users@pg where id = 99901"#,
    )
    .await
    .expect("select after insert failed");

    assert_row_count(&result, 1);

    // Check name
    match &result.rows[0][0] {
        Value::String(s) => assert_eq!(s, "Test User PG"),
        other => panic!("Expected String for name, got {:?}", other),
    }

    // Check email
    match &result.rows[0][1] {
        Value::String(s) => assert_eq!(s, "test99901@example.com"),
        other => panic!("Expected String for email, got {:?}", other),
    }

    // Check department
    match &result.rows[0][2] {
        Value::String(s) => assert_eq!(s, "QA"),
        other => panic!("Expected String for department, got {:?}", other),
    }

    // Cleanup
    adapter
        .execute("DELETE FROM users WHERE id = 99901")
        .await
        .expect("cleanup of pg insert failed");
}

#[tokio::test]
async fn insert_and_select_mysql() {
    let ctx = ctx().await;
    let adapter = ctx.adapters.get("mysql").expect("mysql adapter not found");

    // Insert a test user
    adapter
        .execute(
            "INSERT INTO users (id, name, email, department, salary, status, is_verified) \
             VALUES (99902, 'Test User MySQL', 'test99902@example.com', 'DevOps', 88888, 'active', 1)",
        )
        .await
        .expect("insert into mysql failed");

    // Verify via RiverQL
    let result = execute_river(
        &ctx,
        r#"find [name, email, department] from users@mysql where id = 99902"#,
    )
    .await
    .expect("select after mysql insert failed");

    assert_row_count(&result, 1);

    // Check name
    match &result.rows[0][0] {
        Value::String(s) => assert_eq!(s, "Test User MySQL"),
        other => panic!("Expected String for name, got {:?}", other),
    }

    // Check email
    match &result.rows[0][1] {
        Value::String(s) => assert_eq!(s, "test99902@example.com"),
        other => panic!("Expected String for email, got {:?}", other),
    }

    // Cleanup
    adapter
        .execute("DELETE FROM users WHERE id = 99902")
        .await
        .expect("cleanup of mysql insert failed");
}

// ── UPDATE Test ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn update_and_verify_pg() {
    let ctx = ctx().await;
    let adapter = ctx.adapters.get("pg").expect("pg adapter not found");

    // First, read the original department for user id=500
    let original = execute_river(
        &ctx,
        r#"find [department] from users@pg where id = 500"#,
    )
    .await
    .expect("failed to read original department");

    assert_row_count(&original, 1);
    let original_dept = value_as_string(&original.rows[0][0])
        .expect("original department is null");

    // Update the department
    adapter
        .execute("UPDATE users SET department = 'TestDept' WHERE id = 500")
        .await
        .expect("update failed");

    // Verify the update via RiverQL
    let updated = execute_river(
        &ctx,
        r#"find [department] from users@pg where id = 500"#,
    )
    .await
    .expect("select after update failed");

    assert_row_count(&updated, 1);
    match &updated.rows[0][0] {
        Value::String(s) => assert_eq!(s, "TestDept"),
        other => panic!("Expected String 'TestDept', got {:?}", other),
    }

    // Restore original department
    adapter
        .execute(&format!(
            "UPDATE users SET department = '{}' WHERE id = 500",
            original_dept.replace('\'', "''")
        ))
        .await
        .expect("restore of original department failed");

    // Confirm restoration
    let restored = execute_river(
        &ctx,
        r#"find [department] from users@pg where id = 500"#,
    )
    .await
    .expect("select after restore failed");

    assert_row_count(&restored, 1);
    match &restored.rows[0][0] {
        Value::String(s) => assert_eq!(s, &original_dept),
        other => panic!(
            "Expected restored department '{}', got {:?}",
            original_dept, other
        ),
    }
}

// ── DELETE Test ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_and_verify_pg() {
    let ctx = ctx().await;
    let adapter = ctx.adapters.get("pg").expect("pg adapter not found");

    // Insert a row to delete
    adapter
        .execute(
            "INSERT INTO users (id, name, email, department, salary, status, is_verified) \
             VALUES (99903, 'DeleteMe', 'delete99903@example.com', 'Temp', 10000, 'active', TRUE)",
        )
        .await
        .expect("insert for delete test failed");

    // Verify it exists
    let exists = execute_river(
        &ctx,
        r#"find [name] from users@pg where id = 99903"#,
    )
    .await
    .expect("select before delete failed");

    assert_row_count(&exists, 1);
    match &exists.rows[0][0] {
        Value::String(s) => assert_eq!(s, "DeleteMe"),
        other => panic!("Expected String 'DeleteMe', got {:?}", other),
    }

    // Delete the row
    adapter
        .execute("DELETE FROM users WHERE id = 99903")
        .await
        .expect("delete failed");

    // Verify it's gone
    let gone = execute_river(
        &ctx,
        r#"find [name] from users@pg where id = 99903"#,
    )
    .await
    .expect("select after delete failed");

    assert_row_count(&gone, 0);
}

// ── Cross-DB Join with Inserted Data ────────────────────────────────────────────

#[tokio::test]
async fn insert_appears_in_cross_db_join() {
    let ctx = ctx().await;
    let pg_adapter = ctx.adapters.get("pg").expect("pg adapter not found");
    let mysql_adapter = ctx.adapters.get("mysql").expect("mysql adapter not found");

    // Insert a user in PG
    pg_adapter
        .execute(
            "INSERT INTO users (id, name, email, department, salary, status, is_verified) \
             VALUES (99904, 'CrossDB User', 'cross99904@example.com', 'Integration', 77777, 'active', TRUE)",
        )
        .await
        .expect("insert user into pg failed");

    // Insert a matching order in MySQL
    mysql_adapter
        .execute(
            "INSERT INTO orders (id, user_id, status, total, created_at) \
             VALUES (99904, 99904, 'paid', 555.55, NOW())",
        )
        .await
        .expect("insert order into mysql failed");

    // Verify the cross-DB join finds the new data
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u join orders@mysql as o on u.id = o.user_id where u.id = 99904"#,
    )
    .await
    .expect("cross-db join after insert failed");

    assert_row_count(&result, 1);

    // Verify user name
    match &result.rows[0][0] {
        Value::String(s) => assert_eq!(s, "CrossDB User"),
        other => panic!("Expected String 'CrossDB User', got {:?}", other),
    }

    // Verify order total (may come back as Float or Int)
    match &result.rows[0][1] {
        Value::Float(f) => assert!(
            (*f - 555.55).abs() < 0.01,
            "Expected total ~555.55, got {}",
            f
        ),
        Value::Int(i) => assert_eq!(*i, 555, "Expected total ~555, got {}", i),
        Value::String(s) => {
            let f: f64 = s.parse().expect("total string not a number");
            assert!(
                (f - 555.55).abs() < 0.01,
                "Expected total ~555.55, got {}",
                f
            );
        }
        other => panic!("Unexpected type for total: {:?}", other),
    }

    // Cleanup: delete from both databases
    mysql_adapter
        .execute("DELETE FROM orders WHERE id = 99904")
        .await
        .expect("cleanup of mysql order failed");

    pg_adapter
        .execute("DELETE FROM users WHERE id = 99904")
        .await
        .expect("cleanup of pg user failed");
}
