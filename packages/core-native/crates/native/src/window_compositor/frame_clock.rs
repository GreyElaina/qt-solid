use std::time::Duration;

use napi::Result;
use qt_solid_widget_core::{runtime::QtValue, vello::FrameTime as VelloFrameTime};

use crate::{
    bootstrap::widget_registry,
    runtime::{
        NodeHandle, apply_prop_by_name, invalid_arg, node_by_id, node_parent_id,
        qt_value_type_name, read_prop_exact, wrap_node_id,
    },
};

pub(crate) fn window_ancestor_id_for_node(generation: u64, node_id: u32) -> Result<Option<u32>> {
    let mut current = Some(node_id);
    while let Some(id) = current {
        let node = node_by_id(generation, id)?;
        if widget_registry()
            .binding_for_node_class(node.inner().class)
            .kind_name
            == "window"
        {
            return Ok(Some(id));
        }
        current = node_parent_id(generation, id)?;
    }

    Ok(None)
}

pub(crate) fn read_frame_f64_prop(window: &impl NodeHandle, js_name: &str) -> Result<f64> {
    let Some(value) = read_prop_exact(window, js_name)? else {
        return Err(invalid_arg(format!("missing window frame prop {js_name}",)));
    };

    match value {
        QtValue::F64(value) => Ok(value),
        other => Err(invalid_arg(format!(
            "window frame prop {js_name} returned {} instead of f64",
            qt_value_type_name(&other),
        ))),
    }
}

fn read_frame_bool_prop(window: &impl NodeHandle, js_name: &str) -> Result<bool> {
    let Some(value) = read_prop_exact(window, js_name)? else {
        return Err(invalid_arg(format!("missing window frame prop {js_name}",)));
    };

    match value {
        QtValue::Bool(value) => Ok(value),
        other => Err(invalid_arg(format!(
            "window frame prop {js_name} returned {} instead of bool",
            qt_value_type_name(&other),
        ))),
    }
}

pub(crate) fn write_frame_bool_prop(
    window: &impl NodeHandle,
    js_name: &str,
    value: bool,
) -> Result<()> {
    apply_prop_by_name(window, js_name, QtValue::Bool(value))?
        .ok_or_else(|| invalid_arg(format!("missing window frame prop {js_name}")))
}

pub(crate) fn tick_window_frame_exact(window: &impl NodeHandle) -> Result<()> {
    write_frame_bool_prop(window, "tick", true)
}

pub(crate) fn take_window_next_frame_request_exact(window: &impl NodeHandle) -> Result<bool> {
    let requested = read_frame_bool_prop(window, "nextFrameRequested")?;
    if requested {
        write_frame_bool_prop(window, "nextFrameRequested", false)?;
    }
    Ok(requested)
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
