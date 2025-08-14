use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::hint::unreachable_unchecked;
use std::io::Write;
use std::mem::{forget, MaybeUninit};
use std::ops::Deref;
use std::sync::Arc;
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

pub(crate) static KERNELS: LazyMut<Option<&'static mut Kernels>> = LazyMut::new(|| None);
#[unsafe(no_mangle)]
pub extern "C" fn init() {
    std::panic::set_hook(Box::new(|panic_info| {
        if let Ok(mut file) =  File::create("panic_log.txt") {
            let _ = writeln!(file, "Panic occurred: {:?}", panic_info.payload_as_str().unwrap());
            let location = panic_info.location().unwrap();
            let _ = writeln!(file,"Occurred in file {} at line {}",location.file(),location.line());
        }
    }));
    let shaders = HashMap::<(&str,&str,u32),Vec<u8>>::from([
        (("IntersectionKernels","kernel_heightmap",1),include_bytes!("shaders/main/IntersectionKernels/kernel_heightmap.spv").to_vec()),
        (("IntersectionKernels","kernel_shadow",2),include_bytes!("shaders/main/IntersectionKernels/kernel_shadow.spv").to_vec()),
        (("IntersectionKernels","kernel_shadow_heightmap",3),include_bytes!("shaders/main/IntersectionKernels/kernel_shadow_heightmap.spv").to_vec()),
        (("IntersectionKernels","kernel_trace",4),include_bytes!("shaders/main/IntersectionKernels/kernel_trace.spv").to_vec()),
        (("RayGenKernels","CacheCompact",1),include_bytes!("shaders/main/RayGenKernels/CacheCompact.spv").to_vec()),
        (("RayGenKernels","CacheResolve",2),include_bytes!("shaders/main/RayGenKernels/CacheResolve.spv").to_vec()),
        (("RayGenKernels","Generate",3),include_bytes!("shaders/main/RayGenKernels/Generate.spv").to_vec()),
        (("RayGenKernels","GeneratePanorama",4),include_bytes!("shaders/main/RayGenKernels/GeneratePanorama.spv").to_vec()),
        (("RayTracingShader","kernel_finalize",1),include_bytes!("shaders/main/RayTracingShader/kernel_finalize.spv").to_vec()),
        (("RayTracingShader","kernel_shade",2),include_bytes!("shaders/main/RayTracingShader/kernel_shade.spv").to_vec()),
        (("RayTracingShader","MVKernel",3),include_bytes!("shaders/main/RayTracingShader/MVKernel.spv").to_vec()),
        (("RayTracingShader","OIDNtoTTKernel",4),include_bytes!("shaders/main/RayTracingShader/OIDNtoTTKernel.spv").to_vec()),
        (("RayTracingShader","RefineMVKernel",5),include_bytes!("shaders/main/RayTracingShader/RefineMVKernel.spv").to_vec()),
        (("RayTracingShader","ResetMVKernel",6),include_bytes!("shaders/main/RayTracingShader/ResetMVKernel.spv").to_vec()),
        (("RayTracingShader","TransferKernel",7),include_bytes!("shaders/main/RayTracingShader/TransferKernel.spv").to_vec()),
        (("RayTracingShader","TTtoOIDNKernel",8),include_bytes!("shaders/main/RayTracingShader/TTtoOIDNKernel.spv").to_vec()),
        (("RayTracingShader","TTtoOIDNKernelPanorama",9),include_bytes!("shaders/main/RayTracingShader/TTtoOIDNKernelPanorama.spv").to_vec()),
        (("ReSTIRGI","ReSTIRGIKernel",1),include_bytes!("shaders/main/ReSTIRGI/ReSTIRGIKernel.spv").to_vec()),
        (("ReSTIRGI","ReSTIRGISpatial",2),include_bytes!("shaders/main/ReSTIRGI/ReSTIRGISpatial.spv").to_vec()),
        (("ReSTIRGI","ReSTIRGISpatial2",3),include_bytes!("shaders/main/ReSTIRGI/ReSTIRGISpatial2.spv").to_vec()),
        (("BVHRefitter","BLASLightRefitKernel",1),include_bytes!("shaders/Utility/BVHRefitter/BLASLightRefitKernel.spv").to_vec()),
        (("BVHRefitter","BLASSGTreeRefitKernel",2),include_bytes!("shaders/Utility/BVHRefitter/BLASSGTreeRefitKernel.spv").to_vec()),
        (("BVHRefitter","Construct",3),include_bytes!("shaders/Utility/BVHRefitter/Construct.spv").to_vec()),
        (("BVHRefitter","RefitBVHLayer",4),include_bytes!("shaders/Utility/BVHRefitter/RefitBVHLayer.spv").to_vec()),
        (("BVHRefitter","RefitLayer",5),include_bytes!("shaders/Utility/BVHRefitter/RefitLayer.spv").to_vec()),
        (("BVHRefitter","TLASLightBVHRefitKernel",6),include_bytes!("shaders/Utility/BVHRefitter/TLASLightBVHRefitKernel.spv").to_vec()),
        (("BVHRefitter","TLASSGTreeRefitKernel",7),include_bytes!("shaders/Utility/BVHRefitter/TLASSGTreeRefitKernel.spv").to_vec()),
        (("BVHRefitter","TransferKernel",8),include_bytes!("shaders/Utility/BVHRefitter/TransferKernel.spv").to_vec()),
        (("BVHRefitter","UpdateGlobalBufferAABBKernel",9),include_bytes!("shaders/Utility/BVHRefitter/UpdateGlobalBufferAABBKernel.spv").to_vec()),
        (("CopyTextureShader","BC4Kernel",1),include_bytes!("shaders/Utility/CopyTextureShader/BC4Kernel.spv").to_vec()),
        (("CopyTextureShader","BC5Kernel",2),include_bytes!("shaders/Utility/CopyTextureShader/BC5Kernel.spv").to_vec()),
        (("CopyTextureShader","Compress",3),include_bytes!("shaders/Utility/CopyTextureShader/Compress.spv").to_vec()),
        (("CopyTextureShader","FullKernel",4),include_bytes!("shaders/Utility/CopyTextureShader/FullKernel.spv").to_vec()),
        (("CopyTextureShader","FullKernelSplit",5),include_bytes!("shaders/Utility/CopyTextureShader/FullKernelSplit.spv").to_vec()),
        (("CopyTextureShader","HeightmapCompressKernel",6),include_bytes!("shaders/Utility/CopyTextureShader/HeightmapCompressKernel.spv").to_vec()),
        (("CopyTextureShader","NormalMapKernel",7),include_bytes!("shaders/Utility/CopyTextureShader/NormalMapKernel.spv").to_vec()),
        (("CopyTextureShader","SingleChannelKernel",8),include_bytes!("shaders/Utility/CopyTextureShader/SingleChannelKernel.spv").to_vec()),
        (("GeneralMeshFunctions","CombineLightBuffers",1),include_bytes!("shaders/Utility/GeneralMeshFunctions/CombineLightBuffers.spv").to_vec()),
        (("GeneralMeshFunctions","CombineLightNodes",2),include_bytes!("shaders/Utility/GeneralMeshFunctions/CombineLightNodes.spv").to_vec()),
        (("GeneralMeshFunctions","CombineNodeBuffers",3),include_bytes!("shaders/Utility/GeneralMeshFunctions/CombineNodeBuffers.spv").to_vec()),
        (("GeneralMeshFunctions","CombineSGTreeNodes",4),include_bytes!("shaders/Utility/GeneralMeshFunctions/CombineSGTreeNodes.spv").to_vec()),
        (("GeneralMeshFunctions","CombineTriBuffers",5),include_bytes!("shaders/Utility/GeneralMeshFunctions/CombineTriBuffers.spv").to_vec()),
    ]);
    println!("Hello from init!");
    KERNELS.get_mut().replace(Box::leak(Box::new(Kernels {
        intersection_kernels: Some(KernelStruct {
            kernel_name_and_name_to_binding: HashMap::new(),
            kernel_to_bgl_and_shader_mod: HashMap::new(),
            name_to_globals_offset: HashMap::new(),
            kernel_index_to_name: HashMap::new(),
            globals: Vec::new(),
        }),
        raygen_kernels: Some(KernelStruct {
            kernel_name_and_name_to_binding: HashMap::new(),
            kernel_to_bgl_and_shader_mod: HashMap::new(),
            name_to_globals_offset: HashMap::new(),
            kernel_index_to_name: HashMap::new(),
            globals: Vec::new(),
        }),
        ray_tracing_shader: Some(KernelStruct {
            kernel_name_and_name_to_binding: HashMap::new(),
            kernel_to_bgl_and_shader_mod: HashMap::new(),
            name_to_globals_offset: HashMap::new(),
            kernel_index_to_name: HashMap::new(),
globals: Vec::new(),
        }),
        restir_gi: Some(KernelStruct {
            kernel_name_and_name_to_binding: HashMap::new(),
            kernel_to_bgl_and_shader_mod: HashMap::new(),
            name_to_globals_offset: HashMap::new(),
            kernel_index_to_name: HashMap::new(),
            globals: Vec::new(),
        }),
        bvh_refitter: Some(KernelStruct {
            kernel_name_and_name_to_binding: HashMap::new(),
            kernel_to_bgl_and_shader_mod: HashMap::new(),
            name_to_globals_offset: HashMap::new(),
            kernel_index_to_name: HashMap::new(),
            globals: Vec::new(),
        }),
        copy_texture_shader: Some(KernelStruct {
            kernel_name_and_name_to_binding: HashMap::new(),
            kernel_to_bgl_and_shader_mod: HashMap::new(),
            name_to_globals_offset: HashMap::new(),
            kernel_index_to_name: HashMap::new(),
            globals: Vec::new(),
        }),
        general_mesh_functions: Some(KernelStruct {
            kernel_name_and_name_to_binding: HashMap::new(),
            kernel_to_bgl_and_shader_mod: HashMap::new(),
            name_to_globals_offset: HashMap::new(),
            kernel_index_to_name: HashMap::new(),
            globals: Vec::new(),
        }),
    })));
    let mut binding = DEVICE.get_mut();
    let (device,queue) = binding.as_mut().unwrap();
    for ((group,kernel,index), mut shader) in shaders.clone() {
        let mut binding = KERNELS.get_mut();
        let kernels_struct = &mut **binding.as_mut().unwrap();
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
            "IntersectionKernels" => kernels_struct.intersection_kernels.as_mut().unwrap(),
            "RayGenKernels" => kernels_struct.raygen_kernels.as_mut().unwrap(),
            "RayTracingShader" => kernels_struct.ray_tracing_shader.as_mut().unwrap(),
            "ReSTIRGI" => kernels_struct.restir_gi.as_mut().unwrap(),
            "BVHRefitter" => kernels_struct.bvh_refitter.as_mut().unwrap(),
            "CopyTextureShader" => kernels_struct.copy_texture_shader.as_mut().unwrap(),
            "GeneralMeshFunctions" => kernels_struct.general_mesh_functions.as_mut().unwrap(),
            _ => unreachable!()
        };
        kernel_struct.kernel_index_to_name.insert(index,String::from(kernel));
        for reflect_binding in bindings {
            if reflect_binding.name == "$Globals" {
                global_binding = reflect_binding.binding;
              //  println!("$Global has {:#?} members",reflect_binding.block.members.len());
                for member in reflect_binding.block.members {
                    kernel_struct.name_to_globals_offset.insert(member.name.clone(), member.offset);
                    for _ in 0..member.size {
                        kernel_struct.globals.push(0);
                    }
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