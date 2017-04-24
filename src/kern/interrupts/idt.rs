use core::marker::PhantomData;
use core::mem::size_of;
use bit_field::BitField;

pub type HandlerFunc = extern "C" fn (&mut ExceptionStackFrame);
pub type HandlerFuncWithErrCode = extern "C" fn (&mut ExceptionStackFrame, u64);

#[derive(Debug, Clone, Copy)]
pub struct EntryOptions(u16);

impl EntryOptions {
    pub fn new() -> EntryOptions {
        let mut ret = EntryOptions::empty();
        //disable interrupts by default, syscall later will use trap gate
        ret.set_present(true).set_interrupt_gate();
        ret
    }

    // bit 9-11 should be ones
    pub fn empty() -> EntryOptions {
        let mut ret = EntryOptions(0);
        ret.0.set_bits(9..12, 0b111);
        ret
    }

    // set interrupt stack table index
    pub fn set_ist_index(&mut self, ist: u16) -> &mut Self {
        self.0.set_bits(0..3, ist);
        self
    }

    /// bit 8: 0: intr gate, 1: trap gate
    pub fn set_interrupt_gate(&mut self) -> &mut Self {
        self.0.set_bit(8, false);
        self
    }

    pub fn set_trap_gate(&mut self) -> &mut Self {
        self.0.set_bit(8, true);
        self
    }

    pub fn set_dpl(&mut self, dpl: u16) -> &mut Self {
        self.0.set_bits(13..15, dpl);
        self
    }

    pub fn set_present(&mut self, val: bool) -> &mut Self {
        self.0.set_bit(15, val);
        self
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Entry {
    pointer_low: u16,
    gdt_selector: u16,
    options: EntryOptions,
    pointer_middle: u16,
    pointer_high: u32,
    zero: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ExceptionStackFrame {
    rip: u64,
    cs: u64,
    rflags: u64,
    old_rsp: u64,
    odl_ss: u64
}

impl Entry {
    pub fn new(selector: u16, address: u64) -> Entry {
        Entry {
            pointer_low: address as u16,
            pointer_middle: (address >> 16) as u16,
            pointer_high: (address >> 32) as u32,
            gdt_selector: selector,
            options: EntryOptions::new(),
            zero: 0,
        }
    }

    pub fn empty() -> Entry {
        Entry::new(0, 0)
    }
}

macro_rules! define_handler {
    ($handler:ident) => ({
        #[naked]
        extern "C" fn handler_wrapper () -> ! {
            unsafe { 
                asm!("
                     push rax
                     push rcx
                     push rdx
                     push rsi
                     push rdi
                     push r8
                     push r9
                     push r10
                     push r11

                     mov rdi, rsp
                     add rdi, 9*8

                     call $0

                     pop r11
                     pop r10
                     pop r9
                     pop r8
                     pop rdi
                     pop rsi
                     pop rdx
                     pop rcx
                     pop rax

                     iretq"
                     ::"i"($handler as HandlerFunc)
                     :"rdi"
                     :"intel", "volatile");
                ::core::intrinsics::unreachable()
            };
        }

        handler_wrapper
    })
}

macro_rules! define_handler_with_errno {
    ($handler:ident) => ({
        #[naked]
        extern "C" fn handler_with_err_wrapper () -> ! {
            unsafe { 
                asm!("
                     push rax
                     push rcx
                     push rdx
                     push rsi
                     push rdi
                     push r8
                     push r9
                     push r10
                     push r11

                     mov rsi, [rsp + 9*8]
                     mov rdi, rsp
                     add rdi, 10*8

                     sub rsp, 8
                     call $0
                     add rsp, 8

                     pop r11
                     pop r10
                     pop r9
                     pop r8
                     pop rdi
                     pop rsi
                     pop rdx
                     pop rcx
                     pop rax

                     add rsp, 8 // remove errno
                     iretq"
                     ::"i"($handler as HandlerFuncWithErrCode)
                     :"rdi", "rsi"
                     :"intel", "volatile");
                ::core::intrinsics::unreachable()
            };
        }

        handler_with_err_wrapper
    })
}

/// this table comes from am64 vol2, it's a litte different with x86_64 crate
#[allow(dead_code)]
pub struct InterruptDescriptorTable {
    pub divide_by_zero: Entry,
    pub debug: Entry,
    pub non_maskable_interrupt: Entry,
    pub breakpoint: Entry,
    pub overflow: Entry,

    pub bound_range_exceeded: Entry,
    pub invalid_opcode: Entry,
    pub device_not_available: Entry,
    pub double_fault: Entry,
    coprocessor_segment_overrun: Entry,

    pub invalid_tss: Entry,
    pub segment_not_present: Entry,
    pub stack_segment_fault: Entry,
    pub general_protection_fault: Entry,
    pub page_fault: Entry,

    /// vector nr. 15
    reserved_1: Entry,
    pub x87_floating_point: Entry,
    pub alignment_check: Entry,
    pub machine_check: Entry,
    pub simd_floating_point: Entry,

     /// vector nr. 20-28
    reserved_2: [Entry; 9],
    pub vmm_communication_exception: Entry,

    pub security_exception: Entry,
    /// vector nr. 31
    reserved_3: Entry,

    pub irqs: [Entry; 16],

    pub interrupts: [Entry; 256 - 48],
}

impl InterruptDescriptorTable {
    pub fn new() -> Self {
        debug_assert_eq!(size_of::<Self>(), 256 * 16);

        InterruptDescriptorTable {
            divide_by_zero: Entry::empty(),
            debug: Entry::empty(),
            non_maskable_interrupt: Entry::empty(),
            breakpoint: Entry::empty(),
            overflow: Entry::empty(),

            bound_range_exceeded: Entry::empty(),
            invalid_opcode: Entry::empty(),
            device_not_available: Entry::empty(),
            double_fault: Entry::empty(),
            coprocessor_segment_overrun: Entry::empty(),

            invalid_tss: Entry::empty(),
            segment_not_present: Entry::empty(),
            stack_segment_fault: Entry::empty(),
            general_protection_fault: Entry::empty(),
            page_fault: Entry::empty(),

            reserved_1: Entry::empty(),
            x87_floating_point: Entry::empty(),
            alignment_check: Entry::empty(),
            machine_check: Entry::empty(),
            simd_floating_point: Entry::empty(),

            reserved_2: [Entry::empty(); 9],
            vmm_communication_exception: Entry::empty(),

            security_exception: Entry::empty(),
            reserved_3: Entry::empty(),

            irqs: [Entry::empty(); 16],
            interrupts: [Entry::empty(); 256 - 48],
        }
    }

    pub fn load(&self) {
        let dtp = DescriptorTablePointer {
            base: self as *const _ as u64,
            limit: (size_of::<Self>() - 1) as u16,
        };
        unsafe { load_idt(&dtp); }
    }
}

#[derive(Debug)]
#[repr(C, packed)]
pub struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

pub unsafe fn load_idt(idt: &DescriptorTablePointer) {
    asm!("lidt ($0)" :: "r"(idt) : "memory");
}


