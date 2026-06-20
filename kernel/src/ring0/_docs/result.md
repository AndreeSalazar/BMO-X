# Error Type: `KError`

> Stable, kernel-wide error type used by every Ring 0 API. Maps
> directly to BMO API v2 errno (negative values) so user-space can
> interpret errors without needing a separate `last_error` field.

## Import

```rust
use crate::result::{KError, KResult};
```

## Variants

| Variant | errno | When to use |
|---|---|---|
| `OutOfMemory` | -12 | physical or virtual memory exhausted |
| `InvalidArgument` | -22 | argument outside the valid range |
| `Timeout` | -110 | operation took too long; hardware did not respond |
| `Io` | -5 | bus error, device not ready, generic I/O failure |
| `NotSupported` | -95 | the driver / API does not implement this op |
| `AlreadyInUse` | -16 | resource (IRQ, BAR, port) is already taken |
| `NotFound` | -2 | the handle / device / resource does not exist |
| `Again` | -11 | try again later (mutex held, transient condition) |
| `HardwareFault` | -71 | hardware is in a bad state; needs a reset |
| `Other` | -1 | catch-all; prefer a specific variant |

## Helpers

```rust
pub fn ok_or_io(cond: bool) -> KResult<()>;
pub fn some_or_notfound<T>(opt: Option<T>) -> KResult<T>;
pub fn ok_or_errno(rc: u64) -> KResult<()>;
```

## Why this type, not `anyhow::Error` or `core::fmt::Error`

1. **Stable size**: `KError` is `Copy + 1 byte` (well, a `u32`-ish enum).
   No allocation, no `dyn`, no panic on construction. Works in `no_std`
   without `alloc`.
2. **Stable across ABI**: every variant is a fixed integer; adding a
   variant is a breaking change to the BMO API. This forces us to
   think before adding noise.
3. **Maps to BMO API errno**: drivers can `return Err(KError::Timeout)`
   and the BMO API can pass `KError::Timeout.errno()` directly to
   user-space as `-110`.
4. **No string formatting in the hot path**: the kernel log can format
   `KError::as_str()` for humans, but the error path itself is allocation-free.

## Pattern: driver init returns `KResult<()>`

```rust
pub fn init() -> KResult<()> {
    let bar0 = MmioRegion::map(pci.read_bar(0)?, 0x1000, CacheType::Uncacheable)?;
    bar0.write_u32(RESET_REG, 1);
    if !bar0.poll_u32(STATUS_REG, READY_BIT, READY_BIT, 1_000)? {
        return Err(KError::Timeout);
    }
    Ok(())
}
```

## Anti-patterns

```rust
// ❌ Don't: panic in driver init
let bar0 = MmioRegion::map(...).unwrap();

// ❌ Don't: return u64 (0 = success) like Linux legacy
pub fn init() -> u64 { 0 }

// ❌ Don't: return a string error
pub fn init() -> Result<(), &'static str> { Err("oops") }
```

The first hides bugs; the second is what we are moving away from; the
third is what BMO API v2.0 does (errno) and what KError exists to replace.
