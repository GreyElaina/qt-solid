use rustc_hash::{FxHashMap, FxHashSet};
use vello::wgpu;
use vello_common::paint::ImageId;
use vello_hybrid::Renderer;

use crate::canvas::vello::Scene;

pub(crate) type HybridImageCache = FxHashMap<u64, ImageId>;

/// Collect live image Blob IDs from a recorded scene, then destroy any
/// cached atlas entries that are no longer referenced.
pub(crate) fn sweep_stale_images(
    scene: &Scene,
    renderer: &mut Renderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    image_cache: &mut HybridImageCache,
) {
    use anyrender::recording::RenderCommand;
    use peniko::Brush;

    if image_cache.is_empty() {
        return;
    }

    let mut live = FxHashSet::default();
    for cmd in &scene.commands {
        let brush = match cmd {
            RenderCommand::Fill(c) => &c.brush,
            RenderCommand::Stroke(c) => &c.brush,
            RenderCommand::GlyphRun(c) => &c.brush,
            _ => continue,
        };
        if let Brush::Image(ib) = brush {
            live.insert(ib.image.data.id());
        }
    }

    let stale: Vec<(u64, ImageId)> = image_cache
        .iter()
        .filter(|(k, _)| !live.contains(k))
        .map(|(&k, &v)| (k, v))
        .collect();

    for (blob_id, image_id) in stale {
        image_cache.remove(&blob_id);
        renderer.destroy_image(device, queue, encoder, image_id);
    }
}
