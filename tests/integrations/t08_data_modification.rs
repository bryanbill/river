use crate::common::{assert_row_count, execute_river, TestContext};
use river::adapters::Value;

// ── INSERT ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn insert_and_query_pg() {
    let ctx = TestContext::new().await;

    // Insert a test row with a unique identifier
    let _insert_result = execute_river(
        &ctx,
        r#"insert into users@pg (name, email, department, salary, status, is_verified) values ("Test User E2E", "test_e2e_insert@example.com", "Engineering", 99999, "active", true)"#,
    )
    .await
    .unwrap();

    // Query to verify the insert
    let query_result = execute_river(
        &ctx,
        r#"find [name, email, salary] from users@pg where email = "test_e2e_insert@example.com""#,
    )
    .await
    .unwrap();
    assert_row_count(&query_result, 1);
    assert_eq!(query_result.rows[0][0], Value::String("Test User E2E".into()));

    // Cleanup
    execute_river(
        &ctx,
        r#"delete from users@pg where email = "test_e2e_insert@example.com""#,
    )
    .await
    .unwrap();

    // Verify cleanup
    let verify = execute_river(
        &ctx,
        r#"find [name] from users@pg where email = "test_e2e_insert@example.com""#,
    )
    .await
    .unwrap();
    assert_row_count(&verify, 0);
}

#[tokio::test]
async fn insert_and_query_mysql() {
    let ctx = TestContext::new().await;

    execute_river(
        &ctx,
        r#"insert into users@mysql (name, email, department, salary, status, is_verified) values ("Test User MySQL E2E", "test_e2e_mysql@example.com", "Sales", 88888, "pending", false)"#,
    )
    .await
    .unwrap();

    let query_result = execute_river(
        &ctx,
        r#"find [name, salary] from users@mysql where email = "test_e2e_mysql@example.com""#,
    )
    .await
    .unwrap();
    assert_row_count(&query_result, 1);
    assert_eq!(query_result.rows[0][0], Value::String("Test User MySQL E2E".into()));

    // Cleanup
    execute_river(
        &ctx,
        r#"delete from users@mysql where email = "test_e2e_mysql@example.com""#,
    )
    .await
    .unwrap();
}

// ── UPDATE ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn update_row_pg() {
    let ctx = TestContext::new().await;

    // Insert a row to update
    execute_river(
        &ctx,
        r#"insert into users@pg (name, email, department, salary, status, is_verified) values ("Update Test", "test_e2e_update@example.com", "HR", 50000, "active", false)"#,
    )
    .await
    .unwrap();

    // Update it
    execute_river(
        &ctx,
        r#"update users@pg set salary = 75000 where email = "test_e2e_update@example.com""#,
    )
    .await
    .unwrap();

    // Verify update
    let result = execute_river(
        &ctx,
        r#"find [salary] from users@pg where email = "test_e2e_update@example.com""#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::Int(75000));

    // Cleanup
    execute_river(
        &ctx,
        r#"delete from users@pg where email = "test_e2e_update@example.com""#,
    )
    .await
    .unwrap();
}

// ── DELETE ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_row_pg() {
    let ctx = TestContext::new().await;

    // Insert a row to delete
    execute_river(
        &ctx,
        r#"insert into users@pg (name, email, department, salary, status, is_verified) values ("Delete Test", "test_e2e_delete@example.com", "Legal", 60000, "inactive", true)"#,
    )
    .await
    .unwrap();

    // Verify it exists
    let before = execute_river(
        &ctx,
        r#"find [name] from users@pg where email = "test_e2e_delete@example.com""#,
    )
    .await
    .unwrap();
    assert_row_count(&before, 1);

    // Delete it
    execute_river(
        &ctx,
        r#"delete from users@pg where email = "test_e2e_delete@example.com""#,
    )
    .await
    .unwrap();

    // Verify deletion
    let after = execute_river(
        &ctx,
        r#"find [name] from users@pg where email = "test_e2e_delete@example.com""#,
    )
    .await
    .unwrap();
    assert_row_count(&after, 0);
}

// ── Delete does not affect other rows ───────────────────────────────────────

#[tokio::test]
async fn delete_only_matching_rows() {
    let ctx = TestContext::new().await;

    // Get count before
    let before = execute_river(&ctx, r#"find [count(*) as cnt] from users@pg"#)
        .await
        .unwrap();

    // Insert and delete a row
    execute_river(
        &ctx,
        r#"insert into users@pg (name, email, department, salary, status, is_verified) values ("Temp Row", "test_e2e_temp@example.com", "Design", 45000, "pending", false)"#,
    )
    .await
    .unwrap();

    execute_river(
        &ctx,
        r#"delete from users@pg where email = "test_e2e_temp@example.com""#,
    )
    .await
    .unwrap();

    // Count should be same as before
    let after = execute_river(&ctx, r#"find [count(*) as cnt] from users@pg"#)
        .await
        .unwrap();
    assert_eq!(before.rows[0][0], after.rows[0][0]);
}
