# Synchronization: SpinLock, IrqSpinLock, OnceCell

> Minimum synchronization primitives for Ring 0 drivers. Use only what
> you need; reach for heavier abstractions (sleeping locks, RCU) only
> when profiling shows contention.

## Import

```rust
use crate::sync::{SpinLock, IrqSpinLock, OnceCell};
```

## SpinLock

A simple spinlock. IRQ state is preserved across `lock()`. Use for
short critical sections (< 100 µs) where the data is not touched by
interrupt handlers.

```rust
static COUNT: SpinLock<u64> = SpinLock::new(0);

fn increment() {
    let mut c = COUNT.lock();
    *c += 1;
}
```

### Rules

- **Hold the lock for the shortest time possible.** No allocations,
  no printing, no sleeping.
- **Do not call back into the kernel while holding the lock** unless
  you have a documented lock order. (Lock ordering is per-driver in
  v1.7.5; v1.8.0 will add a global lock validator.)
- **Do not hold a `SpinLock` across an `udelay` longer than 10 µs.**
  Use `IrqSpinLock` with care, or restructure the code.

## IrqSpinLock

A spinlock that disables interrupts while held. Use when the
protected data is also touched from an interrupt handler.

```rust
static RX_BUF: IrqSpinLock<Vec<u8>> = IrqSpinLock::new(Vec::new());

fn irq_handler() {
    let mut buf = RX_BUF.lock();   // IRQs disabled here
    buf.push(byte_from_hw());
}
```

The IRQ state is restored on `Drop`. If IRQs were enabled when you
called `lock()`, they are re-enabled when the guard drops. If they
were disabled, they stay disabled (preserving the original state).

### Rules

- All other locks held at the call site must also be `IrqSpinLock`
  (you cannot hold a `SpinLock` and try to acquire an `IrqSpinLock`
  that another IRQ handler is also taking — deadlock).
- The lock is held for the absolute minimum time. The classic
  pattern is: read from hardware, push to a queue, set a flag, drop.

## OnceCell

One-shot initialization. The first call to `get_or_init` runs the
closure; subsequent calls return a reference to the cached value.

```rust
static FOO: OnceCell<Foo> = OnceCell::new();

fn use_foo() -> &'static Foo {
    FOO.get_or_init(|| {
        // expensive init, runs at most once
        Foo::new()
    })
}
```

### Caveat

The current implementation is single-core. Once SMP is enabled (v1.8),
the `get_or_init` will spin-loop until the first caller finishes, with
no progress indicator. This is fine for "init this on first use" but
not for fine-grained per-CPU state. For that, use
[`crate::proc::task::current_ptr`] or a per-CPU variable.

## When to use what

| Scenario | Use |
|---|---|
| Short critical section, no IRQ interaction | `SpinLock` |
| Critical section also touched by an IRQ handler | `IrqSpinLock` |
| One-time setup (allocate, register, etc) | `OnceCell` |
| Single-producer, single-consumer queue | `IrqSpinLock<Vec<T>>` (v1.7.5) or a lock-free queue (v1.8) |
| Per-CPU data | not yet supported; use a global `SpinLock` for v1.7.5 |

## Anti-patterns

```rust
// ❌ Holding a lock across I/O
let c = COUNT.lock();
bar0.read_u32(REG);  // blocks IRQs for 100 µs
// ...

// ❌ Allocating inside a lock
let c = COUNT.lock();
let v = vec![1, 2, 3];  // may call into the heap allocator

// ❌ Nested locks without documented order
let a = LOCK_A.lock();
let b = LOCK_B.lock();  // if some other path does B then A, deadlock
```
