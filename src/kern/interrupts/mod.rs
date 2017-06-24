#[macro_use] pub mod idt;
pub mod irq;
pub mod timer;
mod gdt;

pub use self::idt::*;
pub use self::irq::{PIC_CHAIN, Irqs};

use self::gdt::{GlobalDescriptorTable, Descriptor};
use self::timer::{PIT, timer_handler};
use ::kern::driver::keyboard::{KBD, keyboard_irq};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::instructions::interrupts;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::instructions::segmentation::*;
use x86_64::registers::flags;

use ::kern::console::LogLevel::*;
use ::kern::arch::cpu::cr2;
use ::kern::memory::MemoryManager;
use spin::{Once, Mutex};

lazy_static! {
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.page_fault = Entry::new(cs().0, define_handler_with_errno!(page_fault_handler) as u64);
        idt.general_protection_fault = 
            Entry::new(cs().0, define_handler_with_errno!(general_protection_fault) as u64);
        idt.breakpoint = Entry::new(cs().0, define_handler!(int3_handler) as u64);
        idt.double_fault = Entry::new(cs().0, define_handler_with_errno!(double_fault_handler) as u64);
        idt.double_fault.options().set_ist_index(IST_INDEX_DBL_FAULT as u16);
        idt.divide_by_zero = Entry::new(cs().0, define_handler!(divide_by_zero_handler) as u64);

        idt.irqs[Irqs::TIMER as usize-32] = Entry::new(cs().0, define_handler!(timer_handler) as u64);
        idt.irqs[Irqs::KBD as usize-32] = Entry::new(cs().0, define_handler!(keyboard_irq) as u64);

        idt
    };
}


bitflags! {
    flags PageFaultErrorCode: u64 {
        const PROTECTION_VIOLATION = 1 << 0,
        const CAUSED_BY_WRITE = 1 << 1,
        const USER_MODE = 1 << 2,
        const MALFORMED_TABLE = 1 << 3,
        const INSTRUCTION_FETCH = 1 << 4,
    }
}

extern "C" fn double_fault_handler(frame: &mut ExceptionStackFrame, err_code: u64) {
    printk!(Debug, "double fault\n\r{:#?}\n\r", frame);
    loop {
        unsafe { asm!("hlt"); }
    }
}

extern "C" fn general_protection_fault(frame: &mut ExceptionStackFrame, err_code: u64) {
    printk!(Debug, "GPE err code: {:#?}\n\r", err_code);

    loop {
        unsafe { asm!("hlt"); }
    }
}

extern "C" fn page_fault_handler(frame: &mut ExceptionStackFrame, err_code: u64) {
    use ::kern::task::CURRENT_ID;
    use core::sync::atomic::Ordering;

    let err = PageFaultErrorCode::from_bits(err_code).unwrap();
    printk!(Debug, "page fault! {:#?}\n\rerr code: {:#?}, cr2: {:#x} tid: {:#x}\n\r",
            frame, err, cr2(), CURRENT_ID.load(Ordering::SeqCst));
    loop {
        unsafe { asm!("hlt"); }
    }
}

extern "C" fn int3_handler(frame: &mut ExceptionStackFrame) {
    printk!(Debug, "int3!! {:#?}\n\r", frame);
}

extern "C" fn divide_by_zero_handler(frame: &mut ExceptionStackFrame) {
    printk!(Debug, "divide_by_zero!! {:#?}\n\r", frame);
    loop {}
}

const IST_INDEX_DBL_FAULT: usize = 0;
// single tss
pub static mut TSS: TaskStateSegment = TaskStateSegment::new();
static GDT: Once<GlobalDescriptorTable> = Once::new();

pub const KERN_CS_SEL: SegmentSelector = SegmentSelector(1<<3);
pub const KERN_DS_SEL: SegmentSelector = SegmentSelector(2<<3);
pub const USER_DS_SEL: SegmentSelector = SegmentSelector((3<<3) | 3);
pub const USER_CS_SEL: SegmentSelector = SegmentSelector((4<<3) | 3);
pub const TSS_SEL: SegmentSelector = SegmentSelector(5<<3);

pub fn init(mm: &mut MemoryManager) {
    use x86_64;
    use x86_64::instructions::tables::load_tss;
    use x86_64::registers::msr;

    {
        let dbl_fault_stack = mm.alloc_stack(1).expect("alloc double_fault stack failed\n\r");
        printk!(Info, "alloc dbl_fault_stack {:#x}\n\r", dbl_fault_stack.bottom());
        unsafe {
            TSS.interrupt_stack_table[IST_INDEX_DBL_FAULT] = x86_64::VirtualAddress(dbl_fault_stack.top());
        }
    }

    let mut tss_sel = SegmentSelector(0);
    let mut user_cs_sel = SegmentSelector(0);
    let gdt = GDT.call_once(|| {
        let mut gdt = GlobalDescriptorTable::new();
        gdt.add_entry(Descriptor::kernel_code_segment());
        gdt.add_entry(Descriptor::kernel_data_segment());
        gdt.add_entry(Descriptor::user_data_segment());
        gdt.add_entry(Descriptor::user_code_segment());
        unsafe {
            gdt.add_entry(Descriptor::tss_segment(&TSS));
        }
        gdt
    });

    gdt.load();

    // setup for fast syscalls (64-bit submode only)
    unsafe {
        use bit_field::BitField;
        extern { fn syscall_entry(); }

        let mut star_val: u64 = 0;
        star_val.set_bits(32..48, KERN_CS_SEL.0 as u64); // offset to kern cs && ss
        star_val.set_bits(48..64, KERN_DS_SEL.0 as u64); // offset to user cs & ss
        msr::wrmsr(msr::IA32_STAR, star_val);
        msr::wrmsr(msr::IA32_LSTAR, syscall_entry as u64);
        msr::wrmsr(msr::IA32_FMASK, 0x0200); // disable interrupt right now
        ::kern::arch::cpu::enable_sce_bit();
    }

    unsafe {
        load_ds(KERN_DS_SEL);
        load_gs(KERN_DS_SEL);
        set_cs(KERN_CS_SEL);
        load_tss(TSS_SEL);
    }

    IDT.load();

    unsafe {
        PIT.lock().init();
        KBD.lock().init();

        PIC_CHAIN.lock().init();
        PIC_CHAIN.lock().enable(Irqs::IRQ2 as usize);
        PIC_CHAIN.lock().enable(Irqs::TIMER as usize);
        PIC_CHAIN.lock().enable(Irqs::KBD as usize);
        let mut oflags = ::kern::arch::cpu::push_flags();
        printk!(Debug, "oflags {:#?}\n\r", oflags);
        interrupts::enable();
    }
}

pub fn test_idt() {
    let busy_wait =|| {
        for _ in 1..10000 {
            ::kern::util::cpu_relax();
        }
    };
    
    use ::kern::console::{tty1, Console};
    let mut count = 0;

    loop {
        if count > 100 {
            break;
        }

        printk!(Critical, "count: {}\r", count);
        count += 1;

        //busy_wait();
    }
}
