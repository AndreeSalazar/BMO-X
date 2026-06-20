# MMIO: `MmioRegion`

> Safe wrapper around memory-mapped I/O regions. Replaces raw
> `*mut u32` pointer arithmetic with typed accessors that use
> `read_volatile` / `write_volatile` and report the correct cache
> type to the page tables.

## Import

```rust
use crate::mem::mmio::{MmioRegion, CacheType};
use crate::result::KResult;
```

## Mapping a region

```rust
let regs = MmioRegion::map(0xE000_0000, 0x1000, CacheType::Uncacheable)?;
```

The mapping is page-aligned internally. The region is unmapped
automatically when `regs` is dropped (RAII).

`CacheType`:
- `CacheType::Uncacheable` — every read/write goes to the device.
  Use for control registers.
- `CacheType::WriteCombining` — writes are buffered. Use for
  framebuffers and high-throughput data paths.
- `CacheType::WriteBack` — the default for RAM. Use only for
  memory-mapped RAM (VRAM exposed as BAR on some GPUs).

## Reading and writing

```rust
// u32
let id = regs.read_u32(0x00);
regs.write_u32(0x04, 0x01);

// u64
let val = regs.read_u64(0x08);
regs.write_u64(0x10, 0xDEAD_BEEF_CAFE_BABE);

// u8 (for byte-level access)
let byte = regs.read_u8(0x100);
regs.write_u8(0x101, 0xFF);

// Atomic read-modify-write
regs.set_bits_u32(0x04, 0x01);    // set bit 0
regs.clear_bits_u32(0x04, 0x01);  // clear bit 0
```

All accessors are `#[inline]` and use `read_volatile` / `write_volatile`
under the hood. The compiler cannot elide or reorder them.

## Polling for hardware completion

```rust
// Wait until (regs[0x10] & 0x01) == 0x01, up to 1 ms
regs.poll_u32(0x10, 0x01, 0x01, 1_000)?;
```

`poll_u32` returns `Err(KError::Timeout)` if the condition is not met
within the timeout. Use this for hardware that signals completion via
a status register, instead of unbounded `while` loops.

## Why this, not raw pointers

A raw `*mut u32` is dangerous in three ways:

1. **Compiler reordering.** The compiler is free to reorder
   `*ptr = 1; *ptr = 2;` if it determines the writes are independent.
   Hardware may not see `1` at all. `write_volatile` forces the
   compiler to emit the writes in source order.

2. **CPU caching.** If the region is mapped as Write-Back, the CPU
   may buffer writes indefinitely. Hardware never sees the write.
   `MmioRegion::map(..., CacheType::Uncacheable)` sets the PCD/PWT
   bits in the page tables.

3. **Type confusion.** `*mut u32 + 0x80` is easy to write by accident;
   `regs.write_u32(0x80, value)` is harder. The `MmioRegion` API
   makes the offset explicit.

## BarInfo for PCI drivers

When mapping a PCI BAR, the driver usually wants to know the size:

```rust
// (in v1.7.7 — see dev/pcie.rs)
let info = pcie.probe_bar(id, 0)?;
let size = info.size;        // e.g. 16 MiB
let is_64 = info.is_64bit;
let is_prefetchable = info.is_prefetchable;

let cache = if is_prefetchable {
    CacheType::WriteCombining
} else {
    CacheType::Uncacheable
};
let regs = MmioRegion::map(info.addr, size, cache)?;
```

## Anti-patterns

```rust
// ❌ Don't: raw pointer access
let regs = 0xE000_0000 as *mut u32;
unsafe { regs.add(0x04).write_volatile(1) };

// ❌ Don't: forget to unmap
let regs = MmioRegion::map(...)?;
// regs leaks if you forget to drop

// ❌ Don't: use a SpinLock around MMIO reads
let lock = SPIN.lock();
let v = regs.read_u32(0x10);  // volatile, doesn't need locking
drop(lock);
```
