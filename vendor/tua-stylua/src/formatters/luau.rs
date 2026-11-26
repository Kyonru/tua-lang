use crate::{
    context::{
        create_function_definition_trivia, create_indent_trivia, create_newline_trivia, Context,
    },
    fmt_symbol,
    formatters::{
        assignment::hang_equal_token,
        expression::format_expression,
        functions::format_function_body,
        general::{
            format_contained_punctuated_multiline, format_contained_span, format_punctuated,
            format_symbol, format_token_reference,
        },
        table::{create_table_braces, format_multiline_table, format_singleline_table, TableType},
        trivia::{
            strip_leading_trivia, strip_trailing_trivia, strip_trivia, FormatTriviaType,
            UpdateLeadingTrivia, UpdateTrailingTrivia, UpdateTrivia,
        },
        trivia_util::{
            after_tua_comment, comments, contains_comments, contains_singleline_comments,
            space_prefixed_comments, spans_multiple_lines, token_contains_comments,
            trivia_is_comment, trivia_is_newline, CommentSearch, GetLeadingTrivia,
            GetTrailingTrivia,
        },
    },
    shape::Shape,
};
use full_moon::ast::{
    luau::{
        ExportedTypeDeclaration, ExportedTypeFunction, GenericDeclaration,
        GenericDeclarationParameter, GenericParameterInfo, IfConditionBinding, IndexedTypeInfo,
        LuauAttribute, TypeArgument, TypeAssertion, TypeDeclaration, TypeField, TypeFieldKey,
        TypeFunction, TypeInfo, TypeInstantiation, TypeIntersection, TypeSpecifier, TypeUnion,
    },
    punctuated::Pair,
};
use full_moon::ast::{punctuated::Punctuated, span::ContainedSpan};
use full_moon::tokenizer::{Token, TokenReference, TokenType};
use std::boxed::Box;

// If we have a type like
// A | B | {
//    ...
// }
// we should try and hug them together if possible
fn should_hug_type(type_info: &TypeInfo) -> bool {
    match type_info {
        TypeInfo::Union(union) => union.types().iter().any(should_hug_type),
        TypeInfo::Intersection(intersection) => intersection.types().iter().any(should_hug_type),
        TypeInfo::Table { .. } => true,
        _ => false,
    }
}

fn is_union_of_tables(type_info: &TypeInfo) -> bool {
    match type_info {
        TypeInfo::Union(union) => union
            .types()
            .iter()
            .all(|ty| matches!(ty, TypeInfo::Table { .. })),
        _ => false,
    }
}

fn format_hangable_type_info_internal(
    ctx: &Context,
    type_info: &TypeInfo,
    context: TypeInfoContext,
    shape: Shape,
    hang_level: usize,
) -> TypeInfo {
    let singleline_type_info = format_type_info(ctx, type_info, shape.with_infinite_width());

    // If we can hang the type definition, and its over width, then lets try doing so
    if can_hang_type(type_info)
        && (should_hang_type(type_info, CommentSearch::Single)
            || shape.test_over_budget(&strip_trailing_trivia(&singleline_type_info)))
    {
        hang_type_info(ctx, type_info, context, shape, hang_level)
    } else {
        // Use the proper formatting
        format_type_info_internal(ctx, type_info, context, shape)
    }
}

// Formats a type info, then determines whether it is still over width. If so, it tries to hang it.
fn format_hangable_type_info(
    ctx: &Context,
    type_info: &TypeInfo,
    shape: Shape,
    hang_level: usize,
) -> TypeInfo {
    format_hangable_type_info_internal(ctx, type_info, TypeInfoContext::new(), shape, hang_level)
}

fn format_type_info_generics(
    ctx: &Context,
    arrows: &ContainedSpan,
    generics: &Punctuated<TypeInfo>,
    shape: Shape,
) -> (ContainedSpan, Punctuated<TypeInfo>) {
    const ARROW_LEN: usize = 1; // 1 = "<"

    let context = TypeInfoContext::new().mark_within_generic();

    let singleline_arrows = format_contained_span(ctx, arrows, shape);
    let singleline_generics = format_punctuated(
        ctx,
        generics,
        shape.with_infinite_width(),
        |ctx, type_info, shape| format_type_info_internal(ctx, type_info, context, shape),
    );

    let (start_arrow, end_arrow) = arrows.tokens();
    let contains_comments = start_arrow.has_trailing_comments(CommentSearch::Single)
        || end_arrow.has_leading_comments(CommentSearch::Single)
        || generics.pairs().any(|generic_pair| {
            contains_singleline_comments(generic_pair.value())
                    || generic_pair.value().has_leading_comments(CommentSearch::All) // Look for leading multiline comments - these suggest expansion
                    || generic_pair
                        .punctuation()
                        .is_some_and(contains_singleline_comments)
        });

    let should_expand = contains_comments
        || shape
            .add_width(ARROW_LEN * 2)
            .test_over_budget(&singleline_generics);

    // If the generics is just a single type table, then we can hug it
    let can_hug_table = should_expand
        && generics.len() == 1
        && match generics.iter().next().unwrap() {
            TypeInfo::Table { braces, .. } => {
                // Check there is not leading or trailing comments on the brace
                let (start_brace, end_brace) = braces.tokens();
                !start_brace.has_leading_comments(CommentSearch::Single)
                    || !end_brace.has_trailing_comments(CommentSearch::Single)
            }
            _ => false,
        };

    if should_expand && !can_hug_table {
        format_contained_punctuated_multiline(
            ctx,
            arrows,
            generics,
            |ctx, type_info, shape| {
                format_hangable_type_info_internal(ctx, type_info, context, shape, 0)
            },
            shape,
        )
    } else {
        (singleline_arrows, singleline_generics)
    }
}

#[derive(Clone, Copy)]
struct TypeInfoContext {
    // A TypeInfo within an optional type
    // we should NOT remove parentheses in a type (A | B)?
    within_optional: bool,
    // Array suffixes must preserve grouping around composite element types.
    within_array: bool,
    // A TypeInfo within a variadic type
    // we should NOT remove parentheses in a type ...(A | B)
    within_variadic: bool,
    // A TypeInfo as a generic parameter
    // Foo<(string), (number)>
    // we should NOT remove these parentheses are they may correspond to single-type type packs
    within_generic: bool,
    /// A TypeInfo in a table indexer
    /// we should NOT remove parentheses around a compound type [("foo" | "bar")]: string
    within_table_indexer: bool,

    /// A TypeInfo part of a union/intersection operation
    /// If its a mixed composite type, then we should not remove excess parentheses. e.g.
    /// A & (B | C)
    /// A & (B?)
    /// A | (B & C)
    /// Note, we should remove parentheses in these cases:
    /// A | (B | C)
    /// A & (B & C)
    contains_union: bool,
    contains_intersect: bool,
}

impl TypeInfoContext {
    fn new() -> Self {
        Self {
            within_optional: false,
            within_array: false,
            within_variadic: false,
            within_generic: false,
            within_table_indexer: false,
            contains_union: false,
            contains_intersect: false,
        }
    }

    fn mark_within_optional(self) -> TypeInfoContext {
        Self {
            within_optional: true,
            ..self
        }
    }

    fn mark_within_variadic(self) -> TypeInfoContext {
        Self {
            within_variadic: true,
            ..self
        }
    }

    fn mark_within_generic(self) -> TypeInfoContext {
        Self {
            within_generic: true,
            ..self
        }
    }

    fn mark_within_table_indexer(self) -> TypeInfoContext {
        Self {
            within_table_indexer: true,
            ..self
        }
    }

    fn mark_contains_union(self) -> TypeInfoContext {
        Self {
            contains_union: true,
            ..self
        }
    }

    fn mark_contains_intersect(self) -> TypeInfoContext {
        Self {
            contains_intersect: true,
            ..self
        }
    }
}

fn keep_parentheses(internal_type: &TypeInfo, context: TypeInfoContext) -> bool {
    if context.within_array {
        return true;
    }
    match internal_type {
        TypeInfo::Callback { .. }
            if context.within_optional
                || context.within_variadic
                || context.contains_intersect
                || context.contains_union =>
        {
            true
        }
        TypeInfo::Union { .. } | TypeInfo::Optional { .. }
            if context.within_optional
                || context.within_variadic
                || context.within_table_indexer
                || context.contains_intersect =>
        {
            true
        }
        TypeInfo::Intersection { .. }
            if context.within_optional
                || context.within_variadic
                || context.within_table_indexer
                || context.contains_union =>
        {
            true
        }
        _ if context.within_generic => true,
        _ => false,
    }
}

pub fn format_type_info(ctx: &Context, type_info: &TypeInfo, shape: Shape) -> TypeInfo {
    format_type_info_internal(ctx, type_info, TypeInfoContext::new(), shape)
}

fn format_type_info_internal(
    ctx: &Context,
    type_info: &TypeInfo,
    context: TypeInfoContext,
    shape: Shape,
) -> TypeInfo {
    match type_info {
        TypeInfo::Array {
            braces,
            access,
            type_info,
        } => {
            const BRACKET_LEN: usize = "{ ".len();

            let (start_brace, end_brace) = braces.tokens().to_owned();

            let contains_comments = start_brace.trailing_trivia().any(trivia_is_comment)
                || end_brace.leading_trivia().any(trivia_is_comment)
                || contains_comments(access)
                || contains_comments(type_info);

            // Format modifier comments at the multiline element indentation.
            let access_shape = if contains_comments {
                shape.increment_additional_indent()
            } else {
                shape + BRACKET_LEN
            };

            let access = access
                .as_ref()
                .map(|token_reference| format_token_reference(ctx, token_reference, access_shape));

            // Count the modifier and its required separator without trivia.
            let access_shape_increment = access
                .as_ref()
                .map_or(0, |token| strip_trivia(token).to_string().len() + 1);

            let (table_type, new_type_info) = if contains_comments {
                (TableType::MultiLine, None)
            } else {
                let new_type_info = format_hangable_type_info(
                    ctx,
                    type_info,
                    shape + BRACKET_LEN + access_shape_increment,
                    0,
                );

                (
                    if spans_multiple_lines(&new_type_info) {
                        TableType::MultiLine
                    } else {
                        TableType::SingleLine
                    },
                    Some(new_type_info),
                )
            };
            let braces = create_table_braces(ctx, start_brace, end_brace, table_type, shape);

            let (new_type_info, leading_trivia, trailing_trivia) = match table_type {
                TableType::MultiLine => (
                    format_hangable_type_info(
                        ctx,
                        type_info,
                        shape.increment_additional_indent(),
                        0,
                    ),
                    FormatTriviaType::Append(vec![create_indent_trivia(
                        ctx,
                        shape.increment_additional_indent(),
                    )]),
                    FormatTriviaType::Append(vec![create_newline_trivia(ctx)]),
                ),
                _ => (
                    new_type_info.unwrap_or_else(|| format_type_info(ctx, type_info, shape)),
                    FormatTriviaType::NoChange,
                    FormatTriviaType::NoChange,
                ),
            };

            let (access, type_info_leading_trivia) = match access {
                Some(access) => {
                    // A trailing line comment must end before the element type.
                    let separator = if access.has_trailing_comments(CommentSearch::Single) {
                        vec![
                            create_newline_trivia(ctx),
                            create_indent_trivia(ctx, shape.increment_additional_indent()),
                        ]
                    } else {
                        vec![Token::new(TokenType::spaces(1))]
                    };

                    (
                        Some(
                            access
                                .update_leading_trivia(leading_trivia)
                                .update_trailing_trivia(FormatTriviaType::Append(separator)),
                        ),
                        FormatTriviaType::NoChange,
                    )
                }
                None => (None, leading_trivia),
            };

            TypeInfo::Array {
                braces,
                access,
                type_info: Box::new(
                    new_type_info.update_trivia(type_info_leading_trivia, trailing_trivia),
                ),
            }
        }

        TypeInfo::Basic(token_reference) => {
            let token_reference = format_token_reference(ctx, token_reference, shape);
            TypeInfo::Basic(token_reference)
        }

        // Special cases for singleton types
        TypeInfo::String(string) => TypeInfo::String(format_token_reference(ctx, string, shape)),
        TypeInfo::Boolean(boolean) => {
            TypeInfo::Boolean(format_token_reference(ctx, boolean, shape))
        }

        TypeInfo::Callback {
            generics,
            parentheses,
            arguments,
            arrow,
            return_type,
        } => {
            const PAREN_LEN: usize = "(".len();
            const ARROW_LEN: usize = " -> ".len();

            let (start_parens, end_parens) = parentheses.tokens();

            let generics = generics
                .as_ref()
                .map(|generics| format_generic_declaration(ctx, generics, shape));

            let shape = match generics {
                Some(ref generics) => shape.take_last_line(&generics),
                None => shape,
            };

            let force_multiline = start_parens.has_trailing_comments(CommentSearch::All)
                || end_parens.has_leading_comments(CommentSearch::All)
                || contains_comments(arguments)
                || shape
                    .add_width(
                        PAREN_LEN * 2
                            + ARROW_LEN
                            + arguments.to_string().len()
                            + strip_trailing_trivia(&**return_type).to_string().len(),
                    )
                    .over_budget();

            let (parentheses, arguments, shape) = if force_multiline {
                let (parentheses, formatted_arguments) = format_contained_punctuated_multiline(
                    ctx,
                    parentheses,
                    arguments,
                    format_type_argument,
                    shape,
                );
                let shape = shape.reset() + PAREN_LEN;

                (parentheses, formatted_arguments, shape)
            } else {
                let parentheses = format_contained_span(ctx, parentheses, shape);
                let arguments = format_punctuated(ctx, arguments, shape + 1, format_type_argument);
                let shape = shape + (PAREN_LEN * 2 + arguments.to_string().len());

                (parentheses, arguments, shape)
            };

            let arrow = fmt_symbol!(ctx, arrow, " -> ", shape);
            let shape = shape + ARROW_LEN;
            let return_type = Box::new(format_hangable_type_info(ctx, return_type, shape, 1));

            TypeInfo::Callback {
                generics,
                parentheses,
                arguments,
                arrow,
                return_type,
            }
        }

        TypeInfo::Generic {
            base,
            arrows,
            generics,
        } => {
            let base = format_token_reference(ctx, base, shape);
            let shape = shape.take_first_line(&base);
            let (arrows, generics) = format_type_info_generics(ctx, arrows, generics, shape);

            TypeInfo::Generic {
                base,
                arrows,
                generics,
            }
        }

        TypeInfo::GenericPack { name, ellipsis } => {
            let name = format_token_reference(ctx, name, shape);
            let ellipsis = fmt_symbol!(ctx, ellipsis, "...", shape);

            TypeInfo::GenericPack { name, ellipsis }
        }

        TypeInfo::Intersection(intersection) => {
            // TODO: reuse 'format_punctuated' from general.rs?
            let mut types = Punctuated::new();
            let mut shape = shape;

            let leading = match intersection.leading() {
                Some(leading) => {
                    let result = fmt_symbol!(ctx, leading, "& ", shape);
                    shape = shape + 2;
                    Some(result)
                }
                None => None,
            };

            for pair in intersection.types().pairs() {
                let new_pair = match pair {
                    Pair::Punctuated(left, pipe) => {
                        let result = Pair::Punctuated(
                            format_type_info_internal(
                                ctx,
                                left,
                                context.mark_contains_intersect(),
                                shape,
                            ),
                            fmt_symbol!(ctx, pipe, " & ", shape),
                        );
                        shape = shape + 3;
                        result
                    }
                    Pair::End(right) => Pair::End(format_type_info_internal(
                        ctx,
                        right,
                        context.mark_contains_intersect(),
                        shape + 3,
                    )),
                };
                types.push(new_pair);
            }

            TypeInfo::Intersection(TypeIntersection::new(leading, types))
        }

        TypeInfo::Module {
            module,
            punctuation,
            type_info,
        } => {
            let module = format_token_reference(ctx, module, shape);
            let punctuation = fmt_symbol!(ctx, punctuation, ".", shape);
            let type_info = Box::new(format_indexed_type_info(
                ctx,
                type_info,
                shape + (strip_trivia(&module).to_string().len() + 1), // 1 = "."
            ));
            TypeInfo::Module {
                module,
                punctuation,
                type_info,
            }
        }

        TypeInfo::NativeArray { base, brackets } => {
            let base = Box::new(format_type_info_internal(
                ctx,
                base,
                TypeInfoContext {
                    within_array: true,
                    ..context
                },
                shape,
            ));
            let left = fmt_symbol!(ctx, brackets.tokens().0, "[", shape);
            let right = fmt_symbol!(ctx, brackets.tokens().1, "]", shape);
            TypeInfo::NativeArray {
                base,
                brackets: ContainedSpan::new(left, right),
            }
        }
        TypeInfo::Optional {
            base,
            question_mark,
        } => {
            let base = Box::new(format_type_info_internal(
                ctx,
                base,
                context.mark_within_optional().mark_contains_union(),
                shape,
            ));
            let question_mark = fmt_symbol!(ctx, question_mark, "?", shape);
            TypeInfo::Optional {
                base,
                question_mark,
            }
        }

        TypeInfo::Table { braces, fields } => {
            let (start_brace, end_brace) = braces.tokens().to_owned();
            let contains_comments = start_brace.trailing_trivia().any(trivia_is_comment)
                || end_brace.leading_trivia().any(trivia_is_comment)
                || fields.pairs().any(|field| {
                    contains_comments(field.punctuation()) || contains_comments(field.value())
                });

            let table_type = match (contains_comments, fields.iter().next()) {
                // Table contains comments, so force multiline
                (true, _) => TableType::MultiLine,

                (false, Some(_)) => {
                    let braces_range = (
                        start_brace.token().end_position().bytes(),
                        end_brace.token().start_position().bytes(),
                    );

                    let singleline_shape = shape + (braces_range.1 - braces_range.0);

                    match singleline_shape.over_budget() {
                        true => TableType::MultiLine,
                        false => {
                            // Determine if there was a new line at the end of the start brace
                            // If so, then we should always be multiline
                            if start_brace.trailing_trivia().any(trivia_is_newline) {
                                TableType::MultiLine
                            } else {
                                TableType::SingleLine
                            }
                        }
                    }
                }

                (false, None) => TableType::Empty,
            };

            let (braces, fields) = match table_type {
                TableType::Empty => {
                    let braces =
                        create_table_braces(ctx, start_brace, end_brace, table_type, shape);
                    (braces, Punctuated::new())
                }
                TableType::SingleLine => {
                    format_singleline_table(ctx, braces, fields, format_type_field, shape)
                }
                TableType::MultiLine => {
                    format_multiline_table(ctx, braces, fields, format_type_field, shape)
                }
            };

            TypeInfo::Table { braces, fields }
        }

        TypeInfo::Typeof {
            typeof_token,
            parentheses,
            inner,
        } => {
            let typeof_token = format_symbol(
                ctx,
                typeof_token,
                &TokenReference::new(
                    vec![],
                    Token::new(TokenType::Identifier {
                        identifier: "typeof".into(),
                    }),
                    vec![],
                ),
                shape,
            );
            let shape = shape + 6; // 6 = "typeof"
            let parentheses = format_contained_span(ctx, parentheses, shape);
            let inner = Box::new(format_expression(ctx, inner, shape + 1)); // 1 = "("
            TypeInfo::Typeof {
                typeof_token,
                parentheses,
                inner,
            }
        }

        TypeInfo::Tuple { parentheses, types } => {
            let (start_brace, end_brace) = parentheses.tokens();
            let should_format_multiline = start_brace.has_trailing_comments(CommentSearch::Single)
                || end_brace.has_leading_comments(CommentSearch::Single)
                || types.pairs().any(|pair| {
                    pair.punctuation().map_or_else(
                        || pair.value().has_trailing_comments(CommentSearch::All),
                        contains_comments,
                    )
                });

            let singleline_parentheses = format_contained_span(ctx, parentheses, shape);
            let singleline_types =
                format_punctuated(ctx, types, shape + 1, |ctx, type_info, shape| {
                    format_type_info_internal(ctx, type_info, context, shape)
                }); // 1 = "("

            let (parentheses, types) = if should_format_multiline
                || shape.add_width(2).test_over_budget(&singleline_types)
            {
                format_contained_punctuated_multiline(
                    ctx,
                    parentheses,
                    types,
                    |ctx, type_info, shape| format_hangable_type_info(ctx, type_info, shape, 0),
                    shape,
                )
            } else if types.len() == 1 && !keep_parentheses(types.iter().next().unwrap(), context) {
                // If its just a single type inside parentheses, and its not a function or composite type, then remove the parens
                let internal_type = singleline_types.into_iter().next().unwrap();

                // Transfer over any comments
                return internal_type.update_trailing_trivia(FormatTriviaType::Append(
                    singleline_parentheses.tokens().1.trailing_comments(),
                ));
            } else {
                (singleline_parentheses, singleline_types)
            };

            TypeInfo::Tuple { parentheses, types }
        }

        TypeInfo::Union(union) => {
            // TODO: reuse 'format_punctuated' from general.rs?
            let mut types = Punctuated::new();
            let mut shape = shape;

            let leading = match union.leading() {
                Some(leading) => {
                    let result = fmt_symbol!(ctx, leading, "| ", shape);
                    shape = shape + 2;
                    Some(result)
                }
                None => None,
            };

            for pair in union.types().pairs() {
                let new_pair = match pair {
                    Pair::Punctuated(left, pipe) => {
                        let result = Pair::Punctuated(
                            format_type_info_internal(
                                ctx,
                                left,
                                context.mark_contains_union(),
                                shape,
                            ),
                            fmt_symbol!(ctx, pipe, " | ", shape),
                        );
                        shape = shape + 3;
                        result
                    }
                    Pair::End(right) => Pair::End(format_type_info_internal(
                        ctx,
                        right,
                        context.mark_contains_union(),
                        shape + 3,
                    )),
                };
                types.push(new_pair);
            }

            TypeInfo::Union(TypeUnion::new(leading, types))
        }

        TypeInfo::Variadic {
            ellipsis,
            type_info,
        } => {
            let ellipsis = fmt_symbol!(ctx, ellipsis, "...", shape);
            let type_info = Box::new(format_type_info_internal(
                ctx,
                type_info,
                context.mark_within_variadic(),
                shape + 3,
            )); // 3 = "..."

            TypeInfo::Variadic {
                ellipsis,
                type_info,
            }
        }

        TypeInfo::VariadicPack { ellipsis, name } => {
            let name = format_token_reference(ctx, name, shape);
            let ellipsis = fmt_symbol!(ctx, ellipsis, "...", shape);

            TypeInfo::VariadicPack { ellipsis, name }
        }

        other => panic!("unknown node {:?}", other),
    }
}

// A clone of [`hang_binop`], except for TypeInfo tokens. TODO: can we merge the two?
fn hang_type_info_binop(
    ctx: &Context,
    binop: TokenReference,
    shape: Shape,
    next_comments: Vec<Token>,
) -> TokenReference {
    // Get the leading comments of a binop, as we need to preserve them
    // Intersperse a newline and indent trivia between them
    // iter_intersperse is currently not available, so we need to do something different. Tracking issue: https://github.com/rust-lang/rust/issues/79524
    let leading_comments = comments(binop.leading_trivia())
        .flat_map(|x| {
            vec![
                create_newline_trivia(ctx),
                create_indent_trivia(ctx, shape),
                x,
            ]
        })
        // If there are any comments trailing the BinOp, we need to move them to before the BinOp
        .chain(space_prefixed_comments(binop.trailing_trivia()))
        // If there are any leading comments to the RHS expression, we need to move them to before the BinOp
        .chain(next_comments.iter().flat_map(|x| {
            vec![
                create_newline_trivia(ctx),
                create_indent_trivia(ctx, shape),
                x.to_owned(),
            ]
        }))
        // Create a newline just before the BinOp, and preserve the indentation
        .chain(std::iter::once(create_newline_trivia(ctx)))
        .chain(std::iter::once(create_indent_trivia(ctx, shape)))
        .collect();

    binop.update_trivia(
        FormatTriviaType::Replace(leading_comments),
        FormatTriviaType::Replace(vec![Token::new(TokenType::spaces(1))]),
    )
}

/// Hangs a type info at a pipe operator, then reformats either side with the new shape
fn hang_type_info(
    ctx: &Context,
    type_info: &TypeInfo,
    context: TypeInfoContext,
    shape: Shape,
    hang_level: usize,
) -> TypeInfo {
    const PIPE_LENGTH: usize = 2; // "| "

    let hanging_shape = shape.with_indent(shape.indent().add_indent_level(hang_level));

    match type_info {
        TypeInfo::Union(union) => {
            let mut types = Punctuated::new();
            let mut iter = union.types().pairs().peekable();
            let mut is_first = true;
            while let Some(pair) = iter.next() {
                let new_pair = match pair {
                    Pair::Punctuated(type_info, next_pipe) => {
                        let next_comments = iter.peek().leading_comments();
                        Pair::Punctuated(
                            format_type_info_internal(
                                ctx,
                                &(if is_first {
                                    type_info.clone()
                                } else {
                                    type_info
                                        .update_leading_trivia(FormatTriviaType::Replace(vec![]))
                                }),
                                context.mark_contains_union(),
                                if is_first { shape } else { hanging_shape },
                            ),
                            hang_type_info_binop(
                                ctx,
                                next_pipe.to_owned(),
                                hanging_shape,
                                next_comments,
                            ),
                        )
                    }
                    Pair::End(type_info) => Pair::End(format_type_info_internal(
                        ctx,
                        &type_info.update_leading_trivia(FormatTriviaType::Replace(vec![])),
                        context.mark_contains_union(),
                        hanging_shape.reset() + PIPE_LENGTH,
                    )),
                };
                types.push(new_pair);
                is_first = false;
            }

            // TODO: leading should play a role in the shape computation
            // TODO(#910): we should decide whether we add or remove a leading token (maybe if hang_level = 0?)

            TypeInfo::Union(TypeUnion::new(
                union
                    .leading()
                    .map(|token| fmt_symbol!(ctx, token, "| ", shape)),
                types,
            ))
        }

        TypeInfo::Intersection(intersection) => {
            let mut types = Punctuated::new();
            let mut iter = intersection.types().pairs().peekable();
            let mut is_first = true;
            while let Some(pair) = iter.next() {
                let new_pair = match pair {
                    Pair::Punctuated(type_info, next_pipe) => {
                        let next_comments = iter.peek().leading_comments();
                        Pair::Punctuated(
                            format_type_info_internal(
                                ctx,
                                &(if is_first {
                                    type_info.clone()
                                } else {
                                    type_info
                                        .update_leading_trivia(FormatTriviaType::Replace(vec![]))
                                }),
                                context.mark_contains_intersect(),
                                if is_first { shape } else { hanging_shape },
                            ),
                            hang_type_info_binop(
                                ctx,
                                next_pipe.to_owned(),
                                hanging_shape,
                                next_comments,
                            ),
                        )
                    }
                    Pair::End(type_info) => Pair::End(format_type_info_internal(
                        ctx,
                        &type_info.update_leading_trivia(FormatTriviaType::Replace(vec![])),
                        context.mark_contains_intersect(),
                        hanging_shape.reset() + PIPE_LENGTH,
                    )),
                };
                types.push(new_pair);
                is_first = false;
            }

            // TODO: leading should play a role in the shape computation
            // TODO(#910): we should decide whether we add or remove a leading token (maybe if hang_level = 0?)

            TypeInfo::Intersection(TypeIntersection::new(
                intersection
                    .leading()
                    .map(|token| fmt_symbol!(ctx, token, "& ", shape)),
                types,
            ))
        }
        other => format_type_info_internal(ctx, other, context, shape),
    }
}

fn can_hang_type(type_info: &TypeInfo) -> bool {
    matches!(
        type_info,
        // Can hang a binary operation
        TypeInfo::Union(_) | TypeInfo::Intersection(_)
    )
}

pub fn format_indexed_type_info(
    ctx: &Context,
    indexed_type_info: &IndexedTypeInfo,
    shape: Shape,
) -> IndexedTypeInfo {
    match indexed_type_info {
        IndexedTypeInfo::Basic(token_reference) => {
            IndexedTypeInfo::Basic(format_token_reference(ctx, token_reference, shape))
        }

        IndexedTypeInfo::Generic {
            base,
            arrows,
            generics,
        } => {
            let base = format_token_reference(ctx, base, shape);
            let shape = shape.take_first_line(&base);
            let (arrows, generics) = format_type_info_generics(ctx, arrows, generics, shape);

            IndexedTypeInfo::Generic {
                base,
                arrows,
                generics,
            }
        }

        other => panic!("unknown node {:?}", other),
    }
}

fn format_type_argument(ctx: &Context, type_argument: &TypeArgument, shape: Shape) -> TypeArgument {
    const COLON_LEN: usize = ": ".len();

    let name = match type_argument.name() {
        Some((name, colon_token)) => {
            let name = format_token_reference(ctx, name, shape);
            let colon_token = fmt_symbol!(ctx, colon_token, ": ", shape);

            Some((name, colon_token))
        }
        None => None,
    };

    let shape = shape
        + name.as_ref().map_or(0, |(name, _)| {
            strip_trivia(name).to_string().len() + COLON_LEN
        });

    let type_info = format_hangable_type_info(ctx, type_argument.type_info(), shape, 1);

    type_argument
        .to_owned()
        .with_name(name)
        .with_type_info(type_info)
}

/// Formats a [`TypeField`] present inside of a [`TypeInfo::Table`]
/// Returns the new [`TypeField`] and any trailing trivia associated with its value (as this may need to later be moved).
/// If the [`TableType`] provided is [`TableType::MultiLine`] then the trailing trivia from the value will be removed.
pub fn format_type_field(
    ctx: &Context,
    type_field: &TypeField,
    table_type: TableType,
    shape: Shape,
) -> (TypeField, Vec<Token>) {
    let leading_trivia = match table_type {
        TableType::MultiLine => FormatTriviaType::Append(vec![create_indent_trivia(ctx, shape)]),
        _ => FormatTriviaType::NoChange,
    };

    let access = type_field.access().map(|token_reference| {
        format_token_reference(ctx, token_reference, shape)
            .update_leading_trivia(leading_trivia.clone())
            .update_trailing_trivia(FormatTriviaType::Append(vec![Token::new(
                TokenType::spaces(1),
            )]))
    });
    let access_shape_increment = access
        .as_ref()
        .map_or(0, |token| strip_leading_trivia(token).to_string().len() + 1);

    let key = format_type_field_key(
        ctx,
        type_field.key(),
        if access.is_some() {
            FormatTriviaType::NoChange
        } else {
            leading_trivia
        },
        shape,
    );
    let colon_token = fmt_symbol!(ctx, type_field.colon_token(), ": ", shape);
    let colon_token = match &key {
        TypeFieldKey::OptionalName { question, .. } => {
            after_tua_comment(ctx, question, colon_token, shape)
        }
        _ => colon_token,
    };
    let shape = shape + access_shape_increment + (strip_leading_trivia(&key).to_string().len() + 2);

    let mut value = format_type_info(ctx, type_field.value(), shape);

    // Trailing trivia consists only of single line comments - multiline comments are kept in place
    let trailing_trivia = value.trailing_comments_search(CommentSearch::Single);

    if let TableType::MultiLine = table_type {
        // If still over budget, hang the type
        if can_hang_type(type_field.value())
            && (should_hang_type(type_field.value(), CommentSearch::Single)
                || shape.test_over_budget(&value))
        {
            value = hang_type_info(ctx, type_field.value(), TypeInfoContext::new(), shape, 1)
        };

        // Keep multiline comments in place
        let multiline_comments = value.trailing_comments_search(CommentSearch::Multiline);
        value = value.update_trailing_trivia(FormatTriviaType::Replace(multiline_comments))
    }

    if matches!(key, TypeFieldKey::OptionalName { .. }) {
        value = after_tua_comment(ctx, &colon_token, value, shape);
    }

    (
        type_field
            .to_owned()
            .with_access(access)
            .with_key(key)
            .with_colon_token(colon_token)
            .with_value(value),
        trailing_trivia,
    )
}

pub fn format_type_field_key(
    ctx: &Context,
    type_field_key: &TypeFieldKey,
    leading_trivia: FormatTriviaType,
    shape: Shape,
) -> TypeFieldKey {
    match type_field_key {
        TypeFieldKey::OptionalName { name, question } => {
            let name =
                format_token_reference(ctx, name, shape).update_leading_trivia(leading_trivia);
            let question =
                after_tua_comment(ctx, &name, fmt_symbol!(ctx, question, "?", shape), shape);
            TypeFieldKey::OptionalName { name, question }
        }
        TypeFieldKey::Name(token) => TypeFieldKey::Name(
            format_token_reference(ctx, token, shape).update_leading_trivia(leading_trivia),
        ),
        TypeFieldKey::IndexSignature { brackets, inner } => TypeFieldKey::IndexSignature {
            brackets: format_contained_span(ctx, brackets, shape)
                .update_leading_trivia(leading_trivia),
            inner: Box::new(format_type_info_internal(
                ctx,
                inner,
                TypeInfoContext::new().mark_within_table_indexer(),
                shape + 1,
            )), // 1 = "["
        },
        other => panic!("unknown node {:?}", other),
    }
}

pub fn format_type_assertion(
    ctx: &Context,
    type_assertion: &TypeAssertion,
    shape: Shape,
) -> TypeAssertion {
    let assertion_op = fmt_symbol!(ctx, type_assertion.assertion_op(), " :: ", shape);
    let cast_to = format_type_info(ctx, type_assertion.cast_to(), shape + 4); // 4 = " :: "

    TypeAssertion::new(cast_to).with_assertion_op(assertion_op)
}

/// Checks a type info to see if it should be hanged due to comments being present
fn should_hang_type(type_info: &TypeInfo, comment_search: CommentSearch) -> bool {
    // Only hang if its a binary type info, since it doesn't matter for unary types
    match type_info {
        TypeInfo::Union(union) => {
            let has_leading = union.leading().is_some();
            union.leading().has_trailing_comments(comment_search)
                || union
                    .types()
                    .pairs()
                    .enumerate()
                    .any(|(idx, pair)| match pair {
                        Pair::Punctuated(type_info, binop) => {
                            ((idx != 0 || has_leading)
                                && type_info.has_leading_comments(comment_search))
                                || type_info.has_trailing_comments(comment_search)
                                || contains_comments(binop)
                        }
                        Pair::End(type_info) => type_info.has_leading_comments(comment_search),
                    })
        }
        TypeInfo::Intersection(intersection) => {
            let has_leading = intersection.leading().is_some();
            intersection.leading().has_trailing_comments(comment_search)
                || intersection
                    .types()
                    .pairs()
                    .enumerate()
                    .any(|(idx, pair)| match pair {
                        Pair::Punctuated(type_info, binop) => {
                            ((idx != 0 || has_leading)
                                && type_info.has_leading_comments(comment_search))
                                || type_info.has_trailing_comments(comment_search)
                                || contains_comments(binop)
                        }
                        Pair::End(type_info) => type_info.has_leading_comments(comment_search),
                    })
        }
        _ => false,
    }
}

fn attempt_assigned_type_tactics(
    ctx: &Context,
    equal_token: TokenReference,
    type_info: &TypeInfo,
    context: TypeInfoContext,
    shape: Shape,
) -> (TokenReference, TypeInfo) {
    const EQUAL_TOKEN_LENGTH: usize = " = ".len();

    if token_contains_comments(&equal_token) || type_info.has_leading_comments(CommentSearch::All) {
        // We will hang at the equals token, and then format the declaration as necessary
        let equal_token = hang_equal_token(ctx, &equal_token, shape, false);

        let shape = shape.reset().increment_additional_indent();

        // Format declaration, hanging if it contains comments (ignoring leading and trailing comments, as they won't affect anything)
        let declaration = if contains_comments(strip_trivia(type_info)) {
            hang_type_info(ctx, type_info, context, shape, 0)
        } else {
            let proper_declaration = format_type_info_internal(ctx, type_info, context, shape);
            if shape.test_over_budget(&proper_declaration) {
                hang_type_info(ctx, type_info, context, shape, 0)
            } else {
                proper_declaration
            }
        };

        // Take the leading comments and format them nicely
        let leading_comments = type_info
            .leading_comments()
            .iter()
            .flat_map(|x| {
                vec![
                    create_indent_trivia(ctx, shape),
                    x.to_owned(),
                    create_newline_trivia(ctx),
                ]
            })
            .chain(std::iter::once(create_indent_trivia(ctx, shape)))
            .collect();

        let declaration =
            declaration.update_leading_trivia(FormatTriviaType::Replace(leading_comments));

        (equal_token, declaration)
    } else {
        let mut equal_token = equal_token;
        let type_definition;
        let singleline_type_definition =
            format_type_info_internal(ctx, type_info, context, shape.with_infinite_width());
        let proper_type_definition =
            format_type_info_internal(ctx, type_info, context, shape + EQUAL_TOKEN_LENGTH);

        // Test to see whether the type definition must be hung due to comments
        let must_hang = should_hang_type(type_info, CommentSearch::All);

        // If we can hang the type definition, and its over width, then lets try doing so
        if can_hang_type(type_info)
            && (must_hang
                || (shape + EQUAL_TOKEN_LENGTH)
                    .test_over_budget(&strip_trailing_trivia(&singleline_type_definition))
                || spans_multiple_lines(&singleline_type_definition))
        {
            // If we should hug the type, then lets check out the proper definition and see if it fits
            if !must_hang
                && should_hug_type(type_info)
                && !is_union_of_tables(type_info)
                && !(shape + EQUAL_TOKEN_LENGTH).test_over_budget(&proper_type_definition)
            {
                type_definition = proper_type_definition;
            } else {
                // Use a hanging equal token
                equal_token = hang_equal_token(ctx, &equal_token, shape, true);

                let shape = shape.reset().increment_additional_indent();
                let hanging_type_definition = hang_type_info(ctx, type_info, context, shape, 0);
                type_definition = hanging_type_definition;
            }
        } else {
            // Test whether the proper formatting goes over the column width
            // If so, hang at the equals token and reformat
            if (shape + EQUAL_TOKEN_LENGTH).test_over_budget(&proper_type_definition) {
                // Hang at the equal token
                equal_token = hang_equal_token(ctx, &equal_token, shape, true);

                // Add the expression list into the indent range, as it will be indented by one
                let shape = shape.reset().increment_additional_indent();
                type_definition = format_type_info_internal(ctx, type_info, context, shape);
            } else {
                // Use the proper formatting
                type_definition = proper_type_definition;
            }
        }

        (equal_token, type_definition)
    }
}

fn format_type_declaration(
    ctx: &Context,
    type_declaration: &TypeDeclaration,
    add_leading_trivia: bool,
    shape: Shape,
) -> TypeDeclaration {
    const TYPE_TOKEN_LENGTH: usize = "type ".len();

    // Calculate trivia
    let trailing_trivia = vec![create_newline_trivia(ctx)];

    let mut type_token = format_symbol(
        ctx,
        type_declaration.type_token(),
        &TokenReference::new(
            vec![],
            Token::new(TokenType::Identifier {
                identifier: "type".into(),
            }),
            vec![Token::new(TokenType::spaces(1))],
        ),
        shape,
    );

    if add_leading_trivia {
        let leading_trivia = vec![create_indent_trivia(ctx, shape)];
        type_token = type_token.update_leading_trivia(FormatTriviaType::Append(leading_trivia))
    }

    let shape = shape + TYPE_TOKEN_LENGTH;
    let type_name = format_token_reference(ctx, type_declaration.type_name(), shape);
    let shape = shape + type_name.to_string().len();

    let generics = type_declaration
        .generics()
        .map(|generics| format_generic_declaration(ctx, generics, shape));

    let shape = match generics {
        Some(ref generics) => shape.take_last_line(&generics),
        None => shape,
    };

    let equal_token = fmt_symbol!(ctx, type_declaration.equal_token(), " = ", shape);
    let (equal_token, type_definition) = attempt_assigned_type_tactics(
        ctx,
        equal_token,
        type_declaration.type_definition(),
        TypeInfoContext::new(),
        shape,
    );

    // Handle comments in between the type name and generics + generics and equal token
    // (or just type name and equal token if generics not present)

    // If there are comments in between the type name and the generics, then handle them
    let (type_name, equal_token, generics) = if type_name.has_trailing_comments(CommentSearch::All)
        || generics.as_ref().is_some_and(|generics| {
            generics
                .arrows()
                .tokens()
                .0
                .has_leading_comments(CommentSearch::All)
        })
        || equal_token.has_leading_comments(CommentSearch::All)
    {
        // See if we have generics
        if let Some(generics) = generics {
            let (start_arrow, end_arrow) = generics.arrows().tokens();

            let type_name_comments = space_prefixed_comments(
                type_name
                    .trailing_trivia()
                    .chain(start_arrow.leading_trivia()),
            )
            .collect::<Vec<_>>();
            let type_name_comments_len = type_name_comments.len();

            let arrow_comments = space_prefixed_comments(
                end_arrow
                    .trailing_trivia()
                    .chain(equal_token.leading_trivia()),
            )
            .collect();

            (
                type_name.update_trailing_trivia(FormatTriviaType::Replace(type_name_comments)),
                equal_token.update_leading_trivia(FormatTriviaType::Replace(vec![Token::new(
                    TokenType::spaces(1),
                )])),
                Some(generics.to_owned().with_arrows(ContainedSpan::new(
                    start_arrow.update_leading_trivia(FormatTriviaType::Replace(
                        // If there are some comments present between the type name and generics,
                        // then lets add a single space before the arrow to make it look nicer
                        if type_name_comments_len > 0 {
                            vec![Token::new(TokenType::spaces(1))]
                        } else {
                            vec![]
                        },
                    )),
                    end_arrow.update_trailing_trivia(FormatTriviaType::Replace(arrow_comments)),
                ))),
            )
        } else {
            let name_comments = space_prefixed_comments(
                type_name
                    .trailing_trivia()
                    .chain(equal_token.leading_trivia()),
            )
            .collect();

            (
                type_name.update_trailing_trivia(FormatTriviaType::Replace(name_comments)),
                equal_token.update_leading_trivia(FormatTriviaType::Replace(vec![Token::new(
                    TokenType::spaces(1),
                )])),
                generics,
            )
        }
    } else {
        (type_name, equal_token, generics)
    };

    let type_definition =
        type_definition.update_trailing_trivia(FormatTriviaType::Append(trailing_trivia));

    type_declaration
        .to_owned()
        .with_type_token(type_token)
        .with_type_name(type_name)
        .with_generics(generics)
        .with_equal_token(equal_token)
        .with_type_definition(type_definition)
}

/// Wrapper around `format_type_declaration` for statements
/// This is required as `format_type_declaration` is also used for ExportedTypeDeclaration, and we don't want leading trivia there
pub fn format_type_declaration_stmt(
    ctx: &Context,
    type_declaration: &TypeDeclaration,
    shape: Shape,
) -> TypeDeclaration {
    format_type_declaration(ctx, type_declaration, true, shape)
}

fn format_type_function(
    ctx: &Context,
    type_function: &TypeFunction,
    add_leading_trivia: bool,
    shape: Shape,
) -> TypeFunction {
    const TYPE_TOKEN_LENGTH: usize = "type ".len();
    const FUNCTION_TOKEN_LENGTH: usize = "function ".len();

    // Calculate trivia
    let trailing_trivia = vec![create_newline_trivia(ctx)];
    let function_definition_trivia = vec![create_function_definition_trivia(ctx)];

    let mut type_token = format_symbol(
        ctx,
        type_function.type_token(),
        &TokenReference::new(
            vec![],
            Token::new(TokenType::Identifier {
                identifier: "type".into(),
            }),
            vec![Token::new(TokenType::spaces(1))],
        ),
        shape,
    );

    if add_leading_trivia {
        let leading_trivia = vec![create_indent_trivia(ctx, shape)];
        type_token = type_token.update_leading_trivia(FormatTriviaType::Append(leading_trivia))
    }

    let function_token = fmt_symbol!(ctx, type_function.function_token(), "function ", shape);
    let function_name = format_token_reference(ctx, type_function.function_name(), shape)
        .update_trailing_trivia(FormatTriviaType::Append(function_definition_trivia));

    let shape = shape
        + (TYPE_TOKEN_LENGTH
            + FUNCTION_TOKEN_LENGTH
            + strip_trivia(&function_name).to_string().len());
    let function_body = format_function_body(ctx, type_function.function_body(), shape)
        .update_trailing_trivia(FormatTriviaType::Append(trailing_trivia));

    TypeFunction::new(function_name, function_body)
        .with_type_token(type_token)
        .with_function_token(function_token)
}

/// Wrapper around `format_type_function` for statements
/// This is required as `format_type_function` is also used for ExportedTypeFunction, and we don't want leading trivia there
pub fn format_type_function_stmt(
    ctx: &Context,
    type_function: &TypeFunction,
    shape: Shape,
) -> TypeFunction {
    format_type_function(ctx, type_function, true, shape)
}

fn format_generic_parameter(
    ctx: &Context,
    generic_parameter: &GenericDeclarationParameter,
    shape: Shape,
) -> GenericDeclarationParameter {
    let parameter_info = match generic_parameter.parameter() {
        GenericParameterInfo::Bounded { name, colon, bound } => {
            let name = format_token_reference(ctx, name, shape);
            let colon = after_tua_comment(ctx, &name, fmt_symbol!(ctx, colon, ": ", shape), shape);
            let bound = after_tua_comment(
                ctx,
                &colon,
                format_token_reference(ctx, bound, shape + name.to_string().len() + 2),
                shape,
            );
            GenericParameterInfo::Bounded { name, colon, bound }
        }
        GenericParameterInfo::Name(token_reference) => {
            GenericParameterInfo::Name(format_token_reference(ctx, token_reference, shape))
        }
        GenericParameterInfo::Variadic { name, ellipsis } => {
            let name = format_token_reference(ctx, name, shape);
            let ellipsis = fmt_symbol!(ctx, ellipsis, "...", shape);

            GenericParameterInfo::Variadic { name, ellipsis }
        }

        other => panic!("unknown node {:?}", other),
    };

    let context = match generic_parameter.parameter() {
        GenericParameterInfo::Variadic { .. } => TypeInfoContext::new().mark_within_generic(),
        _ => TypeInfoContext::new(),
    };

    let default_type = match (generic_parameter.equals(), generic_parameter.default_type()) {
        (Some(equals), Some(default_type)) => {
            let equals = fmt_symbol!(ctx, equals, " = ", shape);
            let (equals, default_type) =
                attempt_assigned_type_tactics(ctx, equals, default_type, context, shape);
            Some((equals, default_type))
        }
        (None, None) => None,
        _ => unreachable!("have generic parameter default type with no equals or vice versa"),
    };

    generic_parameter
        .to_owned()
        .with_parameter(parameter_info)
        .with_default(default_type)
}

pub fn format_generic_declaration(
    ctx: &Context,
    generic_declaration: &GenericDeclaration,
    shape: Shape,
) -> GenericDeclaration {
    const ARROW_LEN: usize = 1; // 1 = "<"

    let singleline_arrows = format_contained_span(ctx, generic_declaration.arrows(), shape);
    let singleline_generics = format_punctuated(
        ctx,
        generic_declaration.generics(),
        shape.with_infinite_width(),
        format_generic_parameter,
    );

    let (start_arrow, end_arrow) = generic_declaration.arrows().tokens();
    let contains_comments = start_arrow.has_trailing_comments(CommentSearch::All)
        || end_arrow.has_leading_comments(CommentSearch::All)
        || contains_comments(generic_declaration.generics());

    let should_expand = contains_comments
        || shape
            .add_width(ARROW_LEN * 2)
            .test_over_budget(&singleline_generics);

    let (arrows, generics) = if should_expand {
        format_contained_punctuated_multiline(
            ctx,
            generic_declaration.arrows(),
            generic_declaration.generics(),
            format_generic_parameter, // TODO: hangable?
            shape,
        )
    } else {
        (singleline_arrows, singleline_generics)
    };

    generic_declaration
        .to_owned()
        .with_arrows(arrows)
        .with_generics(generics)
}

pub fn format_type_specifier(
    ctx: &Context,
    type_specifier: &TypeSpecifier,
    shape: Shape,
) -> TypeSpecifier {
    let punctuation = fmt_symbol!(ctx, type_specifier.punctuation(), ": ", shape);
    let type_info = format_type_info(ctx, type_specifier.type_info(), shape + 2); // 2 = ": "

    type_specifier
        .to_owned()
        .with_punctuation(punctuation)
        .with_type_info(type_info)
}

/// The width of the last line of an [`IfConditionBinding`], ignoring comments.
pub fn if_condition_binding_width(binding: &IfConditionBinding) -> usize {
    let text = format!(
        "{} {}{} =",
        strip_trivia(binding.local_token()),
        strip_trivia(binding.name()),
        binding
            .type_specifier()
            .map_or(String::new(), |specifier| format!(
                ": {}",
                strip_trivia(specifier.type_info())
            )),
    );
    text.lines().last().map_or(0, |line| line.chars().count())
}

/// Formats an [`IfConditionBinding`] node - the `local <name> =` / `const <name> =` part of an
/// `if local` / `if const` binding, as in `if local player = getPlayer() then ... end`.
/// The formatted binding is `<local|const> <name>[: <type>] = ` (with a trailing space), so it
/// slots directly between the `if `/`elseif ` token and the condition expression.
pub fn format_if_condition_binding(
    ctx: &Context,
    binding: &IfConditionBinding,
    shape: Shape,
) -> IfConditionBinding {
    // Preserve the keyword (`local` or `const`), normalising to a single trailing space
    let local_token = format_token_reference(ctx, binding.local_token(), shape)
        .update_trailing_trivia(FormatTriviaType::Append(vec![Token::new(
            TokenType::spaces(1),
        )]));
    let name_shape = shape
        + (strip_trivia(binding.local_token())
            .to_string()
            .chars()
            .count()
            + 1);
    let name = format_token_reference(ctx, binding.name(), name_shape);
    let specifier_shape = name_shape + strip_trivia(binding.name()).to_string().chars().count();
    let type_specifier = binding
        .type_specifier()
        .map(|type_specifier| format_type_specifier(ctx, type_specifier, specifier_shape));
    let equal_token = fmt_symbol!(ctx, binding.equal_token(), " = ", shape);

    binding
        .to_owned()
        .with_local_token(local_token)
        .with_name(name)
        .with_type_specifier(type_specifier)
        .with_equal_token(equal_token)
}

pub fn format_type_instantiation(
    ctx: &Context,
    type_instantiation: &TypeInstantiation,
    shape: Shape,
) -> TypeInstantiation {
    let outer_arrows = format_contained_span(ctx, type_instantiation.outer_arrows(), shape);
    let inner_arrows = format_contained_span(ctx, type_instantiation.inner_arrows(), shape);
    let context = TypeInfoContext::new().mark_within_generic();
    let types = format_punctuated(
        ctx,
        type_instantiation.types(),
        shape,
        |ctx, type_info, shape| format_type_info_internal(ctx, type_info, context, shape),
    );

    type_instantiation
        .to_owned()
        .with_outer_arrows(outer_arrows)
        .with_inner_arrows(inner_arrows)
        .with_types(types)
}

pub fn format_exported_type_declaration(
    ctx: &Context,
    exported_type_declaration: &ExportedTypeDeclaration,
    shape: Shape,
) -> ExportedTypeDeclaration {
    // Calculate trivia
    let shape = shape.reset();
    let leading_trivia = vec![create_indent_trivia(ctx, shape)];

    let export_token = format_symbol(
        ctx,
        exported_type_declaration.export_token(),
        &TokenReference::new(
            vec![],
            Token::new(TokenType::Identifier {
                identifier: "export".into(),
            }),
            vec![Token::new(TokenType::spaces(1))],
        ),
        shape,
    )
    .update_leading_trivia(FormatTriviaType::Append(leading_trivia));
    let type_declaration = format_type_declaration(
        ctx,
        exported_type_declaration.type_declaration(),
        false,
        shape + 7, // 7 = "export "
    );

    exported_type_declaration
        .to_owned()
        .with_export_token(export_token)
        .with_type_declaration(type_declaration)
}

pub fn format_exported_type_function(
    ctx: &Context,
    exported_type_function: &ExportedTypeFunction,
    shape: Shape,
) -> ExportedTypeFunction {
    // Calculate trivia
    let shape = shape.reset();
    let leading_trivia = vec![create_indent_trivia(ctx, shape)];

    let export_token = format_symbol(
        ctx,
        exported_type_function.export_token(),
        &TokenReference::new(
            vec![],
            Token::new(TokenType::Identifier {
                identifier: "export".into(),
            }),
            vec![Token::new(TokenType::spaces(1))],
        ),
        shape,
    )
    .update_leading_trivia(FormatTriviaType::Append(leading_trivia));
    let type_function = format_type_function(
        ctx,
        exported_type_function.type_function(),
        false,
        shape + 7, // 7 = "export "
    );

    exported_type_function
        .to_owned()
        .with_export_token(export_token)
        .with_type_function(type_function)
}

pub fn const_keyword_token_ref() -> TokenReference {
    TokenReference::new(
        vec![],
        Token::new(TokenType::Identifier {
            identifier: "const".into(),
        }),
        vec![Token::new(TokenType::spaces(1))],
    )
}

pub fn format_luau_attribute(
    ctx: &Context,
    attribute: &LuauAttribute,
    shape: Shape,
) -> LuauAttribute {
    let at_sign = fmt_symbol!(ctx, attribute.at_sign(), "@", shape);
    let name = format_token_reference(ctx, attribute.name(), shape);

    attribute.clone().with_at_sign(at_sign).with_name(name)
}

/// Format an authored enum without converting it into a type alias.
pub fn format_tua_enum(
    ctx: &Context,
    declaration: &full_moon::ast::luau::TuaEnumDeclaration,
    shape: Shape,
) -> full_moon::ast::luau::TuaEnumDeclaration {
    use crate::formatters::general::{format_end_token, EndTokenType};
    let leading = FormatTriviaType::Append(vec![create_indent_trivia(ctx, shape)]);
    let newline = FormatTriviaType::Append(vec![create_newline_trivia(ctx)]);
    let enum_token = format_token_reference(ctx, &declaration.enum_token, shape)
        .update_leading_trivia(leading.clone())
        .update_trailing_trivia(FormatTriviaType::Append(vec![Token::new(
            TokenType::spaces(1),
        )]));
    let name = format_token_reference(ctx, &declaration.name, shape + 5)
        .update_trailing_trivia(newline.clone());
    let inner = shape.reset().increment_block_indent();
    let mut members = Punctuated::new();
    for pair in declaration.members.pairs() {
        let member = format_token_reference(ctx, pair.value(), inner).update_leading_trivia(
            FormatTriviaType::Append(vec![create_indent_trivia(ctx, inner)]),
        );
        let pair = match pair.punctuation() {
            Some(comma) => Pair::Punctuated(
                member,
                fmt_symbol!(ctx, comma, ",", inner).update_trailing_trivia(newline.clone()),
            ),
            None => Pair::End(member.update_trailing_trivia(newline.clone())),
        };
        members.push(pair);
    }
    let end_token = format_end_token(
        ctx,
        &declaration.end_token,
        EndTokenType::IndentComments,
        shape,
    )
    .update_trivia(leading, newline);
    full_moon::ast::luau::TuaEnumDeclaration {
        enum_token,
        name,
        members,
        end_token,
    }
}

fn format_protocol_parameter(
    ctx: &Context,
    parameter: &full_moon::ast::luau::TuaProtocolParameter,
    shape: Shape,
) -> full_moon::ast::luau::TuaProtocolParameter {
    let name = format_token_reference(ctx, &parameter.name, shape);
    let question = parameter
        .question
        .as_ref()
        .map(|token| after_tua_comment(ctx, &name, fmt_symbol!(ctx, token, "?", shape), shape));
    let annotation = format_type_specifier(
        ctx,
        &parameter.annotation,
        shape + name.to_string().len() + usize::from(question.is_some()),
    );
    let colon = after_tua_comment(
        ctx,
        question.as_ref().unwrap_or(&name),
        annotation.punctuation().clone(),
        shape,
    );
    let info = after_tua_comment(ctx, &colon, annotation.type_info().clone(), shape);
    full_moon::ast::luau::TuaProtocolParameter {
        name,
        question,
        annotation: annotation.with_punctuation(colon).with_type_info(info),
    }
}

/// Format bodyless method contracts using the shared type/parameter layout.
pub fn format_tua_protocol(
    ctx: &Context,
    declaration: &full_moon::ast::luau::TuaProtocolDeclaration,
    shape: Shape,
) -> full_moon::ast::luau::TuaProtocolDeclaration {
    use crate::formatters::general::{format_end_token, EndTokenType};
    let indent = FormatTriviaType::Append(vec![create_indent_trivia(ctx, shape)]);
    let newline = FormatTriviaType::Append(vec![create_newline_trivia(ctx)]);
    let keyword = |token: &TokenReference| {
        format_symbol(
            ctx,
            token,
            &TokenReference::new(
                vec![],
                token.token().clone(),
                vec![Token::new(TokenType::spaces(1))],
            ),
            shape,
        )
    };
    let export = declaration
        .export
        .as_ref()
        .map(|token| keyword(token).update_leading_trivia(indent.clone()));
    let protocol_token = keyword(&declaration.protocol_token);
    let protocol_token = match &export {
        Some(export) => after_tua_comment(ctx, export, protocol_token, shape),
        None => protocol_token.update_leading_trivia(indent.clone()),
    };
    let name = after_tua_comment(
        ctx,
        &protocol_token,
        format_token_reference(ctx, &declaration.name, shape),
        shape,
    );
    let left = after_tua_comment(
        ctx,
        &name,
        fmt_symbol!(ctx, declaration.braces.tokens().0, " {", shape),
        shape,
    )
    .update_trailing_trivia(newline.clone());
    let inner = shape.reset().increment_block_indent();
    let mut methods = Vec::new();
    for method in &declaration.methods {
        let name = format_token_reference(ctx, &method.name, inner).update_leading_trivia(
            FormatTriviaType::Append(vec![create_indent_trivia(ctx, inner)]),
        );
        let parameter_shape = inner + strip_trivia(&name).to_string().len();
        let single = format_punctuated(
            ctx,
            &method.parameters,
            parameter_shape + 1,
            format_protocol_parameter,
        );
        let returns = format_type_specifier(
            ctx,
            &method.returns,
            parameter_shape + single.to_string().len() + 2,
        );
        let multiline = contains_comments(&method.parameters)
            || contains_comments(&method.parentheses)
            || (parameter_shape + single.to_string().len() + returns.to_string().len() + 2)
                .over_budget();
        let (parentheses, parameters) = if multiline && !method.parameters.is_empty() {
            format_contained_punctuated_multiline(
                ctx,
                &method.parentheses,
                &method.parameters,
                format_protocol_parameter,
                inner,
            )
        } else {
            (
                format_contained_span(ctx, &method.parentheses, parameter_shape),
                single,
            )
        };
        let left = after_tua_comment(ctx, &name, parentheses.tokens().0.clone(), inner);
        let right = parentheses.tokens().1.clone();
        let colon = after_tua_comment(ctx, &right, returns.punctuation().clone(), inner);
        let info = after_tua_comment(ctx, &colon, returns.type_info().clone(), inner);
        let mut returns = returns.with_punctuation(colon).with_type_info(info);
        let comma = method.comma.as_ref().map(|token| {
            fmt_symbol!(ctx, token, ",", inner).update_trailing_trivia(newline.clone())
        });
        if comma.is_none() {
            returns = returns.update_trailing_trivia(newline.clone());
        }
        methods.push(full_moon::ast::luau::TuaProtocolMethod {
            name,
            parentheses: ContainedSpan::new(left, right),
            parameters,
            returns,
            comma,
        });
    }
    let right = format_end_token(
        ctx,
        declaration.braces.tokens().1,
        EndTokenType::IndentComments,
        shape,
    )
    .update_trivia(indent, newline);
    full_moon::ast::luau::TuaProtocolDeclaration {
        export,
        protocol_token,
        name,
        braces: ContainedSpan::new(left, right),
        methods,
    }
}

/// Format an authored class while sharing ordinary function and expression layout.
pub fn format_tua_class(
    ctx: &Context,
    declaration: &full_moon::ast::luau::TuaClassDeclaration,
    shape: Shape,
) -> full_moon::ast::luau::TuaClassDeclaration {
    use crate::formatters::general::{format_end_token, EndTokenType};
    use full_moon::{ast::luau::*, node::Node};
    let indent = FormatTriviaType::Append(vec![create_indent_trivia(ctx, shape)]);
    let newline = FormatTriviaType::Append(vec![create_newline_trivia(ctx)]);
    let word = |token: &TokenReference, leading_space: bool| {
        format_symbol(
            ctx,
            token,
            &TokenReference::new(
                if leading_space {
                    vec![Token::new(TokenType::spaces(1))]
                } else {
                    vec![]
                },
                token.token().clone(),
                vec![Token::new(TokenType::spaces(1))],
            ),
            shape,
        )
    };
    let qualifier = declaration
        .qualifier
        .as_ref()
        .map(|token| word(token, false).update_leading_trivia(indent.clone()));
    let class_token = word(&declaration.class_token, false);
    let class_token = match &qualifier {
        Some(token) => after_tua_comment(ctx, token, class_token, shape),
        None => class_token.update_leading_trivia(indent.clone()),
    };
    let name = after_tua_comment(
        ctx,
        &class_token,
        format_token_reference(ctx, &declaration.name, shape),
        shape,
    );
    let generics = declaration
        .generics
        .as_ref()
        .map(|g| format_generic_declaration(ctx, g, shape + name.token().to_string().len()));
    let mut previous = generics
        .as_ref()
        .and_then(|g| g.tokens().next_back())
        .unwrap_or(&name)
        .clone();
    let extends = declaration
        .extends
        .as_ref()
        .map(|token| after_tua_comment(ctx, &previous, word(token, true), shape));
    let base = declaration.base.as_ref().map(|base| {
        let base = format_type_info(ctx, base, shape);
        let base = after_tua_comment(ctx, extends.as_ref().unwrap(), base, shape);
        previous = base.tokens().next_back().unwrap().clone();
        base
    });
    let implements = declaration
        .implements
        .as_ref()
        .map(|token| after_tua_comment(ctx, &previous, word(token, true), shape));
    let mut contracts = format_punctuated(ctx, &declaration.contracts, shape, format_type_info);
    if let Some(token) = &implements {
        let first = contracts.pairs().next().cloned();
        if let Some(first) = first {
            let mut updated = Punctuated::new();
            updated.push(first.map(|ty| after_tua_comment(ctx, token, ty, shape)));
            for pair in contracts.pairs().skip(1) {
                updated.push(pair.clone());
            }
            contracts = updated;
        }
        if let Some(last) = contracts.tokens().next_back() {
            previous = last.clone();
        }
    }
    let left = after_tua_comment(
        ctx,
        &previous,
        fmt_symbol!(ctx, declaration.braces.tokens().0, " {", shape),
        shape,
    )
    .update_trailing_trivia(newline.clone());
    let inner = shape.reset().increment_block_indent();
    let mut members = Vec::new();
    for member in &declaration.members {
        members.push(match member {
            TuaClassMember::Field(field) => {
                let name = format_token_reference(ctx, &field.name, inner).update_leading_trivia(
                    FormatTriviaType::Append(vec![create_indent_trivia(ctx, inner)]),
                );
                let question = field.question.as_ref().map(|token| {
                    after_tua_comment(ctx, &name, fmt_symbol!(ctx, token, "?", inner), inner)
                });
                let annotation = format_type_specifier(
                    ctx,
                    &field.annotation,
                    inner + name.token().to_string().len(),
                );
                let colon = after_tua_comment(
                    ctx,
                    question.as_ref().unwrap_or(&name),
                    annotation.punctuation().clone(),
                    inner,
                );
                let ty = after_tua_comment(ctx, &colon, annotation.type_info().clone(), inner);
                let mut annotation = annotation.with_punctuation(colon).with_type_info(ty);
                let (equal, value) = match (&field.equal, &field.value) {
                    (Some(equal), Some(value)) => {
                        let previous = annotation.tokens().next_back().unwrap();
                        let equal = after_tua_comment(
                            ctx,
                            previous,
                            fmt_symbol!(ctx, equal, " = ", inner),
                            inner,
                        );
                        let (equal, value) = if equal.has_trailing_comments(CommentSearch::Single)
                            || value.has_leading_comments(CommentSearch::All)
                        {
                            crate::formatters::assignment::hang_at_equals_due_to_comments(
                                ctx, &equal, value, inner,
                            )
                        } else {
                            (
                                equal,
                                format_expression(
                                    ctx,
                                    value,
                                    inner
                                        + strip_trivia(&name).to_string().len()
                                        + annotation.to_string().len()
                                        + 3,
                                ),
                            )
                        };
                        (
                            Some(equal),
                            Some(Box::new(value.update_trailing_trivia(newline.clone()))),
                        )
                    }
                    (None, None) => {
                        annotation = annotation.update_trailing_trivia(newline.clone());
                        (None, None)
                    }
                    _ => unreachable!("class default token/expression must occur together"),
                };
                TuaClassMember::Field(TuaClassField {
                    name,
                    question,
                    annotation,
                    equal,
                    value,
                })
            }
            TuaClassMember::Method(method) => {
                let mut modifiers = Vec::new();
                for token in &method.modifiers {
                    let token = word(token, false);
                    let token = if let Some(previous) = modifiers.last() {
                        after_tua_comment(ctx, previous, token, inner)
                    } else {
                        token.update_leading_trivia(FormatTriviaType::Append(vec![
                            create_indent_trivia(ctx, inner),
                        ]))
                    };
                    modifiers.push(token);
                }
                let name = format_token_reference(ctx, &method.name, inner);
                let name = if let Some(previous) = modifiers.last() {
                    after_tua_comment(ctx, previous, name, inner)
                } else {
                    name.update_leading_trivia(FormatTriviaType::Append(vec![
                        create_indent_trivia(ctx, inner),
                    ]))
                };
                let width = modifiers
                    .iter()
                    .map(|token| token.token().to_string().len() + 1)
                    .sum::<usize>()
                    + name.token().to_string().len();
                let body = format_function_body(ctx, &method.body, inner + width);
                let parameters = body.parameters_parentheses();
                let left = after_tua_comment(ctx, &name, parameters.tokens().0.clone(), inner);
                let parentheses = ContainedSpan::new(left, parameters.tokens().1.clone());
                let end_token = body.end_token().update_trailing_trivia(newline.clone());
                let body = body
                    .with_parameters_parentheses(parentheses)
                    .with_end_token(end_token);
                TuaClassMember::Method(TuaClassMethod {
                    modifiers,
                    name,
                    body,
                })
            }
        });
    }
    let right = format_end_token(
        ctx,
        declaration.braces.tokens().1,
        EndTokenType::IndentComments,
        shape,
    )
    .update_trivia(indent, newline);
    TuaClassDeclaration {
        qualifier,
        class_token,
        name,
        generics,
        extends,
        base,
        implements,
        contracts,
        braces: ContainedSpan::new(left, right),
        members,
    }
}

/// Format authored imports without loading modules or lowering their syntax.
pub fn format_tua_type_import(
    ctx: &Context,
    declaration: &full_moon::ast::luau::TuaTypeImport,
    shape: Shape,
) -> full_moon::ast::luau::TuaTypeImport {
    use full_moon::ast::luau::TuaImportName;
    let space = FormatTriviaType::Append(vec![Token::new(TokenType::spaces(1))]);
    let newline = FormatTriviaType::Append(vec![create_newline_trivia(ctx)]);
    let word = |token: &TokenReference| {
        format_token_reference(ctx, token, shape).update_trailing_trivia(space.clone())
    };
    let keyword = word(&declaration.keyword).update_leading_trivia(FormatTriviaType::Append(vec![
        create_indent_trivia(ctx, shape),
    ]));
    let type_token = after_tua_comment(ctx, &keyword, word(&declaration.type_token), shape);
    let multiline = declaration.open.is_some()
        && (declaration.names.to_string().contains('\n')
            || declaration
                .open
                .as_ref()
                .is_some_and(|token| token.trailing_trivia().any(trivia_is_newline))
            || declaration
                .close
                .as_ref()
                .is_some_and(|token| token.leading_trivia().any(trivia_is_newline))
            || shape.test_over_budget(declaration));
    let inner = if multiline {
        shape.reset().increment_block_indent()
    } else {
        shape
    };
    let open = declaration.open.as_ref().map(|token| {
        let token = fmt_symbol!(ctx, token, "{", shape).update_trailing_trivia(if multiline {
            newline.clone()
        } else {
            space.clone()
        });
        after_tua_comment(ctx, &type_token, token, shape)
    });
    let mut names = Punctuated::new();
    for (index, pair) in declaration.names.pairs().enumerate() {
        let binding = pair.value();
        let mut name = format_token_reference(ctx, &binding.name, inner);
        if multiline {
            name =
                name.update_leading_trivia(FormatTriviaType::Append(vec![create_indent_trivia(
                    ctx, inner,
                )]));
        } else if index == 0 {
            name = after_tua_comment(ctx, open.as_ref().unwrap_or(&type_token), name, inner);
        }
        let as_token = binding.as_token.as_ref().map(|token| {
            name = name.update_trailing_trivia(space.clone());
            after_tua_comment(ctx, &name, word(token), inner)
        });
        let alias = binding.alias.as_ref().map(|token| {
            after_tua_comment(
                ctx,
                as_token.as_ref().unwrap(),
                format_token_reference(ctx, token, inner),
                inner,
            )
        });
        let mut binding = TuaImportName {
            name,
            as_token,
            alias,
        };
        let mut comma = if pair.punctuation().is_some() || (multiline && declaration.open.is_some())
        {
            let original = pair
                .punctuation()
                .cloned()
                .unwrap_or_else(|| TokenReference::symbol(",").unwrap());
            Some(
                fmt_symbol!(ctx, &original, ",", inner).update_trailing_trivia(if multiline {
                    newline.clone()
                } else {
                    space.clone()
                }),
            )
        } else {
            None
        };
        if let Some(punctuation) = &mut comma {
            let last = binding.alias.as_mut().unwrap_or(&mut binding.name);
            let mut trailing = last.trailing_trivia().cloned().collect::<Vec<_>>();
            *last = last.update_trailing_trivia(FormatTriviaType::Replace(Vec::new()));
            trailing.extend(punctuation.trailing_trivia().cloned());
            *punctuation = punctuation.update_trailing_trivia(FormatTriviaType::Replace(trailing));
        }
        if comma.is_none() {
            if let Some(alias) = &mut binding.alias {
                *alias = alias.update_trailing_trivia(space.clone());
            } else {
                binding.name = binding.name.update_trailing_trivia(space.clone());
            }
        }
        names.push(Pair::new(binding, comma));
    }
    let close = declaration.close.as_ref().map(|token| {
        let token = fmt_symbol!(ctx, token, "}", shape).update_trailing_trivia(space.clone());
        if multiline {
            token.update_leading_trivia(FormatTriviaType::Append(vec![create_indent_trivia(
                ctx, shape,
            )]))
        } else {
            token
        }
    });
    let previous = close.as_ref().unwrap_or_else(|| {
        let binding = names.last().expect("an import has a binding").value();
        binding.alias.as_ref().unwrap_or(&binding.name)
    });
    let from = after_tua_comment(ctx, previous, word(&declaration.from), shape);
    let module = after_tua_comment(
        ctx,
        &from,
        format_token_reference(ctx, &declaration.module, shape),
        shape,
    )
    .update_trailing_trivia(newline);
    full_moon::ast::luau::TuaTypeImport {
        keyword,
        type_token,
        open,
        names,
        close,
        from,
        module,
    }
}
