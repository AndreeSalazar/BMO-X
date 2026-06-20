# Ring 3 — Architecture

> Detalle del memory map de un proceso Ring 3, la transición
> ring 0 ↔ ring 3, y la convención de syscalls.

## Memory map

```
0x0000_0000_0000 ──┐
                   │  ELF segments
                   │  ├─ .text     (RX)
                   │  ├─ .rodata   (R)
                   │  └─ .data     (RW)
                   │
                   │  BSS
                   │  heap (brk)
0x0040_0000 ───────┘
0x0040_0000 ──┐
              │  stack (8 MB, RW-, NX)
              │  crece hacia abajo
0x0080_0000 ──┘
0x0080_0000 ──┐
              │  mmap regions
              │  ├─ shm
              │  ├─ ventanas (en el futuro)
              │  └─ bibliotecas (futuro)
0xFFFF_FFFF ──┘
```

## Page tables

Cada proceso tiene un PML4 propio. El kernel mantiene:

- `kernel_pml4` (compartido): 0xFFFF_8000_0000_0000 en adelante.
- `process_pml4` (per-proc): todo el rango bajo.

En el context switch, se carga `process_pml4.CR3`. El kernel
también está mapeado en el `process_pml4` (kernel half copiado
del `kernel_pml4`) para que las syscalls puedan ejecutarse.

## Transición ring 0 → ring 3

### Setup (en `ring3::init`)

1. Cargar el ELF del primer proceso.
2. Crear un nuevo PML4 (`page_alloc` + `memset 0` + copiar
   entries 256..511 del kernel PML4).
3. Mapear las ELF segments en el PML4.
4. Mapear el stack (8 MB en 0x0040_0000).
5. Crear un Task con:
   - `context[0]` = entry point (RIP)
   - `stack_ptr` = 0x0080_0000 - 16 (top of stack, 16-byte aligned)
   - `page_table` = addr del nuevo PML4
   - `priority` = 5 (default)
6. Llamar a `schedule::spawn(task)`.

### Primer switch (en `schedule::dispatch`)

1. Save context de la task actual (la que estaba en ring 0).
2. Load context de la nueva task.
3. Load CR3 con el page table de la nueva task.
4. Cargar los registros de segmento:
   - CS = 0x33 (ring 3, 64-bit)
   - SS = 0x2B (ring 3, 32-bit)
5. `iretq` con:
   - RIP = entry
   - CS = 0x33
   - RFLAGS = 0x202 (interrupts enabled)
   - RSP = stack top
   - SS = 0x2B

Tras esto, el CPU está en ring 3 ejecutando el código de la app.

## Transición ring 3 → ring 0 (syscall)

La app hace:

```asm
mov rax, 0x100     ; syscall number
mov rdi, 0         ; arg0
mov rsi, 0         ; arg1
mov rdx, 0         ; arg2
mov r10, 0         ; arg3
mov r8, 0          ; arg4
mov r9, 0          ; arg5
syscall
; rax = return value
```

El CPU entra a ring 0 via `IA32_LSTAR` (configurado en
`interrupt::syscall::init`):

```asm
; En ring 0 entry:
swapgs
mov gs:[ring0_per_cpu_data], rsp    ; save user RSP
mov rsp, gs:[ring0_kernel_stack]    ; load kernel stack
push rcx                            ; user RIP
push r11                            ; user RFLAGS
push rbp
push rbx
push r12
push r13
push r14
push r15
; (rdi..r10, rax ya están en regs)
call bmo_api::dispatch(rax, rdi, rsi, rdx, r10, r8, r9)
; (rax = return value)
pop r15
pop r14
pop r13
pop r12
pop rbx
pop rbp
pop r11
pop rcx
mov rsp, gs:[ring0_per_cpu_data]    ; restore user RSP
swapgs
sysretq
```

## Validación en ring 0

Antes de ejecutar la syscall, el kernel valida:

- El syscall number está en 0x100..0x1FF (si no, kill -1).
- Los punteros en args están en user space [0x10000, 0x7FFF_FFFF_FFFF].
- Los `len` no son 0 ni mayores a 1 MB.
- El proceso actual tiene la capability para esa syscall
  (v1.8.0 introduce capabilities; v1.7.4 no las valida).

Si falla la validación, el proceso recibe SIGSYS y muere con
exit code 0xFF.

## Page fault en ring 3

Si la app accede a memoria no mapeada, el CPU genera #PF con
error code bit 2 = 1 (user mode). El handler:

1. Lee CR2 (dirección faulted).
2. Si está en el rango del heap → expand heap (brk).
3. Si está en mmap region → grow mmap.
4. Si no → kill con SIGSEGV (exit code 0xFE).

## Interrupciones en ring 3

El timer interrupt puede ocurrir durante la ejecución de la
app. El handler de timer está en ring 0; tras `schedule::tick()`,
puede haber context switch. La app no se entera; sólo ve que
sigue ejecutando después de un yield.

## Syscall 0x00..0xFF (legacy)

Estos NO se exponen a Ring 3. Son syscalls internos que BMO
Core usa para acceder a Ring 0. Ring 3 sólo ve 0x100..0x1FF.

Si Ring 3 intenta hacer syscall 0x00..0xFF, el kernel responde
con -1 (EPERM) y loguea el intento.

## Capacidades (v1.8.0)

v1.8.0 introduce capabilities para Ring 3:

- `CAP_WINDOW` — usar windowing API
- `CAP_DRAW` — usar drawing API
- `CAP_FILE_READ` — leer archivos
- `CAP_FILE_WRITE` — escribir archivos
- `CAP_NET` — usar red (futuro)
- `CAP_AUDIO` — usar audio
- `CAP_PROCESS_SPAWN` — spawn nuevos procesos

Por defecto, el primer proceso (init) tiene todas las caps.
Cada proceso hijo hereda las caps de su padre, y puede
revocar caps a sus hijos.

## Limitaciones v1.7.4

- Sin swap.
- Sin COW.
- Sin shared libs.
- Sin threads en Ring 3 (multiproceso sí).
- Sin signals (sólo exit codes).
- Sin capabilities (todo permitido).
