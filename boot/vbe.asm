; ============================================================================
; FastOS VBE — VESA BIOS Extensions for 1920×1080 framebuffer
; ============================================================================
; Sets 1920×1080×32bpp mode using INT 10h AX=4F02h (VBE 2.0+).
; The GPU's VBIOS (loaded by MSI B550 CSM) provides VBE support.
;
; Output:
;   [vbe_fb_addr]   = u32 physical address of linear framebuffer
;   [vbe_mode_ok]   = 1 if mode set succeeded, 0 if fallback to VGA text
; ============================================================================

[BITS 16]

VBE_INFO_BLOCK  equ 0x7000     ; Temporary buffer for VBE info (512 bytes)
VBE_MODE_INFO   equ 0x7200     ; Temporary buffer for mode info (256 bytes)

; Desired mode parameters
VBE_WANT_WIDTH  equ 1920
VBE_WANT_HEIGHT equ 1080
VBE_WANT_BPP    equ 32

setup_vbe:
    ; ── Step 1: Get VBE controller info ──────────────────────────────
    mov ax, 0x4F00
    mov di, VBE_INFO_BLOCK
    mov dword [di], 'VBE2'     ; Request VBE 2.0+ info
    int 0x10
    cmp ax, 0x004F
    jne .vbe_fail               ; VBE not supported

    ; ── Step 2: Walk mode list to find 1920×1080×32bpp ──────────────
    ; Mode list pointer is at VBE_INFO_BLOCK + 14 (far pointer seg:off)
    mov si, [VBE_INFO_BLOCK + 14]    ; offset
    mov ax, [VBE_INFO_BLOCK + 16]    ; segment
    mov fs, ax                        ; FS:SI = mode list

.scan_modes:
    mov cx, [fs:si]
    cmp cx, 0xFFFF                    ; End of mode list
    je .vbe_fail
    add si, 2

    ; Get info for this mode
    push si
    push cx
    mov ax, 0x4F01
    mov di, VBE_MODE_INFO
    int 0x10
    pop cx
    pop si

    cmp ax, 0x004F
    jne .scan_modes                   ; Skip if query failed

    ; Check mode attributes (bit 0 = supported, bit 7 = LFB available)
    mov ax, [VBE_MODE_INFO + 0]
    test ax, 0x0081                    ; Supported + LFB
    jz .scan_modes

    ; Check resolution
    cmp word [VBE_MODE_INFO + 18], VBE_WANT_WIDTH
    jne .scan_modes
    cmp word [VBE_MODE_INFO + 20], VBE_WANT_HEIGHT
    jne .scan_modes

    ; Check bits per pixel
    cmp byte [VBE_MODE_INFO + 25], VBE_WANT_BPP
    jne .scan_modes

    ; Found our mode! CX = mode number
    ; ── Step 3: Set the mode ────────────────────────────────────────
    mov bx, cx
    or bx, 0x4000                     ; Bit 14 = use linear framebuffer
    mov ax, 0x4F02
    int 0x10
    cmp ax, 0x004F
    jne .vbe_fail

    ; ── Step 4: Save framebuffer address ────────────────────────────
    mov eax, [VBE_MODE_INFO + 40]     ; PhysBasePtr
    mov [vbe_fb_addr], eax
    mov word [vbe_pitch], 0
    mov ax, [VBE_MODE_INFO + 16]      ; BytesPerScanLine
    mov [vbe_pitch], ax
    mov byte [vbe_mode_ok], 1

    ret

.vbe_fail:
    ; VBE failed — stay in VGA text mode (80×25)
    mov byte [vbe_mode_ok], 0
    mov dword [vbe_fb_addr], 0xB8000
    mov word [vbe_pitch], 160         ; 80 * 2 for VGA text
    ret

; ── VBE Data ────────────────────────────────────────────────────────────
vbe_fb_addr:  dd 0
vbe_pitch:    dw 0
vbe_mode_ok:  db 0
