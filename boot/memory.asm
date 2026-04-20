; ============================================================================
; FastOS Memory Detection — Hardcoded (MSI B550 CSM workaround)
; ============================================================================
; INT 15h E820 hangs, E801h triple-faults on this board's CSM.
; Pure hardcoded map using only 16-bit operations. No BIOS calls.
; Kernel refines via ACPI later.
;
; Output at 0x8000:
;   [0x8000] u32 count = 2
;   [0x8004] entry 0: base=0, len=640KB, type=1 (usable)
;   [0x801C] entry 1: base=1MB, len=255MB, type=1 (usable)
; ============================================================================

[BITS 16]

MEMORY_MAP_ADDR  equ 0x6000
MEMORY_MAP_COUNT equ 0x6000

detect_memory_e820:
    pusha
    push es

    ; Point ES:DI to 0x8000 (segment 0, offset 0x8000)
    xor ax, ax
    mov es, ax
    mov di, MEMORY_MAP_ADDR

    ; Count = 2 entries (write as two words: low=2, high=0)
    mov word [es:di], 2
    add di, 2
    mov word [es:di], 0
    add di, 2

    ; Entry 0: base=0x00000000, length=0x000A0000 (640KB), type=1, attrs=0
    ; base low word, base low-high, base high-low, base high-high
    xor ax, ax
    mov word [es:di], ax        ; base[0:15]  = 0
    add di, 2
    mov word [es:di], ax        ; base[16:31] = 0
    add di, 2
    mov word [es:di], ax        ; base[32:47] = 0
    add di, 2
    mov word [es:di], ax        ; base[48:63] = 0
    add di, 2
    ; length = 0x000A0000
    mov word [es:di], 0x0000    ; len[0:15]
    add di, 2
    mov word [es:di], 0x000A    ; len[16:31]
    add di, 2
    mov word [es:di], ax        ; len[32:47] = 0
    add di, 2
    mov word [es:di], ax        ; len[48:63] = 0
    add di, 2
    ; type = 1 (usable)
    mov word [es:di], 1
    add di, 2
    mov word [es:di], ax        ; type high = 0
    add di, 2
    ; attrs = 0
    mov word [es:di], ax
    add di, 2
    mov word [es:di], ax
    add di, 2

    ; Entry 1: base=0x00100000 (1MB), length=0x0FF00000 (255MB), type=1
    ; base = 0x00100000
    mov word [es:di], 0x0000    ; base[0:15]
    add di, 2
    mov word [es:di], 0x0010    ; base[16:31]
    add di, 2
    mov word [es:di], ax        ; base[32:47] = 0
    add di, 2
    mov word [es:di], ax        ; base[48:63] = 0
    add di, 2
    ; length = 0x0FF00000
    mov word [es:di], 0x0000    ; len[0:15]
    add di, 2
    mov word [es:di], 0x0FF0    ; len[16:31]
    add di, 2
    mov word [es:di], ax        ; len[32:47] = 0
    add di, 2
    mov word [es:di], ax        ; len[48:63] = 0
    add di, 2
    ; type = 1
    mov word [es:di], 1
    add di, 2
    mov word [es:di], ax        ; type high = 0
    add di, 2
    ; attrs = 0
    mov word [es:di], ax
    add di, 2
    mov word [es:di], ax

    pop es
    popa
    ret
