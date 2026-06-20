# Scheduler (`bmo_core::schedule`)

> Scheduler round-robin con prioridades. Quantum = 10 ms.
> Preempción via timer APIC (vector 32).

## Estructura

```
schedule/
├── mod.rs       — init() + yield() + schedule() + tick()
├── queue.rs     — Cola de listos por prioridad
├── task.rs      — Task struct + estados
└── context.rs   — Context switch (save/restore 15 GPRs)
```

## Estados de una task

```
    ┌────────┐ spawn()  ┌────────┐
    │  NEW   │─────────▶│ READY  │◀────────┐
    └────────┘          └────────┘         │
                            │              │ quantum expire
                            ▼              │
                       ┌────────┐          │
                       │ RUNNING│──────────┘
                       └────────┘
                            │
              ┌─────────────┼─────────────┐
              ▼             ▼             ▼
         ┌────────┐   ┌────────┐    ┌────────┐
         │ BLOCKED│   │ EXITED │    │ KILLED │
         └────────┘   └────────┘    └────────┘
```

## Task struct

```rust
pub struct Task {
    pub pid: u32,
    pub state: TaskState,
    pub priority: u8,        // 0..7
    pub quantum_used_ms: u32,
    pub context: [u64; 15],  // 15 GPRs
    pub stack_ptr: u64,
    pub page_table: u64,     // CR3
    pub parent: Option<u32>,
    pub exit_code: i32,
    pub name: [u8; 32],
}
```

## API

### `init()`
Inicializa el scheduler. Crea la `idle_task` (priority 0, no se
bloquea nunca, llama `hlt` en loop).

### `spawn(entry: u64, stack: u64, priority: u8) -> u32`
Crea una nueva task. Asigna un PID (32 bits, único). Devuelve PID.

### `exit(code: i32) -> !`
Termina la task actual. Marca el estado `EXITED` y salta a
`schedule()` que despacha la siguiente.

### `yield_current() -> ()`
Cede el CPU. Decrementa quantum, mueve a ready, dispatch a
la siguiente task.

### `block(reason: BlockReason) -> ()`
Bloquea la task actual (I/O wait, sleep, etc).

### `unblock(pid: u32) -> ()`
Desbloquea una task.

### `kill(pid: u32) -> ()`
Mata una task (cualquier estado excepto NEW).

### `wait(pid: u32) -> i32`
Espera a que un hijo termine. Bloquea hasta `pid` exit.
Devuelve el exit_code.

## Scheduling policy

- **8 colas** (priority 0..7).
- En cada tick, decrementa `quantum_used_ms` de la running task.
- Si llega a 0, mueve la running a la cola de su prioridad y
  hace `dispatch`.
- `dispatch` elige la siguiente task de la cola de mayor
  prioridad no vacía. Si todas vacías, idle_task.
- Round-robin: la cola es circular, cada dispatch toma el
  primero y lo pone al final.

## Context switch

- Save 15 GPRs en `Task.context` (RSP se guarda aparte en
  `Task.stack_ptr`).
- Load 15 GPRs de la nueva task.
- Load CR3 con el page table de la nueva task.
- `ret` a la nueva RIP (que está en `Task.context[0]`).

## Hook de timer (ring 0)

El timer APIC vector 32 llama a `interrupt::apic::timer_isr`,
que llama a `crate::bmo_core::schedule::tick()`.

`tick()`:

1. Incrementa `global_ticks`.
2. `current_task.quantum_used_ms += 1`.
3. Si `quantum_used_ms >= quantum_max`, llama a `yield_current()`.

## Stack layout (kernel stack per-task)

```
Dirección alta:
  +-------------------+
  |  return address   |  (push al entrar a schedule)
  |    (RIP new)      |
  +-------------------+
  |  saved RBP        |
  +-------------------+
  |  ...              |
  |  espacio de stack |
  |  para la task     |
  |  ...              |
Dirección baja:
```

16 KB por task. Reservados en `memory::stack_alloc`.

## Sincronización con Ring 0

El scheduler corre en Ring 0 (con interrupts deshabilitados
durante context switch). Las syscalls desde Ring 3 se
atienden en ring 0 y luego vuelven a BMO Core (también en
ring 0) para hacer el dispatch final al ring 3 de la nueva task.

## Métricas

`stats()` devuelve:

```rust
pub struct SchedulerStats {
    pub total_tasks: u32,
    pub running: u32,
    pub ready: u32,
    pub blocked: u32,
    pub context_switches: u64,
    pub idle_ticks: u64,
    pub total_ticks: u64,
}
```

## Limitaciones v1.7.4

- Sin multi-core (sólo BSP).
- Sin real-time guarantees.
- Sin CFS/BFS advanced features.
- Sin load balancing.
