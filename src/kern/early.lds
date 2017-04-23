ENTRY(early_start)

SECTIONS 
{
    . = 1M;

    _start = .;

    .rodata : {
        KEEP(*(.mboot2))
        *(.rodata .rodata.*)
        . = ALIGN(4K);
    }


    .text : {
        *(.text .text.*)
        . = ALIGN(4K);
    }

    .data : {
        *(.early_gdt)
        *(.data .data.*)
        *(.got .got.plt)
        *(.early_stack)
        . = ALIGN(4K);
    }

    .data.rel.ro : {
        *(.data.rel.ro.local*)
        *(.data.rel.ro .data.rel.ro.*)
        . = ALIGN(4K);
    }

    .gcc_except_table : {
        *(.gcc_except_table)
        . = ALIGN(4K);
    }

    .debug : {
        *(.debug_str .debug_abbrev)
        *(.debug_info .debug_ranges)
        *(.debug_macinfo .debug_pubnames)
        *(.debug_pubtypes .debug_line) 
        *(.symtab .strtab)
        . = ALIGN(4K);
    }
    _end = .;
}
