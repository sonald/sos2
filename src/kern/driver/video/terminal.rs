use core::ptr::{Unique};
use core::fmt::{Write, Result};

use ::kern::console::{TerminalDriver, Color, Char};

use super::framebuffer::*;
use super::builtin_font::{BUILTIN_FONTINFO, FontInfo};

use spin::Once;

pub struct FramebufferDriver {
    fb: Framebuffer,
    // used cols & rows
    width: usize,
    height: usize,
    // maximum supported 

    max_cols: usize,
    max_rows: usize
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

impl FramebufferDriver {
    pub fn new(fb: Framebuffer) -> FramebufferDriver {
        let w = fb.width / BUILTIN_FONTINFO.xadvance as i32;
        let h = fb.height / BUILTIN_FONTINFO.yadvance as i32;
        FramebufferDriver {
            fb: fb,
            max_cols: w as usize,
            max_rows: h as usize,

            width: w as usize,
            height: h as usize
        }
    }
}

impl TerminalDriver for FramebufferDriver {
    //TODO: draw cursor
    fn update_cursor(&mut self, row: usize, col: usize) {
    }

    fn draw_byte(&mut self, cursor: usize, byte: Char) {
        let (ch, fg, bg) = (byte.ascii, byte.attr.fg(), byte.attr.bg());

        let p = {
            let (cy, cx) = (cursor / self.width, cursor % self.width);
            let FontInfo {xadvance: fw, yadvance: fh} = BUILTIN_FONTINFO;
            Point {
                x: cx as i32 * fw as i32,
                y: cy as i32 * fh as i32
            }
        };
        self.fb.draw_char(p, ch, COLORMAP[fg as usize], COLORMAP[bg as usize]);
    }

    fn get_max_cols(&self) -> usize {
        self.max_cols
    }

    fn get_max_rows(&self) -> usize {
        self.max_rows
    }

    fn set_size(&mut self, rows: usize, cols: usize) {
        self.width = rows;
        self.height = cols;
        //update fb
    }

    fn resizable(&self) -> bool {
        return true;
    }

    fn scroll_up(&mut self, cursor: usize) {
        let (cy, _) = (cursor / self.width, cursor % self.width);

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
    }
}


