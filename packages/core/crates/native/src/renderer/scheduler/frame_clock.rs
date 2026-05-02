use std::time::Duration;

use napi::Result;
use crate::canvas::vello::FrameTime as VelloFrameTime;

use crate::runtime::{NodeHandle, node_by_id, node_parent_id, wrap_node_id};

pub(crate) fn window_ancestor_id_for_node(generation: u64, node_id: u32) -> Result<Option<u32>> {
    let mut current = Some(node_id);
    while let Some(id) = current {
        let node = node_by_id(generation, id)?;
        if node.inner().is_window() {
            return Ok(Some(id));
        }
        current = node_parent_id(generation, id)?;
    }

    Ok(None)
}

pub(crate) fn node_frame_time(node: &impl NodeHandle) -> Result<VelloFrameTime> {
    let window_id = node.inner().id;
    let clock = crate::renderer::with_renderer(|r| r.scheduler.frame_clock(window_id));

    Ok(VelloFrameTime {
        elapsed: Duration::from_secs_f64(clock.elapsed_ms.max(0.0) / 1000.0),
        delta: Duration::from_secs_f64(clock.delta_ms.max(0.0) / 1000.0),
    })
}

pub(crate) fn qt_window_frame_tick(node_id: u32) -> Result<()> {
    let node = wrap_node_id(node_id)?;
    let window_id = node.inner().id;
    crate::renderer::with_renderer_mut(|r| r.scheduler.tick_frame(window_id));
    Ok(())
}

pub(crate) fn qt_window_frame_take_next_frame_request(node_id: u32) -> Result<bool> {
    let node = wrap_node_id(node_id)?;
    let window_id = node.inner().id;
    Ok(crate::renderer::with_renderer_mut(|r| r.scheduler.take_next_frame_request(window_id)))
}
