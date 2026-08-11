//! **RED. Vacio a proposito** -- 2026-08-11.
//!
//! ## Que habia aqui, y por que no esta
//!
//! 287 lineas de driver de **Intel e1000**: registros `RDBAL/TDBAL`, anillos de
//! descriptores, `probe`/`send`/`receive`. Compilaba. No lo llamaba **nadie**
//! --uno de los crates huerfanos de la auditoria del 2026-07-26-- y sobre todo:
//!
//! > **La NIC de esta maquina es `PCI\VEN_10EC&DEV_8168`** -- una Realtek
//! > RTL8111/8168, la de casi toda placa MSI A320M.
//!
//! El e1000 es la NIC por defecto de **QEMU**. O sea que ese codigo no habria
//! encendido un LED en el Ryzen: era un driver para el emulador, escrito antes
//! de mirar el aparato. Es el mismo hallazgo que el del sonido, donde el
//! audifono resulto ser USB y no HDA -- ver `bmo-uaudio`.
//!
//! Borrarlo no es tirar trabajo: es dejar de tener una respuesta a una pregunta
//! que nadie hizo. Sigue en el historial (`git log -- platform/drivers/net`)
//! por si el dia que haga falta un e1000 --una maquina virtual, otra placa--
//! sale mas barato leerlo que reescribirlo.
//!
//! ## El contrato que va a ocupar este sitio
//!
//! Y se escribe **antes** que el driver, que es la regla de la casa: el formato
//! primero, el cerebro nunca.
//!
//! ```text
//!    Ring 0                          Ring 3
//!    ------                          ------
//!    KIND_RED                        ARP, IP, TCP, DNS, TLS
//!    tramas Ethernet crudas          todo lo que tiene versiones
//!    la MAC, el enlace, el DMA       y por tanto se equivoca
//! ```
//!
//! **El kernel no sabe lo que es una IP**, y eso no es minimalismo por gusto:
//! es que una pila TCP es la superficie de ataque mas grande que existe en un
//! sistema conectado, y aqui puede morirse sin llevarse la maquina. Windows y
//! Linux la tienen dentro del nucleo porque en 1990 no habia otra forma.
//!
//! De la capa de abajo hay poco que inventar: enumerar PCIe, mapear el BAR,
//! programar MSI y llevar anillos DMA es **exactamente** lo que ya hacen
//! `bmo-ahci` y `bmo-xhci`. Lo unico genuinamente nuevo es el RTL8168.
//!
//! ## Lo que este fichero NO debe volver a ser
//!
//! Un crate que compila y que no llama nadie. Mientras no haya un
//! `KIND_RED` cableado y un programa de Ring 3 que hable por el, esto es deuda,
//! no progreso -- y la forma de que no se olvide es que lo diga aqui.

#![no_std]
