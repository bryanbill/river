use crate::common::{assert_no_nulls, assert_row_count, execute_river, TestContext};
use river::adapters::Value;

// ── ROW_NUMBER ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn row_number_over_partition() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, department, salary, row_number() over (partition by department order by salary desc) as rn]
from users@pg
limit 100"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 100);
    assert_no_nulls(&result, "rn");
    // row numbers should be positive integers
    for row in &result.rows {
        let rn_idx = result.columns.iter().position(|c| c == "rn").unwrap();
        match &row[rn_idx] {
            Value::Int(i) => assert!(*i >= 1, "row_number should be >= 1, got {}", i),
            other => panic!("Expected Int for row_number, got {:?}", other),
        }
    }
}

// ── RANK ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn rank_over_salary() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, salary, rank() over (order by salary desc) as rnk]
from users@pg
limit 50"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 50);
    assert_no_nulls(&result, "rnk");
    // First row should have rank 1
    let rnk_idx = result.columns.iter().position(|c| c == "rnk").unwrap();
    assert_eq!(result.rows[0][rnk_idx], Value::Int(1));
}

// ── DENSE_RANK ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn dense_rank_over_department() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, department, salary, dense_rank() over (partition by department order by salary desc) as dr]
from users@pg
limit 100"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 100);
    assert_no_nulls(&result, "dr");
}

// ── LAG ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn lag_salary() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [name, salary, lag(salary, 1) over (order by salary asc) as prev_salary]
from users@pg
limit 50"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 50);
    // First row's lag should be NULL (no previous)
    let prev_idx = result
        .columns
        .iter()
        .position(|c| c == "prev_salary")
        .unwrap();
    assert_eq!(result.rows[0][prev_idx], Value::Null);
}

// ── Running Total (SUM window) ──────────────────────────────────────────────

#[tokio::test]
async fn running_total() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [id, total, sum(total) over (order by id) as running_total]
from orders@pg
limit 20"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 20);
    assert_no_nulls(&result, "running_total");
    // Running total should be non-decreasing (all totals are positive)
    let rt_idx = result
        .columns
        .iter()
        .position(|c| c == "running_total")
        .unwrap();
    for window in result.rows.windows(2) {
        let a = match &window[0][rt_idx] {
            Value::Int(i) => *i as f64,
            Value::Float(f) => *f,
            _ => continue,
        };
        let b = match &window[1][rt_idx] {
            Value::Int(i) => *i as f64,
            Value::Float(f) => *f,
            _ => continue,
        };
        assert!(
            b >= a,
            "Running total should be non-decreasing: {} > {}",
            a,
            b
        );
    }
}

// ── Cross-DB Consistency ────────────────────────────────────────────────────

#[tokio::test]
async fn window_count_consistency_pg_mysql() {
    let ctx = TestContext::new().await;
    let pg = execute_river(
        &ctx,
        r#"find [name, row_number() over (order by id) as rn] from users@pg limit 10"#,
    )
    .await
    .unwrap();
    let mysql = execute_river(
        &ctx,
        r#"find [name, row_number() over (order by id) as rn] from users@mysql limit 10"#,
    )
    .await
    .unwrap();
    assert_row_count(&pg, 10);
    assert_row_count(&mysql, 10);
    // Same data, same ordering -> same row numbers
    let rn_idx_pg = pg.columns.iter().position(|c| c == "rn").unwrap();
    let rn_idx_my = mysql.columns.iter().position(|c| c == "rn").unwrap();
    for i in 0..10 {
        assert_eq!(pg.rows[i][rn_idx_pg], mysql.rows[i][rn_idx_my]);
    }
}
