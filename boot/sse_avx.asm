; ============================================================================
; FastOS SSE + AVX + AVX2 Initialization (64-bit)
; ============================================================================
; Ryzen 5 5600X: SSE→SSE2→SSE3→SSSE3→SSE4.1→SSE4.2→AVX→AVX2→FMA3
;
; 1. Enable SSE  (CR0.EM=0, CR0.MP=1, CR4.OSFXSR, CR4.OSXMMEXCPT)
; 2. Enable XSAVE (CR4.OSXSAVE)
; 3. Enable AVX via XCR0 (bits 0+1+2 = x87 + SSE + AVX)
; 4. AVX2/FMA3 usable after AVX enabled
; ============================================================================

[BITS 64]

init_sse_avx:
    ; Enable SSE
    mov rax, cr0
    and ax, 0xFFFB              ; Clear CR0.EM (bit 2)
    or ax, 0x0002               ; Set CR0.MP (bit 1)
    mov cr0, rax

    mov rax, cr4
    or ax, (1 << 9)             ; CR4.OSFXSR
    or ax, (1 << 10)            ; CR4.OSXMMEXCPT
    mov cr4, rax

    ; Enable XSAVE
    mov rax, cr4
    or rax, (1 << 18)           ; CR4.OSXSAVE
    mov cr4, rax

    ; Enable AVX via XCR0
    xor rcx, rcx
    xgetbv
    or eax, 0x07                ; x87 + SSE + AVX
    xsetbv

    ret

; AVX2 self-test
avx2_selftest:
    db 0xC5, 0xFD, 0x76, 0xC0      ; vpcmpeqd ymm0, ymm0, ymm0
    db 0xC4, 0xE2, 0x7D, 0x17, 0xC0 ; vptest ymm0, ymm0
    ret
