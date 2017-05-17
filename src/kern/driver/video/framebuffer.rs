use core::ptr::{Unique, write_volatile};
use core::slice::from_raw_parts_mut;
use core::slice::SliceExt;
use multiboot2;
use ::kern::memory::KERNEL_MAPPING;
use super::builtin_font::{BUILTIN_FONT, BUILTIN_FONTINFO};

#[derive(Debug, Clone, Copy)]
#[repr(packed)]
pub struct Rgba(pub u32);

impl Rgba {
    pub const fn new(val: u32) -> Rgba {
        Rgba(val)
    }

    pub fn from(r: u8, g: u8, b: u8) -> Rgba {
        let v = (r as u32) << 16 | (g as u32) << 8 | (b as u32);
        Rgba(v)
    }

    pub const fn a(&self) -> u8 {
        (self.0 >> 24) as u8
    }

    pub const fn r(&self) -> u8 {
        (self.0 >> 16) as u8
    }

    pub const fn g(&self) -> u8 {
        (self.0 >> 8) as u8
    }

    pub const fn b(&self) -> u8 {
        self.0 as u8
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32
}

pub struct Framebuffer {
    buf: Unique<Rgba>,
    pub width: usize,
    pub height: usize,
    pub pitch: usize
}

impl Framebuffer {
    pub fn new(fb: &multiboot2::FramebufferTag) -> Framebuffer {
        assert!(fb.frame_type == multiboot2::FramebufferType::Rgb);
        assert!(fb.bpp == 32);

        let base = fb.addr as usize + KERNEL_MAPPING.KernelMap.start;

        unsafe {
            Framebuffer {
                buf: Unique::new(base as *mut Rgba),
                width: fb.width as usize,
                height: fb.height as usize,
                pitch: fb.pitch as usize
            }
        }
    }

    pub unsafe fn get_mut(&mut self) -> *mut Rgba {
        self.buf.get_mut() as *mut _
    }

    // based on wikipedia bresenham line algorithm
    pub fn draw_line(&mut self, p1: Point, p2: Point, rgb: Rgba) {
        let dx = (p2.x - p1.x).abs();
        let dy = (p2.y - p1.y).abs();
        let mut e = 0;
        if dx >= dy {
            let (p1, p2) = match p1.x > p2.x {
                true => (p2, p1),
                false => (p1, p2)
            };
            let mut y = p1.y;
            let dir = if p2.y >= p1.y {1} else {-1};

            for x in p1.x..p2.x+1 {
                self.draw_pixel(Point{x: x, y: y}, rgb);
                e += 2 * dy;
                if e > 1 {
                    e -= 2 * dx;
                    y += dir;
                }
            }
        } else {
            let (p1, p2) = match p1.y > p2.y {
                true => (p2, p1),
                false => (p1, p2)
            };
            let mut x = p1.x;
            let dir = if p2.x >= p1.x {1} else {-1};

            for y in p1.y..p2.y+1 {
                self.draw_pixel(Point{x: x, y: y}, rgb);
                e += 2 * dx;
                if e > 1 {
                    e -= 2 * dy;
                    x += dir;
                }
            }
        }
    }

    fn draw_pixel(&mut self, p: Point, rgb: Rgba) {
        unsafe {
            let c = self.get_mut().offset((p.y * self.width as i32 + p.x) as isize);
            write_volatile(c, rgb);
        }
    }

    // based on http://web.engr.oregonstate.edu/~sllu/bcircle.pdf
    pub fn draw_circle(&mut self, center: Point, radius: i32, rgb: Rgba) {
        let Point {x: x0, y: y0} = center;
        let mut x = radius;
        let mut y = 0;
        let mut err = 0;
        let mut xchange = 1 - 2*radius;
        let mut ychange = 1;

        while x >= y {
            self.draw_pixel(Point{x: x0 + x, y: y0 + y}, rgb);
            self.draw_pixel(Point{x: x0 + y, y: y0 + x}, rgb);
            self.draw_pixel(Point{x: x0 - y, y: y0 + x}, rgb);
            self.draw_pixel(Point{x: x0 - x, y: y0 + y}, rgb);
            self.draw_pixel(Point{x: x0 - x, y: y0 - y}, rgb);
            self.draw_pixel(Point{x: x0 - y, y: y0 - x}, rgb);
            self.draw_pixel(Point{x: x0 + y, y: y0 - x}, rgb);
            self.draw_pixel(Point{x: x0 + x, y: y0 - y}, rgb);

            y += 1;
            err += ychange;
            ychange += 2;
            if 2 * (err + xchange) + ychange > 0 {
                x -= 1;
                err += xchange;
                xchange += 2;
            }
        }
    }

    pub fn spread_circle(&mut self, center: Point, radius: i32, rgb: Rgba) {
        let Point {x: x0, y: y0} = center;
        let mut x = radius;
        let mut y = 0;
        let mut err = 0;
        let mut xchange = 1 - 2*radius;
        let mut ychange = 1;

        while x >= y {
            self.draw_line(center, Point{x: x0 + x, y: y0 + y}, rgb);
            self.draw_line(center, Point{x: x0 + y, y: y0 + x}, rgb);
            self.draw_line(center, Point{x: x0 - y, y: y0 + x}, rgb);
            self.draw_line(center, Point{x: x0 - x, y: y0 + y}, rgb);
            self.draw_line(center, Point{x: x0 - x, y: y0 - y}, rgb);
            self.draw_line(center, Point{x: x0 - y, y: y0 - x}, rgb);
            self.draw_line(center, Point{x: x0 + y, y: y0 - x}, rgb);
            self.draw_line(center, Point{x: x0 + x, y: y0 - y}, rgb);

            y += 1;
            err += ychange;
            ychange += 2;
            if 2 * (err + xchange) + ychange > 0 {
                x -= 1;
                err += xchange;
                xchange += 2;
            }
        }
    }

    pub fn draw_rect(&mut self, top_left: Point, width: i32, height: i32, rgb: Rgba) {
        use core::cmp::min;
        let width = min(self.width as i32 - top_left.x, width);
        let height = min(self.height as i32 - top_left.y, height);

        let (l, r, t, b) = (top_left.x, top_left.x + width - 1, top_left.y, top_left.y + height - 1);
        self.draw_line(Point{x: l, y: t}, Point{x: r, y: t}, rgb);
        self.draw_line(Point{x: l, y: t}, Point{x: l, y: b}, rgb);
        self.draw_line(Point{x: r, y: t}, Point{x: r, y: b}, rgb);
        self.draw_line(Point{x: l, y: b}, Point{x: r, y: b}, rgb);
    }

    pub fn fill_rect(&mut self, top_left: Point, width: i32, height: i32, rgb: Rgba) {
        use core::ptr::copy_nonoverlapping;
        use core::cmp::min;

        let width = min(self.width as i32 - top_left.x, width);
        let height = min(self.height as i32 - top_left.y, height);

        let base = top_left.y as isize * self.width as isize;
        // our kernel stack is big enough for this whole block of data
        let data = &[rgb; 128 as usize];
        {
            let mut off = base + top_left.x as isize;
            let mut w = width;
            while w > 0 {
                unsafe {
                    copy_nonoverlapping(data.as_ptr(),
                        self.get_mut().offset(off + (width - w) as isize) as *mut Rgba,
                        w as usize);
                }
                w -= 128;
            }
        }

        for i in 1..height {
            let mut off = base + (i-1) as isize * self.width as isize + top_left.x as isize;
            unsafe {
                copy_nonoverlapping(
                    self.get_mut().offset(off) as *mut Rgba,
                    self.get_mut().offset(off + self.width as isize) as *mut Rgba,
                    width as usize);
            }
        }
    }

    pub fn draw_char(&mut self, p: Point, c: u8, rgb: Rgba, bg: Rgba) {
        let base = (p.y * self.width as i32 + p.x) as isize;

        let glyph = BUILTIN_FONT[c as usize - 1];
        for i in 0..16 {
            let off = base + (i * self.width) as isize;
            for j in 0..8 {
                unsafe {
                    *self.get_mut().offset(off+j as isize) = match glyph[i*8+j] {
                        b'*' => rgb,
                        _ => bg,
                    };
                }
            }
        }
    }

    pub fn draw_str(&mut self, p: Point, text: &[u8], rgb: Rgba, bg: Rgba) {
        let info = BUILTIN_FONTINFO; 
        let mut p1 = p;
        for &c in text {
            self.draw_char(p1, c, rgb, bg);
            p1.x += info.xadvance as i32;
            if p1.x as usize >= self.width {
                p1.x = 0;
                p1.y += info.yadvance as i32;
            }
        }
    }
}
