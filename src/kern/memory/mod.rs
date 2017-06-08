pub mod frame;
pub mod paging;
pub mod inactive;
pub mod mapper;
pub mod stack_allocator;

pub use self::stack_allocator::Stack;

use self::paging::*;
use core::ops::Range;
use self::stack_allocator::StackAllocator;
use self::inactive::InactivePML4Table;

use spin::{Mutex, Once};
use kheap_allocator;
use multiboot2::*;

use ::kern::console as con;
use con::LogLevel::*;

pub const PAGE_SIZE: usize = 4096;

/// concrete page mapping schema of memory areas, inspired from linux x86_64
/// ref: https://www.kernel.org/doc/Documentation/x86/x86_64/mm.txt
/// 0000000000000000 - 00007fffffffffff (=47 bits) user space, different per mm
/// hole caused by [48:63] sign extension
/// ffff800000000000 - ffff8007ffffffff (=32G) direct mapping of all phys. memory
/// ffff800800000000 - ffff87ffffffffff (=43bits) reserved now
/// ffff880000000000 - ffff8800ffffffff (=4G)  kernel mapping, from phys 0
#[allow(non_snake_case)]
pub struct MemorySchema {
    pub UserMap: Range<usize>,
    pub Invalid: Range<usize>, // hardware hole
    pub PhysicalDirectMap: Range<usize>,
    pub KernelMap: Range<usize>
}

pub const KERNEL_MAPPING: MemorySchema = MemorySchema {
    UserMap: Range {start: 0, end: 0x7fff_ffffffff},
    Invalid: Range {start: 0x8000_00000000, end: 0xffff7fff_ffffffff},
    PhysicalDirectMap: Range {start: 0xffff8000_00000000, end: 0xffff8007_ffffffff},
    KernelMap: Range {start: 0xffff8800_00000000, end: 0xffff8800_ffffffff},
};

#[allow(non_snake_case)]
pub struct MemoryManager<'a> {
    pub activePML4Table: ActivePML4Table,
    pub kernelPML4Table: InactivePML4Table,
    pub stackAllocator: StackAllocator,
    pub mbinfo: &'a BootInformation
}

impl<'a> MemoryManager<'a> {
    pub fn alloc_stack(&mut self, size_in_pages: usize) -> Option<Stack> {
        self.stackAllocator.alloc_stack(&mut self.activePML4Table, size_in_pages)
    }

}

pub static MM: Once<Mutex<MemoryManager<'static>>> = Once::new();

pub fn init(mbinfo: &'static BootInformation) -> &'static Mutex<MemoryManager<'static>> {
    #[inline]
    fn align_up(start: usize, align: usize) -> usize {
        ((start + align - 1) / align) * align
    }

    printk!(Info, "memory system init.\n\r");
    
    frame::init(mbinfo);

    if cfg!(feature = "test") {
        test_frame_allocator();
        test_paging_before_remap();
    }
    ::kern::arch::cpu::enable_nxe_bit();
    ::kern::arch::cpu::enable_write_protect_bit();
    remap_the_kernel(&mbinfo);
    if cfg!(feature = "test") {
        test_frame_allocator();
        test_paging_after_remap();
    }

    {
        //map kheap area
        //TODO: should be lazily mapped after page fault sets up
        let start_address = align_up(mbinfo.end_address(), PAGE_SIZE);
        let alloc_size = 0x1000 * 1024;
        kheap_allocator::HEAP_RANGE.call_once(|| {
            Range {
                start: start_address,
                end: start_address + alloc_size
            }
        });

        let mut active = ActivePML4Table::new();
        let range = PageRange::new(start_address, start_address + alloc_size);
        printk!(Info, "map heap [{:#x}, {:#x})\n\r", start_address, start_address + alloc_size);
        for page in range {
            active.map(page, WRITABLE);
        }
    }

    //after heap
    let stack_allocator = {
        let start = Page::from_vaddress(align_up(mbinfo.end_address(), PAGE_SIZE) + 0x1000 * 1024);
        let end = start + 1024;

        StackAllocator::new(start, end)
    };

    MM.call_once(|| {
        Mutex::new(MemoryManager {
            activePML4Table: ActivePML4Table::new(),
            kernelPML4Table: InactivePML4Table {
                pml4_frame: frame::Frame::from_paddress(::kern::arch::cpu::cr3())
            },
            stackAllocator: stack_allocator,
            mbinfo: mbinfo
        })
    })
}

fn test_frame_allocator() {
    let mut i = 0;
    while let Some(_) = frame::alloc_frame() {
        //printk!(Warn, "0x{:x}  ", f.number);
        i += 1;
        if i == 400 { break }
    }
    printk!(Warn, "allocated #{} frames\n\r", i);
}

