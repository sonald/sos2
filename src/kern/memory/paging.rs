use super::{Frame, PAGE_SIZE, FrameAllocator};
#[macro_use] use kern::console as con;
use con::LogLevel::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    number: usize
}

impl Page {
    pub const fn from_vaddress(vaddr: usize) -> Page {
        Page {number: vaddr / PAGE_SIZE }
    }

    pub const fn start_address(&self) -> usize {
        self.number * PAGE_SIZE
    }
}


bitflags! {
    // for PML4, PDPT, PDT, PT entries,
    pub flags EntryFlags: usize {
        const PRESENT =         1 << 0,
        const WRITABLE =        1 << 1,
        const USER =            1 << 2,
        const WRITE_THROUGH =   1 << 3,
        const DISABLE_CACHE =   1 << 4,
        const ACCESSED =        1 << 5,
        const DIRTY =           1 << 6,
        const HUGE_PAGE =       1 << 7, // 2M in PDE, 1G in PDPE
        const GLOBAL =          1 << 8,
        const NO_EXECUTE =      1 << 63,

        /// self defined
        const SWAPPED_OUT =     1 << 9,
    }
}

const AddressBitsMask: usize = 0x000fffff_fffff000;
const EntryCount: usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress = usize;

pub trait VirtualAddressSpace {
    fn validate(self);
    fn offset(self) -> usize;
    fn pml4t_index(self) -> usize;
    fn pdpt_index(self) -> usize;
    fn pdt_index(self) -> usize;
    fn pt_index(self) -> usize;
}

impl VirtualAddressSpace for VirtualAddress {
    fn validate(self) {
        let v = self as usize;
        assert!(v < 0x08000_00000000 || v > 0xffff8000_00000000,
                "invalid virtual address: 0x{:x}", v);
    }

    fn offset(self) -> usize {
        self as usize & 0xfff
    }

    fn pml4t_index(self) -> usize {
        (self as usize >> 39) & 0o777
    }

    fn pdpt_index(self) -> usize {
        (self as usize >> 30) & 0o777
    }

    fn pdt_index(self) -> usize {
        (self as usize >> 21) & 0o777
    }

    fn pt_index(self) -> usize {
        (self as usize >> 12) & 0o777
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PageEntry(usize);

impl PageEntry {
    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }

    pub fn set_unused(&mut self) {
        self.0 = 0;
    }

    pub fn flags(&self) -> EntryFlags {
        EntryFlags::from_bits_truncate(self.0)
    }

    pub fn pointed_frame(&self) -> Option<Frame> {
        if self.flags().contains(PRESENT) {
            Some (Frame::from_paddress(self.0 & AddressBitsMask))
        } else {
            None
        }
    }
    
    pub fn set(&mut self, frame: Frame, flags: EntryFlags) {
        assert!(frame.start_address() & !AddressBitsMask == 0);
        self.0 = frame.start_address() | flags.bits();
    }
}

/// table hierachies
/// 
pub trait TableLevel {}
pub trait HierarchyTableLevel: TableLevel {
    type NextLevel: TableLevel;
}

pub enum PML4T {}
pub enum PDPT {}
pub enum PDT {}
pub enum PT {}

impl TableLevel for PML4T {}
impl TableLevel for PDPT {}
impl TableLevel for PDT {}
impl TableLevel for PT {}

impl HierarchyTableLevel for PML4T {
    type NextLevel = PDPT;
}
impl HierarchyTableLevel for PDPT {
    type NextLevel = PDT;
}
impl HierarchyTableLevel for PDT {
    type NextLevel = PT;
}

use core::marker::PhantomData;
pub struct Table<L: TableLevel> {
    entries: [PageEntry; EntryCount],
    phantom: PhantomData<L>
    
}


use core::ops::{Index, IndexMut};

impl<L> Index<usize> for Table<L> where L: TableLevel {
    type Output = PageEntry;
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl<L> IndexMut<usize> for Table<L> where L: TableLevel {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

impl<L> Table<L> where L: TableLevel {
    pub fn zero(&mut self) -> &mut Self {
        for p in self.entries.iter_mut() {
            p.set_unused();
        }

        self
    }

}

impl<L> Table<L> where L: HierarchyTableLevel {
    /// get next level table's virtual address, only if table is recursive-mapped.
    fn next_level_table_address(&self, index: usize) -> Option<usize> {
        let flags = self.entries[index].flags();
        if flags.contains(PRESENT) && !flags.contains(HUGE_PAGE) {
            let table_address = self as *const _ as usize;
            Some((table_address << 9) | (index << 12))
        } else {
            None
        }
    }

    pub fn next_level_table(&self, index: usize) -> Option<&Table<L::NextLevel>> {
        self.next_level_table_address(index)
            .map(|address| unsafe {&*(address as *const _)})
    }

    pub fn next_level_table_mut(&mut self, index: usize) -> Option<&mut Table<L::NextLevel>> {
        self.next_level_table_address(index)
            .map(|address| unsafe {&mut *(address as *mut _)})
    }

    pub fn next_level_table_or_create<A>(&mut self, index: usize, allocator: &mut A) 
        -> &mut Table<L::NextLevel> where A: FrameAllocator {
        if self.next_level_table_mut(index).is_none() {
            let frame = allocator.alloc_frame().expect("no more free frame available");
            self.entries[index].set(frame, WRITABLE | PRESENT);
            self.next_level_table_mut(index).unwrap().zero()
        } else {
            self.next_level_table_mut(index).unwrap()
        }
    }
}

use core::ptr::Unique;
pub struct ActivePML4Table {
    top: Unique<Table<PML4T>>
}

impl ActivePML4Table {
    /// pointer to top level table virtual address, only if table is recursive-mapped.
    pub fn new() -> ActivePML4Table {
        ActivePML4Table {
            top: unsafe { Unique::new(0xffffffff_fffff000 as *mut _) }
        }
    } 

    pub fn get(&self) -> &Table<PML4T> {
        unsafe { self.top.get() }
    }

    pub fn get_mut(&mut self) -> &mut Table<PML4T> {
        unsafe { self.top.get_mut() }
    }

    pub fn translate(&self, vaddr: VirtualAddress) -> Option<PhysicalAddress> {
        vaddr.validate();

        let p3 = self.get().next_level_table(vaddr.pml4t_index());
        let offset = vaddr.offset();

        /// return frame for huge page
        let huge_page = || -> Option<Frame> {
            p3.and_then(|p3| {
                let entry = &p3[vaddr.pdpt_index()];
                if let Some(frame) = entry.pointed_frame() {
                    //1G-page 
                    if entry.flags().contains(HUGE_PAGE) {
                        assert!(frame.number % (EntryCount * EntryCount) == 0);
                        return Some(Frame {
                            number: frame.number + vaddr.pdt_index() * EntryCount + vaddr.pt_index() 
                        });
                    }
                }

                if let Some(p2) = p3.next_level_table(vaddr.pdpt_index()) {
                    let entry = &p2[vaddr.pdt_index()];
                    if let Some(frame) = entry.pointed_frame() {
                        //2M-page
                        if entry.flags().contains(HUGE_PAGE) {
                            assert!(frame.number % EntryCount == 0);
                            return Some(Frame {
                                number: frame.number + vaddr.pt_index()
                            });
                        }
                    }
                }
                None
            })
        };

        p3.and_then(|p3| p3.next_level_table(vaddr.pdpt_index()))
            .and_then(|p2| p2.next_level_table(vaddr.pdt_index()))
            .and_then(|p1| p1[vaddr.pt_index()].pointed_frame())
            .or_else(huge_page)
            .map(|frame| frame.start_address() + offset)
    }

    //FIXME: need to check if frame has been used
    pub fn map_to<A>(&mut self, page: Page, frame: Frame, flags: EntryFlags, allocator: &mut A) 
        where A: FrameAllocator {
        let pml4 = self.get_mut();
        let vaddr = page.start_address() as VirtualAddress;

        let pdpt = pml4.next_level_table_or_create(vaddr.pml4t_index(), allocator);
        let pdt = pdpt.next_level_table_or_create(vaddr.pdpt_index(), allocator);
        let pt = pdt.next_level_table_or_create(vaddr.pdt_index(), allocator);

        assert!(pt[vaddr.pt_index()].is_unused());
        pt[vaddr.pt_index()].set(frame, flags | PRESENT);
    }


    pub fn map<A>(&mut self, page: Page, flags: EntryFlags, allocator: &mut A) 
        where A: FrameAllocator {
        let frame = allocator.alloc_frame().expect("not more free frame available");
        self.map_to(page, frame, flags, allocator)
    }

    pub fn identity_map<A>(&mut self, frame: Frame, flags: EntryFlags, allocator: &mut A)
        where A: FrameAllocator {
        let page = Page::from_vaddress(frame.start_address());
        self.map_to(page, frame, flags, allocator)
    }

    //TODO: support huge page
    pub fn unmap<A>(&mut self, page: Page, allocator: &mut A) where A: FrameAllocator {
        let vaddr = page.start_address() as VirtualAddress;
        assert!(self.translate(vaddr).is_some());

        let p3 = self.get_mut().next_level_table_mut(vaddr.pml4t_index());
        let offset = vaddr.offset();

        let huge_page = || {None};

        p3.and_then(|p3| p3.next_level_table_mut(vaddr.pdpt_index()))
            .and_then(|p2| p2.next_level_table_mut(vaddr.pdt_index()))
            .and_then(|p1| {
                assert!(!p1[vaddr.pt_index()].is_unused());
                let frame = p1[vaddr.pt_index()].pointed_frame().unwrap();
                p1[vaddr.pt_index()].set_unused();

                #[cfg(target_arch="x86_64")]
                ::kern::arch::tlb_flush(vaddr);
                //TODO: free pdpt, pdt, pt tables when empty
                //allocator.dealloc_frame(frame);
                Some({})
            })
            .or_else(huge_page);
    }
}

use core::ops::Deref;
impl Deref for ActivePML4Table {
    type Target = Table<PML4T>;
    fn deref(&self) -> &Self::Target {
        unsafe { self.get() }
    }
}

pub fn test_paging<A>(allocator: &mut A) where A: FrameAllocator {
    let mut pml4 = ActivePML4Table::new();

    {
        // first 1G is contains huge pages
        assert!(pml4.next_level_table(0)
                .and_then(|p3| p3.next_level_table(0))
                .and_then(|p2| p2.next_level_table(2))
                .is_none());

        // 511-th is recursive mapped
        let p = pml4.next_level_table(511).unwrap();
        assert!(unsafe {p as *const _ as usize} == 0xffffffff_fffff000);
        let p2 = p.next_level_table(0).unwrap();
        assert!(unsafe {p2 as *const _ as usize} == 0o177_777_777_777_777_000_0000);
        assert!(p.next_level_table(1).is_none());
    }

    {

        let fb: VirtualAddress = 0xfd00_0000;
        printk!(Debug, "0x{:x} 0x{:x} 0x{:x} 0x{:x} 0x{:x}\n\r", 
                fb.pml4t_index(), fb.pdpt_index(), fb.pdt_index(), fb.pt_index(), fb.offset());
        assert!(pml4.translate(fb).unwrap() == fb);
    }

    {
        let vs = [0x30000000, 0x20000030, 0x10025030, 0x07025030];
        for &v in &vs {
            printk!(Debug, "translate({:x}) = {:x}\n\r", v, pml4.translate(v).unwrap_or(0));
            assert!(pml4.translate(v).unwrap() == v);
        }

        let last = EntryCount * EntryCount * PAGE_SIZE - 1;
        assert!(pml4.translate(last).unwrap() == last);

        let unmapped = EntryCount * EntryCount * PAGE_SIZE;
        assert!(pml4.translate(unmapped).is_none());
    }

    {
        use core::slice::from_raw_parts_mut;

        let frame = allocator.alloc_frame().expect("no more mem");
        let page = Page::from_vaddress(0x6218_2035_3201);
        assert!(pml4.translate(page.start_address()).is_none());
        pml4.map_to(page, frame, EntryFlags::empty(), allocator);
        printk!(Debug, "map {:x} -> {:x}\n\r", page.start_address(), frame.start_address());

        let mut v = unsafe { from_raw_parts_mut(page.start_address() as *mut u8, 4096) };
        for p in v.iter_mut() {
            *p = 2;
        }

        let page2 = Page { number: page.number + 100 };
        pml4.map(page2, USER, allocator);
        let mut v2 = unsafe { from_raw_parts_mut(page2.start_address() as *mut u8, 4096) };
        for p in v2.iter_mut() {
            *p = 3;
        }

        pml4.unmap(page, allocator);
        for p in v2.iter_mut() {
            *p = 3;
        }
    }
}
