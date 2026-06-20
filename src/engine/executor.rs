#![allow(dead_code)]

use std::collections::HashMap;
use std::time::Instant;

use crate::adapters::{DatabaseAdapter, QueryResult, Value};
use crate::connection::DatabaseKind;
use crate::engine::planner::{JoinStrategy, PlanNode, QueryPlan};
use crate::engine::translator::*;
use crate::lang::ast::*;
use crate::error::RiverError;

pub async fn execute_plan(
    plan: &QueryPlan,
    adapters: &HashMap<String, Box<dyn DatabaseAdapter>>,
) -> Result<QueryResult, RiverError> {
    let start = Instant::now();
    let mut result = execute_node(&plan.root, adapters).await?;
    result.elapsed = start.elapsed();
    Ok(result)
}

fn build_scan_query(source: &Source) -> Query {
    let mut q = Query::default();
    q.sources.push(source.clone());
    q.projection = vec![Projection::Wildcard];
    q
}

async fn execute_on_db(
    db_name: &str,
    db_kind: &DatabaseKind,
    query: &Query,
    adapters: &HashMap<String, Box<dyn DatabaseAdapter>>,
) -> Result<QueryResult, RiverError> {
    let adapter = adapters.get(db_name).ok_or_else(|| {
        RiverError::Unsupported(format!("no adapter connected for '{}'", db_name))
    })?;
    let native = translate_for_kind(query, db_kind);
    adapter.execute(&native).await
}

fn translate_for_kind(query: &Query, kind: &DatabaseKind) -> String {
    match kind {
        DatabaseKind::Postgres => translate_query(query, &PostgresDialect),
        DatabaseKind::MySQL => translate_query(query, &MySQLDialect),
        DatabaseKind::MSSQL => translate_query(query, &MSSQLDialect),
        DatabaseKind::SQLite => translate_query(query, &SQLiteDialect),
        DatabaseKind::MongoDB => {
            // Pass an empty string as the database name; the MongoAdapter will
            // use its default_db (derived from the connection URI) as fallback.
            serde_json::to_string(&translate_query_mongo(query, "")).unwrap_or_default()
        }
    }
}

fn find_single_db(node: &PlanNode) -> Option<(String, DatabaseKind)> {
    match node {
        PlanNode::Scan { database, .. } => database.clone(),
        PlanNode::Filter { input, .. }
        | PlanNode::Project { input, .. }
        | PlanNode::Order { input, .. }
        | PlanNode::Limit { input, .. }
        | PlanNode::Aggregate { input, .. }
        | PlanNode::Distinct { input, .. } => find_single_db(input),
        PlanNode::Join { left, right, .. } => {
            let l = find_single_db(left)?;
            let r = find_single_db(right)?;
            if l.0 == r.0 { Some(l) } else { None }
        }
        PlanNode::Union { left, right, .. } => {
            let l = find_single_db(left)?;
            let r = find_single_db(right)?;
            if l.0 == r.0 { Some(l) } else { None }
        }
        PlanNode::Empty => None,
    }
}

fn collect_single_db_query(node: &PlanNode) -> Option<(String, DatabaseKind, Query)> {
    match node {
        PlanNode::Scan { source, database, .. } => {
            let (db_name, db_kind) = database.as_ref()?;
            Some((db_name.clone(), db_kind.clone(), build_scan_query(source)))
        }
        PlanNode::Filter { input, condition } => {
            let (db_name, db_kind, mut q) = collect_single_db_query(input)?;
            q.filter = Some(match q.filter.take() {
                Some(existing) => Expression::BinaryOp {
                    op: BinaryOp::And,
                    left: Box::new(existing),
                    right: Box::new(condition.clone()),
                },
                None => condition.clone(),
            });
            Some((db_name, db_kind, q))
        }
        PlanNode::Project { input, fields } => {
            let (db_name, db_kind, mut q) = collect_single_db_query(input)?;
            q.projection = fields.clone();
            Some((db_name, db_kind, q))
        }
        PlanNode::Order { input, order_by } => {
            let (db_name, db_kind, mut q) = collect_single_db_query(input)?;
            q.order_by = order_by.clone();
            Some((db_name, db_kind, q))
        }
        PlanNode::Limit { input, limit, offset } => {
            let (db_name, db_kind, mut q) = collect_single_db_query(input)?;
            q.limit = Some(*limit);
            q.offset = Some(*offset);
            Some((db_name, db_kind, q))
        }
        PlanNode::Aggregate { input, group_by, aggs } => {
            let (db_name, db_kind, mut q) = collect_single_db_query(input)?;
            q.group_by = group_by.clone();
            for agg in aggs {
                q.projection.push(Projection::Expr(agg.clone(), None));
            }
            Some((db_name, db_kind, q))
        }
        PlanNode::Distinct { input } => {
            let (db_name, db_kind, mut q) = collect_single_db_query(input)?;
            q.distinct = true;
            Some((db_name, db_kind, q))
        }
        PlanNode::Join { .. } | PlanNode::Union { .. } | PlanNode::Empty => None,
    }
}

async fn execute_node(
    node: &PlanNode,
    adapters: &HashMap<String, Box<dyn DatabaseAdapter>>,
) -> Result<QueryResult, RiverError> {
    if let Some((db_name, db_kind, query)) = collect_single_db_query(node) {
        return execute_on_db(&db_name, &db_kind, &query, adapters).await;
    }

    match node {
        PlanNode::Join {
            left,
            right,
            condition,
            strategy,
            join_kind,
        } => {
            let left_fut = Box::pin(execute_node(left, adapters));
            let right_fut = Box::pin(execute_node(right, adapters));
            let (lr, rr) = tokio::join!(left_fut, right_fut);
            let left_result = lr?;
            let right_result = rr?;
            join_results(left_result, right_result, condition, strategy, *join_kind)
        }
        PlanNode::Filter { input, condition } => {
            let result = Box::pin(execute_node(input, adapters)).await?;
            apply_filter(&result, condition)
        }
        PlanNode::Project { input, fields } => {
            let result = Box::pin(execute_node(input, adapters)).await?;
            apply_projection(&result, fields)
        }
        PlanNode::Order { input, order_by } => {
            let result = Box::pin(execute_node(input, adapters)).await?;
            apply_order(&result, order_by)
        }
        PlanNode::Limit { input, limit, offset } => {
            let result = Box::pin(execute_node(input, adapters)).await?;
            apply_limit(&result, *limit, *offset)
        }
        PlanNode::Aggregate { input, group_by, aggs } => {
            let result = Box::pin(execute_node(input, adapters)).await?;
            apply_aggregate(&result, group_by, aggs)
        }
        PlanNode::Distinct { input } => {
            let result = Box::pin(execute_node(input, adapters)).await?;
            apply_distinct(&result)
        }
        PlanNode::Union { left, right, all } => {
            let lf = Box::pin(execute_node(left, adapters));
            let rf = Box::pin(execute_node(right, adapters));
            let (lr, rr) = tokio::join!(lf, rf);
            union_results(lr?, rr?, *all)
        }
        PlanNode::Empty => Ok(empty_result()),
        PlanNode::Scan { .. } => {
            Err(RiverError::Unsupported(
                "no database configured — create a river.yaml file with connections".into(),
            ))
        }
    }
}

fn empty_result() -> QueryResult {
    QueryResult {
        columns: vec![],
        rows: vec![],
        elapsed: std::time::Duration::default(),
        rows_affected: 0,
    }
}

// ── Join algorithms ────────────────────────────────────────────────────────

fn join_results(
    left: QueryResult,
    right: QueryResult,
    condition: &Expression,
    strategy: &JoinStrategy,
    join_kind: JoinKind,
) -> Result<QueryResult, RiverError> {
    let can_hash = matches!(strategy, JoinStrategy::Hash | JoinStrategy::Auto)
        && resolve_equi_columns(condition, &left.columns, &right.columns).is_some();

    if can_hash {
        hash_join(left, right, condition, join_kind)
    } else {
        nested_loop_join(left, right, condition, join_kind)
    }
}

fn resolve_equi_columns(
    condition: &Expression,
    left_cols: &[String],
    right_cols: &[String],
) -> Option<(usize, usize)> {
    match condition {
        Expression::BinaryOp {
            op: BinaryOp::Eq,
            left,
            right,
        } => {
            let li = find_col_idx(left, left_cols, right_cols)?;
            let ri = find_col_idx(right, left_cols, right_cols)?;
            if li.1 != ri.1 {
                // Already on opposite sides
                if li.1 {
                    Some((ri.0, li.0))
                } else {
                    Some((li.0, ri.0))
                }
            } else {
                // Both resolved to the same side. This happens when the column
                // name exists in both tables (e.g., `u.id = m.id` where both
                // have an "id" column). In this case, assume the left expression
                // refers to the left table and the right expression to the right.
                let l_name = extract_field_name(left)?;
                let r_name = extract_field_name(right)?;
                let l_idx = left_cols.iter().position(|c| c == l_name)?;
                let r_idx = right_cols.iter().position(|c| c == r_name)?;
                Some((l_idx, r_idx))
            }
        }
        _ => None,
    }
}

fn extract_field_name(expr: &Expression) -> Option<&str> {
    match expr {
        Expression::Ident(n) => Some(n.as_str()),
        Expression::QualifiedIdent { field, .. } => Some(field.as_str()),
        _ => None,
    }
}

fn find_col_idx(
    expr: &Expression,
    left_cols: &[String],
    right_cols: &[String],
) -> Option<(usize, bool)> {
    let name = match expr {
        Expression::Ident(n) => n,
        Expression::QualifiedIdent { field, .. } => field,
        _ => return None,
    };
    if let Some(i) = left_cols.iter().position(|c| c == name) {
        Some((i, false))
    } else {
        right_cols.iter().position(|c| c == name).map(|i| (i, true))
    }
}

fn hash_join(
    left: QueryResult,
    right: QueryResult,
    condition: &Expression,
    join_kind: JoinKind,
) -> Result<QueryResult, RiverError> {
    let (left_key_idx, right_key_idx) =
        resolve_equi_columns(condition, &left.columns, &right.columns)
            .ok_or_else(|| {
                RiverError::Unsupported("hash join requires equi-join condition".into())
            })?;

    let orig_left_cols = left.columns.len();
    let orig_right_cols = right.columns.len();

    let (build, probe, build_key, probe_key, swapped) = if left.rows.len() <= right.rows.len() {
        (left, right, left_key_idx, right_key_idx, false)
    } else {
        (right, left, right_key_idx, left_key_idx, true)
    };
    let left_col_count = if swapped { orig_right_cols } else { orig_left_cols };
    let right_col_count = if swapped { orig_left_cols } else { orig_right_cols };

    let mut hash_map: HashMap<Value, Vec<usize>> = HashMap::new();
    for (i, row) in build.rows.iter().enumerate() {
        let key = row.get(build_key).cloned().unwrap_or(Value::Null);
        hash_map.entry(key).or_default().push(i);
    }

    let columns = if swapped {
        merge_col_names(&probe.columns, &build.columns)
    } else {
        merge_col_names(&build.columns, &probe.columns)
    };

    let mut rows: Vec<Vec<Value>> = Vec::new();
    let mut probe_matched: Vec<bool> = vec![false; probe.rows.len()];
    let mut build_matched: Vec<bool> = vec![false; build.rows.len()];

    for (pi, probe_row) in probe.rows.iter().enumerate() {
        let key = probe_row.get(probe_key).cloned().unwrap_or(Value::Null);
        if let Some(build_idxs) = hash_map.get(&key) {
            for &bi in build_idxs {
                probe_matched[pi] = true;
                build_matched[bi] = true;
                let merged = if swapped {
                    merge_vals(probe_row, &build.rows[bi])
                } else {
                    merge_vals(&build.rows[bi], probe_row)
                };
                rows.push(merged);
            }
        }
    }

    let include_left = matches!(join_kind, JoinKind::Left | JoinKind::Full);
    let include_right = matches!(join_kind, JoinKind::Right | JoinKind::Full);

    if include_left {
        let nulls = vec![Value::Null; right_col_count];
        if swapped {
            for (pi, &matched) in probe_matched.iter().enumerate() {
                if !matched {
                    rows.push(merge_vals(&probe.rows[pi], &nulls));
                }
            }
        } else {
            for (bi, &matched) in build_matched.iter().enumerate() {
                if !matched {
                    rows.push(merge_vals(&build.rows[bi], &nulls));
                }
            }
        }
    }

    if include_right {
        let nulls = vec![Value::Null; left_col_count];
        if swapped {
            for (bi, &matched) in build_matched.iter().enumerate() {
                if !matched {
                    rows.push(merge_vals(&nulls, &build.rows[bi]));
                }
            }
        } else {
            for (pi, &matched) in probe_matched.iter().enumerate() {
                if !matched {
                    rows.push(merge_vals(&nulls, &probe.rows[pi]));
                }
            }
        }
    }

    Ok(QueryResult {
        columns,
        rows,
        elapsed: std::time::Duration::default(),
        rows_affected: 0,
    })
}

fn nested_loop_join(
    left: QueryResult,
    right: QueryResult,
    condition: &Expression,
    join_kind: JoinKind,
) -> Result<QueryResult, RiverError> {
    let columns = merge_col_names(&left.columns, &right.columns);
    let right_col_count = right.columns.len();
    let left_col_count = left.columns.len();
    let mut rows: Vec<Vec<Value>> = Vec::new();
    let mut left_matched: Vec<bool> = vec![false; left.rows.len()];
    let mut right_matched: Vec<bool> = vec![false; right.rows.len()];

    for (li, l_row) in left.rows.iter().enumerate() {
        for (ri, r_row) in right.rows.iter().enumerate() {
            let merged = merge_vals(l_row, r_row);
            if eval_expr_bool(condition, &columns, &merged) {
                rows.push(merged.clone());
                left_matched[li] = true;
                right_matched[ri] = true;
            }
        }
    }

    // Outer join support
    let include_left = matches!(join_kind, JoinKind::Left | JoinKind::Full);
    let include_right = matches!(join_kind, JoinKind::Right | JoinKind::Full);
    let include_inner = matches!(join_kind, JoinKind::Inner | JoinKind::Cross);

    // Cross join: include everything regardless of condition
    if join_kind == JoinKind::Cross {
        rows.clear();
        for l_row in &left.rows {
            for r_row in &right.rows {
                rows.push(merge_vals(l_row, r_row));
            }
        }
    }

    if include_left || include_inner {
        for (li, &matched) in left_matched.iter().enumerate() {
            if !matched && include_left {
                let nulls = vec![Value::Null; right_col_count];
                rows.push(merge_vals(&left.rows[li], &nulls));
            }
        }
    }

    if include_right {
        for (ri, &matched) in right_matched.iter().enumerate() {
            if !matched {
                let nulls = vec![Value::Null; left_col_count];
                rows.push(merge_vals(&nulls, &right.rows[ri]));
            }
        }
    }

    Ok(QueryResult {
        columns,
        rows,
        elapsed: std::time::Duration::default(),
        rows_affected: 0,
    })
}

// ── In-memory operators ────────────────────────────────────────────────────

fn apply_filter(result: &QueryResult, condition: &Expression) -> Result<QueryResult, RiverError> {
    let rows: Vec<Vec<Value>> = result
        .rows
        .iter()
        .filter(|row| eval_expr_bool(condition, &result.columns, row))
        .cloned()
        .collect();
    Ok(QueryResult {
        columns: result.columns.clone(),
        rows,
        elapsed: std::time::Duration::default(),
        rows_affected: 0,
    })
}

fn apply_projection(
    result: &QueryResult,
    fields: &[Projection],
) -> Result<QueryResult, RiverError> {
    if fields.is_empty() {
        return Ok(result.clone());
    }
    let (new_cols, indices): (Vec<String>, Vec<Option<usize>>) = fields
        .iter()
        .map(|p| match p {
            Projection::Wildcard => {
                ("*".to_string(), None)
            }
            Projection::QualifiedWildcard(_) => ("*".to_string(), None),
            Projection::Expr(expr, alias) => {
                let name = alias.clone().unwrap_or_else(|| match expr {
                    Expression::Ident(n) => n.clone(),
                    Expression::QualifiedIdent { field, .. } => field.clone(),
                    _ => "expr".to_string(),
                });
                let idx = match expr {
                    Expression::Ident(n) | Expression::QualifiedIdent { field: n, .. } => {
                        result.columns.iter().position(|c| c == n)
                    }
                    _ => None,
                };
                (name, idx)
            }
        })
        .unzip();

    // Handle wildcards: if any projection is a wildcard, include all columns
    let has_wildcard = fields.iter().any(|p| matches!(p, Projection::Wildcard));
    if has_wildcard {
        return Ok(result.clone());
    }

    let rows: Vec<Vec<Value>> = result
        .rows
        .iter()
        .map(|row| {
            indices
                .iter()
                .map(|opt_idx| match opt_idx {
                    Some(idx) => row.get(*idx).cloned().unwrap_or(Value::Null),
                    None => Value::Null,
                })
                .collect()
        })
        .collect();

    Ok(QueryResult {
        columns: new_cols,
        rows,
        elapsed: std::time::Duration::default(),
        rows_affected: 0,
    })
}

fn apply_order(
    result: &QueryResult,
    order_by: &[OrderBy],
) -> Result<QueryResult, RiverError> {
    let mut rows = result.rows.clone();
    rows.sort_by(|a, b| {
        for order in order_by {
            let va = eval_expr(&order.expr, &result.columns, a);
            let vb = eval_expr(&order.expr, &result.columns, b);
            let cmp = cmp_values(&va, &vb);
            if cmp != std::cmp::Ordering::Equal {
                return if order.direction == OrderDir::Desc {
                    cmp.reverse()
                } else {
                    cmp
                };
            }
        }
        std::cmp::Ordering::Equal
    });
    Ok(QueryResult {
        columns: result.columns.clone(),
        rows,
        elapsed: std::time::Duration::default(),
        rows_affected: 0,
    })
}

fn apply_limit(
    result: &QueryResult,
    limit: u64,
    offset: u64,
) -> Result<QueryResult, RiverError> {
    let rows: Vec<Vec<Value>> = result
        .rows
        .iter()
        .skip(offset as usize)
        .take(limit as usize)
        .cloned()
        .collect();
    Ok(QueryResult {
        columns: result.columns.clone(),
        rows,
        elapsed: std::time::Duration::default(),
        rows_affected: 0,
    })
}

fn apply_aggregate(
    result: &QueryResult,
    _group_by: &[Expression],
    _aggs: &[Expression],
) -> Result<QueryResult, RiverError> {
    // For now, pass-through — aggregates are pushed down to DB queries
    Ok(result.clone())
}

fn apply_distinct(result: &QueryResult) -> Result<QueryResult, RiverError> {
    let mut seen = std::collections::HashSet::new();
    let rows: Vec<Vec<Value>> = result
        .rows
        .iter()
        .filter(|&row| seen.insert(row.clone()))
        .cloned()
        .collect();
    Ok(QueryResult {
        columns: result.columns.clone(),
        rows,
        elapsed: std::time::Duration::default(),
        rows_affected: 0,
    })
}

fn union_results(
    left: QueryResult,
    right: QueryResult,
    _all: bool,
) -> Result<QueryResult, RiverError> {
    let columns = if left.columns.len() >= right.columns.len() {
        left.columns.clone()
    } else {
        right.columns.clone()
    };
    let mut rows = left.rows.clone();
    for r_row in &right.rows {
        let mut aligned = r_row.clone();
        aligned.resize(columns.len(), Value::Null);
        rows.push(aligned);
    }
    // Deduplicate if not UNION ALL
    let mut seen = std::collections::HashSet::new();
    let rows: Vec<Vec<Value>> = rows
        .into_iter()
        .filter(|row| seen.insert(row.clone()))
        .collect();
    Ok(QueryResult {
        columns,
        rows,
        elapsed: std::time::Duration::default(),
        rows_affected: 0,
    })
}

// ── Expression evaluation ──────────────────────────────────────────────────

fn eval_expr_bool(expr: &Expression, columns: &[String], row: &[Value]) -> bool {
    matches!(eval_expr(expr, columns, row), Value::Bool(true))
}

fn eval_expr(expr: &Expression, columns: &[String], row: &[Value]) -> Value {
    match expr {
        Expression::String(s) => Value::String(s.clone()),
        Expression::Number(n) => Value::Float(*n),
        Expression::Integer(i) => Value::Int(*i),
        Expression::Boolean(b) => Value::Bool(*b),
        Expression::Null => Value::Null,
        Expression::Ident(name) | Expression::QualifiedIdent { field: name, .. } => {
            columns
                .iter()
                .position(|c| c == name)
                .and_then(|i| row.get(i))
                .cloned()
                .unwrap_or(Value::Null)
        }
        Expression::BinaryOp { op, left, right } => {
            let l = eval_expr(left, columns, row);
            let r = eval_expr(right, columns, row);
            eval_binary(op, &l, &r)
        }
        Expression::UnaryOp { op, expr } => {
            let v = eval_expr(expr, columns, row);
            eval_unary(op, &v)
        }
        Expression::Between {
            expr,
            low,
            high,
        } => {
            let v = eval_expr(expr, columns, row);
            let lo = eval_expr(low, columns, row);
            let hi = eval_expr(high, columns, row);
            Value::Bool(cmp_values(&v, &lo) != std::cmp::Ordering::Less
                && cmp_values(&v, &hi) != std::cmp::Ordering::Greater)
        }
        Expression::Case {
            expr: case_val,
            whens,
            else_expr,
        } => {
            for (when, then) in whens {
                let match_val = if let Some(cv) = case_val {
                    let cv_val = eval_expr(cv, columns, row);
                    let when_val = eval_expr(when, columns, row);
                    cv_val == when_val
                } else {
                    eval_expr_bool(when, columns, row)
                };
                if match_val {
                    return eval_expr(then, columns, row);
                }
            }
            else_expr
                .as_ref()
                .map(|e| eval_expr(e, columns, row))
                .unwrap_or(Value::Null)
        }
        Expression::Cast { expr, target } => {
            let v = eval_expr(expr, columns, row);
            cast_value(&v, target)
        }
        Expression::Array(_) => Value::String("[...]".into()),
        Expression::Object(_) => Value::String("{...}".into()),
        _ => Value::Null,
    }
}

fn eval_binary(op: &BinaryOp, left: &Value, right: &Value) -> Value {
    match op {
        BinaryOp::And => Value::Bool(is_truthy(left) && is_truthy(right)),
        BinaryOp::Or => Value::Bool(is_truthy(left) || is_truthy(right)),
        BinaryOp::Eq => Value::Bool(left == right),
        BinaryOp::Neq => Value::Bool(left != right),
        BinaryOp::Gt => Value::Bool(cmp_values(left, right) == std::cmp::Ordering::Greater),
        BinaryOp::Gte => {
            Value::Bool(cmp_values(left, right) != std::cmp::Ordering::Less)
        }
        BinaryOp::Lt => Value::Bool(cmp_values(left, right) == std::cmp::Ordering::Less),
        BinaryOp::Lte => {
            Value::Bool(cmp_values(left, right) != std::cmp::Ordering::Greater)
        }
        BinaryOp::Like | BinaryOp::ILike => {
            // Simple contains match for LIKE
            if let (Value::String(s), Value::String(pat)) = (left, right) {
                let pattern = pat.replace('%', "").replace('_', "");
                Value::Bool(s.contains(&pattern))
            } else {
                Value::Bool(false)
            }
        }
        BinaryOp::Add => arith_op(left, right, |a, b| a + b, |a, b| a + b),
        BinaryOp::Sub => arith_op(left, right, |a, b| a - b, |a, b| a - b),
        BinaryOp::Mul => arith_op(left, right, |a, b| a * b, |a, b| a * b),
        BinaryOp::Div => arith_op(left, right, |a, b| a / b, |a, b| a / b),
        BinaryOp::Mod => arith_op(left, right, |a, b| a % b, |a, b| a % b),
        _ => Value::Null,
    }
}

fn arith_op<F1, F2>(left: &Value, right: &Value, int_op: F1, float_op: F2) -> Value
where
    F1: Fn(i64, i64) -> i64,
    F2: Fn(f64, f64) -> f64,
{
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Value::Int(int_op(*a, *b)),
        (Value::Float(a), Value::Float(b)) => Value::Float(float_op(*a, *b)),
        (Value::Int(a), Value::Float(b)) => Value::Float(float_op(*a as f64, *b)),
        (Value::Float(a), Value::Int(b)) => Value::Float(float_op(*a, *b as f64)),
        _ => Value::Null,
    }
}

fn eval_unary(op: &UnaryOp, val: &Value) -> Value {
    match op {
        UnaryOp::Not => Value::Bool(!is_truthy(val)),
        UnaryOp::Neg => match val {
            Value::Int(i) => Value::Int(-i),
            Value::Float(f) => Value::Float(-f),
            _ => Value::Null,
        },
    }
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Int(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::String(s) => !s.is_empty(),
    }
}

fn cast_value(v: &Value, target: &DataType) -> Value {
    match target {
        DataType::Integer => match v {
            Value::Int(_) => v.clone(),
            Value::Float(f) => Value::Int(*f as i64),
            Value::String(s) => s.parse().map(Value::Int).unwrap_or(Value::Null),
            Value::Bool(b) => Value::Int(if *b { 1 } else { 0 }),
            _ => Value::Null,
        },
        DataType::Float => match v {
            Value::Float(_) => v.clone(),
            Value::Int(i) => Value::Float(*i as f64),
            Value::String(s) => s.parse().map(Value::Float).unwrap_or(Value::Null),
            _ => Value::Null,
        },
        DataType::String => match v {
            Value::String(_) => v.clone(),
            _ => Value::String(format!("{:?}", v)),
        },
        DataType::Boolean => match v {
            Value::Bool(_) => v.clone(),
            _ => Value::Bool(is_truthy(v)),
        },
        DataType::DateTime | DataType::Json => Value::String(format!("{:?}", v)),
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn cmp_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Null, _) => std::cmp::Ordering::Less,
        (_, Value::Null) => std::cmp::Ordering::Greater,
        (Value::String(a), Value::String(b)) => a.cmp(b),
        (Value::Int(a), Value::Int(b)) => a.cmp(b),
        (Value::Float(a), Value::Float(b)) => {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::Int(a), Value::Float(b)) => (*a as f64)
            .partial_cmp(b)
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::Float(a), Value::Int(b)) => a
            .partial_cmp(&(*b as f64))
            .unwrap_or(std::cmp::Ordering::Equal),
        _ => std::cmp::Ordering::Equal,
    }
}

fn merge_col_names(left: &[String], right: &[String]) -> Vec<String> {
    let mut cols = left.to_vec();
    cols.extend(right.iter().cloned());
    cols
}

fn merge_vals(left: &[Value], right: &[Value]) -> Vec<Value> {
    let mut vals = left.to_vec();
    vals.extend(right.iter().cloned());
    vals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::Value;

    fn mk_result(columns: Vec<&str>, rows: Vec<Vec<Value>>) -> QueryResult {
        QueryResult {
            columns: columns.into_iter().map(String::from).collect(),
            rows,
            elapsed: std::time::Duration::default(),
            rows_affected: 0,
        }
    }

    // ── resolve_equi_columns ───────────────────────────────────────────

    #[test]
    fn resolve_simple_equi() {
        let cond = Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::QualifiedIdent {
                table: "u".into(),
                field: "id".into(),
            }),
            right: Box::new(Expression::QualifiedIdent {
                table: "o".into(),
                field: "user_id".into(),
            }),
        };
        let left_cols = ["id".to_string(), "name".to_string()];
        let right_cols = ["user_id".to_string(), "amount".to_string()];
        let res = resolve_equi_columns(&cond, &left_cols, &right_cols);
        assert_eq!(res, Some((0, 0)));
    }

    #[test]
    fn resolve_equi_with_idents() {
        let cond = Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::Ident("a_id".into())),
            right: Box::new(Expression::Ident("b_id".into())),
        };
        let left_cols = ["a_id".to_string()];
        let right_cols = ["b_id".to_string()];
        let res = resolve_equi_columns(&cond, &left_cols, &right_cols);
        assert_eq!(res, Some((0, 0)));
    }

    #[test]
    fn resolve_non_equi_returns_none() {
        let cond = Expression::BinaryOp {
            op: BinaryOp::Gt,
            left: Box::new(Expression::Ident("a".into())),
            right: Box::new(Expression::Ident("b".into())),
        };
        let res = resolve_equi_columns(&cond, &[], &[]);
        assert_eq!(res, None);
    }

    // ── hash_join ──────────────────────────────────────────────────────

    #[test]
    fn hash_join_inner() {
        let left_row1 = vec![Value::Int(1), Value::String("Alice".into())];
        let left_row2 = vec![Value::Int(10), Value::String("Bob".into())];
        let left = mk_result(
            vec!["id", "name"],
            vec![left_row1, left_row2],
        );
        let right_row1 = vec![Value::Int(1), Value::Int(100)];
        let right_row2 = vec![Value::Int(2), Value::Int(200)];
        let right_row3 = vec![Value::Int(3), Value::Int(300)];
        let right = mk_result(
            vec!["user_id", "total"],
            vec![right_row1, right_row2, right_row3],
        );
        let cond = Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::Ident("id".into())),
            right: Box::new(Expression::Ident("user_id".into())),
        };
        let result = hash_join(left, right, &cond, JoinKind::Inner).unwrap();
        // Row 1 (id=1) matches user_id=1
        // Row 2 (id=10) matches nothing
        assert_eq!(result.columns, vec!["id", "name", "user_id", "total"]);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], Value::Int(1));
        assert_eq!(result.rows[0][1], Value::String("Alice".into()));
        assert_eq!(result.rows[0][2], Value::Int(1));
        assert_eq!(result.rows[0][3], Value::Int(100));
    }

    #[test]
    fn hash_join_left() {
        let left = mk_result(
            vec!["id", "name"],
            vec![
                vec![Value::Int(1), Value::String("Alice".into())],
                vec![Value::Int(3), Value::String("Charlie".into())],
            ],
        );
        let right = mk_result(
            vec!["user_id", "total"],
            vec![vec![Value::Int(1), Value::Int(100)]],
        );
        let cond = Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::Ident("id".into())),
            right: Box::new(Expression::Ident("user_id".into())),
        };
        let result = hash_join(left, right, &cond, JoinKind::Left).unwrap();
        assert_eq!(result.rows.len(), 2);
        let matched = result.rows.iter().find(|r| r[0] == Value::Int(1)).unwrap();
        assert_eq!(matched[3], Value::Int(100));
        let unmatched = result.rows.iter().find(|r| r[0] == Value::Int(3)).unwrap();
        assert_eq!(unmatched[2], Value::Null);
        assert_eq!(unmatched[3], Value::Null);
    }

    #[test]
    fn hash_join_multiple_matches() {
        let left = mk_result(
            vec!["id"],
            vec![vec![Value::Int(1)]],
        );
        let right = mk_result(
            vec!["user_id"],
            vec![vec![Value::Int(1)], vec![Value::Int(1)]],
        );
        let cond = Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::Ident("id".into())),
            right: Box::new(Expression::Ident("user_id".into())),
        };
        let result = hash_join(left, right, &cond, JoinKind::Inner).unwrap();
        assert_eq!(result.rows.len(), 2);
    }

    // ── nested_loop_join ───────────────────────────────────────────────

    #[test]
    fn nested_loop_join_inner() {
        let left = mk_result(
            vec!["id", "name"],
            vec![
                vec![Value::Int(1), Value::String("Alice".into())],
                vec![Value::Int(2), Value::String("Bob".into())],
            ],
        );
        let right = mk_result(
            vec!["user_id", "total"],
            vec![
                vec![Value::Int(1), Value::Int(100)],
                vec![Value::Int(1), Value::Int(200)],
            ],
        );
        let cond = Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::Ident("id".into())),
            right: Box::new(Expression::Ident("user_id".into())),
        };
        let result = nested_loop_join(left, right, &cond, JoinKind::Inner).unwrap();
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn nested_loop_cross_join() {
        let left = mk_result(
            vec!["a"],
            vec![vec![Value::Int(1)], vec![Value::Int(2)]],
        );
        let right = mk_result(
            vec!["b"],
            vec![vec![Value::String("x".into())], vec![Value::String("y".into())]],
        );
        let cond = Expression::Boolean(true);
        let result = nested_loop_join(left, right, &cond, JoinKind::Cross).unwrap();
        assert_eq!(result.rows.len(), 4);
    }

    // ── apply_filter ───────────────────────────────────────────────────

    #[test]
    fn filter_gt() {
        let input = mk_result(
            vec!["age"],
            vec![
                vec![Value::Int(10)],
                vec![Value::Int(25)],
                vec![Value::Int(30)],
            ],
        );
        let cond = Expression::BinaryOp {
            op: BinaryOp::Gt,
            left: Box::new(Expression::Ident("age".into())),
            right: Box::new(Expression::Integer(20)),
        };
        let result = apply_filter(&input, &cond).unwrap();
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn filter_and() {
        let input = mk_result(
            vec!["age", "name"],
            vec![
                vec![Value::Int(25), Value::String("Alice".into())],
                vec![Value::Int(30), Value::String("Bob".into())],
                vec![Value::Int(20), Value::String("Charlie".into())],
            ],
        );
        let cond = Expression::BinaryOp {
            op: BinaryOp::And,
            left: Box::new(Expression::BinaryOp {
                op: BinaryOp::Gt,
                left: Box::new(Expression::Ident("age".into())),
                right: Box::new(Expression::Integer(20)),
            }),
            right: Box::new(Expression::BinaryOp {
                op: BinaryOp::Neq,
                left: Box::new(Expression::Ident("name".into())),
                right: Box::new(Expression::String("Bob".into())),
            }),
        };
        let result = apply_filter(&input, &cond).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][1], Value::String("Alice".into()));
    }

    // ── apply_projection ───────────────────────────────────────────────

    #[test]
    fn project_single_column() {
        let input = mk_result(
            vec!["id", "name", "age"],
            vec![
                vec![
                    Value::Int(1),
                    Value::String("Alice".into()),
                    Value::Int(25),
                ],
            ],
        );
        let fields = vec![Projection::Expr(
            Expression::Ident("name".into()),
            None,
        )];
        let result = apply_projection(&input, &fields).unwrap();
        assert_eq!(result.columns, vec!["name"]);
        assert_eq!(result.rows[0], vec![Value::String("Alice".into())]);
    }

    #[test]
    fn project_with_alias() {
        let input = mk_result(
            vec!["id"],
            vec![vec![Value::Int(1)], vec![Value::Int(2)]],
        );
        let fields = vec![Projection::Expr(
            Expression::Ident("id".into()),
            Some("user_id".into()),
        )];
        let result = apply_projection(&input, &fields).unwrap();
        assert_eq!(result.columns, vec!["user_id"]);
        assert_eq!(result.rows[0], vec![Value::Int(1)]);
    }

    // ── apply_order ────────────────────────────────────────────────────

    #[test]
    fn order_asc() {
        let input = mk_result(
            vec!["val"],
            vec![vec![Value::Int(3)], vec![Value::Int(1)], vec![Value::Int(2)]],
        );
        let order = vec![OrderBy {
            expr: Expression::Ident("val".into()),
            direction: OrderDir::Asc,
            nulls: NullsOrder::Default,
        }];
        let result = apply_order(&input, &order).unwrap();
        assert_eq!(
            result.rows,
            vec![vec![Value::Int(1)], vec![Value::Int(2)], vec![Value::Int(3)]]
        );
    }

    #[test]
    fn order_desc() {
        let input = mk_result(
            vec!["val"],
            vec![vec![Value::Int(1)], vec![Value::Int(3)], vec![Value::Int(2)]],
        );
        let order = vec![OrderBy {
            expr: Expression::Ident("val".into()),
            direction: OrderDir::Desc,
            nulls: NullsOrder::Default,
        }];
        let result = apply_order(&input, &order).unwrap();
        assert_eq!(
            result.rows,
            vec![vec![Value::Int(3)], vec![Value::Int(2)], vec![Value::Int(1)]]
        );
    }

    // ── apply_limit ────────────────────────────────────────────────────

    #[test]
    fn limit_basic() {
        let input = mk_result(
            vec!["x"],
            vec![
                vec![Value::Int(1)],
                vec![Value::Int(2)],
                vec![Value::Int(3)],
                vec![Value::Int(4)],
                vec![Value::Int(5)],
            ],
        );
        let result = apply_limit(&input, 3, 0).unwrap();
        assert_eq!(
            result.rows,
            vec![vec![Value::Int(1)], vec![Value::Int(2)], vec![Value::Int(3)]]
        );
    }

    #[test]
    fn limit_offset() {
        let input = mk_result(
            vec!["x"],
            vec![
                vec![Value::Int(1)],
                vec![Value::Int(2)],
                vec![Value::Int(3)],
                vec![Value::Int(4)],
                vec![Value::Int(5)],
            ],
        );
        let result = apply_limit(&input, 2, 2).unwrap();
        assert_eq!(
            result.rows,
            vec![vec![Value::Int(3)], vec![Value::Int(4)]]
        );
    }

    // ── apply_distinct ─────────────────────────────────────────────────

    #[test]
    fn distinct_removes_dupes() {
        let input = mk_result(
            vec!["x"],
            vec![
                vec![Value::Int(1)],
                vec![Value::Int(2)],
                vec![Value::Int(2)],
                vec![Value::Int(3)],
                vec![Value::Int(1)],
                vec![Value::Int(4)],
            ],
        );
        let result = apply_distinct(&input).unwrap();
        assert_eq!(result.rows.len(), 4);
    }

    // ── eval_expr ──────────────────────────────────────────────────────

    #[test]
    fn eval_ident() {
        let cols = ["a", "b"];
        let row = [Value::Int(42), Value::String("hello".into())];
        let expr = Expression::Ident("b".into());
        let result = eval_expr(&expr, &cols.map(String::from), &row);
        assert_eq!(result, Value::String("hello".into()));
    }

    #[test]
    fn eval_eq() {
        let cols = ["x"];
        let row = [Value::Int(10)];
        let expr = Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::Ident("x".into())),
            right: Box::new(Expression::Integer(10)),
        };
        assert!(eval_expr_bool(&expr, &cols.map(String::from), &row));
        let row2 = [Value::Int(5)];
        assert!(!eval_expr_bool(&expr, &cols.map(String::from), &row2));
    }

    #[test]
    fn eval_null() {
        let cols = ["x"];
        let row = [Value::Null];
        let expr = Expression::BinaryOp {
            op: BinaryOp::Eq,
            left: Box::new(Expression::Ident("x".into())),
            right: Box::new(Expression::Integer(10)),
        };
        assert!(!eval_expr_bool(&expr, &cols.map(String::from), &row));
    }

    // ── cmp_values ─────────────────────────────────────────────────────

    #[test]
    fn cmp_ints() {
        assert_eq!(
            cmp_values(&Value::Int(1), &Value::Int(2)),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            cmp_values(&Value::Int(5), &Value::Int(5)),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn cmp_nulls() {
        assert_eq!(
            cmp_values(&Value::Null, &Value::Null),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            cmp_values(&Value::Null, &Value::Int(1)),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            cmp_values(&Value::String("a".into()), &Value::Null),
            std::cmp::Ordering::Greater
        );
    }

    // ── planner: is_cross_db ────────────────────────────────────────────

    #[test]
    fn is_cross_db_true() {
        use crate::engine::planner::{PlanNode, is_cross_db};
        let plan = PlanNode::Join {
            left: Box::new(PlanNode::Scan {
                source: Source {
                    name: "u".into(),
                    alias: None,
                    connection: Some("pg".into()),
                    kind: SourceKind::Table("users".into()),
                },
                database: Some(("pg".into(), crate::connection::DatabaseKind::Postgres)),
                filter: None,
            }),
            right: Box::new(PlanNode::Scan {
                source: Source {
                    name: "o".into(),
                    alias: None,
                    connection: Some("mongo".into()),
                    kind: SourceKind::Table("orders".into()),
                },
                database: Some(("mongo".into(), crate::connection::DatabaseKind::MongoDB)),
                filter: None,
            }),
            condition: Expression::Boolean(true),
            strategy: crate::engine::planner::JoinStrategy::Hash,
            join_kind: JoinKind::Inner,
        };
        assert!(is_cross_db(&plan));
    }

    #[test]
    fn is_cross_db_false_same_db() {
        use crate::engine::planner::{PlanNode, is_cross_db};
        let plan = PlanNode::Join {
            left: Box::new(PlanNode::Scan {
                source: Source {
                    name: "u".into(),
                    alias: None,
                    connection: Some("pg".into()),
                    kind: SourceKind::Table("users".into()),
                },
                database: Some(("pg".into(), crate::connection::DatabaseKind::Postgres)),
                filter: None,
            }),
            right: Box::new(PlanNode::Scan {
                source: Source {
                    name: "o".into(),
                    alias: None,
                    connection: Some("pg".into()),
                    kind: SourceKind::Table("orders".into()),
                },
                database: Some(("pg".into(), crate::connection::DatabaseKind::Postgres)),
                filter: None,
            }),
            condition: Expression::Boolean(true),
            strategy: crate::engine::planner::JoinStrategy::Hash,
            join_kind: JoinKind::Inner,
        };
        assert!(!is_cross_db(&plan));
    }

    #[test]
    fn find_all_databases_returns_distinct() {
        use crate::engine::planner::{PlanNode, find_all_databases};
        let plan = PlanNode::Join {
            left: Box::new(PlanNode::Scan {
                source: Source {
                    name: "a".into(),
                    alias: None,
                    connection: Some("pg".into()),
                    kind: SourceKind::Table("t1".into()),
                },
                database: Some(("pg".into(), crate::connection::DatabaseKind::Postgres)),
                filter: None,
            }),
            right: Box::new(PlanNode::Scan {
                source: Source {
                    name: "b".into(),
                    alias: None,
                    connection: Some("mongo".into()),
                    kind: SourceKind::Table("t2".into()),
                },
                database: Some(("mongo".into(), crate::connection::DatabaseKind::MongoDB)),
                filter: None,
            }),
            condition: Expression::Boolean(true),
            strategy: crate::engine::planner::JoinStrategy::Hash,
            join_kind: JoinKind::Inner,
        };
        let dbs = find_all_databases(&plan);
        assert_eq!(dbs.len(), 2);
    }

    // ── collect_single_db_query ────────────────────────────────────────

    #[test]
    fn collect_single_db_scan() {
        let source = Source {
            name: "users".into(),
            alias: None,
            connection: None,
            kind: SourceKind::Table("users".into()),
        };
        let node = PlanNode::Scan {
            source,
            database: Some(("pg".into(), crate::connection::DatabaseKind::Postgres)),
            filter: None,
        };
        let result = collect_single_db_query(&node);
        assert!(result.is_some());
        let (name, kind, q) = result.unwrap();
        assert_eq!(name, "pg");
        assert_eq!(kind, crate::connection::DatabaseKind::Postgres);
        assert_eq!(q.sources.len(), 1);
    }

    #[test]
    fn collect_single_db_with_filter() {
        let source = Source {
            name: "users".into(),
            alias: None,
            connection: None,
            kind: SourceKind::Table("users".into()),
        };
        let scan = PlanNode::Scan {
            source,
            database: Some(("pg".into(), crate::connection::DatabaseKind::Postgres)),
            filter: None,
        };
        let filter_node = PlanNode::Filter {
            input: Box::new(scan),
            condition: Expression::BinaryOp {
                op: BinaryOp::Gt,
                left: Box::new(Expression::Ident("age".into())),
                right: Box::new(Expression::Integer(21)),
            },
        };
        let (_, _, q) = collect_single_db_query(&filter_node).unwrap();
        assert!(q.filter.is_some());
    }

    #[test]
    fn collect_single_db_rejects_cross_db_join() {
        let left = PlanNode::Scan {
            source: Source {
                name: "a".into(),
                alias: None,
                connection: None,
                kind: SourceKind::Table("t1".into()),
            },
            database: Some(("pg".into(), crate::connection::DatabaseKind::Postgres)),
            filter: None,
        };
        let right = PlanNode::Scan {
            source: Source {
                name: "b".into(),
                alias: None,
                connection: None,
                kind: SourceKind::Table("t2".into()),
            },
            database: Some(("mongo".into(), crate::connection::DatabaseKind::MongoDB)),
            filter: None,
        };
        let join = PlanNode::Join {
            left: Box::new(left),
            right: Box::new(right),
            condition: Expression::Boolean(true),
            strategy: crate::engine::planner::JoinStrategy::Hash,
            join_kind: JoinKind::Inner,
        };
        assert!(collect_single_db_query(&join).is_none());
    }

    // ── translate_for_kind ─────────────────────────────────────────────

    #[test]
    fn translate_postgres() {
        let q = Query {
            sources: vec![Source {
                name: "users".into(),
                alias: None,
                connection: None,
                kind: SourceKind::Table("users".into()),
            }],
            ..Default::default()
        };
        let sql = translate_for_kind(&q, &crate::connection::DatabaseKind::Postgres);
        assert_eq!(sql, r#"SELECT * FROM "users""#);
    }

    #[test]
    fn translate_mysql() {
        let q = Query {
            sources: vec![Source {
                name: "users".into(),
                alias: None,
                connection: None,
                kind: SourceKind::Table("users".into()),
            }],
            ..Default::default()
        };
        let sql = translate_for_kind(&q, &crate::connection::DatabaseKind::MySQL);
        assert_eq!(sql, "SELECT * FROM `users`");
    }
}
