; ============================================================================
; FastOS Payload Loader — Unreal Mode for GSP Firmware
; ============================================================================
; Target: RTX 3060 12GB (GA106)
; Board: MSI MS-7C52 (B550), AMI Aptio V BIOS
; 
; PROBLEM: AMI Aptio V destroys segment limits on every BIOS interrupt.
; SOLUTION: Re-enter Unreal Mode after every INT 13h call.
; ============================================================================

[BITS 16]

; ── Constants ───────────────────────────────────────────────────────────────
GSP_FW_LOAD_ADDR   equ 0x1000000      ; 16MB - firmware target
GSP_FW_SIZE         equ 72845296       ; 69.5MB = 142,196 sectors
SECTORS_PER_BLOCK   equ 16             ; B550 limit = 16 sectors (8KB)
BLOCK_SIZE          equ 8192           ; 16 * 512 = 8KB
BUFFER_ADDR         equ 0x20000        ; 128KB - safe buffer
TOTAL_ITERATIONS    equ 8888           ; 142,196 / 16 = 8887.25 → 8888

; ── DAP for INT 13h AH=42h ───────────────────────────────────────────────────
align 4
dap:
    db 0x10               ; DAP size (16 bytes)
    db 0                  ; Reserved
dap_count:
    dw SECTORS_PER_BLOCK  ; 16 sectors (8KB) - B550 limit
dap_buffer_offset:
    dw 0x0000             ; Offset = 0
dap_buffer_segment:
    dw 0x2000             ; Segment 0x2000 → 0x20000 physical (128KB)
dap_lba:
    dq 0                  ; LBA (set dynamically)

; ── Unreal Mode GDT ────────────────────────────────────────────────────────
; AMI Aptio V B550 requires proper 4GB data descriptor
align 8
unreal_gdt:
    dq 0                  ; Null descriptor
unreal_data_desc:
    ; 4GB Data Descriptor (Base 0, Limit 4GB, 32-bit)
    dw 0xFFFF             ; Limit bits 0-15
    dw 0x0000             ; Base bits 0-15
    db 0x00               ; Base bits 16-23
    db 10010010b          ; Present, Ring 0, Data, R/W
    db 11001111b          ; Granularity=4KB, Size=32-bit, Limit bits 16-19=0xF
    db 0x00               ; Base bits 24-31
    ; Effective limit: 0xFFFFF * 4KB = 4GB

unreal_gdt_desc:
    dw unreal_gdt_desc - unreal_gdt - 1  ; Limit
    dd unreal_gdt                          ; Base (physical address)

; ── External symbols from stage2.asm ─────────────────────────────────────────
extern stage2_boot_drive
extern print_string_16

; ── load_payloads ──────────────────────────────────────────────────────────
; Entry point called from stage2.asm
; Output: Sets payload_base and payload_size for kernel
load_payloads:
    pusha

    ; Print start message (before CLI)
    mov si, msg_start
    call print_string_16

    ; ── STEP 1: Enter Unreal Mode ONCE before loop ───────────────────────
    cli                     ; Disable interrupts for Unreal Mode transition

    ; Load GDT
    lgdt [unreal_gdt_desc]

    ; Enable Protected Mode
    mov eax, cr0
    or eax, 1              ; Set PE bit
    mov cr0, eax

    ; Load 4GB data descriptor into DS/ES/SS
    mov ax, 0x08           ; Selector for 4GB descriptor
    mov ds, ax
    mov es, ax
    mov ss, ax

    ; Disable Protected Mode (back to Real Mode)
    and eax, 0xFFFFFFFE    ; Clear PE bit
    mov cr0, eax

    ; Now in Unreal Mode: segments keep 4GB hidden limit
    ; CS stays in Real Mode (16-bit), DS/ES/SS have 4GB limits

    ; ── STEP 2: Initialize loop variables ───────────────────────────────────
    mov dword [dap_lba], 1000     ; Start at LBA 1000
    mov dword [dap_lba+4], 0

    mov edi, GSP_FW_LOAD_ADDR     ; Destination: 0x1000000
    mov cx, TOTAL_ITERATIONS      ; 8888 iterations

    ; ── STEP 3: Main loading loop (CLI active, NO int 0x10) ─────────────────
.load_loop:
    push cx                  ; Save iteration counter

    ; Check for last iteration (partial block)
    cmp cx, 1
    jne .full_block
    mov word [dap_count], 4      ; Last block: 4 sectors (2KB)
    jmp .do_read

.full_block:
    mov word [dap_count], SECTORS_PER_BLOCK  ; 16 sectors (8KB)

.do_read:
    ; ── CALL BIOS INT 13h AH=42h ────────────────────────────────────────
    ; This destroys segment limits on AMI Aptio V
    mov ah, 0x42
    mov dl, [stage2_boot_drive]
    mov si, dap
    int 0x13
    jc .read_error

    ; ── CRITICAL: Re-enter Unreal Mode after BIOS call ───────────────────
    ; AMI Aptio V restores 16-bit segments on every interrupt
    ; We must re-load the 4GB descriptor
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

    ; ── Copy 8KB from buffer (0x20000) to target (EDI) ───────────────────
    ; Using 32-bit addressing in Unreal Mode
    push edi
    push esi

    mov esi, BUFFER_ADDR          ; Source: 0x20000
    mov ecx, BLOCK_SIZE / 4      ; 8192 / 4 = 2048 dwords

    ; rep movsd with 32-bit addressing
    ; a32 prefix forces 32-bit address size in 16-bit mode
    a32 rep movsd

    pop esi
    pop edi

    ; ── Advance destination by 8KB ───────────────────────────────────────
    add edi, BLOCK_SIZE          ; EDI += 8192

    ; ── Advance LBA by 16 sectors ───────────────────────────────────────────
    mov eax, dword [dap_lba]
    add eax, SECTORS_PER_BLOCK   ; EAX += 16
    mov dword [dap_lba], eax

    ; ── Next iteration ─────────────────────────────────────────────────────
    pop cx
    dec cx
    jnz .load_loop

    ; ── STEP 4: Loop complete, restore segments ───────────────────────────
    xor ax, ax
    mov ds, ax              ; DS = 0 (Real Mode)
    mov es, ax              ; ES = 0
    mov ss, ax              ; SS = 0
    sti                     ; Re-enable interrupts

    ; ── STEP 5: Save firmware info for kernel ─────────────────────────────
    mov dword [payload_base], GSP_FW_LOAD_ADDR
    mov dword [payload_base+4], 0
    mov dword [payload_size], GSP_FW_SIZE
    mov dword [payload_size+4], 0

    ; Print success message
    mov si, msg_done
    call print_string_16

    popa
    ret

.read_error:
    ; Handle read error (should not happen with proper DAP)
    ; Restore segments before printing
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    sti

    mov si, msg_error
    call print_string_16

    ; Continue anyway (kernel will handle missing firmware)
    popa
    ret

; ── Variables ──────────────────────────────────────────────────────────────
msg_start  db "[S2] Loading GSP firmware (Unreal Mode)...", 13, 10, 0
msg_done   db "[S2] GSP firmware loaded OK", 13, 10, 0
msg_error  db "[S2] ERROR: Firmware load failed", 13, 10, 0

payload_base dq 0
payload_size dq 0
