use core::ops::{Range, Add, AddAssign};
use core::iter::Iterator;
use multiboot2::*;

use super::PAGE_SIZE;

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


