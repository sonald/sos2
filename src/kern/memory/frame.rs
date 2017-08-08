use core::ops::{Range, Add, AddAssign};
use core::iter::Iterator;
use multiboot2::*;

use super::PAGE_SIZE;
use super::KERNEL_MAPPING;
use spin::Mutex;
use super::frame_allocator::BuddyAllocator;

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
    used: usize
}

impl FrameAllocator for AreaFrameAllocator {
    fn alloc_frame(&mut self) -> Option<Frame> {
        use ::kern::console as con;
        use con::LogLevel::*;

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
                self.used += 1;
                if self.used % 1000 == 0 {
                    printk!(Debug, "frame usage: {}\n", self.used);
                }
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
            multiboot: mb,
            used: 0
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

/// two stage strategy. during initialization and before kernel heap is running,
/// use fast AreaFrameAllocator, and then reset it to BuddyAllocator
struct FrameAllocatorProxy<T: FrameAllocator, U: FrameAllocator> {
    initial: bool,
    allocator: T,
    alternative: Option<U>,

}

static FRAME_ALLOCATOR: Mutex<Option<FrameAllocatorProxy<AreaFrameAllocator, BuddyAllocator>>> = Mutex::new(None);

impl<T: FrameAllocator, U: FrameAllocator> FrameAllocatorProxy<T, U> {
    pub fn new(allocator: T) -> FrameAllocatorProxy<T, U> {
        let alternative = None;
        let initial = true;
        FrameAllocatorProxy { initial, allocator, alternative }
    }
}

impl<T: FrameAllocator, U: FrameAllocator> FrameAllocatorProxy<T, U> {
    fn alloc_frame(&mut self) -> Option<Frame> {
        match self.initial {
            true => self.allocator.alloc_frame(),
            _ => self.alternative.as_mut().unwrap().alloc_frame()
        }
    }

    fn dealloc_frame(&mut self, frame: Frame) {
        match self.initial {
            true => self.allocator.dealloc_frame(frame),
            _ => self.alternative.as_mut().unwrap().dealloc_frame(frame)
        }
    }
}

pub fn alloc_frame() -> Option<Frame> {
    if let Some(ref mut proxy) = *FRAME_ALLOCATOR.lock() {
        proxy.alloc_frame()
    } else {
        panic!("FRAME_ALLOCATOR is not initialized\n");
    }

}

pub fn dealloc_frame(frame: Frame) {
    if let Some(ref mut proxy) = *FRAME_ALLOCATOR.lock() {
        proxy.dealloc_frame(frame)
    } else {
        panic!("FRAME_ALLOCATOR is not initialized\n");
    }
}

pub fn upgrade_allocator(mbinfo: &'static BootInformation) {
    use ::kern::console as con;
    use con::LogLevel::*;
    use ::core::cmp::max;

    if let Some(ref mut proxy) = *FRAME_ALLOCATOR.lock() {
        //FIXME: exclude heap area
        let area = {
            let area = {
                let mmap = mbinfo.memory_map_tag().expect("memory map is unavailable");
                let max = mmap.memory_areas().max_by_key(|a| a.base_addr).unwrap();
                (max.base_addr as usize, (max.base_addr + max.length) as usize)
            };
            let current = proxy.allocator.next_free_frame.start_address();
            let v = [
                current,
                area.0,
                proxy.allocator.kernel.end.start_address(),
                proxy.allocator.multiboot.end.start_address(),
            ];
            (*v.into_iter().max().unwrap(), area.1)
        };
        printk!(Info, "upgrade allocator for [{:#x}, {:#x})\n", area.0, area.1);

        proxy.initial = false;
        proxy.alternative = Some(BuddyAllocator::new(area.0, area.1 - area.0));
    } else {
        panic!("FRAME_ALLOCATOR is not initialized\n");
    }
}

pub fn init(mbinfo: &'static BootInformation) {
    use ::kern::console as con;
    use con::LogLevel::*;

    let kernel_base = KERNEL_MAPPING.KernelMap.start;
    let mmap = mbinfo.memory_map_tag().expect("memory map is unavailable");
    {
        let start = mmap.memory_areas().map(|a| a.base_addr).min().unwrap();
        let end = mmap.memory_areas().map(|a| a.base_addr + a.length).max().unwrap();
        printk!(Info, "mmap start: {:#x}, end: {:#x}\n\r", start ,end);
    }


    let kr = {
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

        Range {
            start: Frame::from_paddress(kernel_start),
            end: Frame::from_paddress(kernel_end - 1) + 1,
        }
    };
    let mr = {
        use ::core::cmp::{min, max};

        let mods_start = mbinfo.module_tags().map(|a| a.start_address()).min().unwrap() as usize;
        let mods_end = mbinfo.module_tags().map(|a| a.end_address()).max().unwrap() as usize;

        let (mb_start, mb_end) = (
            min(mods_start, mbinfo.start_address() - kernel_base),
            max(mods_end, mbinfo.end_address() - kernel_base)
            );
        printk!(Info, "mboot2(include modules) start: {:#x}, end: {:#x}\n\r", mb_start, mb_end);

        Range {
            start: Frame::from_paddress(mb_start),
            end: Frame::from_paddress(mb_end - 1) + 1,
        }
    };
    
    //FIXME: exclude region used by kernel heap
    let afa = AreaFrameAllocator::new(mmap.memory_areas(), kr, mr);
    let mut guard = FRAME_ALLOCATOR.lock();
    *guard = Some(FrameAllocatorProxy::new(afa));
}

