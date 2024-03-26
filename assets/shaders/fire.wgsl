struct CircleData {
    center: vec3<f32>,
    color: vec4<f32>,
    radius: f32,
}

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;

@group(1) @binding(0) var<uniform> circleData: CircleData;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(input.position, 1.0);

    out.clip_position.x += sign(out.clip_position.x) * circleData.radius;
    out.clip_position.y += sign(out.clip_position.y) * circleData.radius;
    out.clip_position += vec4<f32>(circleData.center, 0.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let distance = length(in.clip_position.xy - circleData.center.xy);

    if (distance > circleData.radius) {
        discard;
    }

    return circleData.color;
}