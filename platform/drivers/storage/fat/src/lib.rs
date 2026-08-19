//! **FAT12/16/32: la FRONTERA de BMO-X, no su sistema de ficheros.**
//!
//! Plan completo: `docs/plan/PLAN_FAT32.md`.
//!
//! # Por que esta crate existe teniendo ESTRATOS
//!
//! El firmware de la placa lee `\EFI\BOOT\BOOTX64.EFI` **con su propio driver
//! FAT, antes de que exista una sola instruccion de BMO**. La particion de
//! arranque no es una eleccion de diseno: es el apreton de manos con la
//! maquina. El dia que ESTRATOS sea perfecto, la ESP seguira siendo FAT32.
//!
//! Asi que aqui no se construye un sistema de ficheros. Se construye **el
//! traductor de la frontera**: leer bien lo que el firmware escribio, escribir
//! lo que tiene que cruzar al otro lado, y decir la verdad sobre el volumen.
//! Todo lo demas --borrar, renombrar, escribir en medio de un archivo,
//! historial-- es de ESTRATOS y no se hace aqui.
//!
//! # Por que se llama `fat` y no `fat32`
//!
//! FAT12, FAT16 y FAT32 son **un formato con tres anchuras de entrada**: mismo
//! BPB, misma entrada de directorio de 32 bytes, mismo nombre largo. Lo unico
//! que cambia de verdad es cuantos bits ocupa una entrada de la tabla y donde
//! vive el directorio raiz.
//!
//! Y hacen falta los tres: UEFI admite FAT12 y FAT16 en la particion de
//! arranque de un medio extraible, o sea **en cualquier pendrive**.
//!
//! exFAT NO vive aqui. Se muda a `bmo-exfat` (paso 7 del plan) porque no
//! comparte casi nada --mapa de bits en vez de FAT libre, tabla de mayusculas,
//! entradas 0x85/0xC0/0xC1, sin nombre largo--, y juntarlos es lo que obligaba
//! al driver de antes a preguntar "quien soy" en seis metodos distintos.
//!
//! # Estado
//!
//! ```text
//!   [x] bpb.rs        QUE VOLUMEN ES ESTE?   -- puro, 22 casillas sin disco
//!   [x] dir/larga.rs  EL NOMBRE DE VERDAD    -- LFN; 15 casillas sin disco
//!   [ ] tabla.rs      DONDE SIGUE ESTO?
//!   [ ] reservada.rs  la zona libre de la seccion 4.B1 del plan
//!   [ ] espacio.rs    DE DONDE SACO SITIO?
//!   [ ] dir/recorrer.rs, datos.rs, tiempo.rs, estado.rs
//! ```
//!
//! `bmo-fat32` sigue en pie y es la que usa el kernel. Esta la sustituye
//! cuando pase sus 21 casillas, no antes.
//!
//! # La regla que este arbol de ficheros obedece
//!
//! Un fichero por PREGUNTA que responde, no por capa (criterio C de
//! `PLAN_ALMACENAMIENTO.md`). Y los que no tocan disco van primero, porque son
//! los que se pueden probar: `bpb` recibe un sector ya leido y contesta, igual
//! que `bmo-particiones`. El bucle que pide sectores vive arriba.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod bpb;
pub mod dir;

pub use bpb::{cuadra_con_respaldo, identificar, leer_fsinfo, FsInfo, Geometria, NoEs, Raiz, Tipo};
