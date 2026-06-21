use chumsky::prelude::*;

use super::ast::*;
use super::lexer::{self, Token};

type Span = std::ops::Range<usize>;
type Spanned = (Token, Span);
type PErr = chumsky::error::Simple<Spanned>;

type Parser_<O> = BoxedParser<'static, Spanned, O, PErr>;

fn tok(t: Token) -> Parser_<Token> {
    filter(move |(tok, _): &Spanned| *tok == t).map(|(t, _)| t).boxed()
}

fn ident() -> impl Parser<Spanned, String, Error = PErr> {
    filter_map(|_: Span, (t, s): Spanned| match t {
        Token::Ident(name) => Ok(name),
        _ => Err(Simple::custom(s, "expected identifier")),
    })
}

fn ident_or_keyword() -> impl Parser<Spanned, String, Error = PErr> {
    filter_map(|_: Span, (t, s): Spanned| match t {
        Token::Ident(name) => Ok(name),
        Token::Find => Ok("find".to_string()),
        Token::With => Ok("with".to_string()),
        Token::Recursive => Ok("recursive".to_string()),
        Token::As => Ok("as".to_string()),
        Token::Where => Ok("where".to_string()),
        Token::From => Ok("from".to_string()),
        Token::Join => Ok("join".to_string()),
        Token::Left => Ok("left".to_string()),
        Token::Right => Ok("right".to_string()),
        Token::Full => Ok("full".to_string()),
        Token::Cross => Ok("cross".to_string()),
        Token::Inner => Ok("inner".to_string()),
        Token::On => Ok("on".to_string()),
        Token::Group => Ok("group".to_string()),
        Token::By => Ok("by".to_string()),
        Token::Having => Ok("having".to_string()),
        Token::Order => Ok("order".to_string()),
        Token::Asc => Ok("asc".to_string()),
        Token::Desc => Ok("desc".to_string()),
        Token::Nulls => Ok("nulls".to_string()),
        Token::First => Ok("first".to_string()),
        Token::Last => Ok("last".to_string()),
        Token::Limit => Ok("limit".to_string()),
        Token::Offset => Ok("offset".to_string()),
        Token::Distinct => Ok("distinct".to_string()),
        Token::Case => Ok("case".to_string()),
        Token::When => Ok("when".to_string()),
        Token::Then => Ok("then".to_string()),
        Token::Else => Ok("else".to_string()),
        Token::End => Ok("end".to_string()),
        Token::Union => Ok("union".to_string()),
        Token::All => Ok("all".to_string()),
        Token::Intersect => Ok("intersect".to_string()),
        Token::Except => Ok("except".to_string()),
        Token::Exists => Ok("exists".to_string()),
        Token::In => Ok("in".to_string()),
        Token::Not => Ok("not".to_string()),
        Token::Between => Ok("between".to_string()),
        Token::And => Ok("and".to_string()),
        Token::Or => Ok("or".to_string()),
        Token::Like => Ok("like".to_string()),
        Token::ILike => Ok("ilike".to_string()),
        Token::Is => Ok("is".to_string()),
        Token::Null => Ok("null".to_string()),
        Token::True => Ok("true".to_string()),
        Token::False => Ok("false".to_string()),
        Token::Create => Ok("create".to_string()),
        Token::Update => Ok("update".to_string()),
        Token::Set => Ok("set".to_string()),
        Token::Remove => Ok("remove".to_string()),
        Token::Explain => Ok("explain".to_string()),
        Token::Describe => Ok("describe".to_string()),
        Token::Show => Ok("show".to_string()),
        Token::Tables => Ok("tables".to_string()),
        Token::Over => Ok("over".to_string()),
        Token::Partition => Ok("partition".to_string()),
        Token::Window => Ok("window".to_string()),
        Token::Coalesce => Ok("coalesce".to_string()),
        Token::Nullif => Ok("nullif".to_string()),
        Token::Ifnull => Ok("ifnull".to_string()),
        Token::Cast => Ok("cast".to_string()),
        Token::Now => Ok("now".to_string()),
        Token::Any => Ok("any".to_string()),
        Token::Some => Ok("some".to_string()),
        Token::Count => Ok("count".to_string()),
        Token::Sum => Ok("sum".to_string()),
        Token::Avg => Ok("avg".to_string()),
        Token::Min => Ok("min".to_string()),
        Token::Max => Ok("max".to_string()),
        Token::CountDistinct => Ok("count_distinct".to_string()),
        _ => Err(Simple::custom(s, "expected identifier")),
    })
}

fn string_raw() -> impl Parser<Spanned, String, Error = PErr> {
    filter_map(|_: Span, (t, s): Spanned| match t {
        Token::StringLit(v) => Ok(v),
        _ => Err(Simple::custom(s, "expected string")),
    })
}

fn int64() -> impl Parser<Spanned, i64, Error = PErr> {
    filter_map(|_: Span, (t, s): Spanned| match t {
        Token::Integer(v) => Ok(v),
        _ => Err(Simple::custom(s, "expected integer")),
    })
}

fn float64() -> impl Parser<Spanned, f64, Error = PErr> {
    filter_map(|_: Span, (t, s): Spanned| match t {
        Token::Float(v) => Ok(v),
        _ => Err(Simple::custom(s, "expected float")),
    })
}

fn param() -> impl Parser<Spanned, String, Error = PErr> {
    filter_map(|_: Span, (t, s): Spanned| match t {
        Token::Param(name) => Ok(name),
        _ => Err(Simple::custom(s, "expected parameter")),
    })
}

fn interval_literal() -> impl Parser<Spanned, Expression, Error = PErr> {
    filter_map(|_: Span, (t, s): Spanned| match t {
        Token::Interval(value, suffix) => {
            let unit = match suffix {
                lexer::IntervalSuffix::Year => IntervalUnit::Year,
                lexer::IntervalSuffix::Month => IntervalUnit::Month,
                lexer::IntervalSuffix::Week => IntervalUnit::Week,
                lexer::IntervalSuffix::Day => IntervalUnit::Day,
                lexer::IntervalSuffix::Hour => IntervalUnit::Hour,
                lexer::IntervalSuffix::Minute => IntervalUnit::Minute,
                lexer::IntervalSuffix::Second => IntervalUnit::Second,
            };
            Ok(Expression::Interval {
                value: value as f64,
                unit,
            })
        }
        _ => Err(Simple::custom(s, "expected interval literal")),
    })
}

fn data_type() -> impl Parser<Spanned, DataType, Error = PErr> {
    ident().map(|s| match s.to_lowercase().as_str() {
        "string" | "text" | "varchar" => DataType::String,
        "int" | "integer" | "bigint" | "smallint" => DataType::Integer,
        "float" | "double" | "real" | "numeric" | "decimal" => DataType::Float,
        "bool" | "boolean" => DataType::Boolean,
        "datetime" | "timestamp" | "date" | "time" => DataType::DateTime,
        "json" | "jsonb" => DataType::Json,
        _ => DataType::String,
    })
}

fn ident_or_call(expr: Parser_<Expression>) -> Parser_<Expression> {
    let name_p = choice((
        ident(),
        tok(Token::Now).map(|_| "now".to_string()),
        tok(Token::Coalesce).map(|_| "coalesce".to_string()),
        tok(Token::Nullif).map(|_| "nullif".to_string()),
        tok(Token::Ifnull).map(|_| "ifnull".to_string()),
    ));
    let expr_rc = std::rc::Rc::new(expr);
    name_p
        .then_with(move |name: String| {
            let expr = expr_rc.clone();

            // Try function call: name(args)
            call_args((*expr).clone())
                .map({
                    let n = name.clone();
                    move |args| Expression::FnCall { name: n.clone(), args }
                })
                // Try name.* → QualifiedWildcard
                .or(tok(Token::Dot).ignore_then(tok(Token::Star))
                    .map({
                        let n = name.clone();
                        move |_| Expression::QualifiedWildcard(n.clone())
                    }))
                // Try name.field → QualifiedIdent or name.field(args)
                .or(tok(Token::Dot).ignore_then(ident()).then_with({
                    let n = name.clone();
                    let expr = expr.clone();
                    move |field: String| {
                        call_args((*expr).clone())
                            .map({
                                let n = n.clone();
                                let f = field.clone();
                                move |args| Expression::FnCall {
                                    name: format!("{}.{}", n.clone(), f.clone()),
                                    args,
                                }
                            })
                            .or_not()
                            .map({
                                let n = n.clone();
                                let f = field.clone();
                                move |rest| match rest {
                                    Some(fc) => fc,
                                    None => Expression::QualifiedIdent {
                                        table: n.clone(),
                                        field: f.clone(),
                                    },
                                }
                            })
                    }
                }))
                .or_not()
                .map({
                    let n = name;
                    move |rest| match rest {
                        Some(e) => e,
                        None => Expression::Ident(n.clone()),
                    }
                })
        })
        .boxed()
}

fn array_lit(expr: Parser_<Expression>) -> Parser_<Expression> {
    expr.clone()
        .separated_by(tok(Token::Comma))
        .allow_trailing()
        .delimited_by(tok(Token::LBracket), tok(Token::RBracket))
        .map(Expression::Array)
        .boxed()
}

fn object_field(expr: Parser_<Expression>) -> impl Parser<Spanned, (String, Expression), Error = PErr> {
    string_raw()
        .or(ident_or_keyword())
        .then_ignore(tok(Token::Colon))
        .then(expr)
}

fn object_lit(expr: Parser_<Expression>) -> Parser_<Expression> {
    object_field(expr.clone())
        .separated_by(tok(Token::Comma))
        .allow_trailing()
        .delimited_by(tok(Token::LBrace), tok(Token::RBrace))
        .map(Expression::Object)
        .boxed()
}

fn call_args(expr: Parser_<Expression>) -> Parser_<Vec<Expression>> {
    expr.separated_by(tok(Token::Comma))
        .allow_trailing()
        .delimited_by(tok(Token::LParen), tok(Token::RParen))
        .boxed()
}

fn fn_call(expr: Parser_<Expression>) -> Parser_<Expression> {
    ident()
        .then(call_args(expr))
        .map(|(name, args)| Expression::FnCall { name, args })
        .boxed()
}

fn agg_call(expr: Parser_<Expression>) -> Parser_<Expression> {
    let star_agg = choice((
        tok(Token::Count).map(|_| "count".to_string()),
        tok(Token::Sum).map(|_| "sum".to_string()),
        tok(Token::Avg).map(|_| "avg".to_string()),
        tok(Token::Min).map(|_| "min".to_string()),
        tok(Token::Max).map(|_| "max".to_string()),
        tok(Token::CountDistinct).map(|_| "count_distinct".to_string()),
    ))
    .then(
        tok(Token::LParen)
            .ignore_then(tok(Token::Star).or_not())
            .then_ignore(tok(Token::RParen)),
    )
    .map(|(name, star)| Expression::Aggregate {
        name,
        distinct: false,
        args: if star.is_some() { vec![] } else { vec![] },
    });

    let args_agg = choice((
        tok(Token::CountDistinct).map(|_| ("count".to_string(), true)),
        tok(Token::Count).map(|_| ("count".to_string(), false)),
        tok(Token::Sum).map(|_| ("sum".to_string(), false)),
        tok(Token::Avg).map(|_| ("avg".to_string(), false)),
        tok(Token::Min).map(|_| ("min".to_string(), false)),
        tok(Token::Max).map(|_| ("max".to_string(), false)),
    ))
    .then(call_args(expr))
    .map(|((name, distinct), args)| Expression::Aggregate {
        name,
        distinct,
        args,
    });

    args_agg.or(star_agg).boxed()
}

fn window_spec(expr: Parser_<Expression>) -> Parser_<WindowSpec> {
    tok(Token::LParen)
        .ignore_then(
            tok(Token::Partition)
                .ignore_then(tok(Token::By))
                .ignore_then(expr.clone().separated_by(tok(Token::Comma)))
                .or_not()
                .map(|v| v.unwrap_or_default()),
        )
        .then(
            tok(Token::Order)
                .ignore_then(tok(Token::By))
                .ignore_then(order_by_item(expr).repeated())
                .or_not()
                .map(|v| v.unwrap_or_default()),
        )
        .then_ignore(tok(Token::RParen))
        .map(|(partition_by, order_by)| WindowSpec {
            partition_by,
            order_by,
        })
        .boxed()
}

fn window_fn_call(expr: Parser_<Expression>) -> Parser_<Expression> {
    // Match only identifiers that are window function names
    let win_ident = filter_map(|_: Span, (t, s): Spanned| match t {
        Token::Ident(name) => {
            let lower = name.to_lowercase();
            match lower.as_str() {
                "row_number" | "rank" | "dense_rank" | "lag" | "lead"
                | "first_value" | "last_value" | "nth_value" => Ok(name),
                _ => Err(Simple::custom(s, "expected window function")),
            }
        }
        _ => Err(Simple::custom(s, "expected identifier")),
    });

    let func = choice((
        win_ident
            .clone()
            .try_map(|n: String, s| {
                if n.to_lowercase() == "row_number" { Ok(n) }
                else { Err(Simple::custom(s, "expected row_number")) }
            })
            .ignore_then(tok(Token::LParen).ignore_then(tok(Token::RParen)))
            .map(|_| WindowFunction::RowNumber),
        win_ident
            .clone()
            .try_map(|n: String, s| {
                if n.to_lowercase() == "rank" { Ok(n) }
                else { Err(Simple::custom(s, "expected rank")) }
            })
            .ignore_then(tok(Token::LParen).ignore_then(tok(Token::RParen)))
            .map(|_| WindowFunction::Rank),
        win_ident
            .clone()
            .try_map(|n: String, s| {
                if n.to_lowercase() == "dense_rank" { Ok(n) }
                else { Err(Simple::custom(s, "expected dense_rank")) }
            })
            .ignore_then(tok(Token::LParen).ignore_then(tok(Token::RParen)))
            .map(|_| WindowFunction::DenseRank),
        win_ident
            .clone()
            .try_map(|n: String, s| {
                if n.to_lowercase() == "lag" { Ok(n) }
                else { Err(Simple::custom(s, "expected lag")) }
            })
            .ignore_then(
                tok(Token::LParen).ignore_then(expr.clone())
                    .then(tok(Token::Comma).ignore_then(int64()).or_not())
                    .then_ignore(tok(Token::RParen)),
            )
            .map(|(e, o)| WindowFunction::Lag(Box::new(e), o)),
        win_ident
            .clone()
            .try_map(|n: String, s| {
                if n.to_lowercase() == "lead" { Ok(n) }
                else { Err(Simple::custom(s, "expected lead")) }
            })
            .ignore_then(
                tok(Token::LParen).ignore_then(expr.clone())
                    .then(tok(Token::Comma).ignore_then(int64()).or_not())
                    .then_ignore(tok(Token::RParen)),
            )
            .map(|(e, o)| WindowFunction::Lead(Box::new(e), o)),
        win_ident
            .clone()
            .try_map(|n: String, s| {
                if n.to_lowercase() == "first_value" { Ok(n) }
                else { Err(Simple::custom(s, "expected first_value")) }
            })
            .ignore_then(
                tok(Token::LParen).ignore_then(expr.clone()).then_ignore(tok(Token::RParen)),
            )
            .map(|e| WindowFunction::FirstValue(Box::new(e))),
        win_ident
            .try_map(|n: String, s| {
                if n.to_lowercase() == "last_value" { Ok(n) }
                else { Err(Simple::custom(s, "expected last_value")) }
            })
            .ignore_then(
                tok(Token::LParen).ignore_then(expr.clone()).then_ignore(tok(Token::RParen)),
            )
            .map(|e| WindowFunction::LastValue(Box::new(e))),
        win_ident
            .try_map(|n: String, s| {
                if n.to_lowercase() == "nth_value" { Ok(n) }
                else { Err(Simple::custom(s, "expected nth_value")) }
            })
            .ignore_then(
                tok(Token::LParen).ignore_then(expr.clone())
                    .then_ignore(tok(Token::Comma))
                    .then(int64())
                    .then_ignore(tok(Token::RParen)),
            )
            .map(|(e, n)| WindowFunction::NthValue(Box::new(e), n as u64)),
    ));

    func.then_ignore(tok(Token::Over))
        .then(choice((
            window_spec(expr).map(|spec| (spec, None)),
            ident().map(|name| (WindowSpec {
                partition_by: vec![],
                order_by: vec![],
            }, Some(name))),
        )))
        .map(|(func, (over, window_name))| Expression::WindowFn {
            func,
            over,
            window_name,
        })
        .boxed()
}

fn case_expr(expr: Parser_<Expression>) -> Parser_<Expression> {
    let when_clause = tok(Token::When)
        .ignore_then(expr.clone())
        .then_ignore(tok(Token::Then))
        .then(expr.clone());

    let simple = tok(Token::Case)
        .ignore_then(expr.clone())
        .then(when_clause.clone().repeated())
        .then(tok(Token::Else).ignore_then(expr.clone()).or_not())
        .then_ignore(tok(Token::End))
        .map(|((case_val, whens), else_expr)| Expression::Case {
            expr: Some(Box::new(case_val)),
            whens,
            else_expr: else_expr.map(Box::new),
        });

    let searched = tok(Token::Case)
        .ignore_then(when_clause.repeated())
        .then(tok(Token::Else).ignore_then(expr).or_not())
        .then_ignore(tok(Token::End))
        .map(|(whens, else_expr)| Expression::Case {
            expr: None,
            whens,
            else_expr: else_expr.map(Box::new),
        });

    simple.or(searched).boxed()
}

fn cast_fn(expr: Parser_<Expression>) -> Parser_<Expression> {
    tok(Token::Cast)
        .ignore_then(
            tok(Token::LParen)
                .ignore_then(expr)
                .then_ignore(tok(Token::As))
                .then(data_type())
                .then_ignore(tok(Token::RParen)),
        )
        .map(|(e, target)| Expression::Cast {
            expr: Box::new(e),
            target,
        })
        .boxed()
}

fn order_by_item(expr: Parser_<Expression>) -> Parser_<OrderBy> {
    expr.clone()
        .then(
            choice((
                tok(Token::Asc).to(OrderDir::Asc),
                tok(Token::Desc).to(OrderDir::Desc),
            ))
            .or_not(),
        )
        .then(
            tok(Token::Nulls)
                .ignore_then(choice((
                    tok(Token::First).to(NullsOrder::First),
                    tok(Token::Last).to(NullsOrder::Last),
                )))
                .or_not(),
        )
        .map(|((e, dir), nulls)| OrderBy {
            expr: e,
            direction: dir.unwrap_or(OrderDir::Asc),
            nulls: nulls.unwrap_or(NullsOrder::Default),
        })
        .boxed()
}

fn named_window(expr: Parser_<Expression>) -> Parser_<WindowDef> {
    ident_or_keyword()
        .then_ignore(tok(Token::As))
        .then(window_spec(expr))
        .map(|(name, spec)| WindowDef { name, spec })
        .boxed()
}

// ── Sources and joins ────────────────────────────────────────────────────────

fn source_name() -> Parser_<(String, Option<String>)> {
    ident_or_keyword().then(tok(Token::At).ignore_then(ident_or_keyword()).or_not()).boxed()
}

fn source(_expr: Parser_<Expression>, query_p: Parser_<Query>) -> Parser_<Source> {
    let subquery_src = tok(Token::LParen)
        .ignore_then(query_p)
        .then_ignore(tok(Token::RParen))
        .then(tok(Token::As).ignore_then(ident()))
        .map(|(q, alias)| Source {
            name: alias.clone(),
            alias: Some(alias),
            connection: None,
            kind: SourceKind::Subquery(Box::new(q)),
        });

    let table_src = source_name()
        .then(tok(Token::As).ignore_then(ident()).or_not())
        .map(|((name, conn), alias)| {
            let display = alias.clone().unwrap_or_else(|| name.clone());
            Source {
                name: display,
                alias,
                connection: conn,
                kind: SourceKind::Table(name),
            }
        });

    subquery_src.or(table_src).boxed()
}

fn join_kind() -> Parser_<JoinKind> {
    choice((
        tok(Token::Cross).ignore_then(tok(Token::Join).or_not()).map(|_| JoinKind::Cross),
        tok(Token::Left).ignore_then(tok(Token::Join).or_not()).map(|_| JoinKind::Left),
        tok(Token::Right).ignore_then(tok(Token::Join).or_not()).map(|_| JoinKind::Right),
        tok(Token::Full).ignore_then(tok(Token::Join).or_not()).map(|_| JoinKind::Full),
        tok(Token::Inner).ignore_then(tok(Token::Join).or_not()).map(|_| JoinKind::Inner),
        tok(Token::Join).map(|_| JoinKind::Inner),
    ))
    .boxed()
}

fn join(expr: Parser_<Expression>, query_p: Parser_<Query>) -> Parser_<Join> {
    join_kind()
        .then(source(expr.clone(), query_p))
        .then(tok(Token::On).ignore_then(expr).or_not())
        .map(|((kind, src), condition)| Join {
            kind,
            source: src,
            alias: None,
            condition,
        })
        .boxed()
}

fn projection_item(expr: Parser_<Expression>) -> Parser_<Projection> {
    let wildcard = tok(Token::Star).map(|_| Projection::Wildcard);

    let expr_proj = expr
        .then(tok(Token::As).ignore_then(ident()).or_not())
        .map(|(e, alias)| match e {
            Expression::QualifiedWildcard(t) => Projection::QualifiedWildcard(t),
            _ => Projection::Expr(e, alias),
        });

    choice((wildcard, expr_proj)).boxed()
}

// ── Expression parser (recursive with lazy query ref) ────────────────────────

fn make_expr_parser(query_p: Parser_<Query>) -> Parser_<Expression> {
    recursive(|expr| {
        let expr: Parser_<Expression> = expr.boxed();
        let query_p = query_p.clone();

        let atom = choice((
            string_raw().map(Expression::String).boxed(),
            float64().map(Expression::Number).boxed(),
            int64().map(|v| Expression::Integer(v)).boxed(),
            tok(Token::True).map(|_| Expression::Boolean(true)).boxed(),
            tok(Token::False).map(|_| Expression::Boolean(false)).boxed(),
            tok(Token::Null).map(|_| Expression::Null).boxed(),
            param().map(Expression::NamedParam).boxed(),
            interval_literal().boxed(),
            window_fn_call(expr.clone()).boxed(),
            ident_or_call(expr.clone()).boxed(),
            array_lit(expr.clone()).boxed(),
            object_lit(expr.clone()).boxed(),
            case_expr(expr.clone()).boxed(),
            cast_fn(expr.clone()).boxed(),
            agg_call(expr.clone()).boxed(),
            tok(Token::LParen)
                .ignore_then(query_p.clone())
                .then_ignore(tok(Token::RParen))
                .map(|q| Expression::Subquery(Box::new(q)))
                .boxed(),
            tok(Token::Not)
                .or_not()
                .then_ignore(tok(Token::Exists))
                .then(
                    tok(Token::LParen)
                        .ignore_then(query_p.clone())
                        .then_ignore(tok(Token::RParen)),
                )
                .map(|(not_token, q)| Expression::Exists(Box::new(q), not_token.is_none()))
                .boxed(),
            expr.clone().delimited_by(tok(Token::LParen), tok(Token::RParen)).boxed(),
        ))
        .boxed();

        let unary = choice((
            tok(Token::Minus)
                .ignore_then(atom.clone())
                .map(|e| Expression::UnaryOp {
                    op: UnaryOp::Neg,
                    expr: Box::new(e),
                }),
            tok(Token::Not)
                .ignore_then(atom.clone())
                .map(|e| Expression::UnaryOp {
                    op: UnaryOp::Not,
                    expr: Box::new(e),
                }),
            atom,
        ))
        .boxed();

        let cast = unary.clone().then(
            tok(Token::CastOp)
                .ignore_then(data_type())
                .repeated(),
        )
        .foldl(|e, dt| Expression::Cast {
            expr: Box::new(e),
            target: dt,
        })
        .boxed();

        // OVER clause postfix: expr OVER (spec) or expr OVER name
        let with_over = cast.clone().then(
            tok(Token::Over).ignore_then(
                choice((
                    window_spec(expr.clone()).map(|spec| (spec, None::<String>)),
                    ident().map(|name| (WindowSpec {
                        partition_by: vec![],
                        order_by: vec![],
                    }, Some(name))),
                ))
            ).or_not()
        )
        .map(|(e, over)| match over {
            Some((spec, name)) => Expression::WindowFn {
                func: WindowFunction::Expr(Box::new(e)),
                over: spec,
                window_name: name,
            },
            None => e,
        })
        .boxed();

        let mul = with_over.clone().then(
            choice((
                tok(Token::Star).to(BinaryOp::Mul),
                tok(Token::Slash).to(BinaryOp::Div),
                tok(Token::Percent).to(BinaryOp::Mod),
            ))
            .then(with_over.clone())
            .repeated(),
        )
        .foldl(|a, (op, b)| Expression::BinaryOp {
            op,
            left: Box::new(a),
            right: Box::new(b),
        })
        .boxed();

        let add = mul.clone().then(
            choice((
                tok(Token::Plus).to(BinaryOp::Add),
                tok(Token::Minus).to(BinaryOp::Sub),
                tok(Token::Concat).to(BinaryOp::Concat),
            ))
            .then(mul)
            .repeated(),
        )
        .foldl(|a, (op, b)| Expression::BinaryOp {
            op,
            left: Box::new(a),
            right: Box::new(b),
        })
        .boxed();

        let between = add.clone().then(
            tok(Token::Between)
                .ignore_then(add.clone())
                .then_ignore(tok(Token::And))
                .then(add)
                .or_not(),
        )
        .map(|(e, range)| match range {
            Some((low, high)) => Expression::Between {
                expr: Box::new(e),
                low: Box::new(low),
                high: Box::new(high),
            },
            None => e,
        })
        .boxed();

        let cmp = between.clone().then(
            choice((
                tok(Token::Eq).to(BinaryOp::Eq),
                tok(Token::Neq).to(BinaryOp::Neq),
                tok(Token::Lt).to(BinaryOp::Lt),
                tok(Token::Gt).to(BinaryOp::Gt),
                tok(Token::Lte).to(BinaryOp::Lte),
                tok(Token::Gte).to(BinaryOp::Gte),
                tok(Token::Like).to(BinaryOp::Like),
                tok(Token::ILike).to(BinaryOp::ILike),
            ))
            .then(between.clone())
            .repeated(),
        )
        .foldl(|a, (op, b)| Expression::BinaryOp {
            op,
            left: Box::new(a),
            right: Box::new(b),
        })
        .boxed();

        let quantified = cmp.clone().then(
            choice((
                tok(Token::Eq).to(BinaryOp::Eq),
                tok(Token::Neq).to(BinaryOp::Neq),
                tok(Token::Lt).to(BinaryOp::Lt),
                tok(Token::Gt).to(BinaryOp::Gt),
                tok(Token::Lte).to(BinaryOp::Lte),
                tok(Token::Gte).to(BinaryOp::Gte),
            ))
            .then(
                choice((
                    tok(Token::All).to(Quantifier::All),
                    tok(Token::Any).to(Quantifier::Any),
                    tok(Token::Some).to(Quantifier::Some),
                ))
            )
            .then(
                tok(Token::LParen)
                    .ignore_then(query_p.clone())
                    .then_ignore(tok(Token::RParen))
            )
            .or_not()
        )
        .map(|(left, maybe)| match maybe {
            Some(((op, quant), subquery)) => Expression::QuantifiedCmp {
                op,
                left: Box::new(left),
                quant,
                subquery: Box::new(subquery),
            },
            None => left,
        })
        .boxed();

        let in_rhs = choice((
            expr.clone()
                .separated_by(tok(Token::Comma))
                .delimited_by(tok(Token::LParen), tok(Token::RParen))
                .map(Expression::Array),
            tok(Token::LParen)
                .ignore_then(query_p.clone())
                .then_ignore(tok(Token::RParen))
                .map(|q| Expression::Subquery(Box::new(q))),
        ))
        .boxed();

        let in_expr = quantified.clone().then(
            choice((
                tok(Token::In).ignore_then(in_rhs.clone()).map(|rhs| (BinaryOp::In, rhs)),
                tok(Token::Not)
                    .ignore_then(tok(Token::In))
                    .ignore_then(in_rhs)
                    .map(|rhs| (BinaryOp::NotIn, rhs)),
            ))
            .or_not(),
        )
        .map(|(left, maybe)| match maybe {
            Some((op, rhs)) => Expression::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(rhs),
            },
            None => left,
        })
        .boxed();

        let is_null = in_expr.clone().then(
            tok(Token::Is)
                .ignore_then(tok(Token::Not).or_not().then(tok(Token::Null)))
                .or_not(),
        )
        .map(|(e, isnull)| match isnull {
            Some((not, _)) => Expression::BinaryOp {
                op: if not.is_some() { BinaryOp::Neq } else { BinaryOp::Eq },
                left: Box::new(e),
                right: Box::new(Expression::Null),
            },
            None => e,
        })
        .boxed();

        let and_ = is_null.clone().then(
            tok(Token::And).ignore_then(is_null.clone()).repeated(),
        )
        .foldl(|a, b| Expression::BinaryOp {
            op: BinaryOp::And,
            left: Box::new(a),
            right: Box::new(b),
        })
        .boxed();

        let or_ = and_.clone().then(
            tok(Token::Or).ignore_then(and_.clone()).repeated(),
        )
        .foldl(|a, b| Expression::BinaryOp {
            op: BinaryOp::Or,
            left: Box::new(a),
            right: Box::new(b),
        })
        .boxed();

        or_
    })
    .boxed()
}

// ── Query parser (recursive with lazy expr ref) ──────────────────────────────

fn make_query_parser() -> Parser_<Query> {
    recursive(|query_self| {
        let query_self: Parser_<Query> = query_self.boxed();
        let expr_p = make_expr_parser(query_self.clone());
        let src = source(expr_p.clone(), query_self.clone());
        let join_p = join(expr_p.clone(), query_self);

        let distinct = tok(Token::Distinct).or_not().map(|d| d.is_some());

        let proj_explicit = projection_item(expr_p.clone())
            .separated_by(tok(Token::Comma))
            .allow_trailing()
            .delimited_by(tok(Token::LBracket), tok(Token::RBracket));

        let proj_star = tok(Token::Star).map(|_| vec![Projection::Wildcard]);

        let projection_and_sources = proj_explicit
            .or(proj_star)
            .then(
                tok(Token::From)
                    .ignore_then(src.clone().separated_by(tok(Token::Comma)))
                    .or_not()
                    .map(|v| v.unwrap_or_default()),
            )
            .or(src.clone().separated_by(tok(Token::Comma))
                .map(|sources| (vec![Projection::Wildcard], sources)));

        let joins_list = join_p.repeated();

        let where_clause = tok(Token::Where)
            .ignore_then(expr_p.clone())
            .or_not();

        let group = tok(Token::Group)
            .ignore_then(tok(Token::By))
            .ignore_then(expr_p.clone().separated_by(tok(Token::Comma)))
            .or_not()
            .map(|v| v.unwrap_or_default());

        let having = tok(Token::Having)
            .ignore_then(expr_p.clone())
            .or_not();

        let windows = tok(Token::Window)
            .ignore_then(named_window(expr_p.clone()).separated_by(tok(Token::Comma)))
            .or_not()
            .map(|v| v.unwrap_or_default());

        let order = tok(Token::Order)
            .ignore_then(tok(Token::By))
            .ignore_then(order_by_item(expr_p.clone()).separated_by(tok(Token::Comma)))
            .or_not()
            .map(|v| v.unwrap_or_default());

        let limit_clause = tok(Token::Limit)
            .ignore_then(int64().map(|v| v as u64))
            .or_not();

        let offset_clause = tok(Token::Offset)
            .ignore_then(int64().map(|v| v as u64))
            .or_not();

        tok(Token::Find)
            .ignore_then(distinct)
            .then(projection_and_sources)
            .then(joins_list)
            .then(where_clause)
            .then(group)
            .then(having)
            .then(windows)
            .then(order)
            .then(limit_clause)
            .then(offset_clause)
            .map(
                |(
                    ((((((((distinct, (projection, sources)), joins), filter), group_by), having), window_defs), order_by), limit),
                    offset,
                )| Query {
                    distinct,
                    projection,
                    sources,
                    joins,
                    filter,
                    group_by,
                    having,
                    window_defs,
                    order_by,
                    limit,
                    offset,
                },
            )
            .boxed()
    })
    .boxed()
}

// ── Statement parser ─────────────────────────────────────────────────────────

fn resolve_cte_refs_in_statement(stmt: &mut Statement, cte_names: &std::collections::HashSet<String>) {
    match stmt {
        Statement::Query(q) => resolve_cte_refs_in_query(q, cte_names),
        Statement::With(w) => {
            for cte in &mut w.ctes {
                resolve_cte_refs_in_query(&mut cte.query, cte_names);
                for (_kind, q) in &mut cte.chain {
                    resolve_cte_refs_in_query(q, cte_names);
                }
            }
            resolve_cte_refs_in_statement(&mut w.body, cte_names);
        }
        Statement::Insert(i) => {
            if let Some(q) = &mut i.query {
                resolve_cte_refs_in_query(q, cte_names);
            }
        }
        _ => {}
    }
}

fn resolve_cte_refs_in_query(query: &mut Query, cte_names: &std::collections::HashSet<String>) {
    for source in &mut query.sources {
        if let SourceKind::Table(name) = &source.kind {
            if cte_names.contains(name) {
                source.kind = SourceKind::CteRef(name.clone());
            }
        }
    }
    for join in &mut query.joins {
        if let SourceKind::Table(name) = &join.source.kind {
            if cte_names.contains(name) {
                join.source.kind = SourceKind::CteRef(name.clone());
            }
        }
    }
}

pub fn parser() -> Parser_<Vec<Statement>> {
    let stmt = recursive(|stmt| {
        let query_p = make_query_parser();
        let expr_p = make_expr_parser(query_p.clone());

        let set_op_kw = choice((
            tok(Token::Union)
                .ignore_then(tok(Token::All).or_not())
                .map(|all| if all.is_some() { SetOpKind::UnionAll } else { SetOpKind::Union }),
            tok(Token::Intersect).map(|_| SetOpKind::Intersect),
            tok(Token::Except).map(|_| SetOpKind::Except),
        ));

        let query_chain = query_p
            .clone()
            .then(set_op_kw.then(query_p.clone()).repeated())
            .map(|(first, rest)| {
                if rest.is_empty() {
                    (first, rest)
                } else {
                    (first, rest)
                }
            });

        let cte = ident_or_keyword()
            .then(
                tok(Token::LParen)
                    .ignore_then(ident_or_keyword().separated_by(tok(Token::Comma)))
                    .then_ignore(tok(Token::RParen))
                    .or_not(),
            )
            .then_ignore(tok(Token::As))
            .then(
                tok(Token::LParen)
                    .ignore_then(query_chain.clone())
                    .then_ignore(tok(Token::RParen)),
            )
            .map(|((name, columns), (first, chain))| Cte {
                name,
                columns,
                query: Box::new(first),
                chain,
            });

        let with_stmt = tok(Token::With)
            .ignore_then(tok(Token::Recursive).or_not())
            .then(cte.separated_by(tok(Token::Comma)).allow_trailing())
            .then(stmt.clone())
            .map(|((recursive, mut ctes), mut body)| {
                let cte_names: std::collections::HashSet<String> =
                    ctes.iter().map(|c: &Cte| c.name.clone()).collect();
                for cte in &mut ctes {
                    resolve_cte_refs_in_query(&mut cte.query, &cte_names);
                    for (_kind, q) in &mut cte.chain {
                        resolve_cte_refs_in_query(q, &cte_names);
                    }
                }
                resolve_cte_refs_in_statement(&mut body, &cte_names);
                Statement::With(With {
                    recursive: recursive.is_some(),
                    ctes,
                    body: Box::new(body),
                })
            });

        let explain = tok(Token::Explain)
            .ignore_then(stmt.clone())
            .map(|s| Statement::Explain(Box::new(s)));

        let describe = tok(Token::Describe)
            .ignore_then(ident_or_keyword())
            .then(tok(Token::At).ignore_then(ident_or_keyword()).or_not())
            .map(|(table, conn)| {
                Statement::Describe(Describe {
                    table,
                    connection: conn,
                })
            });

        let show_tables = tok(Token::Show)
            .ignore_then(tok(Token::Tables))
            .ignore_then(tok(Token::At).ignore_then(ident()).or_not())
            .map(Statement::ShowTables);

        let param_assign = param()
            .then_ignore(tok(Token::Eq))
            .then(expr_p.clone())
            .map(|(name, value)| Statement::ParamAssign { name, value });

        let insert = tok(Token::Create)
            .ignore_then(ident_or_keyword())
            .then(tok(Token::At).ignore_then(ident_or_keyword()).or_not())
            .then(choice((
                object_field(expr_p.clone())
                    .separated_by(tok(Token::Comma))
                    .allow_trailing()
                    .delimited_by(tok(Token::LBrace), tok(Token::RBrace))
                    .map(|fields: Vec<(String, Expression)>| {
                        let _: Option<Box<Query>> = None;
                        (Vec::<String>::new(), vec![fields], None)
                    }),
                object_field(expr_p.clone())
                    .separated_by(tok(Token::Comma))
                    .allow_trailing()
                    .delimited_by(tok(Token::LBrace), tok(Token::RBrace))
                    .separated_by(tok(Token::Comma))
                    .allow_trailing()
                    .delimited_by(tok(Token::LBracket), tok(Token::RBracket))
                    .map(|rows: Vec<Vec<(String, Expression)>>| {
                        (Vec::<String>::new(), rows, None)
                    }),
                tok(Token::LParen)
                    .ignore_then(query_p.clone())
                    .then_ignore(tok(Token::RParen))
                    .map(|q| (Vec::<String>::new(), Vec::new(), Some(Box::new(q)))),
            )))
            .map(|((table, conn), (_cols, rows, query))| {
                Statement::Insert(Insert {
                    table,
                    connection: conn,
                    columns: None,
                    rows,
                    query,
                })
            });

        let update = tok(Token::Update)
            .ignore_then(ident_or_keyword())
            .then(tok(Token::At).ignore_then(ident_or_keyword()).or_not())
            .then_ignore(tok(Token::Set))
            .then(
                ident_or_keyword()
                    .then_ignore(tok(Token::Eq))
                    .then(expr_p.clone())
                    .separated_by(tok(Token::Comma)),
            )
            .then(
                tok(Token::Where)
                    .ignore_then(expr_p.clone())
                    .or_not(),
            )
            .map(|(((table, conn), assignments), filter)| {
                Statement::Update(Update {
                    table,
                    connection: conn,
                    assignments,
                    filter,
                })
            });

        let delete = tok(Token::Remove)
            .ignore_then(ident_or_keyword())
            .then(tok(Token::At).ignore_then(ident_or_keyword()).or_not())
            .then(
                tok(Token::Where)
                    .ignore_then(expr_p)
                    .or_not(),
            )
            .map(|((table, conn), filter)| {
                Statement::Delete(Delete {
                    table,
                    connection: conn,
                    filter,
                })
            });

        let query_or_setop = query_chain
            .map(|(first, rest)| {
                if rest.is_empty() {
                    Statement::Query(first)
                } else {
                    let mut current = Statement::Query(first);
                    for (kind, right) in rest {
                        let left = match current {
                            Statement::Query(q) => Box::new(q),
                            _ => Box::new(Query::default()),
                        };
                        current = Statement::SetOp(SetOp {
                            kind,
                            left,
                            right: Box::new(right),
                        });
                    }
                    current
                }
            });

        choice((
            query_or_setop.boxed(),
            with_stmt.boxed(),
            explain.boxed(),
            describe.boxed(),
            show_tables.boxed(),
            param_assign.boxed(),
            insert.boxed(),
            update.boxed(),
            delete.boxed(),
        ))
        .boxed()
    });

    stmt.then_ignore(tok(Token::Semicolon).or_not())
        .repeated()
        .then_ignore(end())
        .boxed()
}

