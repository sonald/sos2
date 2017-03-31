ENTRY(early_start)

SECTIONS 
{
    . = 1M;

    _start = .;

    .text : {
        KEEP(*(.mboot2))
        *(.text .text.*)
    }

    .data : {
        *(.early_gdt)
        *(.data .data.*)
        *(.early_stack)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data.rel.ro : {
        *(.data.rel.ro.local*)
        *(.data.rel.ro .data.rel.ro.*)
    }

    _end = .;
}
