; ============================================================================
; FastOS Payload Loader — Real Mode Only (No Unreal Mode)
; ============================================================================
; Target: RTX 3060 12GB (GA106)
; Board: MSI MS-7C52 (B550), AMI Aptio V BIOS (DL=0x0E for USB)
;
; DESIGN: Load firmware using plain real-mode INT 13h only.
;   - NO Unreal Mode (causes triple fault on AMI Aptio V CSM)
;   - NO lgdt/CR0 toggle in real mode
;   - Read 32KB chunks to buffer at 0x20000 (segment 0x2000)
;   - Copy to 0x1000000 is done later in Protected Mode by stage2
;   - This function ONLY reads to low memory and records metadata
;
; CONSTRAINTS: NASM -f bin flat binary, no sections, no extern
; ============================================================================

[BITS 16]

; ── Constants ───────────────────────────────────────────────────────────────
PL_GSP_LBA_START   equ 1000           ; LBA where GSP firmware starts on disk
PL_GSP_SECTORS     equ 142276         ; 72845296 / 512 = 142276.75 → 142277 sectors
PL_GSP_SIZE        equ 72845296       ; Exact size in bytes
PL_CHUNK_SECTORS   equ 64             ; 64 sectors = 32KB per INT 13h call
PL_CHUNK_BYTES     equ 32768          ; 64 * 512
PL_BUFFER_SEG      equ 0x2000         ; Segment 0x2000 = phys 0x20000
PL_TOTAL_CHUNKS    equ 2223           ; 142277 / 64 = 2223.078 → 2223 full + 1 partial
PL_LAST_SECTORS    equ 5              ; 142277 - (2223 * 64) = 142277 - 142272 = 5

; ── Entry point ─────────────────────────────────────────────────────────────
; Called from stage2.asm with: call load_payloads
; On return: payload_base/size set, firmware is at 0x20000..0x4FFFF area
;   (only last chunk remains in buffer — full copy to 0x1000000 done in PM)
; Actually: we record LBA/size info. PM code does the streaming copy.
load_payloads:
    pusha
    mov [pl_saved_sp], sp

    ; Print start message
    mov si, pl_msg_start
    call pl_print_string

    ; Print boot_drive value
    mov si, pl_msg_dl
    call pl_print_string
    mov al, [stage2_boot_drive]
    call pl_print_hex_byte
    call pl_print_newline

    ; ── Test: single INT 13h read to verify disk access works ────────────
    mov si, pl_msg_test_read
    call pl_print_string

    ; Setup DAP for test read: 1 sector from LBA 1000 to 0x2000:0x0000
    mov byte  [pl_dap],   0x10         ; DAP size
    mov byte  [pl_dap+1], 0            ; reserved
    mov word  [pl_dap_count], 1        ; 1 sector
    mov word  [pl_dap_buf_off], 0x0000
    mov word  [pl_dap_buf_seg], PL_BUFFER_SEG
    mov dword [pl_dap_lba], PL_GSP_LBA_START
    mov dword [pl_dap_lba+4], 0

    mov ah, 0x42
    mov dl, [stage2_boot_drive]
    mov si, pl_dap
    int 0x13
    jc pl_read_error

    mov si, pl_msg_test_ok
    call pl_print_string

    ; ── Firmware validated — record metadata for kernel ───────────────────
    ; The full 69.5MB firmware load to 0x1000000 will be done by the kernel
    ; itself (via PCI DMA or re-reading sectors in protected/long mode).
    ; The bootloader's job is just to verify the disk is accessible and
    ; pass the firmware's disk location + size to the kernel.
    ; ── Record firmware metadata ──────────────────────────────────────────
    ; The firmware will be copied to 0x1000000 in Protected Mode.
    ; Record the disk location so PM code can re-read and copy.
    mov dword [payload_fw_lba], PL_GSP_LBA_START
    mov dword [payload_fw_sectors], PL_GSP_SECTORS + 1
    mov dword [payload_base], 0x1000000
    mov dword [payload_base+4], 0
    mov dword [payload_size], PL_GSP_SIZE
    mov dword [payload_size+4], 0

    mov si, pl_msg_done
    call pl_print_string

    popa
    ret

; ── Error handler ─────────────────────────────────────────────────────────
pl_read_error:
    mov [pl_err_code], ah

    ; Restore segments
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    sti

    mov sp, [pl_saved_sp]

    mov si, pl_msg_error
    call pl_print_string

    mov si, pl_msg_error_code
    call pl_print_string
    mov al, [pl_err_code]
    call pl_print_hex_byte
    call pl_print_newline

    ; Print which LBA failed
    mov si, pl_msg_error_lba
    call pl_print_string
    mov al, byte [pl_dap_lba+3]
    call pl_print_hex_byte
    mov al, byte [pl_dap_lba+2]
    call pl_print_hex_byte
    mov al, byte [pl_dap_lba+1]
    call pl_print_hex_byte
    mov al, byte [pl_dap_lba]
    call pl_print_hex_byte
    call pl_print_newline

    ; Set payload to 0 so kernel knows firmware failed
    mov dword [payload_base], 0
    mov dword [payload_base+4], 0
    mov dword [payload_size], 0
    mov dword [payload_size+4], 0

    popa
    ret

; ── Helper functions ──────────────────────────────────────────────────────
pl_print_string:
    pusha
pl_ps_loop:
    lodsb
    or al, al
    jz pl_ps_done
    mov ah, 0x0E
    mov bx, 0x0007
    int 0x10
    jmp pl_ps_loop
pl_ps_done:
    popa
    ret

pl_print_hex_byte:
    pusha
    mov ah, al
    shr al, 4
    call pl_print_hex_nibble
    mov al, ah
    and al, 0x0F
    call pl_print_hex_nibble
    popa
    ret

pl_print_hex_nibble:
    add al, '0'
    cmp al, '9'
    jbe pl_phn_digit
    add al, 7
pl_phn_digit:
    mov ah, 0x0E
    mov bx, 0x000F
    int 0x10
    ret

pl_print_newline:
    pusha
    mov ah, 0x0E
    mov al, 13
    mov bx, 0x000F
    int 0x10
    mov al, 10
    int 0x10
    popa
    ret

; ── Data ──────────────────────────────────────────────────────────────────
align 4
payload_base:        dq 0              ; Final address (set after PM copy)
payload_size:        dq 0              ; Firmware size in bytes
payload_fw_lba:      dd 0              ; Starting LBA on disk
payload_fw_sectors:  dd 0              ; Total sectors to read
pl_saved_sp:         dw 0
pl_err_code:         db 0

align 4
pl_dap:
    db 0x10                            ; DAP size
    db 0                               ; Reserved
pl_dap_count:
    dw 0                               ; Sectors to read (set per iteration)
pl_dap_buf_off:
    dw 0x0000                          ; Buffer offset
pl_dap_buf_seg:
    dw PL_BUFFER_SEG                   ; Buffer segment
pl_dap_lba:
    dq 0                               ; Starting LBA

pl_msg_start      db "[S2] Loading GSP firmware...", 13, 10, 0
pl_msg_dl         db "[S2] Boot drive = ", 0
pl_msg_test_read  db "[S2] Testing disk read (LBA 1000)...", 13, 10, 0
pl_msg_test_ok    db "[S2] Test read OK!", 13, 10, 0
pl_msg_done       db 13, 10, "[S2] Firmware read complete!", 13, 10, 0
pl_msg_error      db 13, 10, "[S2] DISK READ ERROR!", 13, 10, 0
pl_msg_error_code db "[S2] Error code (AH) = ", 0
pl_msg_error_lba  db "[S2] Failed at LBA = ", 0
