use ::kern::memory::inactive::InactivePML4Table;
use ::kern::memory::stack_allocator::{Stack, StackAllocator};
use ::kern::memory::MemoryManager;
use ::kern::console::LogLevel::*;
use ::kern::console::{Console, tty1};
use core::sync::atomic::{AtomicUsize, Ordering};
use x86_64::instructions::interrupts;

use spin::Mutex;

pub type ProcId = isize;

#[derive(Debug, Clone, Copy)]
pub enum TaskState {
    Unused,
    Created,
    Ready,
    Running,
    Sleep,
    Zombie
}

#[derive(Debug, Clone, Copy)]
pub struct Context {
    pub rflags: usize,
    pub cr3: usize, // phyiscal address
    pub rbp: usize,
    pub rbx: usize,
    pub rsp: usize,
    pub r12: usize,
    pub r13: usize,
    pub r14: usize,
    pub r15: usize,
}

impl Context {
    pub const fn new() -> Context {
        Context {
            rflags: 0,
            cr3: 0, 
            rbp: 0, 
            rbx: 0, 
            rsp: 0, 
            r12: 0, 
            r13: 0, 
            r14: 0, 
            r15: 0, 
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Task {
    pub pid: ProcId,
    pub ppid: ProcId,
    pub name: Box<[char]>,
    pub cr3: Option<InactivePML4Table>,
    pub kern_stack: Option<Stack>,
    pub ctx: Context,
    pub state: TaskState
}

impl Task {
    pub const fn empty() -> Task {
        Task {
            pid: 0,
            cr3: None,
            kern_stack: None,
            state: TaskState::Unused,
            ctx: Context::new()
        }
    }
}

pub const MAX_TASK: usize = 64;

pub struct TaskList {
    pub tasks: [Task; MAX_TASK], // item 0 is left as empty
    pub nr: usize
}

impl TaskList {
    pub const fn new() -> TaskList {
        TaskList {
            tasks: [Task::empty(); MAX_TASK],
            nr: 0
        }
    }
}

pub static mut TASKS: TaskList = TaskList::new();
pub static CURRENT_ID: AtomicUsize = AtomicUsize::new(0);

pub fn init(mm: &mut MemoryManager) {
    printk!(Info, "tasks init\n\r");
    use core::mem::size_of;
    use ::kern::arch::cpu;

    unsafe {
        let oflags = unsafe { cpu::push_flags() };

        let init_stack = mm.alloc_stack(1).expect("alloc init task stack failed\n\r");
        printk!(Info, "alloc init stack {:#x}\n\r", init_stack.bottom());

        let mut task;
        let rips = [
            idle as usize,
            test_thread as usize,
            test_thread2 as usize,
            test_thread3 as usize
        ];
        for (id, &rip) in rips.iter().enumerate() {
            let pid = id + 1;
            task = &mut TASKS.tasks[pid];
            task.pid = pid as isize;
            task.state = TaskState::Created;
            task.kern_stack = mm.alloc_stack(2);
            task.ctx = Context::new();

            let kern_rsp = task.kern_stack.as_ref().map(|st| st.top()).unwrap() - size_of::<usize>();
            task.ctx.rflags = 0x0202;
            task.ctx.rsp = kern_rsp;
            *(kern_rsp as *mut usize) = rip;

            task.cr3 = None; //share with kernel
            //printk!(Info, "{:?}\n\r", task);
        }

        TASKS.nr = rips.len();

        cpu::pop_flags(oflags);
    }


    CURRENT_ID.store(1, Ordering::Release);
    unsafe { start_tasking(&mut TASKS.tasks[1]); }

    printk!(Info, "tasks done\n\r");
}

pub fn idle() {
    loop {
        unsafe { asm!("hlt":::: "volatile"); }
    }
}

pub fn test_thread3() {
    let mut count = 0;
    let busy_wait = || {
        for _ in 1..30 {
            unsafe { asm!("hlt":::: "volatile"); }
        }
    };

    loop {
        Console::with(&tty1, 22, 0, || {
            printk!(Debug, "kernel thread 3: {}\n\r", count);
        });
        count += 1;
        busy_wait();
    }
}

pub fn test_thread2() {
    let mut count = 0;
    let busy_wait = || {
        for _ in 1..50 {
            unsafe { asm!("hlt":::: "volatile"); }
        }
    };

    loop {
        Console::with(&tty1, 21, 0, || {
            printk!(Debug, "kernel thread 2: {}\n\r", count);
        });
        count += 1;
        busy_wait();
    }
}

pub fn test_thread() {
    let mut count = 0;
    let busy_wait = || {
        for _ in 1..10 {
            unsafe { asm!("hlt":::: "volatile"); }
        }
    };

    loop {
        Console::with(&tty1, 20, 0, || {
            printk!(Debug, "kernel thread 1: {}\n\r", count);
        });
        count += 1;
        busy_wait();
    }
}


#[inline(never)]
#[naked]
pub unsafe extern "C" fn switch_to(current: &mut Task, next: &mut Task) {
    // save context
    asm!("pushfq; popq $0" : "=r"(current.ctx.rflags) ::"memory": "volatile");
    asm!("movq %rbp, $0"   : "=r"(current.ctx.rbp) ::"memory": "volatile");
    asm!("movq %rbx, $0"   : "=r"(current.ctx.rbx) ::"memory": "volatile");
    asm!("movq %r12, $0"   : "=r"(current.ctx.r12) ::"memory": "volatile");
    asm!("movq %r13, $0"   : "=r"(current.ctx.r13) ::"memory": "volatile");
    asm!("movq %r14, $0"   : "=r"(current.ctx.r14) ::"memory": "volatile");
    asm!("movq %r15, $0"   : "=r"(current.ctx.r15) ::"memory": "volatile");

    asm!("movq %rsp, $0"   : "=r"(current.ctx.rsp) ::"memory": "volatile");

    // load context
    asm!("pushq $0; popfq":: "r"(next.ctx.rflags) :"memory": "volatile");
    asm!("movq $0, %rbx"  :: "r"(next.ctx.rbx) :"memory": "volatile");
    asm!("movq $0, %r12"  :: "r"(next.ctx.r12) :"memory": "volatile");
    asm!("movq $0, %r13"  :: "r"(next.ctx.r13) :"memory": "volatile");
    asm!("movq $0, %r14"  :: "r"(next.ctx.r14) :"memory": "volatile");
    asm!("movq $0, %r15"  :: "r"(next.ctx.r15) :"memory": "volatile");

    asm!("movq $0, %rsp"  :: "r"(next.ctx.rsp) :"memory": "volatile");
    
    //NOTE: rbp is used by switch_to, to override rbp at the end
    asm!("movq $0, %rbp"  :: "r"(next.ctx.rbp) :"memory": "volatile");
}

#[inline(never)]
#[naked]
unsafe extern "C" fn start_tasking(next: &mut Task) {
    // load context
    asm!("pushq $0; popfq":: "r"(next.ctx.rflags) :"memory": "volatile");
    asm!("movq $0, %rbx"  :: "r"(next.ctx.rbx) :"memory": "volatile");
    asm!("movq $0, %r12"  :: "r"(next.ctx.r12) :"memory": "volatile");
    asm!("movq $0, %r13"  :: "r"(next.ctx.r13) :"memory": "volatile");
    asm!("movq $0, %r14"  :: "r"(next.ctx.r14) :"memory": "volatile");
    asm!("movq $0, %r15"  :: "r"(next.ctx.r15) :"memory": "volatile");

    asm!("movq $0, %rsp"  :: "r"(next.ctx.rsp) :"memory": "volatile");
    
    //NOTE: rbp is used by switch_to, to override rbp at the end
    asm!("movq $0, %rbp"  :: "r"(next.ctx.rbp) :"memory": "volatile");
}
