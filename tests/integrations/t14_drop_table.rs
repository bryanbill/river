use crate::common::{
    drop_table_if_exists, execute_raw, execute_river, TestContext,
};
use river::adapters::Value;

// ── DROP TABLE: basic roundtrip ──────────────────────────────────────────

#[tokio::test]
async fn drop_table_pg() {
    let ctx = TestContext::new().await;
    let tn = "t14_drop_tbl_pg";

    drop_table_if_exists(&ctx, tn, "pg").await;
    execute_river(
        &ctx,
        &format!("create table {}@pg (id int primary key, name string)", tn),
    )
    .await
    .unwrap();

    let tables = execute_river(&ctx, "show tables @pg").await.unwrap();
    let found = tables.rows.iter().any(|r| r[0] == Value::String(tn.into()));
    assert!(found, "table should exist before drop: {}", tn);

    execute_river(&ctx, &format!("drop table {}@pg", tn))
        .await
        .unwrap();

    let tables = execute_river(&ctx, "show tables @pg").await.unwrap();
    let found = tables.rows.iter().any(|r| r[0] == Value::String(tn.into()));
    assert!(!found, "table should be gone after drop: {}", tn);
}

#[tokio::test]
async fn drop_table_mysql() {
    let ctx = TestContext::new().await;
    let tn = "t14_drop_tbl_mysql";

    execute_river(&ctx, &format!("drop table if exists {}@mysql", tn))
        .await
        .unwrap();
    execute_river(
        &ctx,
        &format!(
            "create table if not exists {}@mysql (id int primary key, name string)",
            tn
        ),
    )
    .await
    .unwrap();

    let tables = execute_river(&ctx, "show tables @mysql").await.unwrap();
    let found = tables.rows.iter().any(|r| r[0] == Value::String(tn.into()));
    assert!(found, "table should exist before drop: {}", tn);

    execute_river(&ctx, &format!("drop table {}@mysql", tn))
        .await
        .unwrap();

    let tables = execute_river(&ctx, "show tables @mysql").await.unwrap();
    let found = tables.rows.iter().any(|r| r[0] == Value::String(tn.into()));
    assert!(!found, "table should be gone after drop: {}", tn);
}

#[tokio::test]
async fn drop_table_sqlite() {
    let ctx = TestContext::new().await;
    let tn = "t14_drop_tbl_sqlite";

    drop_table_if_exists(&ctx, tn, "sqlite").await;
    execute_river(
        &ctx,
        &format!(
            "create table if not exists {}@sqlite (id int primary key, name string)",
            tn
        ),
    )
    .await
    .unwrap();

    let tables = execute_river(&ctx, "show tables @sqlite").await.unwrap();
    let found = tables.rows.iter().any(|r| r[0] == Value::String(tn.into()));
    assert!(found, "table should exist before drop: {}", tn);

    execute_river(&ctx, &format!("drop table {}@sqlite", tn))
        .await
        .unwrap();

    let tables = execute_river(&ctx, "show tables @sqlite").await.unwrap();
    let found = tables.rows.iter().any(|r| r[0] == Value::String(tn.into()));
    assert!(!found, "table should be gone after drop: {}", tn);
}

#[tokio::test]
async fn drop_table_mongo() {
    let ctx = TestContext::new().await;
    let tn = "t14_drop_tbl_mongo";

    execute_river(&ctx, &format!("remove {}@mongo where id > 0", tn))
        .await
        .unwrap();
    execute_river(
        &ctx,
        &format!("create table {}@mongo (id int, name string)", tn),
    )
    .await
    .unwrap();

    let tables = execute_river(&ctx, "show tables @mongo").await.unwrap();
    let found = tables.rows.iter().any(|r| r[0] == Value::String(tn.into()));
    assert!(found, "collection should exist before drop: {}", tn);

    execute_river(&ctx, &format!("drop table {}@mongo", tn))
        .await
        .unwrap();

    let tables = execute_river(&ctx, "show tables @mongo").await.unwrap();
    let found = tables.rows.iter().any(|r| r[0] == Value::String(tn.into()));
    assert!(!found, "collection should be gone after drop: {}", tn);
}

// ── DROP TABLE: IF EXISTS ────────────────────────────────────────────────

#[tokio::test]
async fn drop_table_if_exists_missing_no_error() {
    let ctx = TestContext::new().await;
    let tn = "t14_does_not_exist_anywhere";

    let result = execute_river(&ctx, &format!("drop table if exists {}@pg", tn)).await;
    assert!(result.is_ok(), "IF EXISTS on missing table should not error");
}

#[tokio::test]
async fn drop_table_if_exists_present() {
    let ctx = TestContext::new().await;
    let tn = "t14_if_exists_pg";

    drop_table_if_exists(&ctx, tn, "pg").await;
    execute_river(
        &ctx,
        &format!("create table {}@pg (id int primary key)", tn),
    )
    .await
    .unwrap();

    execute_river(&ctx, &format!("drop table if exists {}@pg", tn))
        .await
        .unwrap();

    let tables = execute_river(&ctx, "show tables @pg").await.unwrap();
    let found = tables.rows.iter().any(|r| r[0] == Value::String(tn.into()));
    assert!(!found, "table should be gone after drop if exists: {}", tn);
}

#[tokio::test]
async fn drop_table_missing_error() {
    let ctx = TestContext::new().await;
    let tn = "t14_missing_no_if_exists_pg";

    drop_table_if_exists(&ctx, tn, "pg").await;

    let result = execute_river(&ctx, &format!("drop table {}@pg", tn)).await;
    assert!(
        result.is_err(),
        "DROP TABLE without IF EXISTS on missing table should error"
    );
}

// ── DROP TABLE: CASCADE / RESTRICT ───────────────────────────────────────

#[tokio::test]
async fn drop_table_if_exists_cascade_pg() {
    let ctx = TestContext::new().await;
    let tn = "t14_if_exists_cascade_pg";

    drop_table_if_exists(&ctx, tn, "pg").await;
    execute_river(
        &ctx,
        &format!("create table {}@pg (id int primary key, name string)", tn),
    )
    .await
    .unwrap();

    execute_river(&ctx, &format!("drop table if exists {}@pg cascade", tn))
        .await
        .unwrap();

    let tables = execute_river(&ctx, "show tables @pg").await.unwrap();
    let found = tables.rows.iter().any(|r| r[0] == Value::String(tn.into()));
    assert!(!found, "table should be gone after cascade drop: {}", tn);
}

#[tokio::test]
async fn drop_table_restrict_pg() {
    let ctx = TestContext::new().await;
    let tn = "t14_restrict_pg";

    drop_table_if_exists(&ctx, tn, "pg").await;
    execute_river(
        &ctx,
        &format!("create table {}@pg (id int primary key)", tn),
    )
    .await
    .unwrap();

    execute_river(&ctx, &format!("drop table {}@pg restrict", tn))
        .await
        .unwrap();

    let tables = execute_river(&ctx, "show tables @pg").await.unwrap();
    let found = tables.rows.iter().any(|r| r[0] == Value::String(tn.into()));
    assert!(!found, "table should be gone after restrict drop: {}", tn);
}

// ── DROP TABLE: schema-qualified ─────────────────────────────────────────

#[tokio::test]
async fn drop_table_with_schema_pg() {
    let ctx = TestContext::new().await;
    let tn = "t14_schema_pg";

    let _ = execute_raw(
        &ctx,
        "pg",
        &format!("DROP TABLE IF EXISTS public.\"{}\"", tn),
    )
    .await;

    execute_river(
        &ctx,
        &format!(
            "create table if not exists public.{}@pg (id int primary key, note string)",
            tn
        ),
    )
    .await
    .unwrap();

    let desc = execute_river(&ctx, &format!("describe public.{}@pg", tn))
        .await
        .unwrap();
    assert!(!desc.rows.is_empty(), "table should be describable before drop");

    execute_river(&ctx, &format!("drop table public.{}@pg", tn))
        .await
        .unwrap();

    let desc = execute_river(&ctx, &format!("describe public.{}@pg", tn))
        .await
        .unwrap();
    assert!(
        desc.rows.is_empty(),
        "describe should return no rows after table is dropped"
    );
}

// ── DROP COLUMN: extended tests ──────────────────────────────────────────
// (basic PG drop column covered by t13_alter_table::alter_drop_column)

#[tokio::test]
async fn alter_drop_column_mysql() {
    let ctx = TestContext::new().await;
    let tn = "t14_drop_col_mysql";

    execute_river(&ctx, &format!("drop table if exists {}@mysql", tn))
        .await
        .unwrap();
    execute_river(
        &ctx,
        &format!(
            "create table if not exists {}@mysql (id int primary key, temp string)",
            tn
        ),
    )
    .await
    .unwrap();

    execute_river(
        &ctx,
        &format!("alter table {}@mysql drop column temp", tn),
    )
    .await
    .unwrap();

    let desc = execute_river(&ctx, &format!("describe {}@mysql", tn))
        .await
        .unwrap();
    let temp_row = desc
        .rows
        .iter()
        .find(|r| r[0] == Value::String("temp".into()));
    assert!(temp_row.is_none(), "temp column should be gone after drop");
}

#[tokio::test]
async fn alter_drop_column_sqlite() {
    let ctx = TestContext::new().await;
    let tn = "t14_drop_col_sqlite";

    drop_table_if_exists(&ctx, tn, "sqlite").await;
    execute_river(
        &ctx,
        &format!(
            "create table if not exists {}@sqlite (id int primary key, temp string)",
            tn
        ),
    )
    .await
    .unwrap();

    execute_river(
        &ctx,
        &format!("alter table {}@sqlite drop column temp", tn),
    )
    .await
    .unwrap();

    let desc = execute_river(&ctx, &format!("describe {}@sqlite", tn))
        .await
        .unwrap();
    let temp_row = desc
        .rows
        .iter()
        .find(|r| r[0] == Value::String("temp".into()));
    assert!(temp_row.is_none(), "temp column should be gone after drop");
}

// ── DROP COLUMN: error cases ─────────────────────────────────────────────

#[tokio::test]
async fn alter_drop_column_missing_table_error() {
    let ctx = TestContext::new().await;
    let tn = "t14_nonexistent_table_pg";

    drop_table_if_exists(&ctx, tn, "pg").await;

    let result = execute_river(
        &ctx,
        &format!("alter table {}@pg drop column anything", tn),
    )
    .await;
    assert!(
        result.is_err(),
        "ALTER DROP COLUMN on missing table should error"
    );
}

#[tokio::test]
async fn alter_drop_column_missing_column_error() {
    let ctx = TestContext::new().await;
    let tn = "t14_missing_col_pg";

    drop_table_if_exists(&ctx, tn, "pg").await;
    execute_river(
        &ctx,
        &format!("create table {}@pg (id int primary key)", tn),
    )
    .await
    .unwrap();

    let result = execute_river(
        &ctx,
        &format!("alter table {}@pg drop column nonexistent", tn),
    )
    .await;
    assert!(
        result.is_err(),
        "ALTER DROP COLUMN for nonexistent column should error"
    );

    drop_table_if_exists(&ctx, tn, "pg").await;
}

// ── DROP COLUMN: MongoDB graceful error ──────────────────────────────────

#[tokio::test]
async fn alter_drop_column_mongodb_error() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "alter table users@mongo drop column bio",
    )
    .await;
    assert!(
        result.is_err(),
        "ALTER DROP COLUMN on MongoDB should error gracefully"
    );
}
