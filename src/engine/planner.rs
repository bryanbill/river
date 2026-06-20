#![allow(dead_code)]

use crate::connection::DatabaseKind;
use crate::lang::ast::*;

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
        aggs: Vec<Expression>,
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
    let database = resolve_database(source, source_db);
    if let SourceKind::Subquery(subquery) = &source.kind {
        let inner_plan = plan_query(subquery, source_db);
        PlanNode::Project {
            input: Box::new(inner_plan.root),
            fields: vec![Projection::Wildcard],
        }
    } else {
        PlanNode::Scan {
            source: source.clone(),
            database,
            filter: None,
        }
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
            let right = plan_source(&join.source, source_db);
            root = PlanNode::Join {
                left: Box::new(root),
                right: Box::new(right),
                condition: join
                    .condition
                    .clone()
                    .unwrap_or(Expression::Boolean(true)),
                strategy: match join.kind {
                    JoinKind::Inner => JoinStrategy::Hash,
                    JoinKind::Left => JoinStrategy::Hash,
                    JoinKind::Right => JoinStrategy::Hash,
                    JoinKind::Full => JoinStrategy::Hash,
                    JoinKind::Cross => JoinStrategy::NestedLoop,
                },
                join_kind: join.kind,
            };
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
        let aggs: Vec<Expression> = query
            .projection
            .iter()
            .filter_map(|p| match p {
                Projection::Expr(e, _) if matches!(e, Expression::Aggregate { .. }) => {
                    Some(e.clone())
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
        let has_aggregate = query
            .projection
            .iter()
            .any(|p| matches!(p, Projection::Expr(Expression::Aggregate { .. }, _)));

        let fields = if has_aggregate {
            query
                .projection
                .iter()
                .filter(|p| !matches!(p, Projection::Expr(Expression::Aggregate { .. }, _)))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            query.projection.clone()
        };

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
        PlanNode::Empty => {}
    }
}
