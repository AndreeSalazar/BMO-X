# BMO API — Design Specification v2.0

> A deep, modular, Ring 3-capable windowing API for **FastOS/BMO** (a no_std
> Rust x86_64 UEFI bare-metal OS). This document is the spec — it is
> implementation guidance, not source code. Targets the gap between the
> existing Ring 0 `bmo_api` (window/manager/message primitives, max 16
> windows, single Ring 0 control thread) and a real userland windowing
> system capable of running independent Ring 3 GUI programs.

---

## 0. Goals and Non-Goals

### Goals
- A modular, no_std-friendly windowing API that any Ring 3 ELF/PE/ÑEXO
  program can link against.
- A clean, deterministic syscall ABI in the `0x100..0x1FF` range, separate
  from the existing `0x00..0xF0` FastOS syscalls.
- A Win32-style *window procedure* model — small, opt-in callbacks invoked
  by the kernel when a window has work to do.
- A bounded, allocation-free fast path: every public type is fixed-size,
  every queue is a power-of-two SPSC ring, every syscall has a worst-case
  bound.
- Server-side window management (Z-order, focus, drag/resize, snap,
  decorations) so user programs don't have to reimplement it.

### Non-Goals (v2.0)
- Multi-display hotplug, GPU acceleration, HiDPI scaling, Vulkan/GL
  integration. (These land in v3.x.)
- Full xdg-shell, full EGL, or full Wayland wire protocol compatibility.
  We use Wayland-inspired *roles* (`bmo_toplevel`, `bmo_popup`) but ship a
  private ABI.
- Pre-emptive multithreading per Ring 3 process. v2.0 supports *one* GUI
  thread per process; a process may spawn worker threads that do *not*
  hold windows.
- A POSIX-style `select`/`epoll` subsystem. (The window procedure *is* the
  event loop, à la Win32.)

---

## 1. Background — Survey of Existing APIs

### 1.1 Comparison Table

| Aspect                       | Win32 (USER32)                                    | X11 (Xlib)                                                | Wayland                                                | Cocoa (AppKit)                                  | Linux DRM/KMS + evdev                              | Linux Framebuffer                          |
|------------------------------|---------------------------------------------------|-----------------------------------------------------------|--------------------------------------------------------|-------------------------------------------------|----------------------------------------------------|--------------------------------------------|
| Topology                     | One kernel-side window manager (`win32k.sys`)      | Separate X server process; clients connect via socket     | Compositor is the display server; clients connect      | WindowServer process; clients use Mach IPC       | DRM master + libinput; or direct KMS              | Single process writes /dev/fb0            |
| Per-process connection       | Implicit (USER32 loaded into every GUI process)   | `Display*` (a TCP/UNIX socket handle)                     | `wl_display*` (UNIX socket)                            | Implicit (Mach ports)                           | `drmModeGetResources` (file descriptor)            | `mmap(/dev/fb0)`                           |
| Window identifier            | `HWND` (handle table index, opaque)               | `XID` (32-bit, server-assigned)                           | `wl_surface*` (proxy)                                  | `NSWindow*` (Objective-C object)                | `drmModeCrtc*` (plane ID, not really a window)    | n/a                                        |
| Event delivery               | Per-thread message queue + `WndProc(hwnd,msg,...)` | Per-connection event queue + `XNextEvent`                  | Per-`wl_event_queue` callbacks                          | Per-thread run loop + `NSEvent` + responder chain | Per-fd `poll()`; page-flip events via `drmEvent`   | None; libinput provides input             |
| Message ordering             | FIFO *except* `WM_PAINT`, `WM_TIMER`, `WM_QUIT` (low-priority); `WM_PAINT` coalesced | FIFO; `Expose` events coalesced into bounding box        | Strict request/response ordering; events serialized     | Run loop dispatches in mode order               | Kernel-defined                                      | n/a                                        |
| Painting model               | GDI: `BeginPaint` → draw → `EndPaint`; clipping to update region | Server-side clipping via `GC` and `SetClipRectangles`; no retained mode | Client attaches `wl_buffer` to surface; no paint events  | `drawRect:` on `NSView`; Core Animation composes | Client draws directly to dumb buffer; `drmModePageFlip` | Client writes to mmap'd fb                 |
| Decoration                   | Server-side (DWM in Vista+)                       | Server-side (window manager draws it)                     | **Client-side** (libdecor or toolkit draws it)         | Server-side (WindowServer)                       | n/a (no WM)                                        | n/a                                        |
| Double-buffering             | Implicit (DWM)                                    | Implicit (compositor)                                     | Explicit (`wl_buffer` swap)                             | Implicit (Core Animation)                        | Explicit (`drmModePageFlip`)                       | Manual (or vsync via wait-for-vblank ioctl) |
| Input delivery               | `WM_KEYDOWN` / `WM_MOUSEMOVE` to focused window    | `KeyPress` / `MotionNotify` based on event mask            | `wl_keyboard::key` / `wl_pointer::motion`              | `keyDown:` / `mouseMoved:` to first responder   | `libinput_event` structs (kernel-internal)         | `evdev` events read from `/dev/input/*`     |
| Threading model              | One GUI thread per process; window owned by that thread | Single-threaded; events from one Display*                 | Per-thread event queues possible                        | Main thread only; observers may be on bg threads | Single DRM master thread recommended                | n/a                                        |
| Window roles                 | Overlapped, popup, child, message-only, layered     | Same hierarchy in tree                                    | `wl_surface` + `xdg_toplevel` / `xdg_popup` / `wl_subsurface` | `NSWindow` (style mask) + `NSPanel` etc.      | DRM planes (primary, cursor, overlay)              | n/a                                        |
| Focus model                  | `SetFocus`; only one window has keyboard focus    | `SetInputFocus` (revert-to on focus loss)                  | `wl_keyboard::enter` on surface under pointer          | `makeFirstResponder:`                            | n/a                                                 | n/a                                        |
| Drag/resize                  | `WM_NCLBUTTONDOWN` → `DefWindowProc` enters modal loop | `ConfigureRequest` from WM                                | `xdg_toplevel::resize` / `::move` initiated by client | `windowWillResize:` delegate                    | n/a                                                 | n/a                                        |
| Reentrancy on `SendMessage`  | **Yes** (callers block until wnd_proc returns)    | No (Xlib is single-threaded by design)                    | No (re-entrancy guaranteed by design)                   | Yes (`performSelectorOnMainThread:` etc.)        | n/a                                                 | n/a                                        |
| Handle validation            | Server-side (`USER32` checks handle table)         | Server-side (XID + generation check)                       | n/a (proxy, dereferenced)                              | n/a (Objective-C refcount)                      | File descriptor; not validated                     | n/a                                        |
| Failure mode of bad handle   | Returns `NULL` / `ERROR_INVALID_HANDLE`           | `BadWindow` error                                          | Protocol error → fatal `wl_display` disconnect         | Exception                                       | `errno = EBADF`                                     | n/a                                        |

### 1.2 Key Design Takeaways (what BMO API v2 inherits)

1. **One message queue per thread, not per window** (Win32). Posting
   `WM_TIMER` to a window on another thread is a programmer error, and
   we want it to be detectable.
2. **A real wnd_proc callback, not just event polling** (Win32). This lets
   a Ring 3 program have a single, well-known function that the kernel
   calls on every event — no need to maintain a state machine in user code.
3. **Coalesce paint requests** (Win32 + X11). Multiple invalidations
   between paints produce a single `BMO_MSG_PAINT` with a bounding-box
   update region.
4. **Bounded, fixed-size message struct** (Win32 `MSG` is 40 bytes; BMO
   v2 is **32 bytes**).
5. **Generation counter on handles** (X11 XIDs, plus the `bmo_handle_t`
   format below). Prevents accidental reuse after destroy.
6. **Server-side decorations and a built-in window manager** (Win32 DWM
   model, *not* Wayland). Every Ring 3 program gets snap, Z-order,
   focus-follows-mouse, alt-tab, and a title bar for free.
7. **wl_buffer-style explicit double buffer** but mediated by the kernel:
   each window owns an offscreen `bmo_surface`; the user draws into it,
   then calls `bmo_flip` and the kernel composites it onto the
   framebuffer. (Hybrid Win32 + Wayland.)

### 1.3 What we explicitly do NOT copy

- **Wayland's client-side decoration burden.** Every existing Wayland
  compositor (sway, Hyprland, dwl, KWin) requires libdecor or a
  toolkit-side CSD implementation. We choose server-side for v2.0 to
  keep Ring 3 programs tiny.
- **X11's network transparency.** Adds nothing for a single-machine
  kernel and complicates handle validation.
- **Cocoa's Objective-C runtime.** We're on no_std Rust; the responder
  chain becomes a flat list of `wnd_proc` functions, one per window.
- **Win32's user/kernel split via `USER32.DLL` thunking.** Our kernel
  is the window manager; syscalls go directly to ring 0.

---

## 2. Core Data Structures

All public types are `#[repr(C)]` (or `repr(C, packed)` where noted) so
they can be passed across the ring boundary by value or by pointer
without `unsafe` transmute. Sizes are exact: no niche-optimized Rust
enums cross the ring.

### 2.1 Handle Type

```c
// 64 bits, big enough to encode (generation, index, kind).
// 16 bits generation + 16 bits index + 16 bits kind + 16 bits reserved.
typedef struct {
    uint32_t index    : 16;  // index into the table for this kind
    uint32_t generation: 16; // bumped on every destroy; stale handle detected
} bmo_index_t;

typedef struct {
    uint32_t kind     : 8;   // BMO_HANDLE_KIND_WINDOW, _DC, _SURFACE, _TIMER, _CLASS
    uint32_t slot     : 24;  // index into the global handle table
} bmo_handle_t;
```

Two handle encodings are provided:

| Encoding   | Use                                                                   |
|------------|-----------------------------------------------------------------------|
| `bmo_index_t` (32-bit) | Public window/DC/timer identifiers returned to Ring 3; carries generation for safety. |
| `bmo_handle_t` (32-bit) | Internal kernel-side identifiers in the global handle table; kind field disambiguates slot type. |

A public `bmo_index_t` is what `bmo_create_window` returns. To use it in
another syscall, the kernel looks up `(kind, slot)` → handle table entry
→ checks `generation` matches. If not, return `BMO_ERR_STALE_HANDLE`.

This is X11's XID pattern applied to Win32's HWND pattern.

### 2.2 Window Table (kernel-owned)

```c
#define BMO_MAX_WINDOWS  256        // hard cap; not configurable
#define BMO_MAX_CLASSES  32
#define BMO_MAX_DCS      256
#define BMO_MAX_SURF     256
#define BMO_MAX_TIMERS   1024
#define BMO_QUEUE_CAP    64         // messages per thread queue
#define BMO_WND_PROC_MAGIC 0xB17D  // signature on wnd_proc registration

typedef uint16_t bmo_msg_t;        // fits in BMO_MSG_* enum range
typedef uint16_t bmo_wid_t;        // public window id (0..=BMO_MAX_WINDOWS-1)
typedef uint16_t bmo_classid_t;
typedef uint16_t bmo_dc_t;
typedef uint16_t bmo_surf_t;
typedef uint16_t bmo_timer_t;

struct bmo_class {
    char           name[32];       // registered class name (NUL-terminated)
    uint32_t       magic;          // BMO_WND_PROC_MAGIC once live
    uint64_t       wnd_proc;       // Ring 3 RIP of the wnd_proc
    uint32_t       style;          // BMO_CS_* class style flags
    uint32_t       style_ex;       // extended style
    uint16_t       extra_bytes;    // class-private bytes (like WNDCLASSEX.cbWndExtra)
    uint16_t       owner_pid;      // process that registered the class
    uint8_t        hbr_background; // index into system brush table (0..31)
    uint8_t        reserved[3];
    uint64_t       last_used_tick; // for GC of unused classes
};

struct bmo_window {
    bmo_wid_t      id;             // matches slot index
    uint16_t       generation;     // bumped on destroy
    uint16_t       class_id;       // into class table
    uint16_t       owner_tid;      // thread that created the window
    uint16_t       owner_pid;
    uint32_t       flags;          // BMO_WF_* (live flags)
    uint32_t       style;          // WS_* mirror
    uint32_t       style_ex;       // WS_EX_* mirror
    int32_t        x, y, w, h;     // outer window rect, screen coords
    int32_t        cx, cy, cw, ch; // last client rect (cached for BM_SIZE)
    bmo_wid_t      parent;         // BMO_WID_INVALID if top-level
    bmo_wid_t      owner;          // for owned (popup) windows
    bmo_wid_t      first_child;
    bmo_wid_t      next_sibling;
    bmo_wid_t      prev_sibling;
    int32_t        z_order;        // 0 = bottom; higher = on top
    bmo_wid_t      next_z;         // singly-linked Z list (head = top)
    bmo_wid_t      prev_z;
    uint8_t        visible;        // 0/1 (cached for fast hit-test)
    uint8_t        enabled;        // 0/1
    uint8_t        focus;          // 0/1
    uint8_t        captured;       // 0/1 (mouse capture)
    uint8_t        dirty;          // 0/1 (needs paint)
    uint8_t        erase_pending;  // 0/1 (BM_ERASEBKGND needs to be sent)
    uint8_t        in_sizemove;    // 0/1
    uint8_t        reserved;
    bmo_surf_t     surface;        // offscreen double buffer
    bmo_dc_t       dc;             // paint DC (valid only during BM_PAINT)
    uint64_t       last_paint_tick;
    uint64_t       user_data;      // passed to wnd_proc in BM_CREATE.lParam
    char           title[64];      // cached title for window manager
};
```

Windows are kept in three structures, all indexed by `bmo_wid_t`:

1. The flat `windows[BMO_MAX_WINDOWS]` array.
2. The Z-order list (singly-linked from bottom to top).
3. The parent/child/sibling tree (used for hit testing and message
   forwarding).

### 2.3 Message Struct (the heart of the API)

```c
struct bmo_msg {
    bmo_msg_t      kind;       // BMO_MSG_*
    uint16_t       target;     // bmo_wid_t of receiving window
    uint16_t       source;     // bmo_wid_t of source window (0 = kernel)
    uint32_t       timestamp;  // ms since boot (TSC/1000, fits 4.29e6 hrs)
    uint64_t       wparam;     // message-specific
    uint64_t       lparam;     // message-specific
    int64_t        lparam_s;   // signed alias for messages that need it
    int32_t        pt_x;       // mouse position (or -1, -1 if N/A)
    int32_t        pt_y;
};
// Total size: 32 bytes. (matches 8-byte alignment, 4 of them in one cache line)
```

The `lparam`/`lparam_s` union is solved by always storing `uint64_t` and
documenting per-message which interpretation is used. `kind` and
`target` are 16-bit so the struct fits in 32 bytes and two messages
fit in one 64-byte cache line — the most common queue-read pattern.

### 2.4 Message Queue (per-thread, SPSC ring)

```c
struct bmo_msg_queue {
    uint32_t       magic;            // BMO_QUEUE_MAGIC
    uint16_t       head;             // producer index (kernel)
    uint16_t       tail;             // consumer index (Ring 3)
    uint16_t       cap;              // BMO_QUEUE_CAP (64)
    uint16_t       overflow_count;   // messages dropped since last drain
    uint8_t        waiting;          // 1 = thread blocked in bmo_get_message
    uint8_t        pad[3];
    struct bmo_msg msgs[BMO_QUEUE_CAP];
};
```

**Producer = kernel**, **consumer = the thread that owns the queue**.
This is a strict SPSC ring, so the only atomic needed is a release-store
on `head` after writing a message. The reader does an acquire-load on
`head` and a plain load on `tail`. The `magic` and `overflow_count` are
for diagnostics, not correctness.

The queue is allocated by the kernel in a fixed per-thread control block
(see §6.4). Capacity is fixed at 64 messages; the kernel never blocks
on a full queue — instead it sets `overflow_count` and drops the
message. The user's wnd_proc is responsible for draining fast.

### 2.5 Event Types Enum (C-compatible as `u16`)

```c
// All values < 0x0400 are reserved for the kernel.
// 0x0000..0x00FF = system lifecycle
// 0x0100..0x01FF = window state
// 0x0200..0x02FF = keyboard
// 0x0300..0x03FF = mouse
// 0x0400..0x7FFF = user-defined (BMO_WM_USER)

enum bmo_msg_kind {
    BMO_MSG_NULL            = 0x0000,
    BMO_MSG_CREATE          = 0x0001,
    BMO_MSG_DESTROY         = 0x0002,
    BMO_MSG_PAINT           = 0x0003,
    BMO_MSG_SIZE            = 0x0004,
    BMO_MSG_MOVE            = 0x0005,
    BMO_MSG_ACTIVATE        = 0x0006,
    BMO_MSG_SETFOCUS        = 0x0007,
    BMO_MSG_KILLFOCUS       = 0x0008,
    BMO_MSG_CLOSE           = 0x0009,
    BMO_MSG_SHOWWINDOW      = 0x000A,
    BMO_MSG_HIDE            = 0x000B,
    BMO_MSG_ERASEBKGND      = 0x000C,
    BMO_MSG_NCPAINT         = 0x000D,
    BMO_MSG_NCCALCSIZE      = 0x000E,
    BMO_MSG_NCCREATE        = 0x000F,
    BMO_MSG_NCDESTROY       = 0x0010,
    BMO_MSG_INITDIALOG      = 0x0011,
    BMO_MSG_COMMAND         = 0x0012,
    BMO_MSG_TIMER           = 0x0013,
    BMO_MSG_QUIT            = 0x0014,
    BMO_MSG_ENTERSIZEMOVE   = 0x0015,
    BMO_MSG_EXITSIZEMOVE    = 0x0016,
    BMO_MSG_GETMINMAXINFO   = 0x0017,
    BMO_MSG_WINDOWPOSCHANGED= 0x0018,
    BMO_MSG_DESTROYCLIPBOARD= 0x0019,

    BMO_MSG_KEYDOWN         = 0x0200,
    BMO_MSG_KEYUP           = 0x0201,
    BMO_MSG_CHAR            = 0x0202,
    BMO_MSG_SYSKEYDOWN      = 0x0203,
    BMO_MSG_SYSKEYUP        = 0x0204,
    BMO_MSG_DEADCHAR        = 0x0205,
    BMO_MSG_SYSCHAR         = 0x0206,

    BMO_MSG_MOUSEMOVE       = 0x0300,
    BMO_MSG_LBUTTONDOWN     = 0x0301,
    BMO_MSG_LBUTTONUP       = 0x0302,
    BMO_MSG_RBUTTONDOWN     = 0x0303,
    BMO_MSG_RBUTTONUP       = 0x0304,
    BMO_MSG_MBUTTONDOWN     = 0x0305,
    BMO_MSG_MBUTTONUP       = 0x0306,
    BMO_MSG_MOUSEWHEEL      = 0x0307,
    BMO_MSG_MOUSEHOVER      = 0x0308,
    BMO_MSG_MOUSELEAVE      = 0x0309,
    BMO_MSG_CAPTURECHANGED  = 0x030A,

    BMO_MSG_USER            = 0x0400,   // 0x0400..0x7FFF user-defined
    BMO_MSG_APP             = 0x8000,   // 0x8000..0xBFFF reserved app
    BMO_MSG_REGISTERED      = 0xC000,   // 0xC000..0xFFFF registered (string)
};
```

The split between `BUTTONDOWN` for L/R/M matches Win32's
`WM_LBUTTONDOWN` etc. We do *not* merge them into a single
`BMO_MSG_BUTTONDOWN` with a button-id in wparam; that loses the symmetry
with `MOUSEMOVE` and complicates `bmo_input_poll` translation.

### 2.6 Window Class / Window Flags

```c
// Class styles (passed to bmo_register_class)
#define BMO_CS_VREDRAW         0x0001
#define BMO_CS_HREDRAW         0x0002
#define BMO_CS_OWNDC           0x0020   // each window gets its own DC
#define BMO_CS_CLASSDC         0x0040   // class shares one DC
#define BMO_CS_DBLCLKS         0x0008   // generate BMO_MSG_*DBLCLK
#define BMO_CS_NOCLOSE         0x0200   // no close button
#define BMO_CS_SAVEBITS        0x0800   // save under for popups
#define BMO_CS_BYTEALIGNCLIENT 0x1000
#define BMO_CS_BYTEALIGNWINDOW 0x2000
#define BMO_CS_GLOBALCLASS     0x4000   // visible to all processes

// Per-window styles (bmo_create_window.style)
#define BMO_WS_OVERLAPPED      0x00000000
#define BMO_WS_POPUP           0x80000000
#define BMO_WS_CHILD           0x40000000
#define BMO_WS_MINIMIZE        0x20000000
#define BMO_WS_VISIBLE         0x10000000
#define BMO_WS_DISABLED        0x08000000
#define BMO_WS_CLIPSIBLINGS    0x04000000
#define BMO_WS_CLIPCHILDREN    0x02000000
#define BMO_WS_MAXIMIZE        0x01000000
#define BMO_WS_CAPTION         0x00C00000
#define BMO_WS_BORDER          0x00800000
#define BMO_WS_DLGFRAME        0x00400000
#define BMO_WS_VSCROLL         0x00200000
#define BMO_WS_HSCROLL         0x00100000
#define BMO_WS_SYSMENU         0x00080000
#define BMO_WS_THICKFRAME      0x00040000
#define BMO_WS_GROUP           0x00020000
#define BMO_WS_TABSTOP         0x00010000
#define BMO_WS_MODAL           0x00000400  // custom; not in Win32

// Per-window *flags* (live, kernel-tracked; not user-overridable after create)
#define BMO_WF_VISIBLE         0x00000001
#define BMO_WF_ENABLED         0x00000002
#define BMO_WF_FOCUSED         0x00000004
#define BMO_WF_CAPTURED        0x00000008
#define BMO_WF_DIRTY           0x00000010
#define BMO_WF_TOPMOST         0x00000020
#define BMO_WF_TOOL            0x00000040
#define BMO_WF_POPUP           0x00000080
#define BMO_WF_MODAL           0x00000100
#define BMO_WF_TRANSIENT       0x00000200
#define BMO_WF_SIZEMOVE        0x00000400
#define BMO_WF_DESTROYED       0x80000000
```

`style` and `style_ex` mirror Win32 verbatim, so any guide that talks
about `WS_OVERLAPPEDWINDOW` is directly applicable. The `flags` field
is the *live* state the kernel tracks; modifying it from Ring 3 is a
no-op (the kernel ignores it).

### 2.7 Thread State (per-GUI-thread control block)

```c
struct bmo_thread_state {
    uint32_t       magic;             // BMO_THREAD_MAGIC
    uint16_t       pid;               // owning process
    uint16_t       tid;               // = slot in the per-process thread table
    uint64_t       kernel_stack_top;  // where syscalls land
    uint64_t       user_stack_top;    // where the user wnd_proc lives
    bmo_wid_t      focused_window;
    bmo_wid_t      captured_window;
    bmo_wid_t      active_window;     // top-level foreground window
    struct bmo_msg_queue queue;        // see §2.4
    uint64_t       wnd_proc_ctx;      // saved RBX-like context (TLS pointer)
    uint32_t       wakeup_reason;     // last event that woke this thread
    uint8_t        in_wnd_proc;       // 1 = currently running user wnd_proc
    uint8_t        wait_for_paint;    // 1 = slept waiting for paint request
    uint8_t        reserved[2];
};
```

A thread becomes a "GUI thread" the first time it calls any BMO API
syscall. The kernel allocates a `bmo_thread_state` from a fixed pool
(`BMO_MAX_GUI_THREADS = 64`); it lives in a per-CPU data area addressed
via `IA32_GS_BASE`.

### 2.8 Drawing Context (`bmo_dc_t`)

```c
struct bmo_dc {
    uint16_t       id;
    uint16_t       generation;
    uint16_t       owner_window;      // bmo_wid_t
    uint16_t       reserved;
    int32_t        clip_x, clip_y, clip_w, clip_h;  // active clip rect
    int32_t        orig_clip_x, orig_clip_y, orig_clip_w, orig_clip_h;
    uint32_t       text_color;
    uint32_t       bg_color;
    uint32_t       pen_color;
    uint32_t       brush_color;
    uint8_t        font_id;           // index into system font table
    uint8_t        pen_style;         // BMO_PS_SOLID, _DASH, ...
    uint8_t        brush_style;       // BMO_BS_SOLID, _HATCHED, ...
    uint8_t        rop;               // raster op (BMO_R2_COPYPEN etc.)
    bmo_surf_t     target_surface;    // what bmo_draw_* writes into
};
```

A DC is *always* paired with a surface. `bmo_dc_create(window)` returns
a DC that targets the window's offscreen surface. `bmo_dc_release(dc)`
is a no-op for the default DC (the kernel frees it on `bmo_paint_end`).

### 2.9 Surface (offscreen double buffer)

```c
struct bmo_surface {
    uint16_t       id;
    uint16_t       generation;
    uint16_t       width, height;     // 0..=65535
    uint16_t       pitch;             // bytes per row (rounded up to 16)
    uint32_t       format;            // BMO_PF_* (see §3.6)
    uint64_t       phys_addr;         // for kernel compositor DMA
    uint64_t       virt_addr;         // for Ring 3 mmap
    uint32_t       refcount;
    uint16_t       owner_window;      // bmo_wid_t
    uint16_t       flags;             // BMO_SF_DIRTY, BMO_SF_LOCKED
};
```

Surfaces are allocated by the kernel (not by user code) and live in
a contiguous kernel heap. They are 16-byte aligned. The pitch is always
a multiple of 16 bytes. `bmo_create_surface` returns a handle that
allows sharing; v2.0 only supports the window-owned case (1:1).

---

## 3. Syscall ABI

### 3.1 Calling Convention

```c
// x86-64 System V AMD64 / Linux convention:
uint64_t syscall(bmo_syscall_nr_t nr,
                 uint64_t a0, uint64_t a1, uint64_t a2,
                 uint64_t a3, uint64_t a4, uint64_t a5);

// Register usage:
//   RAX  = syscall number (in), return value (out)
//   RDI  = a0
//   RSI  = a1
//   RDX  = a2
//   R10  = a3   (NOT RCX — syscall clobbers RCX)
//   R8   = a4
//   R9   = a5
//   R11  = saved RFLAGS (clobbered)
//   RCX  = saved RIP (clobbered)
// On return: RAX = result. Negative = error code (-1..-4095 mapped to errno).
```

This matches the existing FastOS 0x00..0xF0 syscalls (see
`arch/syscall_entry.rs`), so the BMO API inherits the convention
without changes to the entry asm.

### 3.2 Number Allocation

| Range          | Subsystem                                            |
|----------------|------------------------------------------------------|
| `0x000..0x0FF` | Process / thread lifecycle                           |
| `0x100..0x10F` | Process info (pid, tid, exit_code)                  |
| `0x110..0x11F` | Memory (mmap, munmap, mprotect)                      |
| `0x120..0x12F` | IPC (pipe, futex)                                    |
| `0x130..0x13F` | Signals                                              |
| `0x140..0x14F` | Time (clock_gettime, nanosleep) — already at 0x50    |
| `0x150..0x15F` | I/O ports                                            |
| `0x160..0x16F` | Reserved                                             |
| `0x180..0x18F` | VFS — already at 0x20..0x25                          |
| `0x190..0x19F` | Userinfo                                             |
| `0x1A0..0x1AF` | Security / capabilities                              |
| `0x1F0..0x1FF` | Debug                                                |
| `0x100..0x1FF` | **BMO API — windowing**                              |
| `0x200..0x2FF` | (reserved for future subsystems)                     |
| `0x300..0x3FF` | (reserved)                                           |

**The BMO API occupies the entire `0x100..0x1FF` range, 256 numbers.**
This is wider than the current FastOS 0xF0 ceiling, so the syscall
dispatcher is widened from 8-bit to 16-bit (one `match` arm per
range). The dispatcher is extended in `arch/syscall_entry.rs::match nr`,
adding a second `match` covering 0x100..=0x1FF that calls into
`bmo_api::syscall::dispatch`.

### 3.3 Complete BMO API Syscall List

All numbers are **hexadecimal**. Return value: `0` = success (or
non-negative value), negative = `-BMO_ERR_*` error code. Out-pointers
are kernel-validated for user-pointer containment in the calling
process's VMA.

#### Window class / window lifecycle (0x100..0x10F)

| #  | Name                | a0 (RDI)            | a1 (RSI)            | a2 (RDX)                | a3 (R10)  | a4 (R8)    | a5 (R9)    | Returns                  |
|----|---------------------|---------------------|---------------------|-------------------------|-----------|------------|------------|--------------------------|
| 0x100 | `bmo_register_class` | `const struct bmo_class*` (in user mem) | class size (must be 64) | 0 | 0 | 0 | 0 | `bmo_classid_t` or `-BMO_ERR_*` |
| 0x101 | `bmo_unregister_class` | `bmo_classid_t`   | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x102 | `bmo_create_window_ex` | `bmo_classid_t`   | title (user ptr)    | title_len (≤63)         | style     | style_ex   | x          | `bmo_wid_t` (low 16 = id, high 16 = gen) |
| 0x103 | `bmo_create_window` (variadic) | (same as 0x102 but in stack-passed struct) |  |  |  |  |  | (alias for 0x102) |
| 0x104 | `bmo_destroy_window` | `bmo_wid_t`         | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x105 | `bmo_show_window`    | `bmo_wid_t`         | cmd (SW_*)          | 0                       | 0         | 0          | 0          | 0 or `BOOL` non-zero    |
| 0x106 | `bmo_hide_window`    | `bmo_wid_t`         | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x107 | `bmo_set_title`      | `bmo_wid_t`         | title (user ptr)    | title_len (≤63)         | 0         | 0          | 0          | 0 or error               |
| 0x108 | `bmo_get_title`      | `bmo_wid_t`         | buf (user ptr)      | buf_len                 | 0         | 0          | 0          | bytes written            |
| 0x109 | `bmo_set_size`       | `bmo_wid_t`         | width               | height                  | flags     | 0          | 0          | 0 or error               |
| 0x10A | `bmo_set_pos`        | `bmo_wid_t`         | x                   | y                       | flags     | 0          | 0          | 0 or error               |
| 0x10B | `bmo_get_rect`       | `bmo_wid_t`         | out: `struct bmo_rect*` | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x10C | `bmo_set_parent`     | `bmo_wid_t` (child) | `bmo_wid_t` (parent, BMO_WID_INVALID to detach) | 0 | 0 | 0 | 0 | 0 or error |
| 0x10D | `bmo_invalidate`     | `bmo_wid_t`         | flags (RDW_*)       | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x10E | `bmo_update_window`  | `bmo_wid_t`         | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x10F | `bmo_redraw_window`  | `bmo_wid_t`         | flags (RDW_*)       | 0                       | 0         | 0          | 0          | 0 or error               |

#### Paint / drawing (0x110..0x11F)

| #  | Name                | a0 (RDI)            | a1 (RSI)            | a2 (RDX)                | a3 (R10)  | a4 (R8)    | a5 (R9)    | Returns                  |
|----|---------------------|---------------------|---------------------|-------------------------|-----------|------------|------------|--------------------------|
| 0x110 | `bmo_paint_begin`   | `bmo_wid_t`         | out: `struct bmo_paintstruct*` | 0           | 0         | 0          | 0          | `bmo_dc_t` (or 0 on err) |
| 0x111 | `bmo_paint_end`     | `bmo_wid_t`         | `bmo_dc_t`          | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x112 | `bmo_draw_pixel`    | `bmo_dc_t`          | x                   | y                       | color (ARGB) | 0          | 0          | 0 or error               |
| 0x113 | `bmo_draw_line`     | `bmo_dc_t`          | x0                  | y0                      | x1         | y1         | 0          | 0 or error               |
| 0x114 | `bmo_draw_rect`     | `bmo_dc_t`          | x                   | y                       | w          | h          | color      | 0 or error               |
| 0x115 | `bmo_fill_rect`     | `bmo_dc_t`          | x                   | y                       | w          | h          | color      | 0 or error               |
| 0x116 | `bmo_draw_text`     | `bmo_dc_t`          | x                   | y                       | text (user ptr) | len | color | 0 or error               |
| 0x117 | `bmo_draw_image`    | `bmo_dc_t`          | x                   | y                       | w, h (split) | src (user ptr) | src_pitch | 0 or error |
| 0x118 | `bmo_draw_polyline` | `bmo_dc_t`          | count               | points (user ptr, `struct bmo_point*`) | color | 0 | 0 | 0 or error |
| 0x119 | `bmo_set_clip`      | `bmo_dc_t`          | x                   | y                       | w          | h          | 0          | 0 or error               |
| 0x11A | `bmo_reset_clip`    | `bmo_dc_t`          | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x11B | `bmo_set_text_color`| `bmo_dc_t`          | color               | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x11C | `bmo_set_bg_color`  | `bmo_dc_t`          | color               | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x11D | `bmo_set_font`      | `bmo_dc_t`          | font_id             | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x11E | `bmo_create_surface` | width              | height              | format (BMO_PF_*)        | 0         | 0          | 0          | `bmo_surf_t`             |
| 0x11F | `bmo_destroy_surface`| `bmo_surf_t`        | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |

#### Message dispatch (0x120..0x12F)

| #  | Name                | a0 (RDI)            | a1 (RSI)            | a2 (RDX)                | a3 (R10)  | a4 (R8)    | a5 (R9)    | Returns                  |
|----|---------------------|---------------------|---------------------|-------------------------|-----------|------------|------------|--------------------------|
| 0x120 | `bmo_get_message`   | out: `struct bmo_msg*` | 0                 | 0                       | 0         | 0          | 0          | non-zero = got msg, 0 = no msg, -1 = quit pending |
| 0x121 | `bmo_peek_message`  | out: `struct bmo_msg*` | filter_kind_min  | filter_kind_max         | 0         | 0          | 0          | non-zero = got msg, 0 = no msg |
| 0x122 | `bmo_post_message`  | `bmo_wid_t`         | `bmo_msg_t` kind    | wparam                  | lparam    | 0          | 0          | 0 or error               |
| 0x123 | `bmo_send_message`  | `bmo_wid_t`         | `bmo_msg_t` kind    | wparam                  | lparam    | 0          | 0          | wnd_proc's return value  |
| 0x124 | `bmo_dispatch_message` | `const struct bmo_msg*` | 0               | 0                       | 0         | 0          | 0          | wnd_proc's return value  |
| 0x125 | `bmo_translate_message` | `const struct bmo_msg*` | 0              | 0                       | 0         | 0          | 0          | 0 (kernel may generate BMO_MSG_CHAR) |
| 0x126 | `bmo_wait_message`  | 0                   | 0                   | 0                       | 0         | 0          | 0          | 0 or error (block until queue non-empty) |
| 0x127 | `bmo_post_quit`     | exit_code           | 0                   | 0                       | 0         | 0          | 0          | 0                        |
| 0x128 | `bmo_post_thread_message` | target_tid       | kind                | wparam                  | lparam    | 0          | 0          | 0 or error               |
| 0x129 | `bmo_set_timer`     | `bmo_wid_t`         | timer_id            | timeout_ms              | 0         | 0          | 0          | `bmo_timer_t`            |
| 0x12A | `bmo_kill_timer`    | `bmo_wid_t`         | timer_id            | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x12B | `bmo_set_capture`   | `bmo_wid_t`         | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x12C | `bmo_release_capture`| 0                   | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x12D | `bmo_set_focus`     | `bmo_wid_t`         | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x12E | `bmo_get_focus`     | 0                   | 0                   | 0                       | 0         | 0          | 0          | `bmo_wid_t`              |
| 0x12F | `bmo_get_active`    | 0                   | 0                   | 0                       | 0         | 0          | 0          | `bmo_wid_t` (top-level fg) |

#### Device context (0x130..0x13F)

| #  | Name                | a0 (RDI)            | a1 (RSI)            | a2 (RDX)                | a3 (R10)  | a4 (R8)    | a5 (R9)    | Returns                  |
|----|---------------------|---------------------|---------------------|-------------------------|-----------|------------|------------|--------------------------|
| 0x130 | `bmo_dc_create`     | `bmo_wid_t`         | 0                   | 0                       | 0         | 0          | 0          | `bmo_dc_t` (or 0 on err) |
| 0x131 | `bmo_dc_release`    | `bmo_dc_t`          | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x132 | `bmo_get_dc`        | `bmo_wid_t`         | 0                   | 0                       | 0         | 0          | 0          | `bmo_dc_t` (alias of paint_begin) |
| 0x133 | `bmo_release_dc`    | `bmo_wid_t`         | `bmo_dc_t`          | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x134 | `bmo_save_dc`       | `bmo_dc_t`          | 0                   | 0                       | 0         | 0          | 0          | int (saved state id)     |
| 0x135 | `bmo_restore_dc`    | `bmo_dc_t`          | saved_id            | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x136 | `bmo_select_object` | `bmo_dc_t`          | object_id (font/pen/brush) | 0                 | 0         | 0          | 0          | 0 or error               |
| 0x137 | `bmo_get_pixel`     | `bmo_dc_t`          | x                   | y                       | 0         | 0          | 0          | color (ARGB)             |
| 0x138 | `bmo_set_pixel`     | `bmo_dc_t`          | x                   | y                       | color     | 0          | 0          | 0 or error               |
| 0x139 | `bmo_bit_blt`       | dst_dc              | dst_x               | dst_y                    | w, h (split) | src_dc   | src_pitch  | 0 or error               |
| 0x13A | `bmo_stretch_blt`   | dst_dc              | dst_x, dst_y, dst_w, dst_h (via struct ptr) | src_dc | src_x, src_y, src_w, src_h | 0 | 0 or error |
| 0x13B | `bmo_transparent_blt`| dst_dc             | ...                 | ...                     | ...       | ...        | ...        | 0 or error               |
| 0x13C | `bmo_alpha_blend`   | dst_dc              | ...                 | ...                     | ...       | ...        | ...        | 0 or error               |
| 0x13D | (reserved)          |                     |                     |                         |           |            |            |                          |
| 0x13E | (reserved)          |                     |                     |                         |           |            |            |                          |
| 0x13F | (reserved)          |                     |                     |                         |           |            |            |                          |

#### Input polling (0x140..0x14F)

| #  | Name                | a0 (RDI)            | a1 (RSI)            | a2 (RDX)                | a3 (R10)  | a4 (R8)    | a5 (R9)    | Returns                  |
|----|---------------------|---------------------|---------------------|-------------------------|-----------|------------|------------|--------------------------|
| 0x140 | `bmo_input_poll_key` | out: `struct bmo_keyevent*` | 0          | 0                       | 0         | 0          | 0          | non-zero if event available |
| 0x141 | `bmo_input_poll_mouse`| out: `struct bmo_mouseevent*` | 0        | 0                       | 0         | 0          | 0          | non-zero if event available |
| 0x142 | `bmo_input_wait`    | timeout_ms          | 0                   | 0                       | 0         | 0          | 0          | bits set: BMO_WAIT_KEY (1), BMO_WAIT_MOUSE (2) |
| 0x143 | `bmo_input_grab`    | `bmo_wid_t`         | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x144 | `bmo_input_ungrab`  | 0                   | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x145 | `bmo_show_cursor`   | 0                   | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x146 | `bmo_hide_cursor`   | 0                   | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x147 | `bmo_set_cursor_pos`| x                   | y                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x148 | `bmo_set_cursor`    | cursor_id (0..15)   | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x149..0x14F | (reserved) |                     |                     |                         |           |            |            |                          |

#### Window manager / Z-order (0x150..0x15F)

| #  | Name                | a0 (RDI)            | a1 (RSI)            | a2 (RDX)                | a3 (R10)  | a4 (R8)    | a5 (R9)    | Returns                  |
|----|---------------------|---------------------|---------------------|-------------------------|-----------|------------|------------|--------------------------|
| 0x150 | `bmo_bring_to_front`| `bmo_wid_t`         | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x151 | `bmo_send_to_back`  | `bmo_wid_t`         | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x152 | `bmo_set_topmost`   | `bmo_wid_t`         | bool                | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x153 | `bmo_set_transient_for`| `bmo_wid_t`       | `bmo_wid_t` (owner) | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x154 | `bmo_begin_modal`   | `bmo_wid_t`         | 0                   | 0                       | 0         | 0          | 0          | 0 or error (disables other windows in thread) |
| 0x155 | `bmo_end_modal`     | `bmo_wid_t`         | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x156 | `bmo_set_window_pos`| `bmo_wid_t`         | hwnd_insert_after   | x                       | y         | cx         | cy         | 0 or error (mirrors SetWindowPos) |
| 0x157 | `bmo_get_window`    | pid (0 = own)       | out: `struct bmo_window_info*` | 0          | 0         | 0          | 0          | 0 or error               |
| 0x158 | `bmo_enum_windows`  | out: `bmo_wid_t*` array | max_count         | 0                       | 0         | 0          | 0          | count written            |
| 0x159 | `bmo_get_desktop_window`| 0                | 0                   | 0                       | 0         | 0          | 0          | `bmo_wid_t` (root)       |
| 0x15A | `bmo_get_foreground_window`| 0              | 0                   | 0                       | 0         | 0          | 0          | `bmo_wid_t`              |
| 0x15B..0x15F | (reserved) |                     |                     |                         |           |            |            |                          |

#### Cursor, icon, system (0x160..0x16F)

| #  | Name                | a0 (RDI)            | a1 (RSI)            | a2 (RDX)                | a3 (R10)  | a4 (R8)    | a5 (R9)    | Returns                  |
|----|---------------------|---------------------|---------------------|-------------------------|-----------|------------|------------|--------------------------|
| 0x160 | `bmo_load_cursor`   | cursor_id (0..15 builtin) | 0              | 0                       | 0         | 0          | 0          | `bmo_cursor_t`           |
| 0x161 | `bmo_load_icon`     | icon_id (0..15 builtin) | 0                | 0                       | 0         | 0          | 0          | `bmo_icon_t`             |
| 0x162 | `bmo_set_class_cursor`| `bmo_classid_t`   | `bmo_cursor_t`      | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x163 | `bmo_set_class_icon` | `bmo_classid_t`    | `bmo_icon_t`        | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x164..0x16F | (reserved) |                     |                     |                         |           |            |            |                          |

#### Clipboard (0x170..0x17F)

| #  | Name                | a0 (RDI)            | a1 (RSI)            | a2 (RDX)                | a3 (R10)  | a4 (R8)    | a5 (R9)    | Returns                  |
|----|---------------------|---------------------|---------------------|-------------------------|-----------|------------|------------|--------------------------|
| 0x170 | `bmo_open_clipboard`| 0                   | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x171 | `bmo_close_clipboard`| 0                  | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x172 | `bmo_set_clipboard_data`| format_id        | data (user ptr)     | size                    | 0         | 0          | 0          | 0 or error               |
| 0x173 | `bmo_get_clipboard_data`| format_id        | out: user_buf       | buf_size                | 0         | 0          | 0          | bytes written            |
| 0x174 | `bmo_empty_clipboard`| 0                  | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x175..0x17F | (reserved) |                     |                     |                         |           |            |            |                          |

#### Memory / surface mapping (0x180..0x18F)

| #  | Name                | a0 (RDI)            | a1 (RSI)            | a2 (RDX)                | a3 (R10)  | a4 (R8)    | a5 (R9)    | Returns                  |
|----|---------------------|---------------------|---------------------|-------------------------|-----------|------------|------------|--------------------------|
| 0x180 | `bmo_map_surface`   | `bmo_surf_t`        | 0                   | 0                       | 0         | 0          | 0          | user-space pointer (in RAX) |
| 0x181 | `bmo_unmap_surface` | `bmo_surf_t`        | 0                   | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x182 | `bmo_surface_flush` | `bmo_surf_t`        | x, y, w, h (split)  | 0                       | 0         | 0          | 0          | 0 or error               |
| 0x183 | `bmo_flip`          | `bmo_surf_t`        | 0                   | 0                       | 0         | 0          | 0          | 0 or error (present to compositor) |
| 0x184..0x18F | (reserved) |                     |                     |                         |           |            |            |                          |

#### Reserved / future (0x190..0x1FF)

0x190..0x1CF: reserved for future BMO extensions (drag-drop, accessibility,
IME, GPU).

0x1F0..0x1FF: debug / introspection (e.g. `bmo_dump_windows`,
`bmo_trace_msg`).

### 3.4 Number-Allocation Strategy

- **Hard cap of 256 syscalls** for the BMO API. Adding a new one
  requires a kernel patch (matches Linux's syscall table).
- **Bits 0x00..0x0F of nr are reserved** for FastOS core. BMO never uses
  them.
- **Multi-arg syscalls pass structs by pointer**, not by value, when the
  struct > 40 bytes. `bmo_create_window_ex` takes 7 scalars; for the
  variadic `bmo_create_window` we use a 56-byte struct passed in rdi.
- **Errors are negative errno-like values** in the range -1..=-4095.
  Positive returns are either an unsigned handle (0..=0xFFFF), a flag
  bitmask, or a small count.

### 3.5 Error Codes

```c
#define BMO_OK                 0
#define BMO_ERR_GENERIC       -1
#define BMO_ERR_BAD_HANDLE    -2     // handle table: gen mismatch
#define BMO_ERR_INVALID       -3     // invalid arg
#define BMO_ERR_NO_MEMORY     -4
#define BMO_ERR_NO_WINDOW     -5     // window not found
#define BMO_ERR_NOT_GUI_THR   -6     // thread not a GUI thread
#define BMO_ERR_QUEUE_FULL    -7     // 64-msg queue overflow
#define BMO_ERR_BAD_CLASS     -8
#define BMO_ERR_CLASS_EXISTS  -9
#define BMO_ERR_NO_CLASS      -10
#define BMO_ERR_BAD_DC        -11
#define BMO_ERR_BAD_SURFACE   -12
#define BMO_ERR_BUSY          -13    // surface locked
#define BMO_ERR_TIMEOUT       -14
#define BMO_ERR_BAD_FORMAT    -15
#define BMO_ERR_NO_QUIT       -16    // bmo_get_message w/o WM_QUIT ever
#define BMO_ERR_REENTRANT     -17    // wnd_proc called itself
#define BMO_ERR_PERM          -18    // permission denied (cross-pid)
#define BMO_ERR_STALE         -19    // alias for BAD_HANDLE
```

### 3.6 Pixel Format Constants

```c
#define BMO_PF_ARGB32        0x01    // 32 bpp, 0xAARRGGBB
#define BMO_PF_XRGB32        0x02    // 32 bpp, 0x00RRGGBB
#define BMO_PF_RGB24         0x03    // packed RGB
#define BMO_PF_RGB565        0x04    // 16 bpp
#define BMO_PF_A8            0x05    // 8 bpp alpha mask
#define BMO_PF_INDEX8        0x06    // 256-color indexed (palette TBD)
```

The kernel framebuffer is XRGB32 (UEFI GOP standard). All surfaces
allocated via `bmo_create_surface` default to ARGB32 so that alpha
blending works without conversion. Conversion is done lazily on `bmo_flip`.

---

## 4. Window Procedure Model

### 4.1 The Callback

```c
typedef uint64_t (*bmo_wnd_proc_fn)(
    bmo_wid_t     hwnd,         // in: target window
    bmo_msg_t     msg,          // in: BMO_MSG_*
    uint64_t      wparam,       // in
    uint64_t      lparam,       // in
    uint64_t      user_data     // in: from bmo_create_window's lparam
);
// Returns: message-specific (0 for messages that don't return a value)
```

The wnd_proc is **Ring 3 code** at a known address. Registration
happens via `bmo_register_class`:

```c
struct bmo_class {
    char        name[32];       // class name
    uint64_t    wnd_proc;       // RIP of Ring 3 wnd_proc
    uint32_t    style;          // BMO_CS_*
    uint32_t    style_ex;
    uint16_t    extra_bytes;    // optional per-window storage
    uint8_t     hbr_background; // 0..31 system brush index
    uint8_t     reserved[9];
};
```

The kernel stores the wnd_proc address. It is invoked via the
`bmo_dispatch_message` syscall (which the user calls after
`bmo_get_message` returns). The kernel:

1. Sets up a small **call frame** in the thread's `bmo_thread_state`
   (saves the previous wnd_proc RIP, RSP, R12, RBP, RBX so a nested
   wnd_proc call — e.g. from `bmo_send_message` — is possible).
2. Builds a synthetic iretq frame pointing at the wnd_proc.
3. Restores user GS, switches to user RSP, `iretq` to the wnd_proc.

This is the same mechanism as a syscall return, but the kernel
deliberately does *not* save the user-mode CS/SS — they're already
correct. The kernel just hands off.

### 4.2 The Default Window Procedure

The kernel exports a *default* wnd_proc for windows whose class
wnd_proc returns `BMO_DEFDLGPROC` (0xFFFFFFFFFFFFFFFE) for an
unhandled message. Default behavior mirrors Win32's `DefWindowProc`:

| Message             | Default action                                          |
|---------------------|---------------------------------------------------------|
| `BMO_MSG_CLOSE`      | Post `BMO_MSG_QUIT` if window is main window of process |
| `BMO_MSG_SIZE`       | Recompute client rect, invalidate non-client area       |
| `BMO_MSG_PAINT`      | Validate update region; paint background                |
| `BMO_MSG_NCPAINT`    | Draw title bar, borders, close/min/max buttons          |
| `BMO_MSG_NCCALCSIZE` | Compute client area from frame                          |
| `BMO_MSG_KEYDOWN`    | Translate to `BMO_MSG_CHAR` (via `bmo_translate_message`)|
| `BMO_MSG_GETMINMAXINFO` | Fill in min/max size constraints                     |
| `BMO_MSG_ERASEBKGND` | Fill with `hbr_background`                              |
| (others)            | Return 0                                                |

### 4.3 Message Dispatch Sequence

1. Thread T (in Ring 3) calls `bmo_get_message` via syscall.
2. Kernel pops one message from T's queue (or blocks if empty and
   `bmo_wait_message` semantics were requested via a flag).
3. Thread T receives the message in user memory.
4. Thread T calls `bmo_dispatch_message(&msg)` via syscall.
5. Kernel looks up the window's class, finds the wnd_proc address.
6. Kernel sets `T.in_wnd_proc = 1`, saves the user RSP and a 5-deep
   return stack.
7. Kernel constructs an iretq frame:
   - `rip = class.wnd_proc`
   - `rsp = T.user_stack_top - 32` (room for 4 saved regs by ABI)
   - `rdi = hwnd, rsi = msg, rdx = wparam, rcx = lparam, r8 = user_data`
8. Kernel switches to user RSP, `iretq` to the wnd_proc.
9. wnd_proc runs in Ring 3. It may call further BMO syscalls (e.g.
   `bmo_invalidate`, `bmo_post_message`, `bmo_send_message`).
10. When the wnd_proc returns, the user-mode wrapper (a small stub
    provided by the BMO ABI) does `syscall BMO_DISPATCH_RETURN (0x198)`
    to hand control back to the kernel.
11. Kernel restores the previous RSP, sets `T.in_wnd_proc = 0`,
    returns 0 in RAX to the original caller of `bmo_dispatch_message`.

### 4.4 Reentrancy Rules

- `bmo_send_message` blocks the calling thread (synchronous). It is
  detected as a re-entrant call: the kernel pushes a new frame onto
  T's wnd_proc call stack (max depth 5; deeper = `BMO_ERR_REENTRANT`).
- `bmo_post_message` is non-blocking; the message lands in the target
  thread's queue and returns immediately. If the target thread is the
  *same* thread, the message is appended to the queue; it will be
  dequeued on the next `bmo_get_message` call.
- A wnd_proc MUST NOT call `bmo_destroy_window` on its own window
  during `BMO_MSG_CREATE` or `BMO_MSG_PAINT`. The kernel returns
  `BMO_ERR_REENTRANT`.
- A wnd_proc calling `bmo_get_message` recursively enters a *new* wnd_proc
  call frame; this is the **dialog modal** pattern.

### 4.5 Pre-Translate Hook

The kernel implements `bmo_translate_message` as a *pure function*:
it inspects the message and, if it is a `BMO_MSG_KEYDOWN` with a
printable virtual key, *generates* a `BMO_MSG_CHAR` message and inserts
it after the keydown. The wnd_proc then sees CHAR on the next
`bmo_get_message`.

This matches Win32's `TranslateMessage` in the user-mode message loop,
but the implementation is kernel-side to keep Ring 3 programs simple.

---

## 5. Module Breakdown (kernel side)

```
kernel/src/bmo_api/
├── mod.rs                # facade, init, public re-exports
├── handle.rs             # bmo_handle_t, bmo_index_t, handle table, generations
├── window.rs             # bmo_window struct, class struct, lookup helpers
├── message.rs            # bmo_msg, bmo_msg_kind enum, marshalling
├── queue.rs              # bmo_msg_queue (SPSC ring), push/pop, wakeup
├── event.rs              # coalescing, paint-region combine, mouse-move dedup
├── draw.rs               # bmo_dc, surface ops, font blit, primitive draw
├── surface.rs            # bmo_surface, mmap user-space, flip, double-buffer
├── input.rs              # PS/2 + USB HID keyboard/mouse, button states
├── syscall.rs            # dispatcher table (0x100..0x1FF), arg marshalling
├── wm.rs                 # window manager: Z-order, focus, drag/resize, snap
├── timer.rs              # timer wheel, expiration queue
├── cursor.rs             # cursor management, sprite blit
├── class.rs              # class registration, lookup, default wnd_proc
├── input_thread.rs       # kernel thread that reads input & posts messages
├── paint_compositor.rs   # dirty-region tracking, vsync, page-flip
└── compat/
    └── v1.rs             # bridges the old bmo_api (Ring 0) to v2 (Ring 3)
```

### 5.1 Per-Module Responsibilities

- **`mod.rs`** — One global `static mut BMO_STATE: BmoState` that owns
  every other table. Exposes `bmo_api::init()` (called from
  `boot::phase5`) and `bmo_api::tick()` (called every scheduler tick).
- **`handle.rs`** — Fixed-size `HANDLE_TABLE[1024]`; each slot has
  `kind`, `generation`, `pid` (owner for cleanup), and a tagged union
  pointer. `bmo_alloc_handle(kind) -> slot`, `bmo_lookup_handle(slot, kind, gen) -> *mut T`.
- **`window.rs`** — `windows[BMO_MAX_WINDOWS]`, parent/child/sibling
  list manipulation, generation bumps on destroy, parent-window walk
  (used for hit-test and message forwarding).
- **`message.rs`** — Definition of `bmo_msg` and the encode/decode
  helpers (`bmo_msg_from_wparam_lparam`, etc.). Pure data; no IO.
- **`queue.rs`** — Lock-free SPSC ring with a single byte of padding
  to avoid false sharing. Producer (kernel) writes, consumer (Ring 3)
  reads; both sides are wait-free in the common case.
- **`event.rs`** — Coalesces `MOUSEMOVE` (only the latest kept), merges
  `PAINT` update regions into bounding box, drops duplicate `SIZE` for
  the same size, etc. Pure-function pipeline: kernel input thread
  produces raw events, event.rs coalesces, queue.rs stores.
- **`draw.rs`** — Implements `bmo_dc_*` syscalls. Each primitive
  operation is a small function: clip-test, write to the user-mapped
  surface. Font blit is a 8x16 monochrome unpack; v2.0 supports
  exactly one builtin font (8x16 VGA).
- **`surface.rs`** — Owns the kernel heap backing surfaces, the
  per-process VMA mapping for `bmo_map_surface`, and the flip
  operation that hands a surface to the compositor.
- **`input.rs`** — Wakes on PS/2 IRQ1 (keyboard) and IRQ12 (mouse) and
  on USB HID interrupt transfers. Pushes events onto the input thread's
  queue; does *not* touch the window tables directly.
- **`syscall.rs`** — A `match` over the 0x100..0x1FF range with one arm
  per syscall. Marshals args from `InterruptFrame` into a `BmoCallArgs`
  struct and dispatches to the appropriate module. Returns -errno on
  failure.
- **`wm.rs`** — Implements Z-order list, focus rules (focus-follows-mouse
  with click-to-raise), drag/resize modal loop (intercepted
  `BMO_MSG_MOUSEMOVE` / `BMO_MSG_LBUTTONUP`), snap-to-edge,
  alt-tab cycling. Replaces the Ring 0-only logic in the current
  `bmo_api/manager.rs`.
- **`timer.rs`** — Hierarchical timer wheel with 4 levels (256 buckets
  each, ~1 ms granularity, 4.2 hour range). Each timer entry is a
  `(window_id, timer_id, expiration_tick)`. On tick, the wheel
  advances and any expired timers are converted to `BMO_MSG_TIMER` and
  posted.
- **`cursor.rs`** — Owns the 16x16 sprite bitmap for each of the 16
  builtin cursors (arrow, ibeam, wait, cross, size-NESW, etc.).
  Handles the hardware cursor if available (GOP supports an
  8-bpp 64x64 cursor plane), falls back to software cursor with
  dirty-region tracking.
- **`class.rs`** — `classes[BMO_MAX_CLASSES]`, lookup by name, magic
  check, owner-PID check. Default wnd_proc is a kernel-side function
  exposed at a fixed `RIP` (it's part of the kernel image).
- **`input_thread.rs`** — A dedicated kernel thread that reads from the
  PS/2 and USB HID input rings, runs `event.rs::coalesce`, and posts
  to the appropriate window's owning thread queue. Runs at idle
  priority; never blocks user code.
- **`paint_compositor.rs`** — A periodic task (driven by the
  APIC timer) that walks the Z-order list, finds dirty windows, blits
  their surfaces to the framebuffer, and emits `BMO_MSG_PAINT` for
  dirty ones. v2.0 is software-only; v3.0 will add a GPU path.
- **`compat/v1.rs`** — A thin shim so the existing `bmo_api` Ring 0
  code keeps working: routes calls through the v2 kernel tables but
  doesn't expose any of the new syscalls to Ring 0 processes.

### 5.2 Init Order

`bmo_api::init()` is called from `boot::phase5` after scheduler, FPU,
and APIC are up. It does, in order:

1. Allocate the kernel heap for `BmoState`.
2. Init handle table.
3. Init class table (register the kernel's default classes:
   `BmoClass`, `BmoButton`, `BmoEdit`, `BmoListBox`, `BmoStatic`).
4. Init window table.
5. Init the desktop window (special `BMO_WID_DESKTOP = 0`, covers the
   full framebuffer).
6. Init timer wheel.
7. Init input thread (lazy: starts on first `bmo_input_poll` call).
8. Init paint compositor (lazy: starts on first paint request).
9. Init cursor subsystem, load builtin cursor bitmaps.

The default wnd_proc for `BmoClass` and friends is a kernel function
exposed in `.text` at `bmo_default_wnd_proc`. Ring 3 callers don't
call it directly; the kernel's `bmo_dispatch_message` recognizes
`BMO_DEFDLGPROC` and dispatches in-kernel.

---

## 6. Ring 0/3 Transition

### 6.1 Entry (Ring 3 → Ring 0)

Already in place via `arch/syscall_entry.rs`. The BMO API inherits:

- `IA32_LSTAR` = `syscall_entry_naked` (existing).
- On entry: `swapgs`, save user RSP, switch to kernel stack (from
  per-thread `kernel_stack_top` in `bmo_thread_state`).
- Build iretq frame + push GPRs.
- Call `syscall_handler_rust` (existing).
- The 0x100..0x1FF range dispatches to
  `crate::bmo_api::syscall::dispatch(frame)`.

### 6.2 The wnd_proc Call (Ring 0 → Ring 3, *not* via syscall return)

This is the new transition that doesn't exist today. From
`bmo_dispatch_message`:

1. Validate the message, the window, and the wnd_proc address.
2. Reject if `T.in_wnd_proc == 1` and call depth ≥ 5 (re-entrancy).
3. On the kernel stack, push a "fake syscall frame" so that
   `bmo_dispatch_return` (the user stub) can `syscall` back into the
   kernel with the correct context. The frame stores the saved user
   RSP, R12, RBP, RBX, and the wnd_proc return address.
4. Construct an iretq frame in the per-thread kernel area:
   - `rip = class.wnd_proc`
   - `cs = USER_CS (0x23)`
   - `rflags = current RFLAGS | 0x200 (IF) | 0x2 (reserved)`
   - `rsp = T.user_stack_top - 64` (room for 4 args + alignment)
   - `ss = USER_DS (0x1B)`
5. Set `T.in_wnd_proc = 1` and increment call depth.
6. **Do not `swapgs` here** — the user GS is already correct. The user
   code is using GS as a thread-local pointer (Ring 3 ABI provides
   `bmo_tls` accessors).
7. `iretq`.

### 6.3 Return from wnd_proc (Ring 3 → Ring 0)

The BMO ABI ships a `bmo_dispatch_trampoline` function in every Ring 3
binary (placed in `.text.bmo`):

```asm
bmo_dispatch_trampoline:
    ; rdi..r8 already hold the wnd_proc's return value path
    ; (caller is responsible for spilling callee-saved regs)
    mov rax, 0x198            ; BMO_DISPATCH_RETURN
    syscall                   ; enters kernel at LSTAR
    ; kernel restores everything, returns here with the saved RAX
    ret
```

The kernel sees syscall #0x198 (in the 0x100..0x1FF range, with the
high bit set as a "return from wnd_proc" sentinel — same encoding as
Linux's `sys_rt_sigreturn`):

1. Validate `T.in_wnd_proc == 1`.
2. Pop the saved user RSP and callee-saved regs from the kernel stack
   frame constructed in step 3 of §6.2.
3. Set `T.in_wnd_proc = 0` and decrement call depth.
4. Return RAX to the original caller of `bmo_dispatch_message`.

The user-mode `ret` from the trampoline then unwinds to wherever
`bmo_dispatch_message` was called from.

### 6.4 Per-Thread Kernel Stack

Each GUI thread has two stacks:

- **User stack** (Ring 3) — set up by the process loader; the wnd_proc
  runs here. Default 64 KiB, grows on demand via the existing demand
  pager.
- **Kernel stack** (Ring 0) — allocated by the kernel when the thread
  is registered as a GUI thread (first BMO API call). Fixed 16 KiB,
  never grows. Used for syscall handling and the wnd_proc transition
  frame.

The kernel stack is recorded in the TSS for the thread's CPU. The
syscall entry's `mov rsp, [T.kernel_stack_top]` (currently a global)
becomes `mov rsp, [gs:THREAD_KERNEL_STACK]` after we set up the
per-CPU per-thread data area. v2.0 may punt on per-CPU and use a
global "current thread" pointer initially; per-CPU is a v2.1 concern.

### 6.5 Reentrancy and Stack Depth

A single Ring 3 thread can be inside at most **5** nested wnd_procs at
once. This is checked by `T.wnd_proc_depth`; the 6th attempt returns
`BMO_ERR_REENTRANT`. The limit exists to bound the kernel-stack usage
of nested transitions (5 × ~256 bytes = 1.25 KiB worst case).

### 6.6 Async Notification

If a kernel-side event (e.g. a new input event, a timer expiration)
needs to wake a thread blocked in `bmo_get_message`, the kernel does:

1. Write the message to the queue.
2. If `T.waiting`, mark the thread Ready and `iretq` it back to user
   mode with RAX = 0 and a synthetic `BMO_MSG_NULL` result (so the
   caller knows to call `bmo_get_message` again to fetch the real one).
3. If the thread is mid-wnd_proc, just leave the message in the queue
   for the next loop iteration.

---

## 7. Sequence Diagrams

### 7.1 Window Creation

```
Ring 3                            Ring 0 (kernel)                BMO API
  │                                    │                            │
  │ 1. bmo_register_class(&cls)         │                            │
  │ ──────────────────────────────────►│                            │
  │                                    │ 2. validate class struct   │
  │                                    │   store in class table     │
  │ ◄──────────────────────────────────│   return classid           │
  │ 3. bmo_create_window_ex(            │                            │
  │      classid, "Hello", 6,           │                            │
  │      WS_OVERLAPPEDWINDOW, 0,        │                            │
  │      100,100,400,300)               │                            │
  │ ──────────────────────────────────►│                            │
  │                                    │ 4. alloc window slot       │
  │                                    │ 5. set parent, owner       │
  │                                    │ 6. alloc surface (offscreen│
  │                                    │    buffer)                 │
  │                                    │ 7. alloc DC (if CS_OWNDC)  │
  │                                    │ 8. add to Z-order (top)    │
  │                                    │ 9. set focused_window     │
  │                                    │ 10. send BMO_MSG_NCCREATE  │
  │                                    │     BMO_MSG_NCCALCSIZE     │
  │                                    │     BMO_MSG_CREATE         │
  │                                    │     (via wnd_proc call)    │
  │ ◄──────────────────────────────────│ 11. return wid             │
  │                                    │                            │
  │                                    │ 12. wnd_proc (Ring 3) runs │
  │     ... wnd_proc returns ...        │                            │
  │ 13. bmo_show_window(wid, SW_SHOW)   │                            │
  │ ──────────────────────────────────►│                            │
  │                                    │ 14. set WF_VISIBLE         │
  │                                    │ 15. invalidate             │
  │                                    │ 16. mark focused if needed │
  │ ◄──────────────────────────────────│ 17. return 0               │
```

### 7.2 Paint Cycle

```
Ring 3 (user wnd_proc)          Ring 0 (kernel)                  GPU/fb
  │                                  │                              │
  │ (kernel delivers BMO_MSG_PAINT   │                              │
  │  via bmo_dispatch_message)        │                              │
  │                                  │                              │
  │ 1. bmo_paint_begin(hwnd, &ps)     │                              │
  │ ────────────────────────────────►│                              │
  │                                  │ 2. validate window           │
  │                                  │ 3. intersect dirty rect      │
  │                                  │    with update region        │
  │                                  │ 4. return DC + ps            │
  │ ◄────────────────────────────────│                              │
  │                                  │                              │
  │ 5. bmo_fill_rect(dc, 0, 0, w, h, bg_color)                      │
  │ ────────────────────────────────►│                              │
  │                                  │ 6. clip-test                 │
  │                                  │ 7. write to surface          │
  │ ◄────────────────────────────────│                              │
  │                                  │                              │
  │ 8. bmo_draw_text(dc, 10, 10, "Hello", 5, fg)                    │
  │ ────────────────────────────────►│                              │
  │                                  │ 9. font blit to surface      │
  │ ◄────────────────────────────────│                              │
  │                                  │                              │
  │ 10. bmo_paint_end(hwnd, dc)       │                              │
  │ ────────────────────────────────►│                              │
  │                                  │ 11. validate update region   │
  │                                  │ 12. mark WM_PAINT delivered  │
  │                                  │ 13. mark surface dirty       │
  │ ◄────────────────────────────────│                              │
  │                                  │                              │
  │ (next APIC tick)                 │ 14. compositor tick          │
  │                                  │ ────────────────────────────►│
  │                                  │ 15. blit dirty surfaces      │
  │                                  │ 16. clear dirty              │
```

The **dirty region** is the union of all `bmo_invalidate` calls since
the last `bmo_paint_begin`. Multiple invalidations coalesce into a
bounding box (Win32 semantics). The compositor only blits the bounding
box, not the full window, so partial updates are cheap.

### 7.3 Message Dispatch (normal wnd_proc)

```
Ring 3 (main loop)             Ring 0 (kernel)                  BMO state
  │                                  │                              │
  │ 1. while(running) {               │                              │
  │      bmo_get_message(&msg);       │                              │
  │    }                              │                              │
  │ ────────────────────────────────►│                              │
  │                                  │ 2. lock-free pop from queue  │
  │                                  │ 3. if empty:                 │
  │                                  │      T.waiting = 1           │
  │                                  │      hlt / schedule          │
  │                                  │    else:                     │
  │                                  │      return msg              │
  │ ◄────────────────────────────────│                              │
  │                                  │                              │
  │ 4. bmo_translate_message(&msg)    │                              │
  │ ────────────────────────────────►│                              │
  │                                  │ 5. if KEYDOWN: insert CHAR   │
  │ ◄────────────────────────────────│                              │
  │                                  │                              │
  │ 6. bmo_dispatch_message(&msg)     │                              │
  │ ────────────────────────────────►│                              │
  │                                  │ 7. push wnd_proc frame       │
  │                                  │ 8. iretq to wnd_proc         │
  │                                  │                              │
  │ 9. (now in Ring 3 wnd_proc)       │                              │
  │    wnd_proc(hwnd, msg, wp, lp)    │                              │
  │    {                              │                              │
  │      switch (msg) {               │                              │
  │        case BMO_MSG_PAINT:        │                              │
  │          ... draw stuff ...       │                              │
  │          return 0;                │                              │
  │      }                            │                              │
  │    }                              │                              │
  │                                  │                              │
  │ 10. bmo_dispatch_trampoline:      │                              │
  │     syscall 0x198 (DISPATCH_RET)  │                              │
  │ ────────────────────────────────►│                              │
  │                                  │ 11. pop wnd_proc frame       │
  │                                  │ 12. return 0 to caller       │
  │ ◄────────────────────────────────│                              │
  │ 13. (back in user main loop)      │                              │
```

### 7.4 Mouse Click on a Button (a built-in control)

```
User              Window A (button's parent)  Button B (child)     Kernel
 │ click ↓          │                          │                    │
 │                   │                          │                    │
 │                   │  1. WM_LBUTTONDOWN        │                    │
 │                   │     (kernel posts msg)    │                    │
 │                   │ ◄────────────────────────────────────────────  │
 │                   │                          │                    │
 │                   │  2. bmo_dispatch_message  │                    │
 │                   │     → wnd_proc A runs    │                    │
 │                   │                          │                    │
 │                   │  3. A.wnd_proc:           │                    │
 │                   │     bmo_send_message(     │                    │
 │                   │       B, BMO_MSG_MOUSEDOWN,│                   │
 │                   │       x, y)              │                    │
 │                   │ ───────────────────────► │                    │
 │                   │                          │ 4. validate B,      │
 │                   │                          │    push frame       │
 │                   │                          │    iretq to B.proc  │
 │                   │                          │                    │
 │                   │                          │ 5. B.wnd_proc:      │
 │                   │                          │    (built-in)       │
 │                   │                          │    - set pushed=1  │
 │                   │                          │    - invalidate    │
 │                   │                          │    - bmo_post_msg( │
 │                   │                          │      A, COMMAND,   │
 │                   │                          │      BN_CLICKED)   │
 │                   │                          │ ──────────────────►│
 │                   │                          │                    │
 │                   │  6. (B.wnd_proc returns)  │                    │
 │                   │ ◄────────────────────── │                    │
 │                   │                          │                    │
 │                   │  7. A.wnd_proc continues  │                    │
 │                   │     (B's return value    │                    │
 │                   │      becomes              │                    │
 │                   │      bmo_send_message's   │                    │
 │                   │      return value)        │                    │
 │                   │                          │                    │
 │  release ↑         │                          │                    │
 │                   │  8. WM_LBUTTONUP         │                    │
 │                   │     (kernel posts)        │                    │
 │                   │ ◄────────────────────────────────────────────  │
 │                   │                          │                    │
 │                   │  9. bmo_send_message(B, BMO_MSG_MOUSEUP, ...)  │
 │                   │                          │                    │
 │                   │                          │ 10. B.proc:        │
 │                   │                          │   - pushed=0       │
 │                   │                          │   - invalidate     │
 │                   │                          │   - bmo_post_msg(  │
 │                   │                          │     A, BN_CLICKED) │
 │                   │                          │ ──────────────────►│
 │                   │                          │                    │
 │                   │  11. (next msg loop iter)  │                    │
 │                   │     bmo_get_message:      │                    │
 │                   │     → BMO_MSG_COMMAND,    │                    │
 │                   │       wparam=BN_CLICKED,  │                    │
 │                   │       lparam=B.id         │                    │
 │                   │ ◄────────────────────────────────────────────  │
 │                   │                          │                    │
 │                   │  12. A.wnd_proc handles    │                    │
 │                   │     BMO_MSG_COMMAND       │                    │
```

Key points:

- `bmo_send_message` is **synchronous**: A's wnd_proc is suspended
  while B's wnd_proc runs. A's call stack is preserved on A's user
  stack.
- The button's built-in wnd_proc is kernel-side. It is *not* a Ring 3
  function; the kernel dispatches `BMO_MSG_MOUSEDOWN/UP` to built-in
  controls directly without a Ring 0→3 transition.
- `BN_CLICKED` is `BMO_MSG_COMMAND` with `wparam` = (notification code
  << 16) | control_id, exactly as in Win32.

### 7.5 Window Destruction

```
Ring 3 (wnd_proc)              Ring 0 (kernel)              BMO state
  │                                  │                            │
  │ 1. user calls bmo_destroy_window │                            │
  │ ───────────────────────────────►│                            │
  │                                  │ 2. validate wid            │
  │                                  │ 3. mark WF_DESTROYED       │
  │                                  │ 4. remove from Z-order     │
  │                                  │ 5. reparent children to    │
  │                                  │    parent (or destroy)     │
  │                                  │ 6. send BMO_MSG_CLOSE      │
  │                                  │    (wnd_proc may veto)     │
  │                                  │ 7. if not vetoed:          │
  │                                  │    send BMO_MSG_DESTROY    │
  │                                  │ 8. destroy surface, DC     │
  │                                  │ 9. generation++            │
  │                                  │ 10. free window slot       │
  │                                  │ 11. if was focused:        │
  │                                  │     focus next top window  │
  │                                  │                            │
  │ (any wnd_proc-internal cleanup   │                            │
  │  happens in BMO_MSG_DESTROY      │                            │
  │  handler; e.g. close files,      │                            │
  │  free user data)                 │                            │
```

If a wnd_proc returns nonzero from `BMO_MSG_CLOSE`, the destroy is
cancelled. This is the Win32 pattern (used by Notepad to prompt
"Save changes?").

---

## 8. Edge Cases — What Win32/X11 Do

### 8.1 What happens on `BMO_MSG_PAINT` if you don't call `bmo_paint_begin`/`bmo_paint_end`?

**Win32:** If you don't call `BeginPaint`, the update region is **not
validated**. The kernel keeps sending `WM_PAINT` indefinitely
(actually, it coalesces — but you still get the message on every
dispatch loop iteration). If you don't call `EndPaint`, the update
region stays invalid; you also can't draw (no DC). If you draw *outside*
BeginPaint/EndPaint, the result is undefined and may not appear on
screen.

**X11:** There is no equivalent; the server sends `Expose` events, and
the client must call `XClearArea` or just draw. The server doesn't
"expect" any matching "EndPaint".

**Wayland:** Drawing is committed via `wl_surface::attach` +
`wl_surface::commit`. There's no "EndPaint" — the commit IS the end.

**BMO v2.0:** Mirrors Win32. `bmo_paint_begin` is the only way to
get a DC and validate the update region. If you receive
`BMO_MSG_PAINT` and do *not* call `bmo_paint_begin`:

- The update region is *not* validated.
- The kernel will re-post `BMO_MSG_PAINT` (low priority) on the next
  idle cycle.
- Calling any `bmo_draw_*` syscall without an active paint DC returns
  `BMO_ERR_BAD_DC`.
- Calling `bmo_paint_end` without `bmo_paint_begin` returns
  `BMO_ERR_BAD_DC`.

This means a buggy wnd_proc that ignores `BMO_MSG_PAINT` enters a
busy-loop of paint messages, eating CPU. The wnd_proc *should* still
call `bmo_paint_begin` + `bmo_paint_end` with empty draws to clear the
update region.

### 8.2 What happens on window destruction with queued messages?

**Win32:**

- `WM_DESTROY` is sent.
- Any `WM_QUIT` already in the queue stays.
- `WM_CLOSE`, `WM_PAINT`, etc. targeting the destroyed window are
  silently dropped by `GetMessage` (the `hwnd` filter in `GetMessage`
  still matches them, but `DispatchMessage` on a destroyed HWND returns
  0 — actually it faults! The user must `GetMessage(&msg, NULL, 0, 0)`
  to drain).
- In practice: a wnd_proc that processes `WM_DESTROY` by calling
  `PostQuitMessage(0)` causes `GetMessage` to return 0 on the next
  iteration, ending the loop cleanly. Messages in the queue from the
  destroyed window are dropped.

**X11:**

- `DestroyNotify` is sent to interested clients.
- Events targeting the destroyed window are still sent (server has
  already enqueued them before processing the destroy request) but
  most Xlib code checks `XFilterEvent` and silently drops them.

**BMO v2.0:** The kernel scans the owning thread's queue on
`bmo_destroy_window` and removes all messages where `target ==
destroyed_wid` (except `BMO_MSG_DESTROY` itself, which is delivered
first). The `overflow_count` is incremented by the number of dropped
messages for diagnostic purposes. `BMO_MSG_QUIT` (which has no target)
is *not* dropped.

### 8.3 What happens to messages posted to a thread that is exiting?

**Win32:** Messages posted to a thread that has called
`ExitThread` are silently dropped.

**BMO v2.0:** Same. `bmo_post_thread_message` to an exited thread
returns `BMO_ERR_NO_WINDOW` (using "window" generically). `bmo_post_message`
to a window whose owning thread has exited fails similarly.

### 8.4 What if the queue is full?

**Win32:** `PostMessage` returns 0; `GetLastError()` is
`ERROR_NOT_ENOUGH_QUOTA`. The message is lost.

**BMO v2.0:** `bmo_post_message` returns `BMO_ERR_QUEUE_FULL`. The
caller is expected to retry or coalesce. The kernel sets
`queue.overflow_count` for diagnostics. A *high* `overflow_count` is a
strong signal that a wnd_proc is too slow.

### 8.5 What if the wnd_proc calls `bmo_destroy_window` on its own window during `BMO_MSG_PAINT`?

**Win32:** Returns 0 from `DestroyWindow` (with `GetLastError() ==
ERROR_INVALID_PARAMETER`). The window continues to exist.

**BMO v2.0:** Returns `BMO_ERR_REENTRANT`. The window is *not*
destroyed. The wnd_proc should defer destruction by posting
`BMO_MSG_CLOSE` to itself.

### 8.6 What if two threads call `bmo_set_focus` simultaneously?

**Win32:** The kernel serializes; the last call wins. There is no
defined ordering.

**BMO v2.0:** Same. The wnd_proc call is the only place that can
mutate focus; cross-thread focus changes happen via
`bmo_post_message(B, BMO_MSG_SETFOCUS, ...)` (i.e. async). Direct
`bmo_set_focus` from a non-owning thread returns `BMO_ERR_PERM` (the
window belongs to another thread's message queue).

### 8.7 What if the wnd_proc blocks (e.g. infinite loop)?

**Win32:** The system replaces the window with a "ghost" (Not
Responding) after ~5 seconds. The user can drag or close the ghost
but cannot interact with the real window.

**BMO v2.0:** v2.0 does *not* implement hung-window detection. The
process is considered stuck. The watchdog (if enabled) can kill it
after a configurable timeout (default: 30 s). v2.1 will add a
per-thread deadline.

### 8.8 What about ring-3 → ring-3 reentrancy via `bmo_send_message`?

If a wnd_proc calls `bmo_send_message(B, msg, ...)`, the kernel pushes
a *second* iretq frame onto the per-thread kernel stack, so we now
have:

```
[user stack: A's call frames]
[kernel stack: B's iretq frame]
[kernel stack: A's iretq frame]
```

When B returns, B's frame is popped and we resume A's wnd_proc. This
is exactly how Win32's `SendMessage` works and is essential for
controls.

Max depth: 5. Deeper returns `BMO_ERR_REENTRANT`.

### 8.9 What about modal windows?

A modal window is a regular top-level window that, while visible,
disables input to all other windows in its thread. The kernel
implements this by:

1. `bmo_begin_modal(modal_wid)` — sets `modal_wid` as the active
   "modal root".
2. While `modal_wid != 0`, all input events are routed *only* to
   `modal_wid` (and its children). Other windows get no input.
3. `bmo_end_modal(modal_wid)` clears it.

This is a per-thread concept (matches Win32). Modal windows across
threads are out of scope for v2.0.

### 8.10 What if a wnd_proc calls `bmo_post_message` to itself inside `BMO_MSG_PAINT`?

**Win32:** The message is enqueued; the wnd_proc sees it after the
current paint completes.

**BMO v2.0:** Same. The message is appended to the queue and
dispatched on the next `bmo_get_message` iteration. It will *not*
interrupt the current paint.

### 8.11 What about `bmo_set_timer` with timeout 0?

**Win32:** A `SetTimer(hwnd, id, 0, NULL)` posts a `WM_TIMER` *once*,
immediately.

**BMO v2.0:** Same. `bmo_set_timer(wid, id, 0)` is equivalent to
`bmo_post_message(wid, BMO_MSG_TIMER, id, 0)`.

### 8.12 What about Z-order when a window is destroyed mid-paint?

The compositor tick reads the Z-order under a `wm_lock` spinlock.
The wnd_proc does not hold this lock. If a window is destroyed
between the compositor's read of the Z-list and its blit, the
compositor skips it (checks `WF_DESTROYED` before blit). No crash.

### 8.13 What if a built-in class is unregistered while a window uses it?

**Win32:** The window continues to use the class until destroyed.
Unregistering a class that has live windows is allowed but the class
is actually freed only when the last window is destroyed.

**BMO v2.0:** `bmo_unregister_class` returns `BMO_ERR_BUSY` if any
window still references the class. The user must destroy all windows
first.

---

## 9. References

### 9.1 Primary documentation

- **Microsoft Win32 USER32 reference** —
  https://learn.microsoft.com/en-us/windows/win32/api/winuser/
  (CreateWindowExW, BeginPaint, About Messages and Message Queues,
  WM_PAINT, etc.)
- **Xlib Programming Manual** (Christophe Tronche) —
  https://tronche.com/gui/x/xlib/
  (event types, event processing, window attributes, event masks)
- **Wayland Protocol Specification** —
  https://wayland.freedesktop.org/docs/html/
  (Appendix A: protocol spec, Appendix B: client API)
- **Linux DRM/KMS driver documentation** —
  https://dri.freedesktop.org/docs/drm/gpu/drm-kms.html
- **Apple AppKit / Cocoa** — developer.apple.com/documentation/appkit
  (NSApplication, NSWindow, NSView, NSEvent, NSResponder)
- **GTK 4 Documentation** — docs.gtk.org/gtk4/
  (GMainContext, GMainLoop, GdkEvent, gtk_widget_class)
- **Linux evdev / libinput** — here: input event codes from
  https://www.kernel.org/doc/Documentation/input/event-codes.txt

### 9.2 Implementation references (open source)

- **dwl** (suckless Wayland compositor, ~3 kLOC) —
  https://codeberg.org/dwl/dwl
  Reference for minimal Wayland compositor design.
- **sway** (i3-compatible Wayland compositor) —
  https://github.com/swaywm/sway
- **Hyprland** (dynamic tiling Wayland compositor) —
  https://github.com/hyprwm/Hyprland
- **XMonad** (tiling X11 WM in Haskell) — https://xmonad.org/
  Reference for tiling WM algorithms.
- **Wine USER32 implementation** (how Win32 is reimplemented on
  Linux) — https://gitlab.winehq.org/wine/wine/-/tree/master/dlls/user32
  Particularly `win.c`, `message.c`, `wndproc.c`, `dce.c`.
- **ReactOS USER32** (cleaner reimplementation of Win32) —
  https://github.com/reactos/reactos/tree/master/win32ss/user/ntuser
- **KWin** (KDE's Wayland/X11 compositor) — invent.kde.org/plasma/kwin
  Reference for advanced window management effects.
- **wayland-rs** (Rust Wayland protocol implementation) —
  https://github.com/Smithay/wayland-rs
  Reference for type-safe event/req dispatch in Rust.
- **Smithay** (Rust Wayland compositor library) —
  https://github.com/Smithay/smithay

### 9.3 Internal FastOS references

- Existing `bmo_api` (Ring 0) — `kernel/src/bmo_api/{mod,window,message,manager,widget}.rs`
  (max 16 windows, single control thread, will be replaced by the v2
  module).
- Existing syscall entry — `kernel/src/arch/syscall_entry.rs`
  (LSTAR setup, InterruptFrame, syscall return via iretq).
- Existing `bmo_abi` — `kernel/src/bmo_abi/`
  (Ring 3 ABI design: handles, status codes, type descriptors, vtable,
  closures). The BMO API reuses `bmo_abi::fundamentals::handle` and
  `bmo_abi::fundamentals::status` for handle and error types.
- Existing desktop (Ring 0 desktop loop) — `kernel/src/desktop/`
  (will be subsumed by `bmo_api/wm.rs`).

### 9.4 Books

- Petzold, *Programming Windows*, 5th ed. — the canonical Win32
  reference. The wnd_proc and message-pump model are explained
  in detail in chapters 3–5.
- Rosen, *Windows Internals*, 7th ed. — Part 2 covers USER32 and
  win32k.sys in depth.
- Friedl, *Xlib Reference Manual* — the X11 equivalent of Petzold.

---

## 10. Open Questions / Future Work (v2.1+)

These are explicitly **out of scope** for v2.0 but are listed so the
design doesn't paint itself into a corner.

1. **HiDPI / scaling** — surfaces are 1:1 with framebuffer pixels.
   v2.1 will add a `bmo_dpi` per-monitor value and a scale factor
   on each window.
2. **Multi-monitor** — `bmo_get_monitor_info(monitor_id, ...)` and a
   per-monitor origin. v2.0 is single-monitor.
3. **GPU acceleration** — `bmo_create_gpu_surface(wid)` returns a
   surface backed by GPU memory. The compositor uses a GPU blit
   instead of a memcpy. Out of scope for v2.0.
4. **Drag-and-drop** — `OleInitialize`-style. The 0x190..0x19F range
   is reserved.
5. **Accessibility** — `bmo_register_accessibility_provider` and
   `BMO_MSG_*_ACCESSIBILITY`. Not in v2.0.
6. **IME / composition** — `bmo_ime_*`. v2.0 assumes Latin-1 input.
7. **Per-thread `bmo_wnd_proc_depth` enforcement** — a soft limit;
   a hard 5-deep limit is good enough for v2.0.
8. **Clipboard format negotiation** — v2.0 supports one format
   (`BMO_CF_TEXT` = `text/plain;charset=utf-8`). Multi-format is v2.1.
9. **Cross-process windows** — child windows can have a different
   process as parent. v2.0 disallows this; a child is always owned
   by the parent's process.
10. **Anti-aliasing** — v2.0 has no AA primitives. The compositor
    will do cheap 2x2 box filter AA on text. Native AA is v2.1.

---

## 11. Implementation Sketch (file-level)

For the engineer who picks this up:

```
bmo_api/
├── mod.rs           # init() + BMO_STATE global
├── handle.rs        # HANDLE_TABLE[1024], alloc, lookup, free
├── window.rs        # WINDOWS[256], parent/child/Z-order manipulation
├── class.rs         # CLASSES[32], registration, default wnd_proc
├── message.rs       # bmo_msg, BMO_MSG_* enum, encode/decode
├── queue.rs         # bmo_msg_queue SPSC ring
├── event.rs         # coalesce, dedup, paint-region merge
├── draw.rs          # DC state, primitives, font blit
├── surface.rs       # surface table, mmap, flip
├── input.rs         # PS/2 + USB HID input → events
├── input_thread.rs  # kernel thread for input
├── syscall.rs       # 0x100..0x1FF dispatcher
├── wm.rs            # Z-order, focus, drag/resize, snap
├── timer.rs         # hierarchical timer wheel
├── cursor.rs        # cursor sprite management
├── paint_compositor.rs  # periodic blit, vsync
└── compat/
    └── v1.rs        # shim for old bmo_api calls

bmo_abi/             # (existing, mostly unchanged)
├── ...
└── dispatch_table.rs   # NEW: per-process syscall stub table

sched/thread.rs      # add: per-thread kernel_stack, GUI-thread flag
arch/syscall_entry.rs # extend 0x100..=0x1FF to bmo_api::syscall::dispatch
                      # extend 0x198 = BMO_DISPATCH_RETURN

bmo_runtime/         # NEW: ring 3 library (libbmo.a or .so)
├── trampoline.S     # bmo_dispatch_trampoline, bmo_default_wnd_proc shim
├── msg.rs           # user-mode message struct, ring buffer wait
├── class.rs         # safe wrappers for register_class
├── window.rs        # WindowGuard (RAII), title, size
├── paint.rs         # PaintDC, draw_text, draw_image
├── input.rs         # blocking get_message
└── lib.rs
```

Total estimated size: ~6,000–8,000 lines of Rust for the kernel side
and ~1,500 lines for the Ring 3 ABI lib. Comparable to Wine's
`dlls/user32` minus the multi-process complexity.

---

*End of BMO API v2.0 design spec.*
