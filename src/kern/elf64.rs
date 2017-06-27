use core::fmt;
use core::iter::Iterator;
use ::kern::console::LogLevel::*;

pub const SIZEOF_IDENT: usize = 16;
pub const SIZEOF_EHDR: usize = 64;
pub const ELFCLASS: u8 = ELFCLASS64;

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Header {
    /// Magic number and other info
    pub e_ident: [u8; SIZEOF_IDENT],
    /// Object file type
    pub e_type: u16,
    /// Architecture
    pub e_machine: u16,
    /// Object file version
    pub e_version: u32,
    /// Entry point virtual address
    pub e_entry: u64,
    /// Program header table file offset
    pub e_phoff: u64,
    /// Section header table file offset
    pub e_shoff: u64,
    /// Processor-specific flags
    pub e_flags: u32,
    /// ELF header size in bytes
    pub e_ehsize: u16,
    /// Program header table entry size
    pub e_phentsize: u16,
    /// Program header table entry count
    pub e_phnum: u16,
    /// Section header table entry size
    pub e_shentsize: u16,
    /// Section header table entry count
    pub e_shnum: u16,
    /// Section header string table index
    pub e_shstrndx: u16,
}

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "e_ident: {:?} e_type: {} e_machine: 0x{:x} e_version: 0x{:x} e_entry: 0x{:x} \
               e_phoff: 0x{:x} e_shoff: 0x{:x} e_flags: {:x} e_ehsize: {} e_phentsize: {} \
               e_phnum: {} e_shentsize: {} e_shnum: {} e_shstrndx: {}",
               self.e_ident,
               et_to_str(self.e_type),
               self.e_machine,
               self.e_version,
               self.e_entry,
               self.e_phoff,
               self.e_shoff,
               self.e_flags,
               self.e_ehsize,
               self.e_phentsize,
               self.e_phnum,
               self.e_shentsize,
               self.e_shnum,
               self.e_shstrndx)
    }
}


/// No file type.
pub const ET_NONE: u16 = 0;
/// Relocatable file.
pub const ET_REL: u16 = 1;
/// Executable file.
pub const ET_EXEC: u16 = 2;
/// Shared object file.
pub const ET_DYN: u16 = 3;
/// Core file.
pub const ET_CORE: u16 = 4;
/// Number of defined types.
pub const ET_NUM: u16 = 5;

/// The ELF magic number.
pub const ELFMAG: &'static [u8; 4] = b"\x7FELF";
/// Sizeof ELF magic number.
pub const SELFMAG: usize = 4;

/// File class byte index.
pub const EI_CLASS: usize = 4;
/// Invalid class.
pub const ELFCLASSNONE: u8 = 0;
/// 32-bit objects.
pub const ELFCLASS32: u8 = 1;
/// 64-bit objects.
pub const ELFCLASS64: u8 = 2;
/// ELF class number.
pub const ELFCLASSNUM: u8 = 3;


/// Convert an ET value to their associated string.
#[inline]
pub fn et_to_str(et: u16) -> &'static str {
    match et {
        ET_NONE => "NONE",
        ET_REL => "REL",
        ET_EXEC => "EXEC",
        ET_DYN => "DYN",
        ET_CORE => "CORE",
        ET_NUM => "NUM",
        _ => "UNKNOWN_ET",
    }
}


#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct ProgramHeader {
    /// Segment type
    pub p_type: u32,
    /// Segment flags
    pub p_flags: u32,
    /// Segment file offset
    pub p_offset: u64,
    /// Segment virtual address
    pub p_vaddr: u64,
    /// Segment physical address
    pub p_paddr: u64,
    /// Segment size in file
    pub p_filesz: u64,
    /// Segment size in memory
    pub p_memsz: u64,
    /// Segment alignment
    pub p_align: u64,
}

pub const SIZEOF_PHDR: usize = 56;

/// Program header table entry unused
pub const PT_NULL: u32 = 0;
/// Loadable program segment
pub const PT_LOAD: u32 = 1;
/// Dynamic linking information
pub const PT_DYNAMIC: u32 = 2;
/// Program interpreter
pub const PT_INTERP: u32 = 3;
/// Auxiliary information
pub const PT_NOTE: u32 = 4;
/// Reserved
pub const PT_SHLIB: u32 = 5;
/// Entry for header table itself
pub const PT_PHDR: u32 = 6;
/// Thread-local storage segment
pub const PT_TLS: u32 = 7;
/// Number of defined types
pub const PT_NUM: u32 = 8;
/// Start of OS-specific
pub const PT_LOOS: u32 = 0x60000000;
/// GCC .eh_frame_hdr segment
pub const PT_GNU_EH_FRAME: u32 = 0x6474e550;
/// Indicates stack executability
pub const PT_GNU_STACK: u32 = 0x6474e551;
/// Read-only after relocation
pub const PT_GNU_RELRO: u32 = 0x6474e552;
/// Sun Specific segment
pub const PT_LOSUNW: u32 = 0x6ffffffa;
/// Sun Specific segment
pub const PT_SUNWBSS: u32 = 0x6ffffffa;
/// Stack segment
pub const PT_SUNWSTACK: u32 = 0x6ffffffb;
/// End of OS-specific
pub const PT_HISUNW: u32 = 0x6fffffff;
/// End of OS-specific
pub const PT_HIOS: u32 = 0x6fffffff;
/// Start of processor-specific
pub const PT_LOPROC: u32 = 0x70000000;
/// ARM unwind segment
pub const PT_ARM_EXIDX: u32 = 0x70000001;
/// End of processor-specific
pub const PT_HIPROC: u32 = 0x7fffffff;

/// Segment is executable
pub const PF_X: u32 = 1 << 0;
/// Segment is writable
pub const PF_W: u32 = 1 << 1;
/// Segment is readable
pub const PF_R: u32 = 1 << 2;

pub struct ProgramHeaderIter<'a> {
    data: &'a [u8],
    header: &'a Header,
    next: usize
}

pub struct Elf64<'a> {
    pub header: &'a Header,
    pub data: &'a [u8]
}

impl<'a> Elf64<'a> {
    pub unsafe fn from(bytes: &'a [u8]) -> Elf64<'a> {
        let h = &*(bytes.as_ptr() as *const Header);

        Elf64 {
            data: bytes,
            header: h,
        }
    }

    pub fn program_headers(&self) -> ProgramHeaderIter<'a> {
        ProgramHeaderIter {
            data: self.data,
            header: self.header,
            next: 0
        }
    }
}

impl<'a> Iterator for ProgramHeaderIter<'a> {
    type Item = &'a ProgramHeader;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next < self.header.e_phnum as usize {
            let program = unsafe { 
                &*(self.data.as_ptr().offset(
                    self.header.e_phoff as isize +
                    self.header.e_phentsize as isize * self.next as isize)
                    as *const ProgramHeader)
            };
            self.next += 1;
            Some(program)
        } else {
            None
        }
    }
}

