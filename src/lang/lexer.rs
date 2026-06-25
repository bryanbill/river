use std::fmt;
use std::ops::Range;

use tracing::warn;

#[derive(Debug, Clone)]
pub enum Token {
    // Keywords
    Find,
    With,
    Recursive,
    As,
    Where,
    From,
    Join,
    Left,
    Right,
    Full,
    Cross,
    Inner,
    On,
    Group,
    By,
    Having,
    Order,
    Asc,
    Desc,
    Nulls,
    First,
    Last,
    Limit,
    Offset,
    Distinct,
    Case,
    When,
    Then,
    Else,
    End,
    Union,
    All,
    Intersect,
    Except,
    Exists,
    In,
    Not,
    Between,
    And,
    Or,
    Like,
    ILike,
    Is,
    Null,
    True,
    False,
    Create,
    Table,
    Update,
    Set,
    Remove,
    Insert,
    If,
    Conflict,
    Ignore,
    Replace,
    Primary,
    Key,
    Default_,
    Explain,
    Describe,
    Show,
    Tables,
    Over,
    Partition,
    Window,
    Coalesce,
    Nullif,
    Ifnull,
    Cast,
    Now,
    Any,
    Some,

    // Aggregate function names (treated as keywords)
    Count,
    Sum,
    Avg,
    Min,
    Max,
    CountDistinct,

    // Symbols
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Dot,
    Colon,
    Semicolon,
    Star,
    Plus,
    Minus,
    Slash,
    Percent,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    Concat,
    CastOp,
    At,
    Arrow,

    // Literals
    Ident(String),
    StringLit(String),
    Integer(i64),
    Float(f64),
    Param(String),
    Interval(i64, IntervalSuffix),
}

impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Token::Float(a), Token::Float(b)) => a.to_bits() == b.to_bits(),
            (Token::Integer(a), Token::Integer(b)) => a == b,
            (Token::Interval(a, ua), Token::Interval(b, ub)) => a == b && ua == ub,
            (Token::StringLit(a), Token::StringLit(b)) => a == b,
            (Token::Ident(a), Token::Ident(b)) => a == b,
            (Token::Param(a), Token::Param(b)) => a == b,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl Eq for Token {}

impl std::hash::Hash for Token {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Token::Float(v) => v.to_bits().hash(state),
            Token::Integer(v) => v.hash(state),
            Token::Interval(v, u) => {
                v.hash(state);
                u.hash(state);
            }
            Token::StringLit(s) => s.hash(state),
            Token::Ident(s) => s.hash(state),
            Token::Param(s) => s.hash(state),
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntervalSuffix {
    Year,
    Month,
    Week,
    Day,
    Hour,
    Minute,
    Second,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub token: T,
    pub span: Range<usize>,
}

impl<T> Spanned<T> {
    pub fn new(token: T, span: Range<usize>) -> Self {
        Self { token, span }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Find => write!(f, "find"),
            Token::With => write!(f, "with"),
            Token::Recursive => write!(f, "recursive"),
            Token::As => write!(f, "as"),
            Token::Where => write!(f, "where"),
            Token::From => write!(f, "from"),
            Token::Join => write!(f, "join"),
            Token::Left => write!(f, "left"),
            Token::Right => write!(f, "right"),
            Token::Full => write!(f, "full"),
            Token::Cross => write!(f, "cross"),
            Token::Inner => write!(f, "inner"),
            Token::On => write!(f, "on"),
            Token::Group => write!(f, "group"),
            Token::By => write!(f, "by"),
            Token::Having => write!(f, "having"),
            Token::Order => write!(f, "order"),
            Token::Asc => write!(f, "asc"),
            Token::Desc => write!(f, "desc"),
            Token::Nulls => write!(f, "nulls"),
            Token::First => write!(f, "first"),
            Token::Last => write!(f, "last"),
            Token::Limit => write!(f, "limit"),
            Token::Offset => write!(f, "offset"),
            Token::Distinct => write!(f, "distinct"),
            Token::Case => write!(f, "case"),
            Token::When => write!(f, "when"),
            Token::Then => write!(f, "then"),
            Token::Else => write!(f, "else"),
            Token::End => write!(f, "end"),
            Token::Union => write!(f, "union"),
            Token::All => write!(f, "all"),
            Token::Intersect => write!(f, "intersect"),
            Token::Except => write!(f, "except"),
            Token::Exists => write!(f, "exists"),
            Token::In => write!(f, "in"),
            Token::Not => write!(f, "not"),
            Token::Between => write!(f, "between"),
            Token::And => write!(f, "and"),
            Token::Or => write!(f, "or"),
            Token::Like => write!(f, "like"),
            Token::ILike => write!(f, "ilike"),
            Token::Is => write!(f, "is"),
            Token::Null => write!(f, "null"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::Create => write!(f, "create"),
            Token::Table => write!(f, "table"),
            Token::Update => write!(f, "update"),
            Token::Set => write!(f, "set"),
            Token::Remove => write!(f, "remove"),
            Token::Insert => write!(f, "insert"),
            Token::If => write!(f, "if"),
            Token::Conflict => write!(f, "conflict"),
            Token::Ignore => write!(f, "ignore"),
            Token::Replace => write!(f, "replace"),
            Token::Primary => write!(f, "primary"),
            Token::Key => write!(f, "key"),
            Token::Default_ => write!(f, "default"),
            Token::Explain => write!(f, "explain"),
            Token::Describe => write!(f, "describe"),
            Token::Show => write!(f, "show"),
            Token::Tables => write!(f, "tables"),
            Token::Over => write!(f, "over"),
            Token::Partition => write!(f, "partition"),
            Token::Window => write!(f, "window"),
            Token::Coalesce => write!(f, "coalesce"),
            Token::Nullif => write!(f, "nullif"),
            Token::Ifnull => write!(f, "ifnull"),
            Token::Cast => write!(f, "cast"),
            Token::Now => write!(f, "now"),
            Token::Any => write!(f, "any"),
            Token::Some => write!(f, "some"),
            Token::Count => write!(f, "count"),
            Token::Sum => write!(f, "sum"),
            Token::Avg => write!(f, "avg"),
            Token::Min => write!(f, "min"),
            Token::Max => write!(f, "max"),
            Token::CountDistinct => write!(f, "count_distinct"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::Comma => write!(f, ","),
            Token::Dot => write!(f, "."),
            Token::Colon => write!(f, ":"),
            Token::Semicolon => write!(f, ";"),
            Token::Star => write!(f, "*"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Slash => write!(f, "/"),
            Token::Percent => write!(f, "%"),
            Token::Eq => write!(f, "="),
            Token::Neq => write!(f, "!="),
            Token::Lt => write!(f, "<"),
            Token::Gt => write!(f, ">"),
            Token::Lte => write!(f, "<="),
            Token::Gte => write!(f, ">="),
            Token::Concat => write!(f, "||"),
            Token::CastOp => write!(f, "::"),
            Token::At => write!(f, "@"),
            Token::Arrow => write!(f, ">>"),
            Token::Ident(s) => write!(f, "{}", s),
            Token::StringLit(s) => write!(f, "\"{}\"", s),
            Token::Integer(n) => write!(f, "{}", n),
            Token::Float(n) => write!(f, "{}", n),
            Token::Param(s) => write!(f, ":{}", s),
            Token::Interval(n, u) => write!(f, "{}{:?}", n, u),
        }
    }
}

pub struct Lexer<'a> {
    #[allow(dead_code)]
    input: &'a str,
    chars: Vec<char>,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.advance();
        }
    }

    fn skip_block_comment(&mut self) {
        let mut depth = 1;
        while depth > 0 {
            match (self.advance(), self.peek()) {
                (Some('/'), Some('*')) => {
                    self.advance();
                    depth += 1;
                }
                (Some('*'), Some('/')) => {
                    self.advance();
                    depth -= 1;
                }
                (None, _) => break,
                _ => {}
            }
        }
    }

    fn read_string(&mut self, start: usize) -> Spanned<Token> {
        let mut s = String::new();
        // The opening quote was already consumed
        loop {
            match self.advance() {
                Some('"') => break,
                Some('\\') => {
                    if let Some(next) = self.advance() {
                        match next {
                            '"' => s.push('"'),
                            '\\' => s.push('\\'),
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'r' => s.push('\r'),
                            c => {
                                s.push('\\');
                                s.push(c);
                            }
                        }
                    }
                }
                Some(c) => s.push(c),
                None => break,
            }
        }
        let end = self.pos;
        Spanned::new(Token::StringLit(s), start..end)
    }

    fn read_number(&mut self, start: usize, first_char: char) -> Spanned<Token> {
        let mut num_str = String::new();
        num_str.push(first_char);

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' || c == '_' {
                num_str.push(c);
                self.advance();
            } else {
                break;
            }
        }

        // Check for interval suffix
        if let Some(c) = self.peek()
            && c.is_alphabetic() {
                let suffix_start = self.pos;
                let mut suffix = String::new();
                while let Some(c) = self.peek() {
                    if c.is_alphabetic() {
                        suffix.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                let suffix_lower = suffix.to_lowercase();
                let interval_unit = match suffix_lower.as_str() {
                    "y" => Some(IntervalSuffix::Year),
                    "mon" | "months" => Some(IntervalSuffix::Month),
                    "w" | "weeks" => Some(IntervalSuffix::Week),
                    "d" | "days" => Some(IntervalSuffix::Day),
                    "h" | "hours" => Some(IntervalSuffix::Hour),
                    "m" | "minutes" => Some(IntervalSuffix::Minute),
                    "s" | "seconds" => Some(IntervalSuffix::Second),
                    _ => None,
                };

                if let Some(unit) = interval_unit {
                    let value: i64 = num_str.replace('_', "").parse().unwrap_or_else(|e| {
                        warn!("invalid interval value '{}': {}", num_str, e);
                        0
                    });
                    let end = self.pos;
                    return Spanned::new(Token::Interval(value, unit), start..end);
                }

                self.pos = suffix_start;
            }

        let end = self.pos;
        let clean = num_str.replace('_', "");

        if clean.contains('.') {
            let val: f64 = clean.parse().unwrap_or_else(|e| {
                warn!("invalid float literal '{}': {}", clean, e);
                0.0
            });
            Spanned::new(Token::Float(val), start..end)
        } else {
            let val: i64 = clean.parse().unwrap_or_else(|e| {
                warn!("invalid integer literal '{}': {}", clean, e);
                0
            });
            Spanned::new(Token::Integer(val), start..end)
        }
    }

    fn read_ident_or_keyword(&mut self, start: usize, first_char: char) -> Spanned<Token> {
        let mut ident = String::new();
        ident.push(first_char);

        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                ident.push(c);
                self.advance();
            } else {
                break;
            }
        }

        let end = self.pos;
        let lower = ident.to_lowercase();

        match lower.as_str() {
            "find" => Spanned::new(Token::Find, start..end),
            "with" => Spanned::new(Token::With, start..end),
            "recursive" => Spanned::new(Token::Recursive, start..end),
            "as" => Spanned::new(Token::As, start..end),
            "where" => Spanned::new(Token::Where, start..end),
            "from" => Spanned::new(Token::From, start..end),
            "join" => Spanned::new(Token::Join, start..end),
            "left" => Spanned::new(Token::Left, start..end),
            "right" => Spanned::new(Token::Right, start..end),
            "full" => Spanned::new(Token::Full, start..end),
            "cross" => Spanned::new(Token::Cross, start..end),
            "inner" => Spanned::new(Token::Inner, start..end),
            "on" => Spanned::new(Token::On, start..end),
            "group" => Spanned::new(Token::Group, start..end),
            "by" => Spanned::new(Token::By, start..end),
            "having" => Spanned::new(Token::Having, start..end),
            "order" => Spanned::new(Token::Order, start..end),
            "asc" => Spanned::new(Token::Asc, start..end),
            "desc" => Spanned::new(Token::Desc, start..end),
            "nulls" => Spanned::new(Token::Nulls, start..end),
            "first" => Spanned::new(Token::First, start..end),
            "last" => Spanned::new(Token::Last, start..end),
            "limit" => Spanned::new(Token::Limit, start..end),
            "offset" => Spanned::new(Token::Offset, start..end),
            "distinct" => Spanned::new(Token::Distinct, start..end),
            "case" => Spanned::new(Token::Case, start..end),
            "when" => Spanned::new(Token::When, start..end),
            "then" => Spanned::new(Token::Then, start..end),
            "else" => Spanned::new(Token::Else, start..end),
            "end" => Spanned::new(Token::End, start..end),
            "union" => Spanned::new(Token::Union, start..end),
            "all" => Spanned::new(Token::All, start..end),
            "intersect" => Spanned::new(Token::Intersect, start..end),
            "except" => Spanned::new(Token::Except, start..end),
            "exists" => Spanned::new(Token::Exists, start..end),
            "in" => Spanned::new(Token::In, start..end),
            "not" => Spanned::new(Token::Not, start..end),
            "between" => Spanned::new(Token::Between, start..end),
            "and" => Spanned::new(Token::And, start..end),
            "or" => Spanned::new(Token::Or, start..end),
            "like" => Spanned::new(Token::Like, start..end),
            "ilike" => Spanned::new(Token::ILike, start..end),
            "is" => Spanned::new(Token::Is, start..end),
            "null" => Spanned::new(Token::Null, start..end),
            "true" => Spanned::new(Token::True, start..end),
            "false" => Spanned::new(Token::False, start..end),
            "create" => Spanned::new(Token::Create, start..end),
            "table" => Spanned::new(Token::Table, start..end),
            "update" => Spanned::new(Token::Update, start..end),
            "set" => Spanned::new(Token::Set, start..end),
            "remove" => Spanned::new(Token::Remove, start..end),
            "insert" => Spanned::new(Token::Insert, start..end),
            "if" => Spanned::new(Token::If, start..end),
            "conflict" => Spanned::new(Token::Conflict, start..end),
            "ignore" => Spanned::new(Token::Ignore, start..end),
            "replace" => Spanned::new(Token::Replace, start..end),
            "primary" => Spanned::new(Token::Primary, start..end),
            "key" => Spanned::new(Token::Key, start..end),
            "default" => Spanned::new(Token::Default_, start..end),
            "explain" => Spanned::new(Token::Explain, start..end),
            "describe" => Spanned::new(Token::Describe, start..end),
            "show" => Spanned::new(Token::Show, start..end),
            "tables" => Spanned::new(Token::Tables, start..end),
            "over" => Spanned::new(Token::Over, start..end),
            "partition" => Spanned::new(Token::Partition, start..end),
            "window" => Spanned::new(Token::Window, start..end),
            "coalesce" => Spanned::new(Token::Coalesce, start..end),
            "nullif" => Spanned::new(Token::Nullif, start..end),
            "ifnull" => Spanned::new(Token::Ifnull, start..end),
            "cast" => Spanned::new(Token::Cast, start..end),
            "now" => Spanned::new(Token::Now, start..end),
            "any" => Spanned::new(Token::Any, start..end),
            "some" => Spanned::new(Token::Some, start..end),
            "count" => Spanned::new(Token::Count, start..end),
            "sum" => Spanned::new(Token::Sum, start..end),
            "avg" => Spanned::new(Token::Avg, start..end),
            "min" => Spanned::new(Token::Min, start..end),
            "max" => Spanned::new(Token::Max, start..end),
            "count_distinct" => Spanned::new(Token::CountDistinct, start..end),
            _ => Spanned::new(Token::Ident(ident), start..end),
        }
    }

    pub fn tokenize(&mut self) -> Vec<Spanned<Token>> {
        let mut tokens = Vec::new();

        while self.pos < self.chars.len() {
            self.skip_whitespace();

            let start = self.pos;
            let c = match self.peek() {
                Some(c) => c,
                None => break,
            };

            match c {
                '-' => {
                    if self.peek_next() == Some('-') {
                        self.advance();
                        self.advance();
                        self.skip_line_comment();
                        continue;
                    }
                    self.advance();
                    tokens.push(Spanned::new(Token::Minus, start..self.pos));
                }
                '/' => {
                    if self.peek_next() == Some('*') {
                        self.advance();
                        self.advance();
                        self.skip_block_comment();
                        continue;
                    }
                    self.advance();
                    tokens.push(Spanned::new(Token::Slash, start..self.pos));
                }
                '"' => {
                    self.advance();
                    tokens.push(self.read_string(start));
                }
                '(' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::LParen, start..self.pos));
                }
                ')' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::RParen, start..self.pos));
                }
                '[' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::LBracket, start..self.pos));
                }
                ']' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::RBracket, start..self.pos));
                }
                '{' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::LBrace, start..self.pos));
                }
                '}' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::RBrace, start..self.pos));
                }
                ',' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::Comma, start..self.pos));
                }
                '.' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::Dot, start..self.pos));
                }
                ';' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::Semicolon, start..self.pos));
                }
                '*' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::Star, start..self.pos));
                }
                '+' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::Plus, start..self.pos));
                }
                '%' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::Percent, start..self.pos));
                }
                '=' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::Eq, start..self.pos));
                }
                '!' => {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Spanned::new(Token::Neq, start..self.pos));
                    } else {
                        // Single '!' is not a valid token, but we can handle it
                        tokens.push(Spanned::new(Token::Neq, start..self.pos));
                    }
                }
                '<' => {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Spanned::new(Token::Lte, start..self.pos));
                    } else if self.peek() == Some('>') {
                        self.advance();
                        tokens.push(Spanned::new(Token::Neq, start..self.pos));
                    } else {
                        tokens.push(Spanned::new(Token::Lt, start..self.pos));
                    }
                }
                '>' => {
                    self.advance();
                    if self.peek() == Some('>') {
                        self.advance();
                        tokens.push(Spanned::new(Token::Arrow, start..self.pos));
                    } else if self.peek() == Some('=') {
                        self.advance();
                        tokens.push(Spanned::new(Token::Gte, start..self.pos));
                    } else {
                        tokens.push(Spanned::new(Token::Gt, start..self.pos));
                    }
                }
                '|' => {
                    self.advance();
                    if self.peek() == Some('|') {
                        self.advance();
                        tokens.push(Spanned::new(Token::Concat, start..self.pos));
                    } else {
                        // Single | not recognized
                        tokens.push(Spanned::new(Token::Concat, start..self.pos));
                    }
                }
                ':' => {
                    self.advance();
                    if self.peek() == Some(':') {
                        self.advance();
                        tokens.push(Spanned::new(Token::CastOp, start..self.pos));
                    } else if self.peek().is_some_and(|c| c.is_alphabetic() || c == '_') {
                        // Named parameter
                        let mut name = String::new();
                        while let Some(c) = self.peek() {
                            if c.is_alphanumeric() || c == '_' {
                                name.push(c);
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        tokens.push(Spanned::new(Token::Param(name), start..self.pos));
                    } else {
                        tokens.push(Spanned::new(Token::Colon, start..self.pos));
                    }
                }
                '@' => {
                    self.advance();
                    tokens.push(Spanned::new(Token::At, start..self.pos));
                }
                c if c.is_ascii_digit() => {
                    self.advance();
                    tokens.push(self.read_number(start, c));
                }
                c if c.is_alphabetic() || c == '_' => {
                    self.advance();
                    tokens.push(self.read_ident_or_keyword(start, c));
                }
                _ => {
                    // Skip unknown characters
                    self.advance();
                }
            }
        }

        tokens
    }
}

pub fn lex(input: &str) -> Vec<Spanned<Token>> {
    let mut lexer = Lexer::new(input);
    lexer.tokenize()
}
