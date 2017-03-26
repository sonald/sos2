#![feature(lang_items)]
#![no_std]

extern crate rlibc;

pub extern fn display(msg: &str, col: isize, row: isize) {
    let vga;
    unsafe {
        vga = 0xb8000 as *mut u8;
        for (i, b) in msg.bytes().enumerate() {
            let off = (row * 80 + col + i as isize) * 2;
            *vga.offset(off) = b;
            *vga.offset(off+1) = 0x4f;
        }
    }
}

pub extern fn clear() {
    let vga = 0xb8000 as *mut _;
    let blank = [0_u8; 80 * 24 * 2];
    unsafe { *vga = blank; }
}

#[no_mangle]
pub extern fn kernel_main() {
    clear();
    display("Loading sos2....", 30, 10);
    display("M", 79, 24);
    loop {}
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
