use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::hint::unreachable_unchecked;
use std::io::Write;
use std::mem::forget;
use std::ops::Deref;
use lazy_mut::LazyMut;
use lazy_static::lazy::Lazy;
use spirv_cross2::compile::CompilableTarget;
use spirv_cross2::compile::hlsl::HlslShaderModel;
use spirv_reflect::types::{ReflectDecorationFlags, ReflectDescriptorType, ReflectImageFormat, ReflectTypeFlags};
use wgpu::{include_spirv, include_spirv_raw, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BufferBindingType, Label, SamplerBindingType, ShaderModule, ShaderModuleDescriptor, ShaderModuleDescriptorPassthrough, ShaderSource, TextureSampleType};
use wgpu::custom::{AsAny, DispatchShaderModule};
use wgpu::naga::MathFunction::Reflect;
use wgpu::wgt::CreateShaderModuleDescriptorPassthrough;
use wgpu_hal::{Device, DynDevice, DynShaderModule, ShaderInput};
use winapi::um::winuser::DefDlgProcA;
use crate::{DEVICE, LOG_FILE};
use crate::shaders;
use crate::shaders::{KernelStruct, Kernels};

static KERNELS: LazyMut<Option<Kernels>> = LazyMut::new(|| None);
#[unsafe(no_mangle)]
pub extern "C" fn init() {
    std::panic::set_hook(Box::new(|panic_info| {
        if let Ok(mut file) =  File::create("panic_log.txt") {
            let _ = writeln!(file, "Panic occurred: {:?}", panic_info.payload_as_str().unwrap());
            let location = panic_info.location().unwrap();
            let _ = writeln!(file,"Occurred in file {} at line {}",location.file(),location.line());
        }
    }));
    let shaders = HashMap::<(&str,&str),Vec<u8>>::from([
        (("IntersectionKernels","kernel_heightmap"),include_bytes!("shaders/main/IntersectionKernels/kernel_heightmap.spv").to_vec()),
        (("IntersectionKernels","kernel_shadow"),include_bytes!("shaders/main/IntersectionKernels/kernel_shadow.spv").to_vec()),
        (("IntersectionKernels","kernel_shadow_heightmap"),include_bytes!("shaders/main/IntersectionKernels/kernel_shadow_heightmap.spv").to_vec()),
        (("IntersectionKernels","kernel_trace"),include_bytes!("shaders/main/IntersectionKernels/kernel_trace.spv").to_vec()),
        (("RayGenKernels","CacheCompact"),include_bytes!("shaders/main/RayGenKernels/CacheCompact.spv").to_vec()),
        (("RayGenKernels","CacheResolve"),include_bytes!("shaders/main/RayGenKernels/CacheResolve.spv").to_vec()),
        (("RayGenKernels","Generate"),include_bytes!("shaders/main/RayGenKernels/Generate.spv").to_vec()),
        (("RayGenKernels","GeneratePanorama"),include_bytes!("shaders/main/RayGenKernels/GeneratePanorama.spv").to_vec()),
        (("RayTracingShader","kernel_finalize"),include_bytes!("shaders/main/RayTracingShader/kernel_finalize.spv").to_vec()),
        (("RayTracingShader","kernel_shade"),include_bytes!("shaders/main/RayTracingShader/kernel_shade.spv").to_vec()),
        (("RayTracingShader","MVKernel"),include_bytes!("shaders/main/RayTracingShader/MVKernel.spv").to_vec()),
        (("RayTracingShader","OIDNtoTTKernel"),include_bytes!("shaders/main/RayTracingShader/OIDNtoTTKernel.spv").to_vec()),
        (("RayTracingShader","RefineMVKernel"),include_bytes!("shaders/main/RayTracingShader/RefineMVKernel.spv").to_vec()),
        (("RayTracingShader","ResetMVKernel"),include_bytes!("shaders/main/RayTracingShader/ResetMVKernel.spv").to_vec()),
        (("RayTracingShader","TransferKernel"),include_bytes!("shaders/main/RayTracingShader/TransferKernel.spv").to_vec()),
        (("RayTracingShader","TTtoOIDNKernel"),include_bytes!("shaders/main/RayTracingShader/TTtoOIDNKernel.spv").to_vec()),
        (("RayTracingShader","TTtoOIDNKernelPanorama"),include_bytes!("shaders/main/RayTracingShader/TTtoOIDNKernelPanorama.spv").to_vec()),
        (("ReSTIRGI","ReSTIRGIKernel"),include_bytes!("shaders/main/ReSTIRGI/ReSTIRGIKernel.spv").to_vec()),
        (("ReSTIRGI","ReSTIRGISpatial"),include_bytes!("shaders/main/ReSTIRGI/ReSTIRGISpatial.spv").to_vec()),
        (("ReSTIRGI","ReSTIRGISpatial2"),include_bytes!("shaders/main/ReSTIRGI/ReSTIRGISpatial2.spv").to_vec()),
    ]);
    println!("Hello from init!");
    KERNELS.get_mut().replace(Kernels {
        intersection_kernels: KernelStruct {
            kernel_name_and_name_to_binding: HashMap::new(),
            kernel_to_bgl_and_shader_mod: HashMap::new(),
            kernel_name_and_name_to_globals_offset: HashMap::new(),
        },
        raygen_kernels: KernelStruct {
            kernel_name_and_name_to_binding: HashMap::new(),
            kernel_to_bgl_and_shader_mod: HashMap::new(),
            kernel_name_and_name_to_globals_offset: HashMap::new(),
        },
        ray_tracing_shader: KernelStruct {
            kernel_name_and_name_to_binding: HashMap::new(),
            kernel_to_bgl_and_shader_mod: HashMap::new(),
            kernel_name_and_name_to_globals_offset: HashMap::new(),
        },
        restir_gi: KernelStruct {
            kernel_name_and_name_to_binding: HashMap::new(),
            kernel_to_bgl_and_shader_mod: HashMap::new(),
            kernel_name_and_name_to_globals_offset: HashMap::new(),
        },
    });
    let mut binding = DEVICE.get_mut();
    let (device,queue) = binding.as_mut().unwrap();
    for ((group,kernel), mut shader) in shaders.clone() {
        let mut binding = KERNELS.get_mut();
        let kernels_struct = binding.as_mut().unwrap();
        let cap = shader.capacity();
        let len = shader.len();
        let ptr = shader.as_mut_ptr();
        forget(shader);
        let shader = unsafe {Vec::from_raw_parts(ptr as *mut u32,len / 4,cap / 4)};
        let src_mod = spirv_cross2::Module::from_words(&shader);
        let mut compiler = spirv_cross2::Compiler::<spirv_cross2::targets::Hlsl>::new(src_mod).unwrap();
        let mut options = spirv_cross2::targets::Hlsl::options();
        options.shader_model = HlslShaderModel::ShaderModel6_8;
        options.enable_16bit_types = true;
        println!("module_reflect");

        let module_reflect = spirv_reflect::ShaderModule::load_u32_data(&shader).unwrap();
        let bindings = module_reflect.enumerate_descriptor_bindings(None).unwrap();
        let mut entries = Vec::<wgpu::BindGroupLayoutEntry>::new();
        let new_src = compiler.compile(&options).unwrap().to_string();
        let local_size = module_reflect.enumerate_entry_points().unwrap().first().unwrap().local_size;
        let shader_mod = unsafe {device.create_shader_module_passthrough(ShaderModuleDescriptorPassthrough {
            entry_point: module_reflect.get_entry_point_name(),
            label: None,
            hlsl: Some(Cow::from(new_src.as_str())),
            ..Default::default()
        })};
        println!("Loading shader {:#?}",kernel);
       // println!("shader_mod: {:#?}",shader_mod);
        let mut global_binding: u32 = 0;
        let kernel_struct = match group {
            "IntersectionKernels" => &mut kernels_struct.intersection_kernels,
            "RayGenKernels" => &mut kernels_struct.raygen_kernels,
            "RayTracingShader" => &mut kernels_struct.ray_tracing_shader,
            "ReSTIRGI" => &mut kernels_struct.restir_gi,
            _ => unreachable!()
        };
        for reflect_binding in bindings {
            if reflect_binding.name == "$Globals" {
                global_binding = reflect_binding.binding;
              //  println!("$Global has {:#?} members",reflect_binding.block.members.len());
                for member in reflect_binding.block.members {
                    kernel_struct.kernel_name_and_name_to_globals_offset.insert((String::from(kernel), member.name.clone()), member.offset);
                }
            }
            entries.push(BindGroupLayoutEntry {
                binding: reflect_binding.binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                count: None,
                ty: match reflect_binding.descriptor_type {
                    ReflectDescriptorType::StorageBuffer => wgpu::BindingType::Buffer {
                        ty: BufferBindingType::Storage {
                            read_only: reflect_binding.type_description.unwrap().decoration_flags.contains(ReflectDecorationFlags::NON_WRITABLE),
                        },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    ReflectDescriptorType::AccelerationStructureKHR => wgpu::BindingType::AccelerationStructure {
                        vertex_return: false,
                    },
                    ReflectDescriptorType::CombinedImageSampler => unimplemented!(),
                    ReflectDescriptorType::SampledImage => wgpu::BindingType::Texture {
                        view_dimension: match (reflect_binding.image.dim, reflect_binding.image.arrayed != 0) {
                            (spirv_reflect::types::ReflectDimension::Cube, false) => wgpu::TextureViewDimension::Cube,
                            (spirv_reflect::types::ReflectDimension::Cube, true) => wgpu::TextureViewDimension::CubeArray,
                            (spirv_reflect::types::ReflectDimension::Type1d, false) => wgpu::TextureViewDimension::D1,
                            (spirv_reflect::types::ReflectDimension::Type2d, false) => wgpu::TextureViewDimension::D2,
                            (spirv_reflect::types::ReflectDimension::Type2d, true) => wgpu::TextureViewDimension::D2Array,
                            (spirv_reflect::types::ReflectDimension::Type3d, false) => wgpu::TextureViewDimension::D3,
                            _ => unimplemented!()
                        },
                        sample_type: {
                            let flags = reflect_binding.type_description.unwrap().type_flags;
                            if flags.contains(ReflectTypeFlags::FLOAT) {
                                TextureSampleType::Float { filterable: false }
                            } else if flags.contains(ReflectTypeFlags::INT) {
                                TextureSampleType::Sint
                            } else {
                                unimplemented!()
                            }
                        },
                        multisampled: reflect_binding.image.ms != 0,
                    },
                    ReflectDescriptorType::Sampler => wgpu::BindingType::Sampler(SamplerBindingType::NonFiltering),
                    ReflectDescriptorType::StorageBufferDynamic => wgpu::BindingType::Buffer {
                        ty: BufferBindingType::Storage {
                            read_only: reflect_binding.type_description.unwrap().decoration_flags.contains(ReflectDecorationFlags::NON_WRITABLE),
                        },
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    ReflectDescriptorType::StorageImage => wgpu::BindingType::StorageTexture {
                        access: match reflect_binding.type_description.unwrap().decoration_flags.contains(ReflectDecorationFlags::NON_WRITABLE) {
                            true => wgpu::StorageTextureAccess::ReadOnly,
                            false => wgpu::StorageTextureAccess::ReadWrite,
                        },
                        format: convert_reflection_image_fmt_to_wgpu(reflect_binding.image.image_format),
                        view_dimension: match (reflect_binding.image.dim, reflect_binding.image.arrayed != 0) {
                            (spirv_reflect::types::ReflectDimension::Cube, false) => wgpu::TextureViewDimension::Cube,
                            (spirv_reflect::types::ReflectDimension::Cube, true) => wgpu::TextureViewDimension::CubeArray,
                            (spirv_reflect::types::ReflectDimension::Type1d, false) => wgpu::TextureViewDimension::D1,
                            (spirv_reflect::types::ReflectDimension::Type2d, false) => wgpu::TextureViewDimension::D2,
                            (spirv_reflect::types::ReflectDimension::Type2d, true) => wgpu::TextureViewDimension::D2Array,
                            (spirv_reflect::types::ReflectDimension::Type3d, false) => wgpu::TextureViewDimension::D3,
                            _ => unimplemented!()
                        },
                    },
                    ReflectDescriptorType::StorageTexelBuffer => wgpu::BindingType::Buffer {
                        ty: BufferBindingType::Storage {
                            read_only: reflect_binding.type_description.unwrap().decoration_flags.contains(ReflectDecorationFlags::NON_WRITABLE),
                        },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    ReflectDescriptorType::UniformBuffer => wgpu::BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    ReflectDescriptorType::UniformBufferDynamic => wgpu::BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    ReflectDescriptorType::UniformTexelBuffer => wgpu::BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    _ => unimplemented!()
                },
            });
            kernel_struct.kernel_name_and_name_to_binding.insert((kernel.to_string(), reflect_binding.name), reflect_binding.binding);
        }
        let bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            entries: &entries,
            label: Some(format!("bgl for {}",kernel).as_str()),
        });
        kernel_struct.kernel_to_bgl_and_shader_mod.insert(String::from(kernel), (bgl, shader_mod));
        // LOG_FILE.get_mut().as_ref().unwrap().write("Worked I guess!".as_bytes()).unwrap();
    }
}
fn convert_reflection_image_fmt_to_wgpu(fmt: ReflectImageFormat) -> wgpu::TextureFormat {
    match fmt {
        ReflectImageFormat::R8_INT => wgpu::TextureFormat::R8Sint,
        ReflectImageFormat::R8 => wgpu::TextureFormat::R8Unorm,
        ReflectImageFormat::R8_SNORM => wgpu::TextureFormat::R8Snorm,
        ReflectImageFormat::R8_UINT => wgpu::TextureFormat::R8Uint,
        ReflectImageFormat::R16_UINT => wgpu::TextureFormat::R16Uint,
        ReflectImageFormat::R16_SNORM => wgpu::TextureFormat::R16Snorm,
        ReflectImageFormat::R16 => wgpu::TextureFormat::R16Unorm,
        ReflectImageFormat::R16_FLOAT => wgpu::TextureFormat::R16Float,
        ReflectImageFormat::R16_INT => wgpu::TextureFormat::R16Sint,
        ReflectImageFormat::R32_INT  => wgpu::TextureFormat::R32Sint,
        ReflectImageFormat::R32_FLOAT => wgpu::TextureFormat::R32Float,
        ReflectImageFormat::R32_UINT => wgpu::TextureFormat::R32Uint,
        ReflectImageFormat::RG8 => wgpu::TextureFormat::Rg8Unorm,
        ReflectImageFormat::RG8_INT => wgpu::TextureFormat::Rg8Sint,
        ReflectImageFormat::RG8_SNORM => wgpu::TextureFormat::Rg8Snorm,
        ReflectImageFormat::RG8_UINT => wgpu::TextureFormat::Rg8Uint,
        ReflectImageFormat::RG16_UINT => wgpu::TextureFormat::Rg16Uint,
        ReflectImageFormat::RG16 =>  wgpu::TextureFormat::Rg16Unorm,
        ReflectImageFormat::RG16_SNORM => wgpu::TextureFormat::Rg16Snorm,
        ReflectImageFormat::RG16_FLOAT => wgpu::TextureFormat::Rg16Float,
        ReflectImageFormat::RG32_FLOAT => wgpu::TextureFormat::Rg32Float,
        ReflectImageFormat::RG32_UINT => wgpu::TextureFormat::Rg32Uint,
        ReflectImageFormat::RG32_INT => wgpu::TextureFormat::Rg32Sint,
        ReflectImageFormat::RGB10A2 => wgpu::TextureFormat::Rgb10a2Unorm,
        ReflectImageFormat::RGB10A2_UINT => wgpu::TextureFormat::Rgb10a2Uint,
        ReflectImageFormat::RGBA8 => wgpu::TextureFormat::Rgba8Unorm,
        ReflectImageFormat::RGBA8_SNORM => wgpu::TextureFormat::Rgba8Snorm,
        ReflectImageFormat::RGBA8_UINT => wgpu::TextureFormat::Rgba8Uint,
        ReflectImageFormat::RGBA8_INT => wgpu::TextureFormat::Rgba8Sint,
        ReflectImageFormat::RGBA16 => wgpu::TextureFormat::Rgba16Unorm,
        ReflectImageFormat::RGBA16_FLOAT => wgpu::TextureFormat::Rgba16Float,
        ReflectImageFormat::RGBA16_INT => wgpu::TextureFormat::Rgba16Sint,
        ReflectImageFormat::RGBA16_UINT => wgpu::TextureFormat::Rgba16Uint,
        ReflectImageFormat::RGBA16_SNORM => wgpu::TextureFormat::Rgba16Snorm,
        ReflectImageFormat::RGBA32_FLOAT => wgpu::TextureFormat::Rgba32Float,
        ReflectImageFormat::RGBA32_INT => wgpu::TextureFormat::Rgba32Sint,
        ReflectImageFormat::RGBA32_UINT => wgpu::TextureFormat::Rgba32Uint,
        ReflectImageFormat::R11G11B10_FLOAT => wgpu::TextureFormat::Rg11b10Ufloat,
        ReflectImageFormat::RG16_INT => wgpu::TextureFormat::Rg16Sint,
        ReflectImageFormat::Undefined => unimplemented!(),
    }
}