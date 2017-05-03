section .mboot2

MBOOT2_MAGIC equ 0xE85250D6
mboot:
	dd MBOOT2_MAGIC
	dd 0					; i386 PM
	dd mboot_end - mboot
	dd 0x100000000 - (MBOOT2_MAGIC + 0 + (mboot_end - mboot))
	align 8

	; tags
	;dw 5
	;dw 0
	;dd 20
	;dd 800
	;dd 600
	;dd 32
	;align 8

	dw 0, 0, 8
mboot_end:


global early_start
extern kernel_main

KERNEL_VMA equ 0xffff880000000000

section .text
bits 32

early_start:
	mov esp, kern_stack_top
	push ebx  ; save multiboot2 info struct

	call check_mbooted
	call check_cpuid
	call enable_SSE
	call check_long_mode
	call setup_early_pages
	call enter_long_mode

	lgdt [early_gdt_pointer]

	pop ebx
	jmp CODE_SELECTOR:long_mode_start

	hlt

setup_early_pages:
	;; Is it necessary to map 1G identity adresses here?
	mov ecx, 0
.loop:
	mov eax, (1<<21) ;; 2M base
	mul ecx
	or eax, 0x83 ; add attrs
	mov [early_pd_base + ecx * 8], eax
	inc ecx
	cmp ecx, 512
	jne .loop

	;; map 0xfd00_0000 + 24M (12 x 2M-page) area for framebuffer usage
	mov ecx, 3
	mov eax, framebuffer_pd + 0x3
	mov [early_pdp_base + ecx * 8], eax

	mov ecx, 0
.loop2:
	mov eax, (1<<21)
	mul ecx
	add eax, 0xfd000000
	or eax, 0x83
	mov [early_pd_base + ecx * 8 + 3904], eax
	inc ecx
	cmp ecx, 12
	jne .loop2

	;; map first 1G into higher half (0xffff_8800_0000_0000)
	mov ecx, 272
	mov eax, early_pdp_base_higher + 0x3
	mov [early_pml4_base + ecx * 8], eax

	mov ecx, 0
.loop3:
	mov eax, (1<<21) ;; 2M base
	mul ecx
	or eax, 0x83 ; add attrs
	mov [early_pd_base_higher + ecx * 8], eax
	inc ecx
	cmp ecx, 512
	jne .loop3

	ret

enter_long_mode:
	; load P4 to cr3 register (cpu uses this to access the P4 table)
	mov eax, early_pml4_base
	mov cr3, eax

	; enable PAE-flag in cr4 (Physical Address Extension)
	mov eax, cr4
	or eax, 1 << 5
	mov cr4, eax

	; set the long mode bit in the EFER MSR (model specific register)
	mov ecx, 0xC0000080
	rdmsr
	or eax, 1 << 8
	wrmsr

	; enable paging in the cr0 register
	mov eax, cr0
	or eax, 1 << 31
	mov cr0, eax

	ret
	
check_mbooted:
	cmp eax, 0x36d76289
	jne .no_mboot
	ret
.no_mboot:
	mov al, '0'
	call error 

check_cpuid:
    call is_cpuid_capable
	test eax, eax
	jz .no_cpuid
	ret
.no_cpuid:
	mov al, '1'
	call error

check_long_mode:
    ; test if extended processor info in available
    mov eax, 0x80000000    ; implicit argument for cpuid
    cpuid                  ; get highest supported argument
    cmp eax, 0x80000001    ; it needs to be at least 0x80000001
    jb .no_long_mode       ; if it's less, the CPU is too old for long mode

    ; use extended info to test if long mode is available
    mov eax, 0x80000001    ; argument for extended processor info
    cpuid                  ; returns various feature bits in ecx and edx
    test edx, 1 << 29      ; test if the LM-bit is set in the D-register
    jz .no_long_mode       ; If it's not set, there is no long mode
    ret
.no_long_mode:
    mov al, '2'
    call error

; Check for SSE and enable it. If it's not supported throw error "a".
; from osdev: http://wiki.osdev.org/SSE#Checking_for_SSE
; rust may use sse instructions by default
enable_SSE:
	; check for SSE
	mov eax, 0x1
	cpuid
	test edx, 1<<25
	jz .no_SSE

	; enable SSE
	mov eax, cr0
	and ax, 0xFFFB      ; clear coprocessor emulation CR0.EM
	or ax, 0x2          ; set coprocessor monitoring  CR0.MP
	mov cr0, eax
	mov eax, cr4
	or ax, 3 << 9       ; set CR4.OSFXSR and CR4.OSXMMEXCPT at the same time
	mov cr4, eax

	ret
.no_SSE:
	mov al, 'a'
	call error

error:
	mov dword [0xb8000], 0x4f524f45
	mov dword [0xb8004], 0x4f3a4f52
	mov dword [0xb8008], 0x4f204f20
	mov byte  [0xb800a], al
	hlt


is_cpuid_capable:
	; try to modify ID flag
	pushfd
	pop eax
	mov ecx, eax
	xor eax, 0x200000 ; flip ID
	push eax
	popfd

	; test if modify success
	pushfd
	pop eax
	xor eax, ecx
	shr eax, 21
	and eax, 1
	push ecx
	popfd
	ret



section .text
bits 64
long_mode_start:
	; clear all other selectors, since they ignored by 64-bit sub-mode
    mov ax, 0
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

	movsxd rdi, ebx
	mov rax, KERNEL_VMA
	add rdi, rax
	mov rax, kernel_main
	call rax

	cli
	hlt


LONG_MODE_BIT equ (1<<21)
PRESENT_BIT  equ  (1<<15)
CODE_SEG_BITS equ (3<<11)
CODE_SELECTOR equ (early_gdt_base.code - early_gdt_base)
; early datas
section .early_gdt
align 8
early_gdt_base:
	dq 0 ; Null
.code:
	; code descriptor
	;dq (1<<43) | (1<<44) | (1<<47) | (1<<53) ; code segment
	dd 0
	dd LONG_MODE_BIT | PRESENT_BIT | (CODE_SEG_BITS)
early_gdt_pointer:
	dw $ - early_gdt_base - 1 ; limit
	dq early_gdt_base


section .data
align 0x1000
early_pml4_base:
	dq early_pdp_base + 0x3 ; S,R/W,P
	times (0x200 - 2) dq 0
	dq early_pml4_base + 0x3 ; recursive-mapping 511-th entry
early_pdp_base:
	dq early_pd_base + 0x3 ; S,R/W,P
	times (0x200 - 1) dq 0
early_pd_base:
	times 0x200 dq 0

framebuffer_pd:
	times 0x200 dq 0

;; entries for higher-half
early_pdp_base_higher:
	dq early_pd_base_higher + 0x3 ; S,R/W,P
	times (0x200 - 1) dq 0
early_pd_base_higher:
	times 0x200 dq 0

section .early_stack nobits
_kern_stack:
    times 8 resb 4096
kern_stack_top:
