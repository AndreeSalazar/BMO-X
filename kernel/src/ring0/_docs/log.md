# Per-Subsystem Logger

> Each driver / bus / kernel subsystem creates a `Logger` with its name
> and uses it for all messages. Cheap to construct (zero-sized, no
> allocation). Replaces the global `boot::log::info(phase, msg)` style.

## Import

```rust
use crate::log::Logger;
```

## Declaring a logger

```rust
static LOG: Logger = Logger::new("amdgpu");
```

The name should be a short, lowercase identifier. It appears in the
serial output and in the BMO API's diag buffer.

## Logging

```rust
LOG.info("probing PCI device 1002:73bf");
LOG.warn("BAR0 size mismatch; using 16 MiB default");
LOG.error("GPU reset failed");
LOG.error_u64("MMIO timeout at register", 0x1234);
LOG.info_u64("VRAM size", vram_bytes);
LOG.debug("writing ring buffer entry");
LOG.fatal("hardware fault; halting");  // never returns
```

## Sinks

Every message goes to:

1. **BMO Core's diag buffer** — for `dmesg` and post-boot inspection.
2. **COM1 serial** — for hardware capture during early boot.
3. **GOP framebuffer overlay** — only if the visual overlay is active
   (i.e. the desktop is not up yet). After the desktop is up, only
   serial + diag are kept.

## Levels

| Method | Severity | Typical use |
|---|---|---|
| `debug` | low-volume | trace events, register values |
| `info` | normal | state transitions, successful operations |
| `warn` | recoverable | fallbacks taken, missing optional data |
| `error` | operation failed | probe failure, MMIO timeout |
| `fatal` | unrecoverable | halts the CPU |

In v1.7.5, `debug` is aliased to `info`. Per-level filtering will be
added in v1.8.

## Built-in loggers

```rust
use crate::log::KERNEL;  // generic "kernel" logger for boot phases
KERNEL.info("phase 0 starting");
```

## Compat with old `boot::log::*`

The old API (`boot::log::info(phase, msg)`) is still available via
`crate::log::compat::info(phase, msg)`. Existing callsites work
unchanged; new code should use `Logger` directly.

## Per-driver message volume

A driver should not log more than ~10 messages per second. Higher
volumes:

1. Are hard to read in the serial output.
2. Flood the diag buffer (currently 8 KB).
3. Make the visual overlay flicker.

If a driver needs more verbose logging, gate it behind a `static
DEBUG: AtomicBool` and expose a `set_debug(true)` function for
post-boot debugging.

## Anti-patterns

```rust
// ❌ Don't: log in a tight loop
for _ in 0..1000 {
    LOG.info("loop iteration");  // floods the log
}

// ❌ Don't: include huge values
LOG.info_u64("VRAM", vram);  // 12 digits, eats 12 chars of framebuffer
// Prefer LOG.info(&format!("VRAM = {} MiB", vram / (1024*1024)));

// ❌ Don't: use a different subsystem name on each call
LOG.info("amdgpu1");
LOG.info("amdgpu_init");
LOG.info("amdgpu-probe");  // impossible to filter consistently
```
