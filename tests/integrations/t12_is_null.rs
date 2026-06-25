use crate::common::{assert_no_nulls, assert_row_count, execute_river, TestContext};
use river::adapters::Value;

// ── IS NULL ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn is_null_pg() {
    let ctx = TestContext::new().await;
    let tn = "t12_is_null_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;

    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key, val string)", tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 1, val: "hello" }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 2 }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 3, val: "world" }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 4 }}"#, tn)).await.unwrap();

    let nulls = execute_river(&ctx, &format!("find [id] from {}@pg where val is null order by id asc", tn)).await.unwrap();
    assert_row_count(&nulls, 2);
    assert_eq!(nulls.rows[0][0], Value::Int(2));
    assert_eq!(nulls.rows[1][0], Value::Int(4));

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

// ── IS NOT NULL ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn is_not_null_pg() {
    let ctx = TestContext::new().await;
    let tn = "t12_is_not_null_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;

    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key, val string)", tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 1, val: "hello" }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 2 }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 3, val: "world" }}"#, tn)).await.unwrap();

    let not_nulls = execute_river(&ctx, &format!("find [id, val] from {}@pg where val is not null order by id asc", tn)).await.unwrap();
    assert_row_count(&not_nulls, 2);
    assert_no_nulls(&not_nulls, "val");

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

// ── IS NULL with compound condition ─────────────────────────────────────────

#[tokio::test]
async fn is_null_with_and_pg() {
    let ctx = TestContext::new().await;
    let tn = "t12_is_null_and_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;

    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key, name string, note string)", tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 1, name: "a", note: "x" }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 2, name: "a" }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 3, name: "b", note: "x" }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 4, name: "b" }}"#, tn)).await.unwrap();

    let result = execute_river(&ctx, &format!(r#"find [id] from {}@pg where name = "a" and note is null"#, tn)).await.unwrap();
    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::Int(2));

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

// ── IS NOT NULL with compound condition ─────────────────────────────────────

#[tokio::test]
async fn is_not_null_with_or_pg() {
    let ctx = TestContext::new().await;
    let tn = "t12_is_not_null_or_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;

    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key, status string)", tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 1, status: "active" }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 2 }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 3, status: "disabled" }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 4 }}"#, tn)).await.unwrap();

    let result = execute_river(&ctx, &format!(r#"find [id] from {}@pg where status is not null or id = 4"#, tn)).await.unwrap();
    assert_row_count(&result, 3);

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

// ── IS NULL: empty result (all rows have values) ────────────────────────────

#[tokio::test]
async fn is_null_empty_result_pg() {
    let ctx = TestContext::new().await;
    let tn = "t12_is_null_empty_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;

    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key, val string)", tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 1, val: "hello" }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 2, val: "world" }}"#, tn)).await.unwrap();

    let result = execute_river(&ctx, &format!("find [id] from {}@pg where val is null", tn)).await.unwrap();
    assert_row_count(&result, 0);

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

// ── IS NULL: all rows null ──────────────────────────────────────────────────

#[tokio::test]
async fn is_null_all_rows_pg() {
    let ctx = TestContext::new().await;
    let tn = "t12_is_null_all_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;

    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key, val string)", tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 1 }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 2 }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 3 }}"#, tn)).await.unwrap();

    let nulls = execute_river(&ctx, &format!("find [id] from {}@pg where val is null", tn)).await.unwrap();
    assert_row_count(&nulls, 3);

    let not_nulls = execute_river(&ctx, &format!("find [id] from {}@pg where val is not null", tn)).await.unwrap();
    assert_row_count(&not_nulls, 0);

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

// ── IS NULL with integer column ─────────────────────────────────────────────

#[tokio::test]
async fn is_null_integer_pg() {
    let ctx = TestContext::new().await;
    let tn = "t12_is_null_int_pg";
    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;

    execute_river(&ctx, &format!("create table if not exists {}@pg (id int primary key, score int)", tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 1, score: 100 }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 2, score: 200 }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@pg {{ id: 3 }}"#, tn)).await.unwrap();

    let nulls = execute_river(&ctx, &format!("find [id] from {}@pg where score is null", tn)).await.unwrap();
    assert_row_count(&nulls, 1);
    assert_eq!(nulls.rows[0][0], Value::Int(3));

    let _ = execute_river(&ctx, &format!("remove {}@pg where id > 0", tn)).await;
}

// ── MySQL ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn is_null_mysql() {
    let ctx = TestContext::new().await;
    let tn = "t12_is_null_mysql";

    let _ = execute_river(&ctx, &format!("remove {}@mysql where id > 0", tn)).await;
    execute_river(&ctx, &format!("create table if not exists {}@mysql (id int primary key, val string)", tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@mysql {{ id: 1, val: "a" }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@mysql {{ id: 2 }}"#, tn)).await.unwrap();

    let nulls = execute_river(&ctx, &format!("find [id] from {}@mysql where val is null", tn)).await.unwrap();
    assert_row_count(&nulls, 1);
    assert_eq!(nulls.rows[0][0], Value::Int(2));

    let not_nulls = execute_river(&ctx, &format!("find [id] from {}@mysql where val is not null", tn)).await.unwrap();
    assert_row_count(&not_nulls, 1);
    assert_eq!(not_nulls.rows[0][0], Value::Int(1));

    let _ = execute_river(&ctx, &format!("remove {}@mysql where id > 0", tn)).await;
}

// ── SQLite ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn is_null_sqlite() {
    let ctx = TestContext::new().await;
    let tn = "t12_is_null_sqlite";

    let _ = execute_river(&ctx, &format!("remove {}@sqlite where id > 0", tn)).await;
    execute_river(&ctx, &format!("create table if not exists {}@sqlite (id int primary key, val string)", tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@sqlite {{ id: 1, val: "a" }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@sqlite {{ id: 2 }}"#, tn)).await.unwrap();

    let nulls = execute_river(&ctx, &format!("find [id] from {}@sqlite where val is null", tn)).await.unwrap();
    assert_row_count(&nulls, 1);
    assert_eq!(nulls.rows[0][0], Value::Int(2));

    let not_nulls = execute_river(&ctx, &format!("find [id] from {}@sqlite where val is not null", tn)).await.unwrap();
    assert_row_count(&not_nulls, 1);
    assert_eq!(not_nulls.rows[0][0], Value::Int(1));

    let _ = execute_river(&ctx, &format!("remove {}@sqlite where id > 0", tn)).await;
}

// ── MongoDB ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn is_null_mongo() {
    let ctx = TestContext::new().await;
    let tn = "t12_is_null_mongo";

    let _ = execute_river(&ctx, &format!("remove {}@mongo where val = val", tn)).await;
    execute_river(&ctx, &format!("create table {}@mongo (id int, val string)", tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@mongo {{ id: 1, val: "a" }}"#, tn)).await.unwrap();
    execute_river(&ctx, &format!(r#"create {}@mongo {{ id: 2 }}"#, tn)).await.unwrap();

    let nulls = execute_river(&ctx, &format!("find [id] from {}@mongo where val is null", tn)).await.unwrap();
    assert_row_count(&nulls, 1);
    assert_eq!(nulls.rows[0][0], Value::Int(2));

    let not_nulls = execute_river(&ctx, &format!("find [id] from {}@mongo where val is not null", tn)).await.unwrap();
    assert_row_count(&not_nulls, 1);
    assert_eq!(not_nulls.rows[0][0], Value::Int(1));

    let _ = execute_river(&ctx, &format!("remove {}@mongo where val = val", tn)).await;
}

// ── Cross-DB Consistency ───────────────────────────────────────────────────

#[tokio::test]
async fn is_null_count_consistency_pg_mysql_sqlite() {
    let ctx = TestContext::new().await;
    let tn = "t12_null_consistency";

    for conn in &["pg", "mysql", "sqlite"] {
        let _ = execute_river(&ctx, &format!("remove {}@{} where id > 0", tn, conn)).await;
        execute_river(&ctx, &format!("create table if not exists {}@{} (id int primary key, val string)", tn, conn)).await.unwrap();
        execute_river(&ctx, &format!(r#"create {}@{} {{ id: 1, val: "x" }}"#, tn, conn)).await.unwrap();
        execute_river(&ctx, &format!(r#"create {}@{} {{ id: 2 }}"#, tn, conn)).await.unwrap();
        execute_river(&ctx, &format!(r#"create {}@{} {{ id: 3, val: "y" }}"#, tn, conn)).await.unwrap();
    }

    let pg_null = execute_river(&ctx, &format!("find [count(*) as cnt] from {}@pg where val is null", tn)).await.unwrap();
    let mysql_null = execute_river(&ctx, &format!("find [count(*) as cnt] from {}@mysql where val is null", tn)).await.unwrap();
    let sqlite_null = execute_river(&ctx, &format!("find [count(*) as cnt] from {}@sqlite where val is null", tn)).await.unwrap();
    assert_eq!(pg_null.rows[0][0], mysql_null.rows[0][0]);
    assert_eq!(pg_null.rows[0][0], sqlite_null.rows[0][0]);

    let pg_not = execute_river(&ctx, &format!("find [count(*) as cnt] from {}@pg where val is not null", tn)).await.unwrap();
    let mysql_not = execute_river(&ctx, &format!("find [count(*) as cnt] from {}@mysql where val is not null", tn)).await.unwrap();
    let sqlite_not = execute_river(&ctx, &format!("find [count(*) as cnt] from {}@sqlite where val is not null", tn)).await.unwrap();
    assert_eq!(pg_not.rows[0][0], mysql_not.rows[0][0]);
    assert_eq!(pg_not.rows[0][0], sqlite_not.rows[0][0]);

    for conn in &["pg", "mysql", "sqlite"] {
        let _ = execute_river(&ctx, &format!("remove {}@{} where id > 0", tn, conn)).await;
    }
}
