#![no_std]
#![no_main]

use cortex_m::singleton;
// pick a panicking behavior
use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
// use panic_abort as _; // requires nightly
// use panic_itm as _; // logs messages over ITM; requires ITM support
// use panic_semihosting as _; // logs messages to the host stderr; requires a debugger

use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use core::alloc::{GlobalAlloc, Layout};
use core::f32::consts::PI;
use core::ptr;
use core::mem;
use core::arch::{naked_asm, asm};
use alloc::alloc::{alloc, dealloc};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TaskContext {
    // Software-saved (PendSV)
    pub r4:  u32,
    pub r5:  u32,
    pub r6:  u32,
    pub r7:  u32,
    pub r8:  u32,
    pub r9:  u32,
    pub r10: u32,
    pub r11: u32,

    // Hardware-saved (exception entry)
    pub r0:   u32,
    pub r1:   u32,
    pub r2:   u32,
    pub r3:   u32,
    pub r12:  u32,
    pub lr:   u32,
    pub pc:   u32,
    pub xpsr: u32,
}

pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}


fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}



struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}
impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}


#[no_mangle]
pub static mut pxCurrentTCB: *mut Tcb = core::ptr::null_mut();
pub struct LinkedListAllocator {
    head: ListNode,
}

#[repr(C)]
pub struct Tcb {
    // First field MUST be the saved stack pointer (like FreeRTOS)
    pub top_of_stack: *mut u32,
    // ... other fields (priority, list nodes, etc.)
}


impl LinkedListAllocator {
    /// Creates an empty LinkedListAllocator.
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// This function is unsafe because the caller must guarantee that the given
    /// heap bounds are valid and that the heap is unused. This method must be
    /// called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe {
            self.add_free_region(heap_start, heap_size);
        }
    }

    /// Adds the given memory region to the front of the list.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // ensure that the freed region is capable of holding ListNode
        let aligned_address = align_up(addr, mem::align_of::<ListNode>());
        if size >= mem::size_of::<ListNode>()
        {
            // create a new list node and append it at the start of the list
            let mut node = ListNode::new(size);

            node.next = self.head.next.take();

            let node_ptr = aligned_address as *mut ListNode;

            unsafe {
                node_ptr.write(node);
                self.head.next = Some(&mut *node_ptr);
            }
        }

    }

    fn find_region(&mut self, size: usize, align: usize)
        -> Option<(&'static mut ListNode, usize)>
    {
        // reference to current list node, updated for each iteration
        let mut current = &mut self.head;
        // look for a large enough memory region in linked list
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                // region suitable for allocation -> remove node from list
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                // region not suitable -> continue with next region
                current = current.next.as_mut().unwrap();
            }
        }

        // no suitable region found
        None
    }

    /// Returns the allocation start address on success.
    fn alloc_from_region(region: &ListNode, size: usize, align: usize)
        -> Result<usize, ()>
    {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            // region too small
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            // rest of region too small to hold a ListNode (required because the
            // allocation splits the region in a used and a free part)
            return Err(());
        }

        // region suitable for allocation
        Ok(alloc_start)
    }
    
    /// Returns the adjusted size and alignment as a (size, align) tuple.
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            .pad_to_align();
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}

// unsafe fn alloc(Test : &mut LinkedListAllocator, layout: Layout) -> *mut u8 {
//         // perform layout adjustme

//         let (size, align) = LinkedListAllocator::size_align(layout);

//         if let Some((region, alloc_start)) = Test.find_region(size, align) {
//             let alloc_end = alloc_start.checked_add(size).expect("overflow");

//             hprintln!("{:X}", alloc_end);
            
            
//             let excess_size = region.end_addr() - alloc_end;
//             if excess_size > 0 {
//                 unsafe {
//                     Test.add_free_region(alloc_end, excess_size);
//                 }
//             }
//             return alloc_start as *mut u8;
//         } else {
//             return ptr::null_mut();
//         }
//     }


// unsafe fn dealloc(test : &mut LinkedListAllocator, ptr: *mut u8, layout: Layout) {
//     // perform layout adjustments
//     let (size, _) = LinkedListAllocator::size_align(layout);

//     unsafe { test.add_free_region(ptr as usize, size) }
// }


unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // perform layout adjustme

        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.lock();

        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");

        
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                unsafe {
                    allocator.add_free_region(alloc_end, excess_size);
                }
            }
            return alloc_start as *mut u8;
        } else {
            return ptr::null_mut();
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // perform layout adjustments
        let (size, _) = LinkedListAllocator::size_align(layout);

        unsafe { self.lock().add_free_region(ptr as usize, size) }
    }
}

extern crate alloc;
#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> = Locked::new(LinkedListAllocator::new());

use alloc::{boxed::Box, vec::Vec};


struct Nigger {


    name : usize,
    priority : usize
}

extern "C" {
    static mut _heap_start: u8;
    static mut _heap_end:   u8;
}


#[entry]
fn main() -> ! {

    let mut test : LinkedListAllocator = LinkedListAllocator::new();
    


    unsafe {

    ALLOCATOR.lock().init(&raw mut _heap_start as *mut u8 as usize, 4096);

    }

    hprintln!("{:p}", &raw mut _heap_start as *mut u8);

    unsafe {
        test.init(&raw mut _heap_start as *mut u8 as usize, 4096);
    }

    unsafe {

        // hprintln!("{}", mem::size_of::<Nigger>());
        let layout = Layout::from_size_align_unchecked(mem::size_of::<Nigger>(), mem::size_of::<usize>());

        let b = Box::new(49);

        // 

        
    }

    
    svc_call(SysCall::ALLOC, 1, 1, 1);
    // unsafe {
    //     ALLOCATOR.lock().init(0x20000000 as usize, 0x4096 as usize);
    // }
    unsafe {

        let layout = Layout::from_size_align_unchecked(mem::size_of::<TaskContext>(), mem::size_of::<usize>());

        let context = TaskContext {
                r4:  0,
                r5:  0,
                r6:  0,
                r7:  0,
                r8:  0,
                r9:  0,
                r10: 0,
                r11: 0,

                // Hardware-saved (exception entry)
                r0:   0,
                r1:   0,
                r2:   0,
                r3:   0,
                r12:  0,
                lr:   32,
                pc:   0,
                xpsr: 0,
        };

        

        let test = alloc(layout) as *mut TaskContext;

        ptr::write(test, context);
        // (*pxCurrentTCB).top_of_stack = test as *mut u32;
        
    }

    // hprintln!("{}", 1);
    // drop(b);
    
    // let mut v = Vec::new();
   
    // v.push(2);
    hprintln!("HEllow");

    cortex_m::peripheral::SCB::set_pendsv();
    
    debug::exit(debug::EXIT_SUCCESS);

    loop {
        // your code goes here
    }
}


pub enum SysCall {
    ALLOC,
    FREE,
}

#[inline(always)]
pub fn svc_call(service: SysCall, arg0: usize, arg1: usize, arg2: usize) -> usize {
    hprintln!("S");
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



#[no_mangle]
#[unsafe(naked)]
unsafe extern "C" fn SVCall() {
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


#[no_mangle]
pub unsafe extern "C" fn vTaskSwitchContext() {
    hprintln!("FUC");
    dump_current_task_registers();
}





pub unsafe fn dump_current_task_registers() {
    let tcb = pxCurrentTCB;
    if tcb.is_null() {

        hprintln!("NULL");
        return;
    }
   hprintln!("NOT NULL");
    // r4 is at top_of_stack
    let ctx = (*tcb).top_of_stack as *const TaskContext;
    let ctx = &*ctx;

    hprintln!("===== TASK REGISTER DUMP =====");
    hprintln!("r0  = 0x{:08x}", ctx.r0);
    hprintln!("r1  = 0x{:08x}", ctx.r1);
    hprintln!("r2  = 0x{:08x}", ctx.r2);
    hprintln!("r3  = 0x{:08x}", ctx.r3);
    hprintln!("r4  = 0x{:08x}", ctx.r4);
    hprintln!("r5  = 0x{:08x}", ctx.r5);
    hprintln!("r6  = 0x{:08x}", ctx.r6);
    hprintln!("r7  = 0x{:08x}", ctx.r7);
    hprintln!("r8  = 0x{:08x}", ctx.r8);
    hprintln!("r9  = 0x{:08x}", ctx.r9);
    hprintln!("r10 = 0x{:08x}", ctx.r10);
    hprintln!("r11 = 0x{:08x}", ctx.r11);
    hprintln!("r12 = 0x{:08x}", ctx.r12);
    hprintln!("lr  = 0x{:08x}", ctx.lr);
    hprintln!("pc  = 0x{:08x}", ctx.pc);
    hprintln!("xPSR= 0x{:08x}", ctx.xpsr);
}

#[no_mangle]
#[unsafe(naked)]
pub unsafe extern "C" fn PendSV() {

    // hprintln!("FACU");

    naked_asm!(
        // r0 = PSP (current task's stack pointer)
        "mrs r0, psp",
        "isb",

        // r3 = &pxCurrentTCB
        "ldr r3, ={pxCurrentTCB}",
        // r2 = pxCurrentTCB (pointer to current TCB)
        "ldr r2, [r3]",

        // Save r4–r11 onto current task's stack (PSP)
        "stmdb r0!, {{r4-r11}}",
        // Store updated PSP into TCB->top_of_stack
        "str r0, [r2]",

        // Save kernel working registers (r3 = &pxCurrentTCB, r14 = EXC_RETURN) on MSP
        "stmdb sp!, {{r3, r14}}",

        // Ensure BASEPRI = 0 (enable interrupts for scheduler if you use BASEPRI)
        "mov r0, #0",
        "msr basepri, r0",

        // Call C/Rust scheduler: updates pxCurrentTCB to next ready task
        "bl {vTaskSwitchContext}",

        // Again ensure BASEPRI = 0 after scheduler
        "mov r0, #0",
        "msr basepri, r0",

        // Restore r3 (&pxCurrentTCB) and r14 (EXC_RETURN) from MSP
        "ldmia sp!, {{r3, r14}}",

        // r1 = pxCurrentTCB (new current TCB)
        "ldr r1, [r3]",
        // r0 = new TCB->top_of_stack (saved PSP)
        "ldr r0, [r1]",

        // Restore r4–r11 from new task's stack
        "ldmia r0!, {{r4-r11}}",
        // Write PSP = new task stack pointer
        "msr psp, r0",
        "isb",

        // Exception return: hardware will pop r0–r3, r12, pc, lr, xPSR
        "bx r14",

        pxCurrentTCB = sym pxCurrentTCB,
        vTaskSwitchContext = sym vTaskSwitchContext
    );
}