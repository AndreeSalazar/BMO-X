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

    ; ── Test LBA extensions (AH=41h) ─────────────────────────────────────
    mov ah, 0x41
    mov bx, 0x55AA
    mov dl, [boot_drive]
    int 0x13
    jc .use_chs             ; CF=1 → no LBA → use CHS
    cmp bx, 0xAA55
    jne .use_chs            ; bad signature → use CHS

    ; ── LBA path: load Stage2 via AH=42h ─────────────────────────────────
    mov si, msg_lba
    call print_string_16
    mov si, msg_loading
    call print_string_16

    mov ah, 0x42
    mov dl, [boot_drive]
    mov si, dap
    int 0x13
    jc disk_error
    jmp .verify

.use_chs:
    ; ── CHS path: load Stage2 via AH=02h ─────────────────────────────────
    mov si, msg_chs
    call print_string_16
    mov si, msg_loading
    call print_string_16

    ; Reset disk controller first
    xor ax, ax
    mov dl, [boot_drive]
    int 0x13

    ; Read 32 sectors: CHS(0,0,2) = LBA 1, destination 0x0000:0x7E00
    mov ah, 0x02
    mov al, 32             ; 32 sectors = 16KB = Stage2
    mov ch, 0              ; Cylinder 0
    mov cl, 2              ; Sector 2 (CHS is 1-based, LBA 1 = sector 2)
    mov dh, 0              ; Head 0
    mov dl, [boot_drive]
    mov bx, 0x7E00         ; ES:BX = 0x0000:0x7E00
    int 0x13
    jc disk_error

.verify:
    ; ── Verify Stage2 loaded correctly ───────────────────────────────────
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
msg_lba:      db "[FastOS] LBA mode", 13, 10, 0
msg_chs:      db "[FastOS] CHS mode", 13, 10, 0
msg_loading:  db "[FastOS] Loading Stage2...", 13, 10, 0
msg_jumping:  db "[FastOS] Stage2 verified OK!", 13, 10, 0
msg_disk_err: db "[FastOS] DISK READ ERROR!", 13, 10, 0
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
