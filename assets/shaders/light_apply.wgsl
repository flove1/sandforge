#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var t_1: texture_2d<f32>;
@group(0) @binding(1) var t_2: texture_2d<f32>;
@group(0) @binding(2) var ts: sampler;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    var output = textureSample(t_1, ts, in.uv);
    let lighting = textureSample(t_2, ts, (in.uv + 0.125) / 1.25);

    if lighting.a > 0.0 {
        output.r = output.r * lighting.r;
        output.g = output.g * lighting.g;
        output.b = output.b * lighting.b;
    }
    
    return output;
}