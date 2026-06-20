# BMOASM — Bytecode + JIT Spec

> BMOASM es el lenguaje assembly-like que se compila a BMO bytecode,
> y luego se JIT-ea a x86_64 nativo en runtime.

## Sintaxis

```bmoasm
; Comentarios con ;
fn main:
    mov r0, 42        ; r0 = 42
    mov r1, 2
    mul r0, r1        ; r0 *= r1 (r0 = 84)
    push r0
    mov r8, 0x01      ; syscall 0x01 (print u32)
    mov rdi, r0
    syscall
    pop r0
    halt
```

## Mnemónicos

| Mnemónico | Opcode | Args | Descripción |
|---|---|---|---|
| NOP | 0x00 | — | No operation |
| PUSH | 0x01 | imm64 | Push imm64 a stack |
| POP | 0x02 | r | Pop a r |
| ADD | 0x03 | r1, r2 | r1 += r2 |
| SUB | 0x04 | r1, r2 | r1 -= r2 |
| MUL | 0x05 | r1, r2 | r1 *= r2 |
| DIV | 0x06 | r1, r2 | r1 /= r2 (zero check) |
| AND | 0x07 | r1, r2 | r1 &= r2 |
| OR | 0x08 | r1, r2 | r1 \|= r2 |
| XOR | 0x09 | r1, r2 | r1 ^= r2 |
| SHL | 0x0A | r1, imm8 | r1 <<= imm |
| SHR | 0x0B | r1, imm8 | r1 >>= imm |
| MOV | 0x0C | r1, r2 | r1 = r2 |
| MOVI | 0x0D | r, imm64 | r = imm |
| JMP | 0x0E | imm | jump to imm |
| JZ | 0x0F | imm | jump if ZF=1 |
| JNZ | 0x10 | imm | jump if ZF=0 |
| CALL | 0x11 | imm | call function |
| RET | 0x12 | — | return |
| SYSCALL | 0x20 | nr | syscall con 0 args |
| SYSCALL6 | 0x21 | nr, a, b, c, d, e, f | syscall con 6 args |
| LOAD | 0x30 | r, addr | r = *addr |
| STORE | 0x31 | addr, r | *addr = r |
| HALT | 0xFF | — | halt execution |

## Registros

- r0..r3: argumentos (caller-saved), mapean a rdi, rsi, rdx, rcx.
- r4: 4to arg (r10).
- r5, r6, r7: 5to-7mo arg (r8, r9, r15).
- r8..r15: locales (callee-saved).
- rsp: stack pointer (no se puede usar directamente).
- rbp: frame pointer (opcional).

## Bytecode binary format

```
Header (32 bytes):
  magic:    u32 = 0xB0A5_CAFE
  version:  u16 = 0x0001
  flags:    u16
  entry:    u32 (offset al primer opcode)
  text_sz:  u32
  data_sz:  u32
  reloc_sz: u32
  reserved: [u8; 4]

Code (.text):
  Array of { opcode: u8, operands: [...] }

Data (.data):
  Array of u8

Relocations:
  Array of { offset: u32, kind: u8, sym: u32 }
```

## JIT pipeline

1. **Decode**: parse bytecode a `Vec<Inst>`.
2. **Regalloc**: linear scan sobre las 16 regs virtuales.
3. **Spill**: si excede 16 regs, spill a stack.
4. **Emit**: traduce cada `Inst` a 1-3 instrucciones x86_64.
5. **Relocate**: parchea saltos y direcciones de memoria.
6. **Map**: marca el código generado como RX en `vmm`.
7. **Call**: salta al código generado.

## Calling convention

Caller-saved: r0..r7. Si se preservan, caller debe hacer `push`/`pop`.

Callee-saved: r8..r15. Si se modifican, callee debe hacer `push`/`pop`.

Stack: crece hacia abajo. 16-byte aligned al `call`.

## Ejemplo compilado

```bmoasm
fn add_forty_two:
    mov r0, rdi      ; r0 = arg
    mov r1, 42
    add r0, r1
    ret
```

Bytecode:

```
0x0C 0x00 0x07    ; MOV r0, r7 (rdi)
0x0D 0x01 0x2A 0x00 0x00 0x00 0x00 0x00 0x00 0x00  ; MOVI r1, 42
0x03 0x00 0x01    ; ADD r0, r1
0x12              ; RET
```

Total: 15 bytes.

## Tooling

- `bmoasm/emit.rs`: codifica BMOASM a bytecode.
- `bmoasm/decode.rs`: parsea bytecode a AST.
- `bmoasm/jit.rs`: compila bytecode a x86_64.
- `bmoasm/disasm.rs`: desensambla x86_64 a BMOASM (debug).

## Performance

En Ryzen 5 5600X, el JIT de BMOASM emite ~200 MB/s de código
nativo. Una función típica (10 instrucciones) tarda 50 µs en
compilar y 200 ns en ejecutar.

## Limitaciones v1.7.4

- Sin SIMD (AVX/SSE en BMO bytecode).
- Sin floating point.
- Sin threads.
- Sin exceptions.
- Sin debugger.
