; ============================================================================
; FastOS Payload Loader — Unreal Mode & BIOS Int 13h Extensions
; ============================================================================
; This module separates the logic of loading huge files (like GSP firmware)
; from the core Stage2 bootloader. It avoids creating a monolith.
;
; How it works:
; 1. Enter Unreal Mode to bypass the 1MB Real Mode limit.
; 2. Read sectors into a 32KB conventional memory buffer (0x1000:0x0000).
; 3. Use 32-bit registers to `rep movsd` the data to physical > 1MB addresses.
; ============================================================================

[BITS 16]

; DAP (Disk Address Packet) for Int 13h AH=42h
; Use safe buffer at 0x20000 (128KB) - after BIOS data area
align 4
dap:
    db 0x10               ; Size of DAP (16 bytes)
    db 0                  ; Unused
dap_count:
    dw 16                 ; Number of sectors to read (8KB blocks for B550)
dap_buffer_offset:
    dw 0x0000             ; Offset
dap_buffer_segment:
    dw 0x2000             ; Segment (0x2000:0x0000 = 0x20000 physical = 128KB)
                            ; Safe area - after BIOS data, before kernel at 1MB
dap_lba:
    dq 0                  ; LBA to read from (Set dynamically)

; ── Unreal Mode GDT ────────────────────────────────────────────────────────
align 8
unreal_gdt:
    dq 0                  ; Null descriptor
unreal_data_desc:
    ; 4GB Data Descriptor (Base 0, Limit 4GB)
    dw 0xFFFF             ; Limit (bits 0-15)
    dw 0x0000             ; Base (bits 0-15)
    db 0x00               ; Base (bits 16-23)
    db 10010010b          ; Access (Present, Ring 0, Data, R/W)
    db 11001111b          ; Flags (Granularity 4KB, 32-bit) + Limit (16-19)
    db 0x00               ; Base (bits 24-31)

unreal_gdt_desc:
    dw unreal_gdt_desc - unreal_gdt - 1
    dd unreal_gdt

; ── load_payloads ──────────────────────────────────────────────────────────
; Input:
;   stage2_boot_drive (memory byte) must be set.
; Output:
;   Populates BootInfo with payload information.
load_payloads:
    pusha

    ; Print entering payload loader
    mov si, msg_loading_payloads
    call print_string_16

    ; CHK1 - Before Unreal Mode
    mov si, msg_chk1
    call print_string_16

    ; Step 1: Enter Unreal Mode
    cli

    ; CHK2 - Before lgdt
    mov si, msg_chk2
    call print_string_16

    ; Load Unreal GDT
    lgdt [unreal_gdt_desc]

    ; CHK3 - Before CR0
    mov si, msg_chk3
    call print_string_16

    ; Enable PM bit in CR0 briefly
    mov eax, cr0
    or al, 1
    mov cr0, eax

    ; Load DS and ES with 4GB limit selector (0x08) while in PM
    mov bx, 0x08
    mov ds, bx
    mov es, bx

    ; Disable PM bit in CR0 (back to Real Mode)
    and al, 0xFE
    mov cr0, eax

    ; Use retf to return to Real Mode - safer than far jump for relocatable code
    push word 0x0000      ; Segment = 0
    push word .rm_entry   ; Offset
    retf                  ; Far return to Real Mode

.rm_entry:
    ; CRITICAL: Restore segment registers immediately after retf
    xor ax, ax
    mov ds, ax    ; DS = 0
    mov es, ax    ; ES = 0
    mov ss, ax    ; SS = 0 (critical for stack)
    mov sp, 0x7C00 ; Stack at known safe location (same as MBR load address)
    
    ; CHK4 - After retf (Unreal Mode active)
    push ds
    xor ax, ax
    mov ds, ax
    mov si, msg_chk4
    call print_string_16
    pop ds

    ; CHK5 - Before INT 13h
    push ds
    xor ax, ax
    mov ds, ax
    mov si, msg_chk5
    call print_string_16
    pop ds
    
    ; Step 2: Read Módulo 1 (GSP Firmware - 69.5MB)
    ; In build.ps1, the firmware is written at LBA 1000.
    ; GSP firmware size: 72845296 bytes = 142196 sectors
    
    mov dword [dap_lba], 1000     ; Start reading at LBA 1000
    mov dword [dap_lba+4], 0
    
    mov edi, GSP_FW_LOAD_ADDR     ; Physical destination (16MB = 0x1000000)
    
    ; Total: 142196 sectors
    ; With 16 sectors per block: 142196 / 16 = 8887.25
    ; Use 8888 iterations (last one will be partial)
    mov cx, 8888                ; Loop counter
    
    ; Keep DS/ES in Unreal Mode (4GB limit) for entire loop
    ; Don't restore to 0 - this prevents entering/exiting PM repeatedly
    mov bx, 0x08
    mov ds, bx
    mov es, bx

.read_loop:
    push cx
    
    ; Check if this is the last iteration (cx = 1)
    ; If so, only read 4 sectors (partial block)
    cmp cx, 1
    jne .full_block
    mov word [dap_count], 4      ; Last block: 4 sectors = 2KB
    jmp .do_read
    
.full_block:
    ; Setup DAP to read 16 sectors (8KB) - AMI Aptio V B550 limit
    mov word [dap_count], 16

.do_read:
    ; Call BIOS Int 13h AH=42h
    ; Reads to safe buffer at 0x20000
    mov ah, 0x42
    mov dl, [stage2_boot_drive]
    mov si, dap
    int 0x13
    jc .read_error
    
    ; CHK6 - After first INT 13h success
    ; Only print on first iteration
    cmp word [progress_counter], 0
    jne .skip_chk6
    push ds
    xor ax, ax
    mov ds, ax
    mov si, msg_chk6
    call print_string_16
    pop ds
.skip_chk6:

    ; Copy from buffer (0x20000) to high memory (EDI)
    ; Since we are in Unreal Mode, we can use 32-bit registers for address!
    push edi
    push esi
    
    ; Source is physical 0x20000
    mov esi, 0x20000
    
    ; Calculate copy size based on dap_count
    mov ax, [dap_count]
    shl ax, 9                 ; Multiply by 512 (sector size)
    mov cx, ax
    shr cx, 2                 ; Divide by 4 for dwords
    
    ; rep movsd (32-bit copy in Real Mode thanks to Unreal Mode limits)
    a32 rep movsd

    pop esi
    pop edi

    ; Increment destination by (dap_count * 512)
    mov ax, [dap_count]
    shl ax, 9                 ; Multiply by 512
    movzx eax, ax
    add edi, eax
    
    ; Increment LBA by dap_count
    mov eax, dword [dap_lba]
    movzx ecx, word [dap_count]
    add eax, ecx
    mov dword [dap_lba], eax

    ; Progress indicator every 2048 blocks (~16MB with 16-sector blocks)
    ; Reduced frequency to avoid stack overflow from int 0x10 calls
    inc word [progress_counter]
    mov ax, [progress_counter]
    and ax, 0x07FF              ; Every 2048 (0x800)
    jnz .no_progress
    push ds
    xor ax, ax
    mov ds, ax
    mov al, '.'
    mov ah, 0x0E
    mov bx, 0x000F
    int 0x10
    pop ds
.no_progress:

    dec cx
    jnz .read_loop

.load_done:
    ; Restore DS/ES to 0 after loop
    xor ax, ax
    mov ds, ax
    mov es, ax

    ; Save variables for the kernel
    ; Base: GSP_FW_LOAD_ADDR (0x1000000 = 16MB)
    ; Size: 72845296 bytes (69.5MB)
    mov dword [payload_base], GSP_FW_LOAD_ADDR
    mov dword [payload_base+4], 0
    mov dword [payload_size], 72845296
    mov dword [payload_size+4], 0

    ; Verify firmware was actually loaded (check first 4 bytes)
    ; GSP firmware should start with valid data, not zeros
    mov si, msg_verify
    call print_string_16
    
    ; Check if first dword is non-zero
    ; Use 32-bit addressing to access high memory
    ; Need to enter Unreal Mode again to access 0x1000000
    cli
    
    ; Load Unreal GDT
    lgdt [unreal_gdt_desc]
    
    ; Enable PM bit in CR0 briefly
    mov eax, cr0
    or al, 1
    mov cr0, eax
    
    ; Load DS with 4GB limit selector (0x08) while in PM
    mov bx, 0x08
    mov ds, bx
    
    ; Disable PM bit in CR0 (back to Real Mode)
    and al, 0xFE
    mov cr0, eax
    
    ; Use retf to return to Real Mode
    push word 0x0000          ; Segment = 0
    push word .rm_entry_verify ; Offset
    retf                      ; Far return

.rm_entry_verify:
    ; Restore DS to 0
    xor ax, ax
    mov ds, ax
    sti
    
    ; Now access high memory
    mov eax, GSP_FW_LOAD_ADDR
    db 0x67  ; Address size prefix for 32-bit addressing
    mov eax, [eax]
    cmp eax, 0
    je .verify_fail
    
    mov si, msg_payloads_ok
    call print_string_16
    jmp .done

.verify_fail:
    mov si, msg_verify_fail
    call print_string_16
    jmp .done

.read_error:
    mov si, msg_payload_err
    call print_string_16
    
    ; Print AH register (error code from Int 13h)
    mov al, ah
    call print_hex_byte
    
    mov si, msg_at_lba
    call print_string_16
    
    ; Print current LBA (lower 32 bits)
    mov eax, dword [dap_lba]
    call print_hex_dword
    
    call print_newline
    
    ; Ignore error and continue to boot kernel anyway
    
.done:
    popa
    ret

; ── Helper Functions ──────────────────────────────────────────────────────────
print_hex_byte:
    pusha
    mov ah, al
    shr al, 4
    call print_hex_nibble
    mov al, ah
    and al, 0x0F
    call print_hex_nibble
    popa
    ret

print_hex_nibble:
    add al, '0'
    cmp al, '9'
    jbe .digit
    add al, 7  ; 'A' - '9' - 1
.digit:
    mov ah, 0x0E
    mov bx, 0x000F
    int 0x10
    ret

print_hex_dword:
    pusha
    push eax
    shr eax, 24
    mov al, ah
    call print_hex_byte
    pop eax
    push eax
    shr eax, 16
    mov al, ah
    call print_hex_byte
    pop eax
    push eax
    shr eax, 8
    mov al, ah
    call print_hex_byte
    pop eax
    mov al, ah
    call print_hex_byte
    popa
    ret

print_newline:
    pusha
    mov ah, 0x0E
    mov al, 13
    mov bx, 0x000F
    int 0x10
    mov al, 10
    int 0x10
    popa
    ret

; ── Variables ──────────────────────────────────────────────────────────────
msg_loading_payloads db "[S2] Loading Payload Modules (Unreal Mode)... ", 0
msg_chk1             db "CHK1 ", 0
msg_chk2             db "CHK2 ", 0
msg_chk3             db "CHK3 ", 0
msg_chk4             db "CHK4 ", 0
msg_chk5             db "CHK5 ", 0
msg_chk6             db "CHK6 ", 0
msg_payloads_ok      db "OK", 13, 10, 0
msg_payload_err      db "FAIL (Int 13h code=", 0
msg_at_lba           db " at LBA=", 0
msg_verify          db "[S2] Verifying firmware load... ", 0
msg_verify_fail      db "FAIL (firmware is zero)", 13, 10, 0

progress_counter dw 0
payload_base dq 0
payload_size dq 0
