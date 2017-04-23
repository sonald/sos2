#![allow(dead_code)]

#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    asm!("inb $1, $0" : "={al}"(val) : "{dx}N"(port) ::"volatile");
    val
}

#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    asm!("outb $0, $1" :: "{al}"(val), "{dx}"(port));
}

#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let val: u16;
    asm!("inw $1, $0" : "={ax}"(val) : "{dx}"(port) ::"volatile");
    val
}

#[inline]
pub unsafe fn outw(port: u16, val: u16) {
    asm!("outw $0, $1" :: "{ax}"(val), "{dx}"(port));
}


#[inline]
pub unsafe fn outl(port: u16, val: u32) {
    asm!("outl $0, $1" :: "{eax}"(val), "{dx}"(port));
}

#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    let val: u32;
    asm!("inl $1, $0" : "={eax}"(val) : "{dx}"(port) ::"volatile");
    val
}

