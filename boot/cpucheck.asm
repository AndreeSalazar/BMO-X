; ============================================================================
; FastOS CPU Check — Verify Ryzen 5 5600X capabilities
; ============================================================================
; Checks: CPUID, Long Mode, SSE, AVX, AVX2, FMA3, AES-NI, SHA, BMI1/2
; ============================================================================

[BITS 16]

check_cpuid:
    pushfd
    pop eax
    mov ecx, eax
    xor eax, 1 << 21
    push eax
    popfd
    pushfd
    pop eax
    push ecx
    popfd
    cmp eax, ecx
    je .no_cpuid
    ret
.no_cpuid:
    mov si, msg_no_cpuid
    call print_string_16
    cli
    hlt

check_long_mode:
    mov eax, 0x80000000
    cpuid
    cmp eax, 0x80000001
    jb .no_long_mode
    mov eax, 0x80000001
    cpuid
    test edx, 1 << 29
    jz .no_long_mode
    ret
.no_long_mode:
    mov si, msg_no_lm
    call print_string_16
    cli
    hlt

; ── 32-bit: store CPU features for kernel ────────────────────────────────

[BITS 32]

CPU_FEATURES_ADDR equ 0x9000

; Layout at 0x9000:
;   +0  : CPUID(1) EDX  — SSE(25), SSE2(26)
;   +4  : CPUID(1) ECX  — SSE3(0), SSE4.1(19), SSE4.2(20), AVX(28), AES(25), FMA(12)
;   +8  : CPUID(7) EBX  — AVX2(5), BMI1(3), BMI2(8), SHA(29)
;   +12 : CPUID(7) ECX
;   +16 : CPUID(0x80000001) EDX — Long Mode(29), NX(20)

detect_cpu_features_32:
    mov eax, 1
    cpuid
    mov [CPU_FEATURES_ADDR + 0], edx
    mov [CPU_FEATURES_ADDR + 4], ecx

    mov eax, 7
    xor ecx, ecx
    cpuid
    mov [CPU_FEATURES_ADDR + 8], ebx
    mov [CPU_FEATURES_ADDR + 12], ecx

    mov eax, 0x80000001
    cpuid
    mov [CPU_FEATURES_ADDR + 16], edx
    ret

; ── Messages ─────────────────────────────────────────────────────────────

[BITS 16]
msg_no_cpuid: db "[FastOS] ERROR: No CPUID", 13, 10, 0
msg_no_lm:    db "[FastOS] ERROR: No Long Mode", 13, 10, 0
