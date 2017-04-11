#![feature(const_fn)]
#![feature(allocator)]

#![allocator]
#![no_std]

extern crate spin;
use spin::Mutex;

pub struct KHeapAllocator {
    heap_start: usize,
    heap_size: usize,
    current: usize
}

fn align_up(start: usize, align: usize) -> usize {
    ((start + align - 1) / align) * align
}

impl KHeapAllocator {
    /// [start, start+sz) virtual address space for allocator
    pub const fn new(start: usize, sz: usize) -> KHeapAllocator {
        KHeapAllocator {
            heap_start: start,
            heap_size: sz,
            current: start
        }
    }

    pub fn alloc(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        let start = align_up(self.current, align);
        self.current = start + size;
        if self.current - self.heap_start >= self.heap_size {
            return None;
        }

        Some(start as *mut u8)
    }

}

///NOTE: start address should be adjusted according to kernel size and mbinfo 
pub const START_ADDRESS: usize = 0x200_000;
pub const ALLOC_SIZE: usize = 0x1000 * 64;
static KHEAP_ALLOCATOR: Mutex<KHeapAllocator> = Mutex::new(KHeapAllocator::new(START_ADDRESS, ALLOC_SIZE));

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
        use core::ptr::copy_nonoverlapping;
        use core::cmp;
        copy_nonoverlapping(ptr, new_ptr, cmp::min(size, _old_size));
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
