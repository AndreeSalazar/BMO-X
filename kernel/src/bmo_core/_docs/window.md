# Windowing API (`bmo_core::window`, syscalls 0x100..0x1FF)

> API de windowing expuesta a Ring 3 via 256 syscalls (0x100..0x1FF).
> Sigue el modelo de "windows como objetos opacos" similar a Win32,
> pero con IDs de 32 bits en vez de handles.

## Tabla de syscalls

| Rango | Categoría | Total |
|---|---|---|
| 0x100..0x11F | Window lifecycle (create/destroy/show/hide/move/resize) | 32 |
| 0x120..0x13F | Drawing (pixel/rect/line/circle/text/blit) | 32 |
| 0x140..0x15F | Input poll (keyboard/mouse/timeout) | 32 |
| 0x160..0x17F | File operations (open/read/write/close/seek/stat) | 32 |
| 0x180..0x19F | Process control (spawn/exit/yield/kill/wait) | 32 |
| 0x1A0..0x1BF | Memory (mmap/munmap/brk/mprotect) | 32 |
| 0x1C0..0x1DF | IPC (send/recv/peek) | 32 |
| 0x1E0..0x1FF | Time + audio + misc | 32 |

## Window lifecycle (0x100..0x11F)

### 0x100 `window_create(title_ptr, title_len, x, y, w, h) -> u32`
Crea una nueva ventana. Devuelve window_id (32 bits, único).
`title_ptr` apunta a memoria user-space (validada antes).

### 0x101 `window_destroy(window_id) -> i32`
Destruye la ventana. Devuelve 0 si ok, -1 si no existe.

### 0x102 `window_show(window_id) -> i32`
Hace visible la ventana. Devuelve 0 si ok.

### 0x103 `window_hide(window_id) -> i32`
Oculta la ventana.

### 0x104 `window_move(window_id, x, y) -> i32`
Mueve la ventana a (x, y).

### 0x105 `window_resize(window_id, w, h) -> i32`
Redimensiona la ventana a (w, h).

### 0x106 `window_set_title(window_id, title_ptr, title_len) -> i32`
Cambia el título.

### 0x107 `window_get_pos(window_id) -> u64`
Devuelve x|y en u32 packed.

### 0x108 `window_get_size(window_id) -> u64`
Devuelve w|h en u32 packed.

## Drawing (0x120..0x13F)

### 0x120 `draw_pixel(window_id, x, y, color) -> i32`
Dibuja un pixel. color = 0x00RR_GGBB.

### 0x121 `draw_rect(window_id, x, y, w, h, color) -> i32`
Dibuja un rectángulo relleno.

### 0x122 `draw_line(window_id, x1, y1, x2, y2, color) -> i32`
Dibuja una línea (Bresenham).

### 0x123 `draw_circle(window_id, cx, cy, r, color) -> i32`
Dibuja un círculo (midpoint algorithm).

### 0x124 `draw_text(window_id, x, y, str_ptr, len, color) -> i32`
Dibuja texto con la font por defecto (8x16).

### 0x125 `draw_blit(window_id, x, y, w, h, pixels_ptr) -> i32`
Dibuja un bitmap ARGB 32-bit.

### 0x126 `draw_clear(window_id, color) -> i32`
Llena la ventana de un color.

### 0x127 `draw_present(window_id) -> i32`
Hace flush del backbuffer al framebuffer (doble buffering).

## Input (0x140..0x15F)

### 0x140 `input_poll(timeout_ms) -> u64`
Devuelve el próximo evento de input. Bits 0-7: tipo (1=key,
2=mouse, 3=resize, 4=close). Bits 8-31: data.

### 0x141 `keyboard_state() -> u64`
Devuelve bitmap de teclas presionadas (256 bits en 4 u64s).
Actualmente devuelve los primeros 64 bits en un u64.

### 0x142 `mouse_state() -> u64`
Devuelve x|y|buttons packed.

## File operations (0x160..0x17F)

### 0x160 `file_open(path_ptr, path_len, flags) -> u32`
Abre un archivo. flags = 0 (R), 1 (W), 2 (RW). Devuelve fd.

### 0x161 `file_read(fd, buf_ptr, len) -> i64`
Lee hasta `len` bytes del fd. Devuelve bytes leídos (i64, -1 error).

### 0x162 `file_write(fd, buf_ptr, len) -> i64`
Escribe hasta `len` bytes. Devuelve bytes escritos.

### 0x163 `file_close(fd) -> i32`
Cierra el fd.

### 0x164 `file_seek(fd, offset, whence) -> i64`
Mueve el cursor. whence = 0 (set), 1 (cur), 2 (end).

### 0x165 `file_stat(path_ptr, path_len) -> u64`
Devuelve size|ino packed.

## Process (0x180..0x19F)

### 0x180 `proc_spawn(path_ptr, path_len, argv_ptr, argv_count) -> u32`
Spawnea un proceso. Devuelve pid.

### 0x181 `proc_exit(code) -> !`
Termina el proceso actual. Nunca retorna.

### 0x182 `proc_yield() -> i32`
Cede el CPU al scheduler.

### 0x183 `proc_kill(pid) -> i32`
Mata un proceso.

### 0x184 `proc_wait(pid) -> u32`
Espera a un hijo. Devuelve exit code.

## Memory (0x1A0..0x1BF)

### 0x1A0 `mem_mmap(addr, len, prot) -> u64`
Mapea `len` bytes. prot = R=1, W=2, X=4. Devuelve addr.

### 0x1A1 `mem_munmap(addr, len) -> i32`
Desmapea.

### 0x1A2 `mem_brk(new_brk) -> u64`
Mueve el program break.

### 0x1A3 `mem_mprotect(addr, len, prot) -> i32`
Cambia la protección.

## IPC (0x1C0..0x1DF)

### 0x1C0 `ipc_send(pid, msg_ptr, msg_len) -> i32`
Envía un mensaje a otro proceso.

### 0x1C1 `ipc_recv(pid, buf_ptr, buf_len) -> i64`
Recibe un mensaje. Devuelve bytes recibidos.

### 0x1C2 `ipc_peek(pid) -> u64`
Devuelve size|sender packed sin consumir.

## Time + audio (0x1E0..0x1FF)

### 0x1E0 `time_now_ms() -> u64`
Devuelve los ms desde boot.

### 0x1E1 `time_sleep_ms(ms) -> i32`
Duerme el proceso actual.

### 0x1F0 `audio_play(samples_ptr, count) -> i32`
Reproduce `count` samples (u32) en stereo 48 KHz.

### 0x1F1 `audio_set_volume(vol) -> i32`
vol = 0..100.

## Cómo añadir un syscall nuevo

1. Implementar en `bmo_core/api/<categoría>.rs`:
   ```rust
   pub fn my_new_syscall(args: &[u64; 6]) -> u64 {
       let arg0 = args[0];
       let arg1 = args[1];
       // ...
   }
   ```
2. Agregar al dispatch en `bmo_core/api/mod.rs`:
   ```rust
   pub fn dispatch(nr: u32, args: &[u64; 6]) -> u64 {
       match nr {
           ...
           0x1F2 => my_new_syscall(args),
           _ => 0xFFFF_FFFF,
       }
   }
   ```
3. Documentar aquí con firma, return, y comportamiento.

## Validación

Todos los syscalls validan:

- `window_id` está en rango válido (no desbordamiento).
- Los punteros user-space están en [0x10000, 0x7FFF_FFFF_FFFF].
- Los punteros están page-aligned cuando se requiera.
- Los `len` no son 0 ni mayores a un límite (default 1 MB).
- El proceso actual tiene permiso para la syscall (capability check).

Si la validación falla, el syscall devuelve `0xFFFF_FFFF` (-1) y
loguea el error en `diag::log`.
