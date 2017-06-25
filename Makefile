OS := $(shell uname -s)
ifeq ($(OS), Darwin)
	GRUB_MKRESCUE = ${HOME}/crossgcc/bin/grub-mkrescue
	LD = ${HOME}/crossgcc/bin/x86_64-elf-ld
else
	LD = ld
	GRUB_MKRESCUE = grub-mkrescue
endif
ROOT = $(shell pwd)

arch ?= x86_64
target := $(arch)-sos2
user_target := $(arch)-sos2-user
ldscript := src/kern/kernel.lds
QEMU := qemu-system-x86_64 


kernel := build/kernel
init := usermode/init/target/$(user_target)/debug/init
kern_srcs := $(wildcard src/kern/arch/$(arch)/boot/*.asm src/kern/arch/$(arch)/*.asm)
kern_objs := $(patsubst %.asm, build/%.o, $(kern_srcs))
rust_core := target/$(target)/debug/libsos2.a

all: $(kernel) sos2.iso

# print makefile variable (for debug purpose)
print-%: ; @echo $* = $($*)

run: $(kernel) sos2.iso
	$(QEMU) -cdrom sos2.iso -serial stdio -usb -vga vmware --no-reboot

$(kernel): kern $(ldscript) $(kern_objs) $(rust_core)
	@mkdir -p $(@D)
	$(LD) -n -nostdlib -gc-sections -T $(ldscript)  -o $@ $(kern_objs) $(rust_core)

build/%.o: %.asm
	@mkdir -p $(@D)
	nasm -f elf64 $< -o $@

kern: 
	xargo build --target=$(target) --features "test kdebug"

check:
	xargo check --target=$(target) --features "test kdebug"

init:
	cd usermode/init && xargo build --target=$(user_target)
	#RUST_TARGET_PATH=$(ROOT)/usermode/init xargo build --target $(user_target) --manifest-path usermode/init/Cargo.toml

sos2.iso: $(kernel) init
	@mkdir -p isofiles/boot/grub
	@cp grub.cfg isofiles/boot/grub
	@cp $(kernel) isofiles/
	@cp $(init) isofiles/
	@$(GRUB_MKRESCUE) -o $@ isofiles
