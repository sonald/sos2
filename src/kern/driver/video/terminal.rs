use core::ptr::{Unique};
use core::fmt::{Write, Result};

use ::kern::console::{TerminalDevice, Attribute, Color, Char};

use super::framebuffer::*;
use super::builtin_font::{BUILTIN_FONTINFO, FontInfo};

use spin::Once;

pub struct FramebufferTerminal {
    fb: Framebuffer,
    cursor: usize,  // cursor as offset
    attr: Attribute, // current char attribute
    width: usize,
    height: usize,
}

// map from Console::Color to Rgba
const COLORMAP: [Rgba; 16] = [
    Rgba::new(0x000000),
    Rgba::new(0x0000ff),
    Rgba::new(0x00ff00),
    Rgba::new(0x00ffff),
    Rgba::new(0xff0000),
    Rgba::new(0xff00ff),
    Rgba::new(0xa52a2a),
    Rgba::new(0xd3d3d3),
    Rgba::new(0xbebebe),
    Rgba::new(0xadd8e6),
    Rgba::new(0x90ee90),
    Rgba::new(0xe0ffff),
    Rgba::new(0xcd5c5c),
    Rgba::new(0xee00ee),
    Rgba::new(0xffff00),
    Rgba::new(0xffffff),
];

impl FramebufferTerminal {
    pub fn new(fb: Framebuffer) -> FramebufferTerminal {
        let w = fb.width / BUILTIN_FONTINFO.xadvance as i32;
        let h = fb.height / BUILTIN_FONTINFO.yadvance as i32;
        FramebufferTerminal {
            fb: fb,
            cursor: 0,
            attr: Attribute::new(Color::Green, Color::Black),
            width: w as usize,
            height: h as usize
        }
    }

    fn advance(&mut self) -> usize {
        let old = self.cursor;
        let (mut cy, mut cx) = self.extract_cursor(old);

        cx += 1;
        if cx == self.width {
            cx = 0;
            cy += 1;
        }

        if cy == self.height {
            cy = self.height - 1;
            self.scroll_up();
        }

        self.update_cursor(cy, cx);
        old
    }

    /// extract offset-based cursor into (row, col) pair
    fn extract_cursor(&self, cursor: usize) -> (usize, usize) {
        (cursor / self.width, cursor % self.width)
    }

    /// combine (row, col) pair into offset-based cursor 
    fn contract_cursor(&self, row: usize, col: usize) -> usize {
        row * self.width + col
    }

    //TODO: draw cursor
    fn update_cursor(&mut self, row: usize, col: usize) {
        let v = self.contract_cursor(row, col);
        self.cursor = v;
    }

    fn retreat(&mut self) -> usize {
        let old = self.cursor;
        let (mut cy, mut cx) = self.extract_cursor(old);

        if old == 0 {
            return old;
        }

        if cx == 0 {
            cx = self.width - 1;
            cy -= 1;
        } else {
            cx -= 1;
        }

        self.update_cursor(cy, cx);
        old
    }

    fn draw_byte(&mut self, cursor: usize, byte: Char) {
        let (ch, fg, bg) = (byte.ascii, byte.attr.fg(), byte.attr.bg());

        let p = {
            let (cy, cx) = self.extract_cursor(cursor);
            let FontInfo {xadvance: fw, yadvance: fh} = BUILTIN_FONTINFO;
            Point {
                x: cx as i32 * fw as i32,
                y: cy as i32 * fh as i32
            }
        };
        self.fb.draw_char(p, ch, COLORMAP[fg as usize], COLORMAP[bg as usize]);
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
                    self.draw_byte(cur, blank);
                    self.retreat();
                }
            }, 
            b'\t' => {
                cx = (cx + 8) & !0x7;
                if cx >= self.width {
                    cx = self.width - 1;
                }

                let old = self.cursor;
                self.update_cursor(cy, cx);
                for i in old..self.cursor {
                    self.draw_byte(i, blank);
                }
            },
            b'\n' => {
                cy += 1;
                cx = 0;
                if cy == self.height {
                    cy = self.height - 1;
                    self.scroll_up();
                }
                self.update_cursor(cy, cx);
            },
            b'\r' => {
                cx = 0;
                self.update_cursor(cy, cx);
            }, 
            _ => {
                if self.cursor >= self.width * self.height {
                    return;
                }
                let (cur, attr) = (self.cursor, self.attr);
                self.draw_byte(cur, Char {ascii: byte, attr: attr});
                self.advance();
            }
        }
    }
}

impl TerminalDevice for FramebufferTerminal {
    fn set_attr(&mut self, val: Attribute) -> Attribute {
        let old = self.attr;
        self.attr = val;
        old
    }

    fn get_attr(&self) -> Attribute {
        self.attr
    }

    fn scroll_up(&mut self) {
        let (cy, _) = self.extract_cursor(self.cursor);

        if cy < self.height - 1 {
            return;
        }

        let fh = BUILTIN_FONTINFO.yadvance as i32;
        let (width, height) = (self.fb.width, self.fb.height);
        self.fb.blit_copy(Point{x: 0, y: 0}, Point{x: 0, y: fh}, width, height - fh);
        self.fb.fill_rect(Point{x: 0, y: height - fh}, width, fh, Rgba(0));
    }

    fn clear(&mut self) {
        let (w, h) = (self.fb.width, self.fb.height);
        self.fb.fill_rect(Point{x: 0, y: 0}, w, h, Rgba(0));
        self.update_cursor(0, 0);
    }

    fn putchar(&mut self, byte: u8) {
        self.write_byte(byte)
    }
}

use kern::driver::serial;
impl Write for FramebufferTerminal {
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

