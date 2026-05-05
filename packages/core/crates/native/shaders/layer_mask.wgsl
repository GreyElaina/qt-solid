// Layer mask: multiplies content alpha by mask texture alpha.
// Renders as a fullscreen triangle over the content layer texture.

struct MaskParams {
    texture_size: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> params: MaskParams;
@group(0) @binding(1) var content_texture: texture_2d<f32>;
@group(0) @binding(2) var mask_texture: texture_2d<f32>;
@group(0) @binding(3) var tex_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    let x = f32(i32(vi & 1u)) * 4.0 - 1.0;
    let y = f32(i32(vi >> 1u)) * 4.0 - 1.0;
    var out: VertexOutput;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x, -y) * 0.5 + 0.5;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let content = textureSampleLevel(content_texture, tex_sampler, in.uv, 0.0);
    let mask = textureSampleLevel(mask_texture, tex_sampler, in.uv, 0.0);
    // Multiply content by mask alpha (premultiplied: scale all channels)
    return content * mask.a;
}
