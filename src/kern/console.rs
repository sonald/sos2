use core::ptr;
use core::mem::size_of_val;
use core::ptr::{Unique, write_volatile};
use core::fmt::{Write, Result};
use spin::Mutex;

use ::kern::arch::port::{Port};

const CRTC_ADDR_REG: u16 = 0x3D4;
const CRTC_ADDR_DATA: u16 = 0x3D5;
const CURSOR_LOCATION_HIGH_IND: u8 = 0x0E;
const CURSOR_LOCATION_LOW_IND: u8 = 0x0F;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
#[repr(u8)]
pub enum Color {
    Black      = 0,
    Blue       = 1,
    Green      = 2,
    Cyan       = 3,
    Red        = 4,
    Magenta    = 5,
    Brown      = 6,
    LightGray  = 7,
    DarkGray   = 8,
    LightBlue  = 9,
    LightGreen = 10,
    LightCyan  = 11,
    LightRed   = 12,
    Pink       = 13,
    Yellow     = 14,
    White      = 15,
}

#[derive(Debug, Clone, Copy)]
pub struct Attribute(u8);

impl Attribute {
    pub const fn new(fg: Color, bg: Color) -> Attribute {
        Attribute(((bg as u8) << 4) | (fg as u8))
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Char {
    ascii: u8,
    attr: Attribute
}

const CONSOLE_WIDTH: usize = 80;
const CONSOLE_HEIGHT: usize = 25;

struct Buffer {
    data: [Char; CONSOLE_WIDTH * CONSOLE_HEIGHT]
}

pub struct Console {
    buf: Unique<Buffer>,
    cursor: usize,  // cursor as offset
    attr: Attribute, // current char attribute
    crtc_reg: Port<u8>,
    crtc_data: Port<u8>
}

/// extract offset-based cursor into (row, col) pair
fn extract_cursor(cursor: usize) -> (usize, usize) {
    (cursor / CONSOLE_WIDTH, cursor % CONSOLE_WIDTH)
}

/// combine (row, col) pair into offset-based cursor 
fn contract_cursor(row: usize, col: usize) -> usize {
    row * CONSOLE_WIDTH + col
}

impl Console {
    const fn new() -> Console {
        Console {
            buf: unsafe { Unique::new(0xb8000 as *mut _) },
            cursor: 0,
            attr: Attribute::new(Color::White, Color::Black),
            crtc_reg: Port::new(CRTC_ADDR_REG),
            crtc_data: Port::new(CRTC_ADDR_DATA)
        }
    }

    fn set_phy_cursor(&mut self, cursor: usize) {
        let linear = cursor as u16;
        self.crtc_reg.write(CURSOR_LOCATION_HIGH_IND);
        self.crtc_data.write((linear >> 8) as u8);
        self.crtc_reg.write(CURSOR_LOCATION_LOW_IND);
        self.crtc_data.write(linear as u8);
    }

    fn update_cursor(&mut self, row: usize, col: usize) {
        let v = contract_cursor(row, col);
        self.cursor = v;
        self.set_phy_cursor(v);
    }

    pub fn set_attr(&mut self, val: Attribute) -> Attribute {
        let old = self.attr;
        self.attr = val;
        old
    }

    pub fn get_attr(&self) -> Attribute {
        self.attr
    }

    /// safely call f without potential deadlock of console
    pub fn with<F>(con: &Mutex<Console>, row: usize, col: usize, f: F) where F: FnOnce() {
        let old = con.lock().cursor;
        con.lock().update_cursor(row, col);
        f();
        let (cy, cx) = extract_cursor(old);
        con.lock().update_cursor(cy, cx);
    }

    pub fn clear(&mut self) {
        let blank_line = [Char {
            ascii: b' ',
            attr: Attribute::new(Color::White, Color::Black)
        }; CONSOLE_WIDTH];

        unsafe {
            let data = (&mut self.buf.get_mut().data).as_mut_ptr();
            for off in 0..CONSOLE_HEIGHT {
                ptr::copy_nonoverlapping((&blank_line).as_ptr(),
                    data.offset((off * CONSOLE_WIDTH) as isize), size_of_val(&blank_line));
            }
        }

        self.update_cursor(0, 0);
    }


    /// move cursor forward by one and do correct scrolling up
    /// return old cursor
    fn advance(&mut self) -> usize {
        let old = self.cursor;
        let (mut cy, mut cx) = extract_cursor(old);

        cx += 1;
        if cx == CONSOLE_WIDTH {
            cx = 0;
            cy += 1;
        }

        if cy == CONSOLE_HEIGHT {
            cy = CONSOLE_HEIGHT - 1;
            self.scroll_up();
        }

        self.update_cursor(cy, cx);
        old
    }

    fn retreat(&mut self) -> usize {
        let old = self.cursor;
        let (mut cy, mut cx) = extract_cursor(old);

        if old == 0 {
            return old;
        }

        if cx == 0 {
            cx = CONSOLE_WIDTH - 1;
            cy -= 1;
        } else {
            cx -= 1;
        }

        self.update_cursor(cy, cx);
        old
    }

    fn scroll_up(&mut self) {
        let (cy, _) = extract_cursor(self.cursor);
        let blank_line = [Char {
            ascii: b' ',
            attr: Attribute::new(Color::White, Color::Black)
        }; CONSOLE_WIDTH];
        let off = CONSOLE_WIDTH * (CONSOLE_HEIGHT - 1);


        if cy < CONSOLE_HEIGHT - 1 {
            return;
        }

        unsafe {
            let data = (&mut self.buf.get_mut().data).as_mut_ptr();
            ptr::copy(data.offset(CONSOLE_WIDTH as isize), data, off);
            ptr::copy_nonoverlapping(&blank_line, data.offset(off as isize) as *mut _, 1);
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        let (mut cy, mut cx) = extract_cursor(self.cursor);
        let blank = Char {
            ascii: b' ',
            attr: Attribute::new(Color::White, Color::Black)
        };

        match byte {
            0x08 => { // backspace
                if cx > 0 {
                    unsafe {
                        let p = &mut self.buf.get_mut().data[self.cursor];
                        write_volatile(p, blank);
                    }
                    self.retreat();
                }
            }, 
            b'\t' => {
                cx = (cx + 8) & !0x7;
                if cx >= CONSOLE_WIDTH {
                    cx = CONSOLE_WIDTH - 1;
                }

                let old = self.cursor;
                self.update_cursor(cy, cx);
                unsafe {
                    let data = (&mut self.buf.get_mut().data).as_mut_ptr();
                    for i in old..self.cursor {
                        write_volatile(data.offset(i as isize), blank);
                    }
                }
            },
            b'\n' => {
                cy += 1;
                cx = 0;
                if cy == CONSOLE_HEIGHT {
                    cy = CONSOLE_HEIGHT - 1;
                    self.scroll_up();
                }
                self.update_cursor(cy, cx);
            },
            b'\r' => {
                cx = 0;
                self.update_cursor(cy, cx);
            }, 
            _ => {
                if self.cursor >= CONSOLE_WIDTH * CONSOLE_HEIGHT {
                    return;
                }
                unsafe {
                    let p = &mut self.buf.get_mut().data[self.cursor];
                    write_volatile(p, Char {ascii: byte, attr: self.attr});
                }
                self.advance();
            }
        }
    }
}


use kern::driver::serial;
impl Write for Console {
    fn write_str(&mut self, s: &str) -> Result {
        for b in s.bytes() {
            self.write_byte(b);
        }

        unsafe {
            let mut com1 = serial::COM1.lock();
            for b in s.bytes() {
                com1.write(b);
            }
        }
        Ok(())
    }
}

#[allow(non_upper_case_globals)]
pub static tty1: Mutex<Console> = Mutex::new(Console::new());

macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::kern::console::_print(format_args!($($arg)*)).unwrap();
    });
}

pub fn _print(args: ::core::fmt::Arguments) -> ::core::fmt::Result {
    use core::fmt::Write;
    let mut con = tty1.lock();
    con.write_fmt(args)
}

pub enum LogLevel {
    Debug,
    Normal,
    Info,
    Warn,
    Critical
}

macro_rules! printk {
    ($lv:expr, $($arg:tt)*) => ({
        use $crate::kern::console::*;

        let attr = match $lv {
            LogLevel::Debug => Attribute::new(Color::Green, Color::Black),
            LogLevel::Normal => Attribute::new(Color::White, Color::Black),
            LogLevel::Info => Attribute::new(Color::Cyan, Color::Black),
            LogLevel::Warn => Attribute::new(Color::Red, Color::Black),
            LogLevel::Critical => Attribute::new(Color::LightRed, Color::White),
        };

        let old_attr = {
            let mut con = tty1.lock();
            con.set_attr(attr)
        };
        print!( $($arg)* );
        {
            let mut con = tty1.lock();
            con.set_attr(old_attr);
        }
    });
}

pub fn clear() {
    let mut con = tty1.lock();
    con.clear();
}

