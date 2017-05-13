use super::paging::*;
use super::frame::*;
use super::{MemoryManager, PAGE_SIZE};

#[macro_use] use kern::console as con;
use con::LogLevel::*;

#[derive(Debug)]
pub struct Stack {
    top: usize,
    bottom: usize,
}

impl Stack {
    fn new(top: usize, bottom: usize) -> Stack {
        assert!(top > bottom);
        Stack {
            top: top,
            bottom: bottom,
        }
    }

    pub fn top(&self) -> usize {
        self.top
    }

    pub fn bottom(&self) -> usize {
        self.bottom
    }
}

pub struct StackAllocator {
    pages: PageRange
}

impl StackAllocator {
    pub const fn new(start: Page, end: Page) -> StackAllocator {
        StackAllocator {
            pages: PageRange {
                start: start,
                end: end
            }
        }
    }

    pub fn alloc_stack<A>(&mut self, active: &mut ActivePML4Table, allocator: &mut A,
                       size_in_pages: usize) -> Option<Stack> where A: FrameAllocator {
        assert!(size_in_pages > 0);

        let mut range = self.pages.clone();
        let guard = range.next();
        let start = range.next();
        let end = if size_in_pages == 1 {
            start
        } else {
            range.nth(size_in_pages - 2)
        };

        match (guard, start, end) {
            (Some(_), Some(start), Some(end)) => {
                self.pages = range;

                let r = PageRange {start: start, end: end+1};
                for page in r {
                    active.map(page, WRITABLE, allocator);
                }

                let (top, bottom) = (end.start_address() + PAGE_SIZE, start.start_address());
                printk!(Info, "allocate and map stack [{:#x}, {:#x})\n\r", bottom, top);
                Some(Stack::new(top, bottom))
            },
            _ => None
        }
    }
}
