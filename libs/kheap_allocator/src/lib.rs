#![feature(const_fn)]
#![feature(allocator)]

#![allocator]
#![no_std]

extern crate spin;
use spin::{Mutex, Once};
use core::sync::atomic::{AtomicBool, Ordering};
use core::ops::Range;

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

#[no_mangle]
pub extern fn __rust_allocate(size: usize, _align: usize) -> *mut u8 {
    KHEAP_ALLOCATOR.lock().alloc(size, _align).expect("oom")
}

#[no_mangle]
pub extern fn __rust_deallocate(ptr: *mut u8, _old_size: usize, _align: usize) {
}

#[no_mangle]
pub extern fn __rust_reallocate(ptr: *mut u8, _old_size: usize, size: usize,
                                _align: usize) -> *mut u8 {
    let new_ptr = KHEAP_ALLOCATOR.lock().alloc(size, _align).expect("oom");
    unsafe {
        use core::ptr::copy;
        use core::cmp;
        copy(ptr, new_ptr, cmp::min(size, _old_size));
    }
    __rust_deallocate(ptr, _old_size, _align);

    new_ptr
}

#[no_mangle]
pub extern fn __rust_reallocate_inplace(_ptr: *mut u8, old_size: usize,
                                        _size: usize, _align: usize) -> usize {
    old_size
}

#[no_mangle]
pub extern fn __rust_usable_size(size: usize, _align: usize) -> usize {
    size
}
