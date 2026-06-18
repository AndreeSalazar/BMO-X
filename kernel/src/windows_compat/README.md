# Windows Compatibility Layer for FastOS/BMO

## Philosophy

FastOS is **not** trying to be Windows. It's trying to be the best bare-metal OS possible.
But many users have Windows apps they need to run. This layer provides **transparent PE
compatibility** — Windows .exe/.dll files load and run without modification.

**The BEF format is superior.** Native BEF apps get full capabilities, BLAKE3 integrity,
and zero translation overhead. PE apps go through this compatibility layer, which adds
translation overhead and limited API coverage.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    PE Application (.exe)                        │
├─────────────────────────────────────────────────────────────────┤
│  Imports: kernel32!ExitProcess, user32!CreateWindowEx, etc.     │
└──────────────────────┬──────────────────────────────────────────┘
                       │ PE import resolution
                       ▼
┌─────────────────────────────────────────────────────────────────┐
│              windows_compat (Win32 Shim Layer)                  │
├─────────────────────────────────────────────────────────────────┤
│  kernel32/ → BMO syscalls (process, memory, file, time)         │
│  user32/   → BMO desktop (window, message, input)               │
│  gdi32/    → BMO framebuffer (text, rect, bitmap)               │
│  ntdll/    → BMO low-level (direct syscall)                     │
│  msvcrt/   → BMO alloc + libc shim                              │
│  advapi32/ → BMO security (registry → config files)             │
│  shell32/  → BMO desktop (shell, paths, icons)                  │
│  comctl32/ → BMO desktop (common controls)                      │
│  ws2_32/   → BMO net (BSD sockets → BMO net)                    │
│  d3d12/    → BMO graphics (D3D12 → BareX GPU)                   │
└──────────────────────┬──────────────────────────────────────────┘
                       │ BMO ABI calls
                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    BMO Kernel (Ring 0)                          │
├─────────────────────────────────────────────────────────────────┤
│  Syscalls, Scheduler, Memory, VFS, Desktop, Network, GPU        │
└─────────────────────────────────────────────────────────────────┘
```

## Directory Structure

```
windows_compat/
├── mod.rs                    ← Root module, init(), version info
├── README.md                 ← This file
│
├── kernel32/                 ← kernel32.dll compatibility
│   ├── mod.rs                ← Module root, function dispatch
│   ├── process.rs            ← ExitProcess, CreateProcess, GetCurrentProcess, etc.
│   ├── memory.rs             ← VirtualAlloc, VirtualFree, HeapCreate/Alloc/Free
│   ├── thread.rs             ← CreateThread, ExitThread, TLS, CriticalSection
│   ├── file.rs               ← CreateFile, ReadFile, WriteFile, FindFirstFile
│   ├── time.rs               ← Sleep, GetTickCount, QueryPerformanceCounter
│   ├── module.rs             ← GetModuleHandle, LoadLibrary, GetProcAddress
│   ├── string.rs             ← lstrlen, lstrcpy, MultiByteToWideChar, etc.
│   └── env.rs                ← GetEnvironmentStrings, GetCommandLine
│
├── user32/                   ← user32.dll compatibility
│   ├── mod.rs                ← Module root
│   ├── window.rs             ← RegisterClass, CreateWindow, DestroyWindow
│   ├── message.rs            ← GetMessage, PeekMessage, DispatchMessage
│   ├── paint.rs              ← BeginPaint, EndPaint, InvalidateRect
│   ├── input.rs              ← GetKeyboardState, ToAscii, MapVirtualKey
│   ├── cursor.rs             ← LoadCursor, ShowCursor, SetCursorPos
│   └── metrics.rs            ← GetSystemMetrics, SystemParametersInfo
│
├── gdi32/                    ← gdi32.dll compatibility
│   ├── mod.rs                ← Module root
│   ├── device.rs             ← CreateDC, CreateCompatibleDC, DeleteDC
│   ├── bitmap.rs             ← CreateBitmap, CreateCompatibleBitmap, BitBlt
│   ├── text.rs               ← TextOut, DrawText, GetTextExtentPoint
│   ├── font.rs               ← CreateFont, SelectObject, GetCharWidth
│   ├── brush.rs              ← CreateSolidBrush, FillRect, PatBlt
│   ├── pen.rs                ← CreatePen, LineTo, Rectangle
│   └── region.rs             ← CreateRectRgn, CombineRgn, SelectClipRgn
│
├── msvcrt/                   ← CRT compatibility (msvcrt.dll / vcruntime140.dll)
│   ├── mod.rs                ← Module root
│   ├── memory.rs             ← malloc, free, realloc, calloc, new, delete
│   ├── string.rs             ← strlen, strcmp, strcpy, strncpy, strcat, etc.
│   ├── stdio.rs              ← printf, sprintf, fprintf, fopen, fclose, etc.
│   ├── stdlib.rs             ← exit, atexit, atoi, atol, getenv, system
│   ├── math.rs               ← sin, cos, sqrt, floor, ceil, pow, log
│   ├── init.rs               ← _initterm, _initterm_e, __security_init_cookie
│   └── exception.rs          ← __CxxFrameHandler3, _set_se_translator
│
├── advapi32/                 ← advapi32.dll compatibility
│   ├── mod.rs                ← Module root
│   ├── registry.rs           ← RegOpenKey, RegQueryValue, RegCreateKey, etc.
│   └── security.rs           ← CryptAcquireContext, CryptGenRandom
│
├── shell32/                  ← shell32.dll compatibility
│   ├── mod.rs                ← Module root
│   ├── path.rs               ← SHGetFolderPath, SHGetSpecialFolderPath
│   └── execute.rs            ← ShellExecute, ShellExecuteEx
│
├── comctl32/                 ← comctl32.dll compatibility
│   └── mod.rs                ← InitCommonControlsEx
│
├── ole32/                    ← COM/OLE compatibility
│   ├── mod.rs                ← CoInitialize, CoCreateInstance, CoUninitialize
│   └── memory.rs             ← CoTaskMemAlloc, CoTaskMemFree
│
├── seh/                      ← SEH/VEH exception handling
│   ├── mod.rs                ← Exception dispatch
│   ├── unwind.rs             ← RUNTIME_FUNCTION, .pdata parsing
│   ├── handler.rs            ← ExceptionHandler, VEH chain
│   └── cookie.rs             ← __security_init_cookie, GS handler
│
└── api_map.rs                ← Master Win32 → BMO API mapping table
```

## Priority Levels

### P0 — Run a C "Hello World"
- msvcrt: malloc, free, printf, exit, _initterm, __security_init_cookie
- kernel32: ExitProcess, GetCommandLineA, GetStdHandle, WriteConsoleA
- SEH: __GSHandlerCheck, __CxxFrameHandler3

### P1 — Run console apps
- kernel32: CreateFile, ReadFile, WriteFile, CloseHandle, GetFileSize
- kernel32: CreateThread, ExitThread, Sleep, WaitForSingleObject
- kernel32: VirtualAlloc, VirtualFree, HeapCreate, HeapAlloc, HeapFree
- msvcrt: Full CRT (stdio, stdlib, string, math)

### P2 — Run GUI apps (notepad, etc.)
- user32: RegisterClassEx, CreateWindowEx, DefWindowProc, DestroyWindow
- user32: GetMessage, TranslateMessage, DispatchMessage, PostQuitMessage
- gdi32: CreateFont, TextOut, GetDC, ReleaseDC, BeginPaint, EndPaint
- user32: SetWindowText, GetWindowText, MessageBox

### P3 — Run complex apps
- shell32, comctl32, comdlg32, advapi32
- COM/OLE basics
- D3D12/DXGI (via BareX)
- Networking (ws2_32 → BMO net)

## Sources

- **Wine** (https://github.com/wine-mirror/wine) — 30 years of Win32 API analysis
- **ReactOS** (https://github.com/reactos/reactos) — Open-source Windows reimplementation
- **ntdoc** (https://github.com/nbaksalyak/ntdoc) — Windows NT internals documentation
- **MSDN Win32 API Reference** — Official Microsoft documentation
- **Win32 API Coverage** — Community mapping of API functions to DLLs

## Status

| DLL | Thunks | Implemented | Coverage |
|-----|--------|-------------|----------|
| kernel32.dll | 38 | 0 real | 0% |
| user32.dll | 13 | 0 real | 0% |
| gdi32.dll | 0 | 0 | 0% |
| msvcrt.dll | 0 | 0 | 0% |
| ntdll.dll | 12 | 0 real | 0% |
| advapi32.dll | 0 | 0 | 0% |
| shell32.dll | 0 | 0 | 0% |
| comctl32.dll | 0 | 0 | 0% |
| ole32.dll | 0 | 0 | 0% |
| **TOTAL** | **63** | **0** | **0%** |

**Note:** These are the DLLs in `pe_thunks.rs`. The thunks exist but map to silent stubs.
The `windows_compat/` module will provide real implementations.
