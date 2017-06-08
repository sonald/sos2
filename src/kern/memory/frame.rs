use core::ops::{Range, Add, AddAssign};
use core::iter::Iterator;
use multiboot2::*;

use super::PAGE_SIZE;
use super::KERNEL_MAPPING;
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame {
    pub number: usize,
}

impl Add<usize> for Frame {
    type Output = Frame;
    fn add(self, rhs: usize) -> Frame {
        Frame {number: self.number + rhs}
    }
}

impl AddAssign<isize> for Frame {
    fn add_assign(&mut self, mut inc: isize) {
        if inc.is_negative() {
            assert!(self.number + inc.abs() as usize > 0, "frame number should not below zero");
            self.number -= inc.abs() as usize;
        } else {
            self.number += inc as usize;
        }
    }
}

pub struct FrameRange {
    pub start: Frame,
    pub end: Frame // exclusive
}

impl Iterator for FrameRange {
    type Item = Frame;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start < self.end {
            let f = self.start;
            self.start += 1;
            Some(f)
        } else {
            None
        }
    }
}

impl FrameRange {
    pub fn new(start: usize, end: usize) -> FrameRange {
        FrameRange {
            start: Frame::from_paddress(start),
            end: Frame::from_paddress(end-1) + 1
        }
    }
}

impl Frame {
    pub const fn from_paddress(physical: usize) -> Frame {
        Frame {number: physical / PAGE_SIZE }
    }

    pub const fn start_address(&self) -> usize {
        self.number * PAGE_SIZE
    }
}

pub trait FrameAllocator {
    fn alloc_frame(&mut self) -> Option<Frame>;
    fn dealloc_frame(&mut self, frame: Frame);
}

/// early stage fast frame allocator, dealloc_frame is not implemented,
/// since there is no need to free. after paging system being setuped, 
/// a new frame allocator needed.
#[derive(Debug)]
pub struct AreaFrameAllocator {
    next_free_frame: Frame,
    current_area: Option<Range<Frame>>,
    areas: MemoryAreaIter,
    kernel: Range<Frame>,
    multiboot: Range<Frame>,
}

impl FrameAllocator for AreaFrameAllocator {
    fn alloc_frame(&mut self) -> Option<Frame> {
        let frame = self.next_free_frame;

        if self.current_area.is_some() {
            let current_end = self.current_area.as_ref().unwrap().end;
            if frame >= current_end {
                self.next_area();
            } else if self.kernel.contains(frame) {
                self.next_free_frame = self.kernel.end;
            } else if self.multiboot.contains(self.next_free_frame) {
                self.next_free_frame = self.multiboot.end;
            } else {
                self.next_free_frame += 1;
                return Some(frame);
            }

            self.alloc_frame()

        } else {
            None
        }
    }

    fn dealloc_frame(&mut self, frame: Frame) {
        unimplemented!()
    }
}

impl AreaFrameAllocator {
    pub fn new(areas: MemoryAreaIter, kernel: Range<Frame>, mb: Range<Frame>) -> AreaFrameAllocator {
        let mut afa = AreaFrameAllocator {
            next_free_frame: Frame::from_paddress(0),
            current_area: None,
            areas: areas,
            kernel: kernel,
            multiboot: mb
        };

        afa.next_area();

        afa
    }

    //NOTE: I assume areas are already sorted by base addr
    pub fn next_area(&mut self) {
        if let Some(area) = self.areas.next() {
            self.current_area = Some(Range {
                start: Frame::from_paddress(area.base_addr as usize),
                end: Frame::from_paddress((area.base_addr + area.length - 1) as usize) + 1,
            });
            self.next_free_frame = self.current_area.as_ref().unwrap().start;
        } else {
            self.current_area = None;
        }
    }
}


struct FrameAllocatorProxy<T> {
    allocator: T
}

static FRAME_ALLOCATOR: Mutex<Option<FrameAllocatorProxy<AreaFrameAllocator>>> = Mutex::new(None);

impl<T: FrameAllocator> FrameAllocatorProxy<T> {
    pub fn new(t: T) -> FrameAllocatorProxy<T> {
        FrameAllocatorProxy {
            allocator: t
        }
    }
}

impl<T: FrameAllocator> FrameAllocator for FrameAllocatorProxy<T> {
    fn alloc_frame(&mut self) -> Option<Frame> {
        self.allocator.alloc_frame()
    }

    fn dealloc_frame(&mut self, frame: Frame) {
        self.allocator.dealloc_frame(frame)
    }
}

pub fn alloc_frame() -> Option<Frame> {
    if let Some(ref mut allocator) = *FRAME_ALLOCATOR.lock() {
        allocator.alloc_frame()
    } else {
        panic!("FRAME_ALLOCATOR is not initialized\n");
    }

}

pub fn dealloc_frame(frame: Frame) {
    if let Some(ref mut allocator) = *FRAME_ALLOCATOR.lock() {
        allocator.dealloc_frame(frame)
    } else {
        panic!("FRAME_ALLOCATOR is not initialized\n");
    }
}

pub fn init(mbinfo: &'static BootInformation) {
    use ::kern::console as con;
    use con::LogLevel::*;

    let kernel_base = KERNEL_MAPPING.KernelMap.start;
    let mmap = mbinfo.memory_map_tag().expect("memory map is unavailable");
    let start = mmap.memory_areas().map(|a| a.base_addr).min().unwrap();
    let end = mmap.memory_areas().map(|a| a.base_addr + a.length).max().unwrap();
    printk!(Info, "mmap start: {:#x}, end: {:#x}\n\r", start ,end);

    let elf = mbinfo.elf_sections_tag().expect("elf sections is unavailable");
    let mut kernel_start = elf.sections().filter(|a| a.is_allocated()).map(|a| a.addr).min().unwrap() as usize;
    let mut kernel_end = elf.sections().filter(|a| a.is_allocated()).map(|a| a.addr + a.size).max().unwrap() as usize;

    if kernel_start > kernel_base {
        kernel_start -= kernel_base;
    }
    if kernel_end > kernel_base {
        kernel_end -= kernel_base;
    }
    printk!(Info, "kernel start: {:#x}, end: {:#x}\n\r", kernel_start, kernel_end);

    let (mb_start, mb_end) = (mbinfo.start_address() - kernel_base,
    mbinfo.end_address() - kernel_base);
    printk!(Info, "mboot2 start: {:#x}, end: {:#x}\n\r", mb_start, mb_end);

    let kr = Range {
        start: Frame::from_paddress(kernel_start),
        end: Frame::from_paddress(kernel_end - 1) + 1,
    };
    let mr = Range {
        start: Frame::from_paddress(mb_start),
        end: Frame::from_paddress(mb_end - 1) + 1,
    };
    let afa = AreaFrameAllocator::new(mmap.memory_areas(), kr, mr);
    let mut guard = FRAME_ALLOCATOR.lock();
    *guard = Some(FrameAllocatorProxy::new(afa));
}

