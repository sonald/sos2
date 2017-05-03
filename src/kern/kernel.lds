ENTRY(early_start)
OUTPUT_FORMAT(elf64-x86-64)

KERNEL_VMA = 0xffff880000000000;

SECTIONS 
{
    . = 1M;

    .boot : AT(1M) {
        KEEP(*(.mboot2))
        build/src/kern/arch/x86_64/boot/boot.o (*)
        . = ALIGN(4K);
    }

    . += KERNEL_VMA;
    _start = .;

    .text : AT(ADDR(.text) - KERNEL_VMA) {
        *(.rodata .rodata.*)
        *(EXCLUDE_FILE(*build/src/kern/arch/x86_64/boot/boot.o) .text .text.*)
        . = ALIGN(4K);
    }

    .data : AT(ADDR(.data) - KERNEL_VMA) {
        *(EXCLUDE_FILE(*build/src/kern/arch/x86_64/boot/boot.o) .data .data.*)
        *(.got .got.plt)
        *(.data.rel.ro.local*)
        *(.data.rel.ro .data.rel.ro.*)
        . = ALIGN(4K);
    }


    .bss : AT(ADDR(.bss) - KERNEL_VMA) {
        *(.bss .bss.*)
        . = ALIGN(4K);
    }

    .gcc_except_table : AT(ADDR(.gcc_except_table) - KERNEL_VMA) {
        *(.gcc_except_table)
        . = ALIGN(4K);
    }

    _end = .;
}
