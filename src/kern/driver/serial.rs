use kern::arch::port::Port;
use spin::Mutex;

const SERIAL_PORT: u16 = 0x3f8;   /* COM1 */

#[derive(Debug)]
pub struct Serial {
    ports: [Port<u8>; 8]
}

pub static COM1: Mutex<Serial> = Mutex::new(Serial::new(SERIAL_PORT));

impl Serial {
    pub const fn new(base: u16) -> Serial {
        Serial { 
            ports: [
                Port::new(base),
                Port::new(base + 1),
                Port::new(base + 2),
                Port::new(base + 3),

                Port::new(base + 4),
                Port::new(base + 5),
                Port::new(base + 6),
                Port::new(base + 7),
            ]
        }
    }

    pub unsafe fn init(&mut self) {
        self.ports[1].write(0x00);    // Disable all interrupts
        self.ports[3].write(0x80);    // Enable DLAB (set baud rate divisor)
        self.ports[0].write(0x03);    // Set divisor to 3 (lo byte) 38400 baud
        self.ports[1].write(0x00);    //                  (hi byte)
        self.ports[3].write(0x03);    // 8 bits, no parity, one stop bit
        self.ports[2].write(0xC7);    // Enable FIFO, clear them, with 14-byte threshold
        self.ports[4].write(0x0B);    // IRQs enabled, RTS/DSR set
    }

    unsafe fn is_transmit_empty(&mut self) -> bool {
        self.ports[5].read() & 0x20 != 0
    }

    pub unsafe fn write(&mut self, a: u8) {
        while !self.is_transmit_empty() {
        }

        self.ports[0].write(a);
    }

    unsafe fn serial_received(&mut self) -> bool {
        self.ports[5].read() & 0x1 != 0
    }

    pub unsafe fn read(&mut self) -> u8 {
        while !self.serial_received() {
        }

        self.ports[0].read()
    }
}



