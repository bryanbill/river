use crate::common::{assert_no_nulls, assert_row_count, execute_river, TestContext};

// ── Cross-DB Equi-Join (SemiJoinFetch) ──────────────────────────────────────

#[tokio::test]
async fn cross_db_pg_mysql_equi_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u join orders@mysql as o on u.id = o.user_id limit 100"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 100);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");
}

#[tokio::test]
async fn cross_db_pg_sqlite_equi_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@pg as u join orders@sqlite as o on u.id = o.user_id limit 100"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 100);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");
}

#[tokio::test]
async fn cross_db_mysql_sqlite_equi_join() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, o.total] from users@mysql as u join orders@sqlite as o on u.id = o.user_id limit 100"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 100);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "total");
}

// ── Cross-DB Equi-Join Full Count ───────────────────────────────────────────

#[tokio::test]
async fn cross_db_equi_join_full_count() {
    let ctx = TestContext::new().await;
    // All orders have valid user_ids, so cross-DB inner join count should equal order count
    let cross_db = execute_river(
        &ctx,
        r#"find [count(*) as cnt] from users@pg as u join orders@mysql as o on u.id = o.user_id"#,
    )
    .await
    .unwrap();
    let order_count = execute_river(&ctx, r#"find [count(*) as cnt] from orders@mysql"#)
        .await
        .unwrap();
    assert_eq!(cross_db.rows[0][0], order_count.rows[0][0]);
}

// ── Cross-DB Cross Join WITH LIMIT ──────────────────────────────────────────

#[tokio::test]
async fn cross_db_cross_join_with_limit_works() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, p.name as product] from users@pg as u cross join products@mysql as p limit 25"#,
    )
    .await
    .unwrap();
    assert_row_count(&result, 25);
    assert_no_nulls(&result, "name");
    assert_no_nulls(&result, "product");
}

// ── Cross-DB Cross Join WITHOUT LIMIT (rejected) ────────────────────────────

#[tokio::test]
async fn cross_db_cross_join_without_limit_rejected() {
    let ctx = TestContext::new().await;
    let result = execute_river(
        &ctx,
        r#"find [u.name, p.name as product] from users@pg as u cross join products@mysql as p"#,
    )
    .await;
    assert!(result.is_err(), "Unbounded cross-DB cross join should be rejected");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("LIMIT") || err.contains("limit"),
        "Error should mention LIMIT, got: {}",
        err
    );
}

// ── Cross-DB Consistency Verification ───────────────────────────────────────

#[tokio::test]
async fn cross_db_join_result_matches_same_db() {
    let ctx = TestContext::new().await;
    // Cross-DB join (pg users + mysql orders) should give same count as same-DB join
    let cross = execute_river(
        &ctx,
        r#"find [count(*) as cnt] from users@pg as u join orders@mysql as o on u.id = o.user_id"#,
    )
    .await
    .unwrap();
    let same = execute_river(
        &ctx,
        r#"find [count(*) as cnt] from users@pg as u join orders@pg as o on u.id = o.user_id"#,
    )
    .await
    .unwrap();
    assert_eq!(cross.rows[0][0], same.rows[0][0]);
}

// ── Same Table from Different DBs ───────────────────────────────────────────

#[tokio::test]
async fn same_table_different_dbs_count_match() {
    let ctx = TestContext::new().await;
    let pg = execute_river(&ctx, r#"find [count(*) as cnt] from users@pg"#)
        .await
        .unwrap();
    let mysql = execute_river(&ctx, r#"find [count(*) as cnt] from users@mysql"#)
        .await
        .unwrap();
    let sqlite = execute_river(&ctx, r#"find [count(*) as cnt] from users@sqlite"#)
        .await
        .unwrap();
    assert_eq!(pg.rows[0][0], mysql.rows[0][0]);
    assert_eq!(pg.rows[0][0], sqlite.rows[0][0]);
}
