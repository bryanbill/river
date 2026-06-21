use crate::common::{assert_row_count_gte, execute_river, TestContext};
use river::adapters::Value;

// ── SHOW TABLES ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn show_tables_pg() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "show tables @pg").await.unwrap();
    assert_row_count_gte(&result, 4); // at least users, products, orders, order_items
    // Check that known tables appear in the result
    let has_users = result.rows.iter().any(|row| {
        row.iter()
            .any(|v| matches!(v, Value::String(s) if s.contains("users")))
    });
    assert!(has_users, "show tables should include 'users'");
}

#[tokio::test]
async fn show_tables_mysql() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "show tables @mysql").await.unwrap();
    assert_row_count_gte(&result, 4);
    let has_orders = result.rows.iter().any(|row| {
        row.iter()
            .any(|v| matches!(v, Value::String(s) if s.contains("orders")))
    });
    assert!(has_orders, "show tables should include 'orders'");
}

#[tokio::test]
async fn show_tables_sqlite() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "show tables @sqlite").await.unwrap();
    assert_row_count_gte(&result, 4);
    let has_products = result.rows.iter().any(|row| {
        row.iter()
            .any(|v| matches!(v, Value::String(s) if s.contains("products")))
    });
    assert!(has_products, "show tables should include 'products'");
}

// ── DESCRIBE ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn describe_users_pg() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "describe users @pg").await.unwrap();
    // users has: id, name, email, department, salary, status, is_verified, created_at
    assert_row_count_gte(&result, 7);
    // Check that 'name' column appears somewhere
    let has_name = result.rows.iter().any(|row| {
        row.iter()
            .any(|v| matches!(v, Value::String(s) if s == "name"))
    });
    assert!(has_name, "describe users should include 'name' column");
}

#[tokio::test]
async fn describe_users_mysql() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "describe users @mysql").await.unwrap();
    assert_row_count_gte(&result, 7);
    let has_salary = result.rows.iter().any(|row| {
        row.iter()
            .any(|v| matches!(v, Value::String(s) if s == "salary"))
    });
    assert!(has_salary, "describe users should include 'salary' column");
}

#[tokio::test]
async fn describe_orders_pg() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "describe orders @pg").await.unwrap();
    // orders has: id, user_id, status, total, created_at
    assert_row_count_gte(&result, 4);
    let has_total = result.rows.iter().any(|row| {
        row.iter()
            .any(|v| matches!(v, Value::String(s) if s == "total"))
    });
    assert!(has_total, "describe orders should include 'total' column");
}

#[tokio::test]
async fn describe_products_sqlite() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "describe products @sqlite").await.unwrap();
    // products has: id, name, category, price, stock, rating, is_active, created_at
    assert_row_count_gte(&result, 7);
    let has_price = result.rows.iter().any(|row| {
        row.iter()
            .any(|v| matches!(v, Value::String(s) if s == "price"))
    });
    assert!(has_price, "describe products should include 'price' column");
}

// ── Cross-DB Consistency ────────────────────────────────────────────────────

#[tokio::test]
async fn describe_column_count_consistency() {
    let ctx = TestContext::new().await;
    let pg = execute_river(&ctx, "describe users @pg").await.unwrap();
    let mysql = execute_river(&ctx, "describe users @mysql").await.unwrap();
    // Same table definition → same number of columns
    assert_eq!(
        pg.rows.len(),
        mysql.rows.len(),
        "pg and mysql should have same column count for users table"
    );
}
