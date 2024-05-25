#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var ts: sampler;
@group(0) @binding(2) var<uniform> step_offset: f32;

const damp_factor = 0.7;
const damp_diagonal = 1.0;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(t, ts, in.uv).rgb;
    let dimensions =  vec2<f32>(textureDimensions(t));

    color = max(color, damp_factor * textureSample(t, ts, in.uv + vec2<f32>(step_offset, 0.0) / dimensions).rgb);
    color = max(color, damp_factor * textureSample(t, ts, in.uv + vec2<f32>(step_offset, step_offset) / dimensions).rgb * damp_diagonal);
    color = max(color, damp_factor * textureSample(t, ts, in.uv + vec2<f32>(-step_offset, 0.0) / dimensions).rgb);
    color = max(color, damp_factor * textureSample(t, ts, in.uv + vec2<f32>(-step_offset, step_offset) / dimensions).rgb * damp_diagonal);
    color = max(color, damp_factor * textureSample(t, ts, in.uv + vec2<f32>(0.0, step_offset) / dimensions).rgb);
    color = max(color, damp_factor * textureSample(t, ts, in.uv + vec2<f32>(step_offset, -step_offset) / dimensions).rgb * damp_diagonal);
    color = max(color, damp_factor * textureSample(t, ts, in.uv + vec2<f32>(0.0, -step_offset) / dimensions).rgb);
    color = max(color, damp_factor * textureSample(t, ts, in.uv + vec2<f32>(-step_offset, -step_offset) / dimensions).rgb * damp_diagonal);
    color = max(color, vec3<f32>(0.025, 0.025, 0.025));

    return vec4<f32>(color, 1.0);
}