# FastOS — Top-Level Build
# Two pillars: NASM boot (Ring 0) → Rust kernel + drivers (Ring 0)
# Fagging-Scale architecture: everything Ring 0, no userspace.

.PHONY: all boot kernel driver clean test

all: boot kernel

boot:
	@echo "=== Building NASM bootloader ==="
	$(MAKE) -C boot

kernel:
	@echo "=== Building Rust kernel ==="
	cd kernel && cargo build

driver:
	@echo "=== Building GPU driver (Driver_Canon GA106) ==="
	cd "Driver_Canon GA106" && cargo build

clean:
	$(MAKE) -C boot clean
	cd kernel && cargo clean
	cd "Driver_Canon GA106" && cargo clean

# Test with QEMU
test: all
	qemu-system-x86_64 -drive format=raw,file=boot/fastos.img -serial stdio -m 512M

# Create full disk image with kernel
image: boot kernel
	@echo "=== Creating full FastOS image ==="
	cat boot/stage1.bin boot/stage2.bin > fastos.img
	truncate -s 1M fastos.img
