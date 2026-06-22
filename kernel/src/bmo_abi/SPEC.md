# BMO ABI — Specification (v2.0.0)

The BMO ABI is the **single, language-agnostic interface** for all code
running on FastOS. It replaces the C ABI (cdecl / Win64 / SysV AMD64) and
its stdlib.

```
                 ┌──────────┐
   C source ───▶│ C Plugin │──┐
                 └──────────┘  │
                 ┌──────────┐  │     ┌───────────┐     ┌─────────┐
   C++ src ────▶│ C++      │──┼────▶│  BMO ABI  │────▶│  AOT    │──▶ x86-64
                 │ Plugin   │  │     │  0x100..  │     │ Compiler│     native
                 └──────────┘  │     │  0x1FF    │     └─────────┘
                 ┌──────────┐  │     └───────────┘
   Java src ───▶│ Java     │──┘           ▲
                 │ Plugin   │              │
                 └──────────┘              │
                 ┌──────────┐              │
   BMO src ────▶│ BMO      │──────────────┘
                 │ Native   │
                 └──────────┘
```

**No VM. No bytecode. No interpreter.** Every language compiles to
native x86-64 via the BMO AOT compiler, and every kernel call goes
through the BMO ABI syscalls (0x100..0x1FF).

---

## 1. Calling Convention

### Register usage (SysV AMD64 + 1 extra)

| Purpose       | Register |
|---------------|----------|
| Syscall #     | RAX      |
| Return value  | RAX      |
| Return value2 | RDX      |
| Arg 0         | RDI      |
| Arg 1         | RSI      |
| Arg 2         | RDX      |
| Arg 3         | R10      |
| Arg 4         | R8       |
| Arg 5         | R9       |

**Differences from SysV AMD64:**
- **7 GPRs** for args (SysV has 6). Extra arg 5 goes in R9.
- **64-byte stack alignment** (SysV: 16). Required for SIMD alignment.
- **256-byte red zone** below RSP (SysV: 128).
- **No shadow space** (Win64 has 32 bytes).

### Return value

Functions return a `BmoStatus` (16 bytes) in `RAX:RDX`:

```rust
#[repr(C)]
pub struct BmoStatus {
    pub code:  u32,   // 0 = OK; >0 = error
    pub flags: u32,   // PARTIAL, RETRY, TRUNCATED, etc.
    pub value: u64,   // handle, counter, or return value
}
```

This **replaces** three legacy patterns in one:
- Win32 `HRESULT` + `GetLastError()` (TLS-based, race-prone)
- POSIX `errno` (also TLS-based)
- C out-parameters for return values

---

## 2. Syscall Numbers (0x100..0x1FF)

All kernel services are syscalls in the range `0x100..0x1FF`. There are
**no other numbers** for kernel services. This range is hardwired in:

- `crate::bmo_core::bmo_api::dispatch_syscall` (the dispatcher)
- `crate::bmo_core::lang::bmo::abi` (the AOT compiler's name table)

### Categories

| Range            | Category        |
|------------------|-----------------|
| 0x100..0x10F     | Window manager  |
| 0x110..0x11F     | Drawing         |
| 0x120..0x12F     | Window painting |
| 0x130..0x13F     | Compositor      |
| 0x140..0x14F     | Filesystem      |
| 0x150..0x15F     | Time            |
| 0x160..0x16F     | Input           |
| 0x170..0x17F     | Audio           |
| 0x180..0x18F     | Process/thread  |
| 0x190..0x19F     | Memory          |
| 0x1A0..0x1AF     | IPC             |
| 0x1F0..0x1FF     | Diagnostics     |

For the complete list, see `crate::bmo_core::lang::bmo::abi`.

### Calling a syscall from BMO source

Any function name in the BMO ABI table can be called as if it were a
local function. The AOT compiler resolves the name and emits a real
`syscall` instruction:

```bmo
// BMO source code
let fd = fs_open("readme.txt", 0);     // → syscall 0x140
fs_write(fd, "hello\n".as_bytes(), 6);  // → syscall 0x142
fs_close(fd);                           // → syscall 0x141
let win = win_create("My Window", 100, 100, 800, 600); // → syscall 0x100
```

The AOT compiler does this lookup at compile time (see
`aot::NativeCompiler::compile_expr` for `Expr::Call`).

---

## 3. Type System

### Primitive types

All primitive types are prefixed `bx_` to avoid collisions with C/Rust
type names:

```rust
pub type bx_u8  = u8;
pub type bx_u16 = u16;
pub type bx_u32 = u32;
pub type bx_u64 = u64;
pub type bx_u128 = u128;
pub type bx_i8  = i8;
pub type bx_i16 = i16;
pub type bx_i32 = i32;
pub type bx_i64 = i64;
pub type bx_usize = u64;   // 64-bit on x86-64
pub type bx_isize = i64;
pub type bx_uptr  = u64;   // pointer-sized unsigned
pub type bx_iptr  = i64;
```

Floating point: `bx_f32`, `bx_f64`. Half-precision is library-only.

### Status

`BmoStatus` (16 bytes, returned in `RAX:RDX`):

```rust
pub struct BmoStatus {
    pub code:  bx_u32,  // 0 = OK
    pub flags: bx_u32,  // PARTIAL | RETRY | TRUNCATED | QUEUED | ...
    pub value: bx_u64,  // handle, count, or other return data
}
```

Error codes are stable integers, not enum variants, so adding new
codes doesn't break ABI compatibility.

### Handles

`BmoHandle` (64 bits) with **type tag** and **generation**:

```
  bit 63        : tag        (0 = resource, 1 = channel/queue)
  bits 62..56   : kind       (7 bits — 128 types)
  bits 55..40   : generation (16 bits — detects UAF)
  bits 39..0    : index      (40 bits — 1 trillion slots)
```

Generation invalidates use-after-free automatically. This is better
than Win32 `HANDLE` (no generation) and POSIX `int fd` (no type info).

### Strings

Strings are `(ptr, len)` pairs of UTF-8 bytes. No nul-terminator, no
locale. Replaces C `char*` and Win32 `LPCWSTR`.

```rust
pub struct BmoStr<'a> {
    pub ptr: *const u8,
    pub len: usize,
}
```

### Memory

- `BmoSlice<T>` — `(ptr, len)` of typed elements
- `BmoRange` — `[start, end)` of an index space
- `BmoAligned<T, N>` — alignment marker
- 64-byte alignment is the **default** for all kernel-allocated buffers

### Sync

- `BmoAtomicU32`, `BmoAtomicU64`, `BmoAtomicBool` — typed atomics
- `BmoSpinLock` — TTAS spinlock
- `BmoMutex` — futex-backed lock (planned)
- `BmoFutex` — futex primitive

---

## 4. Layout Conventions

| Type               | Size  | Alignment |
|--------------------|-------|-----------|
| `bx_u8`            | 1     | 1         |
| `bx_u16`           | 2     | 2         |
| `bx_u32`           | 4     | 4         |
| `bx_u64`           | 8     | 8         |
| `BmoStatus`        | 16    | 8         |
| `BmoHandle`        | 8     | 8         |
| `BmoFileHandle`    | 16    | 8         |
| `BmoStr`           | 16    | 8         |
| `BmoResult<T>`     | 16+T  | 8         |
| All BMO types      | 8-byte aligned (minimum) |
| Kernel buffers     | 64-byte aligned |

All types are `#[repr(C)]` and `#[repr(transparent)]` where possible.
No padding tricks, no niche optimization — FFI stability over
ergonomics.

---

## 5. Memory Model

FastOS is **single-address-space**, no NUMA, no swap. All allocations
go through the `mem_alloc` syscall (0x190).

```rust
let buf = mem_alloc(4096);  // 4 KB, 64-byte aligned
// ... use buf ...
mem_free(buf);
```

For larger allocations (>2 MB), use `mem_map` (0x192) which returns
hugepage-backed memory.

---

## 6. Calling Patterns

### Window creation
```rust
let win = win_create(
    "Title",        // title (BmoStr)
    100, 100,       // x, y
    800, 600,       // w, h
);
let status = win_show(win);
if status.is_err() { ... }
```

### File I/O
```rust
let fd = fs_open("data.bin", FS_READ);  // FS_READ = 0
let mut buf = [0u8; 256];
let n = fs_read(fd, &mut buf, buf.len());
fs_close(fd);
```

### Drawing
```rust
draw_clear(win, 0xFF1A2638);                          // bg color
draw_rect(win, 10, 10, 200, 100, 0xFFE2C044);         // rect
draw_text(win, 20, 20, "Hello", 0xFFE6F1F5);          // text
win_end_paint(win);
```

### Time
```rust
let t0 = time_now_ns();
do_work();
let elapsed = time_now_ns() - t0;
```

---

## 7. Error Handling

Every syscall returns `BmoStatus` in `RAX:RDX`. Check `is_err()`:

```rust
let s = fs_open(path, flags);
if s.is_err() {
    let code = s.code;
    diag_print("fs_open failed with code");
    diag_print_u64(code);
}
```

The BMO source-level compiler generates this pattern automatically
when you use `si` (if):

```bmo
si fs_open(path, 0).is_err() {
    diag_print("open failed");
}
```

### Common error codes

| Code | Name             | Meaning              |
|------|------------------|----------------------|
| 0    | OK               | Success              |
| 1    | OUT_OF_MEMORY    | Heap exhausted       |
| 2    | INVALID_ARGUMENT | Bad arg              |
| 7    | TIMEOUT          | Operation timed out  |
| 8    | IO_ERROR         | I/O failure          |
| 9    | PERMISSION_DENIED | No capability        |
| 11   | NOT_FOUND        | No such resource     |
| 12   | BAD_HANDLE       | Invalid handle       |

See `crate::bmo_core::bmo_abi::fundamentals::status::error::error_code`
for the full list.

---

## 8. Integration with Languages

A language "plugs in" by implementing `LanguageAdapter`:

```rust
pub trait LanguageAdapter: Send + Sync {
    fn language(&self) -> Language;
    fn extensions(&self) -> &[&'static str];
    fn compile_native(&self, source: &[u8]) -> Result<Vec<u8>, AdapterError>;
    fn can_compile(&self, source: &[u8]) -> bool;
    fn memory_model(&self) -> MemoryModel;
    fn gc_strategy(&self) -> GcStrategy;
    // ... default impls for the rest
}
```

The adapter is responsible for:
1. Parsing the language's source.
2. Translating to BMO AST (or directly to x86-64).
3. Ensuring **all kernel calls go through BMO ABI syscalls** (0x100..0x1FF).
4. Returning native x86-64 bytes.

The BMO AOT compiler (`crate::bmo_core::lang::bmo::aot`) handles the
final AOT step. It resolves function names in `Expr::Call` against
the BMO ABI table (`crate::bmo_core::lang::bmo::abi`) and emits
`syscall` instructions for matches.

---

## 9. File Format (BEF)

BMO ELF Files (BEF) are x86-64 ELFs with BMO-specific sections:

- `.bmoabi` — exported BMO ABI names and their syscall numbers
- `.bmoimports` — BMO ABI calls imported by this module
- `.bmoext` — BMO extensions (e.g. for graphics, audio)

The BEF loader (`crate::bmo_core::bef`) validates that all imported
syscall numbers are in `0x100..0x1FF` — **a BEF that calls any other
syscall number is rejected**.

---

## 10. Versioning

- `BMO_ABI_VERSION = (1, 0)` in `bmo_abi::mod`
- `BMO_ABI_MAGIC = 0x424D4F31` (`"BMO1"` in little-endian) in BEF header
- `BMO_RUNTIME_VERSION = 2` in `bmo_abi::runtime::mod`

Adding a new syscall is backward-compatible (old code keeps working).
Changing an existing syscall's signature is a **breaking change** —
bump the major version, BEF loader rejects old binaries.

---

## 11. Design Principles

1. **One ABI, one filter**: every language must produce calls to
   0x100..0x1FF. No exceptions.

2. **No global state in ABI**: status codes travel in registers.
   No `errno`, no `GetLastError`.

3. **Layouts are explicit**: every type is `#[repr(C)]` with
   documented size and alignment.

4. **No silent conversions**: all casts go through `From` impls.

5. **Handles carry their own type info**: `BmoHandle` has a kind
   tag, so passing a `File` where a `Texture` is expected is a
   type error caught at compile time.

6. **Strings are not nul-terminated**: `(ptr, len)` everywhere.

7. **No VM, no interpreter, no JIT**: everything is AOT to native
   x86-64. The BMO ABI is the only layer of indirection.

---

## 12. Quick Reference

```bmo
// fs_*
fs_open(path: BmoStr, flags: u32) -> BmoHandle
fs_close(handle: BmoHandle) -> BmoStatus
fs_read(handle: BmoHandle, buf: *mut u8, len: u64) -> u64
fs_write(handle: BmoHandle, buf: *const u8, len: u64) -> u64
fs_seek(handle: BmoHandle, offset: i64, mode: u32) -> u64
fs_stat(path: BmoStr) -> BmoFileInfo

// win_*
win_create(title: BmoStr, x: i32, y: i32, w: u32, h: u32) -> BmoHandle
win_show(handle: BmoHandle) -> BmoStatus
win_hide(handle: BmoHandle) -> BmoStatus
win_set_title(handle: BmoHandle, title: BmoStr) -> BmoStatus
win_set_bounds(handle: BmoHandle, x: i32, y: i32, w: u32, h: u32) -> BmoStatus
win_invalidate(handle: BmoHandle) -> BmoStatus
win_pump_events() -> BmoStatus

// draw_*
draw_clear(handle: BmoHandle, color: u32) -> BmoStatus
draw_rect(handle: BmoHandle, x: i32, y: i32, w: u32, h: u32, color: u32) -> BmoStatus
draw_text(handle: BmoHandle, x: i32, y: i32, text: BmoStr, color: u32) -> BmoStatus
draw_circle(handle: BmoHandle, cx: i32, cy: i32, r: u32, color: u32) -> BmoStatus
draw_line(handle: BmoHandle, x1: i32, y1: i32, x2: i32, y2: i32, color: u32) -> BmoStatus

// time_*
time_now_ns() -> u64
time_now_us() -> u64
time_sleep_ms(ms: u64) -> BmoStatus

// input_*
input_poll_key() -> i32          // -1 if no key
input_poll_mouse() -> MouseState
input_poll_event() -> InputEvent

// audio_*
audio_beep(freq_hz: u32, duration_ms: u32) -> BmoStatus
audio_play(handle: BmoHandle) -> BmoStatus
audio_stop(handle: BmoHandle) -> BmoStatus

// proc_*
proc_spawn(path: BmoStr) -> BmoHandle
proc_exit(code: i32) -> !
proc_get_pid() -> u32
proc_yield() -> BmoStatus

// mem_*
mem_alloc(size: u64) -> *mut u8
mem_free(ptr: *mut u8) -> BmoStatus
mem_map(size: u64) -> *mut u8
mem_unmap(ptr: *mut u8, size: u64) -> BmoStatus

// diag_*
diag_print(text: BmoStr) -> BmoStatus
diag_trace(event: BmoStr) -> BmoStatus
diag_assert(cond: bool, msg: BmoStr) -> BmoStatus
```

---

## 13. See Also

- `crate::bmo_core::bmo_abi` — the type definitions
- `crate::bmo_core::lang::bmo::abi` — the syscall number table
- `crate::bmo_core::lang::bmo::aot` — the AOT compiler
- `crate::bmo_core::bmo_api` — the syscall dispatcher
- `crate::bmo_core::bef` — the BEF loader (validates ABI compliance)
