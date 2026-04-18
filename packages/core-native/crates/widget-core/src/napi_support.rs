use napi::bindgen_prelude::{FromNapiValue, ToNapiValue, TypeName, ValidateNapiValue};
use napi::{ValueType, sys};

use crate::runtime::{FontWeight, NonNegativeF64};

impl TypeName for NonNegativeF64 {
    fn type_name() -> &'static str {
        "number"
    }

    fn value_type() -> ValueType {
        ValueType::Number
    }
}

impl ValidateNapiValue for NonNegativeF64 {}

impl FromNapiValue for NonNegativeF64 {
    unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
        let value = unsafe { <f64 as FromNapiValue>::from_napi_value(env, napi_val)? };
        NonNegativeF64::new(value).map_err(|error| napi::Error::from_reason(error.to_string()))
    }
}

impl ToNapiValue for NonNegativeF64 {
    unsafe fn to_napi_value(env: sys::napi_env, val: Self) -> napi::Result<sys::napi_value> {
        unsafe { <f64 as ToNapiValue>::to_napi_value(env, val.get()) }
    }
}

impl TypeName for FontWeight {
    fn type_name() -> &'static str {
        "number"
    }

    fn value_type() -> ValueType {
        ValueType::Number
    }
}

impl ValidateNapiValue for FontWeight {}

impl FromNapiValue for FontWeight {
    unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
        let value = unsafe { <u32 as FromNapiValue>::from_napi_value(env, napi_val)? };
        FontWeight::new(value).map_err(|error| napi::Error::from_reason(error.to_string()))
    }
}

impl ToNapiValue for FontWeight {
    unsafe fn to_napi_value(env: sys::napi_env, val: Self) -> napi::Result<sys::napi_value> {
        unsafe { <u32 as ToNapiValue>::to_napi_value(env, u32::from(val.get())) }
    }
}
