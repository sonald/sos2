#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(asm)]
#![no_std]

extern crate rlibc;
extern crate multiboot2;
extern crate spin;

#[macro_use]
mod kern;

use kern::console as con;
use con::LogLevel::*;
use kern::driver::serial;

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

pub fn display(fb: &multiboot2::FramebufferTag) {
    use core::ptr::*;
    use core::mem::size_of_val;
    let vga;

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

#[no_mangle]
pub extern fn kernel_main(mb2_header: usize) {
    unsafe { serial::init_serial(); }

    con::clear();
    printk!(Info, "Loading SOS2....\n\r");

    let mbinfo = unsafe { multiboot2::load(mb2_header) };

    if let Some(mmap) = mbinfo.memory_map_tag() {
        for (i, a) in mmap.memory_areas().enumerate() {
            printk!(Info, "#{}: {:?}\n\r", i, a);
        }
    }

    if let Some(fb) = mbinfo.framebuffer_tag() {
        printk!(Debug, "fb: {:?}\n\r", fb);
        display(&fb);
    }
}

#[lang = "eh_personality"]
extern fn eh_personality() {}

#[lang = "panic_fmt"] 
#[no_mangle] pub extern fn panic_fmt() -> ! {
    loop {}
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
