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
align 4
dap:
    db 0x10               ; Size of DAP (16 bytes)
    db 0                  ; Unused
dap_count:
    dw 64                 ; Number of sectors to read (32KB blocks)
dap_buffer_offset:
    dw 0x0000             ; Offset
dap_buffer_segment:
    dw 0x1000             ; Segment (0x1000:0x0000 = 0x10000 physical)
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

    ; Step 1: Enter Unreal Mode
    cli
    push ds
    push es

    ; Load Unreal GDT
    lgdt [unreal_gdt_desc]

    ; Enable PM bit in CR0 briefly
    mov eax, cr0
    or al, 1
    mov cr0, eax

    ; Load DS and ES with 4GB limit selector (0x08)
    mov bx, 0x08
    mov ds, bx
    mov es, bx

    ; Disable PM bit in CR0 (back to Real Mode)
    and al, 0xFE
    mov cr0, eax

    ; Restore DS and ES to 0, but they keep the 4GB hidden limit!
    pop es
    pop ds
    sti

    ; Step 2: Read Módulo 1 (GSP Firmware dummy)
    ; In build.ps1, the firmware is written at LBA 1000.
    ; For now, we will read 1024 sectors (512 KB) just to prove architecture.
    ; In the future we read a header from disk.
    
    mov dword [dap_lba], 1000     ; Start reading at LBA 1000
    mov dword [dap_lba+4], 0
    
    mov ecx, 16                   ; Loop 16 times * 64 sectors = 1024 sectors (512 KB)
    mov edi, 0x02000000           ; Physical destination (32 MB mark)
    
.read_loop:
    push cx
    
    ; Setup DAP to read 64 sectors (32 KB)
    mov word [dap_count], 64

    ; Call BIOS Int 13h AH=42h
    mov ah, 0x42
    mov dl, [stage2_boot_drive]
    mov si, dap
    int 0x13
    jc .read_error

    ; Copy 32KB from buffer (0x10000) to high memory (EDI)
    ; Since we are in Unreal Mode, we can use 32-bit registers for address!
    push edi
    push esi
    
    ; Source is physical 0x10000
    mov esi, 0x10000
    ; Count = 32768 bytes / 4 = 8192 dwords
    mov ecx, 8192
    
    ; rep movsd (32-bit copy in Real Mode thanks to Unreal Mode limits)
    ; We must use a segment prefix if not using DS, but DS=0 so ESI=0x10000 is linear.
    a32 rep movsd

    pop esi
    pop edi

    ; Increment destination by 32KB
    add edi, 32768
    
    ; Increment LBA by 64
    mov eax, dword [dap_lba]
    add eax, 64
    mov dword [dap_lba], eax

    pop cx
    dec cx
    jnz .read_loop

    ; Save variables for the kernel
    ; Base: 0x02000000
    ; Size: 512 * 1024 = 524288
    mov dword [payload_base], 0x02000000
    mov dword [payload_base+4], 0
    mov dword [payload_size], 524288
    mov dword [payload_size+4], 0

    mov si, msg_payloads_ok
    call print_string_16
    jmp .done

.read_error:
    mov si, msg_payload_err
    call print_string_16
    ; Ignore error and continue to boot kernel anyway
    
.done:
    popa
    ret

; ── Variables ──────────────────────────────────────────────────────────────
msg_loading_payloads db "[S2] Loading Payload Modules (Unreal Mode)... ", 0
msg_payloads_ok      db "OK", 13, 10, 0
msg_payload_err      db "FAIL (Int 13h code)", 13, 10, 0

payload_base dq 0
payload_size dq 0
