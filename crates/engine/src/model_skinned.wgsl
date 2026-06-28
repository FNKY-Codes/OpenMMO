struct SceneUniforms {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    tint: vec4<f32>,
};

struct BoneUniforms {
    bones: array<mat4x4<f32>, 64>,
};

@group(0) @binding(0) var<uniform> scene: SceneUniforms;
@group(0) @binding(1) var<uniform> bone_ubo: BoneUniforms;
@group(0) @binding(2) var tex_sampler: sampler;
@group(0) @binding(3) var tex: texture_2d<f32>;

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

fn skin_position(pos: vec3<f32>, joints: vec4<u32>, weights: vec4<f32>) -> vec3<f32> {
    let v = vec4<f32>(pos, 1.0);
    var out = vec4(0.0);
    out += bone_ubo.bones[joints.x] * v * weights.x;
    out += bone_ubo.bones[joints.y] * v * weights.y;
    out += bone_ubo.bones[joints.z] * v * weights.z;
    out += bone_ubo.bones[joints.w] * v * weights.w;
    return out.xyz;
}

fn skin_normal(normal: vec3<f32>, joints: vec4<u32>, weights: vec4<f32>) -> vec3<f32> {
    let v = vec4<f32>(normal, 0.0);
    var out = vec4(0.0);
    out += bone_ubo.bones[joints.x] * v * weights.x;
    out += bone_ubo.bones[joints.y] * v * weights.y;
    out += bone_ubo.bones[joints.z] * v * weights.z;
    out += bone_ubo.bones[joints.w] * v * weights.w;
    return out.xyz;
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let skinned_pos = skin_position(input.position, input.joints, input.weights);
    let skinned_normal = skin_normal(input.normal, input.joints, input.weights);
    let world_pos = scene.model * vec4<f32>(skinned_pos, 1.0);
    out.clip_position = scene.view_proj * world_pos;
    out.uv = input.uv;
    out.world_normal = (scene.model * vec4<f32>(skinned_normal, 0.0)).xyz;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let base = textureSample(tex, tex_sampler, input.uv);
    let light_dir = normalize(vec3<f32>(0.35, 1.0, 0.25));
    let n = normalize(input.world_normal);
    let diff = max(dot(n, light_dir), 0.3);
    let lit = base.rgb * diff;
    return vec4<f32>(lit * scene.tint.rgb, base.a * scene.tint.a);
}
