
#[path = "../common/io.rs"]
pub mod io;

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
    unsafe { asm!("mov %cr3, $0":"=r"(ret)) }
    ret
}
