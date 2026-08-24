//! BMO AHCI/SATA Storage Driver -- HAL-based.
//!
//! Implements controller detection, port enumeration, DMA setup,
//! and sector read/write. Uses `StorageHal` trait for kernel services
//! (memory allocation and logging).

#![no_std]

pub mod storage_hal;
pub mod controller;
/// **Arrancar el HBA**: prepararlo, censar puertos y montar el DMA. Ocurre UNA
/// vez por arranque, y ahi vive la mina de la PRDT.
mod arranque;
/// **Un comando**: emitirlo, esperarlo y saber en que quedo. Esto corre en cada
/// lectura y en cada escritura del sistema.
mod comando;

pub use storage_hal::StorageHal;
pub use controller::{AhciController, PortState, AhciPort, DiskError, read_sectors_phys, write_sectors_phys, flush_cache, identify_phys, probe, init_port_dma, controller, reset_ctrl, SIG_SATA_DISK, SECTOR};
/// **TRIM.** El unico comando de este driver que usa el registro de features:
/// `DATA SET MANAGEMENT` no dice que hacer, lo dice su funcion. El payload lo
/// arma `bmo-trim`, que no toca hardware y por eso se prueba en el anfitrion.
pub use controller::trim_phys;
/// **Pedir sin esperar**: emitir el comando y preguntar despues en que va.
/// Ver [`controller::Estado`] -- es lo que hace posible la E/S asincrona, y lo
/// que `run_command` esconde detras de un bucle que gira.
pub use controller::{emitir, sondear, Estado, ATA_CMD_READ_DMA_EX};
/// **Que el aparato avise.** `habilitar_irq` abre la puerta (despues de armar
/// MSI, nunca antes) y `atender` limpia el aviso desde el manejador.
pub use controller::{atender, habilitar_irq, AVISOS};
