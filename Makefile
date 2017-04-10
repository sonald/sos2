ARCH := $(shell uname -m)
ifeq ($(ARCH), darwin)
	LD = ${HOME}/crossgcc/bin/x86_64-elf-ld
else
	LD = ld
endif
arch ?= x86_64
ldscript := src/kern/early.lds
kernel := build/kernel
kern_srcs := $(wildcard src/kern/arch/$(arch)/boot/*.asm)
kern_objs := $(patsubst %.asm, build/%.o, $(kern_srcs))
rust_core := target/$(arch)-unknown-linux-gnu/debug/libsos2.a

all: $(kernel) sos2.iso

# print makefile variable (for debug purpose)
print-%: ; @echo $* = $($*)

$(kernel): $(ldscript) $(kern_objs) $(rust_core)
	@mkdir -p $(@D)
	@cargo build --target=$(arch)-unknown-linux-gnu
	$(LD) -n -nostdlib -gc-sections -T $(ldscript)  -o $@ $(kern_objs) $(rust_core)

build/%.o: %.asm
	@mkdir -p $(@D)
	nasm -f elf64 $< -o $@

$(rust_core): src/lib.rs
	cargo build --target=$(arch)-unknown-linux-gnu


sos2.iso: $(kernel) 
	@mkdir -p isofiles/boot/grub
	@cp grub.cfg isofiles/boot/grub
	@cp $(kernel) isofiles/
	@grub-mkrescue -o $@ isofiles
