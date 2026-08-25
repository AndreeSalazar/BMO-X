//! **El azar.** Lo unico que le faltaba a toda la criptografia de este crate
//! para servir de algo.
//!
//! # Por que esto era el agujero mas caro del sistema
//!
//! SHA-256, HMAC, X25519 y AES-GCM estan hechos y probados contra sus vectores
//! oficiales. Y **ninguno vale nada sin esto**:
//!
//! ```text
//!    X25519    su secreto privado tiene que ser IMPREDECIBLE
//!    AES-GCM   su nonce no se puede repetir NUNCA con la misma clave
//!    TCP       su numero de secuencia inicial (RFC 6528)
//!    el .bex   la clave con la que se firma
//! ```
//!
//! *** Una clave predecible **es peor que no cifrar**, y no es una frase: sin
//! cifrar, todo el mundo sabe que el dato esta a la vista y actua en
//! consecuencia. Con una clave predecible el sistema *parece* protegido, se
//! confia en el, y el que sabe entra sin dejar rastro. **El fallo silencioso es
//! el caro.**
//!
//! # De donde sale, y por que no cuesta nada
//!
//! De `RDRAND`, que es un generador de hardware dentro del propio CPU. Y la
//! razon de que esto sea una tarde y no un mes esta en una linea de la tabla de
//! extensiones del kernel:
//!
//! > **`RDRAND` NO es privilegiado.** Lo ejecuta Ring 3 directamente.
//!
//! O sea: cero kernel, cero syscalls, cero puertas. La aplicacion que necesita
//! azar se lo pide al silicio sin pasar por nadie. Es lo mas barato del tablero
//! y llevaba meses sin cliente porque no habia criptografia que lo pidiera.
//!
//! # [!!] Y LO QUE NO HACE, QUE ES LA PARTE IMPORTANTE
//!
//! **No hay respaldo. Si `RDRAND` no esta o no es de fiar, esto devuelve
//! `None` y se acabo.**
//!
//! La tentacion es obvia --"si falla, uso el contador de ciclos y lo mezclo"--
//! y es exactamente como se construye el desastre: un respaldo que nadie ve
//! convierte *"no hay azar"* en *"hay azar malo"*, y lo segundo no se nota
//! hasta que alguien lo rompe. Quien pida una clave y reciba `None` tiene que
//! **parar**, no apanarse.
//!
//! > Un generador roto que devuelve numeros es peor que uno que devuelve nada.
//!
//! # ** Y LA SALIDA VA POR SHA-256, NO CRUDA
//!
//! `RDRAND` es **una caja negra que nadie fuera de AMD ha visto por dentro**, y
//! no hay forma de auditarla desde fuera: una salida sesgada se ve exactamente
//! igual que una buena. Por eso se recogen 64 bytes crudos y se sacan 32
//! hasheados -- cualquier sesgo o estructura muere ahi. Ver [`rellenar`].
//!
//! [!] Lo que eso **no** arregla: si `RDRAND` estuviera saboteado de verdad,
//! hashear no salva nada. Linux mezcla `RDRAND` con otras fuentes justamente
//! por eso. **BMO-X hoy se fia del CPU**, y es una decision --la maquina esta
//! perfilada (ley 24) y el silicio es parte de la base de confianza-- no un
//! descuido. Pero queda escrito.

/// Cuantas veces se reintenta una instruccion que dice "todavia no".
///
/// El generador es fisico y tiene un deposito: si se vacia, `RDRAND` levanta
/// CF=0 y **no escribe nada**. Reintentar es lo correcto; reintentar para
/// siempre no, porque un chip roto colgaria a quien pida una clave. Diez es lo
/// que recomienda el fabricante.
pub const REINTENTOS: u32 = 10;

/// Muestras que se piden al arrancar para decidir si el generador es de fiar.
pub const MUESTRAS: usize = 8;

/// Por que no hay azar. Cada variante es un trabajo distinto, y por eso esto no
/// es un `bool`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Motivo {
    /// El CPU no declara `RDRAND`.
    NoLoTiene,
    /// Lo declara y no entrega: el deposito no se llena.
    NoEntrega,
    /// Entrega, y lo que entrega **no puede ser azar**. Ver [`sano`].
    Sospechoso,
}

/// **Es sospechoso este valor por si solo?**
///
/// Todo ceros o todo unos. Son los dos patrones que salen cuando la lectura
/// **no llega al generador**: un bus que devuelve el valor por defecto da unos,
/// un registro sin escribir da ceros.
///
/// *** Y esto no es teorico. Hay una errata conocida en varias familias de AMD
/// --17h y 19h entre ellas, que es la de esta maquina-- por la que `RDRAND`
/// **devuelve `0xFFFF_FFFF_FFFF_FFFF` para siempre** despues de volver de
/// suspension o con cierto firmware. La instruccion levanta CF=1 diciendo "aqui
/// tienes tu numero aleatorio", y el numero son todo unos.
///
/// ** Es el peor fallo posible de un generador: **no falla**. Contesta. Y lo
/// que contesta se usa como clave.
#[inline]
pub fn sospechoso(v: u64) -> bool {
    v == 0 || v == u64::MAX
}

/// **Se puede creer este generador?**, mirando varias muestras.
///
/// Tres pruebas, y las tres son necesarias porque cazan cosas distintas:
///
/// ```text
///   1. ningun valor todo-ceros ni todo-unos   -> la lectura llega
///   2. no todas iguales                       -> la errata de AMD, y
///                                                un registro congelado
///   3. no son un contador                     -> alguien puso un
///                                                sustituto "temporal"
/// ```
///
/// [!] Esto **no es un test de aleatoriedad**. No dice que el generador sea
/// bueno; dice que no esta obviamente roto. Un generador puede pasar esto y ser
/// pesimo, y por eso lo que autoriza a usarlo no es esta funcion: es que sea
/// `RDRAND` y no otra cosa. Esto solo caza los fallos que se disfrazan.
pub fn sano(muestras: &[u64]) -> Result<(), Motivo> {
    if muestras.is_empty() {
        return Err(Motivo::NoEntrega);
    }
    for &v in muestras {
        if sospechoso(v) {
            return Err(Motivo::Sospechoso);
        }
    }
    // Todas iguales: la errata.
    if muestras.iter().all(|&v| v == muestras[0]) && muestras.len() > 1 {
        return Err(Motivo::Sospechoso);
    }
    // Un contador. Nadie pondria esto a proposito -- y por eso aparece: como
    // relleno "de momento" que se queda.
    if muestras.len() > 2 {
        let paso = muestras[1].wrapping_sub(muestras[0]);
        if muestras.windows(2).all(|p| p[1].wrapping_sub(p[0]) == paso) {
            return Err(Motivo::Sospechoso);
        }
    }
    Ok(())
}

/// Una lectura cruda de `RDRAND`, con sus reintentos. `None` = no entrega.
///
/// # Safety
/// Quien llama tiene que haber comprobado que el CPU declara `RDRAND`. Sin esa
/// comprobacion la instruccion es `#UD`, que en Ring 0 es un fault y en Ring 3
/// es la muerte de la tarea.
#[cfg(target_arch = "x86_64")]
unsafe fn crudo() -> Option<u64> {
    for _ in 0..REINTENTOS {
        let v: u64;
        let ok: u8;
        // `rdrand` deja el numero en el registro y el exito en CF. `setc` baja
        // esa bandera a un byte antes de que cualquier otra cosa la pise --y
        // por eso van en el mismo bloque y no en dos.
        core::arch::asm!(
            "rdrand {v}",
            "setc {ok}",
            v = out(reg) v,
            ok = out(reg_byte) ok,
            options(nomem, nostack)
        );
        if ok != 0 {
            return Some(v);
        }
    }
    None
}

/// El CPU declara `RDRAND`? `CPUID.1:ECX[30]`.
#[cfg(target_arch = "x86_64")]
fn lo_tiene() -> bool {
    // [!] `rbx` se preserva a mano: LLVM lo reserva y no deja pedirlo como
    // salida. Es el mismo baile que hace `cpuid.rs` del perfil del Ryzen.
    let ecx: u32;
    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "pop rbx",
            inout("eax") 1u32 => _,
            out("ecx") ecx,
            out("edx") _,
            options(nostack)
        );
    }
    ecx & (1 << 30) != 0
}

/// **Un numero de 64 bits del generador del CPU**, o el motivo de que no.
///
/// ** Comprueba la salud en CADA llamada y no solo al arrancar. Cuesta siete
/// lecturas mas, y a cambio caza la errata de AMD --que aparece **despues** de
/// una suspension, o sea despues de cualquier revision de arranque-- en el
/// momento en que se pide la clave y no tres horas antes.
#[cfg(target_arch = "x86_64")]
pub fn u64() -> Result<u64, Motivo> {
    if !lo_tiene() {
        return Err(Motivo::NoLoTiene);
    }
    let mut m = [0u64; MUESTRAS];
    for hueco in m.iter_mut() {
        match unsafe { crudo() } {
            Some(v) => *hueco = v,
            None => return Err(Motivo::NoEntrega),
        }
    }
    sano(&m)?;
    Ok(m[MUESTRAS - 1])
}

#[cfg(not(target_arch = "x86_64"))]
pub fn u64() -> Result<u64, Motivo> {
    Err(Motivo::NoLoTiene)
}

/// **Llena un bufer de azar.** Todo o nada.
///
/// # *** LA SALIDA DE `RDRAND` NO SALE DE AQUI TAL CUAL. VA POR SHA-256.
///
/// Y el motivo es el unico que importa en un generador de hardware:
///
/// > **`RDRAND` es una caja negra que no se puede auditar.** Nadie fuera de AMD
/// > ha visto lo que hay dentro, y no hay forma de comprobarlo desde fuera:
/// > una salida sesgada, o incluso una predecible, se ve **exactamente igual**
/// > que una buena.
///
/// ** Por eso ningun sistema serio la usa cruda. Se recogen 64 bytes, se pasan
/// por SHA-256 y salen 32. Eso hace tres cosas:
///
/// ```text
///   1. cualquier SESGO --que un bit salga a 1 mas veces-- desaparece
///   2. cualquier ESTRUCTURA en la salida no llega a la clave
///   3. y si un dia se anade otra fuente, entra por el mismo sitio
/// ```
///
/// [!] Lo que **no** arregla, dicho para que nadie se confie: si `RDRAND`
/// estuviera saboteado de verdad --generando una secuencia que su autor puede
/// predecir-- hashearla no salva nada. Contra eso no hay defensa desde dentro
/// del CPU, y por eso Linux mezcla `RDRAND` con otras fuentes en vez de fiarse.
/// BMO-X hoy **se fia**, y eso es una decision, no un descuido: es una maquina
/// perfilada (ley 24) y el CPU es parte de la base de confianza. Pero se dice.
///
/// [!] Y si falla a mitad, **el bufer se borra**. Un relleno a medias deja la
/// primera parte con azar bueno y el resto con lo que hubiera antes -- y una
/// clave con la cola predecible parece una clave entera. Devolver el error no
/// basta.
pub fn rellenar(dst: &mut [u8]) -> Result<(), Motivo> {
    let mut hecho = 0usize;
    while hecho < dst.len() {
        // ** DOS A UNO: 64 bytes crudos por cada 32 que salen. Es la proporcion
        // de siempre para blanquear una fuente de la que no te fias del todo --
        // si la mitad de la entropia que entra fuera basura, lo que sale sigue
        // teniendo 256 bits de verdad.
        let mut crudo = [0u8; 64];
        for i in 0..8 {
            match u64() {
                Ok(v) => crudo[i * 8..i * 8 + 8].copy_from_slice(&v.to_le_bytes()),
                Err(e) => {
                    for x in dst.iter_mut() {
                        *x = 0;
                    }
                    return Err(e);
                }
            }
        }
        let h = crate::sha256::hash(&crudo);
        // ** Y EL CRUDO SE BORRA. Vive en la pila y la pila se reutiliza: dejar
        // ahi la entropia con la que se hizo una clave es dejar la clave.
        for x in crudo.iter_mut() {
            *x = 0;
        }
        let n = (dst.len() - hecho).min(32);
        dst[hecho..hecho + n].copy_from_slice(&h[..n]);
        hecho += n;
    }
    Ok(())
}

/// **Una clave de 32 bytes**, lista para X25519 o AES-256.
pub fn clave() -> Result<[u8; 32], Motivo> {
    let mut k = [0u8; 32];
    rellenar(&mut k)?;
    Ok(k)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_dos_patrones_que_no_pueden_ser_azar() {
        assert!(sospechoso(0));
        assert!(sospechoso(u64::MAX));
        assert!(!sospechoso(1));
        assert!(!sospechoso(0x1234_5678_9ABC_DEF0));
    }

    /// *** LA ERRATA DE AMD, que es el caso que hay que cazar de verdad: la
    /// instruccion dice que SI y devuelve siempre lo mismo.
    #[test]
    fn todo_unos_para_siempre_se_caza() {
        let m = [u64::MAX; MUESTRAS];
        assert_eq!(sano(&m), Err(Motivo::Sospechoso));
    }

    /// Y un valor congelado que **no** es todo unos: la primera prueba no lo
    /// caza y la segunda si. Por eso son tres y no una.
    #[test]
    fn un_valor_congelado_cualquiera_tambien() {
        let m = [0x0BADC0DE_DEADBEEF; MUESTRAS];
        assert_eq!(sano(&m), Err(Motivo::Sospechoso));
    }

    /// [!] Y el sustituto "temporal" que alguien deja puesto.
    #[test]
    fn un_contador_disfrazado_de_azar_no_cuela() {
        let mut m = [0u64; MUESTRAS];
        for (i, x) in m.iter_mut().enumerate() {
            *x = 1000 + i as u64;
        }
        assert_eq!(sano(&m), Err(Motivo::Sospechoso));
        // Y da igual el paso.
        for (i, x) in m.iter_mut().enumerate() {
            *x = 7 + i as u64 * 4096;
        }
        assert_eq!(sano(&m), Err(Motivo::Sospechoso));
    }

    #[test]
    fn unas_muestras_normales_pasan() {
        let m = [
            0x9E37_79B9_7F4A_7C15,
            0x0123_4567_89AB_CDEF,
            0xFEDC_BA98_7654_3210,
            0x5555_AAAA_3333_CCCC,
            0x0001_0002_0003_0005,
            0xDEAD_BEEF_CAFE_BABE,
            0x1111_2222_4444_8888,
            0xABCD_1234_5678_EF90,
        ];
        assert_eq!(sano(&m), Ok(()));
    }

    #[test]
    fn sin_muestras_no_se_inventa_un_si() {
        assert_eq!(sano(&[]), Err(Motivo::NoEntrega));
    }

    /// *** Y LA DE VERDAD, contra el silicio de quien corra los tests.
    ///
    /// ** Si la maquina tiene RDRAND, dos claves seguidas **no pueden** salir
    /// iguales. Si no lo tiene, se comprueba que lo diga -- y no que devuelva
    /// ceros calladamente, que es justo el fallo que este fichero existe para
    /// no cometer.
    #[test]
    fn el_silicio_de_verdad() {
        match (clave(), clave()) {
            (Ok(a), Ok(b)) => {
                assert_ne!(a, b, "*** DOS CLAVES IGUALES: el generador esta roto");
                assert!(a.iter().any(|&x| x != 0), "una clave de ceros no es una clave");
            }
            (Err(e), _) | (_, Err(e)) => {
                // No es un fallo del test: es una maquina sin RDRAND, y lo que
                // se comprueba es que lo DIGA.
                assert!(matches!(e, Motivo::NoLoTiene | Motivo::NoEntrega | Motivo::Sospechoso));
            }
        }
    }

    /// [!] Un fallo NO deja media clave con azar bueno y el resto con basura.
    #[test]
    fn el_bufer_se_borra_si_no_se_pudo_llenar() {
        // Solo se puede comprobar la forma si el generador va; si no va, el
        // bufer tiene que salir a ceros.
        let mut b = [0xAAu8; 64];
        match rellenar(&mut b) {
            Ok(()) => assert!(b.iter().any(|&x| x != 0xAA)),
            Err(_) => assert!(b.iter().all(|&x| x == 0), "quedo basura en el bufer"),
        }
    }
}
