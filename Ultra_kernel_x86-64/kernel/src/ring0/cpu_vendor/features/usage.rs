//! `features::usage` -- what BMO TAKES. The contract, written by hand.
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

        // ** AVX esta HABILITADO y sin emisor. XCR0=0b111 incluye AVX, o sea
        // que el kernel ya guarda y restaura YMM en cada puerta -- se paga su
        // coste desde el primer dia. Lo que falta no es kernel: `sem-asm`
        // compone REX/ModRM y AVX usa prefijo VEX (C5/C4), asi que hoy no hay
        // forma de emitir una sola instruccion AVX ni como intrinseco.
        Feat::Avx => Use::No("XCR0 lo habilita y NADIE lo emite: sem-asm no sabe VEX"),
        Feat::Avx2 => Use::No("BLAKE3 saca ~3x con el; pide VEX en sem-asm"),
        Feat::Fma => Use::No("no hay codigo numerico que lo pida"),
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
        // ** Lo mas barato del tablero: RDRAND NO es privilegiado, lo ejecuta
        // Ring 3. Es una fila de intrinsics.toml y CERO kernel.
        Feat::Rdrand => Use::No("firma con clave, ESTRATOS, red, y el hash de Python"),
        Feat::Rdseed => Use::No("semilla de verdad; RDRAND llega antes y basta"),

        // ================= criptografia =================
        Feat::Aes => Use::No("ESTRATOS descarta el cifrado a proposito"),
        Feat::Pclmul => Use::No("sin cifrado ni CRC de hardware, no tiene cliente"),
        Feat::Sha => Use::No("el gate hashea con BLAKE3, no con SHA-256"),

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
        // puesta, en un microkernel cuyo lema declarado es cero confianza en el
        // codigo. Es la seccion mas incomoda de esta tabla y por eso va entera.
        Feat::Nx => Use::No("nadie toca EFER.NXE: TODA pagina que BMO mapea es ejecutable"),
        Feat::Smep => Use::No("impide que Ring 0 EJECUTE una pagina de Ring 3. Un bit de CR4"),
        Feat::Smap => Use::No("impide que Ring 0 LEA una de Ring 3 sin querer. Otro bit"),
        Feat::Umip => Use::No("esconde SGDT/SIDT/SLDT a Ring 3; fuga de direcciones del kernel"),
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
