use qt_wgpu_renderer::{
    QtNativeTextureLease, QtRhiInteropInfo, TextureCacheKey, load_or_create_context,
};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene, peniko::Color};

use crate::runtime::qt_error;

pub(crate) fn render_vello_scene_to_native_texture(
    node_id: u32,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
    rhi_interop: QtRhiInteropInfo,
) -> napi::Result<QtNativeTextureLease> {
    let context_handle =
        load_or_create_context(rhi_interop).map_err(|error| qt_error(error.to_string()))?;
    let mut context = context_handle
        .lock()
        .expect("qt-wgpu interop context mutex poisoned");
    let device = context.device().clone();
    let queue = context.queue().clone();
    let mut scaled_scene = Scene::new();
    scaled_scene.append(scene, Some(vello::kurbo::Affine::scale(scale_factor)));
    let key = TextureCacheKey::rgba8_storage(u64::from(node_id), width_px, height_px);
    let (texture_view, texture_lease) = match rhi_interop {
        QtRhiInteropInfo::Metal(_) => {
            let entry = qt_wgpu_renderer::metal::cached_rgba8_texture_entry(
                &mut context,
                key,
                "qt-solid-vello-render-target",
            )
            .map_err(|error| qt_error(error.to_string()))?;
            (entry.texture_view().clone(), entry.texture_lease().clone())
        }
        QtRhiInteropInfo::Vulkan(_) => {
            let entry = qt_wgpu_renderer::vk::cached_rgba8_texture_entry(
                &mut context,
                key,
                "qt-solid-vello-render-target",
            )
            .map_err(|error| qt_error(error.to_string()))?;
            (entry.texture_view().clone(), entry.texture_lease().clone())
        }
        QtRhiInteropInfo::D3d12(_) => {
            let entry = qt_wgpu_renderer::d3d::cached_rgba8_texture_entry_d3d12(
                &mut context,
                key,
                "qt-solid-vello-render-target",
            )
            .map_err(|error| qt_error(error.to_string()))?;
            (entry.texture_view().clone(), entry.texture_lease().clone())
        }
        QtRhiInteropInfo::D3d11(info) => {
            let entry = qt_wgpu_renderer::d3d::cached_rgba8_texture_entry_d3d11(
                &mut context,
                info,
                key,
                "qt-solid-vello-render-target",
            )
            .map_err(|error| qt_error(error.to_string()))?;
            (entry.texture_view().clone(), entry.texture_lease().clone())
        }
        QtRhiInteropInfo::OpenGles2(_) => {
            let entry = qt_wgpu_renderer::gles::cached_rgba8_texture_entry(
                &mut context,
                key,
                "qt-solid-vello-render-target",
            )
            .map_err(|error| qt_error(error.to_string()))?;
            (entry.texture_view().clone(), entry.texture_lease().clone())
        }
    };
    let renderer = context
        .state_or_insert_with::<Renderer>(|device| {
            Renderer::new(device, RendererOptions::default())
                .map_err(|error| qt_wgpu_renderer::QtWgpuRendererError::new(error.to_string()))
        })
        .map_err(|error| qt_error(error.to_string()))?;
    renderer
        .render_to_texture(
            &device,
            &queue,
            &scaled_scene,
            &texture_view,
            &RenderParams {
                base_color: Color::from_rgba8(0, 0, 0, 0),
                width: width_px,
                height: height_px,
                antialiasing_method: AaConfig::Area,
            },
        )
        .map_err(|error| {
            qt_error(format!(
                "failed to render vello scene to native texture for node {node_id}: {error}",
            ))
        })?;
    match rhi_interop {
        QtRhiInteropInfo::D3d12(_) => qt_wgpu_renderer::d3d::finish_texture_render_d3d12(&context)
            .map_err(|error| qt_error(error.to_string()))?,
        QtRhiInteropInfo::D3d11(_) => qt_wgpu_renderer::d3d::finish_texture_render_d3d11(&context)
            .map_err(|error| qt_error(error.to_string()))?,
        _ => {}
    }
    Ok(texture_lease)
}
