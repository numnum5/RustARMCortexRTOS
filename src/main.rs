#![no_std]
#![no_main]

use panic_halt as _; 
extern crate alloc;
use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use core::alloc::{GlobalAlloc, Layout};
use core::mem;
use core::arch::{naked_asm, asm};
use cortex_m_rt::exception;
use core::mem::MaybeUninit;


pub mod kernel;
use kernel::scheduler::Scheduler;
use kernel::allocator::LinkedListAllocator;
use kernel::allocator::Locked;
use kernel::thread::Tcb;
use kernel::thread::{StackFrameExtension, StackFrame};

#[repr(C)]
pub struct Stack {
    /// Pointer to the lowest address of the stack
    bottom: *mut u8,
    /// Stack size
    size: usize,
    /// Current stack pointer
    ptr: *mut usize,
}




#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> = Locked::new(LinkedListAllocator::new());

static mut SCHEDULER: MaybeUninit<Scheduler> = MaybeUninit::uninit();

extern "C" 
{
    static mut _heap_start: u8;
    static mut _heap_end:   u8;
}


fn task1(arg : *mut usize) -> !
{
    // hprintln!("Entering task1 function");
    let test_va : u128 = 12423123;
    loop {

    }
}

fn task2(arg : *mut usize) -> !
{
     let test_va : u128 = 12423123;
    // hprintln!("Entering task2 function");
    //  let scheduler =  unsafe {&mut *SCHEDULER.as_mut_ptr()};
    // for thread in scheduler.threads.iter_mut() 
    // {
    //     hprintln!("Thread id: {}, stack pointer: {:p}", thread.id, thread.sp);
    // }

    loop {

    }
}


fn task3(arg : *mut usize) -> !
{
     let test_va : u128 = 12423123;
    // hprintln!("Entering task3 function");
    //  let scheduler =  unsafe {&mut *SCHEDULER.as_mut_ptr()};
    // for thread in scheduler.threads.iter_mut() 
    // {
    //     hprintln!("Thread id: {}, stack pointer: {:p}", thread.id, thread.sp);
    // }

    loop {

    }
}

type TaskFn = fn(arg: *mut usize) -> !;

fn task_exit_error() -> ! {
    
    hprintln!("Task exited\n");

    loop {
        // panic, halt, or delete task
    }
}



fn start_first_task() -> () {
    let stack_ptr : *mut u32;
    unsafe {
        let scheduler =  &mut *SCHEDULER.as_mut_ptr();
        let current_thread = scheduler.threads.pop_front();

        if (current_thread.is_none())
        {
            return;
        }
        scheduler.current_thread = current_thread;
        stack_ptr = scheduler.current_thread.as_mut().unwrap().sp;
    }

    
    unsafe {
        asm!(
        "msr psp, r0",
        "movs r0, #2",
        "msr control, r0",
        "isb",
        "pop   {{r4-r11}}",
        "pop   {{r0-r3,r12,lr}}",   // force function entry
        "pop   {{pc}}",             // 'jump' to the task entry function we put on the stack
        in("r0") stack_ptr as u32,
        options(noreturn),
        )
    }
}


fn task_init(entry : TaskFn)
{
    unsafe {
        let layout = Layout::from_size_align(1024, size_of::<usize>()).expect("Invalid layout");;  
        let stack_ptr = ALLOCATOR.alloc(layout) as *mut usize;
        let mut highest_ptr = stack_ptr.offset(64);
        let mut stack_offset = mem::size_of::<StackFrame>() / mem::size_of::<usize>();

        // Top of the stack
        // xpsr
        //
        // ...
        // r0
        // r11
        // R4
        // Bottom of the stack

        let mut stack_frame2: &mut StackFrame = mem::transmute(&mut *highest_ptr.offset(-(stack_offset as isize)));
        // let mut r3_r12 = highest_ptr.offset(-(stack_offset as isize));
        stack_frame2.xpsr = 0x01000000;
        stack_frame2.lr = 0xFFFFFFFD;
        stack_frame2.pc = entry as u32;
        stack_frame2.r0 = 0;
        stack_frame2.r1 = 0;
        stack_frame2.r3 = 0;
        stack_frame2.r2 = 0;
        stack_frame2.r12 = 0;

        stack_offset += mem::size_of::<StackFrameExtension>() / mem::size_of::<usize>();
        let sp = highest_ptr.offset(-(stack_offset as isize)) as *mut u32;

        let scheduler =  &mut *SCHEDULER.as_mut_ptr();
        scheduler.id_counter += 1;
        scheduler.threads.push_back(Tcb::new(sp, scheduler.id_counter, 1));
    }
}

#[entry]
fn main() -> ! 
{    
    let mut peripheral = unsafe { cortex_m::Peripherals::steal() };
    peripheral.SYST.set_reload(200_000_000 - 1);
    peripheral.SYST.clear_current();
    peripheral.SYST.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    peripheral.SYST.enable_interrupt();
    peripheral.SYST.enable_counter();

    unsafe {
        SCHEDULER = MaybeUninit::new(Scheduler::new());
        peripheral
            .SCB
            .set_priority(cortex_m::peripheral::scb::SystemHandler::PendSV, 0xFF);
    }


    unsafe {

        ALLOCATOR.lock().init(&raw mut _heap_start as *mut u8 as usize, 4096);


        hprintln!("Task 1 pointer: {:p}", task2 as *mut u32);

        task_init(task1);
        task_init(task2);
        task_init(task3);

        // let tc = threads.pop_front();

        // if (tc.is_none())
        // {

        // }

        // let tc=  tc.unwrap();

        hprintln!("Returned to main somehow idfk.");

        start_first_task();
        // High memory
        // stackframe for r4 to r11 lives  
        // 
        //
        // stackframe for r0 to r3  and psxr, pc, lr, etc...
        // Low memory <--- our stack ptr points to atm

        // 
            
    }
    
    // cortex_m::peripheral::SCB::set_pendsv();

    // cortex_m::peripheral::SYST::set_reload(&mut self, value);

    debug::exit(debug::EXIT_SUCCESS);

    loop {
        // your code goes here
    }
}
