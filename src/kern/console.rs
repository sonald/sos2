use core::ptr;
use core::mem::size_of_val;
use core::ptr::{Unique, write_volatile};
use core::fmt::{Write, Result};
use spin::Mutex;

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
    const fn new(fg: Color, bg: Color) -> Attribute {
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
    attr: Attribute // current char attribute

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
        }
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

        self.cursor = contract_cursor(cy, cx);
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

        self.cursor = contract_cursor(cy, cx);
        old
    }

    fn scroll_up(&mut self) {
        let (cy, cx) = extract_cursor(self.cursor);
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
            ptr::copy_nonoverlapping((&blank_line).as_ptr(),
                data.offset(off as isize), size_of_val(&blank_line));
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
                self.cursor = contract_cursor(cy, cx);
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
                self.cursor = contract_cursor(cy, cx);
            },
            b'\r' => {
                cx = 0;
                self.cursor = contract_cursor(cy, cx);
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


impl Write for Console {
    fn write_str(&mut self, s: &str) -> Result {
        for b in s.bytes() {
            self.write_byte(b);
        }
        Ok(())
    }
}

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

#[warn(non_upper_case_globals)]
pub static tty1: Mutex<Console> = Mutex::new(Console::new());
