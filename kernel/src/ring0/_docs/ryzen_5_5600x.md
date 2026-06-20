# Ryzen 5 5600X — CPU & Architecture Support

> v1.7.7 added full CPUID detection and per-CPU data structures
> optimized for the Ryzen 5 5600X (Zen 3 / Vermeer), but portable to
> any x86-64 CPU.

## Hardware summary

```
AMD Ryzen 5 5600X (Vermeer)
├─ Family: 0x19 (Zen 3)
├─ Model: 0x21 (Vermeer, B0 stepping)
├─ Cores: 6 (1 CCD)
├─ Threads: 12 (2 per core, SMT)
├─ L1d: 32 KB / core, 8-way, 64 B lines
├─ L1i: 32 KB / core, 8-way, 64 B lines
├─ L2: 512 KB / core, 8-way, 64 B lines
├─ L3: 32 MB / CCD, 16-way, 64 B lines (shared)
├─ TSC: 3.7 GHz base, ~4.65 GHz boost (PBO)
├─ AVX2, BMI1/2, FMA, AES-NI, SHA-NI
├─ APIC ID encoding: bits[1:0]=thread, bits[3:2]=core, bits[5:4]=CCD
└─ Caches: WriteBack (default); VRAM as WriteCombining via MTRR
```

## What Ring 0 detects

### `platform::cpu::detect()` (new in v1.7.7)

Returns a `CpuIdentity` struct with:

| Field | Value on 5600X | Source |
|---|---|---|
| `vendor` | `Vendor::AMD` | CPUID 0x00 |
| `brand` | "AMD Ryzen 5 5600X 6-Core Processor ..." | CPUID 0x8000_0002-4 |
| `family` | 0x19 | CPUID 0x01 EAX |
| `model` | 0x21 | CPUID 0x01 EAX |
| `stepping` | 0x0 or 0x1 | CPUID 0x01 EAX |
| `microarch` | `Microarch::Zen3` | inferred from family/model |
| `features` | `FeatureBitmap` (see below) | CPUID 0x01, 0x07, 0x8000_0001 |
| `cache` | `CacheInfo` (see below) | CPUID 0x8000_0005, 0x8000_0006 |
| `virt_addr_bits` | 48 | CPUID 0x8000_0008 |
| `phys_addr_bits` | 40 | CPUID 0x8000_0008 |

### FeatureBitmap (selected, on 5600X)

```rust
CpuFeatures {
    sse: true, sse2: true, sse3: true, sse4_1: true, sse4_2: true,
    avx: true, avx2: true,         // 256-bit SIMD
    fma: true,                    // fused multiply-add
    bmi1: true, bmi2: true,       // bit manipulation
    aes_ni: true,                 // hardware AES
    sha_ni: true,                 // hardware SHA-256
    pcid: true, invpcid: true,    // process-context ID
    fsgsbase: true,               // rd/wr GS base
    smep: true, smap: true,       // supervisor mode protections
    xsave: true, osxsave: true,   // extended state
    rdtscp: true,                 // serializing RDTSC
    sys_call_sysret: true,        // AMD SYSCALL
    nx: true,                     // no-execute
    pages_1gb: true,              // 1 GiB huge pages
    lm: true,                     // long mode (64-bit)
    // ... many more
}
```

### CacheInfo (on 5600X)

```rust
CacheInfo {
    l1d_size_kb: 32, l1d_assoc: 8, l1d_line_bytes: 64,
    l1i_size_kb: 32, l1i_assoc: 8, l1i_line_bytes: 64,
    l2_size_kb: 512, l2_assoc: 8, l2_line_bytes: 64,
    l3_size_kb: 32768, l3_assoc: 16, l3_line_bytes: 64,
}
```

## What Ring 0 detects about the topology

### `arch::topology::detect()` (new in v1.7.7)

Returns a `Topology` struct:

```rust
Topology {
    total_threads: 12,           // 6 cores × 2 threads
    core_count: 6,
    ccd_count: 1,                 // single CCD
    threads_per_core: 2,          // SMT enabled
    cores_per_ccd: 6,
    bsp: CpuId { apic_id: 0, thread: 0, core: 0, ccd: 0 },
    apic_ids: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, ...],
}
```

### CpuId decomposition

For each APIC ID, `CpuId::from_apic(apic_id)` gives:

```rust
let id = CpuId::from_apic(7);  // e.g. core 1, thread 1
assert_eq!(id.thread, 1);
assert_eq!(id.core, 1);
assert_eq!(id.ccd, 0);
```

`is_smt_sibling_of(other)` returns true if two threads share an L1/L2.

## Per-CPU data (new in v1.7.7)

Each thread has its own 64-byte data area, accessed via `IA32_GS_BASE`.

```rust
#[repr(C)]
pub struct PerCpu {
    pub magic: u32,                   // 0xBEEF_C0DE if valid
    pub apic_id: u32,                 // which thread this is
    pub kernel_stack_top: AtomicU64,  // top of this thread's kernel stack
    pub online: AtomicBool,           // is this thread running?
    pub running: AtomicBool,          // is this thread currently executing?
    pub idle_task: AtomicU64,         // idle task pointer
}
```

The kernel accesses the current thread's per-CPU data via `swapgs`:

```rust
// On syscall entry (ring 3 → ring 0):
swapgs;
let percpu = arch::topology::current().unwrap();  // reads gs:0
let stack_top = percpu.kernel_stack_top.load(Ordering::SeqCst);
// ... now we can use this thread's kernel stack

// On syscall return (ring 0 → ring 3):
swapgs;
sysretq;
```

## Boot sequence (v1.7.7)

The boot phases use the new APIs:

1. **Phase 0 (arch)**: `cpu::init()` → `platform::cpu::detect()` → log
   vendor, family, microarch, features, cache. Detects BSP APIC ID.
2. **Phase 1 (mem)**: heap + page allocator.
3. **Phase 2 (dev)**: PCI ECAM, framebuffer, watchdog, audio.
4. **Phase 3 (proc)**: scheduler init.
5. **Phase 4 (bmo)**: BMO Core init.
6. **Phase 5 (user)**: Ring 3 first process (skipped on 5600X for now).

## Serial output on first boot (5600X)

```
[platform] CPU: AMD Ryzen 5 5600X 6-Core Processor
[platform] Family 0x19, Model 0x21, Stepping 0x0
[platform] Microarch: Zen3
[platform] Features: SSE, SSE2, SSE3, SSE4.1, SSE4.2, AVX, AVX2,
                   FMA, AES-NI, SHA-NI, BMI1, BMI2, RDTSCP, PCID,
                   FSGSBASE, SMEP, SMAP, XSAVE, NX, 1GB_PAGES
[platform] Cache: L1d=32K, L1i=32K, L2=512K, L3=32768K
[platform] Addr: virtual=48, physical=40
[arch] BSP APIC ID: 0
[arch] Topology: 12 threads, 6 cores, 1 CCD, 2 threads/core
```

## What's NOT yet supported (planned for v1.7.8+)

- **SMP bring-up of all 12 threads.** Today, only the BSP runs.
  The `arch::smp::init` function sends INIT-SIPI-SIPI but the
  trampoline doesn't yet wire up per-CPU data. Adding this is ~300
  LOC (the trampoline + AP startup handshake).
- **IOMMU / AMD-Vi.** For DMA-safe GPU drivers. The Ryzen 5 5600X
  has it but Ring 0 doesn't yet parse the IVRS table.
- **P-states and boost.** CPU runs at base 3.7 GHz. ACPI `_PSS` and
  CPB not yet used.
- **Thermal monitoring.** `k10temp` MSRs not yet polled.
- **Per-CPU data on APs.** PerCpu slots are initialized for the BSP
  but APs get zero-initialized slots; they need a trampoline that
  calls `init_for_apic`.

## How to use the new APIs

```rust
use crate::platform;
use crate::arch::topology;

// Detect CPU at boot
let id = platform::cpu::detect();
crate::log::KERNEL.info_u64("CPU family", id.family as u64);
crate::log::KERNEL.info_u64("CPU model",  id.model as u64);

// Detect topology
let topo = arch::topology::detect();
crate::log::KERNEL.info_u64("Total threads", topo.total_threads as u64);

// Per-CPU init (BSP at boot)
let bsp_stack_top: u64 = /* ... */;
arch::topology::init_for_apic(topo.bsp.apic_id, bsp_stack_top);
arch::topology::set_gs_base_for_apic(topo.bsp.apic_id);

// In a syscall handler, get the current thread's per-CPU data:
swapgs;
let percpu = arch::topology::current().unwrap();
let stack_top = percpu.kernel_stack_top.load(Ordering::SeqCst);
```

## References

- AMD64 Architecture Programmer's Manual, Vol. 3, §3.3 (CPUID)
- AMD PPR for Family 19h, Model 21h (Vermeer B0)
  https://www.amd.com/system/files/TechDocs/55922.pdf
- AMD SVM (Secure Virtual Machine) Architecture
- Intel SDM, Vol. 3, Chapter 3 (also useful for shared features)
