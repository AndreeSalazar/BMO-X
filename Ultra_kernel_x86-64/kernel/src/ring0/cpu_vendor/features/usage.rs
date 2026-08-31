//! `features::usage` -- what BMO TAKES. The contract, written by hand.
//!
//! [carril]  AMARILLO  el contrato a mano: si dice que BMO enciende algo que no, miente
//!
//! The other half of rule 5. `silicon.rs` asks the machine; this declares what
//! we do with the answer, and the two are joined in `mod.rs`.
//!
//! # ** THE RULE THAT KEEPS THIS FILE HONEST
//!
//! > **A `Yes` without a place named is a `Yes` that lies.**
//!
//! Nothing in the build can verify this column -- it is prose about the tree.
//! So the discipline is that every `Yes` carries the file or the mechanism that
//! uses it, and that is what makes the claim checkable by a person in ten
//! seconds instead of believable forever.
//!
//! And every `No` carries **what it would buy**, because a census whose second
//! column is thirty `no` teaches nothing. The list of `No`s is the actual
//! product of this module: it is the roadmap of this CPU, ordered by what each
//! row would pay for.

use super::Feat;

/// Does BMO take this feature, and where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    /// Taken. The string says WHERE, and it is not decoration -- see the rule
    /// in this module's header.
    Yes(&'static str),
    /// Not taken. The string says what it would buy, or why it never will.
    No(&'static str),
}

impl Use {
    pub const fn is_yes(&self) -> bool {
        matches!(self, Use::Yes(_))
    }
    pub const fn nota(&self) -> &'static str {
        match self {
            Use::Yes(s) => s,
            Use::No(s) => s,
        }
    }
}

/// What BMO does with each feature.
///
/// Exhaustive on purpose: a new [`Feat`] does not compile until somebody
/// decides -- and writes down -- what we do about it.
pub fn of(f: Feat) -> Use {
    match f {
        // ================= vectores y matematica =================
        Feat::Sse2 => Use::Yes("BMO C: la ruta de flotantes emite movsd/addsd/comisd"),
        Feat::Sse41 => Use::No("nada lo pide todavia"),
        Feat::Sse42 => Use::No("su CRC32 serviria al gate, pero el hash es BLAKE3"),

        // *** ESTAS TRES FILAS DECIAN QUE NO SE PODIA, Y DEJO DE SER CIERTO EL
        // 2026-08-23. Corregidas el 26-08.
        //
        // Decian: *"sem-asm compone REX/ModRM y AVX usa prefijo VEX (C5/C4),
        // asi que hoy no hay forma de emitir una sola instruccion AVX ni como
        // intrinseco."* Y era verdad cuando se escribio. Tres dias despues
        // entraron **cinco filas VEX** en `intrinsics.toml` --`avx_suma4`,
        // `avx_resta4`, `avx_por4`, `avx_funde4` y `avx_sin_altos`-- y nadie
        // volvio aqui.
        //
        // ** LA CABECERA DE ESTE MODULO YA TENIA LA REGLA, Y SOLO LA MITAD:
        //
        // > *"A `Yes` without a place named is a `Yes` that lies."*
        //
        // Le faltaba el espejo, que es peor: **un `No` cuyo motivo dejo de ser
        // cierto no dice "todavia no": dice "no se puede".** Quien lea esta
        // tabla buscando si puede vectorizar algo concluye que no hay forma, y
        // deja de mirar -- con la instruccion ya escrita y probada a dos
        // carpetas de distancia.
        //
        // [!] Y el estado real de las tres no es "hecho": es **construido y sin
        // un solo cliente**. Cero llamadas en todo el arbol. Eso es exactamente
        // lo que estas filas tienen que decir ahora, porque es lo que separa
        // "no se puede" de "nadie lo ha pedido todavia".
        Feat::Avx => Use::No("SI se puede desde el 23-08 (5 filas VEX en intrinsics.toml): CERO clientes"),
        Feat::Avx2 => Use::No("igual que AVX: emisor listo, sin clientes. BLAKE3 sacaria ~3x"),
        // *** LA UNICA DE LAS TRES QUE TIENE UN DESTINO ESCRITO.
        //
        // `avx_funde4` es `vfmadd231pd`: cuatro flotante64, multiplicar y
        // acumular, en una instruccion. La propia tabla lo dice -- *"es la
        // operacion de la que esta hecho un producto de matrices"*-- y el
        // motor de inferencia de `PLAN_EL_ASISTENTE.md` esta hecho de eso.
        //
        // [!] Y aun asi el asistente NO esta limitado por aqui: su parte 8
        // demuestra que un token lo decide el ANCHO DE MEMORIA, no el calculo.
        // 30 tokens/s por el lado del CPU contra ~10 por el de la memoria: **el
        // CPU sobra tres veces.** Esta fila es potencia disponible, no el cuello.
        Feat::Fma => Use::No("`avx_funde4` = vfmadd231pd, escrita y sin usar. Es el matmul del asistente"),
        Feat::F16c => Use::No("no hay medios flotantes en ningun formato de BMO"),

        // ================= bits: contar y escanear =================
        // ** El grupo con mejor relacion trabajo/beneficio de toda la tabla:
        // son una FILA de intrinsics.toml cada una, sin VEX, y sus clientes ya
        // existen -- `mm/phys.rs` busca marcos libres recorriendo un bitmap y
        // `find_free_cluster` de FAT32 hace lo mismo con la FAT.
        Feat::Popcnt => Use::No("contar bits de un bitmap: mm/phys.rs y FAT32"),
        Feat::Lzcnt => Use::No("escanear un bitmap en 1 instruccion en vez de un bucle"),
        Feat::Bmi1 => Use::No("TZCNT/BLSR: el siguiente marco libre, de golpe"),
        Feat::Bmi2 => Use::No("nada lo pide todavia"),
        Feat::Movbe => Use::No("los formatos de BMO son little-endian a proposito"),
        Feat::Adx => Use::No("aritmetica de precision multiple; no hay"),

        // ================= azar =================
        // *** COBRADO EL 2026-08-24, y era lo mas barato del tablero: RDRAND no
        // es privilegiado, asi que Ring 3 lo ejecuta sin pasar por ninguna
        // puerta. Cero kernel. Llevaba meses sin cliente porque no habia
        // criptografia que lo pidiera -- y el mismo dia que la hubo, se pago.
        //
        // ** `bmo_cripto::azar` NO TIENE RESPALDO a proposito. Si esto falla,
        // devuelve el motivo y quien pedia una clave PARA. Un respaldo que
        // nadie ve convierte "no hay azar" en "hay azar malo", y lo segundo no
        // se nota hasta que alguien entra.
        Feat::Rdrand => Use::Yes("bmo-cripto: claves de X25519, nonces de GCM, y la firma del .bex"),
        Feat::Rdseed => Use::No("semilla de verdad; RDRAND llega antes y basta"),

        // ================= criptografia =================
        //
        // *** LAS TRES CAMBIARON DE SENTIDO EL 2026-08-24 Y NINGUNA SE ENCIENDE
        // TODAVIA -- pero lo que decian ya no era cierto, y una tabla que dice
        // "no tiene cliente" sobre algo que SI lo tiene es peor que una casilla
        // vacia: cierra la pregunta.
        //
        // ** Y las tres apuntan al mismo sitio: `aes.rs` y `gcm.rs` estan
        // escritos en software y **miran tablas indexadas por el dato** --la
        // S-box de AES, y el producto de GHASH si se hiciera con tabla--, o
        // sea que quien comparta el CPU puede medir que lineas de cache se
        // tocaron. AES-NI y PCLMUL no tienen esa exposicion porque no miran
        // ninguna tabla: la ronda entera es una instruccion.
        //
        // [!] O sea que esto no es "ir mas rapido". Es la unica forma de que el
        // cifrado de este sistema deje de tener ese canal abierto, y por eso el
        // motivo de cada fila dice el CLIENTE y no la ganancia.
        Feat::Aes => Use::No("bmo-cripto/aes.rs, y quitaria el canal de cache de la S-box"),
        Feat::Pclmul => Use::No("el producto de GHASH en GF(2^128): bmo-cripto/gcm.rs"),
        Feat::Sha => Use::No("bmo-cripto/sha256.rs, que ya pasa los vectores de NIST"),

        // ================= estado extendido =================
        Feat::Xsave => Use::Yes("entry.rs: el xrstor64 de TODA puerta, y el timer"),

        // ** ESTA FILA ES LA QUE HAY QUE MIRAR EN LA FOTO.
        //
        // `xsave64` es #UD sin CR4.OSXSAVE, y el stub lo ejecuta en cada
        // syscall -- luego el bit ESTA puesto, porque la maquina arranca. Pero
        // el unico `mov cr4` del kernel esta en el trampolin de los AP y pone
        // `0x620` (PAE, OSFXSR, OSXMMEXCPT): **el bit 18 no lo pone BMO.**
        //
        // O sea que el sistema depende, en su camino mas caliente, de un bit
        // que le dejo puesto el firmware. Con otro firmware seria un #UD en la
        // primera puerta. Es exactamente lo que la regla 5 dice que no se hace:
        // dar por hecho un HECHO del hardware en vez de preguntarlo.
        Feat::Osxsave => Use::Yes("lo exige xsave64... pero lo pone el FIRMWARE, no BMO"),

        // ** PASO DE No A Yes EL 2026-08-16, y esta fila es ahora un SEGURO.
        //
        // El stub ejecuta `xsaveopt64` incondicionalmente, asi que en un CPU
        // sin esta extension seria `#UD` en la primera puerta. Declararla usada
        // hace que el censo la cuente como CONFLICTO en esa maquina y que el
        // arranque lo grite -- que es todo lo que se puede hacer sin meter una
        // rama en el camino mas caliente del sistema, y bastante mejor que un
        // `#UD` sin nombre.
        Feat::Xsaveopt => Use::Yes("ring0/syscall/entry.rs: el guardado de TODA puerta"),
        Feat::Xsavec => Use::No("formato compacto: se salta los componentes en init"),
        Feat::Xsaves => Use::No("variante supervisora; no hay estado de kernel que guardar"),

        // ================= memoria y cache =================
        // ERMS se usa SIN SABERLO: memcpy y memset se emiten como rep movsb /
        // rep stosb, y en Zen 3 eso son los caminos anchos del silicio. Por eso
        // esta fila dice `Yes` aunque nadie escribiera nunca la palabra ERMS.
        Feat::Erms => Use::Yes("implicito: memcpy/memset son rep movsb / rep stosb"),
        Feat::Clflushopt => Use::No("nada tira lineas de cache a mano"),
        Feat::Clwb => Use::No("es para memoria persistente; no hay"),
        // ** El blit y el borrado de paginas son EL MISMO problema: escribir
        // mucho que nadie va a releer. `alloc_frames_contig` pone a cero 3.072
        // paginas en cada lanzamiento de DOOM y ensucia la cache entera con
        // datos que el proceso ni ha mirado.
        Feat::Clzero => Use::No("poner paginas a cero sin ensuciar la cache"),
        Feat::Pdpe1gb => Use::No("el physmap va en paginas de 2 MiB; con 1 GiB seria 512x menos tablas"),

        // ================= tiempo y espera =================
        Feat::Rdtscp => Use::Yes("fila de intrinsics.toml, alcanzable desde BMO C"),
        Feat::InvariantTsc => Use::Yes("dev/clock.rs extrapola la hora del CMOS con el TSC"),
        // ** El bloqueante que YA estaba nombrado: AXION apaga nucleos y no
        // sabe encenderlos, y lo que le falta es esto.
        Feat::Monitor => Use::No("AXION: apagar funciona, ENCENDER pide MWAIT"),
        Feat::Monitorx => Use::No("la variante AMD, y ademas funciona en Ring 3"),

        // ================= proteccion que el CPU regala =================
        // ** Las tres son GRATIS -- bits de CR4 y de EFER-- y ninguna esta
        // puesta, en un sistema cuyo lema declarado es cero confianza en el
        // codigo. Es la seccion mas incomoda de esta tabla y por eso va entera.
        //
        // [!] Y esta tabla es el sitio donde MENOS se puede decir "microkernel"
        // --como decia este comentario--: lo que la ley de la casa exige es que
        // una regla traiga el COMPONENTE que la pide y su NUMERO. Aqui el
        // componente es el CPU y el numero son cuatro bits que regala y nadie
        // enciende. Eso es lo que significa Meta-Kernel; el linaje de
        // microkernel es de donde venimos, no lo que juzga.
        // *** ESTAS CUATRO FILAS DECIAN QUE NO Y TRES ERAN FALSAS (2026-08-24).
        //
        // Se escribieron mirando el KERNEL, y los bits los pone `s1_cpu`, que
        // es otra etapa y otro fichero. La tabla que existe para ser la verdad
        // sobre que hace BMO-X con cada bit del CPU llevaba meses diciendo que
        // no hacia nada con tres que SI estan puestos.
        //
        // ** Y el coste no fue teorico: con esta tabla delante se le dijo al
        // dueno que "BMO-X no tiene ni una mitigacion". Falso. Tiene dos
        // funcionando y una tercera armada sin usar.
        //
        // > Una tabla equivocada no deja un hueco: **cierra la pregunta.**
        //   Nadie vuelve a mirar lo que ya esta contestado.

        // `s1_cpu/cpu/zen3.rs`: `EFER |= EFER_NXE`. El mecanismo ESTA armado.
        //
        // [!] Y aun asi toda pagina es ejecutable, por otro motivo: **ninguna
        // entrada de tabla pone el bit 63**. `vmm.rs` no lo escribe en ningun
        // sitio, asi que el permiso existe y nadie lo usa.
        //
        // *** Y SE COBRO EL MISMO DIA: `vmm.rs` pone el bit 63 en toda pagina
        // escribible. La regla es literalmente W^X --escribible O ejecutable,
        // nunca las dos-- y no hizo falta ni un parametro nuevo: el cargador ya
        // calculaba `writable = flags & SECTION_FLAG_EXEC == 0`.
        //
        // ** Lo que compra es la mitad de una explotacion: quien consiga
        // escribir ya no puede escribir instrucciones y saltar a ellas; tiene
        // que armar la cadena con codigo que YA existe, que es otro orden de
        // magnitud. Ver `PTE_NX`, y la trampa de `rodata` que hay anotada ahi.
        Feat::Nx => Use::Yes("EFER.NXE en s1_cpu + PTE_NX en vmm.rs: W^X, escribible XOR ejecutable"),
        // `s1_cpu/cpu/mod.rs`: `if smep { cr4 |= 1 << 20 }`. Funciona solo: el
        // kernel nunca ejecuta una pagina de usuario, asi que no hay nada que
        // adaptar. Un desvio del flujo de Ring 0 hacia codigo de Ring 3 --el
        // final de casi toda cadena de explotacion-- da fault en vez de correr.
        Feat::Smep => Use::Yes("s1_cpu enciende CR4.SMEP: Ring 0 no puede EJECUTAR una pagina de Ring 3"),
        // *** EL ULTIMO DE LOS CUATRO, y no era un bit: el kernel SI tocaba
        // memoria de Ring 3 en dos sitios y habia que quitarlos primero.
        //
        //   ARCH_OP_LEER_EN     tenia un respaldo `None => base + desde` que
        //                       ya era INALCANZABLE desde el arreglo de los
        //                       limites de la misma manana
        //   ARCH_OP_ESCRIBIR_DE no tenia camino de espejo EN ABSOLUTO:
        //                       siempre dereferenciaba la VA del proceso
        //
        // Los dos pasan ahora por el physmap, que ademas es el camino rapido.
        //
        // ** Y queda UN sitio que lee Ring 3 a proposito: la autopsia, cuyo
        // trabajo es contar que habia en la pila del proceso roto. Lleva
        // `stac`/`clac` con nombre propio en `autopsy::con_permiso`.
        //
        // *** Que sea UN SOLO sitio con permiso explicito es lo que hace que la
        // prohibicion valga: cualquier otro acceso a Ring 3 desde Ring 0 da
        // fault, y el fault dice donde.
        Feat::Smap => Use::Yes("s1_cpu enciende CR4.SMAP; solo la autopsia levanta el permiso, con stac/clac"),
        // `s1_cpu/cpu/mod.rs`: `if umip { cr4 |= 1 << 11 }`. Ring 3 ya no puede
        // leer las bases de GDT/IDT/LDT/TR con SGDT y familia.
        Feat::Umip => Use::Yes("s1_cpu enciende CR4.UMIP: SGDT/SIDT/SLDT/STR ya no fugan del kernel"),
    }
}

// ** DONDE ESTA LA COMPROBACION DE ESTE FICHERO
//
// La regla de arriba --ninguna fila muda-- NO se comprueba con un `#[test]`
// aqui, y no por pereza: `cargo test` no puede construir el crate del kernel
// (`no_std` + el arnes de tests da `panic_impl` duplicado), asi que un test
// escrito en este fichero seria un test que existe y no se ejecuta nunca.
//
// Vive como el contador `mudas` de `Censo`, que el comando `ext` imprime y que
// tiene que ser cero. Un numero en la pantalla se mira; un test que no corre
// solo tranquiliza.
