use crate::common::{
    assert_all_match, assert_row_count, assert_row_count_gte, execute_river, TestContext,
};
use crate::helpers;
use river::adapters::Value;

// ── Equality ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn filter_by_status_active() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [id, status] from users@pg where status = "active""#,
    )
    .await
    .unwrap();
    let expected = helpers::count_users_with_status("active");
    assert_row_count(&result, expected);
    assert_all_match(&result, "status", |v| matches!(v, Value::String(s) if s == "active"));
}

#[tokio::test]
async fn filter_by_status_not_equal() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [id, status] from users@pg where status != "active""#,
    )
    .await
    .unwrap();
    let expected = helpers::ROWS - helpers::count_users_with_status("active");
    assert_row_count(&result, expected);
    assert_all_match(&result, "status", |v| matches!(v, Value::String(s) if s != "active"));
}

// ── Comparison ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn filter_salary_greater_than() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, salary] from users@pg where salary > 100000"#,
    )
    .await
    .unwrap();
    assert_row_count_gte(&result, 1);
    assert_all_match(&result, "salary", |v| match v {
        Value::Int(i) => *i > 100000,
        Value::Float(f) => *f > 100000.0,
        _ => false,
    });
}

#[tokio::test]
async fn filter_salary_less_than_or_equal() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, salary] from users@pg where salary <= 50000"#,
    )
    .await
    .unwrap();
    assert_row_count_gte(&result, 1);
    assert_all_match(&result, "salary", |v| match v {
        Value::Int(i) => *i <= 50000,
        Value::Float(f) => *f <= 50000.0,
        _ => false,
    });
}

// ── Logical Operators ───────────────────────────────────────────────────────

#[tokio::test]
async fn filter_and_combination() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, status, department] from users@pg where status = "active" and department = "Engineering""#,
    )
    .await
    .unwrap();
    assert_row_count_gte(&result, 1);
    assert_all_match(&result, "status", |v| matches!(v, Value::String(s) if s == "active"));
    assert_all_match(&result, "department", |v| matches!(v, Value::String(s) if s == "Engineering"));
}

#[tokio::test]
async fn filter_or_combination() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, department] from users@pg where department = "Sales" or department = "Marketing""#,
    )
    .await
    .unwrap();
    let expected = helpers::count_users_in_department("Sales")
        + helpers::count_users_in_department("Marketing");
    assert_row_count(&result, expected);
}

// ── IN ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn filter_in_list() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, department] from users@pg where department in ("Sales", "Marketing", "HR")"#,
    )
    .await
    .unwrap();
    let expected = helpers::count_users_in_department("Sales")
        + helpers::count_users_in_department("Marketing")
        + helpers::count_users_in_department("HR");
    assert_row_count(&result, expected);
    assert_all_match(&result, "department", |v| {
        matches!(v, Value::String(s) if s == "Sales" || s == "Marketing" || s == "HR")
    });
}

// ── BETWEEN ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn filter_between() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, salary] from users@pg where salary between 50000 and 60000"#,
    )
    .await
    .unwrap();
    assert_row_count_gte(&result, 1);
    assert_all_match(&result, "salary", |v| match v {
        Value::Int(i) => *i >= 50000 && *i <= 60000,
        Value::Float(f) => *f >= 50000.0 && *f <= 60000.0,
        _ => false,
    });
}

// ── LIKE ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn filter_like_prefix() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name] from users@pg where name like "Alice%""#,
    )
    .await
    .unwrap();
    assert_row_count_gte(&result, 1);
    assert_all_match(&result, "name", |v| {
        matches!(v, Value::String(s) if s.starts_with("Alice"))
    });
}

// ── Cross-DB Consistency ────────────────────────────────────────────────────

#[tokio::test]
async fn filter_count_consistency_pg_mysql() {
    let ctx = TestContext::new().await;
    let pg = execute_river(
        &ctx,
        r#"find [count(*) as cnt] from users@pg where status = "active""#,
    )
    .await
    .unwrap();
    let mysql = execute_river(
        &ctx,
        r#"find [count(*) as cnt] from users@mysql where status = "active""#,
    )
    .await
    .unwrap();
    assert_eq!(pg.rows[0][0], mysql.rows[0][0]);
}

#[tokio::test]
async fn filter_count_consistency_pg_sqlite() {
    let ctx = TestContext::new().await;
    let pg = execute_river(
        &ctx,
        r#"find [count(*) as cnt] from users@pg where salary > 80000"#,
    )
    .await
    .unwrap();
    let sqlite = execute_river(
        &ctx,
        r#"find [count(*) as cnt] from users@sqlite where salary > 80000"#,
    )
    .await
    .unwrap();
    assert_eq!(pg.rows[0][0], sqlite.rows[0][0]);
}
