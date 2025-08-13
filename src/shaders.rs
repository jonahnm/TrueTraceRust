use std::collections::HashMap;
use wgpu::{BindGroupLayout, PipelineLayout, ShaderModule};

pub struct KernelStruct {
    pub kernel_name_and_name_to_binding: HashMap<(String,String), u32>,
    pub kernel_to_bgl_and_shader_mod: HashMap<String,(BindGroupLayout,ShaderModule)>,
    pub kernel_name_and_name_to_globals_offset: HashMap<(String,String), u32>,
}
pub struct Kernels {
    pub intersection_kernels: KernelStruct,
    pub raygen_kernels: KernelStruct,
    pub ray_tracing_shader: KernelStruct,
    pub restir_gi: KernelStruct,
}