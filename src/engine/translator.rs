#![allow(dead_code)]

use serde_json::json;
use serde_json::Value as JsonValue;

use crate::adapters::Value;
use crate::connection::DatabaseKind;
use crate::lang::ast::*;

pub trait SqlDialect {
    fn quote_ident(&self, name: &str) -> String;
    fn limit_offset(&self, limit: Option<u64>, offset: Option<u64>) -> String;
    fn kind(&self) -> DatabaseKind;
}

pub struct PostgresDialect;
pub struct MySQLDialect;
pub struct MSSQLDialect;
pub struct SQLiteDialect;

impl SqlDialect for PostgresDialect {
    fn quote_ident(&self, name: &str) -> String {
        format!("\"{}\"", name.replace('"', "\"\""))
    }

    fn limit_offset(&self, limit: Option<u64>, offset: Option<u64>) -> String {
        match (limit, offset) {
            (Some(l), Some(o)) => format!("LIMIT {} OFFSET {}", l, o),
            (Some(l), None) => format!("LIMIT {}", l),
            (None, Some(o)) => format!("OFFSET {}", o),
            (None, None) => String::new(),
        }
    }

    fn kind(&self) -> DatabaseKind {
        DatabaseKind::Postgres
    }
}

impl SqlDialect for MySQLDialect {
    fn quote_ident(&self, name: &str) -> String {
        format!("`{}`", name.replace('`', "``"))
    }

    fn limit_offset(&self, limit: Option<u64>, offset: Option<u64>) -> String {
        match (limit, offset) {
            (Some(l), Some(o)) => format!("LIMIT {}, {}", o, l),
            (Some(l), None) => format!("LIMIT {}", l),
            (None, _) => String::new(),
        }
    }

    fn kind(&self) -> DatabaseKind {
        DatabaseKind::MySQL
    }
}

impl SqlDialect for MSSQLDialect {
    fn quote_ident(&self, name: &str) -> String {
        format!("[{}]", name.replace(']', "]]"))
    }

    fn limit_offset(&self, limit: Option<u64>, offset: Option<u64>) -> String {
        match (limit, offset) {
            (Some(l), Some(o)) => {
                format!("OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", o, l)
            }
            (Some(l), None) => {
                format!("OFFSET 0 ROWS FETCH NEXT {} ROWS ONLY", l)
            }
            (None, Some(o)) => {
                format!("OFFSET {} ROWS", o)
            }
            (None, None) => String::new(),
        }
    }

    fn kind(&self) -> DatabaseKind {
        DatabaseKind::MSSQL
    }
}

impl SqlDialect for SQLiteDialect {
    fn quote_ident(&self, name: &str) -> String {
        format!("\"{}\"", name.replace('"', "\"\""))
    }

    fn limit_offset(&self, limit: Option<u64>, offset: Option<u64>) -> String {
        match (limit, offset) {
            (Some(l), Some(o)) => format!("LIMIT {} OFFSET {}", l, o),
            (Some(l), None) => format!("LIMIT {}", l),
            (None, Some(o)) => format!("OFFSET {}", o),
            (None, None) => String::new(),
        }
    }

    fn kind(&self) -> DatabaseKind {
        DatabaseKind::SQLite
    }
}

fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
}

fn translate_literal(expr: &Expression, _dialect: &dyn SqlDialect) -> String {
    match expr {
        Expression::String(s) => format!("'{}'", escape_sql_string(s)),
        Expression::Number(n) => {
            if n.is_nan() {
                "NULL".to_string()
            } else if n.is_infinite() {
                if n.is_sign_positive() {
                    "'Infinity'".to_string()
                } else {
                    "'-Infinity'".to_string()
                }
            } else {
                n.to_string()
            }
        }
        Expression::Integer(i) => i.to_string(),
        Expression::Boolean(true) => "TRUE".to_string(),
        Expression::Boolean(false) => "FALSE".to_string(),
        Expression::Null => "NULL".to_string(),
        Expression::Array(items) => {
            let inner: Vec<String> = items
                .iter()
                .map(|e| translate_expr(e, _dialect))
                .collect();
            format!("ARRAY[{}]", inner.join(", "))
        }
        Expression::Object(pairs) => {
            let inner: Vec<String> = pairs
                .iter()
                .map(|(k, v)| format!("'{}', {}", escape_sql_string(k), translate_expr(v, _dialect)))
                .collect();
            format!("JSON_OBJECT({})", inner.join(", "))
        }
        Expression::Ident(name) => _dialect.quote_ident(name),
        Expression::QualifiedIdent { table, field } => {
            format!(
                "{}.{}",
                _dialect.quote_ident(table),
                _dialect.quote_ident(field)
            )
        }
        Expression::QualifiedWildcard(table) => format!("{}.*", _dialect.quote_ident(table)),
        Expression::BinaryOp { op, left, right } => {
            translate_binary_sql(op, left, right, _dialect)
        }
        Expression::UnaryOp { op, expr } => match op {
            UnaryOp::Not => format!("NOT ({})", translate_expr(expr, _dialect)),
            UnaryOp::Neg => format!("-({})", translate_expr(expr, _dialect)),
        },
        Expression::FnCall { name, args } => {
            let fn_name = match name.to_lowercase().as_str() {
                "now" => "NOW()",
                _ => {
                    let args_str: Vec<String> =
                        args.iter().map(|a| translate_expr(a, _dialect)).collect();
                    return format!("{}({})", name.to_uppercase(), args_str.join(", "));
                }
            };
            fn_name.to_string()
        }
        Expression::Aggregate {
            name,
            distinct,
            args,
        } => {
            let distinct_str = if *distinct { "DISTINCT " } else { "" };
            if args.is_empty() {
                format!("{}({}*)", name.to_uppercase(), distinct_str)
            } else {
                let args_str: Vec<String> =
                    args.iter().map(|a| translate_expr(a, _dialect)).collect();
                format!("{}({}{})", name.to_uppercase(), distinct_str, args_str.join(", "))
            }
        }
        Expression::WindowFn { .. } => translate_window_fn_sql(expr, _dialect),
        Expression::Case {
            expr: case_expr,
            whens,
            else_expr,
        } => translate_case_sql(case_expr, whens, else_expr, _dialect),
        Expression::Between { expr, low, high } => {
            format!(
                "{} BETWEEN {} AND {}",
                translate_expr(expr, _dialect),
                translate_expr(low, _dialect),
                translate_expr(high, _dialect),
            )
        }
        Expression::Subquery(query) => {
            format!("({})", translate_query(query, _dialect))
        }
        Expression::Exists(query, is_exists) => {
            if *is_exists {
                format!("EXISTS ({})", translate_query(query, _dialect))
            } else {
                format!("NOT EXISTS ({})", translate_query(query, _dialect))
            }
        }
        Expression::QuantifiedCmp {
            op,
            left,
            quant,
            subquery,
        } => {
            let quant_str = match quant {
                Quantifier::All => "ALL",
                Quantifier::Any => "ANY",
                Quantifier::Some => "SOME",
            };
            format!(
                "{} {} {} ({})",
                translate_expr(left, _dialect),
                op_to_sql(op),
                quant_str,
                translate_query(subquery, _dialect),
            )
        }
        Expression::Cast { expr, target } => {
            format!(
                "CAST({} AS {})",
                translate_expr(expr, _dialect),
                data_type_to_sql(target),
            )
        }
        Expression::NamedParam(name) => match _dialect.kind() {
            DatabaseKind::Postgres => format!("${}", name),
            DatabaseKind::MySQL => "?".to_string(),
            DatabaseKind::MSSQL => format!("@{}", name),
            DatabaseKind::SQLite => "?".to_string(),
            DatabaseKind::MongoDB => format!(":{}", name),
        },
        Expression::Interval { value, unit } => {
            let unit_str = interval_unit_to_sql(unit, _dialect);
            match _dialect.kind() {
                DatabaseKind::Postgres => {
                    format!("INTERVAL '{} {}'", value, unit_str)
                }
                DatabaseKind::MySQL => {
                    format!("INTERVAL {} {}", value, unit_str)
                }
                _ => format!("INTERVAL {} {}", value, unit_str),
            }
        }
    }
}

fn translate_binary_sql(
    op: &BinaryOp,
    left: &Expression,
    right: &Expression,
    dialect: &dyn SqlDialect,
) -> String {
    let left_s = translate_expr(left, dialect);
    let right_s = translate_expr(right, dialect);

    match op {
        BinaryOp::In => format!("{} IN ({})", left_s, right_s),
        BinaryOp::NotIn => format!("{} NOT IN ({})", left_s, right_s),
        BinaryOp::Like => format!("{} LIKE {}", left_s, right_s),
        BinaryOp::ILike => {
            if dialect.kind() == DatabaseKind::Postgres {
                format!("{} ILIKE {}", left_s, right_s)
            } else {
                format!("{} LIKE {}", left_s, right_s)
            }
        }
        _ => {
            let op_str = op_to_sql(op);
            format!("({} {} {})", left_s, op_str, right_s)
        }
    }
}

fn op_to_sql(op: &BinaryOp) -> &str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::Eq => "=",
        BinaryOp::Neq => "<>",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::Lte => "<=",
        BinaryOp::Gte => ">=",
        BinaryOp::And => "AND",
        BinaryOp::Or => "OR",
        BinaryOp::Like => "LIKE",
        BinaryOp::ILike => "ILIKE",
        BinaryOp::In => "IN",
        BinaryOp::NotIn => "NOT IN",
        BinaryOp::Concat => "||",
    }
}

fn data_type_to_sql(dt: &DataType) -> &str {
    match dt {
        DataType::String => "VARCHAR",
        DataType::Integer => "INTEGER",
        DataType::Float => "FLOAT",
        DataType::Boolean => "BOOLEAN",
        DataType::DateTime => "TIMESTAMP",
        DataType::Json => "JSON",
    }
}

fn interval_unit_to_sql(unit: &IntervalUnit, _dialect: &dyn SqlDialect) -> String {
    unit.to_string()
}

fn translate_case_sql(
    case_expr: &Option<Box<Expression>>,
    whens: &[(Expression, Expression)],
    else_expr: &Option<Box<Expression>>,
    dialect: &dyn SqlDialect,
) -> String {
    let mut parts = vec!["CASE".to_string()];

    if let Some(expr) = case_expr {
        parts.push(translate_expr(expr, dialect));
    }

    for (when, then) in whens {
        parts.push(format!(
            "WHEN {} THEN {}",
            translate_expr(when, dialect),
            translate_expr(then, dialect),
        ));
    }

    if let Some(else_e) = else_expr {
        parts.push(format!("ELSE {}", translate_expr(else_e, dialect)));
    }

    parts.push("END".to_string());
    parts.join(" ")
}

fn translate_window_fn_sql(expr: &Expression, dialect: &dyn SqlDialect) -> String {
    if let Expression::WindowFn {
        func,
        over,
        window_name,
    } = expr
    {
        let func_str = match func {
            WindowFunction::RowNumber => "ROW_NUMBER()".to_string(),
            WindowFunction::Rank => "RANK()".to_string(),
            WindowFunction::DenseRank => "DENSE_RANK()".to_string(),
            WindowFunction::Lag(e, default) => {
                if let Some(d) = default {
                    format!("LAG({}, {})", translate_expr(e, dialect), d)
                } else {
                    format!("LAG({})", translate_expr(e, dialect))
                }
            }
            WindowFunction::Lead(e, default) => {
                if let Some(d) = default {
                    format!("LEAD({}, {})", translate_expr(e, dialect), d)
                } else {
                    format!("LEAD({})", translate_expr(e, dialect))
                }
            }
            WindowFunction::FirstValue(e) => {
                format!("FIRST_VALUE({})", translate_expr(e, dialect))
            }
            WindowFunction::LastValue(e) => {
                format!("LAST_VALUE({})", translate_expr(e, dialect))
            }
            WindowFunction::NthValue(e, n) => {
                format!("NTH_VALUE({}, {})", translate_expr(e, dialect), n)
            }
            WindowFunction::Expr(e) => translate_expr(e, dialect),
        };

        if let Some(name) = window_name {
            return format!("{} OVER {}", func_str, dialect.quote_ident(name));
        }

        let mut over_parts = Vec::new();
        if !over.partition_by.is_empty() {
            let parts: Vec<String> = over
                .partition_by
                .iter()
                .map(|e| translate_expr(e, dialect))
                .collect();
            over_parts.push(format!("PARTITION BY {}", parts.join(", ")));
        }
        if !over.order_by.is_empty() {
            let parts: Vec<String> = over
                .order_by
                .iter()
                .map(|o| translate_order_by_sql(o, dialect))
                .collect();
            over_parts.push(format!("ORDER BY {}", parts.join(", ")));
        }

        if over_parts.is_empty() {
            format!("{} OVER ()", func_str)
        } else {
            format!("{} OVER ({})", func_str, over_parts.join(" "))
        }
    } else {
        translate_expr(expr, dialect)
    }
}

fn translate_order_by_sql(order: &OrderBy, dialect: &dyn SqlDialect) -> String {
    let dir = match order.direction {
        OrderDir::Asc => "ASC",
        OrderDir::Desc => "DESC",
    };
    let nulls = match order.nulls {
        NullsOrder::Default => String::new(),
        NullsOrder::First => " NULLS FIRST".to_string(),
        NullsOrder::Last => " NULLS LAST".to_string(),
    };
    format!("{}{}{}", translate_expr(&order.expr, dialect), format!(" {}", dir), nulls)
}

fn translate_projection_sql(proj: &Projection, dialect: &dyn SqlDialect) -> String {
    match proj {
        Projection::Wildcard => "*".to_string(),
        Projection::QualifiedWildcard(table) => {
            format!("{}.*", dialect.quote_ident(table))
        }
        Projection::Expr(expr, alias) => {
            let expr_str = translate_expr(expr, dialect);
            if let Some(alias_name) = alias {
                format!("{} AS {}", expr_str, dialect.quote_ident(alias_name))
            } else {
                expr_str
            }
        }
    }
}

pub fn translate_expr(expr: &Expression, dialect: &dyn SqlDialect) -> String {
    translate_literal(expr, dialect)
}

pub fn translate_query(query: &Query, dialect: &dyn SqlDialect) -> String {
    let distinct = if query.distinct { "DISTINCT " } else { "" };

    let projection = if query.projection.is_empty() {
        "*".to_string()
    } else {
        query
            .projection
            .iter()
            .map(|p| translate_projection_sql(p, dialect))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut from_parts = Vec::new();

    for source in &query.sources {
        from_parts.push(translate_source_sql(source, dialect));
    }

    for join in &query.joins {
        let join_str = match join.kind {
            JoinKind::Inner => "INNER JOIN",
            JoinKind::Left => "LEFT JOIN",
            JoinKind::Right => "RIGHT JOIN",
            JoinKind::Full => "FULL JOIN",
            JoinKind::Cross => "CROSS JOIN",
        };
        let source_str = translate_source_sql(&join.source, dialect);
        let on_str = if let Some(cond) = &join.condition {
            format!(" ON {}", translate_expr(cond, dialect))
        } else {
            String::new()
        };
        from_parts.push(format!("{} {}{}", join_str, source_str, on_str));
    }

    let mut query_str = format!(
        "SELECT {}{} FROM {}",
        distinct, projection, from_parts.join(" ")
    );

    if let Some(filter) = &query.filter {
        query_str.push_str(&format!(" WHERE {}", translate_expr(filter, dialect)));
    }

    if !query.group_by.is_empty() {
        let groups: Vec<String> = query
            .group_by
            .iter()
            .map(|e| translate_expr(e, dialect))
            .collect();
        query_str.push_str(&format!(" GROUP BY {}", groups.join(", ")));
    }

    if let Some(having) = &query.having {
        query_str.push_str(&format!(" HAVING {}", translate_expr(having, dialect)));
    }

    if !query.order_by.is_empty() {
        let orders: Vec<String> = query
            .order_by
            .iter()
            .map(|o| translate_order_by_sql(o, dialect))
            .collect();
        query_str.push_str(&format!(" ORDER BY {}", orders.join(", ")));
    }

    let limit_offset = dialect.limit_offset(query.limit, query.offset);
    if !limit_offset.is_empty() {
        query_str.push(' ');
        query_str.push_str(&limit_offset);
    }

    query_str
}

fn translate_source_sql(source: &Source, dialect: &dyn SqlDialect) -> String {
    let name = match &source.kind {
        SourceKind::Table(t) => dialect.quote_ident(t),
        SourceKind::Subquery(q) => format!("({})", translate_query(q, dialect)),
        SourceKind::CteRef(name) => dialect.quote_ident(name),
    };

    if let Some(alias) = &source.alias {
        if alias != &source.name {
            format!("{} AS {}", name, dialect.quote_ident(alias))
        } else {
            name
        }
    } else {
        name
    }
}

pub fn translate_statement_sql(stmt: &Statement, dialect: &dyn SqlDialect) -> String {
    match stmt {
        Statement::Query(q) => translate_query(q, dialect),
        Statement::Insert(insert) => translate_insert_sql(insert, dialect),
        Statement::Update(update) => translate_update_sql(update, dialect),
        Statement::Delete(delete) => translate_delete_sql(delete, dialect),
        Statement::Noop => String::new(),
        _ => format!("-- unsupported statement type"),
    }
}

fn translate_insert_sql(insert: &Insert, dialect: &dyn SqlDialect) -> String {
    let table = dialect.quote_ident(&insert.table);

    if let Some(query) = &insert.query {
        return format!("INSERT INTO {} {}", table, translate_query(query, dialect));
    }

    if insert.rows.is_empty() {
        return format!("INSERT INTO {} DEFAULT VALUES", table);
    }

    let columns = match &insert.columns {
        Some(cols) => {
            let quoted: Vec<String> = cols.iter().map(|c| dialect.quote_ident(c)).collect();
            format!("({})", quoted.join(", "))
        }
        None => {
            if let Some(first) = insert.rows.first() {
                let cols: Vec<String> = first.iter().map(|(col, _)| dialect.quote_ident(col)).collect();
                format!("({})", cols.join(", "))
            } else {
                String::new()
            }
        }
    };

    let values: Vec<String> = insert
        .rows
        .iter()
        .map(|row| {
            let vals: Vec<String> = row.iter().map(|(_, val)| translate_expr(val, dialect)).collect();
            format!("({})", vals.join(", "))
        })
        .collect();

    format!("INSERT INTO {} {} VALUES {}", table, columns, values.join(", "))
}

fn translate_update_sql(update: &Update, dialect: &dyn SqlDialect) -> String {
    let table = dialect.quote_ident(&update.table);

    let sets: Vec<String> = update
        .assignments
        .iter()
        .map(|(col, val)| {
            format!("{} = {}", dialect.quote_ident(col), translate_expr(val, dialect))
        })
        .collect();

    let mut query_str = format!("UPDATE {} SET {}", table, sets.join(", "));

    if let Some(filter) = &update.filter {
        query_str.push_str(&format!(" WHERE {}", translate_expr(filter, dialect)));
    }

    query_str
}

fn translate_delete_sql(delete: &Delete, dialect: &dyn SqlDialect) -> String {
    let table = dialect.quote_ident(&delete.table);
    let mut query_str = format!("DELETE FROM {}", table);

    if let Some(filter) = &delete.filter {
        query_str.push_str(&format!(" WHERE {}", translate_expr(filter, dialect)));
    }

    query_str
}

// ── MongoDB translator ──────────────────────────────────────────────────────

fn translate_expr_mongo(expr: &Expression) -> JsonValue {
    match expr {
        Expression::String(s) => JsonValue::String(s.clone()),
        Expression::Number(n) => {
            let val = *n;
            if val.fract() == 0.0
                && val >= (i64::MIN as f64)
                && val <= (i64::MAX as f64)
            {
                json!(val as i64)
            } else {
                json!(val)
            }
        }
        Expression::Integer(i) => json!(i),
        Expression::Boolean(b) => JsonValue::Bool(*b),
        Expression::Null => JsonValue::Null,
        Expression::Array(items) => {
            JsonValue::Array(items.iter().map(translate_expr_mongo).collect())
        }
        Expression::Object(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, v) in pairs {
                map.insert(k.clone(), translate_expr_mongo(v));
            }
            JsonValue::Object(map)
        }
        Expression::Ident(name) => {
            if name == "_id" {
                json!(name)
            } else {
                json!(format!("${}", name))
            }
        }
        Expression::QualifiedIdent { table: _, field } => {
            json!(format!("${}", field))
        }
        Expression::QualifiedWildcard(_) => json!(1),
        Expression::BinaryOp { op, left, right } => {
            translate_binary_mongo(op, left, right)
        }
        Expression::UnaryOp { op, expr: inner } => match op {
            UnaryOp::Not => {
                json!({"$not": translate_expr_mongo(inner)})
            }
            UnaryOp::Neg => {
                json!({"$multiply": [translate_expr_mongo(inner), -1]})
            }
        },
        Expression::FnCall { name, args } => {
            let fn_name = name.to_lowercase();
            match fn_name.as_str() {
                "now" => JsonValue::String("$$NOW".to_string()),
                _ => {
                    let mongo_args: Vec<JsonValue> =
                        args.iter().map(translate_expr_mongo).collect();
                    json!({format!("${}", fn_name): mongo_args})
                }
            }
        }
        Expression::Aggregate {
            name,
            distinct: _,
            args,
        } => {
            let agg_name = name.to_lowercase();
            match (agg_name.as_str(), args.len()) {
                ("count", 0) => json!({"$count": "count"}),
                ("count", _) => {
                    let field = translate_expr_mongo(&args[0]);
                    json!({"$sum": 1, "_field": field})
                }
                ("sum", 1) => {
                    json!({"$sum": translate_expr_mongo(&args[0])})
                }
                ("avg", 1) => {
                    json!({"$avg": translate_expr_mongo(&args[0])})
                }
                ("min", 1) => {
                    json!({"$min": translate_expr_mongo(&args[0])})
                }
                ("max", 1) => {
                    json!({"$max": translate_expr_mongo(&args[0])})
                }
                _ => {
                    let mongo_args: Vec<JsonValue> =
                        args.iter().map(translate_expr_mongo).collect();
                    json!({format!("${}", agg_name): mongo_args})
                }
            }
        }
        Expression::WindowFn { .. } => {
            json!({"$error": "window functions unsupported in MongoDB"})
        }
        Expression::Case {
            expr: case_expr,
            whens,
            else_expr,
        } => translate_case_mongo(case_expr, whens, else_expr),
        Expression::Between { expr, low, high } => {
            let field = translate_expr_mongo(expr);
            let low_val = translate_expr_mongo(low);
            let high_val = translate_expr_mongo(high);
            json!({"$and": [
                {field.as_str().unwrap_or(""): {"$gte": low_val}},
                {field.as_str().unwrap_or(""): {"$lte": high_val}}
            ]})
        }
        Expression::Subquery(_) => {
            json!({"$error": "subqueries unsupported in MongoDB"})
        }
        Expression::Exists(_, _) => {
            json!({"$error": "exists unsupported in MongoDB"})
        }
        Expression::QuantifiedCmp { .. } => {
            json!({"$error": "quantified comparison unsupported in MongoDB"})
        }
        Expression::Cast { expr: inner, target: _ } => {
            translate_expr_mongo(inner)
        }
        Expression::NamedParam(name) => json!(format!(":{}", name)),
        Expression::Interval { value, unit } => {
            let unit_str = unit.to_string();
            json!({"$interval": {"value": value, "unit": unit_str}})
        }
    }
}

fn translate_binary_mongo(
    op: &BinaryOp,
    left: &Expression,
    right: &Expression,
) -> JsonValue {
    if let Expression::Ident(field_name) = left {
        let field = field_name.as_str();
        let right_val = translate_expr_mongo(right);

        match op {
            BinaryOp::Eq => json!({field: right_val}),
            BinaryOp::Neq => json!({field: {"$ne": right_val}}),
            BinaryOp::Gt => json!({field: {"$gt": right_val}}),
            BinaryOp::Gte => json!({field: {"$gte": right_val}}),
            BinaryOp::Lt => json!({field: {"$lt": right_val}}),
            BinaryOp::Lte => json!({field: {"$lte": right_val}}),
            BinaryOp::Like | BinaryOp::ILike => {
                let pattern = match right {
                    Expression::String(s) => s.replace("%", ".*").replace('_', "."),
                    _ => format!("{}", right_val),
                };
                json!({field: {"$regex": pattern}})
            }
            BinaryOp::In => json!({field: {"$in": right_val}}),
            BinaryOp::NotIn => json!({field: {"$nin": right_val}}),
            BinaryOp::And | BinaryOp::Or | BinaryOp::Add | BinaryOp::Sub
            | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod | BinaryOp::Concat => {
                let left_val = translate_expr_mongo(left);
                translate_binary_mongo_generic(op, &left_val, &right_val)
            }
        }
    } else {
        let left_val = translate_expr_mongo(left);
        let right_val = translate_expr_mongo(right);
        translate_binary_mongo_generic(op, &left_val, &right_val)
    }
}

fn translate_binary_mongo_generic(
    op: &BinaryOp,
    left: &JsonValue,
    right: &JsonValue,
) -> JsonValue {
    match op {
        BinaryOp::And => json!({"$and": [left, right]}),
        BinaryOp::Or => json!({"$or": [left, right]}),
        BinaryOp::Eq => json!({"$eq": [left, right]}),
        BinaryOp::Neq => json!({"$ne": [left, right]}),
        BinaryOp::Gt => json!({"$gt": [left, right]}),
        BinaryOp::Gte => json!({"$gte": [left, right]}),
        BinaryOp::Lt => json!({"$lt": [left, right]}),
        BinaryOp::Lte => json!({"$lte": [left, right]}),
        BinaryOp::Add => json!({"$add": [left, right]}),
        BinaryOp::Sub => json!({"$subtract": [left, right]}),
        BinaryOp::Mul => json!({"$multiply": [left, right]}),
        BinaryOp::Div => json!({"$divide": [left, right]}),
        BinaryOp::Mod => json!({"$mod": [left, right]}),
        BinaryOp::Like | BinaryOp::ILike => {
            json!({"$regexMatch": {"input": left, "regex": right}})
        }
        BinaryOp::In => json!({"$in": [left, right]}),
        BinaryOp::NotIn => json!({"$nin": [left, right]}),
        BinaryOp::Concat => json!({"$concat": [left, right]}),
    }
}

fn translate_case_mongo(
    case_expr: &Option<Box<Expression>>,
    whens: &[(Expression, Expression)],
    else_expr: &Option<Box<Expression>>,
) -> JsonValue {
    let mut branches = Vec::new();

    if let Some(expr) = case_expr {
        for (when, then) in whens {
            branches.push(json!({
                "case": {"$eq": [translate_expr_mongo(expr), translate_expr_mongo(when)]},
                "then": translate_expr_mongo(then),
            }));
        }
    } else {
        for (when, then) in whens {
            branches.push(json!({
                "case": translate_expr_mongo(when),
                "then": translate_expr_mongo(then),
            }));
        }
    }

    let default = match else_expr {
        Some(e) => translate_expr_mongo(e),
        None => JsonValue::Null,
    };

    json!({"$switch": {"branches": branches, "default": default}})
}

fn build_filter_mongo(filter: Option<&Expression>) -> JsonValue {
    match filter {
        Some(expr) => match expr {
            Expression::Boolean(true) => json!({}),
            _ => translate_expr_mongo(expr),
        },
        None => json!({}),
    }
}

pub fn translate_query_mongo(query: &Query, database: &str) -> JsonValue {
    let source_name = query
        .sources
        .first()
        .map(|s| match &s.kind {
            SourceKind::Table(name) => name.clone(),
            _ => s.name.clone(),
        })
        .unwrap_or_else(|| "unknown".to_string());

    let mut pipeline = Vec::new();

    let filter = build_filter_mongo(query.filter.as_ref());
    if filter != json!({}) {
        pipeline.push(json!({"$match": filter}));
    }

    if !query.group_by.is_empty() || !query.projection.is_empty() {
        let has_aggs = query
            .projection
            .iter()
            .any(|p| matches!(p, Projection::Expr(Expression::Aggregate { .. }, _)));

        if has_aggs {
            let mut group_spec = serde_json::Map::new();

            if query.group_by.is_empty() {
                group_spec.insert("_id".to_string(), JsonValue::Null);
            } else {
                let group_keys: Vec<JsonValue> = query
                    .group_by
                    .iter()
                    .map(|e| translate_expr_mongo(e))
                    .collect();
                if group_keys.len() == 1 {
                    group_spec.insert("_id".to_string(), group_keys.into_iter().next().unwrap());
                } else {
                    group_spec.insert(
                        "_id".to_string(),
                        JsonValue::Array(group_keys),
                    );
                }
            }

            for proj in &query.projection {
                if let Projection::Expr(Expression::Aggregate { name, distinct: _, args }, alias) = proj {
                    let key = alias.clone().unwrap_or_else(|| name.clone());
                    let agg_val = translate_expr_mongo(&Expression::Aggregate {
                        name: name.clone(),
                        distinct: false,
                        args: args.clone(),
                    });
                    group_spec.insert(key, agg_val);
                }
            }

            pipeline.push(json!({"$group": group_spec}));
        } else if !query.projection.is_empty() {
            let mut project_spec = serde_json::Map::new();
            let mut has_explicit_fields = false;
            let mut has_wildcard = false;
            for proj in &query.projection {
                match proj {
                    Projection::Wildcard => { has_wildcard = true; }
                    Projection::QualifiedWildcard(_) => { has_wildcard = true; }
                    Projection::Expr(expr, alias) => {
                        has_explicit_fields = true;
                        let key = alias.clone().unwrap_or_else(|| match expr {
                            Expression::Ident(name) => name.clone(),
                            _ => "expr".to_string(),
                        });
                        let val = translate_expr_mongo(expr);
                        project_spec.insert(key, val);
                    }
                }
            }
            // Suppress _id unless it was explicitly requested or projection is wildcard
            if has_explicit_fields && !has_wildcard && !project_spec.contains_key("_id") {
                project_spec.insert("_id".to_string(), json!(0));
            }
            if !project_spec.is_empty() {
                pipeline.push(json!({"$project": project_spec}));
            }
        }
    }

    if !query.order_by.is_empty() {
        let mut sort_spec = serde_json::Map::new();
        for order in &query.order_by {
            let field = match &order.expr {
                Expression::Ident(name) => name.clone(),
                _ => continue,
            };
            let dir = match order.direction {
                OrderDir::Asc => 1,
                OrderDir::Desc => -1,
            };
            sort_spec.insert(field, json!(dir));
        }
        if !sort_spec.is_empty() {
            pipeline.push(json!({"$sort": sort_spec}));
        }
    }

    if let Some(limit) = query.limit {
        pipeline.push(json!({"$limit": limit}));
    }

    if let Some(offset) = query.offset {
        pipeline.push(json!({"$skip": offset}));
    }

    json!({
        "database": database,
        "collection": source_name,
        "pipeline": pipeline,
    })
}

// ── Cross-DB pushdown helpers ─────────────────────────────────────────────

pub fn translate_in_list(column: &str, values: &[Value], dialect: &dyn SqlDialect) -> String {
    if values.is_empty() {
        return "1=0".to_string();
    }
    let col = dialect.quote_ident(column);
    let vals: Vec<String> = values
        .iter()
        .filter_map(|v| match v {
            Value::Null => None,
            Value::Int(i) => Some(i.to_string()),
            Value::Float(f) => Some(f.to_string()),
            Value::String(s) => Some(format!("'{}'", escape_sql_string(s))),
            Value::Bool(b) => Some(if *b { "TRUE".into() } else { "FALSE".into() }),
        })
        .collect();
    if vals.is_empty() {
        return "1=0".to_string();
    }
    format!("{} IN ({})", col, vals.join(", "))
}

pub fn translate_in_list_mongo(column: &str, values: &[Value]) -> JsonValue {
    let vals: Vec<JsonValue> = values
        .iter()
        .filter_map(|v| match v {
            Value::Null => None,
            Value::Int(i) => Some(json!(i)),
            Value::Float(f) => Some(json!(f)),
            Value::String(s) => Some(json!(s)),
            Value::Bool(b) => Some(json!(b)),
        })
        .collect();
    json!({ column: { "$in": vals } })
}

pub fn build_probe_query_sql(
    table: &str,
    key_column: &str,
    values: &[Value],
    dialect: &dyn SqlDialect,
) -> String {
    let table_quoted = dialect.quote_ident(table);
    let in_clause = translate_in_list(key_column, values, dialect);
    format!("SELECT * FROM {} WHERE {}", table_quoted, in_clause)
}

pub fn build_probe_query_mongo(
    collection: &str,
    key_column: &str,
    values: &[Value],
    database: &str,
) -> JsonValue {
    let match_stage = translate_in_list_mongo(key_column, values);
    json!({
        "database": database,
        "collection": collection,
        "pipeline": [{ "$match": match_stage }],
    })
}
