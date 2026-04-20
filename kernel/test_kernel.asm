; Simple test kernel for UEFI bootloader
; Loaded at 0x100000, shows VGA diagnostic codes
; 64-bit mode for UEFI Long Mode compatibility

[BITS 64]
[ORG 0x100000]

; VGA text buffer at 0xB8000
; Each character: 2 bytes (attribute + char)

_start:
    ; Write 'K' white on red at VGA 0xB8000
    mov rax, 0xB8000
    mov word [rax], 0x4F4B   ; 'K' + red attribute
    
    ; Loop infinito
.hang:
    hlt
    jmp .hang
