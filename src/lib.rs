#![feature(internal_output_capture)]
#![recursion_limit = "512"]
#![feature(allocator_api)]
#![feature(panic_payload_as_str)]

mod render;
mod shaders;
mod compute_shader_interop;

use std::ffi::{c_void, CStr, CString, OsString};
use std::fs::File;
use std::io::{Read, Write};
use std::iter;
use std::ops::{Add, DerefMut};
use std::os::windows::ffi::OsStringExt;
use std::path::Prefix::Disk;
use std::ptr::{addr_of_mut, NonNull};
use std::sync::{Arc, LazyLock, Mutex, TryLockError};
use std::thread::JoinHandle;
use retour::static_detour;
use lazy_mut::LazyMut;
use stdio_override::StdoutOverride;
use wgpu::{BackendOptions, Backends, Device, Dx12BackendOptions, Dx12Compiler, DxcShaderModel, Features, InstanceDescriptor, InstanceFlags, Label, MemoryBudgetThresholds, Queue, RequestAdapterOptions};
use wgpu::wgt::DeviceDescriptor;
use wgpu_types::Limits;
use winapi::shared::minwindef::DWORD;
use winapi::um::libloaderapi::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use winapi::um::winnt::{DLL_PROCESS_ATTACH, HRESULT, LPCSTR, LPCWSTR, PCSTR, PCWSTR};
use windows::core::Interface;


static DEVICE: LazyMut<Option<(Device,Queue)>> = LazyMut::new(|| {
    None
});
static INSTANCE: LazyMut<Option<wgpu::Instance>> = LazyMut::new(|| None);
static ALREADY_INIT: LazyMut<bool> = LazyMut::new(|| false);
static WRITER_THREAD: LazyMut<Option<JoinHandle<()>>> = LazyMut::new(|| None);
static LOG_FILE: LazyMut<Option<File>> =  LazyMut::new(|| None);
static_detour! {
    static CreateGfxDevice: unsafe extern "win64" fn(i32,i32) -> *mut std::ffi::c_void;
    static LoadLibraryW_: unsafe extern "C" fn(LPCWSTR) -> *mut std::ffi::c_void;
    static CreateDevice: unsafe extern "system" fn(usize,i32,usize,usize) -> i32;
}
#[unsafe(no_mangle)]
unsafe extern "win64" fn CreateGfxDevice_hook(rend: i32,flags: i32) -> *mut std::ffi::c_void {
    println!("CreateGfxDevice!");
    let tramp: unsafe extern "C" fn(i32,i32) -> *mut std::ffi::c_void = std::mem::transmute(CreateGfxDevice.trampoline().unwrap());
    let res = tramp(0x12, flags);
   // log_file.get_mut().write(format!("Called original!: result: {:#x}\n",res as usize).as_bytes()).unwrap();

    res
}
#[unsafe(no_mangle)]
unsafe extern "system" fn GetFileVersionInfoSizeW(a: usize,b: usize) -> usize {
    //log_file.get_mut().write("I'm a stub!\n".as_bytes()).unwrap();
    0
}
#[unsafe(no_mangle)]
unsafe extern "system" fn GetFileVersionInfoSizeA(a: usize,b: usize) -> usize {
  // log_file.get_mut().write("I'm a stub!\n".as_bytes()).unwrap();

    0
}
#[unsafe(no_mangle)]
unsafe extern "system" fn DllMain(_: usize,_: i32, _:usize) -> i32 {
    if !*ALREADY_INIT.get_mut() {
        std::thread::spawn(move || {
            std::panic::set_hook(Box::new(|panic_info| {
                if let Ok(mut file) =  File::create("panic_log.txt") {
                    let _ = writeln!(file, "Panic occurred: {:?}", panic_info.payload_as_str().unwrap());
                    let location = panic_info.location().unwrap();
                    let _ = writeln!(file,"Occurred in file {} at line {}",location.file(),location.line());
                }
            }));
            /*
            let mut buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
            let strong_ref = Arc::clone(&buffer);
            std::io::set_output_capture(Some(buffer));
            let mut file = std::fs::OpenOptions::new().create(true).read(true).write(true).truncate(true).open("truetrace_nativelog.txt").unwrap();
            WRITER_THREAD.get_mut().replace(std::thread::spawn(move || {
                loop {
                    match strong_ref.try_lock() {
                        Ok(l) => {
                            file.set_len(0).unwrap();
                            file.write(l.as_slice()).unwrap();
                            file.flush().unwrap();
                        },
                        Err(TryLockError::Poisoned(err)) => panic!("{}", err),
                        Err(TryLockError::WouldBlock) => {
                            std::hint::spin_loop();
                            std::thread::yield_now();
                            continue
                        }
                    };
                    std::thread::sleep(std::time::Duration::from_millis(250));
                };
            }));
             */
            LOG_FILE.get_mut().replace(File::create("truetrace_nativelog.txt").unwrap());
            let module = "UnityPlayer.dll"
                .encode_utf16()
                .chain(iter::once(0))
                .collect::<Vec<u16>>();
            let mut handle = GetModuleHandleW(PCWSTR::from(module.as_ptr() as _)) as usize;
            while handle == 0 {
                handle = GetModuleHandleW(PCWSTR::from(module.as_ptr() as _)) as usize;
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            println!("handle: {:#x}", handle);
            let func_addr = handle.add(0x8a7c00);
            init_stuff(func_addr);
            let module_ = "d3d12.dll"
                .encode_utf16()
                .chain(iter::once(0))
                .collect::<Vec<u16>>();
            handle = GetModuleHandleW(PCWSTR::from(module_.as_ptr() as _)) as usize;
            if handle == 0 {
                handle = LoadLibraryW(PCWSTR::from(module_.as_ptr() as _)) as usize;
            }
            println!("d3d12 get!");
            let create_addr = get_module_symbol_address("d3d12.dll", "D3D12CreateDevice").unwrap();
            CreateDevice.initialize(std::mem::transmute(create_addr), |adapter_arg: usize, b, c, pDev: usize| {
                println!("Hook!");
                CreateDevice.disable().unwrap();
                //  let tramp: unsafe extern "system" fn(usize,usize,usize,usize) -> i32 = std::mem::transmute(CreateDevice.trampoline().unwrap());
                 INSTANCE.get_mut().replace(wgpu::Instance::new(&InstanceDescriptor {
                    backends: Backends::DX12,
                    flags: InstanceFlags::from_env_or_default(),
                    memory_budget_thresholds: Default::default(),
                    backend_options: BackendOptions {
                        dx12: Dx12BackendOptions {
                            shader_compiler: Dx12Compiler::DynamicDxc {
                                dxc_path: String::from("dxcompiler.dll"),
                                max_shader_model: DxcShaderModel::V6_7,
                            },
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }));
                let rt = tokio::runtime::Runtime::new().unwrap();
                let adapters = INSTANCE.get_mut().as_ref().unwrap().enumerate_adapters(Backends::DX12);
                let adapter = adapters.first().unwrap();
                println!("Adapter name: {:#?}",adapter.get_info().name);
            let dev = rt.block_on(adapter.request_device(&DeviceDescriptor {
                        label: None,
                        required_features: Features::EXPERIMENTAL_RAY_QUERY | Features::EXPERIMENTAL_PASSTHROUGH_SHADERS,
                        required_limits: Limits {
                            max_storage_buffers_per_shader_stage: 50,
                            max_acceleration_structures_per_shader_stage: 25,
                            max_storage_textures_per_shader_stage: 15,
                            ..Limits::defaults()
                        },
                        ..Default::default()
                    })).unwrap();
                DEVICE.get_mut().replace(dev);
                let hal = DEVICE.get_mut().as_ref().unwrap().0.as_hal::<wgpu_hal::dx12::Api>().unwrap();
                let raw = hal.raw_device().as_raw();
                *(pDev as *mut *mut std::ffi::c_void) = raw;
                println!("Created device & stuff");
                0
            }).unwrap();
            CreateDevice.enable().unwrap();
            println!("DllMain done!");
        });
        *ALREADY_INIT.get_mut() = true;
    }
    1
}
fn init_stuff(addr: usize) {
   unsafe { CreateGfxDevice.initialize(std::mem::transmute(addr),|rend,flags| {
        CreateGfxDevice_hook(rend,flags)
    }).unwrap()};
   unsafe { CreateGfxDevice.enable().unwrap()};
}
unsafe fn u16_ptr_to_string(ptr: *const u16) -> OsString {
    let len = (0..).take_while(|&i| *ptr.offset(i) != 0).count();
    let slice = std::slice::from_raw_parts(ptr, len);

    OsString::from_wide(slice)
}
fn get_module_symbol_address(module: &str, symbol: &str) -> Option<usize> {
    let module = module
        .encode_utf16()
        .chain(iter::once(0))
        .collect::<Vec<u16>>();
    let symbol = CString::new(symbol).unwrap();
    unsafe {
        let handle = GetModuleHandleW(PCWSTR::from(module.as_ptr() as _));
        match NonNull::new(GetProcAddress(handle.cast(), PCSTR::from(symbol.as_ptr() as _))) {
            Some(func) => Some(func.addr().get()),
            None => None,
        }
    }
}
