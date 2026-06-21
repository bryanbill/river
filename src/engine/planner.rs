#![allow(dead_code)]

use crate::connection::DatabaseKind;
use crate::lang::ast::*;

pub const CROSS_DB_BATCH_SIZE: usize = 1000;

#[derive(Debug, Clone, PartialEq)]
pub enum JoinStrategy {
    NestedLoop,
    Hash,
    Merge,
    Auto,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlanNode {
    Scan {
        source: Source,
        database: Option<(String, DatabaseKind)>,
        filter: Option<Expression>,
    },
    Join {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
        condition: Expression,
        strategy: JoinStrategy,
        join_kind: JoinKind,
    },
    Project {
        input: Box<PlanNode>,
        fields: Vec<Projection>,
    },
    Aggregate {
        input: Box<PlanNode>,
        group_by: Vec<Expression>,
        aggs: Vec<(Expression, Option<String>)>,
    },
    Limit {
        input: Box<PlanNode>,
        limit: u64,
        offset: u64,
    },
    Order {
        input: Box<PlanNode>,
        order_by: Vec<OrderBy>,
    },
    Distinct {
        input: Box<PlanNode>,
    },
    Filter {
        input: Box<PlanNode>,
        condition: Expression,
    },
    Union {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
        all: bool,
    },
    SemiJoinFetch {
        build: Box<PlanNode>,
        probe_source: Source,
        probe_database: (String, DatabaseKind),
        build_key: Expression,
        probe_key: Expression,
        join_kind: JoinKind,
        condition: Expression,
    },
    InlineData {
        columns: Vec<String>,
        rows: Vec<Vec<crate::adapters::Value>>,
    },
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryPlan {
    pub root: PlanNode,
}

fn resolve_database(
    source: &Source,
    source_db: &[(String, DatabaseKind)],
) -> Option<(String, DatabaseKind)> {
    if let Some(conn_name) = &source.connection {
        source_db
            .iter()
            .find(|(name, _)| name == conn_name)
            .cloned()
    } else {
        source_db.first().cloned()
    }
}

fn plan_source(source: &Source, source_db: &[(String, DatabaseKind)]) -> PlanNode {
    if let SourceKind::Subquery(subquery) = &source.kind {
        let inner_plan = plan_query(subquery, source_db);
        return PlanNode::Project {
            input: Box::new(inner_plan.root),
            fields: vec![Projection::Wildcard],
        };
    }
    let database = if matches!(&source.kind, SourceKind::CteRef(_)) {
        None
    } else {
        resolve_database(source, source_db)
    };
    PlanNode::Scan {
        source: source.clone(),
        database,
        filter: None,
    }
}

fn extract_equi_keys(condition: &Expression) -> Option<(Expression, Expression)> {
    match condition {
        Expression::BinaryOp {
            op: BinaryOp::Eq,
            left,
            right,
        } => Some((*left.clone(), *right.clone())),
        _ => None,
    }
}

fn key_belongs_to_source(key: &Expression, source: &Source) -> bool {
    match key {
        Expression::QualifiedIdent { table, .. } => {
            source.alias.as_deref() == Some(table) || source.name == *table
        }
        Expression::Ident(_) => true,
        _ => false,
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

pub fn plan_query(query: &Query, source_db: &[(String, DatabaseKind)]) -> QueryPlan {
    let mut current: PlanNode = if query.sources.is_empty() {
        PlanNode::Empty
    } else {
        let mut root = plan_source(&query.sources[0], source_db);

        for source in &query.sources[1..] {
            root = PlanNode::Join {
                left: Box::new(root),
                right: Box::new(plan_source(source, source_db)),
                condition: Expression::Boolean(true),
                strategy: JoinStrategy::Auto,
                join_kind: JoinKind::Cross,
            };
        }

        for join in &query.joins {
            let right_node = plan_source(&join.source, source_db);
            let left_db = find_single_db(&root);
            let right_db = match &right_node {
                PlanNode::Scan { database, .. } => database.clone(),
                _ => None,
            };

            let is_cross_db_join = match (&left_db, &right_db) {
                (Some((ln, _)), Some((rn, _))) => ln != rn,
                _ => false,
            };

            if is_cross_db_join {
                let condition = join.condition.clone().unwrap_or(Expression::Boolean(true));
                let is_cross_join = join.kind == JoinKind::Cross
                    || matches!(&condition, Expression::Boolean(true));
                let equi_keys = extract_equi_keys(&condition);

                if let Some((left_key, right_key)) = equi_keys {
                    let (probe_source, probe_database) = match &right_node {
                        PlanNode::Scan { source, database, .. } => {
                            (source.clone(), database.clone().unwrap())
                        }
                        _ => {
                            root = PlanNode::Join {
                                left: Box::new(root),
                                right: Box::new(right_node),
                                condition,
                                strategy: JoinStrategy::Hash,
                                join_kind: join.kind,
                            };
                            continue;
                        }
                    };

                    let (build_key, probe_key) =
                        if key_belongs_to_source(&right_key, &probe_source) {
                            (left_key, right_key)
                        } else {
                            (right_key, left_key)
                        };

                    root = PlanNode::SemiJoinFetch {
                        build: Box::new(root),
                        probe_source,
                        probe_database,
                        build_key,
                        probe_key,
                        join_kind: join.kind,
                        condition,
                    };
                } else if is_cross_join {
                    // Cross-DB cross join - executor will check for LIMIT
                    root = PlanNode::Join {
                        left: Box::new(root),
                        right: Box::new(right_node),
                        condition,
                        strategy: JoinStrategy::NestedLoop,
                        join_kind: join.kind,
                    };
                } else {
                    root = PlanNode::Join {
                        left: Box::new(root),
                        right: Box::new(right_node),
                        condition,
                        strategy: JoinStrategy::NestedLoop,
                        join_kind: join.kind,
                    };
                }
            } else {
                root = PlanNode::Join {
                    left: Box::new(root),
                    right: Box::new(right_node),
                    condition: join
                        .condition
                        .clone()
                        .unwrap_or(Expression::Boolean(true)),
                    strategy: match join.kind {
                        JoinKind::Cross => JoinStrategy::NestedLoop,
                        _ => JoinStrategy::Hash,
                    },
                    join_kind: join.kind,
                };
            }
        }

        root
    };

    if let Some(filter) = &query.filter {
        current = PlanNode::Filter {
            input: Box::new(current),
            condition: filter.clone(),
        };
    }

    if !query.group_by.is_empty()
        || query
            .projection
            .iter()
            .any(|p| matches!(p, Projection::Expr(Expression::Aggregate { .. }, _)))
    {
        let aggs: Vec<(Expression, Option<String>)> = query
            .projection
            .iter()
            .filter_map(|p| match p {
                Projection::Expr(e, alias) if matches!(e, Expression::Aggregate { .. }) => {
                    Some((e.clone(), alias.clone()))
                }
                _ => None,
            })
            .collect();

        current = PlanNode::Aggregate {
            input: Box::new(current),
            group_by: query.group_by.clone(),
            aggs,
        };
    }

    if let Some(having) = &query.having {
        current = PlanNode::Filter {
            input: Box::new(current),
            condition: having.clone(),
        };
    }

    if query.distinct {
        current = PlanNode::Distinct {
            input: Box::new(current),
        };
    }

    if !query.projection.is_empty() {
        let fields = query.projection.clone();

        if !fields.is_empty() {
            current = PlanNode::Project {
                input: Box::new(current),
                fields,
            };
        }
    }

    if !query.order_by.is_empty() {
        current = PlanNode::Order {
            input: Box::new(current),
            order_by: query.order_by.clone(),
        };
    }

    if query.limit.is_some() || query.offset.is_some() {
        current = PlanNode::Limit {
            input: Box::new(current),
            limit: query.limit.unwrap_or(u64::MAX),
            offset: query.offset.unwrap_or(0),
        };
    }

    QueryPlan { root: current }
}

pub fn plan_statement(
    stmt: &Statement,
    source_db: &[(String, DatabaseKind)],
) -> QueryPlan {
    match stmt {
        Statement::Query(q) => plan_query(q, source_db),
        Statement::Insert(_)
        | Statement::Update(_)
        | Statement::Delete(_)
        | Statement::With(_)
        | Statement::SetOp(_)
        | Statement::Explain(_)
        | Statement::Describe(_)
        | Statement::ShowTables(_)
        | Statement::ParamAssign { .. }
        | Statement::Noop => QueryPlan {
            root: PlanNode::Empty,
        },
    }
}

/// Returns true if the plan involves databases of different kinds
pub fn is_cross_db(node: &PlanNode) -> bool {
    let dbs = find_all_databases(node);
    let kinds: std::collections::HashSet<&DatabaseKind> = dbs.iter().map(|(_, k)| k).collect();
    kinds.len() > 1
}

/// Collects all distinct databases referenced in a plan tree
pub fn find_all_databases(node: &PlanNode) -> Vec<(String, DatabaseKind)> {
    let mut dbs = Vec::new();
    collect_databases(node, &mut dbs);
    dbs.sort_by(|a, b| a.0.cmp(&b.0));
    dbs.dedup_by(|a, b| a.0 == b.0);
    dbs
}

fn collect_databases(node: &PlanNode, out: &mut Vec<(String, DatabaseKind)>) {
    match node {
        PlanNode::Scan { database, .. } => {
            if let Some(db) = database {
                out.push(db.clone());
            }
        }
        PlanNode::Filter { input, .. }
        | PlanNode::Project { input, .. }
        | PlanNode::Order { input, .. }
        | PlanNode::Limit { input, .. }
        | PlanNode::Aggregate { input, .. }
        | PlanNode::Distinct { input, .. } => {
            collect_databases(input, out);
        }
        PlanNode::Join { left, right, .. }
        | PlanNode::Union { left, right, .. } => {
            collect_databases(left, out);
            collect_databases(right, out);
        }
        PlanNode::SemiJoinFetch { build, probe_database, .. } => {
            collect_databases(build, out);
            out.push(probe_database.clone());
        }
        PlanNode::InlineData { .. } | PlanNode::Empty => {}
    }
}
