use ::kern::memory::inactive::{TemporaryPage, InactivePML4Table};
use ::kern::memory::stack_allocator::{Stack, StackAllocator};
use ::kern::memory::{MemoryManager, MM, KERNEL_MAPPING};
use ::kern::memory::paging;
use ::kern::console::LogLevel::*;
use ::kern::console::{Console, tty1};
use ::kern::arch::cpu;
use ::kern::interrupts::{self, idt};

use core::sync::atomic::{AtomicIsize, Ordering};
use collections::string::{String, ToString};
use collections::{BTreeMap, Vec};
use alloc::arc::Arc;
use core::ops::{Deref, DerefMut};

use spin::*;
use ::kern::elf64::*;
use x86_64;

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


/// context for kernel side task scheduler
#[derive(Debug, Clone)]
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

    pub fn map(&self, inactive: &mut InactivePML4Table) {
        let mut active = paging::ActivePML4Table::new();
        let mut temp_page = TemporaryPage::new(paging::Page::from_vaddress(0xfffff_cafe_beef_000));
        printk!(Debug, "mapping VirtualMemoryArea {:?} {:?}\n\r", self.get_pages(), self.flags);
        active.with(inactive, &mut temp_page, |mapper| {
            for page in self.get_pages() {
                mapper.map(page, self.flags);
            }
        });
    }

    pub fn unmap(&mut self, inactive: &mut InactivePML4Table) {
    }

    pub fn get_pages(&self) -> paging::PageRange {
        paging::PageRange::new(self.start, self.start + self.size)
    }
}

#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct TLSSegment {
    pub user_rsp: usize,
    pub kern_rsp: usize
}

impl TLSSegment {
    pub const fn empty() -> TLSSegment {
        TLSSegment {
            user_rsp: 0,
            kern_rsp: 0
        }
    }

    pub const fn new(kern_rsp: usize, user_rsp: usize) -> TLSSegment {
        TLSSegment {
            user_rsp,
            kern_rsp
        }
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
    pub data: Option<VirtualMemoryArea>, //including data and bss
    pub exec_entry: usize,
    pub ctx: Context,
    pub state: TaskState,
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
            data: None,
            exec_entry: 0,
            state: TaskState::Unused,
            ctx: Context::new(),
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

    // kernel thread
    pub fn alloc_kernel_task(&mut self, name: &str, rip: usize) {
        use core::mem::size_of;


        let pid = self.next_id;
        assert!(self.next_id < MAX_TASK, "task id exceeds maximum boundary");

        let mut task = Task::empty();
        task.pid = pid as isize;
        task.ppid = 0;
        task.name = Some(name.to_string());
        task.state = TaskState::Created;
        task.exec_entry = rip;

        task.kern_stack = Some({
            let mem = vec![0u8; 8192].into_boxed_slice();
            printk!(Debug, "boxed slice [{:#x}, {:#x})\n\r", mem.as_ptr() as usize, mem.len());
            let top = mem.as_ptr() as usize;
            Stack::new(top + mem.len(), top)
        });
        task.cr3 = Some({
            let mut mm = MM.try().unwrap().lock();
            mm.kernelPML4Table
        });
        task.ctx = Context::new();

        let kern_rsp = task.kern_stack.as_ref().map(|st| st.top()).unwrap();
        task.ctx.rflags = 0x0202;
        task.ctx.rsp = kern_rsp - size_of::<TLSSegment>() 
            - size_of::<idt::ExceptionStackFrame>() - size_of::<usize>();
        unsafe {
            let tlsbase = kern_rsp - size_of::<TLSSegment>();
            let tls = tlsbase as *mut TLSSegment;
            ::core::ptr::write(tls, TLSSegment {
                user_rsp: 0,
                kern_rsp: tlsbase
            });

            let fp = tlsbase as *mut usize;
            *fp.offset(-1) = interrupts::KERN_DS_SEL.0 as usize;
            *fp.offset(-2) = tlsbase; // when task begins, exception frame will be overriden
            *fp.offset(-3) = task.ctx.rflags;
            *fp.offset(-4) = interrupts::KERN_CS_SEL.0 as usize;
            *fp.offset(-5) = task.exec_entry;
            *fp.offset(-6) = start_task as usize;
        }
        task.ctx.cr3 = task.cr3.as_ref().unwrap().pml4_frame.start_address();

        self.entry(pid).or_insert(Arc::new(RwLock::new(task)));
        self.next_id += 1;
    }

    // user task
    pub fn load_task(&mut self, name: &str, parent: ProcId) {
        use core::mem::size_of;

        let pid = self.next_id;
        assert!(self.next_id < MAX_TASK, "task id exceeds maximum boundary");

        let mut task = Task::empty();
        task.pid = pid as isize;
        task.ppid = parent; 
        task.name = Some(name.to_string());
        task.state = TaskState::Created;

        task.cr3 = Some({
            let mut mm = MM.try().unwrap().lock();
            paging::create_address_space(mm.mbinfo)
        });

        task.user_stack = Some({
            let mut vma = VirtualMemoryArea {
                start: KERNEL_MAPPING.UserStack.start,
                size: KERNEL_MAPPING.UserStack.end - KERNEL_MAPPING.UserStack.start + 1,
                mapped: false,
                flags: paging::USER | paging::WRITABLE | paging::NO_EXECUTE
            };

            vma.map(task.cr3.as_mut().unwrap());
            vma.mapped = true;

            vma
        });

        {
            printk!(Debug, "load_task\n\r");
            let kernel_base = KERNEL_MAPPING.KernelMap.start;
            let mut mm = MM.try().unwrap().lock();
            let init_mod = mm.mbinfo.module_tags().next().unwrap();
            assert!(init_mod.name() == "init");

            let (init_start, init_end) = (
                init_mod.start_address() as usize + kernel_base,
                init_mod.end_address() as usize + kernel_base);

            unsafe {
                let bytes = ::core::slice::from_raw_parts(
                    init_start as *const u8, 
                    (init_end - init_start) as usize);
                let elf =  Elf64::from(bytes);
                task.exec_entry = elf.header.e_entry as usize;

                for ph in elf.program_headers() {
                    printk!(Debug, "{:?}\n\r", ph);
                    if ph.p_type != PT_LOAD { continue; }

                    let sz = ph.p_memsz as usize;
                    let data = elf.data.as_ptr().offset(ph.p_offset as isize);
                    match (ph.p_flags & PF_X) != 0 {
                        false => {
                            printk!(Debug, "load data/bss segment\n\r");
                        }, 

                        true => {
                            let code: &[u8; 20] = &*(data as *const [u8; 20]);
                            printk!(Debug, "load code segment {:?}\n\r", code);
                            task.code = Some({
                                let mut vma = VirtualMemoryArea {
                                    start: KERNEL_MAPPING.UserCode.start,
                                    size: sz,
                                    mapped: false,
                                    flags: paging::USER | paging::WRITABLE
                                };

                                vma.map(task.cr3.as_mut().unwrap());
                                vma.mapped = true;

                                vma
                            });

                            use core::ptr;
                            // switching pml4 is heavy
                            let cur_pml4 = paging::switch(task.cr3.clone().unwrap());
                            {
                                let vma = task.code.clone().unwrap();
                                ptr::copy_nonoverlapping(data as *mut u8,
                                                         vma.start as *mut u8, sz);
                            }
                            paging::switch(cur_pml4);
                        }
                    }
                }
            }
        }


        task.kern_stack = Some({
            let mem = vec![0u8; 8192].into_boxed_slice();
            printk!(Debug, "boxed slice [{:#x}, {:#x})\n\r", mem.as_ptr() as usize, mem.len());
            let top = mem.as_ptr() as usize;
            Stack::new(top + mem.len(), top)
        });
        task.ctx = Context::new();
        let kern_rsp = task.kern_stack.as_ref().map(|st| st.top()).unwrap();
        task.ctx.rflags = 0x0202;
        task.ctx.rsp = kern_rsp - size_of::<TLSSegment>();
        unsafe { 
            let mut tlsbase = kern_rsp - size_of::<TLSSegment>();
            let tls = tlsbase as *mut TLSSegment;
            ::core::ptr::write(tls, TLSSegment {
                user_rsp: (KERNEL_MAPPING.UserStack.end+1),
                kern_rsp: tlsbase
            });
        }
        
        task.ctx.cr3 = task.cr3.as_ref().unwrap().pml4_frame.start_address();
        printk!(Debug, "init cr3 {:?} {}\n\r", task.cr3, task.ctx.cr3);

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
        ];
        let names = [
            &"idle",
            &"kthread1",
            &"kthread2",
        ];

        let mut tasks = TaskList::get_mut();
        for (id, &rip) in rips.iter().enumerate() {
            tasks.alloc_kernel_task(names[id], rip);
            //printk!(Info, "{:?}\n\r", task);
        }

        unsafe { cpu::pop_flags(oflags); }
    }


    { 
        use x86_64;

        let init: *mut Task;
        unsafe { x86_64::instructions::interrupts::disable(); }

        {
            let mut tasks = TaskList::get_mut();
            tasks.load_task(&"init", 1);
        }

        {
            let tasks = TaskList::get();
            let task_lock = tasks.get_task(4).expect("task 4");
            let mut task = task_lock.write();
            CURRENT_ID.store(task.pid, Ordering::SeqCst);
            init = task.deref_mut() as *mut Task;
        }


        printk!(Info, "start_tasking\n\r");
        unsafe { ret_to_userspace(&mut *init); }
    }

    panic!("task done");
}

pub fn idle() {
    loop {
        unsafe { asm!("sti; hlt":::: "volatile"); }
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
    asm!("movq $0, %rbx"  :: "r"(next.ctx.rbx) :"memory": "volatile");
    asm!("movq $0, %r12"  :: "r"(next.ctx.r12) :"memory": "volatile");
    asm!("movq $0, %r13"  :: "r"(next.ctx.r13) :"memory": "volatile");
    asm!("movq $0, %r14"  :: "r"(next.ctx.r14) :"memory": "volatile");
    asm!("movq $0, %r15"  :: "r"(next.ctx.r15) :"memory": "volatile");

    asm!("movq $0, %rsp"  :: "r"(next.ctx.rsp) :"memory": "volatile");
    
    //CAUTION: popfq causes IF enabled
    //NOTE: rbp is used by switch_to, to override rbp at the end
    asm!("
         pushq $0
         movq $1, %rbp
         popfq
         "
         :
         :"r"(next.ctx.rflags),
          "r"(next.ctx.rbp)
         :"memory"
         :"volatile");
}

#[inline(never)]
#[naked]
unsafe extern "C" fn start_task() -> ! {
    asm!("iretq" ::: "memory" : "volatile");
    ::core::intrinsics::unreachable()
}

unsafe fn ret_to_userspace(init: &mut Task) -> ! {
    use ::kern::interrupts::{self, idt};
    use ::kern::syscall;

    let frame = idt::ExceptionStackFrame {
        rip: init.exec_entry as u64,
        cs: interrupts::USER_CS_SEL.0 as u64,
        rflags: init.ctx.rflags as u64,
        old_rsp: (KERNEL_MAPPING.UserStack.end+1) as u64,
        old_ss: interrupts::USER_DS_SEL.0 as u64,
    };

    {
        use x86_64::registers::msr;

        let tlsbase = init.kern_stack.as_ref().map(|st| st.top()).unwrap()
            - ::core::mem::size_of::<TLSSegment>();
        let tls = &*(tlsbase as *const TLSSegment);
        if tls.kern_rsp != 0 {
            msr::wrmsr(msr::IA32_GS_BASE, tls.kern_rsp as u64);
        }

        interrupts::TSS.privilege_stack_table[0] = x86_64::VirtualAddress(tls.kern_rsp);

        // alternate way to write rsp0
        //let rsp0: usize;
        //asm!("movq %gs:(8), $0":"=r"(rsp0)::"memory":"volatile");
        //interrupts::TSS.privilege_stack_table[0] = x86_64::VirtualAddress(rsp0);
        //printk!(Debug, "{:?} set TSS.rsp0\n", frame);
    }


    paging::switch(init.cr3.clone().unwrap());


    asm!("
         swapgs
         movq %rbx, %rbp
         movq %rbx, %rsp
         .byte 0x48
         sysret"  //0x48 = REX.W, or we can just use sysretq
         :
         :"{r11}"(frame.rflags),
          "{rcx}"(frame.rip),
          "{rbx}"(frame.old_rsp)
         :"memory"
         :"volatile");

    panic!("sysret wont go here");
    ::core::intrinsics::unreachable()
}

pub unsafe fn sched() {
    use ::kern::arch::cpu::flags;
    let oflags = flags::flags();
    assert!(!oflags.contains(flags::Flags::IF), "sched: should disable IF\n");

    let id = CURRENT_ID.load(Ordering::SeqCst);
    if id == 0 { return  }

    let nid;
    let current: *mut Task;
    let mut next: *mut Task = 0 as *mut Task;

    {
        let tasks = TaskList::get();
        nid = if id + 1 >= tasks.next_id as ProcId { 1 } else { id + 1 };
        CURRENT_ID.store(nid, Ordering::Release);

        assert_ne!(id, nid, "sched: id should not be equal to nid");

        {
            let current_lock = tasks.get_task(id as ProcId).expect("sched: get current task error");
            let mut guard = current_lock.try_read().expect("sched: current lock failed");
            current = guard.deref() as *const Task as *mut Task;
            assert!((*current).pid == id);
        }

        {
            let next_lock = tasks.get_task(nid as ProcId).expect("sched: get next task error");
            match next_lock.try_write() {
                Some(mut guard) => {
                    next = guard.deref_mut() as *mut Task;
                    assert!((*next).pid == nid);
                },
                None => {
                    printk!(Critical, "sched: next({}) lock failed\n\r", nid);
                }
            };
        }
        //now tasklist lock released
    }

    //printk!(Debug, "switch {} {:#x} to {} {:#x}\n", id, (&*current).ctx.rsp, nid, (&*next).ctx.rsp);
    //printk!(Debug, "switch {:?} \n-> {:?}\n", (&*current).ctx, (&*next).ctx);

    if next as usize != 0 {
        use x86_64::registers::msr;
        
        let next = &mut *next;
        if next.user_stack.is_some() { // which means it's a user task
            let tlsbase = next.kern_stack.as_ref().map(|st| st.top()).unwrap()
                - ::core::mem::size_of::<TLSSegment>();
            let tls = &*(tlsbase as *const TLSSegment);
            if tls.kern_rsp != 0 {
                msr::wrmsr(msr::IA32_GS_BASE, tls.kern_rsp as u64);
            }
            interrupts::TSS.privilege_stack_table[0] = x86_64::VirtualAddress(tls.kern_rsp);

            if (*current).ctx.cr3 != next.ctx.cr3 {
                paging::switch(next.cr3.clone().unwrap());
            }
        }
        switch_to(&mut *current, &mut *next); 
    }
}

