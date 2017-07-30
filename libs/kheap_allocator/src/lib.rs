#![feature(global_allocator)]
#![feature(alloc)]
#![feature(const_fn)]
#![feature(allocator_api)]
#![no_std]

extern crate alloc;
use alloc::heap::{Alloc, Layout, AllocErr};

extern crate linked_list_allocator;
use linked_list_allocator::Heap;

extern crate spin;
use spin::{Once, Mutex};


pub static KHEAP_ALLOCATOR: Mutex<Heap> = Mutex::new(Heap::empty());

static INIT: Once<()> = Once::new();

pub fn init(start: usize, size: usize) {
    INIT.call_once(|| {
        unsafe {
            KHEAP_ALLOCATOR.lock().init(start, size)
        }
    });
}

pub struct Allocator;

unsafe impl<'a> Alloc for &'a Allocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {
        KHEAP_ALLOCATOR.lock().allocate_first_fit(layout)
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        KHEAP_ALLOCATOR.lock().deallocate(ptr, layout)
    }
}

