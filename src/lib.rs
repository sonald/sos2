#![feature(lang_items)]
#![feature(global_allocator)]
#![feature(allocator_api)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(asm)]
#![feature(range_contains)]
#![feature(alloc, collections)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]
#![feature(core_slice_ext)]
// stabled since 1.17
#![feature(field_init_shorthand)]
#![feature(drop_types_in_const)]
#![no_std]

extern crate rlibc;
extern crate multiboot2;
extern crate spin;
extern crate x86_64;

extern crate kheap_allocator;
extern crate alloc;
#[macro_use] extern crate collections;

#[macro_use] extern crate bitflags;
extern crate bit_field;
#[macro_use] extern crate lazy_static;

#[macro_use] mod kern;
// make syscall_dispatch exported
pub use kern::syscall_dispatch;

use kern::console as con;
use con::Console;
use con::LogLevel::*;
use kern::driver::serial;
use kern::memory;
use kern::interrupts;
use kheap_allocator as kheap;
use kern::driver::video::{Framebuffer, Point, Rgba};
use kern::task;
use kern::syscall;

#[global_allocator]
static GLOBAL_ALLOCATOR: kheap::Allocator = kheap::Allocator;

#[allow(dead_code)]
fn busy_wait () {
    for _ in 1..500000 {
        kern::util::cpu_relax();
    }
}

/// test rgb framebuffer drawing
fn display(fb: &mut Framebuffer) {
    let w = fb.width as i32;
    let h = fb.height as i32;
    for g in 0..1 {
        fb.fill_rect_grad(Point{x:0, y: 0}, w, h, Rgba(0x0000ff00), Rgba(255<<16));

        fb.draw_line(Point{x: 530, y: 120}, Point{x: 330, y: 10}, Rgba(0xeeeeeeee));
        fb.draw_line(Point{x: 330, y: 120}, Point{x: 530, y: 10}, Rgba(0xeeeeeeee));

        fb.draw_line(Point{x: 300, y: 10}, Point{x: 500, y: 100}, Rgba(0xeeeeeeee));
        fb.draw_line(Point{x: 300, y: 10}, Point{x: 400, y: 220}, Rgba(0xeeeeeeee));

        fb.draw_line(Point{x: 100, y: 220}, Point{x: 300, y: 100}, Rgba(0xeeeeeeee));
        fb.draw_line(Point{x: 100, y: 220}, Point{x: 300, y: 10}, Rgba(0xeeeeeeee));

        for r in (100..150).filter(|x| x % 5 == 0) {
            fb.draw_circle(Point{x: 200, y: 200}, r, Rgba::from(0, g as u8, 0xff));
        }

        fb.spread_circle(Point{x: 400, y: 100}, 90, Rgba::from(0, g as u8, 0xee));

        fb.draw_rect(Point{x:199, y: 199}, 202, 102, Rgba::from(0x00, g as u8, 0xff));
        fb.fill_rect(Point{x:200, y: 200}, 200, 100, Rgba::from(0x80, g as u8, 0x80));

        fb.draw_rect(Point{x:199, y: 309}, 302, 102, Rgba::from(0x00, g as u8, 0xff));
        fb.fill_rect(Point{x:200, y: 310}, 300, 100, Rgba::from(0xa0, g as u8, 0x80));

        fb.draw_rect(Point{x:199, y: 419}, 392, 102, Rgba::from(0x00, g as u8, 0xff));
        fb.fill_rect(Point{x:200, y: 420}, 390, 100, Rgba::from(0xe0, g as u8, 0x80));

        fb.draw_char(Point{x:300, y: 550}, b'A', Rgba(0x000000ff), Rgba(0x00ff0000));
        fb.draw_str(Point{x:40, y: 550}, b"Loading SOS...", Rgba(0x000000ff), Rgba(0x00ff0000));
        fb.blit_copy(Point{x: 200, y: 100}, Point{x: 40, y: 550},  200, 20);
        fb.blit_copy(Point{x: 150, y: 150}, Point{x: 50, y: 50}, 350, 350);

        printk!(Debug, "loop {}\n\r", g);
    }
}

fn test_kheap_allocator() {
    let mut v = vec![1,2,3,4];
    let b = alloc::boxed::Box::new(0xcafe);
    printk!(Debug, "v = {:?}, b = {:?}\n\r", v, b);
    let vs = vec!["Loading", "SOS2", "\n\r"];
    for s in vs {
        printk!(Debug, "{} ", s);
    }

    for i in 1..0x1000 * 40 {
        v.push(i);
    }

    let range = kheap::HEAP_RANGE.try().unwrap();
    printk!(Critical, "Heap usage: {:#x}\n\r", kheap::KHEAP_ALLOCATOR.lock().current - range.start);
}

extern {
    static _start: u64;
    static _end: u64;
    static kern_stack_top: u64;
}

#[no_mangle]
pub extern fn kernel_main(mb2_header: usize) {
    unsafe { 
        let mut com1 = serial::COM1.lock();
        com1.init();
    }

    con::clear();
    printk!(Info, "Loading SOS2....\n\r");

    let mbinfo = unsafe { multiboot2::load(mb2_header) };
    printk!(Info, "{:#?}\n\r", mbinfo);

    let (pa, pe, sp_top) = unsafe {
        (&_start as *const _ as u64, &_end as *const _ as u64, &kern_stack_top as *const _ as u64)
    };
    printk!(Debug, "_start {:#X}, _end {:#X}, sp top: {:#X}\n\r", pa, pe, sp_top);

    let fb = mbinfo.framebuffer_tag().expect("framebuffer tag is unavailale");
    let mm = memory::init(mbinfo);

    //if cfg!(feature = "test") { test_kheap_allocator(); }

    {
        let mut mm = mm.lock();
        interrupts::init(&mut mm);
        if cfg!(feature = "test") { interrupts::test_idt(); }
    }

    if fb.frame_type == multiboot2::FramebufferType::Rgb {
        use kern::arch::cpu;
        //NOTE: if I dont use console in timer, then there is no reason to disable IF here.
        let oflags = unsafe { cpu::push_flags() };
        let mut fb = Framebuffer::new(&fb);
        //if cfg!(feature = "test") { display(&mut fb); }

        {
            let mut term = con::tty1.lock();
            *term = Console::new_with_fb(fb);
        }

        con::clear();
        println!("framebuffer console init.\n\r");
        //if cfg!(feature = "test") { for b in 1..127u8 { print!("{}", b as char); } }
        unsafe { cpu::pop_flags(oflags); }
    }

    task::init();

    loop {
        kern::util::cpu_relax();
    }
}

/// Get a stack trace
/// TODO: extract symbol names from kernel elf
unsafe fn stack_trace() {
    use core::mem;
    let mut rbp: usize;
    asm!("" : "={rbp}"(rbp) : : : "intel", "volatile");

    println!("backtrace: {:>016x}", rbp);
    //Maximum 64 frames
    let active_table = memory::paging::ActivePML4Table::new();
    for _frame in 0..64 {
        if let Some(rip_rbp) = rbp.checked_add(mem::size_of::<usize>()) {
            if active_table.translate(rbp).is_some() && 
                active_table.translate(rip_rbp).is_some() {
                let rip = *(rip_rbp as *const usize);
                if rip == 0 {
                    println!(" {:>016x}: EMPTY RETURN", rbp);
                    break;
                }
                println!("  {:>016x}: ret rip {:>016x}", rbp, rip);
                rbp = *(rbp as *const usize);
            } else {
                println!("  {:>016x}: Invalid", rbp);
                break;
            }
        } else {
            println!("  {:>016x}: RBP OVERFLOW", rbp);
        }
    }
}

#[lang = "eh_personality"]
extern fn eh_personality() {}

#[lang = "panic_fmt"] 
#[no_mangle] pub extern fn panic_fmt(fmt: core::fmt::Arguments, file: &'static str, line: u32) -> ! {
	printk!(Critical, "\n\rPanic at {}:{}\n\r", file, line);
    printk!(Critical, "    {}\n\r", fmt);

    unsafe { stack_trace(); }

    loop {
        unsafe { asm!("hlt":::: "volatile"); }
    }
}

#[lang = "eh_unwind_resume"]
#[no_mangle]
pub extern fn rust_eh_unwind_resume() {
}

/// dummy, this should never gets called
#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}
