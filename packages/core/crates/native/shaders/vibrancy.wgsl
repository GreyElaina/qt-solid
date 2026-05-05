// Vibrancy effect: desaturates blurred backdrop, then blends foreground
// with a special blend mode (typically "plus lighter" or overlay).

struct VibrancyParams {
    // Desaturation amount: 0.0 = keep color, 1.0 = fully desaturated
    desaturation: f32,
    // Blend mode: 0=multiply, 1=screen, 2=overlay, 3=plus-lighter
    blend_mode: f32,
    // padding to align tint to 16 bytes
    _align_pad: vec2<f32>,
    // Foreground tint color multiplier (premultiplied RGBA)
    tint: vec4<f32>,
    texture_size: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> params: VibrancyParams;
@group(0) @binding(1) var backdrop_texture: texture_2d<f32>;  // already blurred
@group(0) @binding(2) var foreground_texture: texture_2d<f32>;
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

fn blend_screen(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return 1.0 - (1.0 - base) * (1.0 - blend);
}

fn blend_multiply(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return base * blend;
}

fn blend_overlay(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    let r = select(1.0 - 2.0 * (1.0 - base.r) * (1.0 - blend.r), 2.0 * base.r * blend.r, base.r < 0.5);
    let g = select(1.0 - 2.0 * (1.0 - base.g) * (1.0 - blend.g), 2.0 * base.g * blend.g, base.g < 0.5);
    let b = select(1.0 - 2.0 * (1.0 - base.b) * (1.0 - blend.b), 2.0 * base.b * blend.b, base.b < 0.5);
    return vec3<f32>(r, g, b);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let backdrop = textureSampleLevel(backdrop_texture, tex_sampler, in.uv, 0.0);
    let foreground = textureSampleLevel(foreground_texture, tex_sampler, in.uv, 0.0);

    // Un-premultiply backdrop
    let ba = max(backdrop.a, 0.001);
    var bg_rgb = backdrop.rgb / ba;

    // Desaturate backdrop
    let lum = dot(bg_rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    bg_rgb = mix(bg_rgb, vec3<f32>(lum), params.desaturation);

    // Apply tint
    bg_rgb = bg_rgb * params.tint.rgb;

    // Un-premultiply foreground
    let fa = max(foreground.a, 0.001);
    let fg_rgb = foreground.rgb / fa;

    // Blend
    var result: vec3<f32>;
    let mode = i32(params.blend_mode + 0.5);
    if mode == 0 {
        result = blend_multiply(bg_rgb, fg_rgb);
    } else if mode == 1 {
        result = blend_screen(bg_rgb, fg_rgb);
    } else if mode == 2 {
        result = blend_overlay(bg_rgb, fg_rgb);
    } else {
        // Plus-lighter (additive)
        result = min(bg_rgb + fg_rgb, vec3<f32>(1.0));
    }

    // Output with foreground alpha
    let out_a = foreground.a;
    return vec4<f32>(result * out_a, out_a);
}
