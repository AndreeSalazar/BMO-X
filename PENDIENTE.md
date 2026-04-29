FALTA IMPLEMENTAR:
1. boot_protocol/src/lib.rs ✅ HECHO
   - vbios_addr: u64 y vbios_size: u64 ya están en BootInfo (líneas 92-93)

2. bootloader/src/main.rs ✅ HECHO
   - Lee firmware/vbios_rtx3060.rom del USB
   - Pasa addr+size al kernel via BootInfo

3. kernel/src/drivers/gsp/loader.rs — FWSEC-FRTS ⚠️ PARCIALMENTE IMPLEMENTADO
   - `fwsec_frts()` existe (línea 852) pero WPR2_HI sigue en 0x0
   - Se ejecuta en `load_full()` paso [7/11] (línea 1329)

   ## Estado actual del problema

   FWSEC-FRTS se ejecuta, SEC2 arranca, pero WPR2_HI=0x0 después del timeout.
   Causas probables (en orden de prioridad):

   ### A. FWSEC-FRTS NO es un HS (Heavy Secure) boot en SEC2 — es GSP Falcon
   **PROBLEMA CRÍTICO:** En GA10x (Ampere), FWSEC-FRTS corre en el **PGSP Falcon**
   en modo HS, NO en SEC2. El código actual DMA a SEC2 IMEM/DMEM, pero nouveau
   (ga102_gsp_fwsec.c) y nova lo ejecutan en PGSP:
   - nouveau: `nvkm_falcon_fw_boot(&fwsec, ...pgsp_falcon...)` 
   - nova:    ejecuta en PGSP → verifica WPR2_HI != 0
   
   **Solución:** Cambiar fwsec_frts() para:
   1. DMA IMEM/DMEM al **PGSP Falcon** (0x0011_xxxx), NO SEC2 (0x0084_xxxx)
   2. Registros PGSP para DMA:
      - NV_PGSP_DMATRFBASE  = 0x0011_0110
      - NV_PGSP_DMATRFMOFFS = 0x0011_0114
      - NV_PGSP_DMATRFCMD   = 0x0011_0118
      - NV_PGSP_DMATRFFBOFFS= 0x0011_011C
      - NV_PGSP_FALCON_CPUCTL   = 0x0011_0100
      - NV_PGSP_FALCON_BOOTVEC  = 0x0011_0104
      - NV_PGSP_FALCON_RESET    = 0x0011_0094
      - NV_PGSP_FALCON_IDLESTATE= 0x0011_0004
   3. NO poner GSP en modo RISC-V antes de FWSEC (FWSEC es Falcon nativo)
   4. Boot PGSP Falcon con FWSEC ucode
   5. Esperar WPR2_HI != 0
   6. DESPUÉS de FWSEC OK → resetear GSP → cambiar a RISC-V mode → continuar

   ### B. Offsets FWSEC del VBIOS hardcodeados
   Los offsets actuales vienen de _find_fwsec7.ps1 para este VBIOS específico:
   ```
   FWSEC v3 Descriptor: SPI 0x4A410 (type 0x85 = FWSEC_PROD)
   IMEM: 57,856 bytes @ SPI 0x4A8BC (= descriptor + hdrSize 0x4AC)
   DMEM:  2,048 bytes @ SPI 0x58ABC (= IMEM_off + IMEM_size)
   InterfaceOffset: 0x1C (dentro de DMEM)
   EngineIdMask: 0x400 (GSP Falcon, bit 10)
   UcodeId: 9
   PKCDataOffset: varía (firma RSA-3072)
   ```
   Los offsets están bien para vbios_rtx3060.rom pero deberían parsearse
   dinámicamente vía BIT 'p' token → PMU table → type 0x85 entry.
   Para MVP hardcodeado está OK.

   ### C. Interface patching incompleto
   El código actual parchea engine_id_mask + ucode_id en DMEM+0x1C.
   nouveau/nova además parchea:
   - `FRTS offset` en VRAM (dirección donde FWSEC escribirá la FRTS)
   - Esto normalmente va en el campo de interfaz del DMEM
   - Sin esto, FWSEC no sabe DÓNDE en VRAM escribir la tabla FRTS

   ### D. Descriptor v3 completo (de _find_fwsec7.ps1)
   ```
   Hdr:            0x04AC0301 (ver=3, size=0x04AC, valid=1)
   StoredSize:     57856 + 2048 = 59904 bytes total ucode
   PKCDataOffset:  offset a firmas PKC (RSA-3072)
   InterfaceOffset:0x1C
   IMEMPhysBase:   depende del VBIOS
   IMEMLoadSize:   0xE200 = 57856
   IMEMVirtBase:   0x0
   DMEMPhysBase:   depende
   DMEMLoadSize:   0x800 = 2048
   EngineIdMask:   0x0400
   UcodeId:        9
   SignatureCount: varía (típico 1-4)
   ```

   ### E. Secuencia correcta en load_full()
   ```
   Paso actual                    Paso correcto
   ─────────────────────────────  ──────────────────────────────
   [1] PRIV Ring init             [1] PRIV Ring init
   [2] Radix3 page table          [2] Radix3 page table  
   [3] WPR meta                   [3] WPR meta
   [4] Boot memory                [4] Boot memory
   [5] Reset GSP → RISC-V mode    [5] FWSEC-FRTS en PGSP Falcon ← ANTES de RISC-V
   [6] PGSP MAILBOX               [6] Verificar WPR2_HI != 0
   [7] FWSEC en SEC2 ← MAL        [7] Reset GSP → RISC-V mode
   [8] HS booter on SEC2          [8] PGSP MAILBOX
   [9-11] verify                  [9] HS booter_load on SEC2
                                  [10-11] verify
   ```

   ## Archivos a modificar

   1. `kernel/src/drivers/gsp/loader.rs`:
      - Añadir `pgsp_dma_xfer_256()` (como sec2_dma_xfer_256 pero con registros PGSP)
      - Reescribir `fwsec_frts()` para usar PGSP en vez de SEC2
      - Reordenar `load_full()` según secuencia correcta arriba

   2. `kernel/src/drivers/gsp/rpc.rs`:
      - Añadir constantes PGSP DMA si no existen:
        ```
        NV_PGSP_DMATRFBASE   = 0x0011_0110
        NV_PGSP_DMATRFMOFFS  = 0x0011_0114
        NV_PGSP_DMATRFCMD    = 0x0011_0118
        NV_PGSP_DMATRFFBOFFS = 0x0011_011C
        ```

   ## Bug secundario en sec2_hs_boot_booter (líneas 472-484)
   ```rust
   // ❌ patch_sig_value es un ÍNDICE de firma (0=prod), NO un offset DMEM
   let sig_patch_off = hs.patch_sig_value.checked_sub(hs.os_data_offset);
   ```
   No causa daño (0 - X = None → no-op) pero la semántica es incorrecta.
   `patch_sig_value` = índice de firma seleccionado (0 = prod).
   No necesita patching en DMEM. Eliminar ese bloque.

   ## Valores HS manifest confirmados OK
   - os_code_sz = 0x100 (256 bytes IMEM stub) ✅ correcto para GA10x booter
   - patch_sig_value = 0x0 (sig index 0 = prod) ✅ correcto
   - patch_loc=0x33C, patch_loc_value=0x410 ✅ correcto
   - dmem_sign = patch_loc_value - os_data_offset ✅ correcto
