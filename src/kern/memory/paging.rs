use core::ops::{Range, Add, AddAssign};
use super::frame::{Frame, FrameRange, alloc_frame};
use super::mapper::Mapper;
use super::{PAGE_SIZE, KERNEL_MAPPING};
use super::inactive::{InactivePML4Table, TemporaryPage};
#[macro_use] use kern::console as con;
use con::LogLevel::*;
use multiboot2::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page {
    number: usize
}

///TODO: use KERNEL_MAPPING to validate page number 
impl Add<usize> for Page {
    type Output = Page;
    fn add(self, rhs: usize) -> Page {
        Page {number: self.number + rhs}
    }
}

impl AddAssign<isize> for Page {
    fn add_assign(&mut self, mut inc: isize) {
        if inc.is_negative() {
            assert!(self.number + inc.abs() as usize > 0, "page number should not below zero");
            self.number -= inc.abs() as usize;
        } else {
            self.number += inc as usize;
        }
    }
}

impl Page {
    pub const fn from_vaddress(vaddr: usize) -> Page {
        Page {number: vaddr / PAGE_SIZE }
    }

    pub const fn start_address(&self) -> usize {
        self.number * PAGE_SIZE
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PageRange {
    pub start: Page,
    pub end: Page // exclusive
}

impl Iterator for PageRange {
    type Item = Page;

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

impl PageRange {
    pub fn new(start: VirtualAddress, end: VirtualAddress) -> PageRange {
        PageRange {
            start: Page::from_vaddress(start),
            end: Page::from_vaddress(end-1) + 1
        }
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
pub const ENTRY_COUNT: usize = 512;

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
        assert!(frame.start_address() & !AddressBitsMask == 0,
            "frame address unaligned {:#x}", frame.start_address());
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
    entries: [PageEntry; ENTRY_COUNT],
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

    pub fn next_level_table_or_create(&mut self, index: usize) 
        -> &mut Table<L::NextLevel> {
        if self.next_level_table(index).is_none() {
            let frame = alloc_frame().expect("no more free frame available");
            //FIXME: mark mid level tables as USER to make them accessable
            self.entries[index].set(frame, WRITABLE | PRESENT | USER);
            self.next_level_table_mut(index).unwrap().zero()
        } else {
            self.next_level_table_mut(index).unwrap()
        }
    }
}

pub struct ActivePML4Table {
    mapper: Mapper
}

use core::ops::{Deref, DerefMut};
impl Deref for ActivePML4Table {
    type Target = Mapper;
    fn deref(&self) -> &Self::Target {
        &self.mapper
    }
}

impl DerefMut for ActivePML4Table {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.mapper
    }
}

impl ActivePML4Table {
    pub fn new() -> ActivePML4Table {
        ActivePML4Table {
            mapper: Mapper::new()
        }
    }

    /// execute closure f with `inactive` as temporarily mapped page tables
    pub fn with<F>(&mut self, inactive: &mut InactivePML4Table, tempPage: &mut TemporaryPage, f: F) where F: FnOnce(&mut Mapper) {
        let backup = Frame::from_paddress(::kern::arch::cpu::cr3());
        let backup2 = self.entries[511];
        assert!(backup2.pointed_frame().is_some());
        assert!(backup == backup2.pointed_frame().unwrap());

        {
            let old_pml4 = tempPage.map_table_frame(backup, self);

            self.entries[511].set(inactive.pml4_frame, backup2.flags());
            ::kern::arch::cpu::tlb_flush_all();

            f(self);

            /// we cannot use self.entries which is derefed from self.mapper.top.get_mut(), since 
            /// active pml4's top now is not recursive-mapped anymore, that's why we temp-mapped it 
            /// to old_pml4
            old_pml4[511].set(backup, backup2.flags());
            ::kern::arch::cpu::tlb_flush_all();
        }

        tempPage.unmap(self);
    }
}

pub fn create_address_space(mbinfo: &BootInformation) -> InactivePML4Table {
    let kernel_base = KERNEL_MAPPING.KernelMap.start;
    let mut active = ActivePML4Table::new();

    //FIXME: this magic address should be taken care of, prevent from conflicting
    //with normal addresses, maybe mark it with unusable
    let mut temp_page = TemporaryPage::new(Page::from_vaddress(0xfffff_cafe_beef_000));

    let mut new_map = {
        let frame = alloc_frame().expect("no more memory");
        InactivePML4Table::new(frame, &mut active, &mut temp_page)
    };

    //TODO: need to move kernel stack into high address area
    active.with(&mut new_map, &mut temp_page, |mapper| {
        let elf = mbinfo.elf_sections_tag().expect("elf sections is unavailable");
        for sect in elf.sections() {
            if !sect.is_allocated() || sect.size == 0 {
                continue;
            }

            let mut flags = PRESENT;
            if !sect.flags().contains(ELF_SECTION_EXECUTABLE) {
                flags |= NO_EXECUTE;
            }
            if sect.flags().contains(ELF_SECTION_WRITABLE) {
                flags |= WRITABLE;
            }

            assert!(sect.start_address() % PAGE_SIZE == 0, "section {:?} not page aligned", sect);
            assert!(sect.end_address() % PAGE_SIZE == 0, "section {:?} not page aligned", sect);

            if sect.start_address() >= kernel_base {
                let r = FrameRange::new(sect.start_address() - kernel_base,
                    sect.end_address() - kernel_base);

                printk!(Info, "map section [{:#x}, {:#x}) -> [{:#x}, {:#x}), flags: {:?}\n\r",
                    r.start.start_address(), r.end.start_address(),
                    r.start.start_address() + kernel_base, r.end.start_address() + kernel_base,
                    flags);
                for f in r {
                    let page = Page::from_vaddress(f.start_address() + kernel_base);
                    mapper.map_to(page, f, flags);
                }
            } else {
                //gdt's been replaced later with a new one, only the stack needed here
                //make the whole area (including booting code) for stack
                let mut r = FrameRange::new(sect.start_address(), sect.end_address());

                flags = PRESENT | NO_EXECUTE | WRITABLE;
                printk!(Info, "map boot section [{:#x}, {:#x}) -> [{:#x}, {:#x}), flags: {:?} as stack\n\r",
                    r.start.start_address(), r.end.start_address(),
                    r.start.start_address() + kernel_base, r.end.start_address() + kernel_base,
                    flags);
                printk!(Info, "skip {:#x} as stack guard\n\r", r.start.start_address());
                // guard page
                r.next();
                for f in r {
                    let page = Page::from_vaddress(f.start_address() + kernel_base);
                    mapper.map_to(page, f, flags);
                }

            }
        }

        let fb = mbinfo.framebuffer_tag().expect("no framebuffer tag");
        if fb.addr != 0xb8000 {
            // map base framebuffer (0xb8000)
            let (fb_addr, pitch, height, bpp) = (0xb8000, 160, 25, 16);
            let r = {
                let (start, sz) = (fb_addr, pitch * height);
                FrameRange::new(start, start + sz as usize)
            };
            printk!(Info, "map console framebuffer [{:#x}, {:#x})\n\r", 
                    r.start.start_address(), r.end.start_address());
            for f in r {
                let page = Page::from_vaddress(f.start_address() + kernel_base);
                mapper.map_to(page, f, WRITABLE);
            }
        }

        {
            // map framebuffer
            let r = {
                let (start, sz) = (fb.addr as usize, fb.pitch * fb.height);
                FrameRange::new(start, start + sz as usize)
            };
            printk!(Info, "map framebuffer [{:#x}, {:#x})\n\r", 
                    r.start.start_address(), r.end.start_address());
            for f in r {
                let page = Page::from_vaddress(f.start_address() + kernel_base);
                mapper.map_to(page, f, WRITABLE);
            }
        }

        {
            // map mbinfo
            let r = FrameRange::new(mbinfo.start_address() - kernel_base,
                mbinfo.end_address() - kernel_base);
            printk!(Info, "map mbinfo({:#x} -> {:#x})\n\r", mbinfo.start_address() - kernel_base,
                mbinfo.start_address());
            for f in r {
                let page = Page::from_vaddress(f.start_address() + kernel_base);
                mapper.map_to(page, f, EntryFlags::empty());
            }
        }

        {
            //map kheap area to high end of physical area
            use kheap_allocator;

            //TODO: should be lazily mapped after page fault sets up
            let start_address = KERNEL_MAPPING.KernelHeap.start;
            let alloc_size = KERNEL_MAPPING.KernelHeap.end - KERNEL_MAPPING.KernelHeap.start + 1;
            kheap_allocator::HEAP_RANGE.call_once(|| {
                Range {
                    start: start_address,
                    end: start_address + alloc_size
                }
            });

            //FIXME: so FrameAllocator should not override this region
            let mut r = {
                let mmap = mbinfo.memory_map_tag().unwrap();
                let end = mmap.memory_areas().map(|a| a.base_addr + a.length).max().unwrap() as usize;
                FrameRange::new(end - alloc_size, end)
            };

            let range = PageRange::new(start_address, start_address + alloc_size);

            printk!(Info, "map heap [{:#x}, {:#x}) -> [{:#x}, {:#x})\n\r",
                range.start.start_address(), range.end.start_address(),
                r.start.start_address(), r.end.start_address());
            for page in range {
                let f = r.next().unwrap();
                mapper.map_to(page, f, WRITABLE);
            }
        }
    });

    printk!(Info, "create_address_space {:?}\n\r", new_map);
    new_map
}

pub fn remap_the_kernel(mbinfo: &BootInformation) {
    let mut new_map = create_address_space(mbinfo);
    switch(new_map);
}

pub fn switch(new_map: InactivePML4Table) -> InactivePML4Table {
    let old = Frame::from_paddress(::kern::arch::cpu::cr3());

    unsafe {
        ::kern::arch::cpu::cr3_set(new_map.pml4_frame.start_address());
    }

    printk!(Info, "switching map from {:?} to {:?}\n\r", old, new_map);
    InactivePML4Table {
        pml4_frame: old
    }
}

pub fn test_paging_before_remap() {
    let mut pml4 = ActivePML4Table::new();
    printk!(Debug, "test_paging_before_remap\n\r");

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
        let vs = [0x3000_0000, 0x2000_0030, 0x1002_5030, 0x0702_5030];
        for &v in &vs {
            printk!(Debug, "translate({:#x}) = {:#x}\n\r", v, pml4.translate(v).unwrap_or(0));
            assert!(pml4.translate(v).unwrap() == v);
        }

        let last = ENTRY_COUNT * ENTRY_COUNT * PAGE_SIZE - 1;
        assert!(pml4.translate(last).unwrap() == last);

        let unmapped = ENTRY_COUNT * ENTRY_COUNT * PAGE_SIZE;
        assert!(pml4.translate(unmapped).is_none());
    }

    {
        use core::slice::from_raw_parts_mut;

        let frame = alloc_frame().expect("no more mem");
        let page = Page::from_vaddress(0x6218_2035_3201);
        assert!(pml4.translate(page.start_address()).is_none());
        pml4.map_to(page, frame, EntryFlags::empty());
        printk!(Debug, "map {:#x} -> {:#x}\n\r", page.start_address(), frame.start_address());

        let mut v = unsafe { from_raw_parts_mut(page.start_address() as *mut u8, 4096) };
        for p in v.iter_mut() {
            *p = 0xBA;
        }

        //printk!(Debug, "read back value {:X}\n\r", v[100]);
        for &p in v.iter() { assert!(p == 0xBA); }

        let page2 = Page { number: page.number + 100 };
        pml4.map(page2, USER);
        let mut v2 = unsafe { from_raw_parts_mut(page2.start_address() as *mut u8, 4096) };
        for p in v2.iter_mut() {
            *p = 3;
        }

        pml4.unmap(page);
        // this works cause cr0.WP is not set yet
        for p in v2.iter_mut() { *p = 3; }
    }
}


pub fn test_paging_after_remap() {
    let mut pml4 = ActivePML4Table::new();

    {
        use core::slice::from_raw_parts_mut;

        let frame = alloc_frame().expect("no more mem");
        let page = Page::from_vaddress(0x7fff_DEAD_BEEF);
        assert!(pml4.translate(page.start_address()).is_none());
        pml4.map_to(page, frame, WRITABLE);
        printk!(Debug, "map {:#x} -> {:#x}\n\r", page.start_address(), frame.start_address());

        let mut v = unsafe { from_raw_parts_mut(page.start_address() as *mut u8, 4096) };
        for p in v.iter_mut() {
            *p = 0xBA;
        }

        //printk!(Debug, "read back value {:X}\n\r", v[100]);
        for &p in v.iter() {
            assert!(p == 0xBA);
        }

        let page2 = Page { number: page.number + 100 };
        pml4.map(page2, WRITABLE);
        let mut v2 = unsafe { from_raw_parts_mut(page2.start_address() as *mut u8, 4096) };
        for p in v2.iter_mut() {
            *p = 3;
        }

        pml4.unmap(page);
        //for p in v2.iter_mut() { *p = 3; }
    }
}
