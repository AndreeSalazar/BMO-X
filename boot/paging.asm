; ============================================================================
; FastOS Paging — 4-level page tables for Long Mode (x86-64)
; ============================================================================
; Identity maps first 4GB using 2MB huge pages.
; Ryzen 5 5600X supports: 4-level, 1GB/2MB/4KB pages, NX, PCID
; ============================================================================

[BITS 32]

PAGE_PRESENT    equ 0x01
PAGE_WRITABLE   equ 0x02
PAGE_HUGE       equ 0x80
PAGE_FLAGS      equ (PAGE_PRESENT | PAGE_WRITABLE)
PAGE_HUGE_FLAGS equ (PAGE_PRESENT | PAGE_WRITABLE | PAGE_HUGE)

PML4_ADDR       equ 0x1000
PDPT_ADDR       equ 0x2000
PD0_ADDR        equ 0x3000
PD1_ADDR        equ 0x4000
PD2_ADDR        equ 0x5000
PD3_ADDR        equ 0x6000

setup_paging:
    ; Clear 6 pages of page tables (24KB)
    mov edi, PML4_ADDR
    xor eax, eax
    mov ecx, 6 * 1024
    rep stosd

    ; PML4[0] → PDPT
    mov dword [PML4_ADDR], PDPT_ADDR | PAGE_FLAGS

    ; PDPT[0..3] → PD0..PD3
    mov dword [PDPT_ADDR + 0*8], PD0_ADDR | PAGE_FLAGS
    mov dword [PDPT_ADDR + 1*8], PD1_ADDR | PAGE_FLAGS
    mov dword [PDPT_ADDR + 2*8], PD2_ADDR | PAGE_FLAGS
    mov dword [PDPT_ADDR + 3*8], PD3_ADDR | PAGE_FLAGS

    ; PD0: 0x00000000 - 0x3FFFFFFF (512 × 2MB pages)
    mov edi, PD0_ADDR
    mov eax, PAGE_HUGE_FLAGS
    mov ecx, 512
.fill_pd0:
    mov [edi], eax
    add eax, 0x200000
    add edi, 8
    loop .fill_pd0

    ; PD1: 0x40000000 - 0x7FFFFFFF
    mov edi, PD1_ADDR
    mov eax, 0x40000000 | PAGE_HUGE_FLAGS
    mov ecx, 512
.fill_pd1:
    mov [edi], eax
    add eax, 0x200000
    add edi, 8
    loop .fill_pd1

    ; PD2: 0x80000000 - 0xBFFFFFFF
    mov edi, PD2_ADDR
    mov eax, 0x80000000 | PAGE_HUGE_FLAGS
    mov ecx, 512
.fill_pd2:
    mov [edi], eax
    add eax, 0x200000
    add edi, 8
    loop .fill_pd2

    ; PD3: 0xC0000000 - 0xFFFFFFFF
    mov edi, PD3_ADDR
    mov eax, 0xC0000000 | PAGE_HUGE_FLAGS
    mov ecx, 512
.fill_pd3:
    mov [edi], eax
    add eax, 0x200000
    add edi, 8
    loop .fill_pd3

    ret
