; ============================================================================
; FastOS Memory Detection — Hardcoded (MSI B550 CSM workaround)
; ============================================================================
; INT 15h E820 hangs, E801h triple-faults on this board's CSM.
; Hardcode a conservative memory map. Kernel refines via ACPI later.
;
; Output: memory map at 0x8000:
;   [0x8000]      = u32 count
;   [0x8004+n*24] = { base_u64, length_u64, type_u32, attrs_u32 }
; ============================================================================

[BITS 16]

MEMORY_MAP_ADDR  equ 0x8000
MEMORY_MAP_COUNT equ 0x8000    ; First 4 bytes = count

detect_memory_e820:
    pushad

    mov dword [MEMORY_MAP_ADDR], 0
    mov di, MEMORY_MAP_ADDR + 4

    ; Entry 0: Conventional 0 - 640KB
    mov dword [di +  0], 0x00000000
    mov dword [di +  4], 0x00000000
    mov dword [di +  8], 0x000A0000
    mov dword [di + 12], 0x00000000
    mov dword [di + 16], 1
    mov dword [di + 20], 0
    add di, 24
    inc dword [MEMORY_MAP_ADDR]

    ; Entry 1: Extended 1MB - 256MB
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
