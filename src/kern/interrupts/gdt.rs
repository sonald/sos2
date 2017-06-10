use x86_64::structures::tss::TaskStateSegment;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::instructions::tables::{lgdt, DescriptorTablePointer};
use x86_64::PrivilegeLevel;
use core::mem::size_of;
use bit_field::BitField;

/// the new GDT after remap kernel and setup paging,
/// some entry is 64bit long, some is 128bit long
#[derive(Debug)]
pub struct GlobalDescriptorTable {
    table: [u64; 8],
    next: usize // point to  next available entry index
}

impl GlobalDescriptorTable {
    pub const fn new() -> GlobalDescriptorTable {
        GlobalDescriptorTable {
            table: [0; 8],
            next: 1
        }
    }

    pub fn add_entry(&mut self, desc: Descriptor) -> SegmentSelector {
        let index = self.next;

        match desc {
            Descriptor::UserSegment(v) => {
                assert!(self.next < self.table.len(), "gdt is full");
                self.table[index] = v;
                self.next += 1;
            },
            Descriptor::SystemSegment(v1, v2) => {
                assert!(self.next + 1 < self.table.len(), "gdt is full");
                self.table[index] = v1;
                self.table[index+1] = v2;
                self.next += 2;
            }
        }

        SegmentSelector::new(index as u16, PrivilegeLevel::Ring0)
    }

    pub fn load(&'static self) {
        let dtp = DescriptorTablePointer {
            base: self.table.as_ptr() as u64,
            limit: (size_of::<[u64; 8]>() - 1) as u16,
        };
        unsafe { lgdt(&dtp); }
    }
}

bitflags! {
    flags DescriptorFlags: u64 {
        const CONFORMING        = 1 << 42,
        const EXECUTABLE        = 1 << 43,
        const USER_SEGMENT      = 1 << 44,
        const PRESENT           = 1 << 47,
        const LONG_MODE         = 1 << 53,
    }
}

pub enum Descriptor {
    UserSegment(u64),
    SystemSegment(u64, u64),
}

impl Descriptor {
    pub fn user_code_segment() -> Descriptor {
        let flags = USER_SEGMENT | PRESENT | EXECUTABLE | LONG_MODE;
        let mut bits = flags.bits();
        bits.set_bit(41, true);
        bits.set_bits(45..47, 0b11); //DPL = 3
        Descriptor::UserSegment(bits)
    }

    pub fn user_data_segment() -> Descriptor {
        let flags = USER_SEGMENT | PRESENT | LONG_MODE;
        let mut bits = flags.bits();
        //NOTE: amd64 says this is ignored in 64-bit submode, 
        //but without it, iretq will generate GP
        bits.set_bit(41, true);
        bits.set_bits(45..47, 0b11); //DPL = 3
        Descriptor::UserSegment(bits)
    }

    pub fn kernel_code_segment() -> Descriptor {
        let flags = USER_SEGMENT | PRESENT | EXECUTABLE | LONG_MODE;
        Descriptor::UserSegment(flags.bits())
    }

    pub fn tss_segment(tss: &'static TaskStateSegment) -> Descriptor {
        let ptr = tss as *const _ as u64;

        let mut low = PRESENT.bits();
        // base
        low.set_bits(16..40, ptr.get_bits(0..24));
        low.set_bits(56..64, ptr.get_bits(24..32));
        // limit (the `-1` in needed since the bound is inclusive)
        low.set_bits(0..16, (size_of::<TaskStateSegment>() - 1) as u64);
        // type (0b1001 = available 64-bit tss)
        low.set_bits(40..44, 0b1001);
        //low.set_bits(45..47, 0b11); //DPL = 3

        let mut high = 0;
        high.set_bits(0..32, ptr.get_bits(32..64));

        Descriptor::SystemSegment(low, high)
    }
}
