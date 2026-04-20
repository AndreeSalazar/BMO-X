; ============================================================================
; FastOS Stage 1 — MBR Boot Sector (512 bytes)
; ============================================================================
; Real mode 16-bit. Loaded by BIOS at 0x7C00.
; Job: set up segments, load stage2 from disk, verify it, jump to it.
; Supports LBA (AH=42h) with CHS (AH=02h) fallback.
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

    ; ── Check INT 13h LBA extensions (AH=41h) ─────────────────────────
    mov ah, 0x41
    mov bx, 0x55AA
    mov dl, [boot_drive]
    int 0x13
    jc .try_chs
    cmp bx, 0xAA55
    jne .try_chs

    ; ── LBA supported — load Stage2 via AH=42h ────────────────────────
    mov si, msg_lba_ok
    call print_string_16

    mov si, msg_loading
    call print_string_16

    mov ah, 0x42
    mov dl, [boot_drive]
    mov si, dap
    int 0x13
    jc .try_chs              ; If LBA read fails, fall through to CHS
    jmp .verify_stage2

.try_chs:
    ; ── No LBA or LBA read failed — use CHS fallback (AH=02h) ─────────
    mov si, msg_chs
    call print_string_16

    mov si, msg_loading
    call print_string_16

    ; Reset disk controller first (important for CHS)
    xor ax, ax
    mov dl, [boot_drive]
    int 0x13

    ; CHS read: Stage2 is at LBA 1-32 = CHS(0,0,2) through CHS(0,0,33)
    ; Read in 2 batches of 16 sectors (some BIOS limit per-read count)
    ; Batch 1: 16 sectors from CHS(0,0,2) to 0x0000:0x7E00
    mov ah, 0x02
    mov al, 16                ; 16 sectors
    mov ch, 0                 ; Cylinder 0
    mov cl, 2                 ; Start sector 2 (CHS sectors are 1-based)
    mov dh, 0                 ; Head 0
    mov dl, [boot_drive]
    mov bx, 0x7E00            ; ES:BX = 0x0000:0x7E00
    int 0x13
    jc disk_error

    ; Batch 2: 16 sectors from CHS(0,0,18) to 0x0000:0x9E00
    mov ah, 0x02
    mov al, 16                ; 16 sectors
    mov ch, 0                 ; Cylinder 0
    mov cl, 18                ; Start sector 18
    mov dh, 0                 ; Head 0
    mov dl, [boot_drive]
    mov bx, 0x9E00            ; ES:BX = 0x0000:0x9E00  (0x7E00 + 16*512)
    int 0x13
    jc disk_error

.verify_stage2:
    ; ── Verify Stage2 loaded correctly ─────────────────────────────────
    ; First byte of stage2.bin must be 0xE9 (near JMP opcode).
    cmp byte [0x7E00], 0xE9
    je .stage2_ok

    mov si, msg_bad_data
    call print_string_16
    cli
    hlt

.stage2_ok:
    mov si, msg_jumping
    call print_string_16

    ; Jump to stage2 (restore DL)
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
msg_lba_ok:   db "[FastOS] LBA OK", 13, 10, 0
msg_chs:      db "[FastOS] CHS mode", 13, 10, 0
msg_loading:  db "[FastOS] Loading Stage2...", 13, 10, 0
msg_jumping:  db "[FastOS] Stage2 OK!", 13, 10, 0
msg_disk_err: db "[FastOS] DISK ERROR!", 13, 10, 0
msg_bad_data: db "[FastOS] BAD STAGE2!", 13, 10, 0

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
