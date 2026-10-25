//! Conservative checks for LOVE's uniform value contract; unknowns stay quiet.
use super::*;
use crate::types::PrimitiveType;

#[derive(Clone, Copy)]
enum Shape {
    Number,
    Boolean,
    Vector(usize, bool),
    Matrix(usize),
    Texture,
}

impl Shape {
    fn for_glsl(name: &str) -> Option<Self> {
        Some(match name {
            "number" | "float" | "int" | "uint" => Self::Number,
            "bool" => Self::Boolean,
            "vec2" | "ivec2" | "uvec2" => Self::Vector(2, false),
            "vec3" | "ivec3" | "uvec3" => Self::Vector(3, false),
            "vec4" | "ivec4" | "uvec4" => Self::Vector(4, false),
            "bvec2" => Self::Vector(2, true),
            "bvec3" => Self::Vector(3, true),
            "bvec4" => Self::Vector(4, true),
            "mat2" => Self::Matrix(2),
            "mat3" => Self::Matrix(3),
            "mat4" => Self::Matrix(4),
            "Image" | "ArrayImage" | "CubeImage" | "VolumeImage" | "sampler2D"
            | "sampler2DArray" | "samplerCube" | "sampler3D" => Self::Texture,
            _ => return None,
        })
    }
}

impl Checker<'_> {
    pub(super) fn check_shader_values(
        &mut self,
        function: &FunctionCall,
        call: &asset::ShaderSendCall,
        uniforms: &ShaderUniforms,
    ) {
        let candidates: Vec<_> = uniforms
            .model
            .uniforms
            .values()
            .filter(|uniform| match &call.uniform {
                asset::ShaderUniformReference::Exact(name) => uniform.name == *name,
                asset::ShaderUniformReference::Pattern { prefix, suffix, .. } => {
                    uniform.name.starts_with(prefix)
                        && uniform.name.ends_with(suffix)
                        && uniform.name.len() > prefix.len() + suffix.len()
                }
            })
            .collect();
        let Some(uniform) = candidates.first() else {
            return;
        };
        // Dynamic patterns with different contracts cannot prove a mismatch.
        if candidates
            .iter()
            .any(|other| other.detail() != uniform.detail())
        {
            return;
        }
        let Some(mut shape) = Shape::for_glsl(&uniform.glsl_type) else {
            return;
        };
        let Some(Suffix::Call(Call::MethodCall(method))) = function.suffixes().next() else {
            return;
        };
        let color = token_text(method.name()) == "sendColor";
        if color {
            if !matches!(uniform.glsl_type.as_str(), "vec3" | "vec4") {
                self.shader_value_error(
                    call.span,
                    uniform,
                    "sendColor requires a vec3 or vec4 uniform",
                );
                return;
            }
            shape = Shape::Vector(3, false);
        }
        let FunctionArgs::Parentheses { arguments, .. } = method.args() else {
            return;
        };
        let mut values: Vec<_> = arguments.iter().skip(1).collect();
        if let Some(first) = values.first() {
            let ty = self.infer_expression(first).unwrap_or(Type::Any);
            // The Data overload and dynamic overload selection need runtime
            // byte sizes, offsets, or layout information that we do not have.
            if dynamic_overload(&ty, self.environment, 0) {
                return;
            }
            if matches!(shape, Shape::Matrix(_))
                && matches!(&ty, Type::StringLiteral(s) if s == "row" || s == "column")
            {
                values.remove(0);
            }
        }
        // LOVE ignores values beyond the declared array length (including
        // extra values for scalar uniforms). Unknown bounds stay conservative.
        let count = if uniform.array_suffix.is_empty() {
            Some(1)
        } else {
            uniform.array_len
        };
        let Some(count) = count else {
            return;
        };
        for value in values.into_iter().take(count) {
            if self.shader_value_rejected(value, shape) {
                self.shader_value_error(
                    node_span(value).unwrap_or(call.span),
                    uniform,
                    &format!(
                        "expected {}",
                        if color {
                            "table with at least 3 numeric components"
                        } else {
                            uniform.lua_value()
                        }
                    ),
                );
            }
        }
    }

    fn shader_value_error(
        &mut self,
        span: SourceSpan,
        uniform: &asset::ShaderUniform,
        reason: &str,
    ) {
        self.push_asset_diagnostic(
            Diagnostic::error(
                &self.parsed.path,
                span.start.line,
                span.start.column,
                format!(
                    "invalid value for shader uniform `{}` ({}): {reason}",
                    uniform.name,
                    uniform.detail()
                ),
            )
            .with_range(
                span.start.line,
                span.start.column,
                span.end.line,
                span.end.column,
            )
            .with_code(DiagnosticCode::ASSET_SHADER_VALUE),
        );
    }

    fn shader_value_rejected(&mut self, expression: &Expression, shape: Shape) -> bool {
        if let Expression::Parentheses { expression, .. } = expression {
            return self.shader_value_rejected(expression, shape);
        }
        if let Expression::TableConstructor(table) = expression {
            if matches!(shape, Shape::Number | Shape::Boolean | Shape::Texture) {
                return true;
            }
            // Only positional constructors have a statically obvious length.
            // Keyed tables and a trailing multi-return expression remain dynamic.
            let values: Option<Vec<_>> = table
                .fields()
                .iter()
                .map(|field| match field {
                    Field::NoKey(value) => Some(value),
                    _ => None,
                })
                .collect();
            if let Some(values) = values {
                if values.last().is_some_and(|last| {
                    matches!(last, Expression::FunctionCall(_))
                        || matches!(last, Expression::Symbol(token) if token_text(token) == "...")
                }) {
                    return false;
                }
                match shape {
                    Shape::Vector(size, boolean) => {
                        let component = if boolean {
                            Shape::Boolean
                        } else {
                            Shape::Number
                        };
                        return values.len() < size
                            || values
                                .iter()
                                .take(size)
                                .any(|value| self.shader_value_rejected(value, component));
                    }
                    Shape::Matrix(size) => {
                        if let Some(first) = values.first() {
                            if matches!(first, Expression::TableConstructor(_)) {
                                return values.len() < size
                                    || values.iter().take(size).any(|value| {
                                        self.shader_value_rejected(
                                            value,
                                            Shape::Vector(size, false),
                                        )
                                    });
                            }
                            let ty = self.infer_expression(first).unwrap_or(Type::Any);
                            if !matches!(ty, Type::Primitive(PrimitiveType::Number)) {
                                return type_rejected(&ty, Shape::Number, self.environment, 0)
                                    && type_rejected(
                                        &ty,
                                        Shape::Vector(size, false),
                                        self.environment,
                                        0,
                                    );
                            }
                        }
                        return values.len() < size * size
                            || values
                                .iter()
                                .take(size * size)
                                .any(|value| self.shader_value_rejected(value, Shape::Number));
                    }
                    _ => unreachable!(),
                }
            }
        }
        let ty = self.infer_expression(expression).unwrap_or(Type::Any);
        type_rejected(&ty, shape, self.environment, 0)
    }
}

fn dynamic_overload(ty: &Type, environment: &TypeEnvironment, depth: usize) -> bool {
    if depth > 16 {
        return true;
    }
    if is_data(ty) || matches!(ty, Type::Any) {
        return true;
    }
    match ty {
        Type::Union(members) => members
            .iter()
            .any(|ty| dynamic_overload(ty, environment, depth + 1)),
        Type::Alias(name) => environment
            .alias(name)
            .is_some_and(|ty| dynamic_overload(ty, environment, depth + 1)),
        _ => false,
    }
}

fn is_data(ty: &Type) -> bool {
    matches!(ty, Type::Alias(name) if matches!(name.as_str(), "Data" | "ByteData" | "DataView" | "FileData" | "ImageData" | "CompressedImageData" | "SoundData"))
}

fn type_rejected(ty: &Type, shape: Shape, environment: &TypeEnvironment, depth: usize) -> bool {
    if depth > 16 || is_data(ty) {
        return false;
    }
    match ty {
        Type::Any => false,
        Type::Union(members) => {
            !members.is_empty()
                && members
                    .iter()
                    .all(|ty| type_rejected(ty, shape, environment, depth + 1))
        }
        Type::Alias(name) => {
            if name == "Transform" {
                return !matches!(shape, Shape::Matrix(4));
            }
            if matches!(name.as_str(), "Image" | "Canvas" | "Texture") {
                return !matches!(shape, Shape::Texture);
            }
            environment
                .alias(name)
                .is_some_and(|ty| type_rejected(ty, shape, environment, depth + 1))
        }
        Type::Primitive(PrimitiveType::Number) => !matches!(shape, Shape::Number),
        Type::Primitive(PrimitiveType::Boolean) => !matches!(shape, Shape::Boolean),
        Type::Primitive(PrimitiveType::String) => {
            !matches!(shape, Shape::Number | Shape::Matrix(_))
        }
        Type::StringLiteral(_) => !matches!(shape, Shape::Number),
        Type::Primitive(PrimitiveType::Nil) | Type::Function(_) => true,
        Type::Table(_) | Type::Dictionary { .. } => {
            matches!(shape, Shape::Number | Shape::Boolean | Shape::Texture)
        }
        Type::Array(item) => match shape {
            Shape::Vector(_, boolean) => type_rejected(
                item,
                if boolean {
                    Shape::Boolean
                } else {
                    Shape::Number
                },
                environment,
                depth + 1,
            ),
            Shape::Matrix(size) => {
                type_rejected(item, Shape::Number, environment, depth + 1)
                    && type_rejected(item, Shape::Vector(size, false), environment, depth + 1)
            }
            _ => true,
        },
        Type::Tuple(items) => match shape {
            Shape::Vector(size, boolean) => {
                items.len() < size
                    || items.iter().take(size).any(|ty| {
                        type_rejected(
                            ty,
                            if boolean {
                                Shape::Boolean
                            } else {
                                Shape::Number
                            },
                            environment,
                            depth + 1,
                        )
                    })
            }
            Shape::Matrix(_) => false,
            _ => true,
        },
    }
}
