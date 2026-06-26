use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Query(Query),
    Insert(Insert),
    Update(Update),
    Delete(Delete),
    With(With),
    SetOp(SetOp),
    Explain(Box<Statement>),
    Describe(Describe),
    ShowTables(Option<String>),
    CreateTable(CreateTable),
    CreateTableAs(CreateTableAs),
    AlterTable(AlterTable),
    DropTable(DropTable),
    ParamAssign {
        name: String,
        value: Expression,
    },
    Noop,
}

#[derive(Debug, Clone, PartialEq)]
pub struct With {
    pub recursive: bool,
    pub ctes: Vec<Cte>,
    pub body: Box<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cte {
    pub name: String,
    pub columns: Option<Vec<String>>,
    pub query: Box<Query>,
    pub chain: Vec<(SetOpKind, Query)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetOp {
    pub kind: SetOpKind,
    pub left: Box<Query>,
    pub right: Box<Query>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetOpKind {
    Union,
    UnionAll,
    Intersect,
    Except,
}

#[derive(Debug, Clone, PartialEq)]
#[derive(Default)]
pub struct Query {
    pub distinct: bool,
    pub projection: Vec<Projection>,
    pub sources: Vec<Source>,
    pub joins: Vec<Join>,
    pub filter: Option<Expression>,
    pub group_by: Vec<Expression>,
    pub having: Option<Expression>,
    pub window_defs: Vec<WindowDef>,
    pub order_by: Vec<OrderBy>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}


#[derive(Debug, Clone, PartialEq)]
pub enum Projection {
    Wildcard,
    QualifiedWildcard(String),
    Expr(Expression, Option<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Join {
    pub kind: JoinKind,
    pub source: Source,
    pub alias: Option<String>,
    pub condition: Option<Expression>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
    Cross,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub name: String,
    pub alias: Option<String>,
    pub connection: Option<String>,
    pub schema: Option<String>,
    pub kind: SourceKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourceKind {
    Table(String),
    Subquery(Box<Query>),
    CteRef(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowDef {
    pub name: String,
    pub spec: WindowSpec,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowSpec {
    pub partition_by: Vec<Expression>,
    pub order_by: Vec<OrderBy>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowFunction {
    RowNumber,
    Rank,
    DenseRank,
    Lag(Box<Expression>, Option<i64>),
    Lead(Box<Expression>, Option<i64>),
    FirstValue(Box<Expression>),
    LastValue(Box<Expression>),
    NthValue(Box<Expression>, u64),
    /// Arbitrary expression used as window function (e.g. avg(salary))
    Expr(Box<Expression>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderBy {
    pub expr: Expression,
    pub direction: OrderDir,
    pub nulls: NullsOrder,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderDir {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NullsOrder {
    Default,
    First,
    Last,
}

#[derive(Debug, Clone)]
pub enum Expression {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Null,
    Array(Vec<Expression>),
    Object(Vec<(String, Expression)>),

    Ident(String),
    QualifiedIdent {
        table: String,
        field: String,
    },
    QualifiedWildcard(String),

    BinaryOp {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    UnaryOp {
        op: UnaryOp,
        expr: Box<Expression>,
    },

    FnCall {
        name: String,
        args: Vec<Expression>,
    },
    Aggregate {
        name: String,
        distinct: bool,
        args: Vec<Expression>,
    },
    WindowFn {
        func: WindowFunction,
        over: WindowSpec,
        window_name: Option<String>,
    },

    Case {
        expr: Option<Box<Expression>>,
        whens: Vec<(Expression, Expression)>,
        else_expr: Option<Box<Expression>>,
    },
    Between {
        expr: Box<Expression>,
        low: Box<Expression>,
        high: Box<Expression>,
    },

    Subquery(Box<Query>),
    Exists(Box<Query>, bool),
    QuantifiedCmp {
        op: BinaryOp,
        left: Box<Expression>,
        quant: Quantifier,
        subquery: Box<Query>,
    },

    Cast {
        expr: Box<Expression>,
        target: DataType,
    },
    NamedParam(String),

    Interval {
        value: f64,
        unit: IntervalUnit,
    },
}

// Custom PartialEq because f64 doesn't implement Eq
impl PartialEq for Expression {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Expression::String(a), Expression::String(b)) => a == b,
            (Expression::Number(a), Expression::Number(b)) => a.to_bits() == b.to_bits(),
            (Expression::Integer(a), Expression::Integer(b)) => a == b,
            (Expression::Boolean(a), Expression::Boolean(b)) => a == b,
            (Expression::Null, Expression::Null) => true,
            (Expression::Array(a), Expression::Array(b)) => a == b,
            (Expression::Object(a), Expression::Object(b)) => a == b,
            (Expression::Ident(a), Expression::Ident(b)) => a == b,
            (Expression::QualifiedIdent { table: t1, field: f1 }, Expression::QualifiedIdent { table: t2, field: f2 }) => {
                t1 == t2 && f1 == f2
            }
            (Expression::QualifiedWildcard(t1), Expression::QualifiedWildcard(t2)) => t1 == t2,
            (Expression::BinaryOp { op: o1, left: l1, right: r1 }, Expression::BinaryOp { op: o2, left: l2, right: r2 }) => {
                o1 == o2 && l1 == l2 && r1 == r2
            }
            (Expression::UnaryOp { op: o1, expr: e1 }, Expression::UnaryOp { op: o2, expr: e2 }) => {
                o1 == o2 && e1 == e2
            }
            (Expression::FnCall { name: n1, args: a1 }, Expression::FnCall { name: n2, args: a2 }) => {
                n1 == n2 && a1 == a2
            }
            (Expression::Aggregate { name: n1, distinct: d1, args: a1 }, Expression::Aggregate { name: n2, distinct: d2, args: a2 }) => {
                n1 == n2 && d1 == d2 && a1 == a2
            }
            (Expression::WindowFn { func: f1, over: o1, window_name: w1 }, Expression::WindowFn { func: f2, over: o2, window_name: w2 }) => {
                f1 == f2 && o1 == o2 && w1 == w2
            }
            (Expression::Case { expr: e1, whens: w1, else_expr: el1 }, Expression::Case { expr: e2, whens: w2, else_expr: el2 }) => {
                e1 == e2 && w1 == w2 && el1 == el2
            }
            (Expression::Between { expr: e1, low: l1, high: h1 }, Expression::Between { expr: e2, low: l2, high: h2 }) => {
                e1 == e2 && l1 == l2 && h1 == h2
            }
            (Expression::Subquery(a), Expression::Subquery(b)) => a == b,
            (Expression::Exists(a, na), Expression::Exists(b, nb)) => a == b && na == nb,
            (Expression::QuantifiedCmp { op: o1, left: l1, quant: q1, subquery: s1 }, Expression::QuantifiedCmp { op: o2, left: l2, quant: q2, subquery: s2 }) => {
                o1 == o2 && l1 == l2 && q1 == q2 && s1 == s2
            }
            (Expression::Cast { expr: e1, target: t1 }, Expression::Cast { expr: e2, target: t2 }) => {
                e1 == e2 && t1 == t2
            }
            (Expression::NamedParam(a), Expression::NamedParam(b)) => a == b,
            (Expression::Interval { value: v1, unit: u1 }, Expression::Interval { value: v2, unit: u2 }) => {
                v1.to_bits() == v2.to_bits() && u1 == u2
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    And,
    Or,
    Like,
    ILike,
    In,
    NotIn,
    Concat,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Quantifier {
    All,
    Any,
    Some,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IntervalUnit {
    Year,
    Month,
    Week,
    Day,
    Hour,
    Minute,
    Second,
}

impl fmt::Display for IntervalUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntervalUnit::Year => write!(f, "y"),
            IntervalUnit::Month => write!(f, "mon"),
            IntervalUnit::Week => write!(f, "w"),
            IntervalUnit::Day => write!(f, "d"),
            IntervalUnit::Hour => write!(f, "h"),
            IntervalUnit::Minute => write!(f, "m"),
            IntervalUnit::Second => write!(f, "s"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    String,
    Integer,
    Float,
    Boolean,
    DateTime,
    Json,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Insert {
    pub table: String,
    pub connection: Option<String>,
    pub schema: Option<String>,
    pub columns: Option<Vec<String>>,
    pub rows: Vec<Vec<(String, Expression)>>,
    pub query: Option<Box<Query>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    pub table: String,
    pub connection: Option<String>,
    pub schema: Option<String>,
    pub assignments: Vec<(String, Expression)>,
    pub filter: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Delete {
    pub table: String,
    pub connection: Option<String>,
    pub schema: Option<String>,
    pub filter: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Describe {
    pub table: String,
    pub connection: Option<String>,
    pub schema: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default: Option<Expression>,
    pub primary_key: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterAction {
    AddColumn(ColumnDef),
    DropColumn {
        name: String,
    },
    AlterColumn {
        name: String,
        data_type: Option<DataType>,
        nullable: Option<bool>,
        default: Option<Expression>,
        drop_default: bool,
    },
    RenameColumn {
        from: String,
        to: String,
    },
    RenameTable {
        to: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlterTable {
    pub table: String,
    pub connection: Option<String>,
    pub schema: Option<String>,
    pub action: AlterAction,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTable {
    pub table: String,
    pub connection: Option<String>,
    pub schema: Option<String>,
    pub columns: Vec<ColumnDef>,
    pub if_not_exists: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropTable {
    pub table: String,
    pub connection: Option<String>,
    pub schema: Option<String>,
    pub if_exists: bool,
    pub cascade: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConflictAction {
    Ignore,
    Replace,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTableAs {
    pub query: Box<Query>,
    pub table: String,
    pub connection: Option<String>,
    pub schema: Option<String>,
    pub on_conflict: Option<ConflictAction>,
}
