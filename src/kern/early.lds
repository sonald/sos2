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
        *(.early_stack)
        . = ALIGN(4K);
    }

    .data.rel.ro : {
        *(.data.rel.ro.local*)
        *(.data.rel.ro .data.rel.ro.*)
        . = ALIGN(4K);
    }


    .gcc_except_table : ALIGN(4K) {
        *(.gcc_except_table)
        . = ALIGN(4K);
    }
    _end = .;
}
