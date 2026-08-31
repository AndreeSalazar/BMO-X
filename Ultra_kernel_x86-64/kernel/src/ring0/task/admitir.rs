//! **ADMITIR UN `.bex`**: de unos bytes a un proceso que puede correr.
//!
//! [carril]  ROJO      que bytes se convierten en codigo ejecutable, y con que permisos
//!
//! [cuesta]  MAQUINA -- decide que bytes se convierten en codigo ejecutable y
//!           con que permisos. Un fallo aqui no da un `#PF`: da un programa
//!           corriendo con mas de lo que pidio.
//!
//! [riesgo]  AJENO -- todo lo que lee viene de un fichero de fuera. Cada campo
//!           de la cabecera BEF es un numero que lo escribio otro.
//!
//! ** NO SE PARTE: es ROJO de arriba abajo. No hay nada verde que separar, y
//! tres ficheros donde solo hay un carril es la aguja mejor escondida.
//!
//! ## Por que soy un fichero (L6a, L6b)
//!
//! Contesto la pregunta mas larga del kernel: reservar marcos, aplicar
//! reubicaciones, cerrar hashes y mapear. `proc.rs` decide **a quien se admite**
//! y esto hace **el trabajo de admitirlo**.
//!
//! ## [!] Y AQUI HAY QUE DECIR ALGO QUE EL CENSO NO VE
//!
//! `admit_payload_desde` es **UNA FUNCION DE 607 LINEAS**. El censo modular
//! llamaba a `proc.rs` un CAJON --media 58 lineas por funcion-- y esa media
//! MIENTE: diecinueve funciones pequenas y un monstruo dan el mismo promedio
//! que veinte medianas.
//!
//! *** Asi que este reparto **mueve el monstruo, no lo desarma**. Mover texto se
//! demuestra con un hash (L6d) y las pruebas lo confirman; partir esa funcion
//! por dentro es otra cosa:
//!
//! ```text
//!    lo que se hizo hoy    sacarla del fichero de al lado -> MECANICO
//!    lo que falta          su estado local compartido tiene que volverse una
//!                          estructura -> CAMBIO DE DISENO, y un hash no lo
//!                          puede demostrar. Otro dia y otro metodo
//! ```
//!
//! Se deja escrito para que nadie lea "1.162 -> 498" y crea que el problema
//! esta resuelto. Esta MOVIDO.
//!
//! ## Y por que `Origen` es un enum y no dos funciones
//!
//! Lo dice el comentario de abajo: **el bucle que reserva, reubica, verifica y
//! mapea es UNO**, y lo unico que cambia es de donde viene el trozo. Dos
//! funciones serian dos copias de ese bucle, y la segunda se quedaria atras.

use super::proc::*;
use super::*;
// ** Los mismos `use` que tenia `proc.rs`: el reparto mueve texto, y el texto
// movido necesita los nombres que usaba donde estaba.
use crate::ring0::mm::{self, phys, vmm};
use crate::ring0::obj::channel;
use crate::ring0::task::{bex, confianza, landing, scheduler};
// La REGLA de si una reloc cabe en su seccion vive en el gate, que es el mismo
// juez que usa el toolchain. Aqui se ponen los DATOS, que es lo unico que este
// modulo tiene y aquel no.
use bmo_bex_gate as gate;
use crate::ring0::plat::trap;

/// **DE DONDE SALEN LOS BYTES DE UNA SECCION.** Un cargador, dos origenes.
///
/// === La pieza B, y por que es un enum y no dos funciones ===
///
/// El cargador traia el fichero a una mesa de 4 MiB y copiaba de ahi a las
/// paginas del proceso. La mesa era **una y de toda la maquina**, con un tope
/// decidido de antemano que hubo que subir dos veces, y con una copia por cada
/// byte de cada programa.
///
/// `PorRangos` la borra: el HBA escribe **en el marco del proceso**, que es el
/// destino final. Cero copias, cero mesa, y el tope de un programa pasa a ser la
/// RAM que hay.
///
/// `EnMemoria` se queda porque hay dos casos que de verdad ya estan en RAM: las
/// imagenes que el kernel **embebe** (`include_bytes!`, no hay disco del que
/// leerlas) y ESTRATOS, cuyo gate hashea el fichero entero de una pasada. Ver la
/// nota de `Fuente::rango`.
///
/// Que sean dos variantes del MISMO camino --y no dos cargadores-- es lo que
/// impide que se separen: el bucle que reserva marcos, aplica relocations,
/// cierra hashes y mapea es **uno**, y lo unico que cambia es de donde viene el
/// trozo.
pub enum Origen<'a> {
    /// La imagen entera, ya en RAM.
    EnMemoria(&'a [u8]),
    /// Se pide al disco por rangos. **Sin mesa.**
    PorRangos(&'a mut crate::ring0::task::launch::Fuente),
}

impl Origen<'_> {
    /// Trae `dst.len()` bytes de la imagen desde `offset`. Devuelve cuantos.
    fn traer(&mut self, offset: usize, dst: &mut [u8]) -> usize {
        match self {
            Origen::EnMemoria(bytes) => {
                let fin = offset.saturating_add(dst.len()).min(bytes.len());
                if offset >= fin {
                    return 0;
                }
                let n = fin - offset;
                dst[..n].copy_from_slice(&bytes[offset..fin]);
                n
            }
            Origen::PorRangos(f) => f.rango(offset, dst),
        }
    }

    /// **Trae un rango SUELTO: no forma parte del flujo de secciones.**
    ///
    /// Lo usan las dos tablas que el cargador necesita **antes** de aterrizar
    /// nada --los hashes y las relocations-- y que viven **al final del
    /// fichero**. Por `traer` las pediria el cursor secuencial, que se iria al
    /// final y ya no podria volver al codigo: solo avanza, a proposito.
    ///
    /// Ver `launch::Fuente::rango_suelto`, que es donde esta contado entero.
    fn traer_suelto(&mut self, offset: usize, dst: &mut [u8]) -> usize {
        match self {
            // En memoria no hay cursor que mover: es la misma lectura.
            Origen::EnMemoria(_) => self.traer(offset, dst),
            Origen::PorRangos(f) => f.rango_suelto(offset, dst),
        }
    }
}

/// Lo mas grande que puede medir una seccion `Signature`, **segun el formato**.
///
/// No es un numero inventado como lo era `MAX_BEX`: la cabecera son 8 bytes y hay
/// como mucho una entrada de 40 por seccion, con `MAX_BEX_SECTIONS` de tope. Un
/// limite que sale del contrato no hay que subirlo nunca.
const MAX_FIRMA: usize = 8 + bex::MAX_BEX_SECTIONS * 40;

/// Admite UN programa BEX como proceso Ring 3 con el `pid` indicado.
///
/// `prologo` son los primeros bytes de la imagen: la cabecera y la tabla de
/// secciones, que es todo lo que hace falta para hacer el plan. **El resto ya no
/// se trae de antemano** -- se pide por `origen` seccion a seccion, y cada una
/// cae directamente en los marcos de su proceso.
pub(crate) fn admit_payload_desde(
    prologo: &[u8],
    origen: &mut Origen,
    pid: u32,
    tam_fichero: usize,
) -> Option<u32> {
    let bytes = prologo;
    set_status("admitting (alloc/map)");
    let plan = match bex::inspect(bytes, tam_fichero) {
        Ok(p) => p,
        Err(e) => {
            // ** EL MOTIVO, CON SU NAME. Aqui habia un `Err(_)`.
            //
            // Trece motivos distintos entraban por esta puerta y salian con la
            // misma frase: *"payload failed BEX admission"*. Un cargador que
            // sabe por que rechaza y no lo dice obliga a adivinar entre "el
            // fichero llego a medias", "otra arquitectura" y "el entry cae
            // fuera" -- tres cosas que se arreglan en tres sitios que no se
            // parecen en nada. Costo una tanda de fotos el 2026-08-09.
            //
            // Se dicen ademas **los bytes que llegaron**, porque el fallo mas
            // probable de los trece es justo el que se cuenta con un numero.
            log("[proc] FATAL: payload failed BEX admission\n");
            crate::ring0::cabina::fault("proc", e.name(), bytes.len() as u64);
            // ** Y LA PRUEBA DE LA ACUSACION: los ocho primeros bytes.
            //
            // Decir *"cabecera invalida"* es una afirmacion, y su evidencia son
            // ocho bytes. Sin ellos hay que **reproducir el fallo para
            // entenderlo**, que el 2026-08-10 fueron dos tandas de fotos y una
            // tarde entre cuatro hipotesis que se distinguen a simple vista:
            //
            // | lo que salga | quiere decir |
            // |---|---|
            // | `00 00 00 00...` | el DMA escribio en otro sitio |
            // | bytes del medio del fichero | llego el sector equivocado |
            // | algo que no es ni una cosa ni otra | el LBA esta mal |
            // | `...31464542` (BEF1 corrido) | un desfase, no una corrupcion |
            //
            // No es una linea de depuracion que se quita: **un error que no
            // puede ensenar su evidencia es un error a medias**, y este camino
            // --el prologo de una imagen que viene del disco-- es justo donde
            // menos se puede permitir adivinar.
            let mut ocho = [0u8; 8];
            let n = bytes.len().min(8);
            ocho[..n].copy_from_slice(&bytes[..n]);
            crate::ring0::cabina::fault(
                "proc",
                "los 8 primeros bytes de lo que llego",
                u64::from_le_bytes(ocho),
            );
            set_status("BEX admission failed");
            return None;
        }
    };
    // ** LOS `?` DE AQUI ABAJO ERAN MUDOS, y ese era el ultimo hueco.
    //
    // El 2026-08-11 se le dio nombre a diez caminos de fallo que hablaban por el
    // klog. Quedaban estos seis, que no hablaban **en absoluto**: un `?` sobre
    // un `Option` devuelve `None` y se acabo. Desde fuera, un `.bex` rechazado
    // por falta de memoria y uno rechazado por un hash malo se ven igual.
    let Some(aspace) = vmm::new_address_space() else {
        crate::ring0::cabina::fault("proc", "sin memoria para el espacio de direcciones", 0);
        return None;
    };

    // Sections: assigned sequentially from USER_IMAGE_BASE, honoring each
    // section's alignment. entry_offset is relative to the Code section.
    // * PASE 1: DONDE VA CADA SECCION, sin reservar ni un marco.
    //
    // Hacia falta partir el bucle en dos, y el motivo es concreto: una
    // relocation dice *"en `.data`+8 escribe la direccion de `.rodata`+17"*, y
    // eso no se puede resolver hasta conocer las VA de **todas** las secciones.
    // Antes se calculaba la VA y se copiaba y mapeaba en la misma vuelta, asi
    // que al llegar a `.data` la de `.rodata` ya se habia perdido.
    //
    // La cuenta es la de siempre y no cambia --alinear, tantas paginas como pida
    // `mem_size`, y la siguiente empieza tras ellas--, solo se hace antes.
    let mut va_de = [0u64; bex::MAX_BEX_SECTIONS];
    {
        let mut va_cursor = vmm::USER_IMAGE_BASE;
        for i in 0..plan.section_count {
            let s = plan.sections[i];
            let align = s.alignment as u64;
            va_cursor = (va_cursor + align - 1) & !(align - 1);
            va_de[i] = va_cursor;
            let pages = (s.mem_size + mm::PAGE - 1) / mm::PAGE;
            va_cursor = va_cursor + pages * mm::PAGE;
        }
    }

    // La VA de una seccion por su CODIGO DE RELOCATION (`0` = code, `1` = data,
    // `2` = rodata), que **no** es el de `SECTION_*`. La traduccion se hace aqui,
    // en un solo sitio, porque es exactamente donde se cruzarian las dos
    // numeraciones.
    // ** Devuelve el INDICE y no la VA desde el 2026-08-25: con la VA sola no se
    // puede comprobar si la reloc CABE en su seccion, y esa comprobacion la
    // tenia el toolchain y el cargador no. Ver `gate::reloc_cabe`.
    let seccion_por_codigo_reloc = |cod: u8| -> Option<usize> {
        let buscado = match cod {
            0 => bex::SECTION_CODE,
            1 => bex::SECTION_DATA,
            2 => bex::SECTION_RODATA,
            _ => return None,
        };
        for i in 0..plan.section_count {
            if plan.sections[i].kind == buscado {
                return Some(i);
            }
        }
        None
    };

    // ** LOS DIGESTS DECLARADOS, localizados UNA vez.
    //
    // `inspect` ya no comprueba hashes: dice donde estan. La comprobacion la
    // hace el bucle de abajo, seccion por seccion y en el momento en que cada
    // una termina de caer en la memoria del proceso. Ver `task/aterrizaje.rs`.
    //
    // Si la imagen no trae firma --las que el kernel EMBEBE no pasan por el
    // escritor-- esto es `None` y cada cierre contesta `SinFirma`. Se cuenta,
    // pero no se rechaza: exigirle una prueba a quien nunca la prometio seria
    // dejar de arrancar.
    let mut buf_firma = [0u8; MAX_FIRMA];
    let firmas = if plan.firma_file_size > 0 && plan.firma_file_size as usize <= MAX_FIRMA {
        let n = plan.firma_file_size as usize;
        // ** SUELTA, y ese detalle es la diferencia entre arrancar y no.
        //
        // La seccion `Signature` esta **al final del fichero** --en `gui.bex`, en
        // el 0x4B680 de 0x4B728-- y esto corre ANTES de aterrizar la primera
        // seccion, que empieza en el 0x200. Por el cursor del flujo, este salto
        // lo dejaria en el final y el codigo quedaria detras: `leer_en`
        // contestaria `0` --correctamente, solo avanza-- y el cargador lo diria
        // como `una seccion se quedo a medias al aterrizar =0`, que manda a
        // mirar el disco cuando el disco esta bien.
        let leidos = origen.traer_suelto(plan.firma_file_offset as usize, &mut buf_firma[..n]);
        if leidos != n {
            crate::ring0::cabina::fault("proc", "la tabla de hashes se quedo sin leer", leidos as u64);
            return None;
        }
        landing::Firmas::abrir(&buf_firma[..n], plan.firma_indice)
    } else {
        None
    };

    // ===================================================================
    //  *** EL GATE DE AUTORIA (2026-08-25)
    // ===================================================================
    //
    // Hasta hoy el cargador sabia contestar UNA pregunta: *"llego lo que se
    // escribio"*. Los hashes por seccion la contestan, y `:firma` de ESTRATOS
    // tambien. Ninguna de las dos contesta la otra: **quien lo escribio.**
    //
    // ** Y CABLEAR Ed25519 A SECAS NO LA HABRIA CONTESTADO TAMPOCO, porque el
    // formato guarda `sig[64] || pubkey[32]`: la clave publica viaja DENTRO de
    // la firma, asi que la comprobacion siempre cuadra -- el firmante eligio las
    // dos cosas. Lo que la convierte en una respuesta es el ANCLA.
    //
    // Ver `task/confianza.rs`, que es donde vive la opinion, y `bmo-firma`, que
    // solo hace la aritmetica.
    //
    // [!] Va AQUI y no en `launch.rs` porque aqui es donde la seccion de firma
    // ya esta en memoria. En `launch` solo hay el prologo, y la seccion
    // `Signature` esta al final del fichero.
    if let Some(f) = firmas.as_ref() {
        let mut ancla = [[0u8; confianza::CLAVE]; 8];
        let cuantas = confianza::claves(&mut ancla);
        let tam_firma = plan.firma_file_size as usize;
        match f.cadena() {
            None => {
                // La tabla no da para leer sus propios digests. No se puede
                // decir ni que si ni que no, asi que se dice que no.
                crate::ring0::cabina::fault(
                    "firma",
                    "la tabla de hashes no deja calcular la cadena",
                    f.cuantos() as u64,
                );
                return None;
            }
            Some(cadena) => {
                let v = bmo_firma::examinar(&buf_firma[..tam_firma], &cadena, &ancla[..cuantas]);
                if !v.permite_ejecutar(confianza::exige_firma()) {
                    // ** El motivo lo pone el VEREDICTO, no este sitio. Cada uno
                    // manda a mirar algo distinto -- "no cuadra" al fichero,
                    // "autor desconocido" al ancla-- y aplanarlos a "no paso el
                    // gate" seria devolver la pregunta al que ya la hizo.
                    crate::ring0::cabina::fault("firma", v.motivo(), f.cuantos() as u64);
                    return None;
                }
                if let bmo_firma::Veredicto::Firmado { clave } = v {
                    // Y cuando SI, se dice QUIEN. Un `si` no distingue a nadie.
                    crate::ring0::cabina::info("firma", confianza::nombre(clave), clave as u64);
                }
            }
        }
    }

    let mut sin_firma = 0usize;

    // * PASE 2: reservar, copiar, CERRAR, PARCHEAR y mapear.
    let mut entry_va: u64 = 0;
    let mut code_bytes: u32 = 0;
    // Donde queda la seccion EJECUTABLE. Se sabia aqui y se tiraba; la
    // autopsia la necesita para distinguir un retorno de un puntero a datos.
    let mut code_va: u64 = 0;
    let mut code_len: u64 = 0;
    let total_relocs = bex::cuantas_relocs(plan.relocs_file_size);

    // ** Y LA TABLA DE RELOCATIONS TAMBIEN SE CIERRA, antes de aplicar ni una.
    //
    // No se mapea, asi que no cae en el bucle de abajo -- y es la seccion cuya
    // corrupcion hace mas dano en silencio: una reloc torcida escribe un puntero
    // inventado dentro de `.data` y el proceso arranca, corre, y muere mucho
    // despues en un sitio que no se parece a la causa. Comprobarla DESPUES de
    // aplicarla no serviria de nada.
    // ** LA TABLA DE RELOCATIONS, EN MARCOS PRESTADOS.
    //
    // No se mapea en el proceso --el programa nunca la ve-- pero hay que tenerla
    // entera en RAM mientras se aplica: cada pagina pregunta por TODAS las
    // relocs, y pedirlas al disco de 24 en 24 bytes serian mil lecturas por
    // pagina.
    //
    // Se piden marcos por su tamano REAL en vez de reservar un maximo. En DOOM
    // son 30.840 bytes --ocho marcos-- que se sueltan en cuanto se aplican. Es
    // el modelo quirofano: entra lo que hace falta para la operacion en curso.
    //
    // [!] Se sueltan por el camino BUENO. Los caminos de error de aqui abajo
    // devuelven `None` sin soltarlos -- igual que ya hacen con el espacio de
    // direcciones, que tampoco se destruye. Es una fuga que solo ocurre cuando un
    // programa NO arranca, y esta apuntada para limpiarla junto con la otra: son
    // el mismo arreglo.
    let mut relocs_marcos: Option<(u64, u64)> = None;
    let relocs: &[u8] = if total_relocs > 0 {
        let n = plan.relocs_file_size as usize;
        let paginas = ((n as u64) + mm::PAGE - 1) / mm::PAGE;
        // ** EL SOSPECHOSO DE DOOM, y por eso lleva el numero puesto.
        //
        // Son marcos **CONTIGUOS**, y eso puede fallar con RAM de sobra si el
        // asignador esta fragmentado. `ray.bex` pide UNO --sus relocs son 24
        // bytes-- y DOOM pide OCHO: 30.840 bytes de tabla. Es la diferencia mas
        // grande entre el que arranca y el que no.
        let Some(base) = phys::alloc_frames_contig(paginas) else {
            crate::ring0::cabina::fault(
                "proc",
                "no hay marcos CONTIGUOS para la tabla de relocs (paginas pedidas)",
                paginas,
            );
            return None;
        };
        relocs_marcos = Some((base, paginas));
        let dst = unsafe {
            core::slice::from_raw_parts_mut(mm::phys_to_virt(base) as *mut u8, n)
        };
        // Suelta por lo mismo que la de hashes: la tabla va detras de todo lo
        // que se ejecuta, y esto corre antes de aterrizar nada.
        let leidos = origen.traer_suelto(plan.relocs_file_offset as usize, dst);
        if leidos != n {
            crate::ring0::cabina::fault("proc", "la tabla de relocs se quedo sin leer", leidos as u64);
            return None;
        }
        let esperado = firmas.as_ref().and_then(|f| f.digest_de(plan.relocs_indice));
        let mut cierre = landing::Aterrizaje::abrir(bex::SECTION_RELOCS, esperado);
        cierre.trozo(dst);
        match cierre.cerrar() {
            Ok(landing::Cierre::Cuadra) => {}
            Ok(landing::Cierre::SinFirma) => sin_firma += 1,
            Err(_) => {
                set_status("relocs corruptas");
                crate::ring0::cabina::fault(
                    "proc",
                    "el HASH de la tabla de relocs NO cuadra",
                    plan.relocs_file_size,
                );
                return None;
            }
        }
        unsafe { core::slice::from_raw_parts(mm::phys_to_virt(base) as *const u8, n) }
    } else {
        &[]
    };
    let mut aplicadas = 0usize;
    // ** EL ORDEN DE ATERRIZAJE: POR OFFSET DE FICHERO, no por direccion virtual.
    //
    // === Hoy no cambia nada. Manana lo decide todo ===
    //
    // Mientras las secciones se copien de un bufer que ya esta entero en RAM, el
    // orden da igual: `copy_nonoverlapping` va a donde le digan. Por eso esto se
    // decide **ahora**, con la prueba de que no rompe nada, y no despues.
    //
    // Con la pieza B --el disco escribiendo en los marcos del proceso-- deja de
    // dar igual: un fichero se lee hacia adelante, y el cursor de FAT32 es el
    // CLUSTER. Aterrizar en orden de VA puede pedir retroceder, y retroceder es
    // volver a recorrer la cadena desde el principio: cuadratico, y justo en el
    // caso que B existe para arreglar.
    //
    // La colocacion en memoria NO se toca -- `va_de[]` se calculo en el pase 1 y
    // sigue mandando. Lo unico que se ordena es en que orden se rellenan.
    let mut orden = [0usize; bex::MAX_BEX_SECTIONS];
    for i in 0..plan.section_count {
        orden[i] = i;
    }
    // Insercion, que para dieciseis como mucho es lo correcto: sin recursion,
    // sin memoria extra, y estable -- dos secciones con el mismo offset (solo la
    // `Bss`, que no ocupa fichero) conservan el orden del plan.
    for a in 1..plan.section_count {
        let mut b = a;
        while b > 0
            && plan.sections[orden[b - 1]].file_offset > plan.sections[orden[b]].file_offset
        {
            orden.swap(b - 1, b);
            b -= 1;
        }
    }

    for paso in 0..plan.section_count {
        let i = orden[paso];
        let s = plan.sections[i];
        let va_start = va_de[i];
        let pages = (s.mem_size + mm::PAGE - 1) / mm::PAGE;
        let writable = s.flags & bex::SECTION_FLAG_EXEC == 0;
        // ** EL CIERRE DE ESTA SECCION, abierto antes de su primer byte.
        //
        // Se busca su digest por `s.indice` --el indice en la tabla del
        // FICHERO-- y no por `i`, que es el de este plan y solo cuenta lo
        // cargable. Ver la nota de `BexMapping::indice`.
        let mut cierre =
            landing::Aterrizaje::abrir(s.kind, firmas.as_ref().and_then(|f| f.digest_de(s.indice)));
        // **Lo que le falta a una relocation partida en la frontera de pagina.**
        // `(valor, cuantos bytes ya se escribieron)`. Ver la nota larga abajo.
        //
        // Vive por SECCION y no por proceso a proposito: una seccion empieza en
        // su propia VA, asi que una cola que sobreviviera al final de una
        // seccion se escribiria al principio de OTRA -- ocho bytes en el sitio
        // equivocado, y el programa arrancando con basura donde va un puntero.
        let mut cola: Option<(u64, usize)> = None;
        for p in 0..pages {
            let Some(frame) = phys::alloc_frame() else {
                crate::ring0::cabina::fault(
                    "proc",
                    "sin marcos libres para una pagina de seccion",
                    va_start + p * mm::PAGE,
                );
                return None;
            };
            phys::zero_frame(frame);
            let chunk = p * mm::PAGE;
            if chunk < s.file_size {
                let n = (s.file_size - chunk).min(mm::PAGE) as usize;
                unsafe {
                    // ** AQUI MURIO LA COPIA (2026-08-10, pieza B).
                    //
                    // Esto era un `copy_nonoverlapping` desde una mesa de 4 MiB
                    // que antes habia que llenar entera desde el disco. Ahora
                    // `dst` **es el marco**, y `traer` le pide al origen justo
                    // este rango: con `PorRangos`, el HBA escribe ahi y el byte
                    // no pasa por ningun sitio intermedio.
                    //
                    // > La RAM no es una bodega. Y ahora tampoco es una cinta
                    // > transportadora.
                    let dst = core::slice::from_raw_parts_mut(
                        mm::phys_to_virt(frame) as *mut u8,
                        n,
                    );
                    let leidos = origen.traer((s.file_offset + chunk) as usize, dst);
                    if leidos != n {
                        // Una seccion que llega a medias NO se rellena con lo que
                        // hubiera: se dice cuanto falto. El marco ya estaba a
                        // cero, asi que lo que no llego son ceros y no basura de
                        // otro -- pero eso no lo convierte en valido.
                        crate::ring0::cabina::fault(
                            "proc",
                            "una seccion se quedo a medias al aterrizar",
                            leidos as u64,
                        );
                        return None;
                    }
                    // ** SE LE DA AL HASHER LO QUE HAY EN EL MARCO, no lo que
                    // habia en el origen.
                    //
                    // Es toda la diferencia entre esta comprobacion y la que
                    // habia antes. Leerlo de `bytes` certificaria que el bufer
                    // era bueno; leerlo de aqui certifica **lo que este proceso
                    // va a ejecutar**, que es lo unico que importa -- y de paso
                    // cubre la copia, que es un sitio donde las cosas se rompen.
                    //
                    // Y son `n` bytes, no la pagina: el relleno de ceros del
                    // final no es parte de la seccion, y meterlo cambiaria el
                    // hash de toda imagen que no acabe en frontera de pagina.
                    cierre.trozo(core::slice::from_raw_parts(
                        mm::phys_to_virt(frame) as *const u8,
                        n,
                    ));
                }
            }
            // * LAS RELOCATIONS QUE CAEN EN ESTA PAGINA.
            //
            // Se parchea AQUI, con el marco todavia alcanzable por
            // `phys_to_virt` y ANTES de mapearlo: asi no hay que caminar las
            // tablas del proceso para escribir en su memoria, ni dejar una
            // pagina escribible que luego habria que volver a proteger.
            let pagina_va = va_start + p * mm::PAGE;
            // ** LA COLA DE LA PAGINA ANTERIOR, ANTES QUE NADA.
            //
            // Si una relocation se partio en la frontera, lo que le falta se
            // escribe aqui: al principio de este marco, que es literalmente el
            // byte siguiente al ultimo de la pagina de antes. Va lo PRIMERO
            // porque los bytes de la seccion ya estan puestos y una relocation
            // manda sobre ellos.
            if let Some((valor, ya)) = cola.take() {
                let quedan = 8 - ya;
                unsafe {
                    let dst = mm::phys_to_virt(frame) as *mut u8;
                    core::ptr::copy_nonoverlapping(
                        valor.to_le_bytes().as_ptr().add(ya),
                        dst,
                        quedan,
                    );
                }
            }
            for r in 0..total_relocs {
                // De `relocs`, que es la tabla que se acaba de traer a sus
                // marcos -- y con offset **0**, porque ese slice empieza donde
                // empieza la tabla. Leerla de `bytes` era leerla de la imagen
                // entera, y desde la pieza B `bytes` es solo el prologo: la tabla
                // de DOOM (30.840 B) cae mucho mas alla y no habria ni una reloc
                // que aplicar.
                let Some(rel) = bex::leer_reloc(relocs, 0, plan.relocs_file_size, r) else {
                    log("[proc] FATAL: tabla de relocations mal formada\n");
                crate::ring0::cabina::fault("proc", "la tabla de relocations esta mal formada", r as u64);
                    return None;
                };
                if rel.kind != bex::RELOC_SECCION_ABS64 {
                    // Aplicar una reloc que no se entiende seria escribir un
                    // numero inventado en la memoria de un proceso. Se rechaza
                    // el programa entero.
                    log("[proc] FATAL: tipo de relocation desconocido\n");
                crate::ring0::cabina::fault("proc", "tipo de relocation DESCONOCIDO", rel.kind as u64);
                    return None;
                }
                let (Some(i_donde), Some(i_destino)) = (
                    seccion_por_codigo_reloc(rel.donde_sec),
                    seccion_por_codigo_reloc(rel.destino_sec),
                ) else {
                    log("[proc] FATAL: relocation a una seccion que no existe\n");
                crate::ring0::cabina::fault("proc", "relocation a una seccion que NO EXISTE", ((rel.donde_sec as u64) << 8) | rel.destino_sec as u64);
                    return None;
                };
                // *** CABE ESTA RELOC EN LA SECCION QUE DICE PARCHEAR? (25-08)
                //
                // El toolchain lo comprobaba --`validator::validate_reloc_section`--
                // y el cargador NO, asi que un `.bex` que no salio de este
                // toolchain entraba con sus relocations sin que nadie las mirara.
                //
                // ** Y NO SE SALE DE LA IMAGEN: las secciones van seguidas desde
                // `USER_IMAGE_BASE`, o sea que un offset pasado de rosca CAE EN
                // LA SIGUIENTE. La unica comprobacion que habia --"cae en la
                // pagina que estoy parcheando"-- se cumplia, y se escribia.
                //
                // Y el hash tampoco lo caza: se cierra ANTES de parchear.
                let sec_donde = plan.sections[i_donde];
                if !gate::reloc_cabe(rel.donde_off, 8, sec_donde.file_size, sec_donde.mem_size) {
                    log("[proc] FATAL: relocation fuera de su seccion
");
                    crate::ring0::cabina::fault(
                        "proc",
                        "una relocation se sale de la seccion que dice parchear",
                        rel.donde_off,
                    );
                    return None;
                }
                let (base_donde, base_destino) = (va_de[i_donde], va_de[i_destino]);
                let donde_va = base_donde.wrapping_add(rel.donde_off);
                // No cae en esta pagina: no es cosa de esta vuelta.
                if donde_va < pagina_va || donde_va >= pagina_va + mm::PAGE {
                    continue;
                }
                let valor = (base_destino as i64).wrapping_add(rel.destino_off) as u64;
                let dentro = (donde_va - pagina_va) as usize;
                let caben = (mm::PAGE as usize) - dentro;
                unsafe {
                    let dst = (mm::phys_to_virt(frame) as *mut u8).add(dentro);
                    core::ptr::copy_nonoverlapping(
                        valor.to_le_bytes().as_ptr(),
                        dst,
                        caben.min(8),
                    );
                }
                // ** UN PUNTERO PARTIDO ENTRE DOS PAGINAS: SE WRITES EN DOS
                // TROZOS, y esto es un arreglo que trajo el metal.
                //
                // === Lo que decia este sitio, y por que era falso ===
                //
                // Aqui habia un rechazo, con este motivo escrito:
                //
                // > *"No puede pasar --los punteros van alineados a 8 y la
                // > pagina es multiplo de 8-- pero se comprueba en vez de
                // > confiarlo."*
                //
                // La comprobacion estaba bien puesta. **La suposicion no.** El
                // 2026-08-11, DOOM en el Ryzen:
                //
                // ```text
                //    FALLO proc: relocation PARTIDA entre dos paginas =1074388988
                // ```
                //
                // `1074388988` es `0x4009DFFC`: offset **4092** dentro de su
                // pagina. Ocho bytes desde ahi se salen por cuatro. Y no esta
                // alineado a 8 porque **nadie lo garantizo nunca**: el codegen
                // coloca los punteros donde caen en su seccion, y con 1.285
                // relocations repartidas por 800 KB, que ninguna caiga en los
                // ultimos siete bytes de una pagina es una loteria que DOOM
                // perdio.
                //
                // > **Un "no puede pasar" con una razon al lado es una hipotesis.
                // > Esta aguanto hasta el primer programa grande.**
                //
                // === Por que basta con UNA ranura ===
                //
                // Dos relocations no pueden partir la misma frontera: se
                // solaparian, y eso ya seria un fichero corrupto. Asi que como
                // mucho hay una pendiente por pagina, y llevarla es un `Option`.
                //
                // Las paginas se recorren **en orden** (`for p in 0..pages`), asi
                // que la siguiente vuelta escribe la cola en el marco nuevo antes
                // de tocar nada mas.
                if caben < 8 {
                    cola = Some((valor, caben));
                }
                aplicadas += 1;
            }
            // ** `_propia`: este marco salio de `alloc_frame` hace veinte lineas
            // y no lo conoce nadie mas, asi que su vida acaba con este espacio
            // de direcciones. Sin el bit, la imagen entera --unas 210 paginas en
            // DOOM-- se quedaba puesta para siempre al morir el programa.
            if vmm::map_page_propia(aspace, pagina_va, frame, true, writable).is_err() {
                log("[proc] FATAL: section map failed\n");
                crate::ring0::cabina::fault(
                    "proc",
                    "no se pudo mapear una pagina de seccion",
                    pagina_va,
                );
                return None;
            }
        }
        // ** Y SI LA COLA SOBREVIVE A LA SECCION, eso SI es un fichero malo.
        //
        // Significa que una relocation empieza dentro de la ultima pagina y
        // acaba fuera de la seccion. No hay marco siguiente donde escribirla, y
        // sobre todo: apunta a memoria que no es suya. Se rechaza con su nombre
        // en vez de escribir los bytes que quepan y dejar medio puntero puesto.
        if cola.is_some() {
            crate::ring0::cabina::fault(
                "proc",
                "una relocation se sale por el FINAL de su seccion",
                va_start + pages * mm::PAGE,
            );
            return None;
        }
        // ** Y SE CIERRA, con la seccion entera ya en la memoria del proceso.
        //
        // Aqui --y no al final del bucle de todas-- porque el numero que hace
        // falta es CUAL fallo. Cerrarlas todas juntas al final daria "algo no
        // cuadra" cuando ya hay tres secciones mapeadas y ninguna pista de por
        // donde empezar a mirar.
        match cierre.cerrar() {
            Ok(landing::Cierre::Cuadra) => {}
            Ok(landing::Cierre::SinFirma) => sin_firma += 1,
            Err(_) => {
                // ** ESTO ERA MUDO PARA CABINA, y era el sospechoso principal.
                //
                // `set_status` escribe en el panel del kernel, que mientras el
                // compositor tiene la pantalla **no se pinta**. O sea que el
                // motivo mas probable de que un `.bex` firmado no arranque se
                // decia justo donde nadie podia leerlo, y desde fuera se veia
                // como `el .bex no paso la admision` a secas.
                set_status("una seccion no cuadra con su hash");
                crate::ring0::cabina::fault(
                    "proc",
                    "el HASH de una seccion NO cuadra: la imagen que llego no es la firmada",
                    s.kind as u64,
                );
                return None;
            }
        }
        if s.kind == bex::SECTION_CODE {
            entry_va = va_start + plan.entry_offset;
            code_va = va_start;
            code_len = s.mem_size;
        }
        code_bytes = code_bytes.saturating_add(s.mem_size as u32);
    }
    // ** CUANTO DE LA IMAGEN QUEDO SIN CUBRIR, dicho en voz alta.
    //
    // No es un fallo --una imagen embebida no promete hashes-- pero callarlo
    // convertiria "verificado" en una palabra que a veces no significa nada. Si
    // esto sube en un `.bex` que SI paso por el escritor, es que el escritor
    // dejo de firmar algo y nadie se habria enterado.
    if sin_firma > 0 {
        crate::ring0::cabina::info("proc", "secciones sin hash con el que comparar", sin_firma as u64);
    }
    // ** Y LOS MARCOS DE LAS RELOCATIONS SE SUELTAN. Ya se aplicaron: la tabla
    // no es memoria del programa y no tiene por que sobrevivirle ni un tick.
    //
    // Es el modelo quirofano en pequeno -- entro lo que hacia falta para la
    // operacion, y sale cuando termina. En DOOM son ocho marcos; en un `.bex`
    // sin punteros que rellenar, ninguno.
    if let Some((base, paginas)) = relocs_marcos {
        for p in 0..paginas {
            phys::free_frame(base + p * mm::PAGE);
        }
    }
    // * Y SE COMPRUEBA QUE SE APLICARON TODAS.
    //
    // Una reloc que no cae en ninguna pagina es una que apunta fuera de su
    // seccion, y su sintoma seria un puntero a cero -- o sea el bug que todo esto
    // existe para matar, otra vez y en silencio. Mejor no arrancar.
    if aplicadas != total_relocs {
        log("[proc] FATAL: quedaron relocations sin aplicar\n");
        crate::ring0::cabina::warn("proc", "relocs sin aplicar", (total_relocs - aplicadas) as u64);
        return None;
    }
    if entry_va == 0 {
        log("[proc] FATAL: no entry point\n");
                crate::ring0::cabina::fault("proc", "el .bex no declara punto de entrada", 0);
        return None;
    }

    // User stack (64 KiB) just below USER_STACK_TOP.
    for p in 0..USER_STACK_PAGES {
        let Some(frame) = phys::alloc_frame() else {
            crate::ring0::cabina::fault("proc", "sin marcos libres para la pila de usuario", p);
            return None;
        };
        phys::zero_frame(frame);
        let va = vmm::USER_STACK_TOP - (p + 1) * mm::PAGE;
        // `_propia` igual que la imagen: 16 paginas que no conoce nadie mas.
        if vmm::map_page_propia(aspace, va, frame, true, true).is_err() {
            log("[proc] FATAL: stack map failed\n");
                crate::ring0::cabina::fault("proc", "no se pudo mapear la pila del proceso", va);
            return None;
        }
    }

    // The 16 BMO Channel estuaries, shared U/S with Ring 0.
    for i in 0..boot_context::MAX_CHANNEL_PAGES {
        let va = vmm::CHANNEL_VA_BASE + (i as u64) * mm::PAGE;
        if vmm::map_page(aspace, va, channel::page_phys(i), true, true).is_err() {
            log("[proc] FATAL: channel map failed\n");
                crate::ring0::cabina::fault("proc", "no se pudo mapear la pagina de canal", va);
            return None;
        }
    }

    // Kernel landing stack + fabricated Ring 3 context. The frames MUST be
    // physically contiguous: the stack is addressed linearly through the
    // physmap, and `fabricate` writes the context in the TOP page -- with two
    // independent alloc_frame calls that page is `base+4K` physical, a frame
    // nobody granted us unless the allocator happened to return neighbors
    // (true on QEMU's clean map, false on real memory maps with holes; the
    // context then lives in a foreign/free frame and dies with the next
    // zero_frame -- restored as zeros: backptr=0, pops to rsp=0x78, iretq
    // with cs=0 => the observed #GP(0)).
    let Some(kstack_base) = phys::alloc_frames_contig(KERNEL_STACK_PAGES) else {
        crate::ring0::cabina::fault(
            "proc",
            "no hay marcos CONTIGUOS para la pila de kernel de la tarea",
            KERNEL_STACK_PAGES,
        );
        return None;
    };
    let kstack_top = mm::phys_to_virt(kstack_base) + KERNEL_STACK_PAGES * mm::PAGE;
    let context = unsafe { trap::fabricate(kstack_top, entry_va, 0, true, vmm::USER_STACK_TOP) };

    let tid = scheduler::spawn_user(pid, context, kstack_base, KERNEL_STACK_PAGES, kstack_top, aspace, 0)?;
    // F3: seed the init process's capability table -- one estuary handle
    // per BMO Channel page, discoverable via TASK_OP_CHANNEL_OPEN.
    crate::ring0::obj::cap::seed_init(pid);
    log("[proc] init.bex admitted: Ring 3 entry at 0x");
    crate::ring0::dev::console::serial_write_u64(entry_va, 16);
    log("\n");
    set_status("admitted");
    // Anotar lo que el BEX declaraba: el registro de programas se llena aqui,
    // con los datos del plan de carga que acabamos de honrar.
    unsafe {
        if let Some(r) = record_mut(pid) {
            r.tid = tid;
            r.sections = plan.section_count as u8;
            r.entry_va = entry_va;
            r.code_bytes = code_bytes;
            r.code_va = code_va;
            r.code_len = code_len;
            r.admitted = true;
        }
    }
    Some(tid)
}
