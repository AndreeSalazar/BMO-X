//! **EL PERFIL DE LA PLACA: lo que BMO-X supone del firmware que tiene debajo.**
//!
//! [carril]  VERDE     cuatro cadenas y un contador. No lee MMIO, no lee ACPI
//!                     y no decide nada
//! [consumo] NADA      compara el perfil en el arranque
//!
//! [cuesta]  NADA -- se dice una vez al arrancar. Lo que cuesta de verdad son
//!           las manas que este fichero NOMBRA, y cada una lleva su precio
//!           escrito en `PERFIL/PLACA/PERFIL.txt`.
//!
//! [riesgo]  ESPEJO -- estos cuatro campos viven TAMBIEN en `PERFIL/PLACA/PERFIL.txt`.
//!           Pueden separarse sin que nadie avise, y por eso los compara un
//!           guardian en cada build. Sin el, este fichero seria una opinion
//!           sobre una placa que ya no esta puesta.
//!
//! # *** POR QUE EXISTE
//!
//! Es la **LEY 24** --*el hardware se PERFILA*-- aplicada a la placa. Ya estaba
//! hecha para el CPU: `cpu_vendor/profile.rs` empieza diciendo que cambiar de
//! CPU *"is a profile swap, never a kernel edit"*.
//!
//! ** Para la placa no lo estaba. Lo que se sabia de esta A320M vivia repartido
//! en cuatro comentarios de TRES CAPAS distintas --el build, el sobre del
//! traspaso, la etapa s1 y el kernel-- y ninguna sabia de las otras.
//!
//! # [!] Y EL HALLAZGO NO FUE EL QUE SE BUSCABA
//!
//! Ninguno de los cuatro rodeos **se rompe** al cambiar de placa: los cuatro son
//! defensivos, y en un firmware que hace las cosas bien sobran pero funcionan.
//!
//! > El peligro no era que se rompieran. Era que **se siguen pagando y nadie
//! > sabe que se pueden quitar.**
//!
//! Por eso este perfil no dice *"esto se rompe"*: dice *"esto se paga por ESTA
//! placa"*. Con otra, la pregunta util deja de ser *que arreglo* y pasa a ser
//! **que puedo dejar de pagar** -- que es la que hoy nadie podia hacerse.
//!
//! # Lo que este fichero NO hace
//!
//! ```text
//!    [ ] no cambia ni un rodeo. Los cuatro siguen donde estaban
//!    [ ] no detecta la placa: la DECLARA. Preguntarle al firmware su modelo
//!        es otra cosa y vive en `plat/placa.rs`, que lee ACPI
//!    [ ] y no decide nada en ejecucion
//! ```
//!
//! ** La tercera es la que lo deja en carril VERDE. Un perfil que ademas
//! encendiera o apagara rodeos seria una segunda politica de arranque viviendo
//! al lado de la primera.

/// Quien la hizo.
pub const FABRICANTE: &str = "MSI";
/// El modelo. **El guardian busca esta cadena dentro de los ficheros de cada
/// mana**: si un rodeo deja de nombrarla, o desaparece, el build para.
pub const MODELO: &str = "A320M";
/// El zocalo del CPU.
pub const SOCKET: &str = "AM4";
/// Quien escribio el firmware. La familia importa mas que la version: las
/// manas de abajo son de AMI, no de MSI.
pub const FIRMWARE: &str = "AMI";

/// Lo que se dice en CABINA. Lleva los cuatro campos porque CABINA guarda
/// `&'static str` y no sabe componer texto.
const NOMBRE: &str = "perfil de placa: MSI A320M (AM4, firmware AMI)";

/// Cuantas manas de este firmware rodea BMO-X. Cada una, con su precio, en
/// `PERFIL/PLACA/PERFIL.txt`.
///
/// ** Es el numero que hay que mirar al cambiar de placa: son cuatro cosas que
/// se estan pagando por esta y que a lo mejor en la siguiente sobran.
pub const MANAS: u32 = 4;

/// **Lo dice al arrancar.** Una vez, desde `phase`.
///
/// Va como aviso y no como dato a proposito: **un perfil declarado no es un
/// perfil comprobado**. Nadie le ha preguntado al firmware si de verdad es esta
/// placa, asi que el renglon tiene que verse como lo que es -- una suposicion
/// escrita, no una medida.
pub fn confesar() {
    crate::ring0::cabina::warn("placa", NOMBRE, MANAS as u64);
}
