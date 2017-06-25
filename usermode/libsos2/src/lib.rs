#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(asm)]
//#![feature(alloc, collections)]
#![feature(naked_functions)]
#![no_std]

extern crate rlibc;
//extern crate alloc;
//#[macro_use] extern crate collections;

#[allow(dead_code)]
fn busy_wait () {
    for _ in 1..500000 {
        unsafe { asm!("pause"::::"volatile"); }
    }
}

#[lang = "eh_personality"]
extern fn eh_personality() {}

#[lang = "panic_fmt"] 
#[no_mangle]
pub extern fn panic_fmt(fmt: core::fmt::Arguments, file: &'static str, line: u32) -> ! {
    loop {}
}

#[lang = "eh_unwind_resume"]
#[no_mangle]
pub extern fn rust_eh_unwind_resume() {}

/// dummy, this should never gets called
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}
