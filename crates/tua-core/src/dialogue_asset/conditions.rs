//! Bounded conditions lower to data instructions, never executable Lua expressions.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Condition {
    pub kind: ConditionKind,
    pub span: SourceSpan,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConditionKind {
    Value(DialogueLiteral),
    Nil,
    Path(String),
    Not(Box<Condition>),
    Binary(String, Box<Condition>, Box<Condition>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch {
    pub condition: Condition,
    pub yes: DialogueName,
    pub no: DialogueName,
}

#[derive(Clone)]
struct Token {
    value: String,
    span: SourceSpan,
}
struct Expressions {
    tokens: Vec<Token>,
    cursor: usize,
}
impl Expressions {
    fn parse(text: &str, at: SourceSpan) -> Result<Condition, String> {
        if text.len() > 8192 {
            return Err("condition exceeds 8192 bytes".into());
        }
        let chars: Vec<char> = text.chars().collect();
        let mut tokens = Vec::new();
        let mut cursor = 0;
        while cursor < chars.len() {
            if chars[cursor].is_whitespace() {
                cursor += 1;
                continue;
            }
            let start = cursor;
            let ch = chars[cursor];
            cursor += 1;
            if ch == '"' {
                let mut closed = false;
                while cursor < chars.len() {
                    let ch = chars[cursor];
                    cursor += 1;
                    if ch == '\\' {
                        cursor += 1;
                    } else if ch == '"' {
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    return Err("unterminated condition string".into());
                }
            } else if ch.is_ascii_alphabetic() || ch == '_' {
                while cursor < chars.len()
                    && (chars[cursor].is_ascii_alphanumeric() || matches!(chars[cursor], '_' | '.'))
                {
                    cursor += 1;
                }
            } else if ch.is_ascii_digit() || ch == '-' {
                while cursor < chars.len()
                    && (chars[cursor].is_ascii_digit()
                        || matches!(chars[cursor], '.' | 'e' | 'E' | '+' | '-'))
                {
                    cursor += 1;
                }
            } else if matches!(ch, '=' | '~' | '!' | '<' | '>') {
                if chars.get(cursor) == Some(&'=') {
                    cursor += 1;
                }
            } else if !matches!(ch, '(' | ')') {
                return Err(format!("unsupported condition character `{ch}`"));
            }
            tokens.push(Token {
                value: chars[start..cursor].iter().collect(),
                span: span(at.start.line, at.start.column + start, cursor - start),
            });
            if tokens.len() > 256 {
                return Err("condition exceeds 256 tokens".into());
            }
        }
        let mut parser = Self { tokens, cursor: 0 };
        let result = parser.expression(0, 1)?;
        if parser.cursor != parser.tokens.len() {
            return Err(
                "unexpected condition token (calls and arithmetic are not supported)".into(),
            );
        }
        fn depth(value: &Condition) -> usize {
            match &value.kind {
                ConditionKind::Not(a) => 1 + depth(a),
                ConditionKind::Binary(_, a, b) => 1 + depth(a).max(depth(b)),
                _ => 1,
            }
        }
        if depth(&result) > 32 {
            return Err("condition nesting exceeds 32".into());
        }
        Ok(result)
    }
    fn expression(&mut self, minimum: u8, depth: usize) -> Result<Condition, String> {
        if depth > 32 {
            return Err("condition nesting exceeds 32".into());
        }
        let token = self
            .tokens
            .get(self.cursor)
            .cloned()
            .ok_or("expected condition expression")?;
        self.cursor += 1;
        let kind = match token.value.as_str() {
            "(" => {
                let result = self.expression(0, depth + 1)?;
                if self.tokens.get(self.cursor).is_none_or(|t| t.value != ")") {
                    return Err("expected closing condition parenthesis".into());
                }
                self.cursor += 1;
                result.kind
            }
            "not" => ConditionKind::Not(Box::new(self.expression(4, depth + 1)?)),
            "nil" => ConditionKind::Nil,
            "true" | "false" => {
                ConditionKind::Value(DialogueLiteral::Boolean(token.value == "true"))
            }
            value if value.starts_with('"') => ConditionKind::Value(DialogueLiteral::String(
                serde_json::from_str(value).map_err(|_| "invalid condition string escape")?,
            )),
            value if value.parse::<f64>().is_ok_and(|v| v.is_finite()) => {
                ConditionKind::Value(DialogueLiteral::Number(value.into()))
            }
            value if value.starts_with("context.") || value.starts_with("locals.") => {
                if value.split('.').count() > 32
                    || !value.split('.').all(|v| identifier(v) && !v.contains('-'))
                {
                    return Err("invalid condition field path".into());
                }
                ConditionKind::Path(value.into())
            }
            _ => {
                return Err(
                    "expected a scalar, nil, field path, not, or parenthesized condition".into(),
                )
            }
        };
        let mut left = Condition {
            kind,
            span: token.span,
        };
        while let Some(operator) = self.tokens.get(self.cursor).cloned() {
            let precedence = match operator.value.as_str() {
                "or" => 1,
                "and" => 2,
                "==" | "~=" | "!=" | "<" | "<=" | ">" | ">=" => 3,
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            self.cursor += 1;
            let right = self.expression(precedence + 1, depth + 1)?;
            left = Condition {
                kind: ConditionKind::Binary(operator.value, Box::new(left), Box::new(right)),
                span: operator.span,
            };
        }
        Ok(left)
    }
}

impl Parser<'_> {
    pub(super) fn condition_line(&mut self, text: &str, at: SourceSpan) -> bool {
        let Some((directive, rest)) = text.split_once(' ') else {
            return false;
        };
        if !matches!(
            directive,
            "@branch" | "@visible" | "@enabled" | "@reason" | "@fallback"
        ) {
            return false;
        }
        if self.document.context.is_none() {
            self.error(at, "conditional flow requires an @context definition");
            return true;
        }
        let rest = rest.trim();
        let rest_at = span(
            at.start.line,
            at.start.column + text.chars().count() - rest.chars().count(),
            rest.chars().count(),
        );
        if directive == "@branch" {
            self.abandon_id();
            let Some((expression, targets)) = rest.rsplit_once(" [target:") else {
                self.error(at, "expected @branch condition [target:yes] [else:no]");
                return true;
            };
            let Some((yes, no)) = targets
                .split_once("] [else:")
                .and_then(|(a, b)| Some((a, b.strip_suffix(']')?)))
            else {
                self.error(at, "branch needs an explicit else target");
                return true;
            };
            let yes_at = span(
                at.start.line,
                rest_at.start.column + expression.chars().count() + 9,
                yes.chars().count(),
            );
            let no_at = span(
                at.start.line,
                yes_at.start.column + yes.chars().count() + 8,
                no.chars().count(),
            );
            let (Some(yes), Some(no)) = (self.name(yes, yes_at), self.name(no, no_at)) else {
                return true;
            };
            let condition = match Expressions::parse(expression, rest_at) {
                Ok(v) => v,
                Err(e) => {
                    self.error(rest_at, e);
                    return true;
                }
            };
            let Some(section) = self.document.sections.last_mut() else {
                self.error(at, "branch requires a label");
                return true;
            };
            if !section.lines.is_empty() || section.branch.is_some() || section.action.is_some() {
                self.error(at, "a branch must be the only content of its label");
                return true;
            }
            section.branch = Some(Branch { condition, yes, no });
            return true;
        }
        if directive == "@reason" {
            let Some(reason) = self.text(rest, rest_at) else {
                return true;
            };
            let choice = self
                .document
                .sections
                .last_mut()
                .and_then(|s| s.lines.last_mut())
                .and_then(|l| l.choices.last_mut());
            if let Some(choice) = choice {
                if choice.reason.is_some() {
                    self.error(at, "duplicate disabled reason");
                } else {
                    choice.reason = Some(reason);
                }
            } else {
                self.error(at, "reason must follow a choice");
            }
            return true;
        }
        self.abandon_id();
        if directive == "@fallback" {
            let Some(target) = self.name(rest, rest_at) else {
                return true;
            };
            let line = self
                .document
                .sections
                .last_mut()
                .and_then(|s| s.lines.last_mut());
            if let Some(line) = line.filter(|l| !l.choices.is_empty() && l.fallback.is_none()) {
                line.fallback = Some(target);
            } else {
                self.error(at, "declare one fallback after choices");
            }
            return true;
        }
        let condition = match Expressions::parse(rest, rest_at) {
            Ok(v) => v,
            Err(e) => {
                self.error(rest_at, e);
                return true;
            }
        };
        let choice = self
            .document
            .sections
            .last_mut()
            .and_then(|s| s.lines.last_mut())
            .and_then(|l| l.choices.last_mut());
        if let Some(choice) = choice {
            let slot = if directive == "@visible" {
                &mut choice.visible
            } else {
                &mut choice.enabled
            };
            if slot.is_some() {
                self.error(at, "duplicate choice condition");
            } else {
                *slot = Some(condition);
            }
        } else {
            self.error(at, "choice condition must follow a choice");
        }
        true
    }
}
