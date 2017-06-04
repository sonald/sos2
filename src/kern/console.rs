use core::ptr;
use core::mem::size_of_val;
use core::ptr::{Unique, write_volatile};
use core::fmt::{Write, Result};
use core::intrinsics::transmute;
use spin::{Mutex, Once};

use ::kern::arch::port::{Port};
use ::kern::driver::video::terminal::FramebufferDriver;
use ::kern::driver::video::framebuffer::Framebuffer;

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

    pub fn bg(&self) -> Color {
        unsafe { transmute(self.0 >> 4) }
    }

    pub fn fg(&self) -> Color {
        unsafe { transmute(self.0 & 0xf) }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Char {
    pub ascii: u8,
    pub attr: Attribute
}

pub trait TerminalDriver {
    fn update_cursor(&mut self, row: usize, col: usize);
    fn draw_byte(&mut self, cursor: usize, byte: Char);
    fn get_max_cols(&self) -> usize; 
    fn get_max_rows(&self) -> usize;
    fn resizable(&self) -> bool;
    fn set_size(&mut self, rows: usize, cols: usize);
    fn scroll_up(&mut self, cursor: usize);
    fn clear(&mut self);
}


const CONSOLE_WIDTH: usize = 80;
const CONSOLE_HEIGHT: usize = 25;

struct Buffer {
    data: [Char; CONSOLE_WIDTH * CONSOLE_HEIGHT]
}

// text only terminal
pub struct ConsoleDriver {
    buf: Unique<Buffer>,
    crtc_reg: Port<u8>,
    crtc_data: Port<u8>,
}

impl TerminalDriver for ConsoleDriver {
    fn scroll_up(&mut self, cursor: usize) {
        let (cy, _) = (cursor / CONSOLE_WIDTH, cursor % CONSOLE_WIDTH);
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

    fn clear(&mut self) {
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
    }

    fn update_cursor(&mut self, row: usize, col: usize) {
        let v = row * CONSOLE_WIDTH + col;
        self.set_phy_cursor(v);
    }

    fn get_max_cols(&self) -> usize {
        CONSOLE_WIDTH
    }

    fn get_max_rows(&self) -> usize {
        CONSOLE_HEIGHT
    }

    fn set_size(&mut self, rows: usize, cols: usize) {
    }

    fn resizable(&self) -> bool {
        return false;
    }

    fn draw_byte(&mut self, cursor: usize, byte: Char) {
        unsafe {
            let p = &mut self.buf.get_mut().data[cursor];
            write_volatile(p, byte);
        }
    }
}

impl ConsoleDriver {
    const fn new() -> ConsoleDriver {
        use ::kern::memory::KERNEL_MAPPING;
        ConsoleDriver {
            buf: unsafe { Unique::new((KERNEL_MAPPING.KernelMap.start + 0xb8000) as *mut _) },
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
    
}



pub struct TerminalHelper<T> {
    pub cursor: usize,  // cursor as offset
    pub attr: Attribute, // current char attribute
    pub cols: usize,
    pub rows: usize,
    pub drv: T
}

impl<T: TerminalDriver> TerminalHelper<T> {
    pub const fn new(drv: T) -> TerminalHelper<T> {
        TerminalHelper {
            drv: drv,
            cursor: 0,
            attr: Attribute::new(Color::Green, Color::Black),
            cols: CONSOLE_WIDTH,
            rows: CONSOLE_HEIGHT
        }
    }

    pub fn update_cursor(&mut self, row: usize, col: usize) {
        let v = self.contract_cursor(row, col);
        self.cursor = v;
        self.drv.update_cursor(row, col);
    }

    pub fn advance(&mut self) -> usize {
        let old = self.cursor;
        let (mut cy, mut cx) = self.extract_cursor(old);

        cx += 1;
        if cx == self.cols {
            cx = 0;
            cy += 1;
        }

        if cy == self.rows {
            cy = self.rows - 1;
            self.drv.scroll_up(self.cursor);
        }

        self.update_cursor(cy, cx);
        old
    }

    /// extract offset-based cursor into (row, col) pair
    pub fn extract_cursor(&self, cursor: usize) -> (usize, usize) {
        (cursor / self.cols, cursor % self.cols)
    }

    /// combine (row, col) pair into offset-based cursor 
    pub fn contract_cursor(&self, row: usize, col: usize) -> usize {
        row * self.cols + col
    }

    fn set_attr(&mut self, val: Attribute) -> Attribute {
        let old = self.attr;
        self.attr = val;
        old
    }

    fn get_attr(&self) -> Attribute {
        self.attr
    }

    fn clear(&mut self) {
        self.drv.clear();
        self.update_cursor(0, 0);
    }

    fn retreat(&mut self) -> usize {
        let old = self.cursor;
        let (mut cy, mut cx) = self.extract_cursor(old);

        if old == 0 {
            return old;
        }

        if cx == 0 {
            cx = self.cols - 1;
            cy -= 1;
        } else {
            cx -= 1;
        }

        self.update_cursor(cy, cx);
        old
    }


    fn write_byte(&mut self, byte: u8) {
        let (mut cy, mut cx) = self.extract_cursor(self.cursor);
        let blank = Char {
            ascii: b' ',
            attr: Attribute::new(Color::White, Color::Black)
        };

        match byte {
            0x08 => { // backspace
                if cx > 0 {
                    let cur = self.cursor;
                    self.drv.draw_byte(cur, blank);
                    self.retreat();
                }
            }, 
            b'\t' => {
                cx = (cx + 8) & !0x7;
                if cx >= self.cols {
                    cx = self.cols - 1;
                }

                let old = self.cursor;
                self.update_cursor(cy, cx);
                for i in old..self.cursor {
                    self.drv.draw_byte(i, blank);
                }
            },
            b'\n' => {
                cy += 1;
                cx = 0;
                if cy == self.rows {
                    cy = self.rows - 1;
                    self.drv.scroll_up(self.cursor);
                }
                self.update_cursor(cy, cx);
            },
            b'\r' => {
                cx = 0;
                self.update_cursor(cy, cx);
            }, 
            _ => {
                if self.cursor >= self.cols * self.rows {
                    return;
                }
                let (cur, attr) = (self.cursor, self.attr);
                self.drv.draw_byte(cur, Char {ascii: byte, attr: attr});
                self.advance();
            }
        }
    }
}

pub enum Console {
    TextTerminal(TerminalHelper<ConsoleDriver>),
    FbTerminal(TerminalHelper<FramebufferDriver>)
}

impl Console {
    pub const fn new_with_text_only() -> Console {
        Console::TextTerminal(TerminalHelper::new(ConsoleDriver::new()))
    }

    pub fn new_with_fb(fb: Framebuffer) -> Console {
        let mut helper = TerminalHelper::new(FramebufferDriver::new(fb));
        helper.cols = helper.drv.get_max_cols();
        helper.rows = helper.drv.get_max_rows();
            
        Console::FbTerminal(helper)
    }

    pub fn putchar(&mut self, byte: u8) {
        match *self {
            Console::TextTerminal(ref mut drv) => drv.write_byte(byte),
            Console::FbTerminal(ref mut drv) => drv.write_byte(byte),
        }
    }

    pub fn set_attr(&mut self, val: Attribute) -> Attribute {
        match *self {
            Console::TextTerminal(ref mut drv) => drv.set_attr(val),
            Console::FbTerminal(ref mut drv) => drv.set_attr(val)
        }
    }

    pub fn clear(&mut self) {
        match *self {
            Console::TextTerminal(ref mut drv) => drv.clear(),
            Console::FbTerminal(ref mut drv) => drv.clear()
        }
    }

    pub fn update_cursor(&mut self, row: usize, col: usize) {
        match *self {
            Console::TextTerminal(ref mut drv) => drv.update_cursor(row, col),
            Console::FbTerminal(ref mut drv) => drv.update_cursor(row, col)
        }
    }

    pub fn get_cursor(&self) -> usize {
        match *self {
            Console::TextTerminal(ref drv) => drv.cursor,
            Console::FbTerminal(ref drv) => drv.cursor
        }
    }

    pub fn extract_cursor(&self, cursor: usize) -> (usize, usize) {
        match *self {
            Console::TextTerminal(ref drv) => drv.extract_cursor(cursor),
            Console::FbTerminal(ref drv) => drv.extract_cursor(cursor)
        }
    }

    /// safely call f without potential deadlock of console
    pub fn with<F>(con: &Mutex<Console>, row: usize, col: usize, f: F) where F: FnOnce() {
        let old = con.lock().get_cursor();
        con.lock().update_cursor(row, col);
        f();
        let (cy, cx) = con.lock().extract_cursor(old);
        con.lock().update_cursor(cy, cx);
    }


}

use kern::driver::serial;
impl Write for Console {
    fn write_str(&mut self, s: &str) -> Result {
        for b in s.bytes() {
            self.putchar(b);
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
pub static tty1: Mutex<Console> = Mutex::new(Console::new_with_text_only());

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Normal,
    Info,
    Warn,
    Critical
}

pub fn _unsafe_print(con: &mut Console, args: ::core::fmt::Arguments) -> ::core::fmt::Result {
    con.write_fmt(args)
}

// NOTE: use only in a situation unlocked access is safe, such as timer (with interrupt disabled)
macro_rules! unlocked_printk {
    ($lv:expr, $row:tt, $col:tt, $($arg:tt)*) => ({
        use $crate::kern::console::*;

        let mut con;
        let maybe_con = tty1.try_lock();
        let need_unlock = if maybe_con.is_none() {
            unsafe { tty1.force_unlock(); }
            con = tty1.lock();
            false
        } else {
            con = maybe_con.unwrap();
            true
        };

        let old_cur = con.get_cursor();
        con.update_cursor($row, $col);

        if $lv != LogLevel::Debug || cfg!(feature = "kdebug") {
            let attr = match $lv {
                LogLevel::Debug => Attribute::new(Color::Green, Color::Black),
                LogLevel::Normal => Attribute::new(Color::White, Color::Black),
                LogLevel::Info => Attribute::new(Color::Cyan, Color::Black),
                LogLevel::Warn => Attribute::new(Color::Red, Color::Black),
                LogLevel::Critical => Attribute::new(Color::LightRed, Color::White),
            };

            let old_attr = con.set_attr(attr);
            _unsafe_print(&mut con, format_args!($($arg)*)).unwrap();
            con.set_attr(old_attr);
        }

        let (cy, cx) = con.extract_cursor(old_cur);
        con.update_cursor(cy, cx);
        if need_unlock {
            unsafe { tty1.force_unlock(); }
        }
    });
}

macro_rules! printk {
    ($lv:expr, $($arg:tt)*) => ({
        use $crate::kern::console::*;

        if $lv != LogLevel::Debug || cfg!(feature = "kdebug") {
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
        }
    });
}

pub fn clear() {
    let mut con = tty1.lock();
    con.clear();
}

