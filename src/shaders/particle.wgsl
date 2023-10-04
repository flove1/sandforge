struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct ParticleInstance {
    @location(1) position: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput, instance: ParticleInstance) -> VertexOutput {
    var out: VertexOutput;
    out.color = instance.color;
    out.clip_position = vec4<f32>(input.position + instance.position, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}