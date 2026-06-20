# Delay: `udelay`, `mdelay`, deadline

> Microsecond- and millisecond-resolution delays. Implemented via the
> calibrated TSC, so they are accurate to the same tolerance as the
> TSC itself (~10 ns on Ryzen 5 5600X).

## Import

```rust
use crate::cpu::delay;
```

## Busy-wait delays

```rust
// Wait 100 microseconds (good for MMIO timeouts)
delay::udelay(100);

// Wait 5 milliseconds (good for hardware "is it alive?" polls)
delay::mdelay(5);
```

These spin the CPU. Do not use them in the scheduler idle loop.

## Deadline-based polling

For hardware that signals completion via a status register, the
pattern is:

```rust
let deadline = delay::deadline_us_from_now(1_000);  // 1 ms
loop {
    if regs.read_u32(STATUS) & DONE == DONE {
        break;
    }
    if delay::deadline_elapsed(deadline) {
        return Err(KError::Timeout);
    }
    core::hint::spin_loop();
}
```

This is equivalent to `regs.poll_u32(STATUS, DONE, DONE, 1_000)?` but
gives you more control (you can do work between polls).

## Accuracy

The TSC ticks at the CPU's base frequency (3.7 GHz typical for
Ryzen 5 5600X). One microsecond is 3,700 ticks. The actual delay
is within ±1 tick, so the accuracy is roughly:

| Delay | Error |
|---|---|
| `udelay(1)` | ±270 ns (one tick) |
| `udelay(100)` | ±27 ns (one tick) |
| `mdelay(1)` | ±0.27 µs |
| `mdelay(10)` | ±27 ns |

The `cpu::tsc::calibrate()` function measures the actual TSC rate at
boot, so the delays are calibrated against the wall clock.

## Anti-patterns

```rust
// ❌ Don't: spin for a long time
delay::mdelay(1000);  // 1 second of 100% CPU

// ❌ Don't: rely on the fallback TSC rate
// If the TSC has not been calibrated, udelay/mdelay use a hardcoded
// 3.7 GHz rate. Always call tsc::calibrate() at boot.

// ❌ Don't: sleep in a SpinLock
let g = SPIN.lock();
delay::udelay(1000);  // blocks other cores for 1 ms
// ...
```
