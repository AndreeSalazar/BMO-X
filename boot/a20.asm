; ============================================================================
; FastOS A20 Line Enable
; ============================================================================
; Must be enabled to access memory above 1MB.
; Tries: BIOS → keyboard controller → fast A20 (port 0x92).
; ============================================================================

[BITS 16]

enable_a20:
    ; Method 1: BIOS
    mov ax, 0x2401
    int 0x15
    jnc .a20_done

    ; Method 2: Keyboard controller
    call .a20_wait_input
    mov al, 0xAD
    out 0x64, al
    call .a20_wait_input
    mov al, 0xD0
    out 0x64, al
    call .a20_wait_output
    in al, 0x60
    push ax
    call .a20_wait_input
    mov al, 0xD1
    out 0x64, al
    call .a20_wait_input
    pop ax
    or al, 0x02
    out 0x60, al
    call .a20_wait_input
    mov al, 0xAE
    out 0x64, al
    call .a20_wait_input

    ; Method 3: Fast A20
    in al, 0x92
    or al, 0x02
    and al, 0xFE
    out 0x92, al

.a20_done:
    ret

.a20_wait_input:
    in al, 0x64
    test al, 0x02
    jnz .a20_wait_input
    ret

.a20_wait_output:
    in al, 0x64
    test al, 0x01
    jz .a20_wait_output
    ret
