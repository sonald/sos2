#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(asm)]
#![feature(range_contains)]
#![no_std]

extern crate rlibc;
extern crate multiboot2;
extern crate spin;
#[macro_use] extern crate bitflags;

#[macro_use]
mod kern;

use kern::console as con;
use con::LogLevel::*;
use kern::driver::serial;
use kern::memory::*;

#[allow(dead_code)]
fn busy_wait () {
    for _ in 1..500000 {
        kern::util::cpu_relax();
    }
}

#[allow(dead_code)]
fn print_test() {
    for i in 1..24 {
        printk!(Info, "#{} \t{} \t{}\n\r", i, i, i);
        busy_wait();
    }

    printk!(Debug, "values: {}, {}, {}\n\r", "hello", 12 / 5, 12.34 / 3.145);
    printk!(Debug, "{}\n\r", {println!("inner"); "outer"});
    printk!(Warn, "kernel log\n\r");
    printk!(Critical, "kernel log\n\r");
}

/// test rgb framebuffer drawing
fn display(fb: &multiboot2::FramebufferTag) {
    use core::ptr::*;
    use core::mem::size_of_val;
    let vga;

    if fb.frame_type != multiboot2::FramebufferType::Rgb {
        return;
    }
    unsafe {
        vga = fb.addr as *mut u32;
        let mut clr: u32 = 0;

        for _ in 0..100 {
            for i in 0..fb.height {
                let data = &[clr; 800];
                let off = i * fb.width;
                copy_nonoverlapping(data, vga.offset(off as isize) as *mut _, 1);
                clr += 1;
                if clr > 0x00ffffff {
                    clr = 0;
                }
            }

            busy_wait();
        }
    }
}

fn test_frame_allocator(afa: &mut AreaFrameAllocator) {
    printk!(Debug, "Allocator: \n\r{:?}\n\r", afa);

    let mut i = 0;
    while let Some(f) = afa.alloc_frame() {
        //printk!(Warn, "0x{:x}  ", f.number);
        i += 1;
        if i == 100 { break }
    }
    printk!(Warn, "allocated #{} frames\n\r", i);
}

#[no_mangle]
pub extern fn kernel_main(mb2_header: usize) {
    unsafe { serial::init_serial(); }

    con::clear();
    printk!(Info, "Loading SOS2....\n\r");

    let mbinfo = unsafe { multiboot2::load(mb2_header) };
    printk!(Info, "{:?}\n\r", mbinfo);

    let mmap = mbinfo.memory_map_tag().expect("memory map is unavailable");
    let start = mmap.memory_areas().map(|a| a.base_addr).min().unwrap();
    let end = mmap.memory_areas().map(|a| a.base_addr + a.length).max().unwrap();
    printk!(Info, "mmap start: 0x{:x}, end: 0x{:x}\n\r", start ,end);

    let elf = mbinfo.elf_sections_tag().expect("elf sections is unavailable");
    let kernel_start = elf.sections().map(|a| a.addr).min().unwrap();
    let kernel_end = elf.sections().map(|a| a.addr + a.size).max().unwrap();
    printk!(Info, "kernel start: 0x{:x}, end: 0x{:x}\n\r", kernel_start, kernel_end);

    let (mb_start, mb_end) = (mb2_header, mb2_header + mbinfo.total_size as usize);
    printk!(Info, "mboot2 start: 0x{:x}, end: 0x{:x}\n\r", mb_start, mb_end);


    let fb = mbinfo.framebuffer_tag().expect("framebuffer tag is unavailale");
    printk!(Debug, "fb: {:?}\n\r", fb);
    display(&fb);

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
        paging::test_paging_before_remap(&mut afa);
    }
    paging::remap_the_kernel(&mut afa, &mbinfo);
    {
        test_frame_allocator(&mut afa);
        paging::test_paging_after_remap(&mut afa);
    }
}

#[lang = "eh_personality"]
extern fn eh_personality() {}

#[lang = "panic_fmt"] 
#[no_mangle] pub extern fn panic_fmt(fmt: core::fmt::Arguments, file: &'static str, line: u32) -> ! {
	printk!(Critical, "\n\rPanic at {}:{}\n\r", file, line);
    printk!(Critical, "    {}\n\r", fmt);
    loop {
        unsafe { asm!("hlt":::: "volatile"); }
    }
}

#[lang = "eh_unwind_resume"]
#[no_mangle]
pub extern fn rust_eh_unwind_resume() {
}

/// dummy, this should never gets called
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}
