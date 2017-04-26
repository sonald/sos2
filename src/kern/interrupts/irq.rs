use ::kern::arch::port::{UnsafePort, Port};
use spin::Mutex;

/**
 * ref: http://wiki.osdev.org/8259_PIC
 */
const PIC1_BASE: u16 = 0x20;	/* IO base address for master PIC */
const PIC2_BASE: u16 = 0xA0;	/* IO base address for slave PIC */
// Initial IRQ mask has interrupt 2 enabled (for slave 8259A).
const IRQ_MASK: u16 = 0xffff & !(1<<2);

/// vector numbers for IRQs
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Irqs {
    TIMER = 32, // PIT
    KBD = 33,
    IRQ2 = 34, // slave
    IRQ3 = 35, // serial2
    IRQ4 = 36, // serial1
    IRQ5 = 37, // LPT2
    IRQ6 = 38, // floppy
    IRQ7 = 39, // LPT1
    IRQ8 =  40, // RTC
    IRQ9 =  41, // IRQ2
    IRQ10 = 42, // reserve
    IRQ11 = 43, // reserve
    MOUSE = 44, // PS/2 mouse
    IRQ13 = 45, // FPU
    ATA1 = 46, // ATA HD1
    ATA2 = 47, // ATA HD2
}

///8259A chip
pub struct Pic8259A {
    offset: u8,
    command: UnsafePort<u8>,
    data: UnsafePort<u8>
}

impl Pic8259A {
    pub const unsafe fn new(base: u16, offset: u8) -> Pic8259A {
        Pic8259A {
            offset: offset,
            command: UnsafePort::new(base),
            data: UnsafePort::new(base+1)
        }
    } 

    pub unsafe fn eoi(&mut self) {
        self.command.write(0x20);
    }
}

/// represent two cascaded pic chips
pub struct PicChain {
    pics: [Pic8259A; 2],
    irqmask: u16
}

pub static PIC_CHAIN: Mutex<PicChain> = Mutex::new(unsafe {PicChain::new()});

/* reinitialize the PIC controllers, giving them specified vector offsets
   rather than 8h and 70h, as configured by default */
 
const ICW1_ICW4: u8 =	0x01;		/* ICW4 (not) needed */
#[allow(dead_code)]
const ICW1_SINGLE: u8 =	0x02;		/* Single (cascade) mode */
#[allow(dead_code)]
const ICW1_INTERVAL4: u8 =	0x04;		/* Call address interval 4 (8) */
#[allow(dead_code)]
const ICW1_LEVEL: u8 =	0x08;		/* Level triggered (edge) mode */
#[allow(dead_code)]
const ICW1_INIT: u8 =	0x10;		/* Initialization - required! */

const ICW4_8086: u8 =	0x01;		/* 8086/88 (MCS-80/85) mode */
#[allow(dead_code)]
const ICW4_AUTO: u8 =	0x02;		/* Auto (normal) EOI */
#[allow(dead_code)]
const ICW4_BUF_SLAVE: u8 =	0x08;		/* Buffered mode/slave */
#[allow(dead_code)]
const ICW4_BUF_MASTER: u8 =	0x0C;		/* Buffered mode/master */
#[allow(dead_code)]
const ICW4_SFNM: u8 =	0x10;		/* Special fully nested (not) */

impl PicChain {
    pub const unsafe fn new() -> PicChain {
        PicChain {
            pics: [
                Pic8259A::new(PIC1_BASE, 0x20),
                Pic8259A::new(PIC2_BASE, 0x28),
            ],
            irqmask: IRQ_MASK
        }
    }

    pub unsafe fn init(&mut self) {
        let mut port80 = Port::new(0x80);
        let mut io_wait = || port80.write(0 as u8);

        self.pics[0].command.write(ICW1_INIT + ICW1_ICW4);
        io_wait();

        self.pics[1].command.write(ICW1_INIT + ICW1_ICW4);
        io_wait();

        self.pics[0].data.write(self.pics[0].offset);
        io_wait();
        self.pics[1].data.write(self.pics[1].offset);
        io_wait();

        // ICW3: tell Master PIC that there is a slave PIC at IRQ2
        self.pics[0].data.write(0b0000_0100);
        io_wait();
        // ICW3: tell Slave PIC its cascade identity (0000 0010)
        self.pics[1].data.write(0x2);
        io_wait();

        self.pics[0].data.write(ICW4_8086);
        io_wait();
        self.pics[1].data.write(ICW4_8086);
        io_wait();

        let mask =self.irqmask;
        self.setmask(mask);
    }

    pub unsafe fn eoi(&mut self, isr: usize) {
        assert!(isr < 0x10);
        if isr >= 8 {
            self.pics[1].eoi();
        }
        self.pics[0].eoi();
    }


    unsafe fn setmask(&mut self, mask: u16) {
        self.irqmask = mask;
        self.pics[0].data.write(mask as u8);
        self.pics[1].data.write((mask >> 8) as u8);
    }

    pub unsafe fn enable(&mut self, irq: usize) {
        assert!(irq >= 0x20 && irq < 0x30);
        let irq = (irq - 0x20) as u16;
        let mask = self.irqmask & !(1<<irq);
        self.setmask(mask);
    }
}
