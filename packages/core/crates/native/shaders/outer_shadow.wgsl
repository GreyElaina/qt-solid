struct ShadowUniforms {
    transform: mat4x4<f32>,
    bounds: vec4<f32>,
    shadow_params: vec4<f32>,
    color: vec4<f32>,
    _padding: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: ShadowUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
};

fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let q = abs(p) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let quad_uv = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let uv = quad_uv[vertex_index];
    let bounds_xy = uniforms.bounds.xy;
    let bounds_wh = uniforms.bounds.zw;
    let local_pos = bounds_xy + uv * bounds_wh;
    let projected = uniforms.transform * vec4<f32>(local_pos, 0.0, 1.0);
    var out: VertexOutput;
    out.position = projected;
    out.local_pos = local_pos - bounds_xy - bounds_wh * 0.5;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let half_size = uniforms.bounds.zw * 0.5;
    let corner_radius = uniforms.shadow_params.x;
    let blur = max(uniforms.shadow_params.y, 0.001);
    let offset = uniforms.shadow_params.zw;

    // SDF to the original rect shape (not the expanded bounds), offset
    let p = in.local_pos - offset;
    // The original rect half-size is smaller than bounds (bounds is expanded by blur extent)
    let orig_half = half_size - vec2<f32>(blur * 3.0);
    let dist = rounded_rect_sdf(p, max(orig_half, vec2<f32>(1.0)), corner_radius);

    // Shadow falloff: outside the shape (dist > 0) fades with Gaussian-like curve
    let alpha = 1.0 - smoothstep(0.0, blur * 2.0, dist);

    return uniforms.color * alpha;
}
