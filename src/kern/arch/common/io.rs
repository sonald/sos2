#![allow(dead_code)]

#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    asm!("inb $1, $0" : "={al}"(val) : "{dx}N"(port));
    val
}

#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    asm!("outb $0, $1" :: "{al}"(val), "{dx}N"(port));
}

#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let val: u16;
    asm!("inw $1, $0" : "={ax}"(val) : "{dx}N"(port));
    val
}

#[inline]
pub unsafe fn outw(port: u16, val: u16) {
    asm!("outw $0, $1" :: "{ax}"(val), "{dx}N"(port));
}

