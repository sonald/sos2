#![feature(const_fn)]
#![feature(global_allocator)]
#![feature(alloc)]
#![feature(allocator_api)]
#![no_std]

extern crate alloc;
use alloc::heap::{Alloc, Layout, AllocErr};

extern crate spin;
use spin::{Mutex, Once};
use core::sync::atomic::{AtomicBool, Ordering};
use core::ops::Range;
use core::ptr;

pub struct KHeapAllocator {
    pub current: usize,
}

fn align_up(start: usize, align: usize) -> usize {
    ((start + align - 1) / align) * align
}

/// this should be inited inside kernel
pub static HEAP_RANGE: Once<Range<usize>> = Once::new();

impl KHeapAllocator {
    /// [start, start+sz) virtual address space for allocator
    pub const fn new() -> KHeapAllocator {
        KHeapAllocator {
            current: 0,
        }
    }

    pub fn alloc(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        let range = HEAP_RANGE.try().expect("kheap is not initialized!");
        if self.current == 0 {
            self.current = range.start;
        }

        let start = align_up(self.current, align);
        let end = start.saturating_add(size);
        if end > range.end {
            return None;
        }

        self.current = end;
        Some(start as *mut u8)
    }

}

pub static KHEAP_ALLOCATOR: Mutex<KHeapAllocator> = Mutex::new(KHeapAllocator::new());


pub struct Allocator;

unsafe impl<'a> Alloc for &'a Allocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {
        KHEAP_ALLOCATOR.lock().alloc(layout.size(), layout.align()).ok_or(AllocErr::Exhausted {request: layout})
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
    }
}

