// Composites a promoted layer with vibrancy effect onto the surface.
// Combines composite_layer (vertex transform) with vibrancy (fragment blending).
// The layer texture is sampled but never written to.

struct LayerUniforms {
    transform: mat4x4<f32>,
    bounds: vec4<f32>,
    viewport: vec4<f32>,
    opacity_flags: vec4<f32>,
    _pad: vec4<f32>,
};

struct VibrancyParams {
    desaturation: f32,
    blend_mode: f32,
    _align_pad: vec2<f32>,
    tint: vec4<f32>,
    texture_size: vec2<f32>,
    backdrop_uv_offset: vec2<f32>,
    backdrop_uv_scale: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0) var tex_sampler: sampler;
@group(0) @binding(1) var layer_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> uniforms: LayerUniforms;
@group(0) @binding(3) var backdrop_texture: texture_2d<f32>;
@group(0) @binding(4) var<uniform> vibrancy: VibrancyParams;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let quad_uv = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let uv = quad_uv[vertex_index];

    let local_pos = uniforms.bounds.xy + uv * uniforms.bounds.zw;
    let projected = uniforms.transform * vec4<f32>(local_pos, 0.0, 1.0);

    var out: VertexOutput;
    out.position = projected;
    out.uv = uv;
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

// Signed distance to a rounded rectangle centered at origin.
fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let q = abs(p) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let opacity = uniforms.opacity_flags.x;
    let corner_radius = uniforms.opacity_flags.z;

    // SDF coverage mask for rounded rect clipping.
    // in.uv is [0,1] over the layer quad; map to local pixel coords.
    let local_size = uniforms.bounds.zw;
    let local_pos = in.uv * local_size;
    let half_size = local_size * 0.5;
    let center = half_size;
    let dist = rounded_rect_sdf(local_pos - center, half_size, corner_radius);
    // Anti-aliased edge: 1.0 inside, 0.0 outside, smooth over ~1px.
    let coverage = clamp(0.5 - dist, 0.0, 1.0);
    if coverage < 0.001 {
        return vec4<f32>(0.0);
    }

    // Sample backdrop via vibrancy UV remapping
    let backdrop_uv = in.uv * vibrancy.backdrop_uv_scale + vibrancy.backdrop_uv_offset;
    let backdrop = textureSampleLevel(backdrop_texture, tex_sampler, backdrop_uv, 0.0);

    // Un-premultiply backdrop
    let ba = max(backdrop.a, 0.001);
    let original_rgb = backdrop.rgb / ba;

    // Process: desaturate then tint
    let lum = dot(original_rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    var processed_rgb = mix(original_rgb, vec3<f32>(lum), vibrancy.desaturation);
    processed_rgb = processed_rgb * vibrancy.tint.rgb;

    // tint.a = effect intensity (0 = original backdrop, 1 = full vibrancy)
    var base_rgb = mix(original_rgb, processed_rgb, vibrancy.tint.a);

    // Sample foreground (layer texture, unmodified)
    let foreground = textureSample(layer_texture, tex_sampler, in.uv);

    if foreground.a > 0.001 {
        let fa = max(foreground.a, 0.001);
        let fg_rgb = foreground.rgb / fa;

        var blended: vec3<f32>;
        let mode = i32(vibrancy.blend_mode + 0.5);
        if mode == 0 {
            blended = blend_multiply(base_rgb, fg_rgb);
        } else if mode == 1 {
            blended = blend_screen(base_rgb, fg_rgb);
        } else if mode == 2 {
            blended = blend_overlay(base_rgb, fg_rgb);
        } else {
            blended = min(base_rgb + fg_rgb, vec3<f32>(1.0));
        }

        // Alpha-over: blended foreground onto vibrancy base
        let out_rgb = blended * foreground.a + base_rgb * (1.0 - foreground.a);
        let out_a = ba * coverage;
        return vec4<f32>(out_rgb * out_a, out_a) * opacity;
    }

    // Transparent foreground — vibrancy base replaces backdrop region
    let out_a = ba * coverage;
    return vec4<f32>(base_rgb * out_a, out_a) * opacity;
}
