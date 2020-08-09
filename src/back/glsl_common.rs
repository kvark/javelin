use crate::{
    proc::ResolveError, Arena, ArraySize, BinaryOperator, Constant, ConstantInner, DerivativeAxis,
    Expression, FastHashMap, Function, FunctionOrigin, GlobalVariable, Handle, ImageDimension,
    ImageFlags, LocalVariable, Module, ScalarKind, Statement, StorageClass, Type, TypeInner,
    UnaryOperator,
};
use std::{
    fmt::{self, Error as FmtError, Write},
    io::Error as IoError,
};

#[derive(Debug)]
pub enum Error {
    FormatError(FmtError),
    IoError(IoError),
    ResolveError(ResolveError),
    Custom(String),
}

impl From<FmtError> for Error {
    fn from(err: FmtError) -> Self {
        Error::FormatError(err)
    }
}

impl From<IoError> for Error {
    fn from(err: IoError) -> Self {
        Error::IoError(err)
    }
}

impl From<ResolveError> for Error {
    fn from(err: ResolveError) -> Self {
        Error::ResolveError(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::FormatError(err) => write!(f, "Formatting error {}", err),
            Error::IoError(err) => write!(f, "Io error: {}", err),
            Error::ResolveError(err) => write!(f, "Resolve error: {}", err),
            Error::Custom(err) => write!(f, "{}", err),
        }
    }
}

pub(crate) struct StatementBuilder<'a> {
    pub functions: &'a FastHashMap<Handle<Function>, String>,
    pub globals: &'a FastHashMap<Handle<GlobalVariable>, String>,
    pub locals_lookup: &'a FastHashMap<Handle<LocalVariable>, String>,
    pub structs: &'a FastHashMap<Handle<Type>, (String, Vec<String>)>,
    pub args: &'a FastHashMap<u32, String>,
    pub expressions: &'a Arena<Expression>,
    pub types: &'a mut Arena<Type>,
    pub locals: &'a Arena<LocalVariable>,
}

impl Statement {
    pub(crate) fn write_glsl(
        &self,
        module: &Module,
        builder: &mut StatementBuilder<'_>,
    ) -> Result<String, Error> {
        Ok(match self {
            Statement::Empty => String::new(),
            Statement::Block(block) => block
                .iter()
                .map(|sta| sta.write_glsl(module, builder))
                .collect::<Result<Vec<_>, _>>()?
                .join("\n"),
            Statement::If {
                condition,
                accept,
                reject,
            } => {
                let mut out = String::new();

                writeln!(
                    &mut out,
                    "if({}) {{",
                    builder.expressions[*condition].write_glsl(module, builder)?
                )?;
                for sta in accept {
                    writeln!(&mut out, "{}", sta.write_glsl(module, builder)?)?;
                }
                writeln!(&mut out, "}} else {{")?;
                for sta in reject {
                    writeln!(&mut out, "{}", sta.write_glsl(module, builder)?)?;
                }
                write!(&mut out, "}}")?;

                out
            }
            Statement::Switch {
                selector,
                cases,
                default,
            } => {
                let mut out = String::new();

                writeln!(
                    &mut out,
                    "switch({}) {{",
                    builder.expressions[*selector].write_glsl(module, builder)?
                )?;

                for (label, (block, fallthrough)) in cases {
                    writeln!(&mut out, "   case {}:", label)?;

                    for sta in block {
                        writeln!(&mut out, "      {}", sta.write_glsl(module, builder)?)?;
                    }

                    if fallthrough.is_some() {
                        writeln!(&mut out, "      break;")?;
                    }
                }

                writeln!(&mut out, "   default:")?;

                for sta in default {
                    writeln!(&mut out, "      {}", sta.write_glsl(module, builder)?)?;
                }

                write!(&mut out, "}}")?;

                out
            }
            Statement::Loop { body, continuing } => {
                let mut out = String::new();

                writeln!(&mut out, "while(true) {{",)?;

                for sta in body.iter().chain(continuing.iter()) {
                    writeln!(&mut out, "    {}", sta.write_glsl(module, builder)?)?;
                }

                write!(&mut out, "}}")?;

                out
            }
            Statement::Break => String::from("break;"),
            Statement::Continue => String::from("continue;"),
            Statement::Return { value } => format!(
                "return  {};",
                value.map_or(Ok(String::from("")), |expr| builder.expressions[expr]
                    .write_glsl(module, builder))?
            ),
            Statement::Kill => String::from("discard;"),
            Statement::Store { pointer, value } => format!(
                "{} = {};",
                builder.expressions[*pointer].write_glsl(module, builder)?,
                builder.expressions[*value].write_glsl(module, builder)?
            ),
        })
    }
}

impl Expression {
    pub(crate) fn write_glsl(
        &self,
        module: &Module,
        builder: &mut StatementBuilder<'_>,
    ) -> Result<String, Error> {
        Ok(match self {
            Expression::Access { base, index } => format!(
                "{}[{}]",
                builder.expressions[*base].write_glsl(module, builder)?,
                builder.expressions[*index].write_glsl(module, builder)?
            ),
            Expression::AccessIndex { base, index } => {
                let handle = crate::proc::Typifier::new().resolve(
                    *base,
                    builder.expressions,
                    builder.types,
                    &module.constants,
                    &module.global_variables,
                    builder.locals,
                    &module.functions,
                )?;

                match builder.types[handle].inner {
                    TypeInner::Vector { .. }
                    | TypeInner::Matrix { .. }
                    | TypeInner::Array { .. } => format!(
                        "{}[{}]",
                        builder.expressions[*base].write_glsl(module, builder)?,
                        index
                    ),
                    TypeInner::Struct { .. } => format!(
                        "{}.{}",
                        builder.expressions[*base].write_glsl(module, builder)?,
                        builder.structs.get(&handle).unwrap().1[*index as usize]
                    ),
                    _ => {
                        return Err(Error::Custom(format!(
                            "Cannot index {}",
                            handle.write_glsl(builder.types, builder.structs)?
                        )))
                    }
                }
            }
            Expression::Constant(constant) => {
                module.constants[*constant].write_glsl(module).to_string()
            }
            Expression::Compose { ty, components } => format!(
                "{}({})",
                match module.types[*ty].inner {
                    TypeInner::Scalar { kind, width } => String::from(match kind {
                        ScalarKind::Sint => "int",
                        ScalarKind::Uint => "uint",
                        ScalarKind::Float => match width {
                            4 => "float",
                            8 => "double",
                            _ =>
                                return Err(Error::Custom(format!(
                                    "Cannot build float of width {}",
                                    width
                                ))),
                        },
                        ScalarKind::Bool => "bool",
                    }),
                    TypeInner::Vector { size, kind, width } => format!(
                        "{}vec{}",
                        match kind {
                            ScalarKind::Sint => "i",
                            ScalarKind::Uint => "u",
                            ScalarKind::Float => match width {
                                4 => "",
                                8 => "d",
                                _ =>
                                    return Err(Error::Custom(format!(
                                        "Cannot build float of width {}",
                                        width
                                    ))),
                            },
                            ScalarKind::Bool => "b",
                        },
                        size as u8,
                    ),
                    TypeInner::Matrix {
                        columns,
                        rows,
                        kind,
                        width,
                    } => format!(
                        "{}mat{}x{}",
                        match kind {
                            ScalarKind::Sint => "i",
                            ScalarKind::Uint => "u",
                            ScalarKind::Float => match width {
                                4 => "",
                                8 => "d",
                                _ =>
                                    return Err(Error::Custom(format!(
                                        "Cannot build float of width {}",
                                        width
                                    ))),
                            },
                            ScalarKind::Bool => "b",
                        },
                        columns as u8,
                        rows as u8,
                    ),
                    TypeInner::Array { .. } => ty.write_glsl(builder.types, builder.structs)?,
                    TypeInner::Struct { .. } => builder.structs.get(ty).unwrap().clone().0,
                    _ =>
                        return Err(Error::Custom(format!(
                            "Cannot compose type {}",
                            ty.write_glsl(builder.types, builder.structs)?
                        ))),
                },
                components
                    .iter()
                    .map(|arg| builder.expressions[*arg].write_glsl(module, builder))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(","),
            ),
            Expression::FunctionParameter(pos) => builder.args.get(&pos).unwrap().to_string(),
            Expression::GlobalVariable(handle) => builder.globals.get(&handle).unwrap().clone(),
            Expression::LocalVariable(handle) => {
                builder.locals_lookup.get(&handle).unwrap().clone()
            }
            Expression::Load { pointer } => todo!(),
            Expression::ImageSample {
                image,
                sampler,
                coordinate,
                depth_ref,
            } => todo!(),
            Expression::Unary { op, expr } => format!(
                "({} {})",
                match op {
                    UnaryOperator::Negate => "-",
                    UnaryOperator::Not => "~",
                },
                builder.expressions[*expr].write_glsl(module, builder)?
            ),
            Expression::Binary { op, left, right } => format!(
                "({} {} {})",
                builder.expressions[*left].write_glsl(module, builder)?,
                match op {
                    BinaryOperator::Add => "+",
                    BinaryOperator::Subtract => "-",
                    BinaryOperator::Multiply => "*",
                    BinaryOperator::Divide => "/",
                    BinaryOperator::Modulo => "%",
                    BinaryOperator::Equal => "==",
                    BinaryOperator::NotEqual => "!=",
                    BinaryOperator::Less => "<",
                    BinaryOperator::LessEqual => "<=",
                    BinaryOperator::Greater => ">",
                    BinaryOperator::GreaterEqual => ">=",
                    BinaryOperator::And => "&",
                    BinaryOperator::ExclusiveOr => "^",
                    BinaryOperator::InclusiveOr => "|",
                    BinaryOperator::LogicalAnd => "&&",
                    BinaryOperator::LogicalOr => "||",
                    BinaryOperator::ShiftLeftLogical => "<<",
                    BinaryOperator::ShiftRightLogical => todo!(),
                    BinaryOperator::ShiftRightArithmetic => ">>",
                },
                builder.expressions[*right].write_glsl(module, builder)?
            ),
            Expression::Intrinsic { fun, argument } => todo!(),
            Expression::DotProduct(left, right) => format!(
                "dot({},{})",
                builder.expressions[*left].write_glsl(module, builder)?,
                builder.expressions[*right].write_glsl(module, builder)?
            ),
            Expression::CrossProduct(left, right) => format!(
                "cross({},{})",
                builder.expressions[*left].write_glsl(module, builder)?,
                builder.expressions[*right].write_glsl(module, builder)?
            ),
            Expression::Derivative { axis, expr } => format!(
                "{}({})",
                match axis {
                    DerivativeAxis::X => "dFdx",
                    DerivativeAxis::Y => "dFdy",
                    _ => todo!(),
                },
                builder.expressions[*expr].write_glsl(module, builder)?
            ),
            Expression::Call { origin, arguments } => format!(
                "{}({})",
                match origin {
                    FunctionOrigin::External(name) => name,
                    FunctionOrigin::Local(handle) => builder.functions.get(&handle).unwrap(),
                },
                arguments
                    .iter()
                    .map(|arg| builder.expressions[*arg].write_glsl(module, builder))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(","),
            ),
        })
    }
}

pub(crate) struct ConstantWriter<'a> {
    inner: &'a Constant,
    module: &'a Module,
}

impl Constant {
    pub(crate) fn write_glsl<'a>(&'a self, module: &'a Module) -> ConstantWriter<'a> {
        ConstantWriter {
            inner: self,
            module,
        }
    }
}

impl<'a> fmt::Display for ConstantWriter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner.inner {
            ConstantInner::Sint(int) => write!(f, "{}", int),
            ConstantInner::Uint(int) => write!(f, "{}", int),
            ConstantInner::Float(float) => write!(f, "{}", float),
            ConstantInner::Bool(boolean) => write!(f, "{}", boolean),
            ConstantInner::Composite(components) => match self.module.types[self.inner.ty].inner {
                _ => todo!(),
            },
        }
    }
}

pub(crate) struct StorageClassWriter<'a> {
    inner: &'a StorageClass,
}

impl StorageClass {
    pub(crate) fn write_glsl<'a>(&'a self) -> StorageClassWriter<'a> {
        StorageClassWriter { inner: self }
    }
}

impl<'a> fmt::Display for StorageClassWriter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self.inner {
                StorageClass::Constant => "const ",
                StorageClass::Function => todo!(),
                StorageClass::Input => "in ",
                StorageClass::Output => "out ",
                StorageClass::Private => "",
                StorageClass::StorageBuffer => "buffer ",
                StorageClass::Uniform => "uniform ",
                StorageClass::WorkGroup => "shared ",
            }
        )
    }
}

impl Handle<Type> {
    pub(crate) fn write_glsl<'a>(
        &self,
        types: &'a Arena<Type>,
        structs: &'a FastHashMap<Handle<Type>, (String, Vec<String>)>,
    ) -> Result<String, Error> {
        Ok(match &types[*self].inner {
            TypeInner::Scalar { kind, width } => match kind {
                ScalarKind::Sint => String::from("int"),
                ScalarKind::Uint => String::from("uint"),
                ScalarKind::Float => match width {
                    4 => String::from("float"),
                    8 => String::from("double"),
                    _ => {
                        return Err(Error::Custom(format!(
                            "Cannot build float of width {}",
                            width
                        )))
                    }
                },
                ScalarKind::Bool => String::from("bool"),
            },
            TypeInner::Vector { size, kind, width } => format!(
                "{}vec{}",
                match kind {
                    ScalarKind::Sint => "i",
                    ScalarKind::Uint => "u",
                    ScalarKind::Float => match width {
                        4 => "",
                        8 => "d",
                        _ =>
                            return Err(Error::Custom(format!(
                                "Cannot build float of width {}",
                                width
                            ))),
                    },
                    ScalarKind::Bool => "b",
                },
                *size as u8
            ),
            TypeInner::Matrix {
                columns,
                rows,
                kind,
                width,
            } => format!(
                "{}mat{}x{}",
                match kind {
                    ScalarKind::Sint => "i",
                    ScalarKind::Uint => "u",
                    ScalarKind::Float => match width {
                        4 => "",
                        8 => "d",
                        _ =>
                            return Err(Error::Custom(format!(
                                "Cannot build float of width {}",
                                width
                            ))),
                    },
                    ScalarKind::Bool => "b",
                },
                *columns as u8,
                *rows as u8
            ),
            TypeInner::Pointer { base, class } => todo!(),
            TypeInner::Array { base, size, stride } => format!(
                "{}[{}]",
                base.write_glsl(types, structs)?,
                size.write_glsl()
            ),
            TypeInner::Struct { .. } => structs.get(self).unwrap().0.clone(),
            TypeInner::Image { base, dim, flags } => format!(
                "{}image{}{}",
                match types[*base].inner {
                    TypeInner::Scalar { kind, .. } => match kind {
                        ScalarKind::Sint => "i",
                        ScalarKind::Uint => "u",
                        ScalarKind::Float => "",
                        _ => return Err(Error::Custom(format!("Cannot build image of booleans",))),
                    },
                    _ =>
                        return Err(Error::Custom(format!(
                            "Cannot build image of type {}",
                            base.write_glsl(types, structs)?
                        ))),
                },
                dim.write_glsl(),
                flags.write_glsl()
            ),
            TypeInner::DepthImage { dim, arrayed } => format!(
                "image{}{}",
                dim.write_glsl(),
                if *arrayed { "Array" } else { "" }
            ),
            TypeInner::Sampler { comparison } => String::from(if *comparison {
                "sampler"
            } else {
                "samplerShadow"
            }),
        })
    }
}

pub(crate) struct ArraySizeWriter<'a> {
    inner: &'a ArraySize,
}

impl ArraySize {
    pub(crate) fn write_glsl<'a>(&'a self) -> ArraySizeWriter<'a> {
        ArraySizeWriter { inner: self }
    }
}

impl<'a> fmt::Display for ArraySizeWriter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner {
            ArraySize::Static(size) => write!(f, "{}", size),
            ArraySize::Dynamic => Ok(()),
        }
    }
}

pub(crate) struct DimWriter<'a> {
    inner: &'a ImageDimension,
}

impl ImageDimension {
    pub(crate) fn write_glsl<'a>(&'a self) -> DimWriter<'a> {
        DimWriter { inner: self }
    }
}

impl<'a> fmt::Display for DimWriter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self.inner {
                ImageDimension::D1 => "1D",
                ImageDimension::D2 => "2D",
                ImageDimension::D3 => "3D",
                ImageDimension::Cube => "Cube",
            }
        )
    }
}

pub(crate) struct ImageFlagsWriter<'a> {
    inner: &'a ImageFlags,
}

impl ImageFlags {
    pub(crate) fn write_glsl<'a>(&'a self) -> ImageFlagsWriter<'a> {
        ImageFlagsWriter { inner: self }
    }
}

impl<'a> fmt::Display for ImageFlagsWriter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.inner.contains(ImageFlags::MULTISAMPLED) {
            write!(f, "MS")?;
        }

        if self.inner.contains(ImageFlags::ARRAYED) {
            write!(f, "Array")?;
        }

        Ok(())
    }
}
