use alloc::collections::linked_list::LinkedList;
use crate::kernel::thread::Tcb;
use core::arch::{naked_asm, asm};
pub struct Scheduler{
    pub current_thread : Option<Tcb>,
    pub threads : LinkedList<Tcb>,
    pub id_counter : usize
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            current_thread: None,
            threads : LinkedList::new(),
            id_counter : 0
        }
    }

}


#[no_mangle]
#[unsafe(naked)]
pub unsafe extern "C" fn PendSV() 
{
         naked_asm!(
         "push    {{lr}}",
        "mrs     r0, psp",         // get process stack pointer
         "stmdb   r0!, {{r4-r11}}", // push registers to stack A
         "bl      switch_context",  // call kernel for context switch
         "pop     {{lr}}",
         "mov     r3, #2",
        "msr     control, r3",      // run in unprivileged mode
         "isb",
         "ldmia   r0!, {{r4-r11}}",  // pop registers from stack B
         "msr     psp, r0",          // set process stack pointer
         "bx      lr"
         )
}