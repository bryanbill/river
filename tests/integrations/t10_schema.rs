use crate::common::{assert_columns, assert_row_count, assert_row_count_gte, execute_river, TestContext};
use river::adapters::Value;
use river::error::RiverError;

// ── Schema-qualified queries ─────────────────────────────────────────────────

#[tokio::test]
async fn schema_public_users_pg() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find * from public.users@pg limit 10")
        .await
        .unwrap();
    assert_row_count(&result, 10);
}

#[tokio::test]
async fn schema_river_users_mysql() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find * from river.users@mysql limit 10")
        .await
        .unwrap();
    assert_row_count(&result, 10);
}

#[tokio::test]
async fn schema_qualified_with_filter() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [id, name] from public.users@pg where id = 1"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 1);
    assert_columns(&result, &["id", "name"]);
}

// ── Schema-qualified describe ────────────────────────────────────────────────

#[tokio::test]
async fn describe_schema_public_users_pg() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "describe public.users@pg")
        .await
        .unwrap();
    assert_row_count_gte(&result, 7);
    let has_name = result.rows.iter().any(|row| {
        row.iter()
            .any(|v| matches!(v, Value::String(s) if s == "name"))
    });
    assert!(has_name, "describe public.users should include 'name'");
}

#[tokio::test]
async fn describe_schema_river_users_mysql() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "describe river.users@mysql")
        .await
        .unwrap();
    assert_row_count_gte(&result, 7);
    let has_email = result.rows.iter().any(|row| {
        row.iter()
            .any(|v| matches!(v, Value::String(s) if s == "email"))
    });
    assert!(has_email, "describe river.users should include 'email'");
}

#[tokio::test]
async fn describe_schema_public_orders_pg() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "describe public.orders@pg")
        .await
        .unwrap();
    assert_row_count_gte(&result, 4);
}

// ── Wrong schema returns error ───────────────────────────────────────────────

#[tokio::test]
async fn wrong_schema_pg_errors() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find * from nonexistent.users@pg limit 1").await;
    assert!(
        result.is_err(),
        "query against nonexistent schema should error"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, RiverError::Connection(_) | RiverError::Other(_)),
        "expected database error for nonexistent schema, got: {:?}",
        err
    );
}

#[tokio::test]
async fn wrong_schema_mysql_errors() {
    let ctx = TestContext::new().await;
    let result = execute_river(&ctx, "find * from nonexistent.users@mysql limit 1").await;
    assert!(
        result.is_err(),
        "query against nonexistent schema in mysql should error"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, RiverError::Connection(_) | RiverError::Other(_)),
        "expected database error for nonexistent schema, got: {:?}",
        err
    );
}

#[tokio::test]
async fn wrong_schema_describe_errors() {
    let ctx = TestContext::new().await;
    // describe with a nonexistent schema returns empty result (no matching columns),
    // not a hard error
    let result = execute_river(&ctx, "describe nonexistent.users@pg").await;
    assert!(result.is_ok(), "describe should not panic on nonexistent schema");
    let describe_result = result.unwrap();
    assert_row_count(&describe_result, 0);
}

// ── Consistency: schema vs no-schema returns same data ───────────────────────

#[tokio::test]
async fn schema_vs_no_schema_same_results_pg() {
    let ctx = TestContext::new().await;
    let with_schema = execute_river(&ctx, "find [id, name, email, department, status] from public.users@pg order by id asc limit 50")
        .await
        .unwrap();
    let without_schema = execute_river(&ctx, "find [id, name, email, department, status] from users@pg order by id asc limit 50")
        .await
        .unwrap();

    assert_eq!(with_schema.rows.len(), without_schema.rows.len());
    assert_eq!(with_schema.columns, without_schema.columns);
    for i in 0..with_schema.rows.len() {
        assert_eq!(
            with_schema.rows[i], without_schema.rows[i],
            "Row {} differs between schema-qualified and bare table query", i
        );
    }
}

#[tokio::test]
async fn schema_vs_no_schema_same_results_mysql() {
    let ctx = TestContext::new().await;
    let with_schema = execute_river(&ctx, "find [id, name, email, department, status] from river.users@mysql order by id asc limit 50")
        .await
        .unwrap();
    let without_schema = execute_river(&ctx, "find [id, name, email, department, status] from users@mysql order by id asc limit 50")
        .await
        .unwrap();

    assert_eq!(with_schema.rows.len(), without_schema.rows.len());
    assert_eq!(with_schema.columns, without_schema.columns);
    for i in 0..with_schema.rows.len() {
        assert_eq!(
            with_schema.rows[i], without_schema.rows[i],
            "Row {} differs between schema-qualified and bare table query on mysql", i
        );
    }
}

// ── Schema-qualified cross-database join ─────────────────────────────────────

#[tokio::test]
async fn schema_cross_db_join_pg_mysql() {
    let ctx = TestContext::new().await;
    // Join public.users@pg with orders@mysql (schema only on pg side, no filter)
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total]
           from public.users@pg as u
           join orders@mysql as o on u.id = o.user_id
           limit 20"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 20);
    assert!(result.columns.contains(&"name".to_string()));
    assert!(result.columns.contains(&"total".to_string()));
}

#[tokio::test]
async fn schema_cross_db_join_both_sides_schema() {
    let ctx = TestContext::new().await;
    // Schema on pg (build) side and mysql (probe) side
    // Note: MySQL uses database name "river" as its schema
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total]
           from public.users@pg as u
           join river.orders@mysql as o on u.id = o.user_id
           limit 20"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 20);
}

#[tokio::test]
async fn schema_cross_db_join_pg_sqlite() {
    let ctx = TestContext::new().await;
    // SQLite has no schemas — schema prefix is ignored; this should still work
    let result = execute_river(
        &ctx,
        r#"find [u.name, p.name as product]
           from public.users@pg as u
           join products@sqlite as p on u.id = p.id
           limit 10"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 10);
}

// ── Schema-qualified with projections and ordering ───────────────────────────

#[tokio::test]
async fn schema_qualified_with_order_by() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        "find [id, name, salary] from public.users@pg order by salary desc limit 10",
    )
    .await
    .unwrap();
    assert_row_count(&result, 10);
    assert_columns(&result, &["id", "name", "salary"]);
}

#[tokio::test]
async fn schema_qualified_with_group_by() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [department, count(*) as cnt] from public.users@pg group by department order by cnt desc"#,
    )
    .await
    .unwrap();
    assert_row_count_gte(&result, 1);
    assert_columns(&result, &["department", "cnt"]);
}

// ── Schema-qualified DML ─────────────────────────────────────────────────────

#[tokio::test]
async fn schema_qualified_insert_and_cleanup() {
    let ctx = TestContext::new().await;

    // INSERT with schema qualification
    let insert = execute_river(
        &ctx,
        r#"create public.users { name: "__schema_test__", email: "schema@test.com", department: "QA", salary: 50000, status: "active", is_verified: false }"#,
    )
    .await;
    assert!(insert.is_ok(), "schema-qualified insert should succeed: {:?}", insert.err());

    // Verify it was inserted
    let verify = execute_river(
        &ctx,
        r#"find [id, name, email] from public.users@pg where name = "__schema_test__""#,
    )
    .await
    .unwrap();
    assert_row_count(&verify, 1);
    let email = verify.rows[0]
        .iter()
        .find(|v| matches!(v, Value::String(s) if s == "schema@test.com"));
    assert!(email.is_some(), "inserted row should have the expected email");

    // Cleanup
    let delete = execute_river(
        &ctx,
        r#"remove public.users@pg where name = "__schema_test__""#,
    )
    .await;
    assert!(delete.is_ok(), "schema-qualified delete should succeed: {:?}", delete.err());

    // Verify cleanup
    let verify_after = execute_river(
        &ctx,
        r#"find [id] from public.users@pg where name = "__schema_test__""#,
    )
    .await
    .unwrap();
    assert_row_count(&verify_after, 0);
}

#[tokio::test]
async fn schema_qualified_update() {
    let ctx = TestContext::new().await;

    // Insert test row (without schema so we target the default schema)
    let insert = execute_river(
        &ctx,
        r#"create users { name: "__schema_upd__", email: "upd@test.com", department: "QA", salary: 50000, status: "active", is_verified: false }"#,
    )
    .await;
    assert!(insert.is_ok(), "pre-insert should succeed: {:?}", insert.err());

    // UPDATE with schema qualification
    let update = execute_river(
        &ctx,
        r#"update public.users@pg set salary = 99999 where name = "__schema_upd__""#,
    )
    .await;
    assert!(update.is_ok(), "schema-qualified update should succeed: {:?}", update.err());

    // Verify
    let verify = execute_river(
        &ctx,
        r#"find [salary] from public.users@pg where name = "__schema_upd__""#,
    )
    .await
    .unwrap();
    assert_row_count(&verify, 1);
    let salary_match = verify.rows[0].iter().any(|v| matches!(v, Value::Float(f) if (*f - 99999.0).abs() < 0.01) || matches!(v, Value::Int(i) if *i == 99999));
    assert!(salary_match, "updated row should have salary 99999, got: {:?}", verify.rows[0]);

    // Cleanup
    execute_river(
        &ctx,
        r#"remove public.users@pg where name = "__schema_upd__""#,
    )
    .await
    .unwrap();
}

// ── Schema-qualified with CTEs ───────────────────────────────────────────────

#[tokio::test]
async fn schema_qualified_in_cte() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"with
           active_users as (
             find [id, name, department] from public.users@pg
             where status = "active" limit 20
           )
           find [name, department] from active_users order by name asc"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 20);
    assert_columns(&result, &["name", "department"]);
}

// ── Schema-qualified with joins (same database) ──────────────────────────────

#[tokio::test]
async fn schema_qualified_single_db_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.status]
           from public.users@pg as u
           join public.orders@pg as o on u.id = o.user_id
           where o.total > 100
           limit 10"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 10);
    assert!(result.columns.contains(&"name".to_string()));
    assert!(result.columns.contains(&"status".to_string()));
}

// ── Describe consistency: schema vs no-schema same column count ──────────────

#[tokio::test]
async fn describe_schema_consistency_pg() {
    let ctx = TestContext::new().await;
    let with_schema = execute_river(&ctx, "describe public.users@pg")
        .await
        .unwrap();
    let without_schema = execute_river(&ctx, "describe users@pg")
        .await
        .unwrap();

    assert_eq!(
        with_schema.rows.len(),
        without_schema.rows.len(),
        "describe with and without schema should return same column count for 'users'"
    );
}

#[tokio::test]
async fn describe_schema_consistency_mysql() {
    let ctx = TestContext::new().await;
    let with_schema = execute_river(&ctx, "describe river.users@mysql")
        .await
        .unwrap();
    let without_schema = execute_river(&ctx, "describe users@mysql")
        .await
        .unwrap();

    assert_eq!(
        with_schema.rows.len(),
        without_schema.rows.len(),
        "describe with and without schema should return same column count for 'users' on mysql"
    );
}
