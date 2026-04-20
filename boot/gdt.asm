; ============================================================================
; FastOS GDT — Global Descriptor Table (32-bit + 64-bit)
; ============================================================================

[BITS 16]

; ── 32-bit GDT (protected mode transition) ──────────────────────────────────

gdt32_start:
    dq 0x0000000000000000           ; Null descriptor

gdt32_code:
    dw 0xFFFF                       ; Limit low
    dw 0x0000                       ; Base low
    db 0x00                         ; Base mid
    db 10011010b                    ; Access: P=1, Ring0, Code, Exec/Read
    db 11001111b                    ; Flags: 4KB gran, 32-bit + Limit high
    db 0x00                         ; Base high

gdt32_data:
    dw 0xFFFF
    dw 0x0000
    db 0x00
    db 10010010b                    ; Access: P=1, Ring0, Data, R/W
    db 11001111b
    db 0x00

gdt32_end:

gdt32_descriptor:
    dw gdt32_end - gdt32_start - 1
    dd gdt32_start

GDT32_CODE_SEG equ gdt32_code - gdt32_start   ; 0x08
GDT32_DATA_SEG equ gdt32_data - gdt32_start   ; 0x10

; ── 64-bit GDT (long mode) ──────────────────────────────────────────────────

gdt64_start:
    dq 0x0000000000000000           ; Null descriptor

gdt64_code:
    dw 0x0000                       ; Limit (ignored in long mode)
    dw 0x0000                       ; Base low
    db 0x00                         ; Base mid
    db 10011010b                    ; Access: P=1, Ring0, Code, Exec/Read
    db 00100000b                    ; Flags: Long mode (bit 5)
    db 0x00                         ; Base high

gdt64_data:
    dw 0x0000
    dw 0x0000
    db 0x00
    db 10010010b                    ; Access: P=1, Ring0, Data, R/W
    db 00000000b
    db 0x00

gdt64_end:

gdt64_descriptor:
    dw gdt64_end - gdt64_start - 1
    dq gdt64_start

GDT64_CODE_SEG equ gdt64_code - gdt64_start   ; 0x08
GDT64_DATA_SEG equ gdt64_data - gdt64_start   ; 0x10
