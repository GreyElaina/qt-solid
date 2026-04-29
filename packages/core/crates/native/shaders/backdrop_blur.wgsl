struct BlurParams {
    rect_min: vec2<f32>,
    rect_size: vec2<f32>,
    corner_radius: f32,
    blur_radius: f32,
    texture_size: vec2<f32>,
    direction: vec2<f32>,
    _padding: vec2<f32>,
}

@group(0) @binding(0) var<uniform> params: BlurParams;
@group(0) @binding(1) var src_texture: texture_2d<f32>;
@group(0) @binding(2) var src_sampler: sampler;

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
    let uv = in.pixel_pos / params.texture_size;

    let half_size = params.rect_size * 0.5;
    let center = params.rect_min + half_size;
    let p = in.pixel_pos - center;
    let dist = rounded_rect_sdf(p, half_size, params.corner_radius);

    // Outside the rounded-rect region: passthrough
    if dist >= 0.0 {
        return textureSampleLevel(src_texture, src_sampler, uv, 0.0);
    }

    // 1D Gaussian blur along params.direction
    let kernel_radius = min(i32(ceil(3.0 * params.blur_radius)), 64);
    let inv_sigma = 1.0 / params.blur_radius;
    let step = params.direction / params.texture_size;

    var acc = vec4<f32>(0.0);
    var total_weight = 0.0;

    for (var i = -kernel_radius; i <= kernel_radius; i = i + 1) {
        let fi = f32(i);
        let w = exp(-0.5 * fi * fi * inv_sigma * inv_sigma);
        let sample_uv = uv + step * fi;
        acc = acc + textureSampleLevel(src_texture, src_sampler, sample_uv, 0.0) * w;
        total_weight = total_weight + w;
    }

    return acc / total_weight;
}
