use std::ffi::{c_char, c_void, CString};
use std::sync::Arc;
use spirv_cross2::spirv::Capability::Kernel;
use wgpu::{BindGroupDescriptor, BindGroupEntry, BindingResource, BufferBinding, ComputePassDescriptor, ComputePipelineDescriptor, PipelineLayoutDescriptor, TextureDescriptor};
use wgpu::custom::AsAny;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu_types::{BufferAddress, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d, TextureFormat, TextureUsages, TextureViewDescriptor};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12::ID3D12Resource;
use crate::DEVICE;
use crate::render::KERNELS;
use crate::shaders::KernelStruct;

pub struct ComputeShader<'a> {
    pub(crate) inner_shader: KernelStruct,
    pub(crate) bind_group_entries: Vec<BindGroupEntry<'a>>
}

#[unsafe(no_mangle)]
pub extern "C" fn ComputeShader_Load(shader: *mut c_char) -> *mut ComputeShader<'static> {
    let shader_binding = unsafe {CString::from_raw(shader)};
    let shader_str = shader_binding.to_str().unwrap();
    let mut binding = KERNELS.get_mut();
    let kernel_structs = binding.as_mut().unwrap();
    let strct = Box::new(ComputeShader {
        inner_shader: match shader_str {
            "MainCompute/IntersectionKernels" => kernel_structs.intersection_kernels.take().unwrap(),
            "MainCompute/RayGenKernels" => kernel_structs.raygen_kernels.take().unwrap(),
            "MainCompute/RayTracingShader" => kernel_structs.ray_tracing_shader.take().unwrap(),
            "MainCompute/ReSTIRGI" => kernel_structs.restir_gi.take().unwrap(),
            "Utility/GeneralMeshFunctions" => kernel_structs.general_mesh_functions.take().unwrap(),
            "Utility/CopyTextureShader" => kernel_structs.copy_texture_shader.take().unwrap(),
            "Utility/BVHRefitter" => kernel_structs.bvh_refitter.take().unwrap(),
            _ => unimplemented!()
        },
        bind_group_entries: Vec::new(),
    });
  //  KERNELS.get_mut().replace(binding);
    Box::into_raw(strct)
}

#[unsafe(no_mangle)]
pub extern "C" fn ComputeShader_Dispatch(_self: *mut ComputeShader,kernel_index: u32,x: u32,y: u32,z: u32) {
    let mut self_to_use = unsafe {Box::from_raw(_self)};
    let mut device_binding = DEVICE.get_mut();
    let (device,queue) = device_binding.as_mut().unwrap();
    let kernel_name = self_to_use.inner_shader.kernel_index_to_name.get(&kernel_index).unwrap().clone();
    let (bgl,module) = self_to_use.inner_shader.kernel_to_bgl_and_shader_mod.get(&kernel_name).unwrap();
    let globals_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("$Globals"),
        contents: &self_to_use.inner_shader.globals,
        usage:  BufferUsages::UNIFORM,
    });
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
    let compute_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[bgl],
        push_constant_ranges: &[],
    });
    let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: None,
        layout: Some(&compute_pipeline_layout),
        module,
        entry_point: None,
        compilation_options: Default::default(),
        cache: None,
    });
    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: bgl,
        entries: &self_to_use.bind_group_entries
    });
    {
        let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, Some(&bind_group), &[]);
        cpass.dispatch_workgroups(x,y,z);
    }
    queue.submit(Some(encoder.finish()));
    unsafe {Box::into_raw(self_to_use)};
}

#[unsafe(no_mangle)]
pub extern "C" fn ComputeShader_DispatchIndirect(_self: *mut ComputeShader,kernel_index: u32,buf: *mut c_void,buf_size: usize) {
    let mut device_binding = DEVICE.get_mut();
    let (device,queue) = device_binding.as_mut().unwrap();
    let indirect_buf_wgpu_hal = unsafe {wgpu_hal::dx12::Device::buffer_from_raw(ID3D12Resource::from_raw(buf), buf_size as BufferAddress)};
    let indirect_buf_wgpu = unsafe {device.create_buffer_from_hal::<wgpu_hal::dx12::Api>(indirect_buf_wgpu_hal,&BufferDescriptor {
        label: None,
        size: buf_size as BufferAddress,
        usage: BufferUsages::INDIRECT,
        mapped_at_creation: false,
    })};

}
#[unsafe(no_mangle)]
pub extern "C" fn ComputeShader_FindKernel(_self: *mut ComputeShader,name: *mut c_char) -> u32 {
    let mut self_to_use = unsafe {Box::from_raw(_self)};
    let name_str = unsafe {CString::from_raw(name)}.into_string().unwrap();
    let index = *self_to_use.inner_shader.kernel_index_to_name.keys().find(|ind| {
        self_to_use.inner_shader.kernel_index_to_name.get(*ind).unwrap().clone() == name_str
    }).unwrap();
    unsafe {Box::into_raw(self_to_use)};
    index
}

#[unsafe(no_mangle)]
pub extern "C" fn ComputeShader_SetBool(_self: *mut ComputeShader,name: *mut c_char,val: bool) {
    let mut self_to_use = unsafe {Box::from_raw(_self)};
    let name_str = unsafe {CString::from_raw(name)}.into_string().unwrap();
    let offset = *self_to_use.inner_shader.name_to_globals_offset.get(&name_str).unwrap() as usize;
    let val_to_set = val as u32;
    self_to_use.inner_shader.globals[offset..offset+4].copy_from_slice(&val_to_set.to_le_bytes());
    unsafe {Box::into_raw(self_to_use)};
}

#[unsafe(no_mangle)]
pub extern "C" fn ComputeShader_SetBuffer(_self: *mut ComputeShader,kernel_index: u32,name: *mut c_char,buf: *mut c_void,buf_size: usize) {
    let mut self_to_use = unsafe {Box::from_raw(_self)};
    let name_str = unsafe {CString::from_raw(name)}.into_string().unwrap();
    let mut device_binding = DEVICE.get_mut();
    let (device,queue) = device_binding.as_mut().unwrap();
    let buf_wgpu_hal = unsafe {wgpu_hal::dx12::Device::buffer_from_raw(ID3D12Resource::from_raw(buf), buf_size as BufferAddress)};
    let buf_wgpu = unsafe {device.create_buffer_from_hal::<wgpu_hal::dx12::Api>(buf_wgpu_hal,&BufferDescriptor {
        label: None,
        size: buf_size as BufferAddress,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })};
    let binding = *self_to_use.inner_shader.kernel_name_and_name_to_binding.get(&(self_to_use.inner_shader.kernel_index_to_name.get(&kernel_index).unwrap().clone(),name_str)).unwrap();
    self_to_use.bind_group_entries.push(BindGroupEntry {
        binding,
        resource: buf_wgpu.as_entire_binding(),
    });
    unsafe {Box::into_raw(self_to_use)};
}

#[unsafe(no_mangle)]
pub extern "C" fn ComputeShader_SetFloat(_self: *mut ComputeShader,name: *mut c_char,val: f32) {
    let mut self_to_use = unsafe {Box::from_raw(_self)};
    let name_str = unsafe {CString::from_raw(name)}.into_string().unwrap();
    let offset = *self_to_use.inner_shader.name_to_globals_offset.get(&name_str).unwrap() as usize;
    self_to_use.inner_shader.globals[offset..offset+4].copy_from_slice(&val.to_le_bytes());
    unsafe {Box::into_raw(self_to_use)};
}

#[unsafe(no_mangle)]
pub extern "C" fn ComputeShader_SetInt(_self: *mut ComputeShader,name: *mut c_char,val: i32) {
    let mut self_to_use = unsafe {Box::from_raw(_self)};
    let name_str = unsafe {CString::from_raw(name)}.into_string().unwrap();
    let offset = *self_to_use.inner_shader.name_to_globals_offset.get(&name_str).unwrap() as usize;
    self_to_use.inner_shader.globals[offset..offset+4].copy_from_slice(&val.to_le_bytes());
    unsafe {Box::into_raw(self_to_use)};
}

#[unsafe(no_mangle)]
pub extern "C" fn ComputeShader_SetMatrix(_self: *mut ComputeShader,name: *mut c_char,mat: *const u8) {
    let mut self_to_use = unsafe {Box::from_raw(_self)};
    let name_str = unsafe {CString::from_raw(name)}.into_string().unwrap();
    let mat_slice = unsafe {std::slice::from_raw_parts(mat,16 * 4)};
    let offset = *self_to_use.inner_shader.name_to_globals_offset.get(&name_str).unwrap() as usize;
    self_to_use.inner_shader.globals[offset..offset+(16 * 4)].copy_from_slice(mat_slice);
    unsafe {Box::into_raw(self_to_use)};
}

#[unsafe(no_mangle)]
pub extern "C" fn ComputeShader_SetTexture(_self: *mut ComputeShader,kernel_index: u32,name: *mut c_char,tex: *mut c_void, width: u32, height: u32,format: u32,dimension: i32,mipCnt: u32) {
    let mut self_to_use = unsafe {Box::from_raw(_self)};
    let name_str = unsafe {CString::from_raw(name)}.into_string().unwrap();
    let kernel_name = self_to_use.inner_shader.kernel_index_to_name.get(&kernel_index).unwrap().clone();
    let binding = *self_to_use.inner_shader.kernel_name_and_name_to_binding.get(&(kernel_name,name_str)).unwrap();
    let mut device_binding = DEVICE.get_mut();
    let (device,queue) = device_binding.as_mut().unwrap();
    let unity_tex_fmt: UnityTextureEnum = unsafe {std::mem::transmute(format)};
    let unity_dim: TextureDimension = unsafe {std::mem::transmute(dimension)};
    let tex_wgpu_hal = unsafe {wgpu_hal::dx12::Device::texture_from_raw(ID3D12Resource::from_raw(tex), TextureFormat::try_from(unity_tex_fmt).unwrap(),wgpu_types::TextureDimension::try_from(unity_dim).unwrap(),Extent3d {
        width,
        height,
        depth_or_array_layers: 1
    },mipCnt,1)};

    let tex_wgpu = unsafe {device.create_texture_from_hal::<wgpu_hal::dx12::Api>(tex_wgpu_hal,&TextureDescriptor {
        mip_level_count: mipCnt,
        label: None,
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1
        },
        sample_count: 1,
        dimension: wgpu_types::TextureDimension::try_from(unity_dim).unwrap(),
        format: TextureFormat::try_from(unity_tex_fmt).unwrap(),
        usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC | TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })};
    let view = tex_wgpu.create_view(&TextureViewDescriptor::default());
    self_to_use.bind_group_entries.push(BindGroupEntry {
        binding,
        resource: BindingResource::TextureView(
            &view,
        )
    });
    unsafe {Box::into_raw(self_to_use)};
}


#[unsafe(no_mangle)]
pub extern "C" fn ComputeShader_SetVector(_self: *mut ComputeShader,name: *mut c_char,val: *const u8) {
    let mut self_to_use = unsafe {Box::from_raw(_self)};
    let name_str = unsafe {CString::from_raw(name)}.into_string().unwrap();
    let vec_slice = unsafe {std::slice::from_raw_parts(val,16)};
    let offset = *self_to_use.inner_shader.name_to_globals_offset.get(&name_str).unwrap() as usize;
    self_to_use.inner_shader.globals[offset..offset+16].copy_from_slice(vec_slice);
    unsafe {Box::into_raw(self_to_use)};
}
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureDimension
{
    Unknown = -1, // 0xFFFFFFFF
    None = 0,
    Any = 1,
    Tex2D = 2,
    Tex3D = 3,
    Cube = 4,
    Tex2DArray = 5,
    CubeArray = 6,
}
impl TryFrom<TextureDimension> for wgpu_types::TextureDimension {
    type Error = &'static str;
    fn try_from(value: TextureDimension) -> Result<Self, Self::Error> {
        match value {
            TextureDimension::Tex2D => Ok(wgpu_types::TextureDimension::D2),
            TextureDimension::Tex3D => Ok(wgpu_types::TextureDimension::D3),
            _ => Err("Invalid texture dimension"),
        }
    }
}
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnityTextureEnum {
    None = 0,
    R8_SRGB = 1,
    R8G8_SRGB = 2,
    R8G8B8_SRGB = 3,
    R8G8B8A8_SRGB = 4,
    R8_UNorm = 5,
    R8G8_UNorm = 6,
    R8G8B8_UNorm = 7,
    R8G8B8A8_UNorm = 8,
    R8_SNorm = 9,
    R8G8_SNorm = 10,
    R8G8B8_SNorm = 11,
    R8G8B8A8_SNorm = 12,
    R8_UInt = 13,
    R8G8_UInt = 14,
    R8G8B8_UInt = 15,
    R8G8B8A8_UInt = 16,
    R8_SInt = 17,
    R8G8_SInt = 18,
    R8G8B8_SInt = 19,
    R8G8B8A8_SInt = 20,
    R16_UNorm = 21,
    R16G16_UNorm = 22,
    R16G16B16_UNorm = 23,
    R16G16B16A16_UNorm = 24,
    R16_SNorm = 25,
    R16G16_SNorm = 26,
    R16G16B16_SNorm = 27,
    R16G16B16A16_SNorm = 28,
    R16_UInt = 29,
    R16G16_UInt = 30,
    R16G16B16_UInt = 31,
    R16G16B16A16_UInt = 32,
    R16_SInt = 33,
    R16G16_SInt = 34,
    R16G16B16_SInt = 35,
    R16G16B16A16_SInt = 36,
    R32_UInt = 37,
    R32G32_UInt = 38,
    R32G32B32_UInt = 39,
    R32G32B32A32_UInt = 40,
    R32_SInt = 41,
    R32G32_SInt = 42,
    R32G32B32_SInt = 43,
    R32G32B32A32_SInt = 44,
    R16_SFloat = 45,
    R16G16_SFloat = 46,
    R16G16B16_SFloat = 47,
    R16G16B16A16_SFloat = 48,
    R32_SFloat = 49,
    R32G32_SFloat = 50,
    R32G32B32_SFloat = 51,
    R32G32B32A32_SFloat = 52,
    B8G8R8_SRGB = 56,
    B8G8R8A8_SRGB = 57,
    B8G8R8_UNorm = 58,
    B8G8R8A8_UNorm = 59,
    B8G8R8_SNorm = 60,
    B8G8R8A8_SNorm = 61,
    B8G8R8_UInt = 62,
    B8G8R8A8_UInt = 63,
    B8G8R8_SInt = 64,
    B8G8R8A8_SInt = 65,
    R4G4B4A4_UNormPack16 = 66,
    B4G4R4A4_UNormPack16 = 67,
    R5G6B5_UNormPack16 = 68,
    B5G6R5_UNormPack16 = 69,
    R5G5B5A1_UNormPack16 = 70,
    B5G5R5A1_UNormPack16 = 71,
    A1R5G5B5_UNormPack16 = 72,
    E5B9G9R9_UFloatPack32 = 73,
    B10G11R11_UFloatPack32 = 74,
    A2B10G10R10_UNormPack32 = 75,
    A2B10G10R10_UIntPack32 = 76,
    A2B10G10R10_SIntPack32 = 77,
    A2R10G10B10_UNormPack32 = 78,
    A2R10G10B10_UIntPack32 = 79,
    A2R10G10B10_SIntPack32 = 80,
    A2R10G10B10_XRSRGBPack32 = 81,
    A2R10G10B10_XRUNormPack32 = 82,
    R10G10B10_XRSRGBPack32 = 83,
    R10G10B10_XRUNormPack32 = 84,
    A10R10G10B10_XRSRGBPack32 = 85,
    A10R10G10B10_XRUNormPack32 = 86,
    RGBA_DXT1_SRGB = 96,
    RGBA_DXT1_UNorm = 97,
    RGBA_DXT3_SRGB = 98,
    RGBA_DXT3_UNorm = 99,
    RGBA_DXT5_SRGB = 100,
    RGBA_DXT5_UNorm = 101,
    R_BC4_UNorm = 102,
    R_BC4_SNorm = 103,
    RG_BC5_UNorm = 104,
    RG_BC5_SNorm = 105,
    RGB_BC6H_UFloat = 106,
    RGB_BC6H_SFloat = 107,
    RGBA_BC7_SRGB = 108,
    RGBA_BC7_UNorm = 109,
    RGB_PVRTC_2Bpp_SRGB = 110,
    RGB_PVRTC_2Bpp_UNorm = 111,
    RGB_PVRTC_4Bpp_SRGB = 112,
    RGB_PVRTC_4Bpp_UNorm = 113,
    RGBA_PVRTC_2Bpp_SRGB = 114,
    RGBA_PVRTC_2Bpp_UNorm = 115,
    RGBA_PVRTC_4Bpp_SRGB = 116,
    RGBA_PVRTC_4Bpp_UNorm = 117,
    RGB_ETC_UNorm = 118,
    RGB_ETC2_SRGB = 119,
    RGB_ETC2_UNorm = 120,
    RGB_A1_ETC2_SRGB = 121,
    RGB_A1_ETC2_UNorm = 122,
    RGBA_ETC2_SRGB = 123,
    RGBA_ETC2_UNorm = 124,
    R_EAC_UNorm = 125,
    R_EAC_SNorm = 126,
    RG_EAC_UNorm = 127,
    RG_EAC_SNorm = 128,
    RGBA_ASTC4X4_SRGB = 129,
    RGBA_ASTC4X4_UNorm = 130,
    RGBA_ASTC5X5_SRGB = 131,
    RGBA_ASTC5X5_UNorm = 132,
    RGBA_ASTC6X6_SRGB = 133,
    RGBA_ASTC6X6_UNorm = 134,
    RGBA_ASTC8X8_SRGB = 135,
    RGBA_ASTC8X8_UNorm = 136,
    RGBA_ASTC10X10_SRGB = 137,
    RGBA_ASTC10X10_UNorm = 138,
    RGBA_ASTC12X12_SRGB = 139,
    RGBA_ASTC12X12_UNorm = 140,
}

impl TryFrom<UnityTextureEnum> for TextureFormat {
    type Error = &'static str;

    fn try_from(value: UnityTextureEnum) -> Result<Self, Self::Error> {
        match value {
          //  UnityTextureEnum::R8_SRGB => Ok(TextureFormat::R8UnormSrgb),
            UnityTextureEnum::R8G8B8A8_SRGB => Ok(TextureFormat::Rgba8UnormSrgb),
            UnityTextureEnum::R8_UNorm => Ok(TextureFormat::R8Unorm),
            UnityTextureEnum::R8G8_UNorm => Ok(TextureFormat::Rg8Unorm),
            UnityTextureEnum::R8G8B8A8_UNorm => Ok(TextureFormat::Rgba8Unorm),
            UnityTextureEnum::R8_SNorm => Ok(TextureFormat::R8Snorm),
            UnityTextureEnum::R8G8_SNorm => Ok(TextureFormat::Rg8Snorm),
            UnityTextureEnum::R8G8B8A8_SNorm => Ok(TextureFormat::Rgba8Snorm),
            UnityTextureEnum::R8_UInt => Ok(TextureFormat::R8Uint),
            UnityTextureEnum::R8G8_UInt => Ok(TextureFormat::Rg8Uint),
            UnityTextureEnum::R8G8B8A8_UInt => Ok(TextureFormat::Rgba8Uint),
            UnityTextureEnum::R8_SInt => Ok(TextureFormat::R8Sint),
            UnityTextureEnum::R8G8_SInt => Ok(TextureFormat::Rg8Sint),
            UnityTextureEnum::R8G8B8A8_SInt => Ok(TextureFormat::Rgba8Sint),
            UnityTextureEnum::R16_UNorm => Ok(TextureFormat::R16Unorm),
            UnityTextureEnum::R16G16_UNorm => Ok(TextureFormat::Rg16Unorm),
            UnityTextureEnum::R16G16B16A16_UNorm => Ok(TextureFormat::Rgba16Unorm),
            UnityTextureEnum::R16_SNorm => Ok(TextureFormat::R16Snorm),
            UnityTextureEnum::R16G16_SNorm => Ok(TextureFormat::Rg16Snorm),
            UnityTextureEnum::R16G16B16A16_SNorm => Ok(TextureFormat::Rgba16Snorm),
            UnityTextureEnum::R16_UInt => Ok(TextureFormat::R16Uint),
            UnityTextureEnum::R16G16_UInt => Ok(TextureFormat::Rg16Uint),
            UnityTextureEnum::R16G16B16A16_UInt => Ok(TextureFormat::Rgba16Uint),
            UnityTextureEnum::R16_SInt => Ok(TextureFormat::R16Sint),
            UnityTextureEnum::R16G16_SInt => Ok(TextureFormat::Rg16Sint),
            UnityTextureEnum::R16G16B16A16_SInt => Ok(TextureFormat::Rgba16Sint),
            UnityTextureEnum::R32_UInt => Ok(TextureFormat::R32Uint),
            UnityTextureEnum::R32G32_UInt => Ok(TextureFormat::Rg32Uint),
            UnityTextureEnum::R32G32B32A32_UInt => Ok(TextureFormat::Rgba32Uint),
            UnityTextureEnum::R32_SInt => Ok(TextureFormat::R32Sint),
            UnityTextureEnum::R32G32_SInt => Ok(TextureFormat::Rg32Sint),
            UnityTextureEnum::R32G32B32A32_SInt => Ok(TextureFormat::Rgba32Sint),
            UnityTextureEnum::R16_SFloat => Ok(TextureFormat::R16Float),
            UnityTextureEnum::R16G16_SFloat => Ok(TextureFormat::Rg16Float),
            UnityTextureEnum::R16G16B16A16_SFloat => Ok(TextureFormat::Rgba16Float),
            UnityTextureEnum::R32_SFloat => Ok(TextureFormat::R32Float),
            UnityTextureEnum::R32G32_SFloat => Ok(TextureFormat::Rg32Float),
            UnityTextureEnum::R32G32B32A32_SFloat => Ok(TextureFormat::Rgba32Float),
            UnityTextureEnum::B8G8R8A8_SRGB => Ok(TextureFormat::Bgra8UnormSrgb),
            UnityTextureEnum::B8G8R8A8_UNorm => Ok(TextureFormat::Bgra8Unorm),
           // UnityTextureEnum::B10G11R11_UFloatPack32 => Ok(TextureFormat::Rg11b10Float),
            UnityTextureEnum::A2B10G10R10_UNormPack32 => Ok(TextureFormat::Rgb10a2Unorm),
            UnityTextureEnum::RGBA_DXT1_SRGB => Ok(TextureFormat::Bc1RgbaUnormSrgb),
            UnityTextureEnum::RGBA_DXT1_UNorm => Ok(TextureFormat::Bc1RgbaUnorm),
            UnityTextureEnum::RGBA_DXT3_SRGB => Ok(TextureFormat::Bc2RgbaUnormSrgb),
            UnityTextureEnum::RGBA_DXT3_UNorm => Ok(TextureFormat::Bc2RgbaUnorm),
            UnityTextureEnum::RGBA_DXT5_SRGB => Ok(TextureFormat::Bc3RgbaUnormSrgb),
            UnityTextureEnum::RGBA_DXT5_UNorm => Ok(TextureFormat::Bc3RgbaUnorm),
            UnityTextureEnum::R_BC4_UNorm => Ok(TextureFormat::Bc4RUnorm),
            UnityTextureEnum::R_BC4_SNorm => Ok(TextureFormat::Bc4RSnorm),
            UnityTextureEnum::RG_BC5_UNorm => Ok(TextureFormat::Bc5RgUnorm),
            UnityTextureEnum::RG_BC5_SNorm => Ok(TextureFormat::Bc5RgSnorm),
            UnityTextureEnum::RGB_BC6H_UFloat => Ok(TextureFormat::Bc6hRgbUfloat),
      //      UnityTextureEnum::RGB_BC6H_SFloat => Ok(TextureFormat::Bc6hRgbSfloat),
            UnityTextureEnum::RGBA_BC7_SRGB => Ok(TextureFormat::Bc7RgbaUnormSrgb),
            UnityTextureEnum::RGBA_BC7_UNorm => Ok(TextureFormat::Bc7RgbaUnorm),
            UnityTextureEnum::RGB_ETC2_SRGB => Ok(TextureFormat::Etc2Rgb8UnormSrgb),
            UnityTextureEnum::RGB_ETC2_UNorm => Ok(TextureFormat::Etc2Rgb8Unorm),
            UnityTextureEnum::RGB_A1_ETC2_SRGB => Ok(TextureFormat::Etc2Rgb8A1UnormSrgb),
            UnityTextureEnum::RGB_A1_ETC2_UNorm => Ok(TextureFormat::Etc2Rgb8A1Unorm),
            UnityTextureEnum::RGBA_ETC2_SRGB => Ok(TextureFormat::Etc2Rgba8UnormSrgb),
            UnityTextureEnum::RGBA_ETC2_UNorm => Ok(TextureFormat::Etc2Rgba8Unorm),
            UnityTextureEnum::R_EAC_UNorm => Ok(TextureFormat::EacR11Unorm),
            UnityTextureEnum::R_EAC_SNorm => Ok(TextureFormat::EacR11Snorm),
            UnityTextureEnum::RG_EAC_UNorm => Ok(TextureFormat::EacRg11Unorm),
            UnityTextureEnum::RG_EAC_SNorm => Ok(TextureFormat::EacRg11Snorm),
          //  UnityTextureEnum::RGBA_ASTC4X4_SRGB => Ok(TextureFormat::Astc4x4RgbaUnormSrgb),
          //  UnityTextureEnum::RGBA_ASTC4X4_UNorm => Ok(TextureFormat::Astc4x4RgbaUnorm),
     //       UnityTextureEnum::RGBA_ASTC5X5_SRGB => Ok(TextureFormat::Astc5x5RgbaUnormSrgb),
     //       UnityTextureEnum::RGBA_ASTC5X5_UNorm => Ok(TextureFormat::Astc5x5RgbaUnorm),
     //       UnityTextureEnum::RGBA_ASTC6X6_SRGB => Ok(TextureFormat::Astc6x6RgbaUnormSrgb),
      //      UnityTextureEnum::RGBA_ASTC6X6_UNorm => Ok(TextureFormat::Astc6x6RgbaUnorm),
      //      UnityTextureEnum::RGBA_ASTC8X8_SRGB => Ok(TextureFormat::Astc8x8RgbaUnormSrgb),
       //     UnityTextureEnum::RGBA_ASTC8X8_UNorm => Ok(TextureFormat::Astc8x8RgbaUnorm),
       //     UnityTextureEnum::RGBA_ASTC10X10_SRGB => Ok(TextureFormat::Astc10x10RgbaUnormSrgb),
        //    UnityTextureEnum::RGBA_ASTC10X10_UNorm => Ok(TextureFormat::Astc10x10RgbaUnorm),
        //    UnityTextureEnum::RGBA_ASTC12X12_SRGB => Ok(TextureFormat::Astc12x12RgbaUnormSrgb),
         //   UnityTextureEnum::RGBA_ASTC12X12_UNorm => Ok(TextureFormat::Astc12x12RgbaUnorm),
            // Explicitly handle unmapped formats
            _ => Err("Unsupported or unmapped UnityTextureEnum variant"),
        }
    }
}