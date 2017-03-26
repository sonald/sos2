ENTRY(early_start)

SECTIONS 
{
    . = 1M;

    .text : {
        KEEP(*(.mboot2))
        *(.text)
    }

    .data : {
        *(.early_gdt)
        *(.data)
        *(.early_stack)
    }
}
