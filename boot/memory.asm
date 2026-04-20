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

    ; ── Try INT 15h AX=E801h ───────────────────────────────────────────
    mov ax, 0xE801
    int 0x15
    jc .e801_fail
    cmp ax, 0
    je .try_cx
    jmp .e801_build

.try_cx:
    mov ax, cx
    mov bx, dx
    cmp ax, 0
    je .e801_fail

.e801_build:
    mov dword [MEMORY_MAP_ADDR], 0
    mov di, MEMORY_MAP_ADDR + 4

    ; Entry 0: Conventional memory 0 - 639KB
    mov dword [di +  0], 0x00000000
    mov dword [di +  4], 0x00000000
    mov dword [di +  8], 0x0009FC00
    mov dword [di + 12], 0x00000000
    mov dword [di + 16], 1
    mov dword [di + 20], 0
    add di, 24
    inc dword [MEMORY_MAP_ADDR]

    ; Entry 1: Extended memory 1MB to 1MB+AX*1024
    movzx eax, ax
    shl eax, 10
    mov dword [di +  0], 0x00100000
    mov dword [di +  4], 0x00000000
    mov dword [di +  8], eax
    mov dword [di + 12], 0x00000000
    mov dword [di + 16], 1
    mov dword [di + 20], 0
    add di, 24
    inc dword [MEMORY_MAP_ADDR]

    ; Entry 2: Memory above 16MB (if BX > 0)
    cmp bx, 0
    je .e801_done

    movzx ebx, bx
    shl ebx, 16
    mov dword [di +  0], 0x01000000
    mov dword [di +  4], 0x00000000
    mov dword [di +  8], ebx
    mov dword [di + 12], 0x00000000
    mov dword [di + 16], 1
    mov dword [di + 20], 0
    add di, 24
    inc dword [MEMORY_MAP_ADDR]

.e801_done:
    popad
    ret

.e801_fail:
    ; Fallback: hardcode conservative map (256MB)
    mov dword [MEMORY_MAP_ADDR], 0
    mov di, MEMORY_MAP_ADDR + 4

    mov dword [di +  0], 0x00000000
    mov dword [di +  4], 0x00000000
    mov dword [di +  8], 0x000A0000
    mov dword [di + 12], 0x00000000
    mov dword [di + 16], 1
    mov dword [di + 20], 0
    add di, 24
    inc dword [MEMORY_MAP_ADDR]

    mov dword [di +  0], 0x00100000
    mov dword [di +  4], 0x00000000
    mov dword [di +  8], 0x0FF00000
    mov dword [di + 12], 0x00000000
    mov dword [di + 16], 1
    mov dword [di + 20], 0
    add di, 24
    inc dword [MEMORY_MAP_ADDR]

    popad
    ret
