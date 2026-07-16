use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const MAX_METATABLE_FIELD_DEPTH: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Primitive(PrimitiveType),
    Any,
    Alias(String),
    StringLiteral(String),
    Union(Vec<Type>),
    Function(FunctionType),
    Table(TableShape),
    Array(Box<Type>),
    Tuple(Vec<Type>),
    Dictionary { key: Box<Type>, value: Box<Type> },
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

    pub fn string_literal(value: impl Into<String>) -> Self {
        Self::StringLiteral(value.into())
    }

    pub fn union(types: impl IntoIterator<Item = Type>) -> Self {
        Self::Union(types.into_iter().collect())
    }

    pub fn function(parameters: impl IntoIterator<Item = Type>, return_type: Type) -> Self {
        Self::Function(FunctionType::new(parameters, return_type))
    }

    pub fn function_returns(
        parameters: impl IntoIterator<Item = Type>,
        return_types: impl IntoIterator<Item = Type>,
    ) -> Self {
        Self::Function(FunctionType::new_returns(parameters, return_types))
    }

    pub fn function_returns_with_required(
        parameters: impl IntoIterator<Item = Type>,
        required_parameter_count: usize,
        return_types: impl IntoIterator<Item = Type>,
    ) -> Self {
        Self::Function(FunctionType::new_returns_with_required(
            parameters,
            required_parameter_count,
            return_types,
        ))
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

    pub fn array(item: Type) -> Self {
        Self::Array(Box::new(item))
    }

    pub fn tuple(items: impl IntoIterator<Item = Type>) -> Self {
        Self::Tuple(items.into_iter().collect())
    }

    pub fn dictionary(key: Type, value: Type) -> Self {
        Self::Dictionary {
            key: Box::new(key),
            value: Box::new(value),
        }
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

#[derive(Clone, Debug)]
pub struct FunctionParameter {
    pub name: Option<String>,
    pub ty: Type,
}

impl PartialEq for FunctionParameter {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
    }
}

impl Eq for FunctionParameter {}

impl FunctionParameter {
    pub fn anonymous(ty: Type) -> Self {
        Self { name: None, ty }
    }

    pub fn named(name: impl Into<String>, ty: Type) -> Self {
        Self {
            name: Some(name.into()),
            ty,
        }
    }
}

impl fmt::Display for FunctionParameter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = self.name.as_deref() {
            write!(formatter, "{name}: ")?;
        }

        write!(formatter, "{}", self.ty)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionType {
    pub parameters: Vec<FunctionParameter>,
    pub required_parameter_count: usize,
    pub vararg_type: Option<Box<Type>>,
    pub return_type: Box<Type>,
    pub return_types: Vec<Type>,
}

impl FunctionType {
    pub fn new(parameters: impl IntoIterator<Item = Type>, return_type: Type) -> Self {
        Self::new_returns(parameters, [return_type])
    }

    pub fn new_returns(
        parameters: impl IntoIterator<Item = Type>,
        return_types: impl IntoIterator<Item = Type>,
    ) -> Self {
        let parameters = parameters.into_iter().collect::<Vec<_>>();
        let required_parameter_count = parameters.len();
        Self::new_returns_with_required(parameters, required_parameter_count, return_types)
    }

    pub fn new_returns_with_required(
        parameters: impl IntoIterator<Item = Type>,
        required_parameter_count: usize,
        return_types: impl IntoIterator<Item = Type>,
    ) -> Self {
        Self::new_signature(
            parameters.into_iter().map(FunctionParameter::anonymous),
            required_parameter_count,
            None,
            return_types,
        )
    }

    pub fn new_signature(
        parameters: impl IntoIterator<Item = FunctionParameter>,
        required_parameter_count: usize,
        vararg_type: Option<Type>,
        return_types: impl IntoIterator<Item = Type>,
    ) -> Self {
        let parameters = parameters.into_iter().collect::<Vec<_>>();
        let mut return_types = return_types.into_iter().collect::<Vec<_>>();

        if return_types.is_empty() {
            return_types.push(Type::nil());
        }

        let required_parameter_count = required_parameter_count.min(parameters.len());

        Self {
            parameters,
            required_parameter_count,
            vararg_type: vararg_type.map(Box::new),
            return_type: Box::new(return_types[0].clone()),
            return_types,
        }
    }

    pub fn return_types_or_any(&self) -> &[Type] {
        if self.return_types.is_empty() {
            std::slice::from_ref(&self.return_type)
        } else {
            &self.return_types
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableShape {
    fields: BTreeMap<String, TableField>,
    metatable: Option<Box<MetatableFacts>>,
    self_index: bool,
    constructor_methods: BTreeSet<String>,
}

impl TableShape {
    pub fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
            metatable: None,
            self_index: false,
            constructor_methods: BTreeSet::new(),
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

    pub fn semantic_field(&self, name: &str) -> Option<&TableField> {
        self.semantic_field_at_depth(name, 0)
    }

    fn semantic_field_at_depth(&self, name: &str, depth: usize) -> Option<&TableField> {
        if is_metamethod_field(name) {
            return None;
        }

        self.field(name).or_else(|| {
            self.metatable
                .as_ref()?
                .implied_field_at_depth(name, depth + 1)
        })
    }

    pub fn fields(&self) -> impl Iterator<Item = (&str, &TableField)> {
        self.fields
            .iter()
            .map(|(name, field)| (name.as_str(), field))
    }

    pub fn semantic_fields(&self) -> BTreeMap<String, TableField> {
        self.semantic_fields_at_depth(0)
    }

    fn semantic_fields_at_depth(&self, depth: usize) -> BTreeMap<String, TableField> {
        let mut fields = self
            .fields
            .iter()
            .filter(|(name, _)| !is_metamethod_field(name))
            .map(|(name, field)| (name.clone(), field.clone()))
            .collect::<BTreeMap<_, _>>();

        if let Some(facts) = &self.metatable {
            for (name, field) in facts.implied_fields_at_depth(depth + 1) {
                fields.entry(name).or_insert(field);
            }
        }

        fields
    }

    pub fn metatable_facts(&self) -> Option<&MetatableFacts> {
        self.metatable.as_deref()
    }

    pub fn with_metatable_facts(mut self, facts: MetatableFacts) -> Self {
        self.metatable = Some(Box::new(facts));
        self
    }

    pub fn set_metatable_facts(&mut self, facts: MetatableFacts) {
        self.metatable = Some(Box::new(facts));
    }

    pub fn mark_self_index(&mut self) {
        self.self_index = true;
    }

    pub fn has_self_index(&self) -> bool {
        self.self_index
    }

    pub fn mark_constructor_method(&mut self, name: impl Into<String>) {
        self.constructor_methods.insert(name.into());
    }

    pub fn is_constructor_method(&self, name: &str) -> bool {
        self.constructor_methods.contains(name)
    }

    pub fn constructor_methods(&self) -> impl Iterator<Item = &str> {
        self.constructor_methods.iter().map(String::as_str)
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

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct MetatableFacts {
    pub index: IndexFallback,
    pub newindex: NewIndexRule,
    pub add: Option<FunctionType>,
    pub sub: Option<FunctionType>,
    pub mul: Option<FunctionType>,
    pub div: Option<FunctionType>,
    pub eq: Option<FunctionType>,
    pub lt: Option<FunctionType>,
    pub le: Option<FunctionType>,
    pub len: Option<FunctionType>,
    pub call: Option<FunctionType>,
    pub pairs: Option<FunctionType>,
    pub ipairs: Option<FunctionType>,
    pub tostring: Option<FunctionType>,
    pub fields: TableShape,
}

impl MetatableFacts {
    pub fn from_type(ty: &Type) -> Self {
        let Type::Table(table) = ty else {
            return Self::default();
        };

        let index = if table.has_self_index() {
            IndexFallback::Table(table.clone())
        } else {
            table
                .field("__index")
                .map(|field| match &field.ty {
                    Type::Table(shape) => IndexFallback::Table(shape.clone()),
                    Type::Function(function) => IndexFallback::Function(function.clone()),
                    _ => IndexFallback::Unknown,
                })
                .unwrap_or_default()
        };
        let newindex = table
            .field("__newindex")
            .map(|field| match &field.ty {
                Type::Table(shape) => NewIndexRule::Table(shape.clone()),
                Type::Function(function) => NewIndexRule::Function(function.clone()),
                _ => NewIndexRule::Unknown,
            })
            .unwrap_or_default();

        Self {
            index,
            newindex,
            add: function_field(table, "__add"),
            sub: function_field(table, "__sub"),
            mul: function_field(table, "__mul"),
            div: function_field(table, "__div"),
            eq: function_field(table, "__eq"),
            lt: function_field(table, "__lt"),
            le: function_field(table, "__le"),
            len: function_field(table, "__len"),
            call: function_field(table, "__call"),
            pairs: function_field(table, "__pairs"),
            ipairs: function_field(table, "__ipairs"),
            tostring: function_field(table, "__tostring"),
            fields: table.clone(),
        }
    }

    pub fn metamethod(&self, name: &str) -> Option<&FunctionType> {
        match name {
            "__add" => self.add.as_ref(),
            "__sub" => self.sub.as_ref(),
            "__mul" => self.mul.as_ref(),
            "__div" => self.div.as_ref(),
            "__eq" => self.eq.as_ref(),
            "__lt" => self.lt.as_ref(),
            "__le" => self.le.as_ref(),
            "__len" => self.len.as_ref(),
            "__call" => self.call.as_ref(),
            "__pairs" => self.pairs.as_ref(),
            "__ipairs" => self.ipairs.as_ref(),
            "__tostring" => self.tostring.as_ref(),
            _ => None,
        }
    }

    pub fn apply_to_type(&self, base: Type) -> Type {
        let Type::Table(mut base_table) = base else {
            return base;
        };

        base_table.set_metatable_facts(self.clone());

        Type::Table(base_table)
    }

    pub fn apply_to_table(&self, table: &mut TableShape) {
        table.set_metatable_facts(self.clone());
    }

    pub fn implied_field(&self, name: &str) -> Option<&TableField> {
        self.implied_field_at_depth(name, 0)
    }

    fn implied_field_at_depth(&self, name: &str, depth: usize) -> Option<&TableField> {
        if depth >= MAX_METATABLE_FIELD_DEPTH {
            return None;
        }

        if is_metamethod_field(name) {
            return None;
        }

        match &self.index {
            IndexFallback::Table(table) => table.semantic_field_at_depth(name, depth + 1),
            _ => None,
        }
    }

    fn implied_fields_at_depth(&self, depth: usize) -> BTreeMap<String, TableField> {
        if depth >= MAX_METATABLE_FIELD_DEPTH {
            return BTreeMap::new();
        }

        match &self.index {
            IndexFallback::Table(table) => table.semantic_fields_at_depth(depth + 1),
            IndexFallback::None | IndexFallback::Function(_) | IndexFallback::Unknown => {
                BTreeMap::new()
            }
        }
    }
}

fn function_field(table: &TableShape, name: &str) -> Option<FunctionType> {
    table.field(name).and_then(|field| match &field.ty {
        Type::Function(function) => Some(function.clone()),
        _ => None,
    })
}

fn is_metamethod_field(name: &str) -> bool {
    name.starts_with("__")
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum IndexFallback {
    #[default]
    None,
    Table(TableShape),
    Function(FunctionType),
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum NewIndexRule {
    #[default]
    None,
    Table(TableShape),
    Function(FunctionType),
    Unknown,
}

impl NewIndexRule {
    pub fn assignment_type_for_key(&self, key: &str) -> Option<Type> {
        match self {
            NewIndexRule::Table(table) => table.field(key).map(|field| field.ty.clone()),
            NewIndexRule::Function(function) => {
                let key_type = function.parameters.get(1)?;
                let value_type = function.parameters.get(2)?;

                if newindex_key_accepts(&key_type.ty, key) {
                    Some(value_type.ty.clone())
                } else {
                    None
                }
            }
            NewIndexRule::None | NewIndexRule::Unknown => None,
        }
    }
}

fn newindex_key_accepts(ty: &Type, key: &str) -> bool {
    match ty {
        Type::Any => false,
        Type::Primitive(PrimitiveType::String) => true,
        Type::StringLiteral(value) => value == key,
        Type::Union(types) => types.iter().any(|ty| newindex_key_accepts(ty, key)),
        _ => false,
    }
}

impl fmt::Display for Type {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Primitive(primitive) => write!(formatter, "{primitive}"),
            Type::Any => write!(formatter, "any"),
            Type::Alias(name) => write!(formatter, "{name}"),
            Type::StringLiteral(value) => write!(formatter, "{value:?}"),
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
                write!(formatter, "(")?;
                for (index, parameter) in function.parameters.iter().enumerate() {
                    if index > 0 {
                        write!(formatter, ", ")?;
                    }
                    if let Some(name) = parameter.name.as_deref() {
                        write!(formatter, "{name}: ")?;
                    }
                    write!(formatter, "{}", parameter.ty)?;
                }
                if let Some(vararg_type) = function.vararg_type.as_deref() {
                    if !function.parameters.is_empty() {
                        write!(formatter, ", ")?;
                    }
                    write!(formatter, "...{vararg_type}")?;
                }
                let returns = function.return_types_or_any();
                if returns.len() > 1 {
                    write!(formatter, ") -> ({})", format_type_list(returns))
                } else {
                    write!(formatter, ") -> {}", format_type_list(returns))
                }
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
            Type::Array(item) => write!(formatter, "{item}[]"),
            Type::Tuple(items) => {
                write!(formatter, "[")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        write!(formatter, ", ")?;
                    }
                    write!(formatter, "{item}")?;
                }
                write!(formatter, "]")
            }
            Type::Dictionary { key, value } => write!(formatter, "{{ [{key}]: {value} }}"),
        }
    }
}

fn format_type_list(types: &[Type]) -> String {
    types
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
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
