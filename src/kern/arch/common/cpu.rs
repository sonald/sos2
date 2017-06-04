use kern::memory::paging::{VirtualAddress, PhysicalAddress};

/// Invalidate the given address in the TLB using the `invlpg` instruction.
pub fn tlb_flush(addr: VirtualAddress) {
    unsafe { asm!("invlpg ($0)"::"r" (addr) : "memory") };
}

/// Invalidate the TLB completely by reloading the CR3 register.
pub fn tlb_flush_all() {
    unsafe { cr3_set(cr3()) }
}

/// read pml4 pointer from cr3
pub fn cr3() -> PhysicalAddress {
    let ret: usize;
    unsafe { asm!("mov %cr3, $0":"=r"(ret)) }
    ret
}

pub unsafe fn cr3_set(paddr: PhysicalAddress) {
    asm!("mov $0, %cr3"::"r"(paddr) : "memory");
}

/// read page fault address
pub fn cr2() -> VirtualAddress {
    let ret: usize;
    unsafe { asm!("mov %cr2, $0":"=r"(ret)) }
    ret
}

/// Read CR0
pub fn cr0() -> usize {
    let ret: usize;
    unsafe { asm!("mov %cr0, $0" : "=r" (ret)) };
    ret
}

pub const CR0_WRITE_PROTECT: usize = 1 << 16;

/// Write CR0.
///
/// # Safety
/// Changing the CR0 register is unsafe, because e.g. disabling paging would violate memory safety.
pub unsafe fn cr0_set(val: usize) {
    asm!("mov $0, %cr0" :: "r" (val) : "memory");
}

pub const IA32_EFER: u32 = 0xc0000080;

// Model specific registers

/// Write 64 bits to msr register.
pub unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    asm!("wrmsr" :: "{ecx}" (msr), "{eax}" (low), "{edx}" (high) : "memory" : "volatile" );
}

/// Read 64 bits msr register.
pub fn rdmsr(msr: u32) -> u64 {
    let (high, low): (u32, u32);
    unsafe {
        asm!("rdmsr" : "={eax}" (low), "={edx}" (high) : "{ecx}" (msr) : "memory" : "volatile");
    }
    ((high as u64) << 32) | (low as u64)
}

/// enable NXE bit, so page flag NO_EXECUTE is applicable
pub fn enable_nxe_bit() {
    let nxe_bit = 1 << 11;
    unsafe {
        let efer = rdmsr(IA32_EFER);
        wrmsr(IA32_EFER, efer | nxe_bit);
    }
}

/// enable WP so page flag WRITABLE takes effect in kernel mode
pub fn enable_write_protect_bit() {
    unsafe { cr0_set(cr0() | CR0_WRITE_PROTECT) };
}

use x86_64::registers::flags;
pub unsafe fn push_flags() -> flags::Flags {
    use x86_64::instructions::interrupts;
    let old = flags::flags();
    interrupts::disable();
    old
}

pub unsafe fn pop_flags(old: flags::Flags) {
    use x86_64::instructions::interrupts;
    flags::set_flags(old);
    if old.contains(flags::Flags::IF) {
        interrupts::enable();
    }
}
