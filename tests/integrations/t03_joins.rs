use crate::common::{
    assert_no_nulls, assert_row_count, assert_row_count_gte, execute_river, TestContext,
};
use river::adapters::Value;

// ── Inner Join ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn inner_join_users_orders() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u join orders@pg as o on u.id = o.user_id limit 100"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 100);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");
}

#[tokio::test]
async fn inner_join_all_orders_have_users() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [count(*) as cnt] from users@pg as u join orders@pg as o on u.id = o.user_id"#,
    )
    .await
    .unwrap();
    // Every order has a valid user_id, so inner join count == order count
    let order_count = execute_river(&ctx, r#"find [count(*) as cnt] from orders@pg"#)
        .await
        .unwrap();
    assert_eq!(result.rows[0][0], order_count.rows[0][0]);
}

// ── Left Join ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn left_join_preserves_all_left_rows() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [count(*) as cnt] from users@pg as u left join orders@pg as o on u.id = o.user_id"#,
    )
    .await
    .unwrap();
    // Left join should have >= user count (users without orders appear once, users with orders appear N times)
    assert_row_count_gte(&result, 1);
    let cnt = match &result.rows[0][0] {
        Value::Int(i) => *i,
        _ => panic!("Expected int count"),
    };
    assert!(cnt >= 10000, "Left join should have at least 10000 rows (one per user), got {}", cnt);
}

// ── Cross Join ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn cross_join_with_limit() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, p.name as product] from users@pg as u cross join products@pg as p limit 50"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 50);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "product");
}

// ── Multiple Joins ──────────────────────────────────────────────────────────

#[tokio::test]
async fn triple_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total, oi.quantity]
from users@pg as u
join orders@pg as o on u.id = o.user_id
join order_items@pg as oi on o.id = oi.order_id
limit 50"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 50);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");
    assert_no_nulls(&result, "quantity");
}

// ── Join with Filter ────────────────────────────────────────────────────────

#[tokio::test]
async fn join_with_where_clause() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total]
from users@pg as u
join orders@pg as o on u.id = o.user_id
where o.status = "paid"
limit 100"#,
    )
    .await
    .unwrap();
    assert_row_count_gte(&result, 1);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");
}

// ── Join with Order By ──────────────────────────────────────────────────────

#[tokio::test]
async fn join_with_order_by() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total]
from users@pg as u
join orders@pg as o on u.id = o.user_id
order by o.total desc
limit 20"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 20);
}

// ── Cross-DB Consistency ────────────────────────────────────────────────────

#[tokio::test]
async fn join_count_consistency_pg_mysql() {
    let ctx = TestContext::new().await;
    let pg = execute_river(
        &ctx,
        r#"find [count(*) as cnt] from users@pg as u join orders@pg as o on u.id = o.user_id"#,
    )
    .await
    .unwrap();
    let mysql = execute_river(
        &ctx,
        r#"find [count(*) as cnt] from users@mysql as u join orders@mysql as o on u.id = o.user_id"#,
    )
    .await
    .unwrap();
    assert_eq!(pg.rows[0][0], mysql.rows[0][0]);
}
