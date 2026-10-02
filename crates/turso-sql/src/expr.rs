//! SQL expressions and conditions, modeled by [`Expr`] and [`Condition`].
//!
//! An [`Expr`] is a plain tree: the builder methods on it only construct
//! nodes and never render anything, so an expression can be inspected,
//! cloned and reused across statements. Rendering, including the decision of
//! where parentheses go, belongs to the writer. The module owns the shape of
//! the tree and the convenience constructors on it; it does not own the list
//! of SQL functions Turso supports — [`Func::call`] accepts any name so new
//! engine functions never require a release of this crate.
//!
//! A [`Condition`] is the `WHERE` / `HAVING` building block: an `AND` or
//! `OR` group that flattens into one [`Expr`] on demand. `OR` groups and
//! negations always parenthesise themselves when flattened so that mixing
//! `AND` and `OR` never depends on the reader knowing SQL precedence rules.
//! An empty condition renders as nothing, which is what lets builders start
//! from `Condition::all()` and add filters incrementally.

use crate::iden::{ColumnRef, Ident, IntoIden};
use crate::query::Select;
use crate::value::Value;

/// A sort direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Ascending (`ASC`).
    Asc,
    /// Descending (`DESC`).
    Desc,
}

/// The binary operators an [`Expr::Binary`] node can carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BinOp {
    /// The `=` operator.
    Eq,
    /// The `<>` operator.
    Ne,
    /// The `<` operator.
    Lt,
    /// The `<=` operator.
    Lte,
    /// The `>` operator.
    Gt,
    /// The `>=` operator.
    Gte,
    /// The `AND` operator.
    And,
    /// The `OR` operator.
    Or,
    /// The `IS` operator.
    Is,
    /// The `IS NOT` operator.
    IsNot,
    /// The `+` operator.
    Add,
    /// The `-` operator.
    Sub,
    /// The `*` operator.
    Mul,
    /// The `/` operator.
    Div,
    /// The `%` operator.
    Mod,
    /// The `||` string concatenation operator.
    Concat,
    /// The `->` JSON extraction operator.
    JsonArrow,
    /// The `->>` JSON extraction operator, yielding a SQL value.
    JsonArrowText,
    /// The `MATCH` full-text search operator.
    Match,
}

impl BinOp {
    /// The operator's SQL spelling.
    pub(crate) fn sql(self) -> &'static str {
        match self {
            BinOp::Eq => "=",
            BinOp::Ne => "<>",
            BinOp::Lt => "<",
            BinOp::Lte => "<=",
            BinOp::Gt => ">",
            BinOp::Gte => ">=",
            BinOp::And => "AND",
            BinOp::Or => "OR",
            BinOp::Is => "IS",
            BinOp::IsNot => "IS NOT",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Concat => "||",
            BinOp::JsonArrow => "->",
            BinOp::JsonArrowText => "->>",
            BinOp::Match => "MATCH",
        }
    }
}

/// A SQL expression.
///
/// The enum is `#[non_exhaustive]` so that new node kinds can be added
/// without breaking downstream matches; construct nodes through the
/// associated functions and methods rather than the variants directly.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[must_use = "an expression does nothing until used in a statement"]
pub enum Expr {
    /// A column reference.
    Column(ColumnRef),
    /// A bound value.
    Value(Value),
    /// A parenthesised list of expressions, for example the right-hand side
    /// of `IN`.
    Tuple(Vec<Expr>),
    /// A binary operation `lhs op rhs`.
    Binary(Box<Expr>, BinOp, Box<Expr>),
    /// A logical negation `NOT expr`.
    Not(Box<Expr>),
    /// An arithmetic negation `-expr`.
    Neg(Box<Expr>),
    /// The `expr IS NULL` test.
    IsNull(Box<Expr>),
    /// The `expr IS NOT NULL` test.
    IsNotNull(Box<Expr>),
    /// The `expr IN (...)` membership test.
    In(Box<Expr>, Box<Expr>),
    /// The `expr NOT IN (...)` membership test.
    NotIn(Box<Expr>, Box<Expr>),
    /// The `expr BETWEEN a AND b` range test.
    Between(Box<Expr>, Box<Expr>, Box<Expr>),
    /// The `expr NOT BETWEEN a AND b` range test.
    NotBetween(Box<Expr>, Box<Expr>, Box<Expr>),
    /// A pattern test `expr [NOT] LIKE pattern [ESCAPE 'c']`.
    ///
    /// The escape character is carried on the node because SQLite treats a
    /// backslash in a pattern literally unless the statement says
    /// otherwise; the `contains` family sets it so that `%` and `_` in
    /// user input match themselves.
    Like {
        /// The tested expression.
        expr: Box<Expr>,
        /// The pattern, normally a bound value.
        pattern: Box<Expr>,
        /// Whether the test is `NOT LIKE`.
        negated: bool,
        /// The `ESCAPE` character, when the pattern uses one.
        escape: Option<char>,
    },
    /// A function call `name(args)`.
    Func(Func),
    /// A scalar subquery `(SELECT ...)`.
    Subquery(Box<Select>),
    /// An existence test `EXISTS (SELECT ...)`.
    Exists(Box<Select>),
    /// A `CASE WHEN ... THEN ... ELSE ... END` expression.
    Case(Vec<(Expr, Expr)>, Option<Box<Expr>>),
    /// A type conversion `CAST(expr AS type)`.
    Cast(Box<Expr>, &'static str),
    /// An aliased expression `expr AS alias`, only meaningful in select
    /// lists.
    Alias(Box<Expr>, Ident),
    /// Raw SQL inserted verbatim, with `?` placeholders for the values
    /// carried in `.1`.
    Raw(String, Vec<Value>),
    /// An explicitly parenthesised expression `(expr)`.
    Paren(Box<Expr>),
}

/// A function call.
#[derive(Clone, Debug, PartialEq)]
pub struct Func {
    /// The function name, written verbatim.
    pub name: &'static str,
    /// The arguments.
    pub args: Vec<Expr>,
    /// Whether `DISTINCT` applies to the first argument, as in aggregates.
    pub distinct: bool,
}

impl Func {
    /// Builds a call to any function.
    ///
    /// The name is written verbatim, so this is the escape hatch for engine
    /// functions that have no dedicated constructor.
    pub fn call(name: &'static str, args: impl IntoIterator<Item = Expr>) -> Expr {
        Expr::Func(Func {
            name,
            args: args.into_iter().collect(),
            distinct: false,
        })
    }

    /// Builds `COUNT(expr)`.
    pub fn count(expr: Expr) -> Expr {
        Self::call("COUNT", [expr])
    }

    /// Builds `COUNT(*)`.
    pub fn count_star() -> Expr {
        Self::call("COUNT", [Expr::Column(ColumnRef::Asterisk)])
    }

    /// Builds `COUNT(DISTINCT expr)`.
    pub fn count_distinct(expr: Expr) -> Expr {
        Expr::Func(Func {
            name: "COUNT",
            args: vec![expr],
            distinct: true,
        })
    }

    /// Builds `MAX(expr)`.
    pub fn max(expr: Expr) -> Expr {
        Self::call("MAX", [expr])
    }

    /// Builds `MIN(expr)`.
    pub fn min(expr: Expr) -> Expr {
        Self::call("MIN", [expr])
    }

    /// Builds `SUM(expr)`.
    pub fn sum(expr: Expr) -> Expr {
        Self::call("SUM", [expr])
    }

    /// Builds `AVG(expr)`.
    pub fn avg(expr: Expr) -> Expr {
        Self::call("AVG", [expr])
    }

    /// Builds `COALESCE(a, b, ...)`.
    pub fn coalesce(args: impl IntoIterator<Item = Expr>) -> Expr {
        Self::call("COALESCE", args)
    }

    /// Builds `LOWER(expr)`.
    pub fn lower(expr: Expr) -> Expr {
        Self::call("LOWER", [expr])
    }

    /// Builds `UPPER(expr)`.
    pub fn upper(expr: Expr) -> Expr {
        Self::call("UPPER", [expr])
    }

    /// Builds `LENGTH(expr)`.
    pub fn length(expr: Expr) -> Expr {
        Self::call("LENGTH", [expr])
    }

    /// Builds `ABS(expr)`.
    pub fn abs(expr: Expr) -> Expr {
        Self::call("ABS", [expr])
    }

    /// Builds `IFNULL(a, b)`.
    pub fn if_null(a: Expr, b: Expr) -> Expr {
        Self::call("IFNULL", [a, b])
    }

    /// Builds `json_extract(json, path)`, binding `path` as a parameter.
    pub fn json_extract(json: Expr, path: impl Into<Value>) -> Expr {
        Self::call("json_extract", [json, Expr::Value(path.into())])
    }

    /// Builds `vector_distance_cos(a, b)` — Turso vector search.
    pub fn vector_distance_cos(a: Expr, b: Expr) -> Expr {
        Self::call("vector_distance_cos", [a, b])
    }

    /// Builds `vector_distance_l2(a, b)` — Turso vector search.
    pub fn vector_distance_l2(a: Expr, b: Expr) -> Expr {
        Self::call("vector_distance_l2", [a, b])
    }

    /// Builds `vector32(text)` — the Turso vector constructor.
    pub fn vector32(expr: Expr) -> Expr {
        Self::call("vector32", [expr])
    }

    /// Builds `fts_match(column, query)` — Turso full-text search, binding
    /// `query` as a parameter.
    pub fn fts_match(column: Expr, query: impl Into<Value>) -> Expr {
        Self::call("fts_match", [column, Expr::Value(query.into())])
    }

    /// Builds `fts_score(column)` — Turso full-text search ranking.
    pub fn fts_score(column: Expr) -> Expr {
        Self::call("fts_score", [column])
    }
}

/// Builds a binary node, boxing both operands.
fn bin(lhs: Expr, op: BinOp, rhs: Expr) -> Expr {
    Expr::Binary(Box::new(lhs), op, Box::new(rhs))
}

impl Expr {
    /// A column reference, from `"name"` or `("table", "name")`.
    pub fn col(column: impl Into<ColumnRef>) -> Self {
        Expr::Column(column.into())
    }

    /// A bound value.
    pub fn val(value: impl Into<Value>) -> Self {
        Expr::Value(value.into())
    }

    /// A tuple of bound values.
    pub fn tuple<V: Into<Value>>(values: impl IntoIterator<Item = V>) -> Self {
        Expr::Tuple(values.into_iter().map(|v| Expr::Value(v.into())).collect())
    }

    /// A scalar subquery.
    pub fn subquery(select: Select) -> Self {
        Expr::Subquery(Box::new(select))
    }

    /// An `EXISTS (subquery)` test.
    pub fn exists(select: Select) -> Self {
        Expr::Exists(Box::new(select))
    }

    /// Raw SQL with `?` placeholders and no values.
    pub fn cust(sql: impl Into<String>) -> Self {
        Expr::Raw(sql.into(), Vec::new())
    }

    /// Raw SQL with `?` placeholders and the values that fill them, in
    /// order.
    pub fn cust_with_values<V: Into<Value>>(
        sql: impl Into<String>,
        values: impl IntoIterator<Item = V>,
    ) -> Self {
        Expr::Raw(sql.into(), values.into_iter().map(Into::into).collect())
    }

    /// Builds `self = rhs`.
    pub fn eq(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Eq, rhs.into())
    }

    /// Builds `self <> rhs`.
    pub fn ne(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Ne, rhs.into())
    }

    /// Builds `self < rhs`.
    pub fn lt(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Lt, rhs.into())
    }

    /// Builds `self <= rhs`.
    pub fn lte(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Lte, rhs.into())
    }

    /// Builds `self > rhs`.
    pub fn gt(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Gt, rhs.into())
    }

    /// Builds `self >= rhs`.
    pub fn gte(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Gte, rhs.into())
    }

    /// Builds `self AND rhs`.
    pub fn and(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::And, rhs.into())
    }

    /// Builds `self OR rhs`.
    pub fn or(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Or, rhs.into())
    }

    /// Builds `NOT self`.
    #[allow(
        clippy::should_implement_trait,
        reason = "the SQL-flavoured name reads as the operator it builds"
    )]
    pub fn not(self) -> Self {
        Expr::Not(Box::new(self))
    }

    /// Builds `-self`.
    #[allow(
        clippy::should_implement_trait,
        reason = "the SQL-flavoured name reads as the operator it builds"
    )]
    pub fn neg(self) -> Self {
        Expr::Neg(Box::new(self))
    }

    /// Builds `self IS NULL`.
    pub fn is_null(self) -> Self {
        Expr::IsNull(Box::new(self))
    }

    /// Builds `self IS NOT NULL`.
    pub fn is_not_null(self) -> Self {
        Expr::IsNotNull(Box::new(self))
    }

    /// Builds `self IS rhs`, the null-safe equality.
    pub fn is(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Is, rhs.into())
    }

    /// Builds `self IS NOT rhs`.
    pub fn is_not(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::IsNot, rhs.into())
    }

    /// Builds `self LIKE pattern`, binding the pattern as a parameter.
    ///
    /// The pattern is taken as written: `%` and `_` are wildcards and no
    /// `ESCAPE` clause is emitted. See [`like_escaped`](Self::like_escaped)
    /// for a pattern that needs one.
    pub fn like(self, pattern: impl Into<Value>) -> Self {
        Expr::Like {
            expr: Box::new(self),
            pattern: Box::new(Expr::Value(pattern.into())),
            negated: false,
            escape: None,
        }
    }

    /// Builds `self NOT LIKE pattern`, binding the pattern as a parameter.
    pub fn not_like(self, pattern: impl Into<Value>) -> Self {
        Expr::Like {
            expr: Box::new(self),
            pattern: Box::new(Expr::Value(pattern.into())),
            negated: true,
            escape: None,
        }
    }

    /// Builds `self LIKE pattern ESCAPE 'escape'`, for a pattern in which
    /// `escape` precedes every wildcard that must match literally.
    pub fn like_escaped(self, pattern: impl Into<Value>, escape: char) -> Self {
        Expr::Like {
            expr: Box::new(self),
            pattern: Box::new(Expr::Value(pattern.into())),
            negated: false,
            escape: Some(escape),
        }
    }

    /// Builds `self MATCH query` for full-text search, binding the query as
    /// a parameter.
    pub fn matches(self, query: impl Into<Value>) -> Self {
        bin(self, BinOp::Match, Expr::Value(query.into()))
    }

    /// Builds `self IN (values)`.
    ///
    /// An empty list renders as `IN (NULL)`, which matches nothing, so a
    /// filter built from an empty collection behaves as expected instead of
    /// producing a syntax error.
    pub fn is_in<V: Into<Value>>(self, values: impl IntoIterator<Item = V>) -> Self {
        Expr::In(Box::new(self), Box::new(Expr::tuple(values)))
    }

    /// Builds `self NOT IN (values)`.
    pub fn is_not_in<V: Into<Value>>(self, values: impl IntoIterator<Item = V>) -> Self {
        Expr::NotIn(Box::new(self), Box::new(Expr::tuple(values)))
    }

    /// Builds `self IN (subquery)`.
    pub fn in_subquery(self, select: Select) -> Self {
        Expr::In(Box::new(self), Box::new(Expr::subquery(select)))
    }

    /// Builds `self NOT IN (subquery)`.
    pub fn not_in_subquery(self, select: Select) -> Self {
        Expr::NotIn(Box::new(self), Box::new(Expr::subquery(select)))
    }

    /// Builds `self BETWEEN a AND b`.
    pub fn between(self, a: impl Into<Expr>, b: impl Into<Expr>) -> Self {
        Expr::Between(Box::new(self), Box::new(a.into()), Box::new(b.into()))
    }

    /// Builds `self NOT BETWEEN a AND b`.
    pub fn not_between(self, a: impl Into<Expr>, b: impl Into<Expr>) -> Self {
        Expr::NotBetween(Box::new(self), Box::new(a.into()), Box::new(b.into()))
    }

    /// Builds `self + rhs`.
    #[allow(
        clippy::should_implement_trait,
        reason = "the SQL-flavoured name reads as the operator it builds"
    )]
    pub fn add(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Add, rhs.into())
    }

    /// Builds `self - rhs`.
    #[allow(
        clippy::should_implement_trait,
        reason = "the SQL-flavoured name reads as the operator it builds"
    )]
    pub fn sub(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Sub, rhs.into())
    }

    /// Builds `self * rhs`.
    #[allow(
        clippy::should_implement_trait,
        reason = "the SQL-flavoured name reads as the operator it builds"
    )]
    pub fn mul(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Mul, rhs.into())
    }

    /// Builds `self / rhs`.
    #[allow(
        clippy::should_implement_trait,
        reason = "the SQL-flavoured name reads as the operator it builds"
    )]
    pub fn div(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Div, rhs.into())
    }

    /// Builds `self % rhs`.
    #[allow(
        clippy::should_implement_trait,
        reason = "the SQL-flavoured name reads as the operator it builds"
    )]
    pub fn rem(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Mod, rhs.into())
    }

    /// Builds `self || rhs`.
    pub fn concat(self, rhs: impl Into<Expr>) -> Self {
        bin(self, BinOp::Concat, rhs.into())
    }

    /// Builds `self -> path`, JSON extraction keeping the JSON
    /// representation.
    pub fn json_get(self, path: impl Into<Value>) -> Self {
        bin(self, BinOp::JsonArrow, Expr::Value(path.into()))
    }

    /// Builds `self ->> path`, JSON extraction yielding a SQL value.
    pub fn json_get_text(self, path: impl Into<Value>) -> Self {
        bin(self, BinOp::JsonArrowText, Expr::Value(path.into()))
    }

    /// Builds `CAST(self AS type)`.
    pub fn cast_as(self, ty: &'static str) -> Self {
        Expr::Cast(Box::new(self), ty)
    }

    /// Builds `self AS alias`.
    pub fn alias(self, alias: impl IntoIden) -> Self {
        Expr::Alias(Box::new(self), alias.into_iden())
    }

    /// Builds `(self)`.
    pub fn paren(self) -> Self {
        Expr::Paren(Box::new(self))
    }

    /// Builds `CASE WHEN ... THEN ... ELSE ... END`.
    pub fn case(whens: Vec<(Expr, Expr)>, otherwise: Option<Expr>) -> Self {
        Expr::Case(whens, otherwise.map(Box::new))
    }

    /// Builds `self LIKE '%s%' ESCAPE '\\'`, with `%`, `_` and `\\` escaped in
    /// `s` so that the fragment matches literally.
    pub fn contains(self, s: &str) -> Self {
        self.like_escaped(format!("%{}%", escape_like(s)), LIKE_ESCAPE)
    }

    /// Builds `self LIKE 's%' ESCAPE '\\'`, with `%`, `_` and `\\` escaped in
    /// `s` so that the fragment matches literally.
    pub fn starts_with(self, s: &str) -> Self {
        self.like_escaped(format!("{}%", escape_like(s)), LIKE_ESCAPE)
    }

    /// Builds `self LIKE '%s' ESCAPE '\\'`, with `%`, `_` and `\\` escaped in
    /// `s` so that the fragment matches literally.
    pub fn ends_with(self, s: &str) -> Self {
        self.like_escaped(format!("%{}", escape_like(s)), LIKE_ESCAPE)
    }
}

/// The escape character the `contains` family declares in its `ESCAPE`
/// clause.
const LIKE_ESCAPE: char = '\\';

/// Escapes the `LIKE` wildcards and the escape character itself in a
/// user-supplied fragment so that, under `ESCAPE '\\'`, it matches literally.
fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '%' || c == '_' || c == LIKE_ESCAPE {
            out.push(LIKE_ESCAPE);
        }
        out.push(c);
    }
    out
}

impl<T: Into<Value>> From<T> for Expr {
    fn from(value: T) -> Self {
        Expr::Value(value.into())
    }
}

/// A composable `WHERE` / `HAVING` condition.
///
/// A condition is an `AND` or `OR` group of parts. Groups nest, so arbitrary
/// boolean shapes can be built without thinking about precedence: `OR`
/// groups and negations are parenthesised when flattened. An empty condition
/// holds trivially and renders as nothing.
///
/// ```
/// use turso_sql::prelude::*;
///
/// let cond = Condition::all()
///     .add(Expr::col("active").eq(true))
///     .add(Condition::any()
///         .add(Expr::col("role").eq("admin"))
///         .add(Expr::col("role").eq("owner")));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Condition {
    /// Whether the parts are joined with `AND` (`true`) or `OR` (`false`).
    all: bool,
    /// Whether the flattened group is wrapped in `NOT`.
    negate: bool,
    /// The parts, already flattened to expressions.
    parts: Vec<Expr>,
}

impl Condition {
    /// A condition that holds when every part holds (`AND`).
    ///
    /// An empty `all()` holds trivially.
    pub fn all() -> Self {
        Self {
            all: true,
            negate: false,
            parts: Vec::new(),
        }
    }

    /// A condition that holds when any part holds (`OR`).
    ///
    /// An empty `any()` holds trivially rather than failing, so that a
    /// filter built from an empty list of alternatives does not silently
    /// exclude every row.
    pub fn any() -> Self {
        Self {
            all: false,
            negate: false,
            parts: Vec::new(),
        }
    }

    /// Adds a part.
    ///
    /// Empty nested conditions are dropped rather than added, so an unused
    /// sub-group never leaves a stray `()` in the rendered SQL.
    #[must_use]
    #[allow(
        clippy::should_implement_trait,
        reason = "the name reads as the SQL it builds"
    )]
    pub fn add(mut self, part: impl IntoCondition) -> Self {
        if let Some(expr) = part.into_condition().into_expr() {
            self.parts.push(expr);
        }
        self
    }

    /// Adds a part when it is `Some`.
    #[must_use]
    pub fn add_option(self, part: Option<impl IntoCondition>) -> Self {
        match part {
            Some(p) => self.add(p),
            None => self,
        }
    }

    /// Negates the whole condition.
    #[must_use]
    #[allow(
        clippy::should_implement_trait,
        reason = "the name reads as the SQL it builds"
    )]
    pub fn not(mut self) -> Self {
        self.negate = !self.negate;
        self
    }

    /// Whether no part has been added.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// The number of parts.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Flattens the condition into one expression, or `None` when empty.
    pub fn into_expr(self) -> Option<Expr> {
        let op = if self.all { BinOp::And } else { BinOp::Or };
        // OR groups and negations are always parenthesised so that nesting
        // them inside an AND chain keeps the intended precedence explicit.
        let needs_paren = !self.all || self.negate;
        let negate = self.negate;
        let mut iter = self.parts.into_iter();
        let first = iter.next()?;
        let joined = iter.fold(first, |acc, e| bin(acc, op, e));
        let joined = if needs_paren {
            Expr::Paren(Box::new(joined))
        } else {
            joined
        };
        Some(if negate { joined.not() } else { joined })
    }
}

impl Default for Condition {
    fn default() -> Self {
        Condition::all()
    }
}

/// Anything usable as a condition: an [`Expr`] or a [`Condition`].
pub trait IntoCondition {
    /// Converts into a condition.
    fn into_condition(self) -> Condition;
}

impl IntoCondition for Condition {
    fn into_condition(self) -> Condition {
        self
    }
}

impl IntoCondition for Expr {
    fn into_condition(self) -> Condition {
        Condition::all().add_expr(self)
    }
}

impl Condition {
    /// Pushes an expression without flattening it, for the single-expression
    /// conversion above.
    fn add_expr(mut self, expr: Expr) -> Self {
        self.parts.push(expr);
        self
    }
}
