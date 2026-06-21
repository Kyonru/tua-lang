use crate::types::{Type, TypeEnvironment};

const MAX_TYPE_RESOLUTION_DEPTH: usize = 64;

pub(crate) fn resolve_field_chain_type(
    environment: &TypeEnvironment,
    parts: &[String],
) -> Option<Type> {
    let (root, fields) = parts.split_first()?;
    let mut current = environment.local(root)?.clone();

    for field in fields {
        current = resolve_table_field_type(environment, &current, field)?;
    }

    Some(current)
}

pub(crate) fn resolve_table_field_type(
    environment: &TypeEnvironment,
    ty: &Type,
    field: &str,
) -> Option<Type> {
    match resolve_aliases(environment, ty, 0) {
        Type::Table(table) => table.field(field).map(|field| field.ty.clone()),
        Type::Union(types) => {
            let mut resolved = Vec::new();

            for ty in types {
                if let Some(field_type) = resolve_table_field_type(environment, &ty, field) {
                    resolved.push(field_type);
                }
            }

            match resolved.len() {
                0 => None,
                1 => resolved.into_iter().next(),
                _ => Some(Type::union(resolved)),
            }
        }
        _ => None,
    }
}

pub(crate) fn resolve_aliases(environment: &TypeEnvironment, ty: &Type, depth: usize) -> Type {
    if depth >= MAX_TYPE_RESOLUTION_DEPTH {
        return ty.clone();
    }

    match ty {
        Type::Alias(name) => environment
            .alias(name)
            .map(|resolved| resolve_aliases(environment, resolved, depth + 1))
            .unwrap_or_else(|| ty.clone()),
        _ => ty.clone(),
    }
}
