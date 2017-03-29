#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![no_std]

extern crate rlibc;
extern crate multiboot2;
extern crate spin;

mod kern;

use kern::console as con;
use core::fmt::Write;

fn busy_wait () {
    for i in 1..500000 {
    }
}

#[no_mangle]
pub extern fn kernel_main(mb2_header: usize) {
    con::clear();
    con::display("Loading sos2....", 30, 10);
    con::display("TM", 78, 24);

    for i in 1..24 {
        writeln!(con::tty1.lock(), "#{} \t{} \t{}", i, i, i).unwrap();
        busy_wait();
    }
    write!(con::tty1.lock(), "Loading SOS2....\n").unwrap();
    write!(con::tty1.lock(), "aofos nofanfons noaf ndosfn anf osafnosafn as oo\n").unwrap();
    write!(con::tty1.lock(), "{}", 12.3 / 2.45).unwrap();
    writeln!(con::tty1.lock(), "current time {} + {} = {}", 12, 34, 12 + 34).unwrap();
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
