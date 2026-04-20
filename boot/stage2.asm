; ============================================================================
; FastOS Stage 2 — Real Mode → Protected Mode → Long Mode
; ============================================================================
; Loaded at 0x7E00 by Stage 1.
; Full transition: 16-bit → 32-bit → 64-bit → Rust kernel.
; Target: AMD Ryzen 5 5600X (Zen 3, Vermeer)
; Board:  MSI MAG B550 TOMAHAWK (MS-7C52) — UEFI CSM quirks handled.
; ============================================================================

[BITS 16]
[ORG 0x7E00]

; ── Entry vector ─────────────────────────────────────────────────────────
; BYTE 0 of stage2.bin — Stage1 jumps here (0x7E00).
; Must be the absolute first instruction. Everything else comes after.
jmp stage2_start

; ── Includes (data tables + subroutines, NOT entry points) ──────────────
%include "gdt.asm"
%include "a20.asm"
%include "memory.asm"
%include "cpucheck.asm"
%include "vbe.asm"
%include "payload_loader.asm"

KERNEL_LOAD_ADDR equ 0x100000
KERNEL_SECTORS   equ 265
GSP_FW_LOAD_ADDR equ 0x1000000    ; 16MB — espacio para el firmware GSP

; --- Print null-terminated string (SI) — local copy for stage2 ---
print_string_16:
    pusha
.ps16_loop:
    lodsb
    or al, al
    jz .ps16_done
    mov ah, 0x0E
    mov bx, 0x0007
    int 0x10
    jmp .ps16_loop
.ps16_done:
    popa
    ret

stage2_start:
    ; ── CRITICAL: Save boot drive (DL) IMMEDIATELY ───────────────────────
    ; BIOS sets DL=0x80 (first hard drive) on boot. Save it NOW before
    ; any code execution to prevent corruption.
    mov [stage2_boot_drive], dl

    ; ── Ultra-early VGA diagnostic ───────────────────────────────────────
    ; Write "S2" directly to VGA text buffer at bottom-left (row 24).
    ; This proves Stage2 code is executing, even if INT 10h or segments
    ; are broken. If you see "S2" on screen but no Stage2 messages,
    ; the crash is happening between here and the first print_string_16.
    mov ax, 0xB800
    mov es, ax
    mov word [es:3840], 0x4F53      ; 'S' white-on-red, row 24, col 0
    mov word [es:3842], 0x4F32      ; '2' white-on-red, row 24, col 1

    ; ── Phase 1: 16-bit Real Mode ────────────────────────────────────────

    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00
    sti

    mov si, msg_s2_start
    call print_string_16

    ; CPU checks
    call check_cpuid
    mov si, msg_cpuid_ok
    call print_string_16

    call check_long_mode
    mov si, msg_lm_ok
    call print_string_16

    ; Enable A20
    call enable_a20
    mov si, msg_a20_ok
    call print_string_16

    ; Detect memory (E801h — E820 hangs on MSI B550 CSM)
    mov si, msg_e820_try
    call print_string_16

    call detect_memory_e820
    mov si, msg_mem_ok
    call print_string_16

    ; ── Load kernel in chunks (64 sectors / 32KB per INT 13h call) ───────
    ; Total: 257 sectors = 131KB loaded to 0x20000 buffer, then copied to 0x100000.
    ; 32KB chunks avoid 64KB segment boundary issues in real mode.
    ; Buffer at 0x20000 provides 256KB of space (more than enough for 131KB kernel).
    ; MSI B550 CSM: AH=42h works despite AH=41h reporting no LBA.
    mov si, msg_loading_kernel
    call print_string_16

    ; Initialize DAP for first chunk
    mov word [dap_k_count], 64
    mov word [dap_k_offset], 0x0000
    mov word [dap_k_seg], 0x2000          ; Segment 0x2000 → phys 0x20000 (128KB)
    mov dword [dap_k_lba], 33             ; LBA 33 (after MBR + Stage2)
    mov dword [dap_k_lba + 4], 0

    mov cx, 4                              ; 4 full chunks of 64
.load_kernel_loop:
    push cx
    mov ah, 0x42
    mov dl, [stage2_boot_drive]
    mov si, dap_kernel
    int 0x13
    jc .kernel_chunk_err

    ; Progress indicator
    mov ah, 0x0E
    mov al, '.'
    mov bx, 0x0007
    int 0x10

    ; Advance DAP for next chunk: segment += 0x800 (+32KB), LBA += 64
    add word [dap_k_seg], 0x800
    add dword [dap_k_lba], 64

    pop cx
    loop .load_kernel_loop

    ; Load final partial chunk (9 sectors = 265 - 256)
    mov word [dap_k_count], 9
    mov ah, 0x42
    mov dl, [stage2_boot_drive]
    mov si, dap_kernel
    int 0x13
    jc .kernel_chunk_err

    ; Progress indicator
    mov ah, 0x0E
    mov al, '.'
    mov bx, 0x0007
    int 0x10

    mov si, msg_kernel_loaded
    call print_string_16

    ; ── Load external modules (GSP Firmware) ─────────────────────────────
    ; Save boot drive (DL) to payload_loader's local variable
    mov al, dl
    mov [payload_boot_drive], al
    call load_payloads

    ; ── Payload loaded, jump directly to Protected Mode transition ───────
    ; Diagnostic: print message before jump
    mov si, msg_entering_pm
    call print_string_16

    jmp .enter_protected_mode

    ; ── VBE: Set 1920x1080x32bpp mode ────────────────────────────────────
    ; This is only executed if we fall through from earlier code
    mov si, msg_vbe_try
    call print_string_16

    call setup_vbe

    cmp byte [vbe_mode_ok], 1
    jne .vbe_skip
    mov si, msg_vbe_ok
    call print_string_16
    jmp .vbe_done
.vbe_skip:
    mov si, msg_vbe_fail
    call print_string_16
.vbe_done:

.enter_protected_mode:
    ; ── Phase 2: Enter 32-bit Protected Mode ─────────────────────────────

    ; Save current BIOS cursor row for VGA continuation in PM
    mov ah, 0x03
    xor bh, bh
    int 0x10
    movzx eax, dh
    mov [vga_row], al

    cli
    lgdt [gdt32_descriptor]

    mov eax, cr0
    or eax, 1
    mov cr0, eax

    jmp GDT32_CODE_SEG:protected_mode_entry

.kernel_chunk_err:
    pop cx                                 ; Balance push cx
.kernel_load_err:
    mov si, msg_kernel_err
    call print_string_16
    cli
    hlt

; ── 16-bit Data ──────────────────────────────────────────────────────────

msg_s2_start:       db "[FastOS] Stage2: starting", 13, 10, 0
msg_cpuid_ok:       db "[FastOS] CPUID: OK", 13, 10, 0
msg_lm_ok:          db "[FastOS] Long Mode: OK", 13, 10, 0
msg_a20_ok:         db "[FastOS] A20: enabled", 13, 10, 0
msg_e820_try:       db "[FastOS] Detecting memory (E801h)...", 13, 10, 0
msg_mem_ok:         db "[FastOS] Memory map: OK", 13, 10, 0
msg_loading_kernel: db "[FastOS] Loading kernel", 0
msg_kernel_loaded:  db 13, 10, "[FastOS] Kernel loaded OK", 13, 10, 0
msg_kernel_err:     db 13, 10, "[FastOS] KERNEL LOAD ERROR!", 13, 10, 0
msg_vbe_try:        db "[FastOS] Setting 1920x1080x32...", 13, 10, 0
msg_vbe_ok:         db "[FastOS] VBE: 1920x1080x32bpp OK!", 13, 10, 0
msg_vbe_fail:       db "[FastOS] VBE: Failed, using VGA text", 13, 10, 0
msg_entering_pm:    db "[FastOS] Entering PM -> LM -> Kernel!", 13, 10, 0

stage2_boot_drive: db 0
vga_row:           db 0

align 4
dap_kernel:
    db 0x10
    db 0
dap_k_count:  dw 64                      ; sectors per chunk (64 × 512 = 32KB)
dap_k_offset: dw 0x0000
dap_k_seg:    dw 0x1000                   ; initial segment → phys 0x10000
dap_k_lba:    dq 33                       ; initial LBA (after MBR + Stage2)

; ── 32-bit Protected Mode ────────────────────────────────────────────────

[BITS 32]

%include "paging.asm"

protected_mode_entry:
    mov ax, GDT32_DATA_SEG
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, 0x90000

    ; VGA print — continue from where BIOS left off (don't overwrite!)
    ; Read saved row from real mode, compute offset
    movzx eax, byte [vga_row]
    imul eax, 160                   ; 80 cols × 2 bytes per char
    add eax, 0xB8000
    mov edi, eax
    mov esi, msg_pm_ok
    mov ah, 0x0A                    ; light green on black
.pm_print:
    lodsb
    test al, al
    jz .pm_print_done
    stosw
    jmp .pm_print
.pm_print_done:

    ; Detect CPU features for kernel
    call detect_cpu_features_32

    ; Copy kernel: 0x20000 → 0x100000 (1MB), 131KB
    mov esi, 0x20000
    mov edi, 0x100000
    mov ecx, 32770                  ; 131KB / 4 bytes = 32770 dwords
    rep movsd

    ; ── Phase 3: Paging + Long Mode ──────────────────────────────────────

    ; VGA progress — next row
    movzx eax, byte [vga_row]
    inc eax
    imul eax, 160
    add eax, 0xB8000
    mov edi, eax
    mov esi, msg_paging_ok
    mov ah, 0x0E                    ; yellow on black
.pg_print:
    lodsb
    test al, al
    jz .pg_print_done
    stosw
    jmp .pg_print
.pg_print_done:

    call setup_paging

    ; Load PML4 into CR3
    mov eax, PML4_ADDR
    mov cr3, eax

    ; Enable PAE
    mov eax, cr4
    or eax, (1 << 5)
    mov cr4, eax

    ; Enable Long Mode (EFER.LME) + NX (EFER.NXE)
    mov ecx, 0xC0000080
    rdmsr
    or eax, (1 << 8)               ; LME
    or eax, (1 << 11)              ; NXE
    wrmsr

    ; Enable paging + write protect
    mov eax, cr0
    or eax, (1 << 31)              ; PG
    or eax, (1 << 16)              ; WP
    mov cr0, eax

    ; Load 64-bit GDT
    lgdt [gdt64_descriptor]

    ; Far jump to 64-bit!
    jmp GDT64_CODE_SEG:long_mode_entry

msg_pm_ok:      db "[FastOS] Protected Mode: OK", 0
msg_paging_ok:  db "[FastOS] Paging + Long Mode setup...", 0

; ── 64-bit Long Mode ────────────────────────────────────────────────────

[BITS 64]
DEFAULT ABS

%include "sse_avx.asm"

long_mode_entry:
    mov ax, GDT64_DATA_SEG
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov rsp, 0x800000

    ; VGA: Long Mode active — row after paging msg
    movzx rax, byte [vga_row]
    add rax, 2                      ; PM row + paging row
    imul rax, 160
    add rax, 0xB8000
    mov rdi, rax
    mov rsi, msg_lm_active
    mov ah, 0x0E                    ; yellow on black
.lm_print:
    lodsb
    test al, al
    jz .lm_print_done
    stosw
    jmp .lm_print
.lm_print_done:

    ; Init SSE + AVX + AVX2
    call init_sse_avx
    call avx2_selftest

    ; VGA: AVX2 ready — next row
    movzx rax, byte [vga_row]
    add rax, 3
    imul rax, 160
    add rax, 0xB8000
    mov rdi, rax
    mov rsi, msg_avx2_ok
    mov ah, 0x0A                    ; light green
.avx_print:
    lodsb
    test al, al
    jz .avx_print_done
    stosw
    jmp .avx_print
.avx_print_done:

    ; VGA: jumping to kernel — next row
    movzx rax, byte [vga_row]
    add rax, 4
    imul rax, 160
    add rax, 0xB8000
    mov rdi, rax
    mov rsi, msg_jump_kernel
    mov ah, 0x0F                    ; white on black
.jk_print:
    lodsb
    test al, al
    jz .jk_print_done
    stosw
    jmp .jk_print
.jk_print_done:

    ; ── Build boot info for Rust kernel ──────────────────────────────────

    ; Boot info at 0x9100 (passed in RDI to Rust)
    mov qword [0x9100], 0xFA5705           ; magic
    mov qword [0x9108], 0x8000             ; memory_map_addr
    mov eax, [0x8000]
    mov qword [0x9110], rax                ; memory_map_count
    mov qword [0x9118], 0x9000             ; cpu_features_addr

    ; Framebuffer: use VBE address if set, otherwise VGA 0xB8000
    mov eax, [vbe_fb_addr]
    mov qword [0x9120], rax                ; framebuffer_addr
    mov qword [0x9128], 0x100000           ; kernel_start
    mov qword [0x9130], 132 * 1024         ; kernel_size (132KB)

    ; Extended boot info: VBE data
    movzx eax, word [vbe_pitch]
    mov qword [0x9138], rax                ; fb_pitch
    movzx eax, byte [vbe_mode_ok]
    mov qword [0x9140], rax                ; vbe_mode (1=graphics, 0=text)

    ; Extended boot info: GSP Firmware (loaded by payload_loader.asm)
    mov eax, dword [payload_base]
    mov qword [0x9148], rax                ; gpu_fw_addr
    mov eax, dword [payload_size]
    mov qword [0x9150], rax                ; gpu_fw_size

    ; ── Jump to Rust kernel with System V AMD64 ABI arguments ─────────────
    ; RDI = GSP firmware address (0x1000000)
    ; RSI = GSP firmware size (72845296)
    ; RDX = memory map pointer (0x8000)

    mov rdi, 0x1000000              ; GSP firmware address
    mov rsi, 72845296               ; GSP firmware size
    mov rdx, 0x8000                 ; Memory map pointer

    mov rax, 0x100000              ; Kernel entry point
    jmp rax

    cli
    hlt

msg_lm_active:   db "[FastOS] 64-bit Long Mode: ACTIVE", 0
msg_avx2_ok:     db "[FastOS] SSE4.2+AVX2+FMA3: READY (Zen 3)", 0
msg_jump_kernel: db "[FastOS] Jumping to Rust kernel @ 0x100000...", 0

; Pad stage2 to 16KB (32 sectors)
times (16384) - ($ - $$) db 0
