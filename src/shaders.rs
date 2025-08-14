use std::collections::HashMap;
use wgpu::{BindGroupLayout, PipelineLayout, ShaderModule};

pub struct KernelStruct {
    pub kernel_name_and_name_to_binding: HashMap<(String,String), u32>,
    pub kernel_to_bgl_and_shader_mod: HashMap<String,(BindGroupLayout,ShaderModule)>,
    pub name_to_globals_offset: HashMap<String, u32>,
    pub globals: Vec<u8>,
    pub kernel_index_to_name: HashMap<u32,String>,
}
pub struct Kernels {
    pub intersection_kernels: Option<KernelStruct>,
    pub raygen_kernels: Option<KernelStruct>,
    pub ray_tracing_shader: Option<KernelStruct>,
    pub restir_gi: Option<KernelStruct>,
    pub bvh_refitter: Option<KernelStruct>,
    pub copy_texture_shader: Option<KernelStruct>,
    pub general_mesh_functions: Option<KernelStruct>,
}