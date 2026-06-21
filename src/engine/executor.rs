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
    let has_limit = plan_has_limit(&plan.root);
    let mut result = execute_node(&plan.root, adapters, has_limit).await?;
    result.elapsed = start.elapsed();
    Ok(result)
}

/// Returns true if a Limit node exists anywhere in the plan tree
fn plan_has_limit(node: &PlanNode) -> bool {
    match node {
        PlanNode::Limit { .. } => true,
        PlanNode::Filter { input, .. }
        | PlanNode::Project { input, .. }
        | PlanNode::Order { input, .. }
        | PlanNode::Aggregate { input, .. }
        | PlanNode::Distinct { input, .. } => plan_has_limit(input),
        PlanNode::Join { left, right, .. }
        | PlanNode::Union { left, right, .. } => plan_has_limit(left) || plan_has_limit(right),
        PlanNode::SemiJoinFetch { build, .. } => plan_has_limit(build),
        PlanNode::Scan { .. } | PlanNode::InlineData { .. } | PlanNode::Empty => false,
    }
}

pub async fn execute_statement(
    stmt: &Statement,
    source_db: &[(String, DatabaseKind)],
    adapters: &HashMap<String, Box<dyn DatabaseAdapter>>,
) -> Result<QueryResult, RiverError> {
    match stmt {
        Statement::With(w) => {
            let mut cte_data: HashMap<String, QueryResult> = HashMap::new();
            for cte in &w.ctes {
                let cte_stmt = Statement::Query(*cte.query.clone());
                let mut plan = crate::engine::planner::plan_statement(&cte_stmt, source_db);
                replace_cte_scans(&mut plan.root, &cte_data);
                let result = execute_plan(&plan, adapters).await?;
                cte_data.insert(cte.name.clone(), result);
            }
            let mut plan = crate::engine::planner::plan_statement(w.body.as_ref(), source_db);
            replace_cte_scans(&mut plan.root, &cte_data);
            execute_plan(&plan, adapters).await
        }
        _ => {
            let plan = crate::engine::planner::plan_statement(stmt, source_db);
            execute_plan(&plan, adapters).await
        }
    }
}

fn replace_cte_scans(node: &mut PlanNode, cte_data: &HashMap<String, QueryResult>) {
    match node {
        PlanNode::Scan { source, database, .. } if database.is_none() => {
            if let SourceKind::CteRef(name) = &source.kind {
                if let Some(data) = cte_data.get(name) {
                    *node = PlanNode::InlineData {
                        columns: data.columns.clone(),
                        rows: data.rows.clone(),
                    };
                }
            }
        }
        PlanNode::Filter { input, .. }
        | PlanNode::Project { input, .. }
        | PlanNode::Order { input, .. }
        | PlanNode::Limit { input, .. }
        | PlanNode::Aggregate { input, .. }
        | PlanNode::Distinct { input, .. } => {
            replace_cte_scans(input, cte_data);
        }
        PlanNode::Join { left, right, .. }
        | PlanNode::Union { left, right, .. } => {
            replace_cte_scans(left, cte_data);
            replace_cte_scans(right, cte_data);
        }
        PlanNode::SemiJoinFetch { build, .. } => {
            replace_cte_scans(build, cte_data);
        }
        _ => {}
    }
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
        PlanNode::SemiJoinFetch { .. } => None,
        PlanNode::InlineData { .. } | PlanNode::Empty => None,
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
            for (agg, alias) in aggs {
                q.projection.push(Projection::Expr(agg.clone(), alias.clone()));
            }
            Some((db_name, db_kind, q))
        }
        PlanNode::Distinct { input } => {
            let (db_name, db_kind, mut q) = collect_single_db_query(input)?;
            q.distinct = true;
            Some((db_name, db_kind, q))
        }
        PlanNode::Join { .. } | PlanNode::Union { .. } | PlanNode::SemiJoinFetch { .. } | PlanNode::InlineData { .. } | PlanNode::Empty => None,
    }
}

async fn execute_node(
    node: &PlanNode,
    adapters: &HashMap<String, Box<dyn DatabaseAdapter>>,
    bounded: bool,
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
            // Guard: reject unbounded cross-DB cross joins (unless a LIMIT exists in the plan)
            if !bounded
                && (*join_kind == JoinKind::Cross
                    || matches!(condition, Expression::Boolean(true)))
            {
                let left_db = find_single_db(left);
                let right_db = find_single_db(right);
                if let (Some((ln, _)), Some((rn, _))) = (&left_db, &right_db) {
                    if ln != rn {
                        return Err(RiverError::Unsupported(
                            "Cross-database cross joins require a LIMIT clause to prevent \
                             unbounded result sets. Add 'limit N' to your query."
                                .into(),
                        ));
                    }
                }
            }
            let left_fut = Box::pin(execute_node(left, adapters, bounded));
            let right_fut = Box::pin(execute_node(right, adapters, bounded));
            let (lr, rr) = tokio::join!(left_fut, right_fut);
            let left_result = lr?;
            let right_result = rr?;
            join_results(left_result, right_result, condition, strategy, *join_kind)
        }
        PlanNode::Filter { input, condition } => {
            let result = Box::pin(execute_node(input, adapters, bounded)).await?;
            apply_filter(&result, condition)
        }
        PlanNode::Project { input, fields } => {
            let result = Box::pin(execute_node(input, adapters, bounded)).await?;
            apply_projection(&result, fields)
        }
        PlanNode::Order { input, order_by } => {
            let result = Box::pin(execute_node(input, adapters, bounded)).await?;
            apply_order(&result, order_by)
        }
        PlanNode::Limit { input, limit, offset } => {
            let result = Box::pin(execute_node(input, adapters, true)).await?;
            apply_limit(&result, *limit, *offset)
        }
        PlanNode::Aggregate { input, group_by, aggs } => {
            let result = Box::pin(execute_node(input, adapters, bounded)).await?;
            apply_aggregate(&result, group_by, aggs)
        }
        PlanNode::Distinct { input } => {
            let result = Box::pin(execute_node(input, adapters, bounded)).await?;
            apply_distinct(&result)
        }
        PlanNode::Union { left, right, all } => {
            let lf = Box::pin(execute_node(left, adapters, bounded));
            let rf = Box::pin(execute_node(right, adapters, bounded));
            let (lr, rr) = tokio::join!(lf, rf);
            union_results(lr?, rr?, *all)
        }
        PlanNode::InlineData { columns, rows } => Ok(QueryResult {
            columns: columns.clone(),
            rows: rows.clone(),
            elapsed: std::time::Duration::default(),
            rows_affected: rows.len() as u64,
        }),
        PlanNode::SemiJoinFetch {
            build,
            probe_source,
            probe_database,
            build_key,
            probe_key,
            join_kind,
            condition,
        } => {
            execute_semi_join_fetch(
                build, probe_source, probe_database, build_key, probe_key,
                *join_kind, condition, adapters,
            ).await
        }
        PlanNode::Empty => Ok(empty_result()),
        PlanNode::Scan { source, .. } => {
            if let SourceKind::CteRef(_) = &source.kind {
                Err(RiverError::Unsupported(
                    format!("CTE \"{}\" was not resolved before execution", source.name),
                ))
            } else {
                Err(RiverError::Unsupported(
                    "no database configured — create a river.yaml file with connections".into(),
                ))
            }
        }
    }
}

async fn execute_semi_join_fetch(
    build: &PlanNode,
    probe_source: &Source,
    probe_database: &(String, DatabaseKind),
    build_key: &Expression,
    probe_key: &Expression,
    join_kind: JoinKind,
    condition: &Expression,
    adapters: &HashMap<String, Box<dyn DatabaseAdapter>>,
) -> Result<QueryResult, RiverError> {
    use crate::engine::planner::CROSS_DB_BATCH_SIZE;

    // 1. Execute the build side
    let build_result = Box::pin(execute_node(build, adapters, false)).await?;

    if build_result.rows.is_empty() {
        return Ok(empty_result());
    }

    // 2. Extract distinct join keys from build result
    let build_key_col = match build_key {
        Expression::Ident(name) => name.clone(),
        Expression::QualifiedIdent { field, .. } => field.clone(),
        _ => {
            return Err(RiverError::Unsupported(
                "SemiJoinFetch requires a simple column reference as build key".into(),
            ));
        }
    };
    let probe_key_col = match probe_key {
        Expression::Ident(name) => name.clone(),
        Expression::QualifiedIdent { field, .. } => field.clone(),
        _ => {
            return Err(RiverError::Unsupported(
                "SemiJoinFetch requires a simple column reference as probe key".into(),
            ));
        }
    };

    let build_key_idx = build_result
        .columns
        .iter()
        .position(|c| c == &build_key_col)
        .ok_or_else(|| {
            RiverError::Unsupported(format!(
                "Build key column '{}' not found in build result columns: {:?}",
                build_key_col, build_result.columns
            ))
        })?;

    let mut distinct_keys: Vec<Value> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for row in &build_result.rows {
        let key = row.get(build_key_idx).cloned().unwrap_or(Value::Null);
        if key != Value::Null && seen.insert(key.clone()) {
            distinct_keys.push(key);
        }
    }

    if distinct_keys.is_empty() {
        return Ok(empty_result());
    }

    // 3. Batch-fetch from probe side
    let (db_name, db_kind) = probe_database;
    let adapter = adapters.get(db_name).ok_or_else(|| {
        RiverError::Unsupported(format!("no adapter connected for '{}'", db_name))
    })?;

    let table_name = match &probe_source.kind {
        SourceKind::Table(t) => t.clone(),
        _ => probe_source.name.clone(),
    };

    let mut probe_rows: Vec<Vec<Value>> = Vec::new();
    let mut probe_columns: Vec<String> = Vec::new();

    for chunk in distinct_keys.chunks(CROSS_DB_BATCH_SIZE) {
        let native_query = match db_kind {
            DatabaseKind::MongoDB => {
                let pipeline = crate::engine::translator::build_probe_query_mongo(
                    &table_name, &probe_key_col, chunk, "",
                );
                serde_json::to_string(&pipeline).unwrap_or_default()
            }
            _ => {
                let dialect: Box<dyn crate::engine::translator::SqlDialect> = match db_kind {
                    DatabaseKind::Postgres => Box::new(crate::engine::translator::PostgresDialect),
                    DatabaseKind::MySQL => Box::new(crate::engine::translator::MySQLDialect),
                    DatabaseKind::MSSQL => Box::new(crate::engine::translator::MSSQLDialect),
                    DatabaseKind::SQLite => Box::new(crate::engine::translator::SQLiteDialect),
                    DatabaseKind::MongoDB => unreachable!(),
                };
                crate::engine::translator::build_probe_query_sql(
                    &table_name, &probe_key_col, chunk, dialect.as_ref(),
                )
            }
        };

        let batch_result = adapter.execute(&native_query).await?;
        if probe_columns.is_empty() {
            probe_columns = batch_result.columns.clone();
        }
        probe_rows.extend(batch_result.rows);
    }

    let probe_result = QueryResult {
        columns: probe_columns,
        rows: probe_rows,
        elapsed: std::time::Duration::default(),
        rows_affected: 0,
    };

    // 4. Hash join the build and probe results
    hash_join(build_result, probe_result, condition, join_kind)
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
                    Expression::Aggregate { .. } => agg_default_name(expr),
                    _ => "expr".to_string(),
                });
                let idx = match expr {
                    Expression::Ident(n) | Expression::QualifiedIdent { field: n, .. } => {
                        result.columns.iter().position(|c| c == n)
                    }
                    Expression::Aggregate { .. } => {
                        result.columns.iter().position(|c| c == &name)
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
    group_by: &[Expression],
    aggs: &[(Expression, Option<String>)],
) -> Result<QueryResult, RiverError> {
    if aggs.is_empty() && group_by.is_empty() {
        return Ok(result.clone());
    }

    // Build output column names
    let mut out_columns: Vec<String> = Vec::new();
    for gb in group_by {
        let name = match gb {
            Expression::Ident(n) => n.clone(),
            Expression::QualifiedIdent { field, .. } => field.clone(),
            _ => "expr".to_string(),
        };
        out_columns.push(name);
    }
    for (agg_expr, alias) in aggs {
        let name = alias.clone().unwrap_or_else(|| agg_default_name(agg_expr));
        out_columns.push(name);
    }

    // Group rows by group-by key values
    let mut groups: HashMap<Vec<Value>, Vec<Vec<Value>>> = HashMap::new();
    let mut group_order: Vec<Vec<Value>> = Vec::new();

    for row in &result.rows {
        let key: Vec<Value> = group_by
            .iter()
            .map(|gb| eval_expr(gb, &result.columns, row))
            .collect();
        if !groups.contains_key(&key) {
            group_order.push(key.clone());
        }
        groups.entry(key).or_default().push(row.clone());
    }

    // Compute aggregates per group
    let mut out_rows: Vec<Vec<Value>> = Vec::new();
    for key in &group_order {
        let group_rows = groups.get(key).unwrap();
        let mut out_row: Vec<Value> = key.clone();
        for (agg_expr, _) in aggs {
            out_row.push(compute_aggregate(agg_expr, &result.columns, group_rows));
        }
        out_rows.push(out_row);
    }

    // Global aggregate (no group-by): produce a single row even if input is empty
    if group_by.is_empty() && !aggs.is_empty() && out_rows.is_empty() {
        let mut out_row: Vec<Value> = Vec::new();
        for (agg_expr, _) in aggs {
            out_row.push(compute_aggregate(agg_expr, &result.columns, &[]));
        }
        out_rows.push(out_row);
    }

    Ok(QueryResult {
        columns: out_columns,
        rows: out_rows,
        elapsed: std::time::Duration::default(),
        rows_affected: 0,
    })
}

fn agg_default_name(expr: &Expression) -> String {
    match expr {
        Expression::Aggregate { name, args, .. } => {
            if args.is_empty() {
                format!("{}(*)", name)
            } else if let Expression::Ident(col) = &args[0] {
                format!("{}({})", name, col)
            } else {
                format!("{}(...)", name)
            }
        }
        _ => "expr".to_string(),
    }
}

fn compute_aggregate(expr: &Expression, columns: &[String], rows: &[Vec<Value>]) -> Value {
    match expr {
        Expression::Aggregate {
            name,
            distinct,
            args,
        } => {
            let arg_values: Vec<Value> = if args.is_empty() {
                rows.iter().map(|_| Value::Int(1)).collect()
            } else {
                rows.iter()
                    .map(|r| eval_expr(&args[0], columns, r))
                    .collect()
            };

            match name.as_str() {
                "count" => {
                    if *distinct {
                        let mut seen = std::collections::HashSet::new();
                        for v in &arg_values {
                            seen.insert(v.clone());
                        }
                        Value::Int(seen.len() as i64)
                    } else {
                        Value::Int(arg_values.len() as i64)
                    }
                }
                "count_distinct" => {
                    let mut seen = std::collections::HashSet::new();
                    for v in &arg_values {
                        seen.insert(v.clone());
                    }
                    Value::Int(seen.len() as i64)
                }
                "sum" => {
                    let vals: Vec<f64> = arg_values.iter().filter_map(to_f64).collect();
                    if vals.is_empty() {
                        Value::Null
                    } else if arg_values.iter().all(|v| matches!(v, Value::Int(_))) {
                        Value::Int(vals.iter().sum::<f64>() as i64)
                    } else {
                        Value::Float(vals.iter().sum())
                    }
                }
                "avg" => {
                    let vals: Vec<f64> = arg_values.iter().filter_map(to_f64).collect();
                    if vals.is_empty() {
                        Value::Null
                    } else {
                        Value::Float(vals.iter().sum::<f64>() / vals.len() as f64)
                    }
                }
                "min" => arg_values
                    .iter()
                    .min_by(|a, b| cmp_values(a, b))
                    .cloned()
                    .unwrap_or(Value::Null),
                "max" => arg_values
                    .iter()
                    .max_by(|a, b| cmp_values(a, b))
                    .cloned()
                    .unwrap_or(Value::Null),
                _ => Value::Null,
            }
        }
        _ => Value::Null,
    }
}

fn to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
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

    // ── Chained CTE with in-memory aggregation ────────────────────────

    use crate::adapters::{DatabaseAdapter, QueryResult, TableInfo, TableSchema};
    use crate::connection::{ConnectionConfig, DatabaseKind};
    use crate::lang::parse;
    use async_trait::async_trait;

    struct MockAdapter;

    #[async_trait]
    impl DatabaseAdapter for MockAdapter {
        async fn connect(_config: &ConnectionConfig) -> Result<Self, RiverError> {
            Ok(MockAdapter)
        }

        async fn execute(&self, query: &str) -> Result<QueryResult, RiverError> {
            if query.contains("orders") {
                Ok(QueryResult {
                    columns: vec![
                        "id".into(),
                        "user_id".into(),
                        "total".into(),
                        "status".into(),
                    ],
                    rows: vec![
                        vec![Value::Int(1), Value::Int(1), Value::Int(500), Value::String("paid".into())],
                        vec![Value::Int(2), Value::Int(1), Value::Int(600), Value::String("paid".into())],
                        vec![Value::Int(3), Value::Int(2), Value::Int(200), Value::String("paid".into())],
                        vec![Value::Int(4), Value::Int(3), Value::Int(1500), Value::String("paid".into())],
                        vec![Value::Int(5), Value::Int(3), Value::Int(300), Value::String("paid".into())],
                    ],
                    elapsed: std::time::Duration::default(),
                    rows_affected: 0,
                })
            } else if query.contains("users") {
                Ok(QueryResult {
                    columns: vec!["id".into(), "name".into()],
                    rows: vec![
                        vec![Value::Int(1), Value::String("Alice".into())],
                        vec![Value::Int(2), Value::String("Bob".into())],
                        vec![Value::Int(3), Value::String("Carol".into())],
                    ],
                    elapsed: std::time::Duration::default(),
                    rows_affected: 0,
                })
            } else {
                Ok(QueryResult {
                    columns: vec![],
                    rows: vec![],
                    elapsed: std::time::Duration::default(),
                    rows_affected: 0,
                })
            }
        }

        async fn list_tables(&self) -> Result<Vec<TableInfo>, RiverError> {
            Ok(vec![])
        }
        async fn describe_table(&self, _table: &str) -> Result<TableSchema, RiverError> {
            Ok(TableSchema {
                name: "mock".into(),
                columns: vec![],
            })
        }
        fn dialect(&self) -> DatabaseKind {
            DatabaseKind::SQLite
        }
    }

    #[tokio::test]
    async fn chained_cte_with_in_memory_aggregation() {
        let mut adapters: HashMap<String, Box<dyn DatabaseAdapter>> = HashMap::new();
        adapters.insert("test".into(), Box::new(MockAdapter));
        let source_db = vec![("test".into(), DatabaseKind::SQLite)];

        let query = r#"with
  paid_orders as ( find * from orders where status = "paid" ),
  user_totals as ( find [user_id, sum(total) as revenue] from paid_orders group by user_id )
find [u.name, ut.revenue]
from users as u
join user_totals as ut on u.id = ut.user_id
where ut.revenue > 1000
order by ut.revenue desc"#;

        let stmt = parse(query).expect("parse failed");
        let result = execute_statement(&stmt, &source_db, &adapters)
            .await
            .expect("execution failed");

        assert_eq!(result.columns, vec!["name", "revenue"]);
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0][0], Value::String("Carol".into()));
        assert_eq!(result.rows[0][1], Value::Int(1800));
        assert_eq!(result.rows[1][0], Value::String("Alice".into()));
        assert_eq!(result.rows[1][1], Value::Int(1100));
    }

    #[tokio::test]
    async fn single_cte_resolution() {
        let mut adapters: HashMap<String, Box<dyn DatabaseAdapter>> = HashMap::new();
        adapters.insert("test".into(), Box::new(MockAdapter));
        let source_db = vec![("test".into(), DatabaseKind::SQLite)];

        let query = r#"with paid_orders as ( find * from orders where status = "paid" ) find [id, total] from paid_orders"#;

        let stmt = parse(query).expect("parse failed");
        let result = execute_statement(&stmt, &source_db, &adapters)
            .await
            .expect("execution failed");

        assert_eq!(result.columns, vec!["id", "total"]);
        assert_eq!(result.rows.len(), 5);
    }

    #[tokio::test]
    async fn in_memory_aggregate_sum() {
        let mut adapters: HashMap<String, Box<dyn DatabaseAdapter>> = HashMap::new();
        adapters.insert("test".into(), Box::new(MockAdapter));
        let source_db = vec![("test".into(), DatabaseKind::SQLite)];

        let query = r#"with all_orders as ( find * from orders ) find [user_id, sum(total) as revenue] from all_orders group by user_id"#;

        let stmt = parse(query).expect("parse failed");
        let result = execute_statement(&stmt, &source_db, &adapters)
            .await
            .expect("execution failed");

        assert_eq!(result.columns, vec!["user_id", "revenue"]);
        assert_eq!(result.rows.len(), 3);
        let by_user: HashMap<i64, i64> = result
            .rows
            .iter()
            .map(|r| match (&r[0], &r[1]) {
                (Value::Int(u), Value::Int(v)) => (*u, *v),
                _ => panic!("expected int values, got {:?}", r),
            })
            .collect();
        assert_eq!(by_user.get(&1), Some(&1100));
        assert_eq!(by_user.get(&2), Some(&200));
        assert_eq!(by_user.get(&3), Some(&1800));
    }

    #[tokio::test]
    async fn in_memory_aggregate_count() {
        let mut adapters: HashMap<String, Box<dyn DatabaseAdapter>> = HashMap::new();
        adapters.insert("test".into(), Box::new(MockAdapter));
        let source_db = vec![("test".into(), DatabaseKind::SQLite)];

        let query = r#"with all_orders as ( find * from orders ) find [count(*) as cnt] from all_orders"#;

        let stmt = parse(query).expect("parse failed");
        let result = execute_statement(&stmt, &source_db, &adapters)
            .await
            .expect("execution failed");

        assert_eq!(result.columns, vec!["cnt"]);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], Value::Int(5));
    }

    #[tokio::test]
    async fn cross_db_cross_join_without_limit_rejected() {
        let mut adapters: HashMap<String, Box<dyn DatabaseAdapter>> = HashMap::new();
        adapters.insert("pg".into(), Box::new(MockAdapter));
        adapters.insert("mysql".into(), Box::new(MockAdapter));
        let source_db = vec![
            ("pg".into(), DatabaseKind::Postgres),
            ("mysql".into(), DatabaseKind::MySQL),
        ];

        let query =
            r#"find [u.name, o.total] from users@pg as u cross join orders@mysql as o"#;
        let stmt = parse(query).expect("parse failed");
        let result = execute_statement(&stmt, &source_db, &adapters).await;
        assert!(
            result.is_err(),
            "Expected error for unbounded cross-DB cross join"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("LIMIT") || err_msg.contains("limit"),
            "Expected error about LIMIT, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn cross_db_cross_join_with_limit_allowed() {
        let mut adapters: HashMap<String, Box<dyn DatabaseAdapter>> = HashMap::new();
        adapters.insert("pg".into(), Box::new(MockAdapter));
        adapters.insert("mysql".into(), Box::new(MockAdapter));
        let source_db = vec![
            ("pg".into(), DatabaseKind::Postgres),
            ("mysql".into(), DatabaseKind::MySQL),
        ];

        let query =
            r#"find [u.name, o.total] from users@pg as u cross join orders@mysql as o limit 5"#;
        let stmt = parse(query).expect("parse failed");
        let result = execute_statement(&stmt, &source_db, &adapters).await;
        assert!(
            result.is_ok(),
            "cross-DB cross join with LIMIT should work: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn semi_join_fetch_equi_join_cross_db() {
        let mut adapters: HashMap<String, Box<dyn DatabaseAdapter>> = HashMap::new();
        adapters.insert("pg".into(), Box::new(MockAdapter));
        adapters.insert("mysql".into(), Box::new(MockAdapter));
        let source_db = vec![
            ("pg".into(), DatabaseKind::Postgres),
            ("mysql".into(), DatabaseKind::MySQL),
        ];

        let query =
            r#"find [u.name, o.total] from users@pg as u join orders@mysql as o on u.id = o.user_id"#;
        let stmt = parse(query).expect("parse failed");
        let result = execute_statement(&stmt, &source_db, &adapters).await;
        assert!(
            result.is_ok(),
            "SemiJoinFetch should execute successfully: {:?}",
            result.err()
        );
        let qr = result.unwrap();
        assert!(!qr.rows.is_empty(), "Should have join results");
    }
}
