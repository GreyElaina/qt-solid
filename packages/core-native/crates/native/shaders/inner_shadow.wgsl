struct Params {
    rect_min: vec2<f32>,
    rect_size: vec2<f32>,
    corner_radius: f32,
    blur_std_dev: f32,
    offset: vec2<f32>,
    color: vec4<f32>,
    texture_size: vec2<f32>,
    _padding: vec2<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) pixel_pos: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    // Fullscreen triangle: 3 vertices that cover [-1,1] clip space
    let x = f32(i32(vi & 1u)) * 4.0 - 1.0;
    let y = f32(i32(vi >> 1u)) * 4.0 - 1.0;

    var out: VertexOutput;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    // Convert clip coords to pixel coords
    let uv = vec2<f32>(x, -y) * 0.5 + 0.5;
    out.pixel_pos = uv * params.texture_size;
    return out;
}

// Signed distance to a rounded rectangle centered at origin.
// Negative inside, positive outside.
fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let q = abs(p) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let half_size = params.rect_size * 0.5;
    let center = params.rect_min + half_size;
    let p = in.pixel_pos - center;

    // Distance to element boundary (negative = inside)
    let rect_d = rounded_rect_sdf(p, half_size, params.corner_radius);

    // Discard pixels outside the element
    if rect_d >= 0.0 {
        return vec4<f32>(0.0);
    }

    // Distance to the offset shadow-casting shape
    let offset_d = rounded_rect_sdf(p - params.offset, half_size, params.corner_radius);

    // Shadow alpha:
    //   offset_d << 0  (deep inside offset shape) → 0 (no shadow)
    //   offset_d ≈ 0   (near edge)                → transitioning
    //   offset_d > 0   (outside offset shape)     → 1 (full shadow)
    let blur = max(params.blur_std_dev, 0.001);
    let shadow_alpha = smoothstep(-blur, 0.0, offset_d);

    // Premultiplied alpha output
    return params.color * shadow_alpha;
}
