// Composites a promoted layer texture onto the surface.
// Per-layer uniforms provide transform, opacity, and bounds mapping.

struct LayerUniforms {
    // Column-major 2x3 affine transform (maps layer UV to viewport NDC).
    // Stored as two vec4: [a, b, c, d] and [e, f, viewport_w, viewport_h].
    transform_ab: vec4<f32>,
    transform_ef: vec4<f32>,
    // Layer bounds in logical pixels: (x, y, w, h).
    bounds: vec4<f32>,
    // Opacity [0..1], padding.
    opacity_pad: vec4<f32>,
};

@group(0) @binding(0) var layer_sampler: sampler;
@group(0) @binding(1) var layer_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> uniforms: LayerUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Quad from two triangles (6 vertices, no index buffer).
    // Vertex order: 0-1-2, 2-1-3 → CCW triangles covering the quad.
    let quad_uv = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let uv = quad_uv[vertex_index];

    // Map UV [0,1] to layer local position.
    let bounds_xy = uniforms.bounds.xy;
    let bounds_wh = uniforms.bounds.zw;
    let local_pos = bounds_xy + uv * bounds_wh;

    // Apply affine transform: [a c e; b d f] * [x y 1]
    let a = uniforms.transform_ab.x;
    let b = uniforms.transform_ab.y;
    let c = uniforms.transform_ab.z;
    let d = uniforms.transform_ab.w;
    let e = uniforms.transform_ef.x;
    let f = uniforms.transform_ef.y;
    let viewport_w = uniforms.transform_ef.z;
    let viewport_h = uniforms.transform_ef.w;

    let world_x = a * local_pos.x + c * local_pos.y + e;
    let world_y = b * local_pos.x + d * local_pos.y + f;

    // World (pixels) → NDC [-1,1]. Y is flipped for wgpu.
    let ndc_x = (world_x / viewport_w) * 2.0 - 1.0;
    let ndc_y = 1.0 - (world_y / viewport_h) * 2.0;

    var out: VertexOutput;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(layer_texture, layer_sampler, in.uv);
    let opacity = uniforms.opacity_pad.x;
    return color * opacity;
}
