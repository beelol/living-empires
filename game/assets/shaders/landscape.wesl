// Original analytic water: three directional waves, no textures or geometry displacement.
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::globals,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> settings: vec4<f32>;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(in, front);
    let p = in.world_position.xz;
    let t = globals.time * 0.6;
    let fade = 1.0 / (1.0 + length(dpdx(p)) + length(dpdy(p)));
    let a = cos(dot(p, vec2<f32>(0.19, 0.12)) + t);
    let b = cos(dot(p, vec2<f32>(-0.11, 0.27)) - t * 0.8);
    let c = cos(dot(p, vec2<f32>(0.48, 0.39)) + t * 1.3);
    let ripple = settings.x * vec2<f32>(a * 0.14 - b * 0.09 + c * 0.035, a * 0.09 + b * 0.15 + c * 0.028);
    pbr.N = normalize(vec3<f32>(-ripple.x * fade, 1.0, -ripple.y * fade));
    let fresnel = pow(1.0 - max(dot(pbr.N, pbr.V), 0.0), 4.0);
    let deep = vec3<f32>(0.015, 0.075, 0.09);
    let sky = vec3<f32>(0.23, 0.36, 0.39);
    pbr.material.base_color = vec4<f32>(mix(deep, sky, fresnel * 0.6), 1.0);
    pbr.material.perceptual_roughness = 0.28;
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr);
    out.color = main_pass_post_lighting_processing(pbr, out.color);
    return out;
}
