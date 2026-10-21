//! Fuses arithmetic on the built-in `Vec2` type into one constructor call.
//!
//! BrineVector, the runtime behind `Vec2`, allocates a new vector for every
//! operator, so `p + v * dt` creates two vectors each time it runs. When the
//! checker knows every operand's type, the whole expression can be emitted as
//! `vec2(p.x + v.x * dt, p.y + v.y * dt)`: one allocation, the same
//! floating-point operations in the same order, and no metamethod dispatch.
//!
//! Fusion only applies where a program cannot observe the difference:
//!
//! - every vector operand is a bare name, or a `vec2(...)` call whose arguments
//!   qualify as scalars, because each component reads it again. Field and
//!   index reads are excluded: Lua may run `__index` on every read;
//! - every scalar operand is a number expression built from bare names and
//!   literals, for the same reason;
//! - `/` is fused only by a nonzero numeric literal, because BrineVector raises
//!   an error when dividing by zero;
//! - no comment sits inside the expression, because the components are
//!   rebuilt from text. Comments before and after it are kept.
//!
//! Everything else keeps BrineVector's metamethods. `+=`, `-=`, `*=`, and `/=`
//! on a `Vec2` target are fused as the equivalent plain assignment.
//!
//! Field and index reads such as `self.pos` or `positions[i]` can still be
//! fused when the whole right-hand side of a statement is the `Vec2`
//! expression. Each read is bound to a local exactly once, in evaluation order,
//! inside a `do ... end` block, and the components use those locals:
//!
//! ```lua
//! do local __tua_self_pos = self.pos; local __tua_self_vel = self.vel; self.pos = vec2(__tua_self_pos.x + __tua_self_vel.x * dt, __tua_self_pos.y + __tua_self_vel.y * dt) end
//! ```
//!
//! This covers `target = expr`, `target op= expr`, `local name = expr`, and
//! `return expr`. The target must be a name, or a name with one field or key,
//! so evaluating it reads no table before the hoisted operands. A local whose
//! name appears in its own right-hand side is left alone, because declaring it
//! first would change what that name refers to.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use full_moon::ast::punctuated::{Pair, Punctuated};
use full_moon::ast::span::ContainedSpan;
use full_moon::ast::{
    self, BinOp, Call, CompoundOp, Expression, FunctionArgs, Index, LastStmt, LuaVersion, Prefix,
    Suffix, UnOp, Var,
};
use full_moon::node::Node;
use full_moon::tokenizer::{Symbol, Token, TokenReference, TokenType};
use full_moon::visitors::{Visitor, VisitorMut};

use crate::semantic::SemanticModel;
use crate::types::TypeEnvironment;
use crate::{vector, ParsedLanguage, ParsedSource, SourcePosition, Type};

// Lua operator precedence, lowest to highest, for the operators fusion emits.
const ADDITIVE: u8 = 5;
const MULTIPLICATIVE: u8 = 6;
const UNARY: u8 = 7;
const POWER: u8 = 8;
const ATOM: u8 = 9;

pub(super) fn fuse_vec2_arithmetic(
    parsed: &ParsedSource,
    environment: &TypeEnvironment,
    ast: ast::Ast,
) -> ast::Ast {
    // Planning queries the type of every arithmetic expression, so skip files
    // that cannot hold a Vec2. A file qualifies if it names `vec2` or `Vec2`,
    // or if its environment carries a Vec2 in some binding: values from
    // required modules and project inference arrive that way even when the
    // consuming file never spells the type. Every `--` comment contains `-`,
    // so an arithmetic-token check alone would let nearly every file through.
    if parsed.raw.language != ParsedLanguage::Tua
        || !parsed
            .source
            .bytes()
            .any(|byte| matches!(byte, b'+' | b'-' | b'*' | b'/'))
        || !(parsed.source.contains(vector::CONSTRUCTOR_NAME)
            || parsed.source.contains(vector::TYPE_NAME)
            || environment_mentions_vec2(environment))
    {
        return ast;
    }

    let identifiers = parsed
        .ast
        .tokens()
        .filter_map(|token| match token.token_type() {
            TokenType::Identifier { identifier } => Some(identifier.to_string()),
            _ => None,
        })
        .collect();
    let mut planner = Planner {
        semantic: SemanticModel::new(parsed, environment),
        environment,
        vec2: vector::vec2_type(),
        number: Type::number(),
        identifiers,
        hoisting: RefCell::new(None),
        claimed: Vec::new(),
        expressions: BTreeMap::new(),
        compounds: BTreeMap::new(),
        statements: BTreeMap::new(),
    };
    planner.visit_ast(&parsed.ast);

    if planner.expressions.is_empty()
        && planner.compounds.is_empty()
        && planner.statements.is_empty()
    {
        return ast;
    }

    Rewriter {
        expressions: planner.expressions,
        compounds: planner.compounds,
        statements: planner.statements,
    }
    .visit_ast(ast)
}

type ByteRange = (usize, usize);

/// Whether any local or function in the environment has a type that mentions
/// the built-in `Vec2`. Aliases are followed only when a binding refers to
/// them, so the large built-in catalogs are not scanned for every file.
fn environment_mentions_vec2(environment: &TypeEnvironment) -> bool {
    let vec2 = vector::vec2_type();
    let mut visited = BTreeSet::new();

    environment
        .locals()
        .any(|(_, ty)| type_mentions_vec2(environment, ty, &vec2, &mut visited))
        || environment.functions().iter().any(|function| {
            function
                .parameters
                .iter()
                .map(|parameter| &parameter.ty)
                .chain(function.return_types.iter())
                .chain(function.return_type.iter())
                .any(|ty| type_mentions_vec2(environment, ty, &vec2, &mut visited))
        })
}

fn type_mentions_vec2(
    environment: &TypeEnvironment,
    ty: &Type,
    vec2: &Type,
    visited: &mut BTreeSet<String>,
) -> bool {
    if ty == vec2 {
        return true;
    }
    match ty {
        Type::Alias(name) => {
            name == vector::TYPE_NAME
                || (visited.insert(name.clone())
                    && environment
                        .alias(name)
                        .is_some_and(|alias| type_mentions_vec2(environment, alias, vec2, visited)))
        }
        Type::Union(types) | Type::Tuple(types) => types
            .iter()
            .any(|ty| type_mentions_vec2(environment, ty, vec2, visited)),
        Type::Function(function) => function
            .parameters
            .iter()
            .map(|parameter| &parameter.ty)
            .chain(function.return_types.iter())
            .chain(function.vararg_type.as_deref())
            .any(|ty| type_mentions_vec2(environment, ty, vec2, visited)),
        Type::Table(table) => table
            .fields()
            .any(|(_, field)| type_mentions_vec2(environment, &field.ty, vec2, visited)),
        Type::Array(item) => type_mentions_vec2(environment, item, vec2, visited),
        Type::Dictionary { key, value } => {
            type_mentions_vec2(environment, key, vec2, visited)
                || type_mentions_vec2(environment, value, vec2, visited)
        }
        Type::Primitive(_) | Type::Any | Type::StringLiteral(_) => false,
    }
}

/// A field or index read bound to a local before a fused statement.
#[derive(Clone, Debug)]
struct HoistedRead {
    temp: String,
    read: String,
}

/// How a fused statement uses the constructed vector.
enum StatementForm {
    Assign(String),
    Local(String),
    Return,
}

#[derive(Clone, Debug)]
struct Fragment {
    text: String,
    precedence: u8,
}

impl Fragment {
    fn atom(text: String) -> Self {
        Self {
            text,
            precedence: ATOM,
        }
    }

    fn parenthesized(self) -> Self {
        Self::atom(format!("({})", self.text))
    }
}

#[derive(Clone, Debug)]
struct Components {
    x: Fragment,
    y: Fragment,
}

impl Components {
    fn of_read(text: &str) -> Self {
        Self {
            x: Fragment::atom(format!("{text}.x")),
            y: Fragment::atom(format!("{text}.y")),
        }
    }

    fn map(self, mut component: impl FnMut(Fragment) -> Fragment) -> Self {
        Self {
            x: component(self.x),
            y: component(self.y),
        }
    }

    fn zip(self, other: Self, operator: Arithmetic) -> Self {
        Self {
            x: binary(operator, self.x, other.x),
            y: binary(operator, self.y, other.y),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Arithmetic {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

impl Arithmetic {
    fn from_binop(binop: &BinOp) -> Option<Self> {
        match binop {
            BinOp::Plus(_) => Some(Self::Add),
            BinOp::Minus(_) => Some(Self::Sub),
            BinOp::Star(_) => Some(Self::Mul),
            BinOp::Slash(_) => Some(Self::Div),
            BinOp::Percent(_) => Some(Self::Mod),
            BinOp::Caret(_) => Some(Self::Pow),
            _ => None,
        }
    }

    fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::Pow => "^",
        }
    }

    fn precedence(self) -> u8 {
        match self {
            Self::Add | Self::Sub => ADDITIVE,
            Self::Mul | Self::Div | Self::Mod => MULTIPLICATIVE,
            Self::Pow => POWER,
        }
    }
}

/// Joins two fragments, adding parentheses only where Lua's precedence and
/// associativity would otherwise regroup them. The emitted tree, and so the
/// floating-point evaluation order, matches the one being fused.
fn binary(operator: Arithmetic, left: Fragment, right: Fragment) -> Fragment {
    let precedence = operator.precedence();
    let right_associative = matches!(operator, Arithmetic::Pow);
    let left =
        if left.precedence < precedence || (right_associative && left.precedence == precedence) {
            left.parenthesized()
        } else {
            left
        };
    let right = if right.precedence < precedence
        || (!right_associative && right.precedence == precedence)
    {
        right.parenthesized()
    } else {
        right
    };

    Fragment {
        text: format!("{} {} {}", left.text, operator.symbol(), right.text),
        precedence,
    }
}

fn negate(operand: Fragment) -> Fragment {
    // A leading `-` would turn `--` into a comment.
    let operand = if operand.precedence < UNARY || operand.text.starts_with('-') {
        operand.parenthesized()
    } else {
        operand
    };

    Fragment {
        text: format!("-{}", operand.text),
        precedence: UNARY,
    }
}

struct Planner<'a> {
    semantic: SemanticModel<'a>,
    environment: &'a TypeEnvironment,
    vec2: Type,
    number: Type,
    // Every identifier in the file, so hoisted locals never shadow one.
    identifiers: BTreeSet<String>,
    // Reads bound to locals while a statement is being planned; `None` means
    // only bare names may be operands.
    hoisting: RefCell<Option<Vec<HoistedRead>>>,
    claimed: Vec<ByteRange>,
    expressions: BTreeMap<ByteRange, Components>,
    compounds: BTreeMap<ByteRange, Components>,
    statements: BTreeMap<ByteRange, String>,
}

impl Visitor for Planner<'_> {
    fn visit_compound_assignment(&mut self, assignment: &ast::CompoundAssignment) {
        let Some(range) = byte_range(assignment) else {
            return;
        };
        if self.is_claimed(range) {
            return;
        }
        // The target and the operator are copied with their comments, and the
        // right-hand side keeps the comments around it, so only a comment
        // inside the right-hand side blocks fusion.
        if !contains_interior_comment(assignment.rhs()) {
            if let Some(components) = self.compound(assignment) {
                self.claimed.push(range);
                self.compounds.insert(range, components);
                return;
            }
        }

        // The hoisted form re-renders the target, so no comment may sit
        // anywhere inside the statement.
        if contains_interior_comment(assignment) {
            return;
        }
        let Some(target) = self.target_text(assignment.lhs()) else {
            return;
        };
        if let Some((components, reads)) = self.with_hoisting(|| self.compound(assignment)) {
            self.claim_statement(
                assignment,
                range,
                StatementForm::Assign(target),
                &components,
                reads,
            );
        }
    }

    fn visit_assignment(&mut self, assignment: &ast::Assignment) {
        let (Some(var), Some(root)) = (
            single(assignment.variables()),
            single(assignment.expressions()),
        ) else {
            return;
        };
        let Some(target) = self.target_text(var) else {
            return;
        };
        self.plan_statement(assignment, root, StatementForm::Assign(target));
    }

    fn visit_local_assignment(&mut self, assignment: &ast::LocalAssignment) {
        let (Some(name), Some(root)) =
            (single(assignment.names()), single(assignment.expressions()))
        else {
            return;
        };
        let name = token_text(name);
        let refers_to_name = root.tokens().any(|token| {
            matches!(token.token_type(), TokenType::Identifier { identifier } if identifier.as_str() == name)
        });
        if refers_to_name {
            return;
        }
        self.plan_statement(assignment, root, StatementForm::Local(name));
    }

    fn visit_return(&mut self, statement: &ast::Return) {
        let Some(root) = single(statement.returns()) else {
            return;
        };
        self.plan_statement(statement, root, StatementForm::Return);
    }

    // Expressions are visited outermost first, so the first fusible node of a
    // tree is its largest fusible root and everything inside it is claimed.
    fn visit_expression(&mut self, expression: &Expression) {
        if !is_vector_operator(expression) {
            return;
        }
        let Some(range) = byte_range(expression) else {
            return;
        };
        if self.is_claimed(range)
            || contains_interior_comment(expression)
            || !self.is_vec2(expression)
        {
            return;
        }
        if let Some(components) = self.vector(expression) {
            self.claimed.push(range);
            self.expressions.insert(range, components);
        }
    }
}

impl Planner<'_> {
    fn is_claimed(&self, (start, end): ByteRange) -> bool {
        self.claimed
            .iter()
            .any(|&(claimed_start, claimed_end)| claimed_start <= start && end <= claimed_end)
    }

    /// Plans a statement whose entire right-hand side is a `Vec2` expression
    /// with field or index operands.
    fn plan_statement(&mut self, node: &impl Node, root: &Expression, form: StatementForm) {
        let Some(range) = byte_range(node) else {
            return;
        };
        if self.is_claimed(range)
            || !is_vector_operator(unparenthesized(root))
            || contains_interior_comment(node)
            || !self.is_vec2(root)
        {
            return;
        }
        if let Some((components, reads)) = self.with_hoisting(|| self.vector(root)) {
            self.claim_statement(node, range, form, &components, reads);
        }
    }

    fn claim_statement(
        &mut self,
        node: &impl Node,
        range: ByteRange,
        form: StatementForm,
        components: &Components,
        reads: Vec<HoistedRead>,
    ) {
        // With only bare names, expression-level fusion already applies.
        if reads.is_empty() {
            return;
        }
        let Some((leading, trailing)) = surrounding_trivia_text(node) else {
            return;
        };

        let bindings = reads
            .iter()
            .map(|read| format!("local {} = {}; ", read.temp, read.read))
            .collect::<String>();
        let value = format!(
            "{}({}, {})",
            vector::CONSTRUCTOR_NAME,
            components.x.text,
            components.y.text
        );
        let statement = match form {
            StatementForm::Assign(target) => format!("do {bindings}{target} = {value} end"),
            StatementForm::Local(name) => {
                format!("local {name}; do {bindings}{name} = {value} end")
            }
            StatementForm::Return => format!("do {bindings}return {value} end"),
        };

        self.claimed.push(range);
        self.statements
            .insert(range, format!("{leading}{statement}{trailing}"));
    }

    /// Runs a plan that may bind field and index reads to locals, returning
    /// the reads in the order the plan made them, which is evaluation order.
    fn with_hoisting<T>(&self, plan: impl FnOnce() -> Option<T>) -> Option<(T, Vec<HoistedRead>)> {
        let previous = self.hoisting.replace(Some(Vec::new()));
        let result = plan();
        let reads = self.hoisting.replace(previous).unwrap_or_default();

        Some((result?, reads))
    }

    fn without_hoisting<T>(&self, render: impl FnOnce() -> T) -> T {
        let previous = self.hoisting.replace(None);
        let result = render();
        self.hoisting.replace(previous);
        result
    }

    /// Binds a field or index read to a new local and returns its name, or
    /// `None` outside statement planning.
    fn hoist(&self, var: &Var) -> Option<String> {
        if self.hoisting.borrow().is_none() {
            return None;
        }
        // A read inside a key would be a separate evaluation within this one,
        // so keys are limited to bare names, literals, and number arithmetic.
        let read = self.without_hoisting(|| self.chain_text(var, usize::MAX))?;
        let mut hoisting = self.hoisting.borrow_mut();
        let reads = hoisting.as_mut()?;
        let temp = self.temp_name(&read, reads);
        reads.push(HoistedRead {
            temp: temp.clone(),
            read,
        });

        Some(temp)
    }

    /// An assignment target whose evaluation reads no table: a name, or a name
    /// with a single field or key.
    fn target_text(&self, var: &Var) -> Option<String> {
        self.without_hoisting(|| self.chain_text(var, 1))
    }

    /// Renders a name followed by at most `max_suffixes` field or key accesses.
    fn chain_text(&self, var: &Var, max_suffixes: usize) -> Option<String> {
        let var_expression = match var {
            Var::Name(token) => return Some(token_text(token)),
            Var::Expression(var_expression) => var_expression,
            _ => return None,
        };
        let Prefix::Name(root) = var_expression.prefix() else {
            return None;
        };
        let mut text = token_text(root);
        let mut suffixes = 0;

        for suffix in var_expression.suffixes() {
            suffixes += 1;
            if suffixes > max_suffixes {
                return None;
            }
            match suffix {
                Suffix::Index(Index::Dot { name, .. }) => {
                    text.push('.');
                    text.push_str(&token_text(name));
                }
                Suffix::Index(Index::Brackets { expression, .. }) => {
                    text.push('[');
                    text.push_str(&self.pure(expression)?.text);
                    text.push(']');
                }
                _ => return None,
            }
        }

        Some(text)
    }

    /// A readable local name derived from the read, such as `__tua_self_pos`,
    /// that no identifier in the file or earlier read in the statement uses.
    fn temp_name(&self, read: &str, reads: &[HoistedRead]) -> String {
        let mut base = String::from("__tua_");
        let mut after_separator = true;
        for character in read.chars() {
            if base.len() >= 40 {
                break;
            }
            if character.is_ascii_alphanumeric() {
                base.push(character);
                after_separator = false;
            } else if !after_separator {
                base.push('_');
                after_separator = true;
            }
        }
        let base = base.trim_end_matches('_').to_owned();
        let taken = |name: &str| {
            self.identifiers.contains(name) || reads.iter().any(|read| read.temp == name)
        };

        if !taken(&base) {
            return base;
        }
        (2..)
            .map(|suffix| format!("{base}_{suffix}"))
            .find(|name| !taken(name))
            .expect("some numbered name is unused")
    }

    fn resolved_type(&self, expression: &Expression) -> Option<Type> {
        let (start, _) = expression.range()?;
        let ty = self.semantic.type_of_expression_return(
            expression,
            SourcePosition::from_full_moon(start),
            0,
        )?;

        Some(self.environment.resolve_aliases(&ty))
    }

    fn is_vec2(&self, expression: &Expression) -> bool {
        self.resolved_type(expression)
            .is_some_and(|ty| ty == self.vec2)
    }

    fn is_number(&self, expression: &Expression) -> bool {
        self.resolved_type(expression)
            .is_some_and(|ty| ty == self.number)
    }

    /// The x and y components of a `Vec2` expression, or `None` when any part
    /// of it can't be fused safely.
    fn vector(&self, expression: &Expression) -> Option<Components> {
        match expression {
            Expression::Parentheses { expression, .. }
            | Expression::TypeAssertion { expression, .. } => self.vector(expression),
            Expression::BinaryOperator { lhs, binop, rhs } => {
                match Arithmetic::from_binop(binop)? {
                    operator @ (Arithmetic::Add | Arithmetic::Sub) => {
                        if !(self.is_vec2(lhs) && self.is_vec2(rhs)) {
                            return None;
                        }
                        Some(self.vector(lhs)?.zip(self.vector(rhs)?, operator))
                    }
                    Arithmetic::Mul => self.multiply(lhs, rhs),
                    Arithmetic::Div => {
                        if !self.is_vec2(lhs) {
                            return None;
                        }
                        let divisor = nonzero_literal(rhs)?;
                        Some(
                            self.vector(lhs)?.map(|component| {
                                binary(Arithmetic::Div, component, divisor.clone())
                            }),
                        )
                    }
                    Arithmetic::Mod | Arithmetic::Pow => None,
                }
            }
            Expression::UnaryOperator {
                unop: UnOp::Minus(_),
                expression: operand,
            } => {
                if !self.is_vec2(operand) {
                    return None;
                }
                Some(self.vector(operand)?.map(negate))
            }
            Expression::Var(var) => {
                if !self.is_vec2(expression) {
                    return None;
                }
                Some(Components::of_read(&self.pure_var(var)?))
            }
            Expression::FunctionCall(call) => {
                if !self.is_vec2(expression) {
                    return None;
                }
                let [x, y] = constructor_arguments(call)?;
                if !(self.is_number(x) && self.is_number(y)) {
                    return None;
                }
                Some(Components {
                    x: self.pure(x)?,
                    y: self.pure(y)?,
                })
            }
            _ => None,
        }
    }

    fn multiply(&self, lhs: &Expression, rhs: &Expression) -> Option<Components> {
        match (self.is_vec2(lhs), self.is_vec2(rhs)) {
            // BrineVector's `*` on two vectors is the component-wise product.
            (true, true) => Some(self.vector(lhs)?.zip(self.vector(rhs)?, Arithmetic::Mul)),
            // Plan the left operand first: hoisted reads are recorded in the
            // order they are planned, which must be Lua's evaluation order.
            (true, false) if self.is_number(rhs) => {
                let vector = self.vector(lhs)?;
                let scale = self.scalar(rhs)?;
                Some(vector.map(|component| binary(Arithmetic::Mul, component, scale.clone())))
            }
            // BrineVector multiplies the vector component by the number, so
            // keep that operand order.
            (false, true) if self.is_number(lhs) => {
                let scale = self.scalar(lhs)?;
                Some(
                    self.vector(rhs)?
                        .map(|component| binary(Arithmetic::Mul, component, scale.clone())),
                )
            }
            _ => None,
        }
    }

    fn compound(&self, assignment: &ast::CompoundAssignment) -> Option<Components> {
        let target = Expression::Var(assignment.lhs().clone());
        if !self.is_vec2(&target) {
            return None;
        }
        let current = Components::of_read(&self.pure_var(assignment.lhs())?);
        let rhs = assignment.rhs();

        match assignment.compound_operator() {
            CompoundOp::PlusEqual(_) | CompoundOp::MinusEqual(_) => {
                if !self.is_vec2(rhs) {
                    return None;
                }
                let operator = if matches!(assignment.compound_operator(), CompoundOp::PlusEqual(_))
                {
                    Arithmetic::Add
                } else {
                    Arithmetic::Sub
                };
                Some(current.zip(self.vector(rhs)?, operator))
            }
            CompoundOp::StarEqual(_) => {
                if self.is_vec2(rhs) {
                    Some(current.zip(self.vector(rhs)?, Arithmetic::Mul))
                } else if self.is_number(rhs) {
                    let scale = self.scalar(rhs)?;
                    Some(current.map(|component| binary(Arithmetic::Mul, component, scale.clone())))
                } else {
                    None
                }
            }
            CompoundOp::SlashEqual(_) => {
                let divisor = nonzero_literal(rhs)?;
                Some(current.map(|component| binary(Arithmetic::Div, component, divisor.clone())))
            }
            _ => None,
        }
    }

    fn scalar(&self, expression: &Expression) -> Option<Fragment> {
        if !self.is_number(expression) {
            return None;
        }
        self.pure(expression)
    }

    /// Renders an expression that can be evaluated more than once without
    /// side effects or allocation. Arithmetic is allowed only on numbers, so
    /// no vector operation hides inside a repeated operand.
    fn pure(&self, expression: &Expression) -> Option<Fragment> {
        match expression {
            Expression::Number(token) | Expression::String(token) => {
                Some(Fragment::atom(token_text(token)))
            }
            Expression::Symbol(token) => {
                let text = token_text(token);
                matches!(text.as_str(), "true" | "false" | "nil").then(|| Fragment::atom(text))
            }
            Expression::Var(var) => self.pure_var(var).map(Fragment::atom),
            Expression::Parentheses { expression, .. } => {
                Some(self.pure(expression)?.parenthesized())
            }
            Expression::TypeAssertion { expression, .. } => self.pure(expression),
            Expression::BinaryOperator { lhs, binop, rhs } => {
                let operator = Arithmetic::from_binop(binop)?;
                if !self.is_number(expression) {
                    return None;
                }
                Some(binary(operator, self.pure(lhs)?, self.pure(rhs)?))
            }
            Expression::UnaryOperator {
                unop: UnOp::Minus(_),
                expression: operand,
            } => {
                if !self.is_number(expression) {
                    return None;
                }
                Some(negate(self.pure(operand)?))
            }
            _ => None,
        }
    }

    /// Renders a value that is safe to evaluate once for each output
    /// component. A bare name qualifies. Lua field and index reads may invoke
    /// `__index`, mutate state, or return a different value each time, so
    /// they are never duplicated: during statement planning each one is bound
    /// to a local and read once; otherwise it blocks fusion.
    fn pure_var(&self, var: &Var) -> Option<String> {
        match var {
            Var::Name(token) => Some(token_text(token)),
            Var::Expression(_) => self.hoist(var),
            _ => None,
        }
    }
}

fn single<T>(items: &Punctuated<T>) -> Option<&T> {
    let mut items = items.iter();
    let first = items.next()?;
    items.next().is_none().then_some(first)
}

fn unparenthesized(expression: &Expression) -> &Expression {
    match expression {
        Expression::Parentheses { expression, .. }
        | Expression::TypeAssertion { expression, .. } => unparenthesized(expression),
        _ => expression,
    }
}

/// Whether a comment sits inside the node. Comments before its first token or
/// after its last one move to the fused replacement; any others would be lost
/// when the components are rebuilt from text.
fn contains_interior_comment(node: &impl Node) -> bool {
    let Some((first, last)) = outer_tokens(node) else {
        return false;
    };
    let is_comment = |token: &Token| {
        matches!(
            token.token_type(),
            TokenType::SingleLineComment { .. } | TokenType::MultiLineComment { .. }
        )
    };

    node.tokens().any(|reference| {
        (!std::ptr::eq(reference, first) && reference.leading_trivia().any(is_comment))
            || (!std::ptr::eq(reference, last) && reference.trailing_trivia().any(is_comment))
    })
}

fn is_vector_operator(expression: &Expression) -> bool {
    match expression {
        Expression::BinaryOperator { binop, .. } => matches!(
            binop,
            BinOp::Plus(_) | BinOp::Minus(_) | BinOp::Star(_) | BinOp::Slash(_)
        ),
        Expression::UnaryOperator { unop, .. } => matches!(unop, UnOp::Minus(_)),
        _ => false,
    }
}

/// The two arguments of a `vec2(x, y)` call.
fn constructor_arguments(call: &ast::FunctionCall) -> Option<[&Expression; 2]> {
    let Prefix::Name(name) = call.prefix() else {
        return None;
    };
    if token_text(name) != vector::CONSTRUCTOR_NAME {
        return None;
    }

    let mut suffixes = call.suffixes();
    let Some(Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. }))) =
        suffixes.next()
    else {
        return None;
    };
    if suffixes.next().is_some() {
        return None;
    }

    let mut arguments = arguments.iter();
    match (arguments.next(), arguments.next(), arguments.next()) {
        (Some(x), Some(y), None) => Some([x, y]),
        _ => None,
    }
}

fn nonzero_literal(expression: &Expression) -> Option<Fragment> {
    let Expression::Number(token) = expression else {
        return None;
    };
    let text = token_text(token);
    let value = text.parse::<f64>().ok()?;

    (value != 0.0 && value.is_finite()).then(|| Fragment::atom(text))
}

fn token_text(token: &TokenReference) -> String {
    token.token().to_string()
}

fn byte_range(node: &impl Node) -> Option<ByteRange> {
    let (start, end) = node.range()?;
    (start.bytes() < end.bytes()).then(|| (start.bytes(), end.bytes()))
}

struct Rewriter {
    expressions: BTreeMap<ByteRange, Components>,
    compounds: BTreeMap<ByteRange, Components>,
    statements: BTreeMap<ByteRange, String>,
}

type BlockStatement = (ast::Stmt, Option<TokenReference>);

impl VisitorMut for Rewriter {
    // Statements with hoisted reads are replaced at the block level: a local
    // declaration becomes two statements, and a final `return` becomes a
    // `do ... end` block that ends in `return`.
    fn visit_block(&mut self, block: ast::Block) -> ast::Block {
        if self.statements.is_empty() {
            return block;
        }

        let mut changed = false;
        let mut statements = Vec::new();
        for (statement, semicolon) in block.stmts_with_semicolon().cloned() {
            match self.replacement(&statement) {
                Some(replacement) => {
                    append_statements(&mut statements, replacement, semicolon);
                    changed = true;
                }
                None => statements.push((statement, semicolon)),
            }
        }

        let mut last_statement = block.last_stmt_with_semicolon().cloned();
        if let Some((LastStmt::Return(statement), semicolon)) = &last_statement {
            if let Some(replacement) = self.replacement(statement) {
                append_statements(&mut statements, replacement, semicolon.clone());
                last_statement = None;
                changed = true;
            }
        }

        if !changed {
            return block;
        }
        block.with_stmts(statements).with_last_stmt(last_statement)
    }

    fn visit_expression(&mut self, expression: Expression) -> Expression {
        let Some(components) =
            byte_range(&expression).and_then(|range| self.expressions.remove(&range))
        else {
            return expression;
        };

        fused_expression(&expression, &components).unwrap_or(expression)
    }

    fn visit_stmt(&mut self, stmt: ast::Stmt) -> ast::Stmt {
        let ast::Stmt::CompoundAssignment(assignment) = &stmt else {
            return stmt;
        };
        let Some(components) =
            byte_range(assignment).and_then(|range| self.compounds.remove(&range))
        else {
            return stmt;
        };

        fused_assignment(assignment, &components)
            .map(ast::Stmt::Assignment)
            .unwrap_or(stmt)
    }
}

impl Rewriter {
    fn replacement(&mut self, node: &impl Node) -> Option<Vec<BlockStatement>> {
        let text = byte_range(node).and_then(|range| self.statements.remove(&range))?;
        parse_statements(&text)
    }
}

/// Appends generated statements, keeping the original statement's semicolon
/// on the last one.
fn append_statements(
    statements: &mut Vec<BlockStatement>,
    replacement: Vec<BlockStatement>,
    semicolon: Option<TokenReference>,
) {
    let count = replacement.len();
    for (index, (statement, generated)) in replacement.into_iter().enumerate() {
        let semicolon = if index + 1 == count {
            semicolon.clone().or(generated)
        } else {
            generated
        };
        statements.push((statement, semicolon));
    }
}

/// Parses generated statements. Their tokens get no source positions, so
/// emission maps them to the surrounding source line.
fn parse_statements(text: &str) -> Option<Vec<BlockStatement>> {
    let ast = full_moon::parse_fallible(text, LuaVersion::luau())
        .into_result()
        .ok()?;
    let ast = PositionStripper.visit_ast(ast);
    if ast.nodes().last_stmt().is_some() {
        return None;
    }

    Some(ast.nodes().stmts_with_semicolon().cloned().collect())
}

/// The text of the trivia before a node's first token and after its last one.
fn surrounding_trivia_text(node: &impl Node) -> Option<(String, String)> {
    let (first, last) = outer_tokens(node)?;

    Some((
        first.leading_trivia().map(ToString::to_string).collect(),
        last.trailing_trivia().map(ToString::to_string).collect(),
    ))
}

fn fused_expression(original: &Expression, components: &Components) -> Option<Expression> {
    let (x, y) = parse_components(components)?;
    let (leading, trailing) = surrounding_trivia(original)?;

    Some(Expression::FunctionCall(constructor_call(
        leading, x, y, trailing,
    )))
}

/// A node's first and last tokens. full_moon's `tokens()` follows field order
/// rather than source order (a parenthesized expression yields both brackets
/// first), so pick them by position.
fn outer_tokens(node: &impl Node) -> Option<(&TokenReference, &TokenReference)> {
    let first = node
        .tokens()
        .min_by_key(|token| token.token().start_position().bytes())?;
    let last = node
        .tokens()
        .max_by_key(|token| token.token().end_position().bytes())?;

    Some((first, last))
}

/// The trivia before a node's first token and after its last one, which the
/// fused replacement keeps.
fn surrounding_trivia(node: &impl Node) -> Option<(Vec<Token>, Vec<Token>)> {
    let (first, last) = outer_tokens(node)?;

    Some((
        first.leading_trivia().cloned().collect(),
        last.trailing_trivia().cloned().collect(),
    ))
}

fn fused_assignment(
    assignment: &ast::CompoundAssignment,
    components: &Components,
) -> Option<ast::Assignment> {
    let (x, y) = parse_components(components)?;
    let operator = assignment.compound_operator().token();
    let equal = TokenReference::new(
        operator.leading_trivia().cloned().collect(),
        Token::new(TokenType::Symbol {
            symbol: Symbol::Equal,
        }),
        operator.trailing_trivia().cloned().collect(),
    );
    let (leading, trailing) = surrounding_trivia(assignment.rhs())?;

    let mut variables = Punctuated::new();
    variables.push(Pair::new(assignment.lhs().clone(), None));
    let mut expressions = Punctuated::new();
    expressions.push(Pair::new(
        Expression::FunctionCall(constructor_call(leading, x, y, trailing)),
        None,
    ));

    Some(ast::Assignment::new(variables, expressions).with_equal_token(equal))
}

/// Builds `vec2(x, y)`. The later builtin-call lowering rewrites it to the
/// emitted BrineVector constructor and adds its `require`.
fn constructor_call(
    leading: Vec<Token>,
    x: Expression,
    y: Expression,
    trailing: Vec<Token>,
) -> ast::FunctionCall {
    let symbol = |symbol| Token::new(TokenType::Symbol { symbol });
    let name = TokenReference::new(
        leading,
        Token::new(TokenType::Identifier {
            identifier: vector::CONSTRUCTOR_NAME.into(),
        }),
        Vec::new(),
    );
    let open = TokenReference::new(Vec::new(), symbol(Symbol::LeftParen), Vec::new());
    let close = TokenReference::new(Vec::new(), symbol(Symbol::RightParen), trailing);
    let comma = TokenReference::new(
        Vec::new(),
        symbol(Symbol::Comma),
        vec![Token::new(TokenType::Whitespace {
            characters: " ".into(),
        })],
    );

    let mut arguments = Punctuated::new();
    arguments.push(Pair::new(x, Some(comma)));
    arguments.push(Pair::new(y, None));

    ast::FunctionCall::new(Prefix::Name(name)).with_suffixes(vec![Suffix::Call(
        Call::AnonymousCall(FunctionArgs::Parentheses {
            parentheses: ContainedSpan::new(open, close),
            arguments,
        }),
    )])
}

/// Parses the component expressions. Their tokens get no source positions, so
/// emission maps them to the surrounding source line instead of to unrelated
/// bytes of the original file.
fn parse_components(components: &Components) -> Option<(Expression, Expression)> {
    let source = format!("return {}, {}", components.x.text, components.y.text);
    let ast = full_moon::parse_fallible(&source, LuaVersion::luau())
        .into_result()
        .ok()?;
    let ast = PositionStripper.visit_ast(ast);
    let Some(LastStmt::Return(statement)) = ast.nodes().last_stmt() else {
        return None;
    };

    let mut returns = statement.returns().iter().cloned();
    match (returns.next(), returns.next(), returns.next()) {
        (Some(x), Some(y), None) => Some((x, y)),
        _ => None,
    }
}

struct PositionStripper;

impl VisitorMut for PositionStripper {
    fn visit_token_reference(&mut self, token: TokenReference) -> TokenReference {
        let strip = |token: &Token| Token::new(token.token_type().clone());

        TokenReference::new(
            token.leading_trivia().map(strip).collect(),
            strip(token.token()),
            token.trailing_trivia().map(strip).collect(),
        )
    }
}
