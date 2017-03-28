global long_mode_start
extern kernel_main

section .data
bits 64
long_mode_start:
	; clear all other selectors, since they ignored by 64-bit sub-mode
    mov ax, 0
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

	mov qword [0xb8000], 0x4f204f204f4d4f4c ;LM

	mov edi, ebx
	call kernel_main

	hlt


