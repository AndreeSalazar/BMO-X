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
global load_payloads
global payload_boot_drive
load_payloads:
    pusha

    ; Print start message
    mov si, msg_start
    call .print_string_16

    ; Print boot_drive value
    mov si, msg_dl
    call .print_string_16
    mov al, [payload_boot_drive]
    call .print_hex_byte
    call .print_newline

    ; Check LBA extensions
    mov si, msg_lba_check
    call .print_string_16
    mov ah, 0x41
    mov bx, 0x55AA
    mov dl, [payload_boot_drive]
    int 0x13
    jc .no_lba
    cmp bx, 0xAA55
    je .lba_ok
.no_lba:
    mov si, msg_lba_fail
    call .print_string_16
    jmp .continue_diag
.lba_ok:
    mov si, msg_lba_ok
    call .print_string_16
.continue_diag:

    ; Enter Unreal Mode
    cli
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

.load_loop:
    push cx
    cmp cx, 1
    jne .full_block
    mov word [dap_count], 4
    jmp .do_read

.full_block:
    mov word [dap_count], SECTORS_PER_BLOCK

.do_read:
    mov ah, 0x42
    mov dl, [payload_boot_drive]
    mov si, dap
    int 0x13
    jc .read_error

    ; Re-enter Unreal Mode
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
    jnz .load_loop

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
    call .print_string_16

    popa
    ret

.read_error:
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    sti

    mov si, msg_error
    call .print_string_16
    mov si, msg_error_code
    call .print_string_16
    mov al, ah
    call .print_hex_byte
    call .print_newline

    popa
    ret

; ── Helper functions ─────────────────────────────────────────────────────────
.print_string_16:
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

.print_hex_byte:
    pusha
    mov ah, al
    shr al, 4
    call .print_hex_nibble
    mov al, ah
    and al, 0x0F
    call .print_hex_nibble
    popa
    ret

.print_hex_nibble:
    add al, '0'
    cmp al, '9'
    jbe .digit
    add al, 7
.digit:
    mov ah, 0x0E
    mov bx, 0x000F
    int 0x10
    ret

.print_newline:
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
