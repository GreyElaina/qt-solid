// Composites a promoted layer texture onto the surface.
// Supports both 2D affine and full 3D perspective transforms.

struct LayerUniforms {
    // 4x4 column-major transform matrix.
    // For 3D: full perspective * model matrix mapping local coords to NDC.
    // The matrix is pre-built by Rust to go from layer-local → clip space.
    transform: mat4x4<f32>,
    // Layer bounds in logical pixels: (x, y, w, h).
    bounds: vec4<f32>,
    // Viewport dimensions: (w, h, 0, 0).
    viewport: vec4<f32>,
    // (opacity, backface_visible, 0, 0)
    opacity_flags: vec4<f32>,
    // padding to 128 bytes
    _pad: vec4<f32>,
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
    let quad_uv = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let uv = quad_uv[vertex_index];

    // Map UV [0,1] to layer local position in pixels.
    let bounds_xy = uniforms.bounds.xy;
    let bounds_wh = uniforms.bounds.zw;
    let local_pos = bounds_xy + uv * bounds_wh;

    // Apply 4x4 transform (handles both 2D affine and 3D perspective).
    // The matrix maps local pixel coords directly to clip space [-1,1].
    let projected = uniforms.transform * vec4<f32>(local_pos, 0.0, 1.0);

    var out: VertexOutput;
    out.position = projected;
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Backface culling: when the quad is facing away, discard.
    // We detect this via the sign of the w component during rasterization.
    // For perspective transforms where the surface flips, gl_FrontFacing
    // won't help since we use triangle list. Instead, check if the
    // determinant of the upper-left 3x3 of the transform is negative
    // (encoded in opacity_flags.y: 0.0 = cull backface, 1.0 = show both).
    // Actually, simpler: if backface_visible == 0 and the triangle is
    // wound CW (flipped), the GPU's front-face culling handles it.
    // But since we don't enable culling in the pipeline, we use a manual check:
    // The Rust side sets opacity to 0 when backface should be hidden and
    // the layer is past 90 degrees. This is the simplest approach.

    let color = textureSample(layer_texture, layer_sampler, in.uv);
    let opacity = uniforms.opacity_flags.x;
    return color * opacity;
}
