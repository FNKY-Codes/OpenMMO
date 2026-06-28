struct SkinnedUniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    tint: vec4<f32>,
    bones: array<mat4x4<f32>, 64>,
};

@group(0) @binding(0) var<uniform> uniforms: SkinnedUniforms;
@group(0) @binding(1) var tex_sampler: sampler;
@group(0) @binding(2) var tex: texture_2d<f32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) joints: vec4<u32>,
    @location(4) weights: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_normal: vec3<f32>,
}

fn skin_matrix(joints: vec4<u32>, weights: vec4<f32>) -> mat4x4<f32> {
    return uniforms.bones[joints.x] * weights.x
        + uniforms.bones[joints.y] * weights.y
        + uniforms.bones[joints.z] * weights.z
        + uniforms.bones[joints.w] * weights.w;
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let skin = skin_matrix(input.joints, input.weights);
    let skinned_pos = skin * vec4<f32>(input.position, 1.0);
    let skinned_normal = skin * vec4<f32>(input.normal, 0.0);
    let world_pos = uniforms.model * skinned_pos;
    out.clip_position = uniforms.view_proj * world_pos;
    out.uv = input.uv;
    out.world_normal = (uniforms.model * skinned_normal).xyz;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let base = textureSample(tex, tex_sampler, input.uv);
    let light_dir = normalize(vec3<f32>(0.35, 1.0, 0.25));
    let n = normalize(input.world_normal);
    let diff = max(dot(n, light_dir), 0.3);
    let lit = base.rgb * diff;
    return vec4<f32>(lit * uniforms.tint.rgb, base.a * uniforms.tint.a);
}
