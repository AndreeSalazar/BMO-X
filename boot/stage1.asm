; ============================================================================
; FastOS Stage 1 — MBR Boot Sector (512 bytes)
; ============================================================================
; Real mode 16-bit. Loaded by BIOS at 0x7C00.
; Job: set up segments, load stage2 from disk, verify it, jump to it.
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

    ; ── Verify INT 13h LBA extensions (AH=41h) ──────────────────────────
    ; Some BIOS/UEFI CSM implementations don't support AH=42h for USB.
    ; Check first so we get a clear error instead of silent failure.
    mov ah, 0x41
    mov bx, 0x55AA
    mov dl, [boot_drive]
    int 0x13
    jc .no_lba
    cmp bx, 0xAA55
    je .lba_ok

.no_lba:
    mov si, msg_no_lba
    call print_string_16
    cli
    hlt

.lba_ok:
    ; ── Load Stage 2 via INT 13h AH=42h (LBA) ───────────────────────────
    mov si, msg_loading
    call print_string_16

    mov ah, 0x42
    mov dl, [boot_drive]
    mov si, dap
    int 0x13
    jc disk_error

    ; ── Verify Stage2 loaded correctly ───────────────────────────────────
    ; First byte of stage2.bin must be 0xE9 (near JMP opcode).
    ; If INT 13h returned success but loaded zeros/garbage, catch it here.
    cmp byte [0x7E00], 0xE9
    je .stage2_ok

    mov si, msg_bad_data
    call print_string_16
    cli
    hlt

.stage2_ok:
    mov si, msg_jumping
    call print_string_16

    ; Jump to stage2 (restore DL — some BIOS don't preserve it after INT 13h)
    mov dl, [boot_drive]
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
msg_loading:  db "[FastOS] Loading Stage2...", 13, 10, 0
msg_jumping:  db "[FastOS] Stage2 verified OK!", 13, 10, 0
msg_disk_err: db "[FastOS] DISK READ ERROR!", 13, 10, 0
msg_no_lba:   db "[FastOS] NO LBA EXTENSIONS!", 13, 10, 0
msg_bad_data: db "[FastOS] STAGE2 DATA INVALID!", 13, 10, 0

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
