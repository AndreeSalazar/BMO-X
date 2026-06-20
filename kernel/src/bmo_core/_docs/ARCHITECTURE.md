# BMO Core — Architecture

> Detalle de las capas internas de BMO Core, BFS layout, y el
> modelo de BMOASM JIT.

## Capas internas

```
┌─────────────────────────────────────────────────────────┐
│  api/ — 256 syscalls (interfaz con Ring 3)              │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────┴────────────────────────────────┐
│  window/ — Windowing system                             │
│  bmo/    — BMO interpreter                              │
│  fs/     — BFS filesystem                                │
│  bmoasm/ — BMOASM emitter + JIT                          │
│  nexolang/ — Nexolang compiler                           │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────┴────────────────────────────────┐
│  schedule/ — Scheduler (round-robin con prioridades)    │
│  event/    — Event loop                                  │
│  task/     — Task/thread management                      │
│  ipc/      — Message passing                             │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────┴────────────────────────────────┐
│  graph/  — 2D primitives (line, rect, blit)             │
│  text/   — Text rendering                                │
│  audio/  — Audio mixer                                   │
│  input/  — Keyboard + mouse                              │
│  time/   — Time API (TSC, RTC)                           │
│  heap/   — User-space heap                               │
│  stack/  — User-space stack                              │
│  stdio/  — STDIN/STDOUT/STDERR                           │
└─────────────────────────────────────────────────────────┘
```

## BFS — BFS Filesystem

Layout del BFS (versión 0.3, v1.7.4):

```
Block 0:    Superblock (4096 bytes)
  magic:    0xBFBF_BFBF
  version:  0x0003_0000
  block_sz: 4096
  total_bl: N
  free_bl:  M
  root_ino: 1
  ino_cnt:  K

Block 1..B0: Block bitmap (1 bit por bloque)
B0..B1:     Inode bitmap
B1..B1+I:   Inode table (128 bytes por inode, packed)
B1+I..:     Data blocks
  - File:   array de block pointers
  - Dir:    entries {ino, name_len, name} (256 bytes por entry)
  - Symlink: target string (max 200 bytes)
```

Inode (128 bytes):

```
magic:        0xB1B1_B1B1
mode:         u32 (regular/dir/symlink)
size:         u64
uid/gid:      u16/u16
atime/mtime:  u64/u64
block_count:  u32
direct[8]:    [u64; 8]  (32 KB directos)
indirect:     u64      (1 ptr)
double_indir: u64      (1 ptr)
reserved:     [u8; 30]
```

Direcciones:

- 1 inodo = 128 bytes.
- 1 bloque de 4 KB = 32 inodos.
- 1 dir entry = 256 bytes = 16 entries/bloque.

## BMOASM Model

BMOASM es el JIT de FastOS. Compila BMO bytecode a x86_64 nativo.

Bytecode:

```
0x00 NOP
0x01 PUSH imm64
0x02 POP r
0x03 ADD r1, r2
0x04 SUB r1, r2
0x05 MUL r1, r2
0x06 DIV r1, r2
0x07 AND r1, r2
0x08 OR  r1, r2
0x09 XOR r1, r2
0x0A SHL r1, imm
0x0B SHR r1, imm
0x0C MOV r1, r2
0x0D JMP imm
0x0E JZ  imm
0x0F JNZ imm
0x10 CALL imm
0x11 RET
0x20 SYSCALL nr
0x21 SYSCALL6 nr, a, b, c, d, e, f
0xFF HALT
```

Registros: r0..r15. Los registros r0..r3 son caller-saved (rdi,
rsi, rdx, rcx); r4..r7 son arg registers para syscall6
(r10, r8, r9); r8..r15 son callee-saved.

JIT:

1. Decodifica el bytecode en una lista de instrucciones.
2. Asigna registros (linear scan, v1.7.4).
3. Emite x86_64 nativo con re-alojación.
4. Marca el código generado como `RX` en `vmm::map_region`.
5. Llama al código generado.

## Nexolang

Nexolang es el lenguaje de scripting de FastOS. Se compila a BMO
bytecode.

Spec v0.1 (v1.7.4):

```nexo
fn main() {
    let x = 42;
    let y = x * 2;
    print("y =", y);
}
```

Compilación:

```
lex  → tokens
parse → AST
emit  → BMO bytecode
```

AST: Function, Var, BinaryOp, UnaryOp, Call, If, While, Return, Assign.

## Scheduler

Round-robin con prioridades. v1.7.4:

- 8 niveles de prioridad (0 = lowest, 7 = highest).
- Quantum = 10 ms.
- Tasks en cola circular por nivel.
- Idle task: priority 0, corre cuando no hay nadie más.

Preempción: timer interrupt (APIC, vector 32) llama a
`sched::tick` que decrementa el quantum y hace context switch
si es necesario.

## Interfaz con Ring 0

BMO Core llama a Ring 0 a través de las 4 APIs:

```rust
use crate::cpu::tsc;
use crate::device::serial;

tsc::busy_wait_ms(10);
serial::serial_write("y = 42\n");
```

Y también para alocación de páginas user-space:

```rust
use crate::memory::{page_alloc, paging};
let frame = page_alloc::alloc_frame().unwrap();
paging::map_page(user_virt, frame, PageFlags::USER_RW);
```

Ver `ring0/_docs/` para más detalles.
