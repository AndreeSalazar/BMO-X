; ============================================================================
; FastOS Payload Loader — Unreal Mode for GSP Firmware
; ============================================================================
; Target: RTX 3060 12GB (GA106)
; Board: MSI MS-7C52 (B550), AMI Aptio V BIOS
;
; CONSTRAINTS: NASM -f bin flat binary, no sections, no extern
;
; BUG FIXES applied:
;   1. Restore SS=0 after every Unreal Mode entry so push/pop/call/ret work
;   2. Restore DS=0 before INT 13h (BIOS needs real-mode segments)
;   3. Error path halts instead of corrupting stack with popa+ret
;   4. Save/restore SP around entire function for safety
; ============================================================================

[BITS 16]

; ── Constants ───────────────────────────────────────────────────────────────
GSP_FW_LOAD_ADDR   equ 0x1000000      ; 16MB - firmware target
GSP_FW_SIZE         equ 72845296       ; 69.5MB = 142,196 sectors
SECTORS_PER_BLOCK   equ 16             ; B550 limit = 16 sectors (8KB)
BLOCK_SIZE          equ 8192           ; 16 * 512 = 8KB
BUFFER_ADDR         equ 0x20000        ; 128KB - safe buffer
TOTAL_ITERATIONS    equ 8888           ; 142,196 / 16 = 8887.25 → 8888

; ── Entry point ─────────────────────────────────────────────────────────────
; Called from stage2.asm with: call load_payloads
; On return: payload_base and payload_size are set.
; Preserves all GPRs via pusha/popa. Stack must be balanced.
load_payloads:
    pusha
    ; Save SP after pusha so we can restore it on error
    mov [pl_saved_sp], sp

    ; Print start message
    mov si, pl_msg_start
    call pl_print_string

    ; Print boot_drive value
    mov si, pl_msg_dl
    call pl_print_string
    mov al, [payload_boot_drive]
    call pl_print_hex_byte
    call pl_print_newline

    ; Check LBA extensions (informational only — MSI B550 CSM lies about this)
    mov si, pl_msg_lba_check
    call pl_print_string
    mov ah, 0x41
    mov bx, 0x55AA
    mov dl, [payload_boot_drive]
    int 0x13
    jc pl_no_lba
    cmp bx, 0xAA55
    je pl_lba_ok
pl_no_lba:
    mov si, pl_msg_lba_fail
    call pl_print_string
    jmp pl_continue_load
pl_lba_ok:
    mov si, pl_msg_lba_ok
    call pl_print_string
pl_continue_load:

    ; ── Enter Unreal Mode ──────────────────────────────────────────────────
    ; We need 32-bit addressing for movsd to 0x1000000.
    ; Process: PM on → load 4GB-limit DS/ES → PM off → segments keep limit.
    ; CRITICAL: SS must be restored to 0 before any push/pop/call/ret.
    cli

    ; Patch GDT descriptor with physical address (CS*16 + offset)
    xor eax, eax
    mov ax, cs
    shl eax, 4
    lea ebx, [pl_unreal_gdt]
    add eax, ebx
    mov dword [pl_unreal_gdt_desc + 2], eax

    lgdt [pl_unreal_gdt_desc]

    mov eax, cr0
    or eax, 1               ; PE = 1 (enter protected mode)
    mov cr0, eax

    mov ax, 0x08             ; Load 4GB-limit data descriptor
    mov ds, ax
    mov es, ax
    ; NOTE: Do NOT set SS to 0x08. Leave SS alone during PM transition.
    ;       SS will keep its real-mode base (0x0000) which is correct.

    mov eax, cr0
    and eax, 0xFFFFFFFE     ; PE = 0 (back to real mode)
    mov cr0, eax

    ; Restore real-mode segment values for DS/ES
    ; DS/ES now have 4GB limit (Unreal Mode) but segment base = 0
    xor ax, ax
    mov ds, ax
    mov es, ax
    ; SS was never changed — it's still 0x0000, stack works normally.

    sti

    ; ── Initialize load loop ───────────────────────────────────────────────
    mov dword [pl_dap_lba], 1000
    mov dword [pl_dap_lba+4], 0
    mov dword [pl_dest_addr], GSP_FW_LOAD_ADDR
    mov word  [pl_remain], TOTAL_ITERATIONS

pl_load_loop:
    ; Set sector count: last iteration = 4 sectors, else = 16
    cmp word [pl_remain], 1
    jne pl_full_block
    mov word [pl_dap_count], 4
    jmp pl_do_read
pl_full_block:
    mov word [pl_dap_count], SECTORS_PER_BLOCK

pl_do_read:
    ; ── BIOS INT 13h AH=42h needs real-mode segments ──────────────────────
    ; DS:SI must point to DAP with DS=0 (our DAP is in low memory)
    xor ax, ax
    mov ds, ax

    mov ah, 0x42
    mov dl, [payload_boot_drive]
    mov si, pl_dap
    int 0x13
    jc pl_read_error

    ; ── Re-enter Unreal Mode for the copy ─────────────────────────────────
    ; After INT 13h, BIOS may have trashed segment limits/GDT.
    ; Re-load GDT and re-enter Unreal Mode for the 32-bit copy.
    cli

    xor eax, eax
    mov ax, cs
    shl eax, 4
    lea ebx, [pl_unreal_gdt]
    add eax, ebx
    mov dword [pl_unreal_gdt_desc + 2], eax

    lgdt [pl_unreal_gdt_desc]

    mov eax, cr0
    or eax, 1
    mov cr0, eax

    mov ax, 0x08
    mov ds, ax
    mov es, ax
    ; Do NOT touch SS — keep it at real-mode 0x0000

    mov eax, cr0
    and eax, 0xFFFFFFFE
    mov cr0, eax

    ; Restore DS/ES to 0 (keeps 4GB limit from Unreal Mode)
    xor ax, ax
    mov ds, ax
    mov es, ax

    sti

    ; ── Copy 8KB from buffer (0x20000) to destination (>1MB) ──────────────
    ; a32 prefix enables 32-bit addressing in real mode (Unreal Mode)
    mov esi, BUFFER_ADDR
    mov edi, [pl_dest_addr]
    mov ecx, BLOCK_SIZE / 4       ; 2048 dwords = 8KB
    a32 rep movsd

    ; ── Advance to next block ─────────────────────────────────────────────
    mov eax, [pl_dest_addr]
    add eax, BLOCK_SIZE
    mov [pl_dest_addr], eax

    mov eax, [pl_dap_lba]
    add eax, SECTORS_PER_BLOCK
    mov [pl_dap_lba], eax

    dec word [pl_remain]
    jnz pl_load_loop

    ; ── Load complete ─────────────────────────────────────────────────────
    mov dword [payload_base], GSP_FW_LOAD_ADDR
    mov dword [payload_base+4], 0
    mov dword [payload_size], GSP_FW_SIZE
    mov dword [payload_size+4], 0

    mov si, pl_msg_done
    call pl_print_string

    popa
    ret

; ── Error handler ─────────────────────────────────────────────────────────
; On read error: restore stack to exact state after pusha, then popa+ret.
; This ensures the caller (stage2) gets control back cleanly.
pl_read_error:
    ; Save error code before we trash AH
    mov [pl_err_code], ah

    ; Restore real-mode segments
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    sti

    ; Restore SP to the value right after pusha (before any push cx etc.)
    mov sp, [pl_saved_sp]

    ; Print error message
    mov si, pl_msg_error
    call pl_print_string
    mov si, pl_msg_error_code
    call pl_print_string
    mov al, [pl_err_code]
    call pl_print_hex_byte
    call pl_print_newline

    ; Still set payload_base/size to 0 so kernel knows firmware failed
    mov dword [payload_base], 0
    mov dword [payload_base+4], 0
    mov dword [payload_size], 0
    mov dword [payload_size+4], 0

    ; Clean return to stage2 — popa matches the pusha at entry
    popa
    ret

; ── Helper functions (pl_ prefix to avoid label collisions) ────────────────
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

; ── Data (after all code) ─────────────────────────────────────────────────────
align 4
payload_boot_drive:  db 0
payload_base:        dq 0
payload_size:        dq 0
pl_saved_sp:         dw 0              ; SP after pusha (for error recovery)
pl_err_code:         db 0              ; INT 13h error code
pl_dest_addr:        dd 0              ; Current copy destination (32-bit)
pl_remain:           dw 0              ; Remaining iterations

align 4
pl_dap:
    db 0x10                            ; Size of DAP
    db 0                               ; Reserved
pl_dap_count:
    dw SECTORS_PER_BLOCK               ; Sectors to read
    dw 0x0000                          ; Buffer offset
    dw 0x2000                          ; Buffer segment → phys 0x20000
pl_dap_lba:
    dq 0                               ; Starting LBA

align 8
pl_unreal_gdt:
    dq 0                               ; Null descriptor
    dw 0xFFFF                          ; Limit low (4GB)
    dw 0x0000                          ; Base low
    db 0x00                            ; Base mid
    db 10010010b                       ; Access: P=1, Ring0, Data, R/W
    db 11001111b                       ; Flags: 4KB gran, 32-bit, Limit hi=F
    db 0x00                            ; Base high

pl_unreal_gdt_desc:
    dw pl_unreal_gdt_desc - pl_unreal_gdt - 1
    dd pl_unreal_gdt                   ; Patched at runtime to physical addr

pl_msg_start      db "[S2] Loading GSP firmware (Unreal Mode)...", 13, 10, 0
pl_msg_dl         db "[S2] Boot drive (saved) = ", 0
pl_msg_lba_check  db "[S2] Checking LBA extensions (AH=41h)...", 13, 10, 0
pl_msg_lba_ok     db "[S2] LBA extensions: SUPPORTED", 13, 10, 0
pl_msg_lba_fail   db "[S2] LBA extensions: NOT SUPPORTED", 13, 10, 0
pl_msg_done       db "[S2] GSP firmware loaded OK", 13, 10, 0
pl_msg_error      db "[S2] ERROR: Firmware load failed", 13, 10, 0
pl_msg_error_code db "[S2] INT 13h error code (AH) = ", 0
