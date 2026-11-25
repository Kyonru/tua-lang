//! Bounded, explicit JSON facts for static condition previews. Never executes Lua.
use super::{Condition, ConditionKind, DialogueLiteral};
use crate::SourceSpan;
use serde_json::Value;

/// Input is an object with optional `context`, `locals` and scalar `args` record roots. Missing
/// fields and JSON null represent nil. Arrays and executable values are excluded.
/// This does not validate facts against an imported game's context type.
#[derive(Clone, Debug)]
pub struct DialogueMockContext {
    facts: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueMockIssue {
    pub message: &'static str,
    pub span: SourceSpan,
}

#[derive(Clone, Copy)]
enum Operand<'a> {
    Nil,
    Boolean(bool),
    Number(f64),
    String(&'a str),
    Record,
}

impl DialogueMockContext {
    /// Parse at most 64 KiB, 4,096 JSON values and 32 nesting levels.
    /// Numbers (including fractions) must be finite and within ±2^53.
    /// Facts stay in memory; imports, getters, callbacks and assets are not read.
    pub fn parse(source: &str) -> Result<Self, String> {
        if source.len() > 65_536 {
            return Err("mock context exceeds 64 KiB".into());
        }
        let facts: Value = serde_json::from_str(source).map_err(|error| error.to_string())?;
        let roots = facts.as_object().ok_or("mock context needs an object")?;
        for (name, value) in roots {
            if !matches!(name.as_str(), "context" | "locals" | "args") || !value.is_object() {
                return Err(
                    "mock context accepts only context, locals and args record roots".into(),
                );
            }
        }
        if roots
            .get("args")
            .and_then(Value::as_object)
            .is_some_and(|args| {
                args.values().any(|value| {
                    !matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_))
                })
            })
        {
            return Err("mock args must contain finite scalar values".into());
        }
        let mut pending = vec![(&facts, 0)];
        let mut count = 0;
        while let Some((value, depth)) = pending.pop() {
            count += 1;
            if count > 4096 || depth > 32 {
                return Err("mock context exceeds 4,096 values or 32 nesting levels".into());
            }
            match value {
                Value::Array(_) => return Err("mock context does not accept arrays".into()),
                Value::Object(fields) => {
                    pending.extend(fields.values().map(|value| (value, depth + 1)));
                }
                Value::Number(number) if preview_number(&number.to_string()).is_none() => {
                    return Err(
                        "mock context numbers must be finite and within the safe integer range"
                            .into(),
                    );
                }
                _ => {}
            }
        }
        Ok(Self { facts })
    }

    pub(super) fn lookup(&self, path: &str) -> Result<&Value, ()> {
        let mut value = &self.facts;
        for key in path.split('.') {
            if value.is_null() {
                return Ok(&Value::Null);
            }
            value = value
                .as_object()
                .ok_or(())?
                .get(key)
                .unwrap_or(&Value::Null);
        }
        Ok(value)
    }

    /// Evaluate a parsed condition with the runtime's boolean/nil and scalar
    /// comparison rules. Errors identify the offending expression's source span.
    /// This is one condition evaluation, not conversation execution or reachability.
    pub fn test(&self, condition: &Condition) -> Result<bool, DialogueMockIssue> {
        let mut remaining = 256;
        let result = self.evaluate(condition, 1, &mut remaining)?;
        boolean(result).map_err(|message| issue(condition, message))
    }

    fn evaluate<'a>(
        &'a self,
        condition: &'a Condition,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Operand<'a>, DialogueMockIssue> {
        let fail = |message| issue(condition, message);
        if depth > 32 || *remaining == 0 {
            return Err(fail("mock condition evaluation budget exceeded"));
        }
        *remaining -= 1;
        match &condition.kind {
            ConditionKind::Nil => Ok(Operand::Nil),
            ConditionKind::Value(DialogueLiteral::Boolean(value)) => Ok(Operand::Boolean(*value)),
            ConditionKind::Value(DialogueLiteral::String(value)) => Ok(Operand::String(value)),
            ConditionKind::Value(DialogueLiteral::Number(value)) => preview_number(value)
                .map(Operand::Number)
                .ok_or_else(|| fail("mock condition number exceeds the safe numeric range")),
            ConditionKind::Path(path) => {
                let value = self
                    .lookup(path)
                    .map_err(|_| fail("condition path needs a plain record"))?;
                Ok(match value {
                    Value::Null => Operand::Nil,
                    Value::Bool(value) => Operand::Boolean(*value),
                    Value::Number(value) => {
                        Operand::Number(value.as_f64().expect("validated number"))
                    }
                    Value::String(value) => Operand::String(value),
                    _ => Operand::Record,
                })
            }
            ConditionKind::Not(inner) => {
                let value = self.evaluate(inner, depth + 1, remaining)?;
                Ok(Operand::Boolean(!boolean(value).map_err(fail)?))
            }
            ConditionKind::Binary(op, left, right) => {
                let left = self.evaluate(left, depth + 1, remaining)?;
                if op == "and" || op == "or" {
                    let left = boolean(left).map_err(fail)?;
                    if (op == "and" && !left) || (op == "or" && left) {
                        return Ok(Operand::Boolean(left));
                    }
                    let right = self.evaluate(right, depth + 1, remaining)?;
                    return boolean(right).map(Operand::Boolean).map_err(fail);
                }
                let right = self.evaluate(right, depth + 1, remaining)?;
                let result = compare(op, left, right).map_err(fail)?;
                Ok(Operand::Boolean(result))
            }
        }
    }
}

fn issue(condition: &Condition, message: &'static str) -> DialogueMockIssue {
    DialogueMockIssue {
        message,
        span: condition.span,
    }
}

fn boolean(value: Operand<'_>) -> Result<bool, &'static str> {
    match value {
        Operand::Nil => Ok(false),
        Operand::Boolean(value) => Ok(value),
        _ => Err("condition needs boolean or nil"),
    }
}

fn compare(op: &str, left: Operand<'_>, right: Operand<'_>) -> Result<bool, &'static str> {
    use Operand::*;
    if matches!(op, "==" | "!=" | "~=") {
        let equal = match (left, right) {
            (Nil, Nil) => true,
            (Nil, _) | (_, Nil) => false,
            (Boolean(left), Boolean(right)) => left == right,
            (Number(left), Number(right)) => left == right,
            (String(left), String(right)) => left == right,
            _ => return Err("equality needs matching scalars or nil"),
        };
        return Ok(if op == "==" { equal } else { !equal });
    }
    if !matches!(op, "<" | "<=" | ">" | ">=") {
        return Err("unknown condition operation");
    }
    if matches!(left, Nil) || matches!(right, Nil) {
        return Ok(false);
    }
    let (Number(left), Number(right)) = (left, right) else {
        return Err("ordered comparison needs numbers");
    };
    Ok(match op {
        "<" => left < right,
        "<=" => left <= right,
        ">" => left > right,
        _ => left >= right,
    })
}

// Keep preview answers portable across LuaJIT/5.1 doubles and Lua 5.4 integers.
pub(super) fn preview_number(source: &str) -> Option<f64> {
    const MAX: u128 = 9_007_199_254_740_992;
    if source
        .parse::<i128>()
        .is_ok_and(|value| value.unsigned_abs() > MAX)
    {
        return None;
    }
    source
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && value.abs() <= MAX as f64)
}
