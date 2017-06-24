extern syscall_dispatch
global syscall_entry

section .text
bits 64
syscall_entry:
	swapgs
	mov [gs:0], rsp ; save user rsp
	mov rsp, [gs:8] ; load kern rsp

	push rbp
	push r11
	push rcx

	; args: rdi, rsi, rdx, r8, r9, r10
	; rax is syscall number, and return value
	push rax
    push r10
    push r9
    push r8
    push rdx
    push rsi
	push rdi

	sti
	mov rdi, rax
	mov rsi, rsp
	mov rcx, syscall_dispatch
	call rcx
	cli

	pop rdi
    pop rsi
    pop rdx
    pop r8
    pop r9
    pop r10
	pop rax

	pop rcx
	pop r11
	pop rbp

	mov rsp, [gs:0]
	swapgs

	db 0x48
	sysret

