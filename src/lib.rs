#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![no_std]

extern crate rlibc;
extern crate multiboot2;
extern crate spin;

#[macro_use]
mod kern;

use kern::console as con;
use core::fmt::Write;
use con::LogLevel::*;

fn busy_wait () {
    for i in 1..500000 {
    }
}

#[no_mangle]
pub extern fn kernel_main(mb2_header: usize) {
    con::clear();

    for i in 1..24 {
        writeln!(con::tty1.lock(), "#{} \t{} \t{}", i, i, i).unwrap();
        //busy_wait();
    }
    printk!(Info, "Loading SOS2....\n");
    printk!(Debug, "values: {}, {}, {}\n", "hello", 12 / 5, 12.34 / 3.145);
    printk!(Debug, "{}\n", {println!("inner"); "outer"});
    printk!(Warn, "kernel log\n");
    printk!(Critical, "kernel log\n");

    let mbinfo = unsafe { multiboot2::load(mb2_header) };

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
