#![feature(lang_items)]
#![feature(start)]
#![feature(asm)]
#![no_std]

extern crate libsos2;

pub fn test() {
    let mut a0 = 1;
    let mut a1 = 2;
    let mut a2 = 3;
    let mut a3 = 4;
    let mut a4 = 5;
    let mut a5 = 6;
    let buf = [b'u', b's', b'e', b'r', b's', b'p', b'a', b'c', b'e'];
    //let buf = b"userspace";

    loop {
        unsafe {
            asm!("
                pushq %rcx
                pushq %r11
                 syscall
                 popq %r11
                 popq %rcx"
                 :
                 :"{rax}"(16), // write is 16
                 "{rdi}"(a0),
                 "{rsi}"(&buf as *const _ as usize),
                 "{rdx}"(9),
                 "{r8}"(a3),
                 "{r9}"(a4),
                 "{r10}"(a5)
                 :"rcx", "r11"
                 :"volatile"
                 ); 
        }
        a0 += 1;
        a1 += 1;
        a2 += 1;
        a3 += 1;
        a4 += 1;
        a5 += 1;

        let mut i = 1;
        while i < 10000 {
            unsafe {
                asm!("pause":::"memory":"volatile");
            }
            i += 1;
        }
    }
}

#[no_mangle]
#[start]
pub fn start(_argc: isize, _argv: *const *const u8) -> isize {
    test();
    0
}
