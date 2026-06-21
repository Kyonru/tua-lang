use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Primitive(PrimitiveType),
    Any,
    Alias(String),
    Union(Vec<Type>),
    Function(FunctionType),
    Table(TableShape),
}

impl Type {
    pub fn named(name: impl Into<String>) -> Self {
        let name = name.into();

        match name.as_str() {
            "number" => Self::number(),
            "string" => Self::string(),
            "boolean" => Self::boolean(),
            "nil" => Self::nil(),
            "any" => Self::Any,
            _ => Self::Alias(name),
        }
    }

    pub fn number() -> Self {
        Self::Primitive(PrimitiveType::Number)
    }

    pub fn string() -> Self {
        Self::Primitive(PrimitiveType::String)
    }

    pub fn boolean() -> Self {
        Self::Primitive(PrimitiveType::Boolean)
    }

    pub fn nil() -> Self {
        Self::Primitive(PrimitiveType::Nil)
    }

    pub fn alias(name: impl Into<String>) -> Self {
        Self::Alias(name.into())
    }

    pub fn union(types: impl IntoIterator<Item = Type>) -> Self {
        Self::Union(types.into_iter().collect())
    }

    pub fn function(parameters: impl IntoIterator<Item = Type>, return_type: Type) -> Self {
        Self::Function(FunctionType::new(parameters, return_type))
    }

    pub fn empty_table() -> Self {
        Self::Table(TableShape::new())
    }

    pub fn table<I, S>(fields: I) -> Self
    where
        I: IntoIterator<Item = (S, TableField)>,
        S: Into<String>,
    {
        Self::Table(TableShape::from_fields(fields))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimitiveType {
    Number,
    String,
    Boolean,
    Nil,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAlias {
    pub name: String,
    pub ty: Type,
}

impl TypeAlias {
    pub fn new(name: impl Into<String>, ty: Type) -> Self {
        Self {
            name: name.into(),
            ty,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionType {
    pub parameters: Vec<Type>,
    pub return_type: Box<Type>,
}

impl FunctionType {
    pub fn new(parameters: impl IntoIterator<Item = Type>, return_type: Type) -> Self {
        Self {
            parameters: parameters.into_iter().collect(),
            return_type: Box::new(return_type),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableShape {
    fields: BTreeMap<String, TableField>,
}

impl TableShape {
    pub fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    pub fn from_fields<I, S>(fields: I) -> Self
    where
        I: IntoIterator<Item = (S, TableField)>,
        S: Into<String>,
    {
        let mut shape = Self::new();

        for (name, field) in fields {
            shape.insert(name, field);
        }

        shape
    }

    pub fn insert(&mut self, name: impl Into<String>, field: TableField) {
        self.fields.insert(name.into(), field);
    }

    pub fn field(&self, name: &str) -> Option<&TableField> {
        self.fields.get(name)
    }

    pub fn fields(&self) -> impl Iterator<Item = (&str, &TableField)> {
        self.fields
            .iter()
            .map(|(name, field)| (name.as_str(), field))
    }
}

impl Default for TableShape {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableField {
    pub ty: Type,
    pub optional: bool,
}

impl TableField {
    pub fn required(ty: Type) -> Self {
        Self {
            ty,
            optional: false,
        }
    }

    pub fn optional(ty: Type) -> Self {
        Self { ty, optional: true }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Primitive(primitive) => write!(formatter, "{primitive}"),
            Type::Any => write!(formatter, "any"),
            Type::Alias(name) => write!(formatter, "{name}"),
            Type::Union(types) => {
                for (index, ty) in types.iter().enumerate() {
                    if index > 0 {
                        write!(formatter, " | ")?;
                    }
                    write!(formatter, "{ty}")?;
                }
                Ok(())
            }
            Type::Function(function) => {
                write!(formatter, "function(")?;
                for (index, parameter) in function.parameters.iter().enumerate() {
                    if index > 0 {
                        write!(formatter, ", ")?;
                    }
                    write!(formatter, "{parameter}")?;
                }
                write!(formatter, "): {}", function.return_type)
            }
            Type::Table(table) => {
                write!(formatter, "{{ ")?;
                for (index, (name, field)) in table.fields().enumerate() {
                    if index > 0 {
                        write!(formatter, ", ")?;
                    }
                    if field.optional {
                        write!(formatter, "{name}?: {}", field.ty)?;
                    } else {
                        write!(formatter, "{name}: {}", field.ty)?;
                    }
                }
                write!(formatter, " }}")
            }
        }
    }
}

impl fmt::Display for PrimitiveType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number => write!(formatter, "number"),
            Self::String => write!(formatter, "string"),
            Self::Boolean => write!(formatter, "boolean"),
            Self::Nil => write!(formatter, "nil"),
        }
    }
}
