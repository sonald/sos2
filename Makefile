OS := $(shell uname -s)
ifeq ($(OS), Darwin)
	LD = ${HOME}/crossgcc/bin/x86_64-elf-ld
else
	LD = ld
endif
arch ?= x86_64
target := $(arch)-sos2
ldscript := src/kern/kernel.lds
QEMU := qemu-system-x86_64 
kernel := build/kernel
kern_srcs := $(wildcard src/kern/arch/$(arch)/boot/*.asm)
kern_objs := $(patsubst %.asm, build/%.o, $(kern_srcs))
rust_core := target/$(target)/debug/libsos2.a

all: $(kernel) sos2.iso

# print makefile variable (for debug purpose)
print-%: ; @echo $* = $($*)

run:
	$(QEMU) -cdrom sos2.iso -serial stdio -usb

$(kernel): kern $(ldscript) $(kern_objs) $(rust_core)
	@mkdir -p $(@D)
	$(LD) -n -nostdlib -gc-sections -T $(ldscript)  -o $@ $(kern_objs) $(rust_core)

build/%.o: %.asm
	@mkdir -p $(@D)
	nasm -f elf64 $< -o $@

kern: 
	xargo build --target=$(target) --features "test kdebug"


sos2.iso: $(kernel) 
	@mkdir -p isofiles/boot/grub
	@cp grub.cfg isofiles/boot/grub
	@cp $(kernel) isofiles/
	@grub-mkrescue -o $@ isofiles
