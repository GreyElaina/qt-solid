// Post-process filter for promoted layer textures.
// Applied in-place: reads from layer texture, writes filtered result.

struct FilterParams {
    // Each value: 0.0 = no effect, 1.0 = full effect (except brightness/contrast/saturate which use multipliers)
    grayscale: f32,      // 0..1
    saturate: f32,       // multiplier, 1.0 = normal
    brightness: f32,     // multiplier, 1.0 = normal
    contrast: f32,       // multiplier, 1.0 = normal
    hue_rotate: f32,     // degrees
    invert: f32,         // 0..1
    sepia: f32,          // 0..1
    _pad: f32,
    texture_size: vec2<f32>,
    _pad2: vec2<f32>,
};

@group(0) @binding(0) var<uniform> params: FilterParams;
@group(0) @binding(1) var src_texture: texture_2d<f32>;
@group(0) @binding(2) var src_sampler: sampler;

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

fn rgb_to_hsl(c: vec3<f32>) -> vec3<f32> {
    let mx = max(max(c.r, c.g), c.b);
    let mn = min(min(c.r, c.g), c.b);
    let l = (mx + mn) * 0.5;
    if mx == mn { return vec3<f32>(0.0, 0.0, l); }
    let d = mx - mn;
    let s = select(d / (2.0 - mx - mn), d / (mx + mn), l < 0.5);
    var h: f32;
    if mx == c.r { h = (c.g - c.b) / d + select(0.0, 6.0, c.g < c.b); }
    else if mx == c.g { h = (c.b - c.r) / d + 2.0; }
    else { h = (c.r - c.g) / d + 4.0; }
    h = h / 6.0;
    return vec3<f32>(h, s, l);
}

fn hue2rgb(p: f32, q: f32, t_in: f32) -> f32 {
    var t = t_in;
    if t < 0.0 { t = t + 1.0; }
    if t > 1.0 { t = t - 1.0; }
    if t < 1.0/6.0 { return p + (q - p) * 6.0 * t; }
    if t < 1.0/2.0 { return q; }
    if t < 2.0/3.0 { return p + (q - p) * (2.0/3.0 - t) * 6.0; }
    return p;
}

fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
    if hsl.y == 0.0 { return vec3<f32>(hsl.z); }
    let q = select(hsl.z + hsl.y - hsl.z * hsl.y, hsl.z * (1.0 + hsl.y), hsl.z < 0.5);
    let p = 2.0 * hsl.z - q;
    return vec3<f32>(
        hue2rgb(p, q, hsl.x + 1.0/3.0),
        hue2rgb(p, q, hsl.x),
        hue2rgb(p, q, hsl.x - 1.0/3.0),
    );
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var c = textureSampleLevel(src_texture, src_sampler, in.uv, 0.0);

    // Un-premultiply for color math
    let a = max(c.a, 0.001);
    var rgb = c.rgb / a;

    // Invert
    if params.invert > 0.001 {
        rgb = mix(rgb, vec3<f32>(1.0) - rgb, params.invert);
    }

    // Brightness
    rgb = rgb * params.brightness;

    // Contrast
    rgb = (rgb - 0.5) * params.contrast + 0.5;

    // Saturate
    let gray = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    rgb = mix(vec3<f32>(gray), rgb, params.saturate);

    // Grayscale
    if params.grayscale > 0.001 {
        let g = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        rgb = mix(rgb, vec3<f32>(g), params.grayscale);
    }

    // Sepia
    if params.sepia > 0.001 {
        let sepia_r = dot(rgb, vec3<f32>(0.393, 0.769, 0.189));
        let sepia_g = dot(rgb, vec3<f32>(0.349, 0.686, 0.168));
        let sepia_b = dot(rgb, vec3<f32>(0.272, 0.534, 0.131));
        rgb = mix(rgb, vec3<f32>(sepia_r, sepia_g, sepia_b), params.sepia);
    }

    // Hue rotate
    if abs(params.hue_rotate) > 0.1 {
        var hsl = rgb_to_hsl(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)));
        hsl.x = fract(hsl.x + params.hue_rotate / 360.0);
        rgb = hsl_to_rgb(hsl);
    }

    // Re-premultiply
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)) * a, c.a);
}
