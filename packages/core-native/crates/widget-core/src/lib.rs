#[doc(hidden)]
pub mod codegen;
pub mod decl;
#[cfg(feature = "napi")]
mod napi_support;
pub mod runtime;
pub mod schema;
pub mod vello;

pub use runtime::{IntoQt, Paint, PaintDevice, TryFromQt};
pub use vello::{PaintSceneFrame, VelloFrame};

#[macro_export]
macro_rules! cpp {
    ($($tt:tt)*) => {
        compile_error!("qt::cpp! may only be used as the full body of a #[qt(host)] method");
    };
}

pub mod qt {
    pub use crate::cpp;
}
