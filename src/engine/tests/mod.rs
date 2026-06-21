use super::translator::{
    MSSQLDialect, MySQLDialect, PostgresDialect, SqlDialect, translate_expr,
    translate_query, translate_query_mongo, translate_statement_sql,
};
use crate::lang::ast::*;

fn make_query(projection: Vec<Projection>, sources: Vec<Source>, filter: Option<Expression>) -> Query {
    Query {
        projection,
        sources,
        filter,
        ..Query::default()
    }
}

fn table_source(name: &str) -> Source {
    Source {
        name: name.to_string(),
        alias: None,
        connection: None,
        kind: SourceKind::Table(name.to_string()),
    }
}

// ── SQL: find users ───────────────────────────────────────────────────────

#[test]
fn translate_find_users_postgres() {
    let q = make_query(vec![], vec![table_source("users")], None);
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, r#"SELECT * FROM "users""#);
}

#[test]
fn translate_find_users_mysql() {
    let q = make_query(vec![], vec![table_source("users")], None);
    let dialect = MySQLDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, "SELECT * FROM `users`");
}

#[test]
fn translate_find_users_mssql() {
    let q = make_query(vec![], vec![table_source("users")], None);
    let dialect = MSSQLDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, "SELECT * FROM [users]");
}

// ── SQL: find users where age > 21 ────────────────────────────────────────

#[test]
fn translate_filter_postgres() {
    let filter = Expression::BinaryOp {
        op: BinaryOp::Gt,
        left: Box::new(Expression::Ident("age".into())),
        right: Box::new(Expression::Integer(21)),
    };
    let q = make_query(vec![], vec![table_source("users")], Some(filter));
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, r#"SELECT * FROM "users" WHERE ("age" > 21)"#);
}

#[test]
fn translate_filter_mysql() {
    let filter = Expression::BinaryOp {
        op: BinaryOp::Gt,
        left: Box::new(Expression::Ident("age".into())),
        right: Box::new(Expression::Integer(21)),
    };
    let q = make_query(vec![], vec![table_source("users")], Some(filter));
    let dialect = MySQLDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, "SELECT * FROM `users` WHERE (`age` > 21)");
}

#[test]
fn translate_filter_mssql() {
    let filter = Expression::BinaryOp {
        op: BinaryOp::Gt,
        left: Box::new(Expression::Ident("age".into())),
        right: Box::new(Expression::Integer(21)),
    };
    let q = make_query(vec![], vec![table_source("users")], Some(filter));
    let dialect = MSSQLDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, "SELECT * FROM [users] WHERE ([age] > 21)");
}

// ── SQL: projection ───────────────────────────────────────────────────────

#[test]
fn translate_projection_postgres() {
    let proj = vec![
        Projection::Expr(Expression::Ident("name".into()), None),
        Projection::Expr(Expression::Ident("email".into()), None),
    ];
    let q = make_query(proj, vec![table_source("users")], None);
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, r#"SELECT "name", "email" FROM "users""#);
}

#[test]
fn translate_wildcard_projection() {
    let proj = vec![Projection::Wildcard];
    let q = make_query(proj, vec![table_source("users")], None);
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, r#"SELECT * FROM "users""#);
}

// ── SQL: aggregation ──────────────────────────────────────────────────────

#[test]
fn translate_group_by_postgres() {
    let proj = vec![
        Projection::Expr(Expression::Ident("department".into()), None),
        Projection::Expr(
            Expression::Aggregate {
                name: "count".into(),
                distinct: false,
                args: vec![],
            },
            Some("total".into()),
        ),
    ];
    let mut q = make_query(proj, vec![table_source("employees")], None);
    q.group_by = vec![Expression::Ident("department".into())];

    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        r#"SELECT "department", COUNT(*) AS "total" FROM "employees" GROUP BY "department""#
    );
}

#[test]
fn translate_sum_postgres() {
    let proj = vec![
        Projection::Expr(Expression::Ident("department".into()), None),
        Projection::Expr(
            Expression::Aggregate {
                name: "sum".into(),
                distinct: false,
                args: vec![Expression::Ident("salary".into())],
            },
            Some("total_salary".into()),
        ),
    ];
    let mut q = make_query(proj, vec![table_source("employees")], None);
    q.group_by = vec![Expression::Ident("department".into())];

    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        r#"SELECT "department", SUM("salary") AS "total_salary" FROM "employees" GROUP BY "department""#
    );
}

// ── SQL: limit/offset ─────────────────────────────────────────────────────

#[test]
fn translate_limit_postgres() {
    let mut q = make_query(vec![], vec![table_source("users")], None);
    q.limit = Some(10);
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, r#"SELECT * FROM "users" LIMIT 10"#);
}

#[test]
fn translate_limit_offset_postgres() {
    let mut q = make_query(vec![], vec![table_source("users")], None);
    q.limit = Some(10);
    q.offset = Some(5);
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, r#"SELECT * FROM "users" LIMIT 10 OFFSET 5"#);
}

#[test]
fn translate_limit_offset_mysql() {
    let mut q = make_query(vec![], vec![table_source("users")], None);
    q.limit = Some(10);
    q.offset = Some(5);
    let dialect = MySQLDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, "SELECT * FROM `users` LIMIT 5, 10");
}

#[test]
fn translate_limit_offset_mssql() {
    let mut q = make_query(vec![], vec![table_source("users")], None);
    q.limit = Some(10);
    q.offset = Some(5);
    let dialect = MSSQLDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        "SELECT * FROM [users] OFFSET 5 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

// ── SQL: literals ─────────────────────────────────────────────────────────

#[test]
fn translate_literals_postgres() {
    let filter = Expression::BinaryOp {
        op: BinaryOp::And,
        left: Box::new(Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::Ident("name".into())),
            right: Box::new(Expression::String("Alice".into())),
        }),
        right: Box::new(Expression::BinaryOp {
            op: BinaryOp::Gte,
            left: Box::new(Expression::Ident("age".into())),
            right: Box::new(Expression::Integer(30)),
        }),
    };
    let q = make_query(vec![], vec![table_source("users")], Some(filter));
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        r#"SELECT * FROM "users" WHERE (("name" = 'Alice') AND ("age" >= 30))"#
    );
}

#[test]
fn translate_boolean_null_literals() {
    let filter = Expression::BinaryOp {
        op: BinaryOp::And,
        left: Box::new(Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::Ident("active".into())),
            right: Box::new(Expression::Boolean(true)),
        }),
        right: Box::new(Expression::BinaryOp {
            op: BinaryOp::Neq,
            left: Box::new(Expression::Ident("deleted_at".into())),
            right: Box::new(Expression::Null),
        }),
    };
    let q = make_query(vec![], vec![table_source("users")], Some(filter));
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        r#"SELECT * FROM "users" WHERE (("active" = TRUE) AND ("deleted_at" <> NULL))"#
    );
}

#[test]
fn translate_float_literal() {
    let filter = Expression::BinaryOp {
        op: BinaryOp::Gt,
        left: Box::new(Expression::Ident("score".into())),
        right: Box::new(Expression::Number(3.5)),
    };
    let q = make_query(vec![], vec![table_source("games")], Some(filter));
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, r#"SELECT * FROM "games" WHERE ("score" > 3.5)"#);
}

// ── SQL: LIKE / IN ────────────────────────────────────────────────────────

#[test]
fn translate_like_postgres() {
    let filter = Expression::BinaryOp {
        op: BinaryOp::Like,
        left: Box::new(Expression::Ident("name".into())),
        right: Box::new(Expression::String("A%".into())),
    };
    let q = make_query(vec![], vec![table_source("users")], Some(filter));
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        r#"SELECT * FROM "users" WHERE "name" LIKE 'A%'"#
    );
}

#[test]
fn translate_ilike_postgres() {
    let filter = Expression::BinaryOp {
        op: BinaryOp::ILike,
        left: Box::new(Expression::Ident("name".into())),
        right: Box::new(Expression::String("a%".into())),
    };
    let q = make_query(vec![], vec![table_source("users")], Some(filter));
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        r#"SELECT * FROM "users" WHERE "name" ILIKE 'a%'"#
    );
}

#[test]
fn translate_ilike_mysql_fallback() {
    let filter = Expression::BinaryOp {
        op: BinaryOp::ILike,
        left: Box::new(Expression::Ident("name".into())),
        right: Box::new(Expression::String("a%".into())),
    };
    let q = make_query(vec![], vec![table_source("users")], Some(filter));
    let dialect = MySQLDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        "SELECT * FROM `users` WHERE `name` LIKE 'a%'"
    );
}

// ── SQL: order by ─────────────────────────────────────────────────────────

#[test]
fn translate_order_by_postgres() {
    let mut q = make_query(vec![], vec![table_source("users")], None);
    q.order_by = vec![OrderBy {
        expr: Expression::Ident("age".into()),
        direction: OrderDir::Desc,
        nulls: NullsOrder::Default,
    }];
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        r#"SELECT * FROM "users" ORDER BY "age" DESC"#
    );
}

#[test]
fn translate_order_by_with_nulls_postgres() {
    let mut q = make_query(vec![], vec![table_source("users")], None);
    q.order_by = vec![OrderBy {
        expr: Expression::Ident("deleted_at".into()),
        direction: OrderDir::Asc,
        nulls: NullsOrder::First,
    }];
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        r#"SELECT * FROM "users" ORDER BY "deleted_at" ASC NULLS FIRST"#
    );
}

// ── SQL: distinct ─────────────────────────────────────────────────────────

#[test]
fn translate_distinct_mysql() {
    let mut q = make_query(
        vec![Projection::Expr(
            Expression::Ident("status".into()),
            None,
        )],
        vec![table_source("users")],
        None,
    );
    q.distinct = true;
    let dialect = MySQLDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(sql, "SELECT DISTINCT `status` FROM `users`");
}

// ── SQL: joins ────────────────────────────────────────────────────────────

#[test]
fn translate_inner_join_postgres() {
    let mut q = make_query(vec![], vec![table_source("users")], None);
    q.joins = vec![Join {
        kind: JoinKind::Inner,
        source: table_source("orders"),
        alias: None,
        condition: Some(Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::QualifiedIdent {
                table: "users".into(),
                field: "id".into(),
            }),
            right: Box::new(Expression::QualifiedIdent {
                table: "orders".into(),
                field: "user_id".into(),
            }),
        }),
    }];
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        r#"SELECT * FROM "users" INNER JOIN "orders" ON ("users"."id" = "orders"."user_id")"#
    );
}

#[test]
fn translate_left_join_mysql() {
    let mut q = make_query(vec![], vec![table_source("customers")], None);
    q.joins = vec![Join {
        kind: JoinKind::Left,
        source: table_source("orders"),
        alias: None,
        condition: Some(Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::QualifiedIdent {
                table: "customers".into(),
                field: "id".into(),
            }),
            right: Box::new(Expression::QualifiedIdent {
                table: "orders".into(),
                field: "cust_id".into(),
            }),
        }),
    }];
    let dialect = MySQLDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        "SELECT * FROM `customers` LEFT JOIN `orders` ON (`customers`.`id` = `orders`.`cust_id`)"
    );
}

// ── SQL: expression literals ──────────────────────────────────────────────

#[test]
fn translate_string_with_quote() {
    let filter = Expression::BinaryOp {
        op: BinaryOp::Eq,
        left: Box::new(Expression::Ident("name".into())),
        right: Box::new(Expression::String("O'Brien".into())),
    };
    let q = make_query(vec![], vec![table_source("users")], Some(filter));
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        r#"SELECT * FROM "users" WHERE ("name" = 'O''Brien')"#
    );
}

// ── MongoDB translator ────────────────────────────────────────────────────

#[test]
fn translate_find_users_mongo() {
    let q = make_query(vec![], vec![table_source("users")], None);
    let json = translate_query_mongo(&q, "mydb");
    assert_eq!(json["database"], "mydb");
    assert_eq!(json["collection"], "users");
    let pipeline = json["pipeline"].as_array().unwrap();
    assert!(pipeline.is_empty());
}

#[test]
fn translate_filter_gt_mongo() {
    let filter = Expression::BinaryOp {
        op: BinaryOp::Gt,
        left: Box::new(Expression::Ident("age".into())),
        right: Box::new(Expression::Integer(21)),
    };
    let q = make_query(vec![], vec![table_source("users")], Some(filter));
    let json = translate_query_mongo(&q, "mydb");
    let pipeline = json["pipeline"].as_array().unwrap();
    assert_eq!(pipeline.len(), 1);
    assert_eq!(
        pipeline[0]["$match"]["age"]["$gt"],
        serde_json::Value::Number(serde_json::Number::from(21))
    );
}

#[test]
fn translate_aggregate_mongo() {
    let proj = vec![
        Projection::Expr(Expression::Ident("department".into()), None),
        Projection::Expr(
            Expression::Aggregate {
                name: "count".into(),
                distinct: false,
                args: vec![],
            },
            Some("total".into()),
        ),
    ];
    let mut q = make_query(proj, vec![table_source("employees")], None);
    q.group_by = vec![Expression::Ident("department".into())];
    let json = translate_query_mongo(&q, "mydb");
    let pipeline = json["pipeline"].as_array().unwrap();
    assert_eq!(pipeline.len(), 1);
    assert!(pipeline[0]["$group"].is_object());
    assert_eq!(
        pipeline[0]["$group"]["_id"],
        serde_json::Value::String("$department".into())
    );
    assert_eq!(
        pipeline[0]["$group"]["total"]["$count"],
        serde_json::Value::String("count".into())
    );
}

#[test]
fn translate_limit_skip_mongo() {
    let mut q = make_query(vec![], vec![table_source("users")], None);
    q.limit = Some(10);
    q.offset = Some(5);
    let json = translate_query_mongo(&q, "mydb");
    let pipeline = json["pipeline"].as_array().unwrap();

    let has_limit = pipeline.iter().any(|s| s["$limit"].is_number());
    let has_skip = pipeline.iter().any(|s| s["$skip"].is_number());
    assert!(has_limit, "pipeline should contain $limit");
    assert!(has_skip, "pipeline should contain $skip");
}

#[test]
fn translate_and_filter_mongo() {
    let filter = Expression::BinaryOp {
        op: BinaryOp::And,
        left: Box::new(Expression::BinaryOp {
            op: BinaryOp::Gt,
            left: Box::new(Expression::Ident("age".into())),
            right: Box::new(Expression::Integer(21)),
        }),
        right: Box::new(Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::Ident("status".into())),
            right: Box::new(Expression::String("active".into())),
        }),
    };
    let q = make_query(vec![], vec![table_source("users")], Some(filter));
    let json = translate_query_mongo(&q, "mydb");
    let pipeline = json["pipeline"].as_array().unwrap();
    assert_eq!(pipeline.len(), 1);
    let match_stage = &pipeline[0]["$match"];
    assert!(match_stage["$and"].is_array());
}

// ── Dialect quoting ───────────────────────────────────────────────────────

#[test]
fn quote_ident_postgres() {
    assert_eq!(PostgresDialect.quote_ident("my_table"), r#""my_table""#);
}

#[test]
fn quote_ident_mysql() {
    assert_eq!(MySQLDialect.quote_ident("my_table"), "`my_table`");
}

#[test]
fn quote_ident_mssql() {
    assert_eq!(MSSQLDialect.quote_ident("my_table"), "[my_table]");
}

#[test]
fn quote_ident_with_special_chars() {
    assert_eq!(
        PostgresDialect.quote_ident(r#"weird"name"#),
        r#""weird""name""#
    );
}

// ── BETWEEN ───────────────────────────────────────────────────────────────

#[test]
fn translate_between_postgres() {
    let filter = Expression::Between {
        expr: Box::new(Expression::Ident("age".into())),
        low: Box::new(Expression::Integer(18)),
        high: Box::new(Expression::Integer(65)),
    };
    let q = make_query(vec![], vec![table_source("users")], Some(filter));
    let dialect = PostgresDialect;
    let sql = translate_query(&q, &dialect);
    assert_eq!(
        sql,
        r#"SELECT * FROM "users" WHERE "age" BETWEEN 18 AND 65"#
    );
}

// ── INSERT / UPDATE / DELETE ──────────────────────────────────────────────

#[test]
fn translate_insert_postgres() {
    let insert = Insert {
        table: "users".into(),
        connection: None,
        columns: Some(vec!["name".into(), "age".into()]),
        rows: vec![vec![
            ("name".into(), Expression::String("Alice".into())),
            ("age".into(), Expression::Integer(30)),
        ]],
        query: None,
    };
    let sql = translate_statement_sql(&Statement::Insert(insert), &PostgresDialect);
    assert_eq!(
        sql,
        r#"INSERT INTO "users" ("name", "age") VALUES ('Alice', 30)"#
    );
}

#[test]
fn translate_update_mysql() {
    let update = Update {
        table: "users".into(),
        connection: None,
        assignments: vec![
            ("status".into(), Expression::String("inactive".into())),
        ],
        filter: Some(Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::Ident("id".into())),
            right: Box::new(Expression::Integer(42)),
        }),
    };
    let sql = translate_statement_sql(&Statement::Update(update), &MySQLDialect);
    assert_eq!(
        sql,
        "UPDATE `users` SET `status` = 'inactive' WHERE (`id` = 42)"
    );
}

#[test]
fn translate_delete_mssql() {
    let delete = Delete {
        table: "users".into(),
        connection: None,
        filter: Some(Expression::BinaryOp {
            op: BinaryOp::Lt,
            left: Box::new(Expression::Ident("age".into())),
            right: Box::new(Expression::Integer(18)),
        }),
    };
    let sql = translate_statement_sql(&Statement::Delete(delete), &MSSQLDialect);
    assert_eq!(
        sql,
        "DELETE FROM [users] WHERE ([age] < 18)"
    );
}

// ── CAST ──────────────────────────────────────────────────────────────────

#[test]
fn translate_cast_postgres() {
    let expr = Expression::Cast {
        expr: Box::new(Expression::String("42".into())),
        target: DataType::Integer,
    };
    let dialect = PostgresDialect;
    let sql = translate_expr(&expr, &dialect);
    assert_eq!(sql, "CAST('42' AS INTEGER)");
}

// ── CASE ──────────────────────────────────────────────────────────────────

#[test]
fn translate_case_postgres() {
    let expr = Expression::Case {
        expr: None,
        whens: vec![
            (
                Expression::BinaryOp {
                    op: BinaryOp::Gt,
                    left: Box::new(Expression::Ident("score".into())),
                    right: Box::new(Expression::Integer(90)),
                },
                Expression::String("A".into()),
            ),
        ],
        else_expr: Some(Box::new(Expression::String("B".into()))),
    };
    let dialect = PostgresDialect;
    let sql = translate_expr(&expr, &dialect);
    assert!(sql.starts_with("CASE WHEN"), "got: {sql}");
    assert!(sql.contains("THEN 'A'"), "got: {sql}");
    assert!(sql.contains("ELSE 'B'"), "got: {sql}");
    assert!(sql.ends_with("END"), "got: {sql}");
}
