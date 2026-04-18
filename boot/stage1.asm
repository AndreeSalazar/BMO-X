; ============================================================================
; FastOS Stage 1 — MBR Boot Sector (512 bytes)
; ============================================================================
; Real mode 16-bit. Loaded by BIOS at 0x7C00.
; Job: set up segments, load stage2 from disk, jump to it.
; ============================================================================

[BITS 16]
[ORG 0x7C00]

stage1_start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00
    sti

    ; Save boot drive (BIOS passes in DL)
    mov [boot_drive], dl

    mov si, msg_boot
    call print_string_16

    ; Load Stage 2 via INT 13h AH=42h (LBA)
    mov ah, 0x42
    mov dl, [boot_drive]
    mov si, dap
    int 0x13
    jc disk_error

    mov si, msg_stage2
    call print_string_16

    ; Jump to stage2
    jmp 0x0000:0x7E00

disk_error:
    mov si, msg_disk_err
    call print_string_16
    cli
    hlt

; --- Print null-terminated string (SI) ---
print_string_16:
    pusha
.loop:
    lodsb
    or al, al
    jz .done
    mov ah, 0x0E
    mov bx, 0x0007
    int 0x10
    jmp .loop
.done:
    popa
    ret

; --- Data ---
boot_drive:   db 0
msg_boot:     db "[FastOS] Stage1: MBR loaded", 13, 10, 0
msg_stage2:   db "[FastOS] Loading Stage2...", 13, 10, 0
msg_disk_err: db "[FastOS] DISK ERROR!", 13, 10, 0

; Disk Address Packet (INT 13h AH=42h)
align 4
dap:
    db 0x10                 ; DAP size
    db 0                    ; Reserved
    dw 32                   ; Sectors to read (32 × 512 = 16KB)
    dw 0x7E00               ; Offset
    dw 0x0000               ; Segment
    dq 1                    ; Starting LBA (sector 1)

; MBR signature
times 510 - ($ - $$) db 0
dw 0xAA55
