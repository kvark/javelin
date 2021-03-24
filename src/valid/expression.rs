use super::{FunctionInfo, ShaderStages, TypeFlags};
use crate::{
    arena::{Arena, Handle},
    proc::ResolveError,
};

#[derive(Clone, Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum ExpressionError {
    #[error("Doesn't exist")]
    DoesntExist,
    #[error("Used by a statement before it was introduced into the scope by any of the dominating blocks")]
    NotInScope,
    #[error("Depends on {0:?}, which has not been processed yet")]
    ForwardDependency(Handle<crate::Expression>),
    #[error("Base type {0:?} is not compatible with this expression")]
    InvalidBaseType(Handle<crate::Expression>),
    #[error("Accessing with index {0:?} can't be done")]
    InvalidIndexType(Handle<crate::Expression>),
    #[error("Accessing index {1} is out of {0:?} bounds")]
    IndexOutOfBounds(Handle<crate::Expression>, u32),
    #[error("Function argument {0:?} doesn't exist")]
    FunctionArgumentDoesntExist(u32),
    #[error("Constant {0:?} doesn't exist")]
    ConstantDoesntExist(Handle<crate::Constant>),
    #[error("Global variable {0:?} doesn't exist")]
    GlobalVarDoesntExist(Handle<crate::GlobalVariable>),
    #[error("Local variable {0:?} doesn't exist")]
    LocalVarDoesntExist(Handle<crate::LocalVariable>),
    #[error("Loading of {0:?} can't be done")]
    InvalidPointerType(Handle<crate::Expression>),
    #[error("Array length of {0:?} can't be done")]
    InvalidArrayType(Handle<crate::Expression>),
    #[error("Compose type {0:?} doesn't exist")]
    ComposeTypeDoesntExist(Handle<crate::Type>),
    #[error("Composing of type {0:?} can't be done")]
    InvalidComposeType(Handle<crate::Type>),
    #[error("Composing expects {expected} components but {given} were given")]
    InvalidComposeCount { given: u32, expected: u32 },
    #[error("Composing {0}'s component {1:?} is not expected")]
    InvalidComponentType(u32, Handle<crate::Expression>),
    #[error("Operation {0:?} can't work with {1:?}")]
    InvalidUnaryOperandType(crate::UnaryOperator, Handle<crate::Expression>),
    #[error("Operation {0:?} can't work with {1:?} and {2:?}")]
    InvalidBinaryOperandTypes(
        crate::BinaryOperator,
        Handle<crate::Expression>,
        Handle<crate::Expression>,
    ),
    #[error("Selecting is not possible")]
    InvalidSelectTypes,
    #[error("Relational argument {0:?} is not a boolean vector")]
    InvalidBooleanVector(Handle<crate::Expression>),
    #[error("Relational argument {0:?} is not a float")]
    InvalidFloatArgument(Handle<crate::Expression>),
    #[error("Type resolution failed")]
    Type(#[from] ResolveError),
    #[error("Not a global variable")]
    ExpectedGlobalVariable,
    #[error("Calling an undeclared function {0:?}")]
    CallToUndeclaredFunction(Handle<crate::Function>),
    #[error("Needs to be an image instead of {0:?}")]
    ExpectedImageType(Handle<crate::Type>),
    #[error("Needs to be an image instead of {0:?}")]
    ExpectedSamplerType(Handle<crate::Type>),
    #[error("Unable to operate on image class {0:?}")]
    InvalidImageClass(crate::ImageClass),
}

struct ExpressionTypeResolver<'a> {
    root: Handle<crate::Expression>,
    types: &'a Arena<crate::Type>,
    info: &'a FunctionInfo,
}

impl<'a> ExpressionTypeResolver<'a> {
    fn resolve(
        &self,
        handle: Handle<crate::Expression>,
    ) -> Result<&'a crate::TypeInner, ExpressionError> {
        if handle < self.root {
            Ok(self.info[handle].ty.inner_with(self.types))
        } else {
            Err(ExpressionError::ForwardDependency(handle))
        }
    }
}

impl super::Validator {
    pub(super) fn validate_expression(
        &self,
        root: Handle<crate::Expression>,
        expression: &crate::Expression,
        function: &crate::Function,
        module: &crate::Module,
        info: &FunctionInfo,
        other_infos: &[FunctionInfo],
    ) -> Result<ShaderStages, ExpressionError> {
        use crate::{Expression as E, ScalarKind as Sk, TypeInner as Ti};

        let resolver = ExpressionTypeResolver {
            root,
            types: &module.types,
            info,
        };

        let stages = match *expression {
            E::Access { base, index } => {
                match *resolver.resolve(base)? {
                    Ti::Vector { .. }
                    | Ti::Matrix { .. }
                    | Ti::Array { .. }
                    | Ti::Pointer { .. } => {}
                    ref other => {
                        log::error!("Indexing of {:?}", other);
                        return Err(ExpressionError::InvalidBaseType(base));
                    }
                }
                match *resolver.resolve(index)? {
                    //TODO: only allow one of these
                    Ti::Scalar {
                        kind: Sk::Sint,
                        width: _,
                    }
                    | Ti::Scalar {
                        kind: Sk::Uint,
                        width: _,
                    } => {}
                    ref other => {
                        log::error!("Indexing by {:?}", other);
                        return Err(ExpressionError::InvalidIndexType(index));
                    }
                }
                ShaderStages::all()
            }
            E::AccessIndex { base, index } => {
                let limit = match *resolver.resolve(base)? {
                    Ti::Vector { size, .. } => size as u32,
                    Ti::Matrix { columns, .. } => columns as u32,
                    Ti::Array {
                        size: crate::ArraySize::Constant(handle),
                        ..
                    } => module.constants[handle].to_array_length().unwrap(),
                    Ti::Array { .. } => !0, // can't statically know, but need run-time checks
                    Ti::Pointer { .. } => !0, //TODO
                    Ti::Struct {
                        ref members,
                        block: _,
                    } => members.len() as u32,
                    ref other => {
                        log::error!("Indexing of {:?}", other);
                        return Err(ExpressionError::InvalidBaseType(base));
                    }
                };
                if index >= limit {
                    return Err(ExpressionError::IndexOutOfBounds(base, index));
                }
                ShaderStages::all()
            }
            E::Constant(handle) => {
                let _ = module
                    .constants
                    .try_get(handle)
                    .ok_or(ExpressionError::ConstantDoesntExist(handle))?;
                ShaderStages::all()
            }
            E::Compose { ref components, ty } => {
                match module
                    .types
                    .try_get(ty)
                    .ok_or(ExpressionError::ComposeTypeDoesntExist(ty))?
                    .inner
                {
                    // vectors are composed from scalars or other vectors
                    Ti::Vector { size, kind, width } => {
                        let mut total = 0;
                        for (index, &comp) in components.iter().enumerate() {
                            total += match *resolver.resolve(comp)? {
                                Ti::Scalar {
                                    kind: comp_kind,
                                    width: comp_width,
                                } if comp_kind == kind && comp_width == width => 1,
                                Ti::Vector {
                                    size: comp_size,
                                    kind: comp_kind,
                                    width: comp_width,
                                } if comp_kind == kind && comp_width == width => comp_size as u32,
                                ref other => {
                                    log::error!("Vector component[{}] type {:?}", index, other);
                                    return Err(ExpressionError::InvalidComponentType(
                                        index as u32,
                                        comp,
                                    ));
                                }
                            };
                        }
                        if size as u32 != total {
                            return Err(ExpressionError::InvalidComposeCount {
                                expected: size as u32,
                                given: total,
                            });
                        }
                    }
                    // matrix are composed from column vectors
                    Ti::Matrix {
                        columns,
                        rows,
                        width,
                    } => {
                        let inner = Ti::Vector {
                            size: rows,
                            kind: Sk::Float,
                            width,
                        };
                        if columns as usize != components.len() {
                            return Err(ExpressionError::InvalidComposeCount {
                                expected: columns as u32,
                                given: components.len() as u32,
                            });
                        }
                        for (index, &comp) in components.iter().enumerate() {
                            let tin = resolver.resolve(comp)?;
                            if tin != &inner {
                                log::error!("Matrix component[{}] type {:?}", index, tin);
                                return Err(ExpressionError::InvalidComponentType(
                                    index as u32,
                                    comp,
                                ));
                            }
                        }
                    }
                    Ti::Array {
                        base,
                        size: crate::ArraySize::Constant(handle),
                        stride: _,
                    } => {
                        let count = module.constants[handle].to_array_length().unwrap();
                        if count as usize != components.len() {
                            return Err(ExpressionError::InvalidComposeCount {
                                expected: count,
                                given: components.len() as u32,
                            });
                        }
                        let base_inner = &module.types[base].inner;
                        for (index, &comp) in components.iter().enumerate() {
                            let tin = resolver.resolve(comp)?;
                            if tin != base_inner {
                                log::error!("Array component[{}] type {:?}", index, tin);
                                return Err(ExpressionError::InvalidComponentType(
                                    index as u32,
                                    comp,
                                ));
                            }
                        }
                    }
                    Ti::Struct {
                        block: _,
                        ref members,
                    } => {
                        for (index, (member, &comp)) in members.iter().zip(components).enumerate() {
                            let tin = resolver.resolve(comp)?;
                            if tin != &module.types[member.ty].inner {
                                log::error!("Struct component[{}] type {:?}", index, tin);
                                return Err(ExpressionError::InvalidComponentType(
                                    index as u32,
                                    comp,
                                ));
                            }
                        }
                        if members.len() != components.len() {
                            return Err(ExpressionError::InvalidComposeCount {
                                given: components.len() as u32,
                                expected: members.len() as u32,
                            });
                        }
                    }
                    ref other => {
                        log::error!("Composing of {:?}", other);
                        return Err(ExpressionError::InvalidComposeType(ty));
                    }
                }
                ShaderStages::all()
            }
            E::FunctionArgument(index) => {
                if index >= function.arguments.len() as u32 {
                    return Err(ExpressionError::FunctionArgumentDoesntExist(index));
                }
                ShaderStages::all()
            }
            E::GlobalVariable(handle) => {
                let _ = module
                    .global_variables
                    .try_get(handle)
                    .ok_or(ExpressionError::GlobalVarDoesntExist(handle))?;
                ShaderStages::all()
            }
            E::LocalVariable(handle) => {
                let _ = function
                    .local_variables
                    .try_get(handle)
                    .ok_or(ExpressionError::LocalVarDoesntExist(handle))?;
                ShaderStages::all()
            }
            E::Load { pointer } => {
                match *resolver.resolve(pointer)? {
                    Ti::Pointer { base, .. }
                        if self.types[base.index()]
                            .flags
                            .contains(TypeFlags::SIZED | TypeFlags::DATA) => {}
                    Ti::ValuePointer { .. } => {}
                    ref other => {
                        log::error!("Loading {:?}", other);
                        return Err(ExpressionError::InvalidPointerType(pointer));
                    }
                }
                ShaderStages::all()
            }
            #[allow(unused)]
            E::ImageSample {
                image,
                sampler,
                coordinate,
                array_index,
                offset,
                level,
                depth_ref,
            } => ShaderStages::all(),
            #[allow(unused)]
            E::ImageLoad {
                image,
                coordinate,
                array_index,
                index,
            } => ShaderStages::all(),
            E::ImageQuery { image, query } => {
                match function.expressions[image] {
                    crate::Expression::GlobalVariable(var_handle) => {
                        let var = &module.global_variables[var_handle];
                        match module.types[var.ty].inner {
                            Ti::Image { class, arrayed, .. } => {
                                let can_level = match class {
                                    crate::ImageClass::Sampled { multi, .. } => !multi,
                                    crate::ImageClass::Storage { .. } => false,
                                    crate::ImageClass::Depth { .. } => true,
                                };
                                let good = match query {
                                    crate::ImageQuery::NumLayers => arrayed,
                                    crate::ImageQuery::Size { level: Some(_) }
                                    | crate::ImageQuery::NumLevels => can_level,
                                    crate::ImageQuery::Size { level: None }
                                    | crate::ImageQuery::NumSamples => !can_level,
                                };
                                if !good {
                                    return Err(ExpressionError::InvalidImageClass(class));
                                }
                            }
                            _ => return Err(ExpressionError::ExpectedImageType(var.ty)),
                        }
                    }
                    _ => return Err(ExpressionError::ExpectedGlobalVariable),
                }
                ShaderStages::all()
            }
            E::Unary { op, expr } => {
                use crate::UnaryOperator as Uo;
                let inner = resolver.resolve(expr)?;
                match (op, inner.scalar_kind()) {
                    (_, Some(Sk::Sint))
                    | (_, Some(Sk::Bool))
                    | (Uo::Negate, Some(Sk::Float))
                    | (Uo::Not, Some(Sk::Uint)) => {}
                    other => {
                        log::error!("Op {:?} kind {:?}", op, other);
                        return Err(ExpressionError::InvalidUnaryOperandType(op, expr));
                    }
                }
                ShaderStages::all()
            }
            E::Binary { op, left, right } => {
                use crate::BinaryOperator as Bo;
                let left_inner = resolver.resolve(left)?;
                let right_inner = resolver.resolve(right)?;
                let good = match op {
                    Bo::Add | Bo::Subtract | Bo::Divide | Bo::Modulo => match *left_inner {
                        Ti::Scalar { kind, .. } | Ti::Vector { kind, .. } => match kind {
                            Sk::Uint | Sk::Sint | Sk::Float => left_inner == right_inner,
                            Sk::Bool => false,
                        },
                        _ => false,
                    },
                    Bo::Multiply => {
                        let kind_match = match left_inner.scalar_kind() {
                            Some(Sk::Uint) | Some(Sk::Sint) | Some(Sk::Float) => true,
                            Some(Sk::Bool) | None => false,
                        };
                        //TODO: should we be more restrictive here? I.e. expect scalar only to the left.
                        let types_match = match (left_inner, right_inner) {
                            (&Ti::Scalar { kind: kind1, .. }, &Ti::Scalar { kind: kind2, .. })
                            | (&Ti::Vector { kind: kind1, .. }, &Ti::Scalar { kind: kind2, .. })
                            | (&Ti::Scalar { kind: kind1, .. }, &Ti::Vector { kind: kind2, .. }) => {
                                kind1 == kind2
                            }
                            (
                                &Ti::Scalar {
                                    kind: Sk::Float, ..
                                },
                                &Ti::Matrix { .. },
                            )
                            | (
                                &Ti::Matrix { .. },
                                &Ti::Scalar {
                                    kind: Sk::Float, ..
                                },
                            ) => true,
                            (
                                &Ti::Vector {
                                    kind: kind1,
                                    size: size1,
                                    ..
                                },
                                &Ti::Vector {
                                    kind: kind2,
                                    size: size2,
                                    ..
                                },
                            ) => kind1 == kind2 && size1 == size2,
                            (
                                &Ti::Matrix { columns, .. },
                                &Ti::Vector {
                                    kind: Sk::Float,
                                    size,
                                    ..
                                },
                            ) => columns == size,
                            (
                                &Ti::Vector {
                                    kind: Sk::Float,
                                    size,
                                    ..
                                },
                                &Ti::Matrix { rows, .. },
                            ) => size == rows,
                            (&Ti::Matrix { columns, .. }, &Ti::Matrix { rows, .. }) => {
                                columns == rows
                            }
                            _ => false,
                        };
                        let left_width = match *left_inner {
                            Ti::Scalar { width, .. }
                            | Ti::Vector { width, .. }
                            | Ti::Matrix { width, .. } => width,
                            _ => 0,
                        };
                        let right_width = match *right_inner {
                            Ti::Scalar { width, .. }
                            | Ti::Vector { width, .. }
                            | Ti::Matrix { width, .. } => width,
                            _ => 0,
                        };
                        kind_match && types_match && left_width == right_width
                    }
                    Bo::Equal | Bo::NotEqual => left_inner.is_sized() && left_inner == right_inner,
                    Bo::Less | Bo::LessEqual | Bo::Greater | Bo::GreaterEqual => {
                        match *left_inner {
                            Ti::Scalar { kind, .. } | Ti::Vector { kind, .. } => match kind {
                                Sk::Uint | Sk::Sint | Sk::Float => left_inner == right_inner,
                                Sk::Bool => false,
                            },
                            ref other => {
                                log::error!("Op {:?} left type {:?}", op, other);
                                false
                            }
                        }
                    }
                    Bo::LogicalAnd | Bo::LogicalOr => match *left_inner {
                        Ti::Scalar { kind: Sk::Bool, .. } | Ti::Vector { kind: Sk::Bool, .. } => {
                            left_inner == right_inner
                        }
                        ref other => {
                            log::error!("Op {:?} left type {:?}", op, other);
                            false
                        }
                    },
                    Bo::And | Bo::ExclusiveOr | Bo::InclusiveOr => match *left_inner {
                        Ti::Scalar { kind, .. } | Ti::Vector { kind, .. } => match kind {
                            Sk::Sint | Sk::Uint => left_inner == right_inner,
                            Sk::Bool | Sk::Float => false,
                        },
                        ref other => {
                            log::error!("Op {:?} left type {:?}", op, other);
                            false
                        }
                    },
                    Bo::ShiftLeft | Bo::ShiftRight => {
                        let (base_size, base_kind) = match *left_inner {
                            Ti::Scalar { kind, .. } => (Ok(None), kind),
                            Ti::Vector { size, kind, .. } => (Ok(Some(size)), kind),
                            ref other => {
                                log::error!("Op {:?} base type {:?}", op, other);
                                (Err(()), Sk::Bool)
                            }
                        };
                        let shift_size = match *right_inner {
                            Ti::Scalar { kind: Sk::Uint, .. } => Ok(None),
                            Ti::Vector {
                                size,
                                kind: Sk::Uint,
                                ..
                            } => Ok(Some(size)),
                            ref other => {
                                log::error!("Op {:?} shift type {:?}", op, other);
                                Err(())
                            }
                        };
                        match base_kind {
                            Sk::Sint | Sk::Uint => base_size.is_ok() && base_size == shift_size,
                            Sk::Float | Sk::Bool => false,
                        }
                    }
                };
                if !good {
                    return Err(ExpressionError::InvalidBinaryOperandTypes(op, left, right));
                }
                ShaderStages::all()
            }
            E::Select {
                condition,
                accept,
                reject,
            } => {
                let accept_inner = resolver.resolve(accept)?;
                let reject_inner = resolver.resolve(reject)?;
                let condition_good = match *resolver.resolve(condition)? {
                    Ti::Scalar {
                        kind: Sk::Bool,
                        width: _,
                    } => accept_inner.is_sized(),
                    Ti::Vector {
                        size,
                        kind: Sk::Bool,
                        width: _,
                    } => match *accept_inner {
                        Ti::Vector {
                            size: other_size, ..
                        } => size == other_size,
                        _ => false,
                    },
                    _ => false,
                };
                if !condition_good || accept_inner != reject_inner {
                    return Err(ExpressionError::InvalidSelectTypes);
                }
                ShaderStages::all()
            }
            #[allow(unused)]
            E::Derivative { axis, expr } => ShaderStages::FRAGMENT,
            E::Relational { fun, argument } => {
                use crate::RelationalFunction as Rf;
                let argument_inner = resolver.resolve(argument)?;
                match fun {
                    Rf::All | Rf::Any => match *argument_inner {
                        Ti::Vector { kind: Sk::Bool, .. } => {}
                        ref other => {
                            log::error!("All/Any of type {:?}", other);
                            return Err(ExpressionError::InvalidBooleanVector(argument));
                        }
                    },
                    Rf::IsNan | Rf::IsInf | Rf::IsFinite | Rf::IsNormal => match *argument_inner {
                        Ti::Scalar {
                            kind: Sk::Float, ..
                        }
                        | Ti::Vector {
                            kind: Sk::Float, ..
                        } => {}
                        ref other => {
                            log::error!("Float test of type {:?}", other);
                            return Err(ExpressionError::InvalidFloatArgument(argument));
                        }
                    },
                }
                ShaderStages::all()
            }
            #[allow(unused)]
            E::Math {
                fun,
                arg,
                arg1,
                arg2,
            } => ShaderStages::all(),
            #[allow(unused)]
            E::As {
                expr,
                kind,
                convert,
            } => ShaderStages::all(),
            E::Call(function) => other_infos[function.index()].available_stages,
            E::ArrayLength(expr) => match *resolver.resolve(expr)? {
                Ti::Array { .. } => ShaderStages::all(),
                ref other => {
                    log::error!("Array length of {:?}", other);
                    return Err(ExpressionError::InvalidArrayType(expr));
                }
            },
        };
        Ok(stages)
    }
}
