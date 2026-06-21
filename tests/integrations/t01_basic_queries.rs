use crate::common::{
    assert_columns, assert_no_nulls, assert_ordered_asc, assert_ordered_desc, assert_row_count,
    execute_river, TestContext,
};
use crate::helpers;
use river::adapters::Value;

// ── Basic Find ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn find_all_users_pg() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find users@pg limit 100").await.unwrap();
    assert_row_count(&result, 100);
}

#[tokio::test]
async fn find_all_users_mysql() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find users@mysql limit 100").await.unwrap();
    assert_row_count(&result, 100);
}

#[tokio::test]
async fn find_all_users_sqlite() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find users@sqlite limit 100").await.unwrap();
    assert_row_count(&result, 100);
}

// ── Column Selection ────────────────────────────────────────────────────────

#[tokio::test]
async fn select_specific_columns() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [name, email] from users@pg limit 5"#)
        .await
        .unwrap();
    assert_columns(&result, &["name", "email"]);
    assert_row_count(&result, 5);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "email");
}

#[tokio::test]
async fn select_three_columns() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, department, salary] from users@pg limit 10"#,
    )
    .await
    .unwrap();
    assert_columns(&result, &["name", "department", "salary"]);
    assert_row_count(&result, 10);
}

// ── Limit & Offset ──────────────────────────────────────────────────────────

#[tokio::test]
async fn limit_results() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find users@pg limit 25").await.unwrap();
    assert_row_count(&result, 25);
}

#[tokio::test]
async fn limit_and_offset() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find users@pg limit 10 offset 50")
        .await
        .unwrap();
    assert_row_count(&result, 10);
}

#[tokio::test]
async fn offset_skips_rows() {
    let ctx = TestContext::new().await;
    let all = execute_river(&ctx, r#"find [id] from users@pg order by id asc limit 20"#)
        .await
        .unwrap();
    let offset = execute_river(
        &ctx,
        r#"find [id] from users@pg order by id asc limit 10 offset 10"#,
    )
    .await
    .unwrap();
    // The first row of offset result should match row 10 of the full result
    assert_eq!(offset.rows[0][0], all.rows[10][0]);
}

// ── Ordering ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn order_by_salary_asc() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, salary] from users@pg order by salary asc limit 50"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 50);
    assert_ordered_asc(&result, "salary");
}

#[tokio::test]
async fn order_by_salary_desc() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, salary] from users@pg order by salary desc limit 50"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 50);
    assert_ordered_desc(&result, "salary");
}

#[tokio::test]
async fn order_by_name_asc() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name] from users@pg order by name asc limit 50"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 50);
    assert_ordered_asc(&result, "name");
}

// ── Cross-DB Consistency ────────────────────────────────────────────────────

#[tokio::test]
async fn count_consistency_pg_mysql() {
    let ctx = TestContext::new().await;
    let pg = execute_river(&ctx, r#"find [count(*) as cnt] from users@pg"#)
        .await
        .unwrap();
    let mysql = execute_river(&ctx, r#"find [count(*) as cnt] from users@mysql"#)
        .await
        .unwrap();
    assert_eq!(pg.rows[0][0], mysql.rows[0][0]);
}

#[tokio::test]
async fn count_consistency_pg_sqlite() {
    let ctx = TestContext::new().await;
    let pg = execute_river(&ctx, r#"find [count(*) as cnt] from users@pg"#)
        .await
        .unwrap();
    let sqlite = execute_river(&ctx, r#"find [count(*) as cnt] from users@sqlite"#)
        .await
        .unwrap();
    assert_eq!(pg.rows[0][0], sqlite.rows[0][0]);
}

#[tokio::test]
async fn exact_row_count_is_10000() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [count(*) as cnt] from users@pg"#)
        .await
        .unwrap();
    assert_eq!(result.rows[0][0], Value::Int(helpers::ROWS as i64));
}
