use crate::common::{assert_row_count, execute_river, TestContext};
use crate::helpers;
use river::adapters::Value;

// ── Count ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn count_all_users() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [count(*) as cnt] from users@pg"#)
        .await
        .unwrap();
    assert_row_count(&result, 1);
    assert_eq!(result.rows[0][0], Value::Int(helpers::ROWS as i64));
}

#[tokio::test]
async fn count_all_orders() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [count(*) as cnt] from orders@pg"#)
        .await
        .unwrap();
    assert_eq!(result.rows[0][0], Value::Int(helpers::ROWS as i64));
}

// ── Sum ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn sum_order_totals() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [sum(total) as total_revenue] from orders@pg"#)
        .await
        .unwrap();
    assert_row_count(&result, 1);
    // Just verify it returns a positive number
    match &result.rows[0][0] {
        Value::Int(i) => assert!(*i > 0),
        Value::Float(f) => assert!(*f > 0.0),
        other => panic!("Expected numeric sum, got {:?}", other),
    }
}

// ── Avg ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn avg_salary() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, r#"find [avg(salary) as avg_sal] from users@pg"#)
        .await
        .unwrap();
    assert_row_count(&result, 1);
    let avg = match &result.rows[0][0] {
        Value::Int(i) => *i as f64,
        Value::Float(f) => *f,
        other => panic!("Expected numeric avg, got {:?}", other),
    };
    // salary range is 35000-149983, avg should be roughly in the middle
    assert!(avg > 35000.0 && avg < 150000.0, "avg salary {} out of expected range", avg);
}

// ── Min / Max ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn min_max_price() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [min(price) as min_p, max(price) as max_p] from products@pg"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 1);
    let min_val = match &result.rows[0][0] {
        Value::Int(i) => *i as f64,
        Value::Float(f) => *f,
        other => panic!("Expected numeric min, got {:?}", other),
    };
    let max_val = match &result.rows[0][1] {
        Value::Int(i) => *i as f64,
        Value::Float(f) => *f,
        other => panic!("Expected numeric max, got {:?}", other),
    };
    assert!(min_val < max_val, "min {} should be < max {}", min_val, max_val);
    assert!(min_val >= 0.0, "min price should be non-negative");
}

// ── GROUP BY ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn group_by_department() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [department, count(*) as cnt] from users@pg group by department"#,
    )
    .await
    .unwrap();
    // 10 departments
    assert_row_count(&result, 10);
}

#[tokio::test]
async fn group_by_order_status() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [status, count(*) as cnt] from orders@pg group by status"#,
    )
    .await
    .unwrap();
    // 6 order statuses
    assert_row_count(&result, 6);
}

#[tokio::test]
async fn group_by_with_sum() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [status, sum(total) as revenue] from orders@pg group by status"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 6);
}

// ── HAVING ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn having_filter() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [department, count(*) as cnt] from users@pg group by department having count(*) > 500"#,
    )
    .await
    .unwrap();
    // All departments should have ~1000 users (10000/10), so all pass > 500
    assert_row_count(&result, 10);
}

#[tokio::test]
async fn having_filter_restrictive() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [department, count(*) as cnt] from users@pg group by department having count(*) > 1500"#,
    )
    .await
    .unwrap();
    // Each dept has ~1000, so none should pass > 1500
    assert_row_count(&result, 0);
}

// ── Multiple Aggregates ─────────────────────────────────────────────────────

#[tokio::test]
async fn multiple_aggregates() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [count(*) as total, sum(total) as revenue, avg(total) as avg_order] from orders@pg"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 1);
    // count should be 10000
    assert_eq!(result.rows[0][0], Value::Int(10000));
}

// ── Cross-DB Consistency ────────────────────────────────────────────────────

#[tokio::test]
async fn aggregation_consistency_pg_mysql() {
    let ctx = TestContext::new().await;
    let pg = execute_river(
        &ctx,
        r#"find [count(*) as cnt, sum(salary) as total] from users@pg"#,
    )
    .await
    .unwrap();
    let mysql = execute_river(
        &ctx,
        r#"find [count(*) as cnt, sum(salary) as total] from users@mysql"#,
    )
    .await
    .unwrap();
    assert_eq!(pg.rows[0][0], mysql.rows[0][0]);
    assert_eq!(pg.rows[0][1], mysql.rows[0][1]);
}
