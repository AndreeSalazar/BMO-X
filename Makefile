# FastOS — Top-Level Build
# Three pillars: NASM boot → Rust kernel → C upper layers

.PHONY: all boot kernel adead clean test

all: boot kernel

boot:
	@echo "=== Building NASM bootloader ==="
	$(MAKE) -C boot

kernel:
	@echo "=== Building Rust kernel ==="
	cd kernel && cargo build

adead:
	@echo "=== Building ADead-BIB (C) ==="
	$(MAKE) -C adead

clean:
	$(MAKE) -C boot clean
	cd kernel && cargo clean
	$(MAKE) -C adead clean

# Test with QEMU
test: all
	qemu-system-x86_64 -drive format=raw,file=boot/fastos.img -serial stdio -m 512M

# Create full disk image with kernel
image: boot kernel
	@echo "=== Creating full FastOS image ==="
	cat boot/stage1.bin boot/stage2.bin > fastos.img
	# Append kernel binary (future: use objcopy to extract raw binary)
	truncate -s 1M fastos.img
