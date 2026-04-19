; ============================================================================
; FastOS Memory Detection — E801h + Fallback
; ============================================================================
; E820 hangs on this UEFI CSM (Ryzen 5 5600X motherboard, USB boot).
; Use INT 15h AX=E801h instead — simpler, more compatible.
;
; If E801h also fails, hardcode a conservative memory map.
; The kernel can refine this later via ACPI tables.
;
; Output: memory map at 0x8000 in same format kernel expects:
;   [0x8000]      = u32 count
;   [0x8004+n*24] = { base_u64, length_u64, type_u32, attrs_u32 }
; ============================================================================

[BITS 16]

MEMORY_MAP_ADDR  equ 0x8000
MEMORY_MAP_COUNT equ 0x8000    ; First 4 bytes = count

detect_memory_e820:
    pushad

    ; ── Try INT 15h AX=E801h ────────────────────────────────────────────
    ; Returns extended memory in two ranges:
    ;   AX/CX = memory between 1MB-16MB in 1KB blocks
    ;   BX/DX = memory above 16MB in 64KB blocks
    ; More compatible than E820 on UEFI CSM.

    mov ax, 0xE801
    int 0x15
    jc .e801_fail               ; CF = not supported
    cmp ax, 0                   ; Some BIOS return 0 in AX
    je .try_cx                  ; Try CX instead

    ; AX = 1KB blocks between 1MB-16MB
    ; BX = 64KB blocks above 16MB
    jmp .e801_build

.try_cx:
    ; Some BIOS put the result in CX/DX instead of AX/BX
    mov ax, cx
    mov bx, dx
    cmp ax, 0
    je .e801_fail               ; Both AX and CX are 0 — fail

.e801_build:
    ; AX = KB between 1MB-16MB, BX = 64KB blocks above 16MB
    ; Build a memory map the kernel understands

    mov dword [MEMORY_MAP_ADDR], 0  ; Reset count
    mov di, MEMORY_MAP_ADDR + 4

    ; ── Entry 0: Conventional memory 0x00000 - 0x9FC00 (639KB) ────────
    mov dword [di +  0], 0x00000000  ; base low
    mov dword [di +  4], 0x00000000  ; base high
    mov dword [di +  8], 0x0009FC00  ; length low (639KB)
    mov dword [di + 12], 0x00000000  ; length high
    mov dword [di + 16], 1           ; type = usable
    mov dword [di + 20], 0           ; attrs
    add di, 24
    inc dword [MEMORY_MAP_ADDR]

    ; ── Entry 1: Extended memory 1MB to 1MB+AX*1024 ──────────────────
    ; AX still holds KB between 1MB-16MB
    movzx eax, ax
    shl eax, 10                      ; AX * 1024 = bytes
    mov dword [di +  0], 0x00100000  ; base = 1MB
    mov dword [di +  4], 0x00000000
    mov dword [di +  8], eax         ; length = AX KB
    mov dword [di + 12], 0x00000000
    mov dword [di + 16], 1           ; type = usable
    mov dword [di + 20], 0
    add di, 24
    inc dword [MEMORY_MAP_ADDR]

    ; ── Entry 2: Memory above 16MB (if BX > 0) ──────────────────────
    cmp bx, 0
    je .e801_done

    movzx ebx, bx
    shl ebx, 16                      ; BX * 64KB = bytes
    mov dword [di +  0], 0x01000000  ; base = 16MB
    mov dword [di +  4], 0x00000000
    mov dword [di +  8], ebx         ; length low
    mov dword [di + 12], 0x00000000
    mov dword [di + 16], 1           ; type = usable
    mov dword [di + 20], 0
    add di, 24
    inc dword [MEMORY_MAP_ADDR]

.e801_done:
    popad
    ret

    ; ── Fallback: hardcoded conservative map ────────────────────────────
    ; If E801h fails, assume at least 256MB (safe for Ryzen 5 5600X).
.e801_fail:
    mov dword [MEMORY_MAP_ADDR], 0
    mov di, MEMORY_MAP_ADDR + 4

    ; Entry 0: Conventional 0 - 640KB
    mov dword [di +  0], 0x00000000
    mov dword [di +  4], 0x00000000
    mov dword [di +  8], 0x000A0000  ; 640KB
    mov dword [di + 12], 0x00000000
    mov dword [di + 16], 1
    mov dword [di + 20], 0
    add di, 24
    inc dword [MEMORY_MAP_ADDR]

    ; Entry 1: Extended 1MB - 256MB (conservative for 16GB+ system)
    mov dword [di +  0], 0x00100000
    mov dword [di +  4], 0x00000000
    mov dword [di +  8], 0x0FF00000  ; 255MB
    mov dword [di + 12], 0x00000000
    mov dword [di + 16], 1
    mov dword [di + 20], 0
    add di, 24
    inc dword [MEMORY_MAP_ADDR]

    popad
    ret
