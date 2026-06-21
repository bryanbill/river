use crate::common::{assert_no_nulls, assert_row_count, execute_river, TestContext};
use river::adapters::Value;

// ── CTEs ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn cte_basic() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"with high_earners as (find [id, name, salary] from users@pg where salary > 100000)
find [name, salary] from high_earners order by salary desc limit 20"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 20);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "salary");
    // All salaries should be > 100000
    for row in &result.rows {
        let sal_idx = result.columns.iter().position(|c| c == "salary").unwrap();
        match &row[sal_idx] {
            Value::Int(i) => assert!(*i > 100000, "salary should be > 100000, got {}", i),
            Value::Float(f) => assert!(*f > 100000.0, "salary should be > 100000, got {}", f),
            other => panic!("Expected numeric salary, got {:?}", other),
        }
    }
}

#[tokio::test]
async fn cte_with_aggregation() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"with dept_stats as (find [department, count(*) as cnt, avg(salary) as avg_sal] from users@pg group by department)
find [department, cnt, avg_sal] from dept_stats"#,
    )
    .await
    .unwrap();
    // 10 departments
    assert_row_count(&result, 10);
    assert_no_nulls(&result, "department");
    assert_no_nulls(&result, "cnt");
}

#[tokio::test]
async fn cte_chained() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"with
  paid_orders as (find [user_id, total] from orders@pg where status = "paid"),
  user_revenue as (find [user_id, sum(total) as revenue] from paid_orders group by user_id)
find [user_id, revenue] from user_revenue where revenue > 100 order by revenue desc limit 20"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 20);
    assert_no_nulls(&result, "revenue");
}

// ── DISTINCT ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn distinct_departments() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find distinct [department] from users@pg"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 10);
}

#[tokio::test]
async fn distinct_categories() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find distinct [category] from products@pg"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 10);
}

#[tokio::test]
async fn distinct_order_statuses() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find distinct [status] from orders@pg"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 6);
}

// ── CAST ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn cast_int_to_float() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, cast(salary as float) as salary_f] from users@pg limit 5"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 5);
    assert_no_nulls(&result, "salary_f");
}

// ── Cross-DB Consistency ────────────────────────────────────────────────────

#[tokio::test]
async fn distinct_consistency_pg_mysql() {
    let ctx = TestContext::new().await;
    let pg = execute_river(
        &ctx,
        r#"find distinct [department] from users@pg"#,
    )
    .await
    .unwrap();
    let mysql = execute_river(
        &ctx,
        r#"find distinct [department] from users@mysql"#,
    )
    .await
    .unwrap();
    assert_eq!(pg.rows.len(), mysql.rows.len());
}
