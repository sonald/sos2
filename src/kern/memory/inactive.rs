use super::frame::{Frame, FrameAllocator};
use super::paging::*;
use super::PAGE_SIZE;
#[macro_use] use kern::console as con;
use con::LogLevel::*;

pub struct TinyAllocator {
    frames: [Option<Frame>; 3]
}

impl FrameAllocator for TinyAllocator {
    fn alloc_frame(&mut self) -> Option<Frame> {
        for p in self.frames.iter_mut() {
            if p.is_some() {
                return p.take()
            }
        }
        None
    }

    fn dealloc_frame(&mut self, frame: Frame) {
        for p in self.frames.iter_mut() {
            if p.is_none() {
                *p = Some(frame);
                break;
            }
        }
    }
}

impl TinyAllocator {
    pub fn new() -> TinyAllocator {
        let data = [
            super::frame::alloc_frame(),
            super::frame::alloc_frame(),
            super::frame::alloc_frame(),
        ];
        TinyAllocator {
            frames: data
        }
    }
}

pub struct TemporaryPage {
    page: Page,
    allocator:  TinyAllocator
}

impl TemporaryPage {
    pub fn new(page: Page) -> TemporaryPage {
        TemporaryPage {
            page: page,
            allocator: TinyAllocator::new()
        }
    }

    pub fn map(&mut self, frame: Frame, activePML4Table: &mut ActivePML4Table) -> VirtualAddress {
        assert!(activePML4Table.translate(self.page.start_address()).is_none(),
            "temporary page should not be mapped");
        activePML4Table.map_to(self.page, frame, WRITABLE|PRESENT);
        printk!(Debug, "TemporaryPage::map {:x} to {:x}\n\r", frame.start_address(), 
                self.page.start_address());
        self.page.start_address() as VirtualAddress
    }

    pub fn unmap(&mut self, activePML4Table: &mut ActivePML4Table) {
        printk!(Debug, "TemporaryPage::unmap\n\r");
        activePML4Table.unmap(self.page)
    }


    /// Maps the temporary page to the given page table frame in the active
    /// table. Returns a reference to the now mapped table.
    /// we cast it as PT level table because it can not call next_level_table*
    pub fn map_table_frame(&mut self, frame: Frame, activePML4Table: &mut ActivePML4Table)
        -> &mut Table<PT> {
        unsafe { &mut *(self.map(frame, activePML4Table) as *mut Table<PT>) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InactivePML4Table {
    pub pml4_frame: Frame,
}

impl InactivePML4Table {
    pub fn new(frame: Frame, activePML4Table:&mut ActivePML4Table, tempPage: &mut TemporaryPage) 
        -> InactivePML4Table {
        let mut inactive = InactivePML4Table { pml4_frame: frame };
        
        {
            let pml4 = tempPage.map_table_frame(frame, activePML4Table);
            pml4.zero();
            pml4[511].set(frame, WRITABLE|PRESENT);
        }

        tempPage.unmap(activePML4Table);
        
        inactive
    }
}
