#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
// #import bevy_render::globals

@group(0) @binding(0)
var t: texture_2d<f32>;

@group(0) @binding(1)
var ts: sampler;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let original = textureSample(t, ts, in.uv);

    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}