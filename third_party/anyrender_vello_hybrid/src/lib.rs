//! A [`vello_hybrid`] backend for the [`anyrender`] 2D drawing abstraction
#![cfg_attr(docsrs, feature(doc_cfg))]

mod scene;
#[cfg(all(target_arch = "wasm32", feature = "webgl"))]
mod webgl_scene;
mod window_renderer;

pub use scene::ImageManager;
pub use scene::VelloHybridScenePainter;
pub use scene::record_anyrender_scene;
pub use vello_common::recording::Recording;
#[cfg(all(target_arch = "wasm32", feature = "webgl"))]
pub use webgl_scene::*;
pub use window_renderer::*;
