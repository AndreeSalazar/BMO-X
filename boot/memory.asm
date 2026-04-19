; ============================================================================
; FastOS Memory Detection (INT 15h, E820) — Hardened
; ============================================================================
; Stores memory map at MEMORY_MAP_ADDR for the kernel.
; Each entry: base(u64) + length(u64) + type(u32) + attrs(u32) = 24 bytes
;
; HARDENING: Max 64 entries to prevent infinite loops on buggy UEFI CSM.
; If INT 15h fails entirely, returns count=0 (kernel must handle this).
; ============================================================================

[BITS 16]

MEMORY_MAP_ADDR  equ 0x8000
MEMORY_MAP_COUNT equ 0x8000    ; First 4 bytes = count
MAX_E820_ENTRIES equ 64        ; Safety limit

detect_memory_e820:
    pushad
    mov dword [MEMORY_MAP_ADDR], 0
    xor ebx, ebx
    mov di, MEMORY_MAP_ADDR + 4
    xor bp, bp                  ; BP = entry counter

.e820_loop:
    mov eax, 0x0000E820
    mov edx, 0x534D4150         ; 'SMAP'
    mov ecx, 24
    int 0x15
    jc .e820_done               ; CF set = end or error

    cmp eax, 0x534D4150         ; Verify SMAP signature
    jne .e820_done

    ; Valid entry — count it
    add di, 24
    inc dword [MEMORY_MAP_ADDR]
    inc bp

    ; Safety: stop if we hit max entries
    cmp bp, MAX_E820_ENTRIES
    jge .e820_done

    ; EBX=0 means last entry
    test ebx, ebx
    jz .e820_done
    jmp .e820_loop

.e820_done:
    popad
    ret
