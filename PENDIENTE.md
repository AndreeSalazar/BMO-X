FALTA IMPLEMENTAR:
1. boot_protocol/src/lib.rs
   - Agregar vbios_addr: u64 y vbios_size: u64 a BootInfo struct

2. bootloader/src/main.rs  
   - Leer firmware/vbios_rtx3060.rom del USB
   - Pasar addr+size al kernel via BootInfo

3. kernel/src/drivers/gsp/loader.rs
   - Implementar fwsec_frts(vbios: &[u8])
   - Extraer FWSEC ucode del VBIOS
   - DMA a SEC2 IMEM
   - Ejecutar ANTES de booter_load
   - Verificar WPR2_HI != 0 después