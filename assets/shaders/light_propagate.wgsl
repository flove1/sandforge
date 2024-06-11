#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var ts: sampler;
@group(0) @binding(2) var<uniform> step_offset: f32;
@group(0) @binding(3) var<uniform> shadow: vec3<f32>;

const damp_factor = 0.7;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(t, ts, in.uv).rgb;
    let dimensions =  vec2<f32>(textureDimensions(t));

    color += textureSample(t, ts, in.uv + vec2<f32>(step_offset, step_offset) / dimensions).rgb;
    color += textureSample(t, ts, in.uv + vec2<f32>(-step_offset, step_offset) / dimensions).rgb;
    color += textureSample(t, ts, in.uv + vec2<f32>(step_offset, -step_offset) / dimensions).rgb;
    color += textureSample(t, ts, in.uv + vec2<f32>(-step_offset, -step_offset) / dimensions).rgb;

    color /= 4.0;

    color = max(color, shadow);

    return vec4<f32>(color, 1.0);
}