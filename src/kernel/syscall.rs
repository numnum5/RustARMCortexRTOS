extern crate alloc;
use cortex_m_semihosting::{hprintln};
use core::arch::{naked_asm, asm};


pub enum SysCall {
    ALLOC,
    FREE,
}

// System call inteface.
#[no_mangle]
#[unsafe(naked)]
unsafe extern "C" fn SVCall() 
{
    naked_asm!(
        "push {{lr}}",
        "bl syscall_handler",
        "mov r4, r0", 
        "pop {{lr}}",
        "bx lr",
    );
}

#[allow(unused_variables)]
#[no_mangle]
pub extern "Rust" fn syscall_handler(
    service: SysCall,
    arg0: usize,
    arg1: usize,
    arg2: usize,
) {

    hprintln!("Handler");
    match service {
        SysCall::ALLOC => {
            hprintln!("ALLOC");
            
        },
        SysCall::FREE => {
            hprintln!("FREE");
        }
    }
    hprintln!("{}, {}, {}", arg0, arg1, arg2);
}

#[inline(always)]
pub fn svc_call(service: SysCall, arg0: usize, arg1: usize, arg2: usize) -> usize 
{
    hprintln!("Svc call");
    let ret: usize;
    unsafe {
        asm!(
            "svc 0",
            in("r0") service as usize,
            in("r1") arg0,
            in("r2") arg1,
            in("r3") arg2,
            lateout("r0") ret,        
            options(nostack),
        );
    }
    ret
}