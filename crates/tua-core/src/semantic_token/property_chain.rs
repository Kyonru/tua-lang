//! Optional positional styling for property accesses. Ordinary semantic kinds remain intact.
use super::*;

pub(super) fn apply_chain_modifiers(source: &[Token], semantic: &mut [SemanticToken]) {
    let mut depths = vec![0usize; source.len()];
    // Track where each receiver expression begins, so a grouped calculation
    // cannot inherit the depth of its final operand.
    let mut starts = vec![None; source.len()];
    let mut delimiters = Vec::new();
    let mut properties = BTreeMap::new();
    for (index, token) in source.iter().enumerate() {
        if token_identifier_text(token).is_some() {
            starts[index] = Some(index);
        }
        match token_symbol(token) {
            Some(Symbol::LeftParen | Symbol::LeftBracket | Symbol::LeftBrace) => {
                delimiters.push(index);
            }
            Some(Symbol::RightParen | Symbol::RightBracket | Symbol::RightBrace) => {
                if let Some(open) = delimiters.pop() {
                    let matching = matches!(
                        (token_symbol(&source[open]), token_symbol(token)),
                        (Some(Symbol::LeftParen), Some(Symbol::RightParen))
                            | (Some(Symbol::LeftBracket), Some(Symbol::RightBracket))
                            | (Some(Symbol::LeftBrace), Some(Symbol::RightBrace))
                    );
                    if matching {
                        // Calls/indexes preserve the receiver's depth, not an argument's depth.
                        let receiver = open.checked_sub(1).filter(|&i| {
                            token_identifier_text(&source[i]).is_some()
                                || matches!(
                                    token_symbol(&source[i]),
                                    Some(Symbol::RightParen | Symbol::RightBracket)
                                )
                        });
                        starts[index] = Some(open);
                        if let Some(receiver) = receiver {
                            depths[index] = depths[receiver];
                            starts[index] = starts[receiver];
                        } else if token_symbol(token) == Some(Symbol::RightParen)
                            && index > open + 1
                            && starts[index - 1] == Some(open + 1)
                        {
                            depths[index] = depths[index - 1];
                        }
                    }
                }
            }
            _ => {}
        }
        if index >= 2
            && token_identifier_text(token).is_some()
            && matches!(
                token_symbol(&source[index - 1]),
                Some(Symbol::Dot | Symbol::Colon)
            )
        {
            depths[index] = depths[index - 2] + 1;
            starts[index] = starts[index - 2];
            properties.insert(span_key(token_span(token)), depths[index]);
        }
    }
    let cycle = [
        SemanticTokenModifier::PropertyChain1,
        SemanticTokenModifier::PropertyChain2,
        SemanticTokenModifier::PropertyChain3,
        SemanticTokenModifier::PropertyChain4,
    ];
    for token in semantic {
        if token.kind != SemanticTokenKind::Property
            || token
                .modifiers
                .contains(&SemanticTokenModifier::Declaration)
        {
            continue;
        }
        if let Some(depth) = properties.get(&span_key(token.span)) {
            token.modifiers.push(cycle[(depth - 1) % cycle.len()]);
        }
    }
}
