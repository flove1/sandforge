#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var ts: sampler;


@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    if textureLoad(t, vec2<i32>(round(in.uv * vec2<f32>(textureDimensions(t)))), 0).a == 1.0 {
        discard;
    }
    
    return vec4<f32>(1.0, 1.0, 0.9, 1.0);
}