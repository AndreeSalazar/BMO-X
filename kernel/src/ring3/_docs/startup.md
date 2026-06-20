# Ring 3 — Startup

> Cómo arranca el primer proceso de usuario (`init`).

## Flujo

```
coordinator::main
  ↓
ring3::init()
  ↓
ring3::loader::load("/init.elf")
  ↓
ring3::startup::run_init(task_id)
  ↓
schedule::dispatch(task_id)
  ↓
[primer context switch a ring 3]
  ↓
init corre en ring 3
```

## Detalle

### `ring3::init()`

```rust
pub fn init() {
    serial::serial_write("[ring3] loading /init.elf\n");
    let task_id = loader::load("/init.elf")
        .expect("init.elf must exist");
    startup::run_init(task_id);
}
```

### `ring3::startup::run_init(task_id)`

1. Set la task como current.
2. Llama `schedule::dispatch(task_id)`.

Esto hace el primer switch a ring 3. Una vez allí, init corre.

## `init.elf`

`init.elf` es un programa mínimo que:

1. Abre `/dev/console` (syscall `file_open`).
2. Lee `/etc/rc.cfg` (syscall `file_read`).
3. Por cada línea de `rc.cfg`:
   - Si empieza con `exec`, hace `proc_spawn`.
4. Loop infinito: `proc_yield` y `proc_wait` para hijos.

## Re-exec

Si init muere, el kernel panic (no hay respawn en v1.7.4).

## Verificación

Para verificar que el primer proceso arranca:

1. Compilar init.elf.
2. Cargarlo a BFS raíz.
3. Boot el kernel.
4. Ver en serial:
   ```
   [ring3] loading /init.elf
   [ring3] entry=0x10000 stack=0x00800000 pml4=0xFFFF_E000
   [ring3] dispatching to ring 3
   [init] opened /dev/console
   [init] read /etc/rc.cfg (123 bytes)
   [init] spawning console.elf
   [init] spawning desktop.elf
   [init] idle
   ```

## Limitaciones v1.7.4

- Sin `respawn` automático.
- Sin service manager.
- Sin init.d scripts (sólo `rc.cfg` line-based).
- Sin `poweroff`/`reboot` desde init (sólo desde el kernel directo).

## Debugging

Si init no arranca:

- Verificar que `/init.elf` existe en BFS.
- Verificar que el ELF es válido (magic, e_machine=x86_64).
- Verificar que el page table se creó (debería aparecer en
  el log de `setup_user_memory`).
- Verificar que el stack está mapeado (si no, page fault inmediato).
- Verificar que `schedule::dispatch` se llama.
