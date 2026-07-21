use super::{FunctionParameter, Type};

const DEFAULT_TYPE_WIDTH: usize = 80;
const INDENT_WIDTH: usize = 2;
const MAX_DISPLAY_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenderMode {
    Flat,
    Break,
}

#[derive(Clone, Debug)]
enum TypeDoc {
    Text(String),
    Line(&'static str),
    Concat(Vec<TypeDoc>),
    Nest(usize, Box<TypeDoc>),
    Group(Box<TypeDoc>),
}

pub(crate) fn format_type_pretty(ty: &Type, initial_column: usize) -> String {
    render(&type_doc(ty, 0), DEFAULT_TYPE_WIDTH, initial_column)
}

pub(crate) fn format_type_with_prefix(prefix: impl Into<String>, ty: &Type) -> String {
    let mut prefix = prefix.into();
    let initial_column = current_line_width(&prefix);

    prefix.push_str(&format_type_pretty(ty, initial_column));
    prefix
}

pub(crate) fn format_named_function_pretty(
    name: &str,
    parameters: &[FunctionParameter],
    has_varargs: bool,
    vararg_type: Option<&Type>,
    return_types: &[Type],
) -> String {
    let mut parameter_docs = parameters
        .iter()
        .map(|parameter| function_parameter_doc(parameter, 1))
        .collect::<Vec<_>>();

    if has_varargs {
        let mut vararg = vec![text("...")];
        if let Some(ty) = vararg_type {
            vararg.push(text(": "));
            vararg.push(type_doc(ty, 1));
        }
        parameter_docs.push(concat(vararg));
    }

    let returns = match return_types {
        [] => text("any"),
        [ty] => type_doc(ty, 1),
        types => group(comma_separated(
            types.iter().map(|ty| type_doc(ty, 1)).collect(),
        )),
    };
    let signature = group(concat([
        text(format!("function {name}")),
        delimited("(", ")", parameter_docs, false),
        text(": "),
        returns,
    ]));

    render(&signature, DEFAULT_TYPE_WIDTH, 0)
}

fn type_doc(ty: &Type, depth: usize) -> TypeDoc {
    if depth >= MAX_DISPLAY_DEPTH {
        return text("…");
    }

    match ty {
        Type::Primitive(primitive) => text(primitive.to_string()),
        Type::Any => text("any"),
        Type::Alias(name) => text(name),
        Type::StringLiteral(value) => text(format!("{value:?}")),
        Type::Union(types) => union_doc(types, depth + 1),
        Type::Function(function) => {
            let mut parameters = function
                .parameters
                .iter()
                .map(|parameter| function_parameter_doc(parameter, depth + 1))
                .collect::<Vec<_>>();

            if let Some(vararg_type) = function.vararg_type.as_deref() {
                parameters.push(concat([text("..."), type_doc(vararg_type, depth + 1)]));
            }

            concat([
                delimited("(", ")", parameters, false),
                text(" -> "),
                return_types_doc(function.return_types_or_any(), depth + 1),
            ])
        }
        Type::Table(table) => {
            let fields = table
                .fields()
                .map(|(name, field)| {
                    concat([
                        text(format!("{name}{}: ", if field.optional { "?" } else { "" })),
                        type_doc(&field.ty, depth + 1),
                    ])
                })
                .collect::<Vec<_>>();

            if fields.is_empty() {
                text("{ }")
            } else {
                delimited("{", "}", fields, true)
            }
        }
        Type::Array(item) if matches!(item.as_ref(), Type::Union(_) | Type::Function(_)) => {
            concat([
                delimited("(", ")", vec![type_doc(item, depth + 1)], false),
                text("[]"),
            ])
        }
        Type::Array(item) => concat([type_doc(item, depth + 1), text("[]")]),
        Type::Tuple(items) => delimited(
            "[",
            "]",
            items.iter().map(|item| type_doc(item, depth + 1)).collect(),
            false,
        ),
        Type::Dictionary { key, value } => delimited(
            "{",
            "}",
            vec![concat([
                text("["),
                type_doc(key, depth + 1),
                text("]: "),
                type_doc(value, depth + 1),
            ])],
            true,
        ),
    }
}

fn function_parameter_doc(parameter: &FunctionParameter, depth: usize) -> TypeDoc {
    if let Some(name) = parameter.name.as_deref() {
        concat([text(format!("{name}: ")), type_doc(&parameter.ty, depth)])
    } else {
        type_doc(&parameter.ty, depth)
    }
}

fn return_types_doc(types: &[Type], depth: usize) -> TypeDoc {
    if types.len() == 1 {
        return type_doc(&types[0], depth);
    }

    delimited(
        "(",
        ")",
        types.iter().map(|ty| type_doc(ty, depth)).collect(),
        false,
    )
}

fn union_doc(types: &[Type], depth: usize) -> TypeDoc {
    let mut types = types.iter();
    let Some(first) = types.next() else {
        return text("");
    };
    let mut parts = vec![type_doc(first, depth)];

    for ty in types {
        parts.push(nest(
            INDENT_WIDTH,
            concat([line(), text("| "), type_doc(ty, depth)]),
        ));
    }

    group(concat(parts))
}

fn delimited(
    open: &'static str,
    close: &'static str,
    items: Vec<TypeDoc>,
    padded: bool,
) -> TypeDoc {
    if items.is_empty() {
        return text(format!("{open}{close}"));
    }

    let boundary = if padded { line() } else { softline() };

    group(concat([
        text(open),
        nest(
            INDENT_WIDTH,
            concat([boundary.clone(), comma_separated(items)]),
        ),
        boundary,
        text(close),
    ]))
}

fn comma_separated(items: Vec<TypeDoc>) -> TypeDoc {
    let mut parts = Vec::new();

    for (index, item) in items.into_iter().enumerate() {
        if index > 0 {
            parts.push(text(","));
            parts.push(line());
        }
        parts.push(item);
    }

    concat(parts)
}

fn render(doc: &TypeDoc, width: usize, initial_column: usize) -> String {
    let mut output = String::new();
    let mut column = initial_column;
    let mut commands = vec![(0, RenderMode::Break, doc)];

    while let Some((indent, mode, doc)) = commands.pop() {
        match doc {
            TypeDoc::Text(value) => {
                output.push_str(value);
                column += display_width(value);
            }
            TypeDoc::Line(flat) if mode == RenderMode::Flat => {
                output.push_str(flat);
                column += display_width(flat);
            }
            TypeDoc::Line(_) => {
                output.push('\n');
                output.extend(std::iter::repeat_n(' ', indent));
                column = indent;
            }
            TypeDoc::Concat(parts) => {
                for part in parts.iter().rev() {
                    commands.push((indent, mode, part));
                }
            }
            TypeDoc::Nest(extra, nested) => {
                commands.push((indent + extra, mode, nested));
            }
            TypeDoc::Group(grouped) => {
                let group_mode = if mode == RenderMode::Flat {
                    RenderMode::Flat
                } else {
                    let mut candidate = commands.clone();
                    candidate.push((indent, RenderMode::Flat, grouped));

                    if fits(width.saturating_sub(column), candidate) {
                        RenderMode::Flat
                    } else {
                        RenderMode::Break
                    }
                };

                commands.push((indent, group_mode, grouped));
            }
        }
    }

    output
}

fn fits(remaining: usize, mut commands: Vec<(usize, RenderMode, &TypeDoc)>) -> bool {
    let mut remaining = remaining as isize;

    while remaining >= 0 {
        let Some((indent, mode, doc)) = commands.pop() else {
            return true;
        };

        match doc {
            TypeDoc::Text(value) => remaining -= display_width(value) as isize,
            TypeDoc::Line(flat) if mode == RenderMode::Flat => {
                remaining -= display_width(flat) as isize;
            }
            TypeDoc::Line(_) => return true,
            TypeDoc::Concat(parts) => {
                for part in parts.iter().rev() {
                    commands.push((indent, mode, part));
                }
            }
            TypeDoc::Nest(extra, nested) => commands.push((indent + extra, mode, nested)),
            TypeDoc::Group(grouped) => commands.push((indent, RenderMode::Flat, grouped)),
        }
    }

    false
}

fn text(value: impl Into<String>) -> TypeDoc {
    TypeDoc::Text(value.into())
}

fn line() -> TypeDoc {
    TypeDoc::Line(" ")
}

fn softline() -> TypeDoc {
    TypeDoc::Line("")
}

fn concat(parts: impl IntoIterator<Item = TypeDoc>) -> TypeDoc {
    TypeDoc::Concat(parts.into_iter().collect())
}

fn nest(indent: usize, doc: TypeDoc) -> TypeDoc {
    TypeDoc::Nest(indent, Box::new(doc))
}

fn group(doc: TypeDoc) -> TypeDoc {
    TypeDoc::Group(Box::new(doc))
}

fn current_line_width(value: &str) -> usize {
    value
        .rsplit_once('\n')
        .map_or(value, |(_, line)| line)
        .chars()
        .count()
}

fn display_width(value: &str) -> usize {
    value.chars().count()
}
