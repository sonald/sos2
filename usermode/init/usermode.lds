ENTRY(start)
OUTPUT_FORMAT(elf64-x86-64)

USER_VMA = 0x04000000;

SECTIONS 
{
    . = USER_VMA;

    _user_start = .;

    .text : {
        *(.text .text.*)
        . = ALIGN(4K);
    }

    .data : {
        *(.rodata .rodata.*)
        *(.got .got.plt)
        *(.data.rel.ro .data.rel.ro.*)
        *(.data.rel.ro.local*)
        . = ALIGN(4K);
    }


    .bss : {
        *(.bss .bss.*)
        . = ALIGN(4K);
    }

    _user_end = .;
}

