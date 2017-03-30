
use kern::arch::io::{outb, inb};
const SERIAL_PORT: u16 = 0x3f8;   /* COM1 */

pub unsafe fn init_serial() {
    outb(SERIAL_PORT + 1, 0x00);    // Disable all interrupts
    outb(SERIAL_PORT + 3, 0x80);    // Enable DLAB (set baud rate divisor)
    outb(SERIAL_PORT + 0, 0x03);    // Set divisor to 3 (lo byte) 38400 baud
    outb(SERIAL_PORT + 1, 0x00);    //                  (hi byte)
    outb(SERIAL_PORT + 3, 0x03);    // 8 bits, no parity, one stop bit
    outb(SERIAL_PORT + 2, 0xC7);    // Enable FIFO, clear them, with 14-byte threshold
    outb(SERIAL_PORT + 4, 0x0B);    // IRQs enabled, RTS/DSR set
}
 
 
pub unsafe fn write_serial(a: u8) {
    unsafe fn is_transmit_empty() -> u8 {
        return inb(SERIAL_PORT + 5) & 0x20;
    }

    while is_transmit_empty() == 0 {
    }

    outb(SERIAL_PORT, a);
}
