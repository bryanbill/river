use crate::common::{execute_river, TestContext};
use river::adapters::Value;

// ── ALTER TABLE: ADD COLUMN ──────────────────────────────────────────────────

#[tokio::test]
async fn alter_add_column_roundtrip() {
    let ctx = TestContext::new().await;
    let tn = "t13_add_col_pg";

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key, name string)", tn))
        .await
        .unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 1, name: "Alice" }}"#, tn))
        .await
        .unwrap();

    execute_river(&ctx, &format!("alter table {}@pg add column age int", tn))
        .await
        .unwrap();

    let desc = execute_river(&ctx, &format!("describe {}@pg", tn)).await.unwrap();
    let age_row = desc.rows.iter().find(|r| r[0] == Value::String("age".into()));
    assert!(age_row.is_some(), "age column should exist after alter");

    let result = execute_river(&ctx, &format!("find [id, name, age] from {}@pg", tn))
        .await
        .unwrap();
    assert_eq!(result.rows[0][0], Value::Int(1));
    assert_eq!(result.rows[0][1], Value::String("Alice".into()));
    assert_eq!(result.rows[0][2], Value::Null);

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

#[tokio::test]
async fn alter_add_column_not_null_with_default() {
    let ctx = TestContext::new().await;
    let tn = "t13_add_col_nndef_pg";

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key, name string)", tn))
        .await
        .unwrap();

    execute_river(&ctx, &format!("create {}@pg {{ id: 1, name: \"Alice\" }}", tn))
        .await
        .unwrap();

    execute_river(&ctx, &format!("alter table {}@pg add column tier string not null default \"free\"", tn))
        .await
        .unwrap();

    let result = execute_river(&ctx, &format!("find [id, tier] from {}@pg", tn))
        .await
        .unwrap();
    assert_eq!(result.rows[0][1], Value::String("free".into()));

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

#[tokio::test]
async fn alter_drop_column() {
    let ctx = TestContext::new().await;
    let tn = "t13_drop_col_pg";

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key, temp string)", tn))
        .await
        .unwrap();

    execute_river(&ctx, &format!("alter table {}@pg drop column temp", tn))
        .await
        .unwrap();

    let desc = execute_river(&ctx, &format!("describe {}@pg", tn)).await.unwrap();
    let temp_row = desc.rows.iter().find(|r| r[0] == Value::String("temp".into()));
    assert!(temp_row.is_none(), "temp column should be gone after drop");

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

#[tokio::test]
async fn alter_rename_column() {
    let ctx = TestContext::new().await;
    let tn = "t13_rename_col_pg";

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key, old_name string)", tn))
        .await
        .unwrap();

    execute_river(
        &ctx,
        &format!(r#"create {}@pg {{ id: 1, old_name: "Alice" }}"#, tn),
    )
    .await
    .unwrap();

    execute_river(
        &ctx,
        &format!("alter table {}@pg rename column old_name to new_name", tn),
    )
    .await
    .unwrap();

    let result = execute_river(&ctx, &format!("find [new_name] from {}@pg", tn))
        .await
        .unwrap();
    assert_eq!(result.rows[0][0], Value::String("Alice".into()));

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

#[tokio::test]
async fn alter_rename_table() {
    let ctx = TestContext::new().await;
    let tn = "t13_rename_tbl_pg";
    let new_tn = "t13_renamed_pg";

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", new_tn)).await;
    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key)", tn))
        .await
        .unwrap();

    execute_river(&ctx, &format!("alter table {}@pg rename to {}", tn, new_tn))
        .await
        .unwrap();

    let tables = execute_river(&ctx, "show tables@pg").await.unwrap();
    let renamed = tables.rows.iter().any(|r| r[0] == Value::String(new_tn.into()));
    assert!(renamed, "renamed table should appear in table list");

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", new_tn)).await;
}

#[tokio::test]
async fn alter_alter_column_type() {
    let ctx = TestContext::new().await;
    let tn = "t13_alter_col_type_pg";

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key, val string)", tn))
        .await
        .unwrap();

    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 1, val: "42" }}"#, tn))
        .await
        .unwrap();

    execute_river(&ctx, &format!("alter table {}@pg alter column val type float", tn))
        .await
        .unwrap();

    let result = execute_river(&ctx, &format!("find val from {}@pg", tn))
        .await
        .unwrap();
    // Value might be Float or Int depending on adapter
    assert!(result.rows[0][0] != Value::Null);

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

#[tokio::test]
async fn alter_drop_default() {
    let ctx = TestContext::new().await;
    let tn = "t13_drop_default_pg";

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
    execute_river(
        &ctx,
        &format!("create table if not exists {}@pg (id int primary key, status string default \"active\")", tn),
    )
    .await
    .unwrap();

    execute_river(&ctx, &format!("alter table {}@pg alter column status drop default", tn))
        .await
        .unwrap();

    // Just verify it doesn't error
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 1 }}"#, tn))
        .await
        .unwrap();

    let result = execute_river(&ctx, &format!("find status from {}@pg", tn))
        .await
        .unwrap();
    // Without default, status should be NULL
    assert_eq!(result.rows[0][0], Value::Null);

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

#[tokio::test]
async fn alter_idempotent_add() {
    let ctx = TestContext::new().await;
    let tn = "t13_idemp_add_pg";

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key)", tn))
        .await
        .unwrap();

    execute_river(&ctx, &format!("alter table {}@pg add column extra string", tn))
        .await
        .unwrap();

    // Adding the same column again should error
    let result = execute_river(&ctx, &format!("alter table {}@pg add column extra string", tn)).await;
    assert!(result.is_err(), "adding existing column should error");

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

#[tokio::test]
async fn alter_table_mongodb_graceful_error() {
    let ctx = TestContext::new().await;

    let result = execute_river(
        &ctx,
        "alter table users@mongo add column bio string",
    )
    .await;
    assert!(
        result.is_err(),
        "ALTER TABLE on MongoDB should error gracefully"
    );
}
