use std::any::Any;
use std::io::Write;
use std::ptr::{addr_of, addr_of_mut};
use winapi::shared::minwindef::{DWORD, LPVOID};
use winapi::um::errhandlingapi::AddVectoredExceptionHandler;
use winapi::um::memoryapi::VirtualProtect;
use winapi::um::minwinbase::EXCEPTION_BREAKPOINT;
use winapi::um::winnt::{PAGE_EXECUTE_READWRITE, PVECTORED_EXCEPTION_HANDLER};
use winapi::vc::excpt::{EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH};
use crate::log_file;

static mut orig: usize = 0;
static mut hook: usize = 0;
static mut og_byte: u8 = 0;
pub struct Hook<T,H> {
    orig: T,
    hook: H,
    original_byte: u8,
    handle: *mut std::ffi::c_void,
}
impl<T,H> Hook<T,H> {
    unsafe extern "system" fn exception_handler(info: winapi::um::winnt::PEXCEPTION_POINTERS) -> i32 {
        log_file.get_mut().write("ExceptionHandler\n".as_bytes()).unwrap();
        let record = (*info).ExceptionRecord;
        let code = (*record).ExceptionCode;
        let addr = (*record).ExceptionAddress;
        if code == EXCEPTION_BREAKPOINT {
            log_file.get_mut().write("Breakpoint!\n".as_bytes()).unwrap();
            if addr == orig as _ {
                (*(*info).ContextRecord).Rip = hook as u64;
                log_file.get_mut().write(format!("RCX: {:#x}\n",(*(*info).ContextRecord).Rcx).as_bytes()).unwrap();
                log_file.get_mut().write("We found the function!\n".as_bytes()).unwrap();
                let mut old_prot: DWORD = 0;
                VirtualProtect(orig as _,1,PAGE_EXECUTE_READWRITE,&mut old_prot);
                   // panic!("VirtualProtect failed");

                std::ptr::copy_nonoverlapping(&raw const og_byte,orig as _,1);
                VirtualProtect(orig as _,1,old_prot,&mut old_prot);
                return EXCEPTION_CONTINUE_EXECUTION;
            }
        }
        EXCEPTION_CONTINUE_SEARCH
    }
    pub fn new(addr: usize,hook_addr: usize)  {
        unsafe {
            let mut old_prot: DWORD = 0;
            hook = hook_addr;
            orig = addr;
            VirtualProtect(addr as _,1,PAGE_EXECUTE_READWRITE, &mut old_prot);
            let breakpnt = [0xCCu32];
            std::ptr::copy_nonoverlapping(addr as _,&raw mut og_byte,1);
            log_file.get_mut().write(format!("og_byte: {:#x}!\n",*(&raw const og_byte)).as_bytes()).unwrap();
            std::ptr::copy_nonoverlapping(breakpnt.as_ptr(),addr as _,breakpnt.len());
            VirtualProtect(addr as _,1,old_prot,&mut old_prot);
            let handle = AddVectoredExceptionHandler(1, Some(Self::exception_handler)) as *mut std::ffi::c_void;
        }
    }
}