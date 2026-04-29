use std::time::Duration;

use napi::Result;
use crate::canvas::vello::FrameTime as VelloFrameTime;

use crate::{
    runtime::{
        NodeHandle, invalid_arg, node_by_id, node_parent_id,
        with_compositor_state, with_compositor_state_mut, wrap_node_id,
    },
};

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

pub(crate) fn read_frame_f64_prop(window: &impl NodeHandle, js_name: &str) -> Result<f64> {
    let window_id = window.inner().id;
    let clock = with_compositor_state(|state| state.frame_clock(window_id));
    match js_name {
        "seq" => Ok(clock.seq),
        "elapsedMs" => Ok(clock.elapsed_ms),
        "deltaMs" => Ok(clock.delta_ms),
        _ => Err(invalid_arg(format!("unknown frame clock prop {js_name}"))),
    }
}

pub(crate) fn write_frame_bool_prop(
    window: &impl NodeHandle,
    js_name: &str,
    value: bool,
) -> Result<()> {
    let window_id = window.inner().id;
    with_compositor_state_mut(|state| {
        let clock = state.frame_clock_mut(window_id);
        match js_name {
            "nextFrameRequested" => clock.next_frame_requested = value,
            "tick" => {
                // tick increments seq and is handled by the compositor pipeline
                if value {
                    clock.seq += 1.0;
                }
            }
            _ => return Err(invalid_arg(format!("unknown frame clock prop {js_name}"))),
        }
        Ok(())
    })
}

pub(crate) fn tick_window_frame_exact(window: &impl NodeHandle) -> Result<()> {
    write_frame_bool_prop(window, "tick", true)
}

pub(crate) fn take_window_next_frame_request_exact(window: &impl NodeHandle) -> Result<bool> {
    let window_id = window.inner().id;
    with_compositor_state_mut(|state| {
        let clock = state.frame_clock_mut(window_id);
        let requested = clock.next_frame_requested;
        if requested {
            clock.next_frame_requested = false;
        }
        Ok(requested)
    })
}

pub(crate) fn node_frame_time(node: &impl NodeHandle) -> Result<VelloFrameTime> {
    let elapsed_ms = read_frame_f64_prop(node, "elapsedMs")?;
    let delta_ms = read_frame_f64_prop(node, "deltaMs")?;

    Ok(VelloFrameTime {
        elapsed: Duration::from_secs_f64(elapsed_ms.max(0.0) / 1000.0),
        delta: Duration::from_secs_f64(delta_ms.max(0.0) / 1000.0),
    })
}

pub(crate) fn qt_window_frame_tick(node_id: u32) -> Result<()> {
    let node = wrap_node_id(node_id)?;
    tick_window_frame_exact(&node)
}

pub(crate) fn qt_window_frame_take_next_frame_request(node_id: u32) -> Result<bool> {
    let node = wrap_node_id(node_id)?;
    take_window_next_frame_request_exact(&node)
}
