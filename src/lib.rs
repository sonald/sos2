#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(asm)]
#![feature(range_contains)]
#![feature(alloc, collections)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]
// stabled since 1.17
#![feature(field_init_shorthand)]
#![no_std]

extern crate rlibc;
extern crate multiboot2;
extern crate spin;
extern crate x86_64;

extern crate kheap_allocator;
extern crate alloc;
#[macro_use] extern crate collections;

#[macro_use] extern crate bitflags;
extern crate bit_field;
#[macro_use] extern crate lazy_static;

#[macro_use] mod kern;

use kern::console as con;
use con::LogLevel::*;
use kern::driver::serial;
use kern::memory;
use kern::interrupts;
use kheap_allocator as kheap;

#[allow(dead_code)]
fn busy_wait () {
    for _ in 1..500000 {
        kern::util::cpu_relax();
    }
}

/// test rgb framebuffer drawing
fn display(fb: &multiboot2::FramebufferTag) {
    use core::ptr::*;
    let vga;

    printk!(Debug, "fb: {:#?}\n\r", fb);

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

fn test_kheap_allocator() {
    let mut v = vec![1,2,3,4];
    let b = alloc::boxed::Box::new(0xcafe);
    printk!(Debug, "v = {:?}, b = {:?}\n\r", v, b);
    let vs = vec!["Loading", "SOS2", "\n\r"];
    for s in vs {
        printk!(Debug, "{} ", s);
    }

    for i in 1..0x1000 * 40 {
        v.push(i);
    }

    let range = kheap::HEAP_RANGE.try().unwrap();
    printk!(Critical, "Heap usage: {:#x}\n\r", kheap::KHEAP_ALLOCATOR.lock().current - range.start);
}

extern {
    static _start: u64;
    static _end: u64;
}

#[no_mangle]
pub extern fn kernel_main(mb2_header: usize) {
    unsafe { 
        let mut com1 = serial::COM1.lock();
        com1.init();
    }

    con::clear();
    printk!(Info, "Loading SOS2....\n\r");

    let mbinfo = unsafe { multiboot2::load(mb2_header) };
    printk!(Info, "{:#?}\n\r", mbinfo);

    let (pa, pe) = unsafe {
        (&_start as *const _ as u64, &_end as *const _ as u64)
    };
    printk!(Debug, "_start {:#X}, _end {:#X}\n\r", pa, pe);

    let fb = mbinfo.framebuffer_tag().expect("framebuffer tag is unavailale");
    if cfg!(feature = "test") {
        display(&fb);
    }

    memory::init(&mbinfo);
    if cfg!(feature = "test") {
        test_kheap_allocator();
    }

    interrupts::init();
    if cfg!(feature = "test") {
        interrupts::test_idt();
    }

    loop {}
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
