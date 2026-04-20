; ============================================================================
; FastOS Memory Detection (INT 15h, E820)
; ============================================================================
; Stores memory map at MEMORY_MAP_ADDR for the kernel.
; Each entry: base(u64) + length(u64) + type(u32) + attrs(u32) = 24 bytes
; ============================================================================

[BITS 16]

MEMORY_MAP_ADDR  equ 0x8000
MEMORY_MAP_COUNT equ 0x8000    ; First 4 bytes = count

detect_memory_e820:
    pushad
    mov dword [MEMORY_MAP_ADDR], 0
    xor ebx, ebx
    mov di, MEMORY_MAP_ADDR + 4

.e820_loop:
    mov eax, 0x0000E820
    mov edx, 0x534D4150         ; 'SMAP'
    mov ecx, 24
    int 0x15
    jc .e820_done
    cmp eax, 0x534D4150
    jne .e820_done

    add di, 24
    inc dword [MEMORY_MAP_ADDR]

    test ebx, ebx
    jz .e820_done
    jmp .e820_loop

.e820_done:
    popad
    ret
