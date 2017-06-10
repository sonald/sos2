use ::kern::memory::inactive::InactivePML4Table;
use ::kern::memory::stack_allocator::{Stack, StackAllocator};
use ::kern::memory::{MemoryManager, MM};
use ::kern::memory::paging;
use ::kern::console::LogLevel::*;
use ::kern::console::{Console, tty1};
use ::kern::arch::cpu;

use core::sync::atomic::{AtomicIsize, Ordering};
use x86_64::instructions::interrupts;
use collections::string::{String, ToString};
use collections::BTreeMap;
use alloc::arc::Arc;
use core::ops::{Deref, DerefMut};

use spin::*;

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

/// for task 
#[derive(Debug, Clone)]
pub struct VirtualMemoryArea {
    pub start: usize,
    pub size: usize,
    pub mapped: bool,
    pub flags: paging::EntryFlags,
}

impl VirtualMemoryArea {
    pub fn new(start: usize, size: usize, flags: paging::EntryFlags) -> VirtualMemoryArea {
        assert!(!flags.contains(paging::PRESENT));

        VirtualMemoryArea {
            start: start,
            size: size,
            mapped: false,
            flags: flags
        }
    }

    pub fn map(&mut self, mm: &mut MemoryManager) {
    }

    pub fn unmap(&mut self, mm: &mut MemoryManager) {
    }
}


#[derive(Debug, Clone)]
pub struct Task {
    pub pid: ProcId,
    pub ppid: ProcId,
    pub name: Option<String>,
    pub cr3: Option<InactivePML4Table>,
    pub kern_stack: Option<Stack>,
    pub user_stack: Option<VirtualMemoryArea>,
    pub code: Option<VirtualMemoryArea>,
    pub ctx: Context,
    pub state: TaskState
}

impl Task {
    pub const fn empty() -> Task {
        Task {
            pid: 0,
            ppid: 0,
            name: None,
            cr3: None,
            kern_stack: None,
            user_stack: None,
            code: None,
            state: TaskState::Unused,
            ctx: Context::new()
        }
    }
}

pub const MAX_TASK: isize = 64;

type TaskMap = BTreeMap<ProcId, Arc<RwLock<Task>>>;

pub struct TaskList {
    pub tasks: TaskMap,
    pub next_id: ProcId,
}

impl TaskList {
    pub fn new() -> TaskList {
        TaskList {
            tasks: BTreeMap::new(),
            next_id: 1
        }
    }

    pub fn get() -> RwLockReadGuard<'static, TaskList> {
        TASKS.call_once(init_tasks).read()
    }

    pub fn get_mut() -> RwLockWriteGuard<'static, TaskList> {
        TASKS.call_once(init_tasks).write()
    }

    pub fn get_task(&self, id: ProcId) -> Option<&Arc<RwLock<Task>>> {
        self.tasks.get(&id)
    }

    pub fn current(&self) -> Option<&Arc<RwLock<Task>>> {
        self.get_task(CURRENT_ID.load(Ordering::SeqCst))
    }

    pub fn alloc_task(&mut self, name: &str, parent: ProcId, rip: usize) {
        use core::mem::size_of;

        let stack = {
            let mut mm = MM.try().unwrap().lock();
            mm.alloc_stack(2)
        };

        let pid = self.next_id;
        assert!(self.next_id < MAX_TASK, "task id exceeds maximum boundary");

        let mut task = Task::empty();
        task.pid = pid as isize;
        task.ppid = if pid > 1 {parent} else {0};
        task.name = Some(name.to_string());
        task.state = TaskState::Created;
        task.kern_stack = stack;
        task.ctx = Context::new();

        let kern_rsp = stack.as_ref().map(|st| st.top()).unwrap() - size_of::<usize>();
        task.ctx.rflags = 0x0202;
        task.ctx.rsp = kern_rsp;
        unsafe {
            *(kern_rsp as *mut usize) = rip;
        }

        task.cr3 = None; //share with kernel
        self.entry(pid).or_insert(Arc::new(RwLock::new(task)));
        self.next_id += 1;
    }
}

impl Deref for TaskList {
    type Target = TaskMap;
    fn deref(&self) -> &Self::Target {
        &self.tasks
    }
}

impl DerefMut for TaskList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tasks
    }
}

static TASKS: Once<RwLock<TaskList>> = Once::new();
pub static CURRENT_ID: AtomicIsize = AtomicIsize::new(0);

fn init_tasks() -> RwLock<TaskList> { RwLock::new(TaskList::new()) }

pub fn init() {
    printk!(Info, "tasks init\n\r");

    {
        let oflags = unsafe { cpu::push_flags() };

        let rips = [
            idle as usize,
            test_thread as usize,
            test_thread2 as usize,
            test_thread3 as usize
        ];
        let names = [
            &"idle",
            &"kthread1",
            &"kthread2",
            &"kthread3",
        ];

        let mut tasks = TaskList::get_mut();
        for (id, &rip) in rips.iter().enumerate() {
            tasks.alloc_task(names[id], 1, rip);
            //printk!(Info, "{:?}\n\r", task);
        }

        unsafe { cpu::pop_flags(oflags); }
    }


    { 
        let init: *mut Task;
        let oflags = unsafe { cpu::push_flags() };

        {
            let tasks = TaskList::get();
            let task_lock = tasks.get_task(1).expect("task 1 does not exists");
            let mut task = task_lock.write();
            init = task.deref_mut() as *mut Task;
        }

        let mut new_map = {
            let mut mm = MM.try().unwrap().lock();
            paging::create_address_space(mm.mbinfo)
        };

        unsafe { cpu::pop_flags(oflags); }
        unsafe { start_tasking(&mut *init); }
    }

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
    printk!(Info, "start_tasking\n\r");
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
    CURRENT_ID.store(1, Ordering::Release);
}

pub unsafe fn sched() {
    use ::kern::arch::cpu::flags;
    let oflags = flags::flags();
    assert!(!oflags.contains(flags::Flags::IF), "sched: should disable IF\n");

    let id = CURRENT_ID.load(Ordering::SeqCst);
    if id == 0 { return  }

    let tasks = TaskList::get();
    let nid = if id + 1 > tasks.len() as ProcId { 1 } else { id + 1 };
    CURRENT_ID.store(nid, Ordering::Release);
    //printk!(Debug, "switch to {:?}\n", nid);
    let current: *mut Task;
    let next: *mut Task;

    {
        let current_lock = tasks.get_task(id as ProcId).expect("sched: get current task error");
        current = current_lock.write().deref_mut() as *mut Task;
        let next_lock = tasks.get_task(nid as ProcId).expect("sched: get next task error");
        next = next_lock.write().deref_mut() as *mut Task;
    }

    switch_to(&mut *current, &mut *next); 
}

