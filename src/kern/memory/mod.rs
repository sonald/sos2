pub mod frame;
pub mod paging;
pub mod inactive;
pub mod mapper;
pub mod stack_allocator;

pub use self::stack_allocator::Stack;

use self::frame::*;
use self::paging::*;
use core::ops::Range;
use self::stack_allocator::StackAllocator;

use spin;
use kheap_allocator;
use multiboot2::*;

#[macro_use] use ::kern::console as con;
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
pub struct MemoryManager {
    pub activePML4Table: ActivePML4Table,
    pub areaFrameAllocator: AreaFrameAllocator,
    pub stackAllocator: StackAllocator
}

impl MemoryManager {
    pub fn alloc_stack(&mut self, size_in_pages: usize) -> Option<Stack> {
        self.stackAllocator.alloc_stack(&mut self.activePML4Table, &mut self.areaFrameAllocator, size_in_pages)
    }

}

pub fn init(mbinfo: &BootInformation) -> MemoryManager {
    #[inline]
    fn align_up(start: usize, align: usize) -> usize {
        ((start + align - 1) / align) * align
    }

    printk!(Info, "memory system init.\n\r");
    let kernel_base = KERNEL_MAPPING.KernelMap.start;

    let mmap = mbinfo.memory_map_tag().expect("memory map is unavailable");
    let start = mmap.memory_areas().map(|a| a.base_addr).min().unwrap();
    let end = mmap.memory_areas().map(|a| a.base_addr + a.length).max().unwrap();
    printk!(Info, "mmap start: {:#x}, end: {:#x}\n\r", start ,end);

    let elf = mbinfo.elf_sections_tag().expect("elf sections is unavailable");
    let mut kernel_start = elf.sections().filter(|a| a.is_allocated()).map(|a| a.addr).min().unwrap() as usize;
    let mut kernel_end = elf.sections().filter(|a| a.is_allocated()).map(|a| a.addr + a.size).max().unwrap() as usize;

    if kernel_start > kernel_base {
        kernel_start -= kernel_base;
    }
    if kernel_end > kernel_base {
        kernel_end -= kernel_base;
    }
    printk!(Info, "kernel start: {:#x}, end: {:#x}\n\r", kernel_start, kernel_end);


    let (mb_start, mb_end) = (mbinfo.start_address() - kernel_base,
    mbinfo.end_address() - kernel_base);
    printk!(Info, "mboot2 start: {:#x}, end: {:#x}\n\r", mb_start, mb_end);

    use core::ops::Range;
    let kr = Range {
        start: Frame::from_paddress(kernel_start),
        end: Frame::from_paddress(kernel_end - 1) + 1,
    };
    let mr = Range {
        start: Frame::from_paddress(mb_start),
        end: Frame::from_paddress(mb_end - 1) + 1,
    };
    let mut afa = AreaFrameAllocator::new(mmap.memory_areas(), kr, mr);

    if cfg!(feature = "test") {
        test_frame_allocator(&mut afa);
        test_paging_before_remap(&mut afa);
    }
    ::kern::arch::cpu::enable_nxe_bit();
    ::kern::arch::cpu::enable_write_protect_bit();
    remap_the_kernel(&mut afa, &mbinfo);
    if cfg!(feature = "test") {
        test_frame_allocator(&mut afa);
        test_paging_after_remap(&mut afa);
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
            active.map(page, WRITABLE, &mut afa);
        }
    }

    //after heap
    let stack_allocator = {
        let start = Page::from_vaddress(align_up(mbinfo.end_address(), PAGE_SIZE) + 0x1000 * 1024);
        let end = start + 100;

        StackAllocator::new(start, end)
    };

    MemoryManager {
        activePML4Table: ActivePML4Table::new(),
        areaFrameAllocator: afa,
        stackAllocator: stack_allocator
    }
}

fn test_frame_allocator(afa: &mut AreaFrameAllocator) {
    //printk!(Debug, "{:#?}\n\r", afa);

    let mut i = 0;
    while let Some(f) = afa.alloc_frame() {
        //printk!(Warn, "0x{:x}  ", f.number);
        i += 1;
        if i == 100 { break }
    }
    printk!(Warn, "allocated #{} frames\n\r", i);
}

