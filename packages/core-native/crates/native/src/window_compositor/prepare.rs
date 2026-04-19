use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use napi::Result;
use qt_solid_widget_core::runtime::{WidgetCapture, WidgetCaptureFormat};
use qt_solid_widget_core::vello::VelloDirtyRect;

use crate::qt;
use crate::{
    qt::QtRect,
    runtime::qt_error,
    window_compositor::state::{
        PartVisibleRect, QtPreparedWindowCompositorFrame, QtPreparedWindowCompositorPart,
        WindowCaptureComposingPart, WindowCompositorCache, WindowCompositorDirtyFlags,
        WindowCompositorDirtyRegion, WindowCompositorLayerEntry, WindowCompositorLayerSourceKind,
        WindowCompositorPartUploadKind,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PixelRect {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
}

impl PixelRect {
    pub(crate) fn is_empty(self) -> bool {
        self.left >= self.right || self.top >= self.bottom
    }

    pub(crate) fn intersect(self, other: Self) -> Option<Self> {
        let rect = Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (!rect.is_empty()).then_some(rect)
    }
}

fn pixel_rects_can_merge(left: PixelRect, right: PixelRect) -> bool {
    left.top <= right.bottom
        && right.top <= left.bottom
        && left.left <= right.right
        && right.left <= left.right
}

pub(crate) fn merge_pixel_rects(mut regions: Vec<PixelRect>) -> Vec<PixelRect> {
    if regions.len() <= 1 {
        return regions;
    }
    regions.sort_by_key(|rect| (rect.top, rect.left, rect.bottom, rect.right));
    let mut merged = Vec::with_capacity(regions.len());
    let mut current = regions[0];
    for rect in regions.into_iter().skip(1) {
        if pixel_rects_can_merge(current, rect) {
            current = PixelRect {
                left: current.left.min(rect.left),
                top: current.top.min(rect.top),
                right: current.right.max(rect.right),
                bottom: current.bottom.max(rect.bottom),
            };
        } else {
            merged.push(current);
            current = rect;
        }
    }
    merged.push(current);
    merged
}

fn union_pixel_rects(left: PixelRect, right: PixelRect) -> PixelRect {
    PixelRect {
        left: left.left.min(right.left),
        top: left.top.min(right.top),
        right: left.right.max(right.right),
        bottom: left.bottom.max(right.bottom),
    }
}

pub(crate) fn coalesce_pixel_rects_for_budget(
    regions: Vec<PixelRect>,
    full_area: usize,
    max_regions: usize,
    max_pair_expansion_ratio: f64,
    merge_all_expansion_ratio: f64,
    merge_all_full_ratio: f64,
) -> Vec<PixelRect> {
    let mut regions = merge_pixel_rects(regions);
    if regions.len() <= 1 {
        return regions;
    }

    let dirty_area = regions.iter().copied().map(pixel_rect_area).sum::<usize>();
    if dirty_area == 0 {
        return Vec::new();
    }

    let full_union = regions
        .iter()
        .copied()
        .reduce(union_pixel_rects)
        .expect("regions is non-empty");
    let full_union_area = pixel_rect_area(full_union);
    if full_union_area > 0
        && (full_area == 0 || (full_union_area as f64 / full_area as f64) <= merge_all_full_ratio)
        && (full_union_area as f64 / dirty_area as f64) <= merge_all_expansion_ratio
    {
        return vec![full_union];
    }

    while regions.len() > max_regions {
        let mut best_pair: Option<(usize, usize, PixelRect, f64)> = None;
        for left_index in 0..regions.len() {
            for right_index in left_index + 1..regions.len() {
                let merged = union_pixel_rects(regions[left_index], regions[right_index]);
                let merged_area = pixel_rect_area(merged);
                if merged_area == 0 {
                    continue;
                }
                if full_area != 0 && (merged_area as f64 / full_area as f64) > merge_all_full_ratio
                {
                    continue;
                }

                let source_area =
                    pixel_rect_area(regions[left_index]) + pixel_rect_area(regions[right_index]);
                let expansion_ratio = merged_area as f64 / source_area as f64;
                if expansion_ratio > max_pair_expansion_ratio {
                    continue;
                }

                match best_pair {
                    Some((_, _, _, best_ratio)) if expansion_ratio >= best_ratio => {}
                    _ => {
                        best_pair = Some((left_index, right_index, merged, expansion_ratio));
                    }
                }
            }
        }

        let Some((left_index, right_index, merged, _)) = best_pair else {
            break;
        };
        regions.swap_remove(right_index);
        regions.swap_remove(left_index);
        regions.push(merged);
        regions = merge_pixel_rects(regions);
    }

    regions
}

pub(crate) fn pixel_rect_to_qt_rect(rect: PixelRect) -> QtRect {
    QtRect {
        x: rect.left,
        y: rect.top,
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
    }
}

fn part_visible_rect_device_bounds_from_dims(
    width_px: u32,
    height_px: u32,
    target_scale_factor: f64,
    part: &WindowCaptureComposingPart,
    rect: PartVisibleRect,
) -> Result<Option<PixelRect>> {
    if (part.capture.scale_factor() - target_scale_factor).abs() > 0.001 {
        return Err(qt_error(format!(
            "window capture part {} uses scale factor {}, expected {}",
            part.node_id,
            part.capture.scale_factor(),
            target_scale_factor
        )));
    }

    let target_width = i32::try_from(width_px).map_err(|_| qt_error("target width overflow"))?;
    let target_height = i32::try_from(height_px).map_err(|_| qt_error("target height overflow"))?;
    let dst_x = (f64::from(part.x) * target_scale_factor).round() as i32;
    let dst_y = (f64::from(part.y) * target_scale_factor).round() as i32;
    let rect_x = (f64::from(rect.x) * target_scale_factor).round() as i32;
    let rect_y = (f64::from(rect.y) * target_scale_factor).round() as i32;
    let rect_width = (f64::from(rect.width) * target_scale_factor).round() as i32;
    let rect_height = (f64::from(rect.height) * target_scale_factor).round() as i32;
    if rect_width <= 0 || rect_height <= 0 {
        return Ok(None);
    }

    let full = PixelRect {
        left: dst_x + rect_x,
        top: dst_y + rect_y,
        right: dst_x + rect_x + rect_width,
        bottom: dst_y + rect_y + rect_height,
    };
    Ok(full.intersect(PixelRect {
        left: 0,
        top: 0,
        right: target_width,
        bottom: target_height,
    }))
}

pub(crate) fn part_visible_device_regions_from_dims(
    width_px: u32,
    height_px: u32,
    target_scale_factor: f64,
    part: &WindowCaptureComposingPart,
) -> Result<Vec<PixelRect>> {
    if part.visible_rects.is_empty() {
        return Ok(Vec::new());
    }

    let mut regions = Vec::new();
    for rect in &part.visible_rects {
        if let Some(region) = part_visible_rect_device_bounds_from_dims(
            width_px,
            height_px,
            target_scale_factor,
            part,
            *rect,
        )? {
            regions.push(region);
        }
    }
    Ok(merge_pixel_rects(regions))
}

pub(crate) fn part_device_bounds_from_dims(
    width_px: u32,
    height_px: u32,
    target_scale_factor: f64,
    part: &WindowCaptureComposingPart,
) -> Result<Option<PixelRect>> {
    let regions =
        part_visible_device_regions_from_dims(width_px, height_px, target_scale_factor, part)?;
    let mut iter = regions.into_iter();
    let Some(first) = iter.next() else {
        return Ok(None);
    };
    let union = iter.fold(first, |acc, region| PixelRect {
        left: acc.left.min(region.left),
        top: acc.top.min(region.top),
        right: acc.right.max(region.right),
        bottom: acc.bottom.max(region.bottom),
    });
    Ok(Some(union))
}

pub(crate) fn dirty_region_device_bounds(
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    region: WindowCompositorDirtyRegion,
) -> Result<Option<PixelRect>> {
    let target_width = i32::try_from(width_px).map_err(|_| qt_error("target width overflow"))?;
    let target_height = i32::try_from(height_px).map_err(|_| qt_error("target height overflow"))?;
    let left = (f64::from(region.x) * scale_factor).round() as i32;
    let top = (f64::from(region.y) * scale_factor).round() as i32;
    let width = (f64::from(region.width) * scale_factor).round() as i32;
    let height = (f64::from(region.height) * scale_factor).round() as i32;
    if width <= 0 || height <= 0 {
        return Ok(None);
    }

    Ok(PixelRect {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
    .intersect(PixelRect {
        left: 0,
        top: 0,
        right: target_width,
        bottom: target_height,
    }))
}

fn layer_capture_device_bounds(
    target_scale_factor: f64,
    part: &WindowCompositorLayerEntry,
) -> Result<PixelRect> {
    if (part.scale_factor - target_scale_factor).abs() > 0.001 {
        return Err(qt_error(format!(
            "window compositor layer {} uses scale factor {}, expected {}",
            part.node_id, part.scale_factor, target_scale_factor
        )));
    }

    let left = (f64::from(part.x) * target_scale_factor).round() as i32;
    let top = (f64::from(part.y) * target_scale_factor).round() as i32;
    let right = left + i32::try_from(part.width_px).map_err(|_| qt_error("part width overflow"))?;
    let bottom =
        top + i32::try_from(part.height_px).map_err(|_| qt_error("part height overflow"))?;

    Ok(PixelRect {
        left,
        top,
        right,
        bottom,
    })
}

fn dirty_region_local_pixel_rect_for_layer(
    target_scale_factor: f64,
    part: &WindowCompositorLayerEntry,
    region: WindowCompositorDirtyRegion,
) -> Result<Option<PixelRect>> {
    let left = (f64::from(region.x) * target_scale_factor).round() as i32;
    let top = (f64::from(region.y) * target_scale_factor).round() as i32;
    let width = (f64::from(region.width) * target_scale_factor).round() as i32;
    let height = (f64::from(region.height) * target_scale_factor).round() as i32;
    if width <= 0 || height <= 0 {
        return Ok(None);
    }

    let part_bounds = layer_capture_device_bounds(target_scale_factor, part)?;
    let Some(intersection) = PixelRect {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
    .intersect(part_bounds) else {
        return Ok(None);
    };

    Ok(Some(PixelRect {
        left: intersection.left - part_bounds.left,
        top: intersection.top - part_bounds.top,
        right: intersection.right - part_bounds.left,
        bottom: intersection.bottom - part_bounds.top,
    }))
}

fn pixel_rect_area(rect: PixelRect) -> usize {
    if rect.is_empty() {
        return 0;
    }

    let width = usize::try_from(rect.right - rect.left).expect("pixel rect width non-negative");
    let height = usize::try_from(rect.bottom - rect.top).expect("pixel rect height non-negative");
    width.saturating_mul(height)
}

fn logical_vello_dirty_rect_to_local_pixel_rect(
    layout: &qt::QtWidgetCaptureLayout,
    rect: VelloDirtyRect,
) -> Result<Option<PixelRect>> {
    let inflate_px = 2_i32;
    let left = (rect.x * layout.scale_factor).floor() as i32 - inflate_px;
    let top = (rect.y * layout.scale_factor).floor() as i32 - inflate_px;
    let right = ((rect.x + rect.width) * layout.scale_factor).ceil() as i32 + inflate_px;
    let bottom = ((rect.y + rect.height) * layout.scale_factor).ceil() as i32 + inflate_px;
    if left >= right || top >= bottom {
        return Ok(None);
    }

    Ok(PixelRect {
        left,
        top,
        right,
        bottom,
    }
    .intersect(PixelRect {
        left: 0,
        top: 0,
        right: i32::try_from(layout.width_px).map_err(|_| qt_error("layout width overflow"))?,
        bottom: i32::try_from(layout.height_px).map_err(|_| qt_error("layout height overflow"))?,
    }))
}

pub(crate) fn vello_dirty_rects_to_local_pixel_rects(
    layout: &qt::QtWidgetCaptureLayout,
    dirty_rects: &[VelloDirtyRect],
) -> Result<Vec<PixelRect>> {
    const VELLO_DIRTY_MAX_REGIONS: usize = 2;
    const VELLO_DIRTY_MAX_PAIR_EXPANSION_RATIO: f64 = 1.6;
    const VELLO_DIRTY_MERGE_ALL_EXPANSION_RATIO: f64 = 1.9;
    const VELLO_DIRTY_MERGE_ALL_FULL_RATIO: f64 = 0.72;

    let mut regions = Vec::new();
    for rect in dirty_rects {
        if let Some(region) = logical_vello_dirty_rect_to_local_pixel_rect(layout, *rect)? {
            regions.push(region);
        }
    }
    let full_area = usize::try_from(layout.width_px)
        .expect("width fits usize")
        .saturating_mul(usize::try_from(layout.height_px).expect("height fits usize"));
    Ok(coalesce_pixel_rects_for_budget(
        regions,
        full_area,
        VELLO_DIRTY_MAX_REGIONS,
        VELLO_DIRTY_MAX_PAIR_EXPANSION_RATIO,
        VELLO_DIRTY_MERGE_ALL_EXPANSION_RATIO,
        VELLO_DIRTY_MERGE_ALL_FULL_RATIO,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PremulPixel {
    pub(crate) red: u8,
    pub(crate) green: u8,
    pub(crate) blue: u8,
    pub(crate) alpha: u8,
}

fn premul_scale(value: u8, factor: u8) -> u8 {
    let scaled = (u32::from(value) * u32::from(factor) + 127) / 255;
    u8::try_from(scaled).expect("premul channel stays within u8")
}

fn premul_over(dst: PremulPixel, src: PremulPixel) -> PremulPixel {
    let inv_alpha = 255_u8.saturating_sub(src.alpha);
    PremulPixel {
        red: src.red.saturating_add(premul_scale(dst.red, inv_alpha)),
        green: src.green.saturating_add(premul_scale(dst.green, inv_alpha)),
        blue: src.blue.saturating_add(premul_scale(dst.blue, inv_alpha)),
        alpha: src.alpha.saturating_add(premul_scale(dst.alpha, inv_alpha)),
    }
}

pub(crate) fn read_capture_pixel(capture: &WidgetCapture, x: u32, y: u32) -> PremulPixel {
    let offset = y as usize * capture.stride() + x as usize * 4;
    let pixel = &capture.bytes()[offset..offset + 4];
    match capture.format() {
        WidgetCaptureFormat::Argb32Premultiplied => {
            #[cfg(target_endian = "little")]
            {
                PremulPixel {
                    blue: pixel[0],
                    green: pixel[1],
                    red: pixel[2],
                    alpha: pixel[3],
                }
            }
            #[cfg(target_endian = "big")]
            {
                PremulPixel {
                    alpha: pixel[0],
                    red: pixel[1],
                    green: pixel[2],
                    blue: pixel[3],
                }
            }
        }
        WidgetCaptureFormat::Rgba8Premultiplied => PremulPixel {
            red: pixel[0],
            green: pixel[1],
            blue: pixel[2],
            alpha: pixel[3],
        },
    }
}

#[cfg(test)]
pub(crate) fn write_argb32_premultiplied_pixel(
    capture: &mut WidgetCapture,
    x: u32,
    y: u32,
    pixel: PremulPixel,
) {
    let offset = y as usize * capture.stride() + x as usize * 4;
    let bytes = &mut capture.bytes_mut()[offset..offset + 4];
    #[cfg(target_endian = "little")]
    {
        bytes[0] = pixel.blue;
        bytes[1] = pixel.green;
        bytes[2] = pixel.red;
        bytes[3] = pixel.alpha;
    }
    #[cfg(target_endian = "big")]
    {
        bytes[0] = pixel.alpha;
        bytes[1] = pixel.red;
        bytes[2] = pixel.green;
        bytes[3] = pixel.blue;
    }
}

fn read_argb32_premultiplied_pixel_from_bytes(
    bytes: &[u8],
    stride: usize,
    x: u32,
    y: u32,
) -> PremulPixel {
    let offset = y as usize * stride + x as usize * 4;
    let pixel = &bytes[offset..offset + 4];
    #[cfg(target_endian = "little")]
    {
        PremulPixel {
            blue: pixel[0],
            green: pixel[1],
            red: pixel[2],
            alpha: pixel[3],
        }
    }
    #[cfg(target_endian = "big")]
    {
        PremulPixel {
            alpha: pixel[0],
            red: pixel[1],
            green: pixel[2],
            blue: pixel[3],
        }
    }
}

fn write_argb32_premultiplied_pixel_to_bytes(
    bytes: &mut [u8],
    stride: usize,
    x: u32,
    y: u32,
    pixel: PremulPixel,
) {
    let offset = y as usize * stride + x as usize * 4;
    let target = &mut bytes[offset..offset + 4];
    #[cfg(target_endian = "little")]
    {
        target[0] = pixel.blue;
        target[1] = pixel.green;
        target[2] = pixel.red;
        target[3] = pixel.alpha;
    }
    #[cfg(target_endian = "big")]
    {
        target[0] = pixel.alpha;
        target[1] = pixel.red;
        target[2] = pixel.green;
        target[3] = pixel.blue;
    }
}

#[cfg(test)]
fn clear_argb32_region(target: &mut WidgetCapture, region: PixelRect) -> Result<()> {
    if target.format() != WidgetCaptureFormat::Argb32Premultiplied {
        return Err(qt_error(
            "partial compose target must be argb32-premultiplied",
        ));
    }

    let Some(region) = region.intersect(PixelRect {
        left: 0,
        top: 0,
        right: i32::try_from(target.width_px()).map_err(|_| qt_error("target width overflow"))?,
        bottom: i32::try_from(target.height_px())
            .map_err(|_| qt_error("target height overflow"))?,
    }) else {
        return Ok(());
    };

    for y in region.top..region.bottom {
        for x in region.left..region.right {
            write_argb32_premultiplied_pixel(
                target,
                u32::try_from(x).expect("non-negative destination x"),
                u32::try_from(y).expect("non-negative destination y"),
                PremulPixel {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 0,
                },
            );
        }
    }

    Ok(())
}

fn clear_argb32_region_in_bytes(
    bytes: &mut [u8],
    width_px: u32,
    height_px: u32,
    stride: usize,
    region: PixelRect,
) -> Result<()> {
    let Some(region) = region.intersect(PixelRect {
        left: 0,
        top: 0,
        right: i32::try_from(width_px).map_err(|_| qt_error("target width overflow"))?,
        bottom: i32::try_from(height_px).map_err(|_| qt_error("target height overflow"))?,
    }) else {
        return Ok(());
    };

    for y in region.top..region.bottom {
        for x in region.left..region.right {
            write_argb32_premultiplied_pixel_to_bytes(
                bytes,
                stride,
                u32::try_from(x).expect("non-negative destination x"),
                u32::try_from(y).expect("non-negative destination y"),
                PremulPixel {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 0,
                },
            );
        }
    }

    Ok(())
}

#[cfg(test)]
fn blend_capture_part_into_window_region(
    target: &mut WidgetCapture,
    target_scale_factor: f64,
    part: &WindowCaptureComposingPart,
    region: PixelRect,
) -> Result<()> {
    let part_origin_x = (f64::from(part.x) * target_scale_factor).round() as i32;
    let part_origin_y = (f64::from(part.y) * target_scale_factor).round() as i32;
    for visible_region in part_visible_device_regions_from_dims(
        target.width_px(),
        target.height_px(),
        target_scale_factor,
        part,
    )? {
        let Some(region) = visible_region.intersect(region) else {
            continue;
        };

        for dst_y_px in region.top..region.bottom {
            let src_y = dst_y_px - part_origin_y;
            for dst_x_px in region.left..region.right {
                let src_x = dst_x_px - part_origin_x;
                let src_pixel = read_capture_pixel(
                    &part.capture,
                    u32::try_from(src_x).expect("non-negative source x"),
                    u32::try_from(src_y).expect("non-negative source y"),
                );
                if src_pixel.alpha == 0 {
                    continue;
                }

                let dst_pixel = read_capture_pixel(
                    target,
                    u32::try_from(dst_x_px).expect("non-negative destination x"),
                    u32::try_from(dst_y_px).expect("non-negative destination y"),
                );
                let out_pixel = premul_over(dst_pixel, src_pixel);
                write_argb32_premultiplied_pixel(
                    target,
                    u32::try_from(dst_x_px).expect("non-negative destination x"),
                    u32::try_from(dst_y_px).expect("non-negative destination y"),
                    out_pixel,
                );
            }
        }
    }

    Ok(())
}

fn blend_capture_part_into_bytes_region(
    bytes: &mut [u8],
    width_px: u32,
    height_px: u32,
    stride: usize,
    target_scale_factor: f64,
    part: &WindowCaptureComposingPart,
    region: PixelRect,
) -> Result<()> {
    let part_origin_x = (f64::from(part.x) * target_scale_factor).round() as i32;
    let part_origin_y = (f64::from(part.y) * target_scale_factor).round() as i32;
    for visible_region in
        part_visible_device_regions_from_dims(width_px, height_px, target_scale_factor, part)?
    {
        let Some(region) = visible_region.intersect(region) else {
            continue;
        };

        for dst_y_px in region.top..region.bottom {
            let src_y = dst_y_px - part_origin_y;
            for dst_x_px in region.left..region.right {
                let src_x = dst_x_px - part_origin_x;
                let src_pixel = read_capture_pixel(
                    &part.capture,
                    u32::try_from(src_x).expect("non-negative source x"),
                    u32::try_from(src_y).expect("non-negative source y"),
                );
                if src_pixel.alpha == 0 {
                    continue;
                }

                let dst_pixel = read_argb32_premultiplied_pixel_from_bytes(
                    bytes,
                    stride,
                    u32::try_from(dst_x_px).expect("non-negative destination x"),
                    u32::try_from(dst_y_px).expect("non-negative destination y"),
                );
                let out_pixel = premul_over(dst_pixel, src_pixel);
                write_argb32_premultiplied_pixel_to_bytes(
                    bytes,
                    stride,
                    u32::try_from(dst_x_px).expect("non-negative destination x"),
                    u32::try_from(dst_y_px).expect("non-negative destination y"),
                    out_pixel,
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
pub(crate) fn compose_window_capture_regions(
    base: &WidgetCapture,
    scale_factor: f64,
    parts: &[WindowCaptureComposingPart],
    regions: &[PixelRect],
) -> Result<WidgetCapture> {
    let mut capture = base.clone();
    for region in regions {
        if region.is_empty() {
            continue;
        }
        clear_argb32_region(&mut capture, *region)?;
        for part in parts {
            blend_capture_part_into_window_region(&mut capture, scale_factor, part, *region)?;
        }
    }
    Ok(capture)
}

pub(crate) fn compose_window_capture_regions_in_place(
    bytes: &mut [u8],
    width_px: u32,
    height_px: u32,
    stride: usize,
    scale_factor: f64,
    parts: &[WindowCaptureComposingPart],
    regions: &[PixelRect],
) -> Result<()> {
    for region in regions {
        if region.is_empty() {
            continue;
        }
        clear_argb32_region_in_bytes(bytes, width_px, height_px, stride, *region)?;
        for part in parts {
            blend_capture_part_into_bytes_region(
                bytes,
                width_px,
                height_px,
                stride,
                scale_factor,
                part,
                *region,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn compose_window_capture_group(
    width_px: u32,
    height_px: u32,
    stride: usize,
    scale_factor: f64,
    parts: &[WindowCaptureComposingPart],
) -> Result<WidgetCapture> {
    let mut capture = WidgetCapture::new_zeroed(
        WidgetCaptureFormat::Argb32Premultiplied,
        width_px,
        height_px,
        stride,
        scale_factor,
    )
    .map_err(|error| qt_error(error.message().to_owned()))?;

    let full_region = PixelRect {
        left: 0,
        top: 0,
        right: i32::try_from(width_px).map_err(|_| qt_error("target width overflow"))?,
        bottom: i32::try_from(height_px).map_err(|_| qt_error("target height overflow"))?,
    };
    for part in parts {
        blend_capture_part_into_window_region(&mut capture, scale_factor, part, full_region)?;
    }

    Ok(capture)
}

pub(crate) fn compose_window_capture_group_in_place(
    bytes: &mut [u8],
    width_px: u32,
    height_px: u32,
    stride: usize,
    scale_factor: f64,
    parts: &[WindowCaptureComposingPart],
) -> Result<()> {
    let full_region = PixelRect {
        left: 0,
        top: 0,
        right: i32::try_from(width_px).map_err(|_| qt_error("target width overflow"))?,
        bottom: i32::try_from(height_px).map_err(|_| qt_error("target height overflow"))?,
    };
    clear_argb32_region_in_bytes(bytes, width_px, height_px, stride, full_region)?;
    for part in parts {
        blend_capture_part_into_bytes_region(
            bytes,
            width_px,
            height_px,
            stride,
            scale_factor,
            part,
            full_region,
        )?;
    }
    Ok(())
}

pub(crate) fn collect_scene_node_dirty_regions(
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    dirty_nodes: &HashSet<u32>,
    old_parts: &HashMap<u32, WindowCaptureComposingPart>,
    new_parts: &HashMap<u32, WindowCaptureComposingPart>,
) -> Result<Vec<PixelRect>> {
    let mut regions = Vec::new();
    for node_id in dirty_nodes {
        if let Some(old_part) = old_parts.get(node_id) {
            if let Some(region) =
                part_device_bounds_from_dims(width_px, height_px, scale_factor, old_part)?
            {
                regions.push(region);
            }
        }
        if let Some(new_part) = new_parts.get(node_id) {
            if let Some(region) =
                part_device_bounds_from_dims(width_px, height_px, scale_factor, new_part)?
            {
                regions.push(region);
            }
        }
    }

    Ok(merge_pixel_rects(regions))
}

pub(crate) fn build_prepared_window_compositor_frame(
    current_cache: &WindowCompositorCache,
    previous_cache: Option<&WindowCompositorCache>,
    dirty_flags: WindowCompositorDirtyFlags,
    dirty_nodes: &HashSet<u32>,
    dirty_region_hints: &[WindowCompositorDirtyRegion],
    base_upload_kind: WindowCompositorPartUploadKind,
    overlay_layout_changed: bool,
) -> Result<Box<QtPreparedWindowCompositorFrame>> {
    const PREPARED_FRAME_MAX_SUBRECT_UPLOADS: usize = 1;
    const PREPARED_FRAME_FULL_UPLOAD_AREA_RATIO: f64 = 0.25;

    let previous_parts = previous_cache.map(|cache| {
        cache
            .parts
            .iter()
            .map(|part| (part.node_id, part))
            .collect::<HashMap<_, _>>()
    });
    let force_full_upload = previous_cache.is_none();
    let mut parts = Vec::with_capacity(current_cache.parts.len());

    for part in &current_cache.parts {
        let previous_part = previous_parts
            .as_ref()
            .and_then(|parts_by_node| parts_by_node.get(&part.node_id))
            .copied();
        let needs_layer_redraw = force_full_upload || dirty_nodes.contains(&part.node_id);
        let mut upload_kind = WindowCompositorPartUploadKind::None;
        let mut dirty_rects = Vec::new();

        if force_full_upload {
            upload_kind = WindowCompositorPartUploadKind::Full;
        } else if dirty_flags.contains(WindowCompositorDirtyFlags::GEOMETRY)
            || dirty_flags.contains(WindowCompositorDirtyFlags::SCENE)
        {
            let capture_reused = previous_part
                .and_then(|previous| previous.capture().zip(part.capture()))
                .map(|(previous, current)| Arc::ptr_eq(previous, current))
                .unwrap_or(false);
            if !capture_reused {
                upload_kind = WindowCompositorPartUploadKind::Full;
            }
        }

        if upload_kind != WindowCompositorPartUploadKind::Full
            && dirty_flags.contains(WindowCompositorDirtyFlags::PIXELS)
            && dirty_nodes.contains(&part.node_id)
        {
            for region in dirty_region_hints
                .iter()
                .copied()
                .filter(|region| region.node_id == part.node_id)
            {
                if let Some(local_rect) = dirty_region_local_pixel_rect_for_layer(
                    current_cache.scale_factor,
                    part,
                    region,
                )? {
                    dirty_rects.push(local_rect);
                }
            }

            let full_area = usize::try_from(part.width_px)
                .expect("part width fits usize")
                .saturating_mul(usize::try_from(part.height_px).expect("part height fits usize"));
            dirty_rects = coalesce_pixel_rects_for_budget(dirty_rects, full_area, 2, 1.5, 1.6, 0.5);
            if dirty_rects.is_empty() {
                upload_kind = WindowCompositorPartUploadKind::Full;
            } else {
                let dirty_area = dirty_rects
                    .iter()
                    .copied()
                    .map(pixel_rect_area)
                    .sum::<usize>();
                if dirty_rects.len() > PREPARED_FRAME_MAX_SUBRECT_UPLOADS
                    || (full_area != 0
                        && (dirty_area as f64 / full_area as f64)
                            >= PREPARED_FRAME_FULL_UPLOAD_AREA_RATIO)
                {
                    dirty_rects.clear();
                    upload_kind = WindowCompositorPartUploadKind::Full;
                } else {
                    upload_kind = WindowCompositorPartUploadKind::SubRects;
                }
            }
        }

        parts.push(QtPreparedWindowCompositorPart {
            meta: part.into_compositor_meta(),
            visible_rects: part
                .visible_rects
                .iter()
                .map(|rect| QtRect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                })
                .collect(),
            upload_kind,
            dirty_rects: dirty_rects.into_iter().map(pixel_rect_to_qt_rect).collect(),
            source_kind: part.source_kind(),
            needs_layer_redraw,
            capture: match part.source_kind() {
                WindowCompositorLayerSourceKind::CpuCapture => part.capture().cloned(),
                WindowCompositorLayerSourceKind::CachedTexture => None,
            },
        });
    }

    Ok(Box::new(QtPreparedWindowCompositorFrame {
        base_upload_kind,
        overlay_layout_changed,
        parts,
    }))
}
