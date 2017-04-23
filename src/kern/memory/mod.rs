pub mod frame;
pub mod paging;
pub mod inactive;
pub mod mapper;

use self::frame::*;
use self::paging::*;

pub const PAGE_SIZE: usize = 4096;

use spin;
use kheap_allocator;
use multiboot2::*;


#[macro_use] use ::kern::console as con;
use con::LogLevel::*;

static INIT: spin::Once<()> = spin::Once::new();

pub fn init(mbinfo: &BootInformation) {

    INIT.call_once(|| {
        printk!(Info, "memory system init.\n\r");
        let mmap = mbinfo.memory_map_tag().expect("memory map is unavailable");
        let start = mmap.memory_areas().map(|a| a.base_addr).min().unwrap();
        let end = mmap.memory_areas().map(|a| a.base_addr + a.length).max().unwrap();
        printk!(Info, "mmap start: {:#x}, end: {:#x}\n\r", start ,end);

        let elf = mbinfo.elf_sections_tag().expect("elf sections is unavailable");
        let kernel_start = elf.sections().filter(|a| a.is_allocated()).map(|a| a.addr).min().unwrap();
        let kernel_end = elf.sections().filter(|a| a.is_allocated()).map(|a| a.addr + a.size).max().unwrap();
        printk!(Info, "kernel start: {:#x}, end: {:#x}\n\r", kernel_start, kernel_end);

        let (mb_start, mb_end) = (mbinfo.start_address(), mbinfo.end_address());
        printk!(Info, "mboot2 start: {:#x}, end: {:#x}\n\r", mb_start, mb_end);

        use core::ops::Range;
        let kr = Range {
            start: Frame::from_paddress(kernel_start as usize),
            end: Frame::from_paddress(kernel_end as usize - 1) + 1,
        };
        let mr = Range {
            start: Frame::from_paddress(mb_start),
            end: Frame::from_paddress(mb_end - 1) + 1,
        };
        let mut afa = AreaFrameAllocator::new(mmap.memory_areas(), kr, mr);
        {
            test_frame_allocator(&mut afa);
            test_paging_before_remap(&mut afa);
        }
        ::kern::arch::cpu::enable_nxe_bit();
        ::kern::arch::cpu::enable_write_protect_bit();
        remap_the_kernel(&mut afa, &mbinfo);
        {
            test_frame_allocator(&mut afa);
            test_paging_after_remap(&mut afa);
        }

        {
            //map kheap area
            //TODO: should be lazily mapped after page fault sets up
            let mut active = ActivePML4Table::new();
            let mut start = kheap_allocator::START_ADDRESS / PAGE_SIZE;
            let end = (kheap_allocator::START_ADDRESS + kheap_allocator::ALLOC_SIZE - 1) / PAGE_SIZE + 1;

            while start < end {
                let page = Page::from_vaddress(start * PAGE_SIZE);
                active.map(page, WRITABLE, &mut afa);
                start += 1;
            }
        }
        printk!(Critical, "Pass All Tests!!!!!\n\r");
    });
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

