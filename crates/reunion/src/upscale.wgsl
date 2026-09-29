// Scales the low-resolution game canvas (one texel per game pixel) to the
// screen. `mode` picks the filter, see upscale.rs.

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct Params {
    // Canvas size in texels.
    source_size: vec2<f32>,
    // Screen pixels per texel, per axis.
    scale: vec2<f32>,
    mode: u32,
    // WebGL2 wants uniform blocks in multiples of 16 bytes.
    padding0: u32,
    padding1: u32,
    padding2: u32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: Params;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var canvas: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var canvas_sampler: sampler;

fn texel(p: vec2<i32>) -> vec4<f32> {
    let size = vec2<i32>(params.source_size);
    return textureLoad(canvas, clamp(p, vec2<i32>(0), size - 1), 0);
}

fn nearest(uv: vec2<f32>) -> vec4<f32> {
    return texel(vec2<i32>(floor(uv * params.source_size)));
}

// Nearest-neighbour blocks with a one-screen-pixel blend at their edges, so
// uneven scales (3.33x on the Steam Deck) don't give uneven pixel sizes.
// Used for the CRT's columns.
fn sharp_bilinear(uv: vec2<f32>) -> vec4<f32> {
    let t = uv * params.source_size;
    let base = floor(t);
    let d = fract(t) - 0.5;
    let region = 0.5 - 0.5 / params.scale;
    let f = (d - clamp(d, -region, region)) * params.scale + 0.5;
    return textureSampleLevel(canvas, canvas_sampler, (base + f) / params.source_size, 0.0);
}

// A CRT look: sharp columns, dark gaps between the scanlines, a faint
// aperture grille.
fn crt(uv: vec2<f32>, frag: vec2<f32>) -> vec4<f32> {
    let color = sharp_bilinear(uv).rgb;
    let row = fract(uv.y * params.source_size.y) - 0.5;
    let beam = exp(-row * row / (2.0 * 0.3 * 0.3));
    var lit = color * mix(0.45, 1.0, beam) * 1.25;
    let column = u32(frag.x) % 3u;
    var mask = vec3<f32>(0.9);
    mask[column] = 1.1;
    lit *= mask;
    return vec4<f32>(lit, 1.0);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    if params.mode == 1u {
        return crt(uv, mesh.position.xy);
    }
    return nearest(uv);
}
