# Ring 0 Roadmap — From v1.7.5 to AMDGPU driver

> v1.7.5 finished the **HAL consolidation**. The next step is the
> **driver foundation**: abstractions that allow any driver (network,
> storage, GPU) to register and use the hardware without reimplementing
> the same boilerplate.

## Why this matters

Without the driver foundation, adding a new driver (e.g. AMDGPU) means:

- ❌ Driver pokes raw `*mut u32` for MMIO, allows reordering
- ❌ Driver implements its own PCI config-space scanner
- ❌ Driver implements its own logging
- ❌ Driver implements its own MSI-X setup
- ❌ Driver doesn't check for resource conflicts
- ❌ Driver's `init()` is hardcoded in `coordinator::main`

With the driver foundation:

- ✅ MMIO goes through `MmioRegion` (volatile + cache-correct)
- ✅ PCI config space via `Bus` trait
- ✅ Logging via `Logger` (per-subsystem)
- ✅ MSI-X via `arch::msi` (allocation + masking)
- ✅ Resources via `ResourceManager` (no conflicts)
- ✅ Driver registers itself; `coordinator::main` is generic

## Current state (v1.7.5)

```
ring0/ — 7,225 LOC, 17 warnings, compiles release clean
✅ platform, arch, mem, dev, proc, cpu, boot       (HAL consolidated)
✅ result.rs                                        (KError + KResult)
✅ sync.rs                                          (SpinLock, IrqSpinLock, OnceCell)
✅ log.rs                                           (Logger struct, per-subsystem)
✅ mem/mmio.rs                                      (MmioRegion)
✅ cpu/delay.rs                                     (udelay, mdelay, deadline)
```

## Roadmap

### v1.7.6 — Foundation for drivers (CURRENT)

Status: **partially complete** (Fase 1 + 2 done; Fase 3 pending)

| Module | LOC | Status |
|---|---|---|
| `result.rs` | 80 | ✅ Done |
| `sync.rs` | 200 | ✅ Done (SpinLock, IrqSpinLock, OnceCell) |
| `log.rs` | 150 | ✅ Done (Logger struct) |
| `cpu/delay.rs` | 50 | ✅ Done (udelay, mdelay) |
| `mem/mmio.rs` | 200 | ✅ Done (MmioRegion) |
| `mem/dma.rs` | 200 | ⏳ TODO — DmaBuffer + alloc helpers |
| Refactor `dev/console` to use `KResult` | 50 | ⏳ TODO |
| Refactor `dev/pcie` to use `KResult` + KError | 100 | ⏳ TODO |

### v1.7.7 — Bus + Registry

| Module | LOC | Status |
|---|---|---|
| `dev/registry.rs` | 250 | ⏳ TODO — trait Driver + register/unregister |
| `dev/bus.rs` | 200 | ⏳ TODO — trait Bus + PciBus impl |
| `dev/pcie.rs` (extended) | +150 | ⏳ TODO — probe_bar, has_capability |
| `arch/msi.rs` | 300 | ⏳ TODO — MSI-X allocation + masking |
| `proc/resource.rs` | 200 | ⏳ TODO — ResourceManager |

### v1.7.8 — Driver refactor

| Task | Status |
|---|---|
| Rewrite `dev/console` using `trait Driver` | ⏳ TODO |
| Rewrite `dev/framebuffer` using `trait Driver` | ⏳ TODO |
| Rewrite `dev/pcie` using `trait Bus` | ⏳ TODO |
| Rewrite `dev/audio` using `trait Driver` | ⏳ TODO |
| Add driver probe/remove lifecycle to coordinator | ⏳ TODO |

### v1.8.0 — AMDGPU driver stub

| Task | Status |
|---|---|
| `dev/amdgpu.rs` skeleton with `trait Driver` impl | ⏳ TODO |
| Vendor 0x1002, Device 0x73BF (RX 580) probe | ⏳ TODO |
| BAR0 MMIO mapping | ⏳ TODO |
| Doorbell + ring buffer (read-only skeleton) | ⏳ TODO |
| Display via framebuffer (no modesetting yet) | ⏳ TODO |

### v1.9.0 — AMDGPU functional

| Task | Status |
|---|---|
| Modesetting (set display resolution) | ⏳ TODO |
| Cursor + 2D blit | ⏳ TODO |
| GEM object allocator | ⏳ TODO |
| PRIME (DMA-BUF) for interop | ⏳ TODO |
| Suspend/resume (PCI state save/restore) | ⏳ TODO |

## File-level changes for v1.7.6

### `mem/dma.rs` (200 LOC)

```rust
// Coherent DMA buffer for low-throughput control (e.g. command rings)
pub struct DmaBuffer { virt: *mut u8, phys: PhysAddr, size: usize }

pub fn alloc_coherent(size: usize) -> KResult<DmaBuffer>;
pub fn alloc_streaming(size: usize) -> KResult<DmaBuffer>;
pub fn sync_for_device(&self);
pub fn sync_for_cpu(&self);
pub fn virt(&self) -> *mut u8;
pub fn phys(&self) -> PhysAddr;
pub fn size(&self) -> usize;
```

### `dev/console` refactor

Replace `init_serial()` returning `()` with `init() -> KResult<()>`.
Replace `serial_write(s)` with `write_str(s) -> KResult<usize>` (or keep
the simple version for boot path).

### `dev/pcie` refactor

Replace `init_ecam(base, end_bus)` with:
```rust
pub struct PciBus { ecam: Option<MmioRegion>, legacy: bool }
impl Bus for PciBus { ... }
```

The existing `init_ecam` becomes a deprecated alias.

## How to add a new driver (post v1.7.7)

Once the registry exists, adding a driver is:

```rust
// 1. Implement the trait
struct MyDriver;
impl Driver for MyDriver {
    fn name(&self) -> &str { "mydev" }
    fn probe(&self, id: DeviceId) -> bool {
        id.vendor == 0x1234 && id.device == 0x5678
    }
    fn init(&mut self, id: DeviceId, res: &mut Resources) -> KResult<()> {
        let bar0 = res.map_bar(id, 0)?;
        let irq = res.alloc_irq(id, 1)?;
        // ... set up hardware
        Ok(())
    }
    fn remove(&mut self) -> KResult<()> { Ok(()) }
}

// 2. Register at boot
static DRIVER: MyDriver = MyDriver;
register_driver(&DRIVER);
```

The kernel enumerates PCI, calls `probe()` for each registered driver,
and the matching driver takes over the device. No `coordinator::main`
changes needed.

## Why traits, not module-per-driver

`trait Driver` + registry:
- ✅ Driver is testable (mock device IDs)
- ✅ Hot-pluggable (register after boot)
- ✅ Conflicting drivers detected at runtime
- ✅ Resource conflicts caught at `init()` time

Module-per-driver with hardcoded `init()`:
- ❌ Driver is wired in stone
- ❌ Conflicts caught by "first one wins" or crash
- ❌ Adding a driver = editing `coordinator::main`

We're going with the module-per-driver **registration** pattern (per
user choice) but still keep a registry so future changes are easy.

## See also

- `ring0/_docs/result.md`     — KError semantics
- `ring0/_docs/sync.md`       — SpinLock / OnceCell usage
- `ring0/_docs/log.md`        — Logger per-subsystem
- `ring0/_docs/mmio.md`       — MmioRegion reference
- `ring0/_docs/delay.md`      — udelay / mdelay
