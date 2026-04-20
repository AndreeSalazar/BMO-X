; ============================================================================
; FastOS Payload Loader — Unreal Mode for GSP Firmware
; ============================================================================
; Target: RTX 3060 12GB (GA106)
; Board: MSI MS-7C52 (B550), AMI Aptio V BIOS
;
; CONSTRAINTS: NASM -f bin flat binary, no sections, no extern
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
load_payloads:
    pusha

    ; Print start message
    mov si, msg_start
    call pl_print_string

    ; Print boot_drive value
    mov si, msg_dl
    call pl_print_string
    mov al, [payload_boot_drive]
    call pl_print_hex_byte
    call pl_print_newline

    ; Check LBA extensions
    mov si, msg_lba_check
    call pl_print_string
    mov ah, 0x41
    mov bx, 0x55AA
    mov dl, [payload_boot_drive]
    int 0x13
    jc pl_no_lba
    cmp bx, 0xAA55
    je pl_lba_ok
pl_no_lba:
    mov si, msg_lba_fail
    call pl_print_string
    jmp pl_continue_diag
pl_lba_ok:
    mov si, msg_lba_ok
    call pl_print_string
pl_continue_diag:

    ; Enter Unreal Mode - Calculate GDT physical address first
    cli

    ; Calculate physical address of GDT: CS * 16 + offset(unreal_gdt)
    xor eax, eax
    mov ax, cs
    shl eax, 4
    lea ebx, [unreal_gdt]
    add eax, ebx
    mov dword [unreal_gdt_desc + 2], eax   ; Patch descriptor with physical address

    lgdt [unreal_gdt_desc]
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    mov ax, 0x08
    mov ds, ax
    mov es, ax
    mov ss, ax
    and eax, 0xFFFFFFFE
    mov cr0, eax

    ; Initialize loop
    mov dword [dap_lba], 1000
    mov dword [dap_lba+4], 0
    mov edi, GSP_FW_LOAD_ADDR
    mov cx, TOTAL_ITERATIONS

pl_load_loop:
    push cx
    cmp cx, 1
    jne pl_full_block
    mov word [dap_count], 4
    jmp pl_do_read

pl_full_block:
    mov word [dap_count], SECTORS_PER_BLOCK

pl_do_read:
    mov ah, 0x42
    mov dl, [payload_boot_drive]
    mov si, dap
    int 0x13
    jc pl_read_error

    ; Re-enter Unreal Mode - Calculate GDT physical address again
    xor eax, eax
    mov ax, cs
    shl eax, 4
    lea ebx, [unreal_gdt]
    add eax, ebx
    mov dword [unreal_gdt_desc + 2], eax

    lgdt [unreal_gdt_desc]
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    mov ax, 0x08
    mov ds, ax
    mov es, ax
    mov ss, ax
    and eax, 0xFFFFFFFE
    mov cr0, eax

    ; Copy 8KB
    push edi
    push esi
    mov esi, BUFFER_ADDR
    mov ecx, BLOCK_SIZE / 4
    a32 rep movsd
    pop esi
    pop edi

    add edi, BLOCK_SIZE
    mov eax, dword [dap_lba]
    add eax, SECTORS_PER_BLOCK
    mov dword [dap_lba], eax

    pop cx
    dec cx
    jnz pl_load_loop

    ; Complete
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    sti

    mov dword [payload_base], GSP_FW_LOAD_ADDR
    mov dword [payload_base+4], 0
    mov dword [payload_size], GSP_FW_SIZE
    mov dword [payload_size+4], 0

    mov si, msg_done
    call pl_print_string

    popa
    ret

pl_read_error:
    ; CRITICAL: Must pop cx before returning if we're in the loop
    ; Check if we're in loop by checking if we pushed cx
    ; We can detect this by checking if CX was pushed
    ; For safety, always restore stack to known state
    mov sp, 0x7C00    ; Reset stack to known location

    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    sti

    mov si, msg_error
    call pl_print_string
    mov si, msg_error_code
    call pl_print_string
    mov al, ah
    call pl_print_hex_byte
    call pl_print_newline

    popa
    ret

; ── Helper functions (unique names to avoid collisions) ────────────────────
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

align 4
dap:
    db 0x10
    db 0
dap_count:
    dw SECTORS_PER_BLOCK
dap_buffer_offset:
    dw 0x0000
dap_buffer_segment:
    dw 0x2000
dap_lba:
    dq 0

align 8
unreal_gdt:
    dq 0
unreal_data_desc:
    dw 0xFFFF
    dw 0x0000
    db 0x00
    db 10010010b
    db 11001111b
    db 0x00

unreal_gdt_desc:
    dw unreal_gdt_desc - unreal_gdt - 1
    dd unreal_gdt

msg_start      db "[S2] Loading GSP firmware (Unreal Mode)...", 13, 10, 0
msg_dl         db "[S2] Boot drive (saved) = ", 0
msg_lba_check  db "[S2] Checking LBA extensions (AH=41h)...", 13, 10, 0
msg_lba_ok     db "[S2] LBA extensions: SUPPORTED", 13, 10, 0
msg_lba_fail   db "[S2] LBA extensions: NOT SUPPORTED", 13, 10, 0
msg_done       db "[S2] GSP firmware loaded OK", 13, 10, 0
msg_error      db "[S2] ERROR: Firmware load failed", 13, 10, 0
msg_error_code db "[S2] INT 13h error code (AH) = ", 0
