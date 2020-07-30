use crate::value::*;
use crate::{runtime::VM, RantError, RantResult};
use std::rc::Rc;

/// Enables conversion from a native type to a `RantValue`.
pub trait ToRant {
    fn to_rant(self) -> RantValue;
}

/// Enables conversion from a `RantValue` to a native type.
pub trait FromRant: Sized {
    fn from_rant(val: RantValue) -> RantResult<Self>;
}

/// Enables conversion from a set of Rant arguments to equivalent supported native types.
pub trait FromRantArgs: Sized {
    fn from_args(&self, args: Vec<RantValue>) -> RantResult<Self>;
}

/// Converts a Vec<RantValue> to VarArgs<T>
impl<'a, T: FromRant> FromRantArgs for VarArgs<T> {
    fn from_args(&self, mut args: Vec<RantValue>) -> RantResult<Self> {
        let vec = args
            .drain(..)
            .map(T::from_rant)
            .collect::<RantResult<Vec<T>>>()?;
        Ok(VarArgs::new(vec))
    }
}

macro_rules! rant_int_conversions {
    ($int_type: ty) => {
        impl ToRant for $int_type {
            fn to_rant(self) -> RantValue {
                RantValue::Integer(self as i64)
            }
        }
        impl FromRant for $int_type {
            fn from_rant(val: RantValue) -> RantResult<Self> {
                if let RantValue::Integer(i) = val {
                    return Ok(i as Self)
                }

                let src_type = val.type_name();
                let dest_type = stringify!{$int_type};
                Err(RantError::ValueConversionError {
                    from: src_type,
                    to: dest_type,
                    message: Some(format!("Rant type '{}' cannot be converted to native type '{}'.", src_type, dest_type))
                })
            }
        }
    };
    ($int_type: ty, $($int_type2: ty), +) => {
        rant_int_conversions! { $int_type }
        rant_int_conversions! { $($int_type2), + }
    };
}

rant_int_conversions! { u8, i8, u16, i16, u32, i32, u64, i64, isize, usize }

pub trait RantForeignFunc {
    fn as_rant_func(&'static self) -> RantFunction;
}

macro_rules! impl_rant_foreign_func_fn {
    ($($generic_types:ident),*) => {
        // Non-variadic implementation
        impl<
            $($generic_types: FromRant,)*
        > RantForeignFunc for dyn FnMut(&mut VM, $($generic_types,)*) -> RantResult<()> {
            fn as_rant_func(&'static self) -> RantFunction {
                RantFunction::Foreign(Rc::new(move |vm, args| {
                    let mut args = args.into_iter();
                    self(vm, $($generic_types::from_rant(args.next().unwrap_or(RantValue::None))?,)*)
                }))
            }
        }

        // Variadic implementation
        impl<
            $($generic_types: FromRant,)*
            Variadic: FromRant
        > RantForeignFunc for dyn FnMut(&mut VM, $($generic_types,)* VarArgs<Variadic>) -> RantResult<()> {
            fn as_rant_func(&'static self) -> RantFunction {
                RantFunction::Foreign(Rc::new(move |vm, mut args| {
                    let mut args = args.drain(..);
                    self(vm, 
                        $($generic_types::from_rant(args.next().unwrap_or(RantValue::None))?,)*
                        VarArgs::new(args
                            .map(Variadic::from_rant)
                            .collect::<RantResult<Vec<Variadic>>>()?
                        ))
                }))
            }
        }
    }
}

impl_rant_foreign_func_fn!();
impl_rant_foreign_func_fn!(A);
impl_rant_foreign_func_fn!(A, B);
impl_rant_foreign_func_fn!(A, B, C);
impl_rant_foreign_func_fn!(A, B, C, D);
impl_rant_foreign_func_fn!(A, B, C, D, E);
impl_rant_foreign_func_fn!(A, B, C, D, E, F);
impl_rant_foreign_func_fn!(A, B, C, D, E, F, G);
impl_rant_foreign_func_fn!(A, B, C, D, E, F, G, H);
impl_rant_foreign_func_fn!(A, B, C, D, E, F, G, H, I);
impl_rant_foreign_func_fn!(A, B, C, D, E, F, G, H, I, J);
impl_rant_foreign_func_fn!(A, B, C, D, E, F, G, H, I, J, K);
impl_rant_foreign_func_fn!(A, B, C, D, E, F, G, H, I, J, K, L);
