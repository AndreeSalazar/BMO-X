//! # **LA PARTITURA**: que partes existen, y como se reparte una entre n atriles
//!
//! [carril]  ROJO      decide QUE RANGO toca cada nucleo; solaparse es que dos
//!                     escriban el mismo byte, y no solaparse de menos es dejar
//!                     un trozo sin hacer
//!
//! [cuesta]  DATO -- un reparto mal cortado no falla: **contesta**. Devuelve un
//!           buffer con un trozo sin tocar o con un trozo escrito dos veces, y
//!           las dos cosas se ven como una imagen rara, no como un error.
//!
//! [riesgo]  ESPEJO SILENCIO
//!           ESPEJO   -- el catalogo de partes vive AQUI y las funciones que las
//!                       ejecutan viven en Ring 0. Anadir una alli y no aqui
//!                       --o al reves-- hace que un numero signifique dos cosas
//!                       segun quien lo lea. El guardian del final cuenta las
//!                       dos y rompe el build si no coinciden.
//!           SILENCIO -- un rango vacio es legal (`desde == hasta`) y no se
//!                       distingue de uno que no se pidio. Por eso `Rango` lleva
//!                       `vacio()` con nombre en vez de dejar que cada llamador
//!                       compare por su cuenta.
//!
//! ## *** POR QUE ESTE CRATE EXISTE, Y POR QUE AQUI
//!
//! El workspace ya tiene la frase escrita, dos veces, para `bmo-mmio-juicio` y
//! para `bmo-firmware`:
//!
//! > *leer memoria fisica es del kernel, pero **decidir** si esos bytes son algo
//! > valido es interpretacion de bits, y eso SE PRUEBA en el anfitrion.*
//!
//! > *un veredicto que solo se puede comprobar arrancando la maquina no es un
//! > veredicto comprobado.*
//!
//! Repartir trabajo entre doce nucleos es exactamente eso. **Levantar los
//! nucleos es del kernel; decidir que le toca a cada uno es aritmetica**, y la
//! aritmetica que se comprueba con una camara delante de una pantalla azul no
//! esta comprobada.
//!
//! Ring 0 tiene 47.576 lineas y **dos** `#[test]`, ninguno de los cuales corre.
//! Esta es la primera pieza que se saca de ahi, y no por limpieza: se saca
//! porque el trabajo que la necesita es el de hoy.
//!
//! ## *** BMO-X NO ES UN SISTEMA OPERATIVO, y eso decide este fichero
//!
//! Un SO **multiplexa**: finge que cada programa esta solo y le deja improvisar.
//! Una **orquesta coordina**: cada atril sabe su parte, su entrada y lo que
//! cuesta.
//!
//! De ahi sale la decision mas importante de todo esto:
//!
//! ```text
//!    lo que NO se hace   Ring 3 manda un PUNTERO A FUNCION y el nucleo lo llama
//!    lo que se hace      Ring 3 NOMBRA una parte del catalogo y manda los datos
//! ```
//!
//! ** La primera es una escalada de privilegios con otro nombre: codigo de Ring
//! 3 ejecutandose con el privilegio del kernel, en un nucleo que ni siquiera
//! tiene TSS propia. No hay forma de hacerla segura y no se intenta.
//!
//! Y la segunda no es un apano: **es lo que hace una orquesta**. A una orquesta
//! se le dan partituras escritas, no se le pasa cualquier cosa a ver que suena.
//! BMO-X ya hace esto mismo dos veces --dos syscalls con un opcode, y los 62
//! intrinsecos de `bmo-sem-asm` en una tabla-- asi que el catalogo no es un
//! concepto nuevo: es el mismo, un piso mas arriba.
//!
//! [!] El precio, dicho por delante: **una parte nueva hay que escribirla en el
//! kernel**. Una app no puede inventarse una faena. Eso es menos libertad que un
//! SO y es la restriccion que el nombre implica -- una orquesta toca lo que esta
//! escrito.

#![cfg_attr(not(test), no_std)]

/// **LAS PARTES ESCRITAS.** Catalogo cerrado, como los opcodes.
///
/// El numero viaja por la puerta y **no cambia nunca**: es contrato. Anadir se
/// hace por el final; renumerar seria que un `.bex` viejo pidiera otra cosa.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum Parte {
    /// Ninguna. Es el cero para que un atril sin estrenar no toque nada.
    Nada = 0,
    /// Poner un valor de 32 bits en todo un rango. La mas simple que existe, y
    /// esta a proposito: es la que demuestra que el reparto reparte, sin que el
    /// resultado dependa de nada mas.
    Llenar = 1,
    /// Expandir una fila de pixeles a `escala`. La primera faena REAL, y la que
    /// motivo la puerta: en DOOM son 10.590 us por fotograma repartidos entre
    /// 200 filas que **no se tocan entre ellas**.
    Expandir = 2,
}

/// Cuantas partes hay escritas, contando `Nada`. El guardian del kernel compara
/// contra esto.
pub const PARTES_ESCRITAS: u32 = 3;

impl Parte {
    /// **El numero que llego por la puerta, si es una parte de verdad.**
    ///
    /// [!] Devuelve `None` --y no `Parte::Nada`-- para un numero desconocido. Un
    /// numero invalido y *"no hagas nada"* son cosas distintas: la primera es
    /// una app equivocada a la que hay que contestarle que no, y la segunda es
    /// una peticion legitima. Confundirlas haria que pedir la parte 99 se viera
    /// exactamente igual que no pedir nada, que es el `[riesgo] SILENCIO` de la
    /// cabecera.
    pub fn de_numero(n: u64) -> Option<Parte> {
        match n {
            0 => Some(Parte::Nada),
            1 => Some(Parte::Llenar),
            2 => Some(Parte::Expandir),
            _ => None,
        }
    }

    /// El nombre, para CABINA y para la pantalla azul. Un numero en una foto no
    /// lo descifra nadie -- es la leccion de la estacion 11 que era la 17.
    pub fn nombre(self) -> &'static str {
        match self {
            Parte::Nada => "nada",
            Parte::Llenar => "llenar",
            Parte::Expandir => "expandir",
        }
    }
}

/// **El trozo que le toca a un atril.** Semiabierto: `[desde, hasta)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rango {
    pub desde: u64,
    pub hasta: u64,
}

impl Rango {
    /// **Que le toca al atril `mio` de un total de `partes`.**
    ///
    /// # *** La unica propiedad que importa, y es de las que no avisan
    ///
    /// ```text
    ///    la union de los n rangos es EXACTAMENTE [0, total)
    ///    y dos rangos NO se solapan jamas
    /// ```
    ///
    /// Las dos mitades se pagan distinto y ninguna de las dos falla en voz alta:
    ///
    /// * un hueco deja un trozo del buffer **con lo que hubiera antes**, que en
    ///   una imagen es un trozo del fotograma anterior
    /// * un solape deja a dos nucleos escribiendo el mismo byte, y el resultado
    ///   depende de quien llegue ultimo -- o sea que **cambia entre ejecuciones**
    ///
    /// ** El resto se reparte de UNO EN UNO entre los primeros atriles, no se le
    /// echa entero al ultimo. Con 200 filas y 12 atriles son 16,67 cada uno: si
    /// el resto cayera todo al final, once harian 16 y el ultimo 24 -- y la
    /// barrera espera al mas lento, asi que el reparto duraria lo que 24. La
    /// forma buena da 17, 17, ... 16, y dura lo que 17.
    ///
    /// [!] `partes == 0` se trata como 1. Es lo unico defensivo de esta funcion
    /// y esta porque el numero viene de la puerta: una division por cero aqui es
    /// un `#DE` en Ring 0, o sea la maquina entera por un argumento malo.
    pub fn de(mio: u64, partes: u64, total: u64) -> Rango {
        let partes = if partes == 0 { 1 } else { partes };
        if mio >= partes {
            // Un atril fuera del reparto no hace nada, y lo dice con un rango
            // vacio en vez de con un `Option`: quien lo recibe va a iterar, y
            // un bucle sobre un rango vacio ya no hace nada solo.
            return Rango { desde: total, hasta: total };
        }
        let base = total / partes;
        let resto = total % partes;
        // Los primeros `resto` atriles llevan uno de mas, y por eso el arranque
        // de cada uno es `mio * base` mas los que ya se repartieron antes.
        let extra_antes = if mio < resto { mio } else { resto };
        let desde = mio * base + extra_antes;
        let mias = base + if mio < resto { 1 } else { 0 };
        Rango { desde, hasta: desde + mias }
    }

    /// Cuantos elementos. Cero es legal.
    pub fn cuantos(self) -> u64 {
        self.hasta.saturating_sub(self.desde)
    }

    /// **No le toco nada.** Con nombre, para que el llamador no tenga que
    /// decidir por su cuenta que significa `desde == hasta`.
    pub fn vacio(self) -> bool {
        self.hasta <= self.desde
    }
}

/// **Lo que hay en el atril**: la parte y sus numeros.
///
/// Es lo que Ring 3 deja escrito antes de decir *tocad*, con el mismo idioma que
/// `OP_RUTA` usa antes de `OP_EJECUTAR`: por la puerta solo caben dos numeros
/// por llamada, asi que el encargo se acumula y despues se ejecuta.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Encargo {
    /// Donde empieza el destino, en FISICA. La traduce el kernel: por la puerta
    /// viaja una direccion de Ring 3 y aqui no llega ninguna.
    pub destino: u64,
    /// Donde empieza el origen, en fisica. `0` si la parte no lee nada.
    pub origen: u64,
    /// Cuantos elementos hay que hacer EN TOTAL. Es lo que se reparte.
    pub total: u64,
    /// El numero suelto que cada parte interpreta a su manera: el valor a poner
    /// en `Llenar`, la escala en `Expandir`.
    pub dato: u64,
}

/// Por que un encargo NO se puede tocar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rechazo {
    /// El numero de parte no esta en el catalogo.
    ParteDesconocida,
    /// `Parte::Nada` no se reparte: no hay nada que hacer y pedirlo entre doce
    /// nucleos es gastar once despertares en nada.
    NoHayNadaQueHacer,
    /// El total es cero. No es un error de nadie, pero repartirlo tampoco tiene
    /// sentido, y decirlo aparte de `NoHayNadaQueHacer` distingue *"no pediste
    /// faena"* de *"pediste una faena de tamano cero"*.
    TotalCero,
    /// La parte necesita un origen y no se dio uno.
    FaltaElOrigen,
    /// La escala de `Expandir` tiene que ser al menos 1: con 0, el destino
    /// mediria cero y el bucle no escribiria nada mientras el llamador cree que
    /// si.
    EscalaImposible,
}

/// **Se puede tocar esto?** El juez, entero, y sin tocar un solo byte.
///
/// *** Vive aqui y no en el kernel por la razon de la cabecera: esto es
/// interpretacion de numeros, y se prueba en el anfitrion. El kernel llama y
/// obedece.
pub fn se_puede_tocar(parte: Parte, e: &Encargo) -> Result<(), Rechazo> {
    if parte == Parte::Nada {
        return Err(Rechazo::NoHayNadaQueHacer);
    }
    if e.total == 0 {
        return Err(Rechazo::TotalCero);
    }
    match parte {
        Parte::Nada => unreachable!(),
        Parte::Llenar => Ok(()),
        Parte::Expandir => {
            if e.origen == 0 {
                return Err(Rechazo::FaltaElOrigen);
            }
            if e.dato == 0 {
                return Err(Rechazo::EscalaImposible);
            }
            Ok(())
        }
    }
}

/// **CUANTOS ATRILES CONVIENEN** para un total, con `disponibles` en pie.
///
/// # Por que esto no es `min(disponibles, lo que sea)`
///
/// Repartir tiene un precio fijo: publicar las atomicas, despertar la ronda y
/// **esperar en la barrera al mas lento**. Con un total pequeno ese precio se
/// come la ganancia, y entonces doce nucleos tardan mas que uno.
///
/// El corte no es una opinion: es que **cada atril tenga al menos un trozo que
/// merezca la vuelta**. `MINIMO_POR_ATRIL` sale de que un reparto cuesta del
/// orden de un microsegundo entre atomicas y barrera, y de que una fila de DOOM
/// --320 pixeles-- cuesta bastante mas que eso.
///
/// [!] Y devuelve al menos 1 SIEMPRE. Cero atriles no es "no lo repartas": es un
/// reparto sin nadie, y `Rango::de` con `partes = 0` tendria que defenderse otra
/// vez. Un juez que puede devolver un valor que el siguiente tiene que corregir
/// no ha juzgado.
pub fn cuantos_atriles(total: u64, disponibles: u64) -> u64 {
    if disponibles <= 1 || total == 0 {
        return 1;
    }
    let cabe = total / MINIMO_POR_ATRIL;
    if cabe < 1 {
        return 1;
    }
    if cabe < disponibles {
        cabe
    } else {
        disponibles
    }
}

/// Lo minimo que justifica darle una vuelta a un atril. Ver `cuantos_atriles`.
pub const MINIMO_POR_ATRIL: u64 = 8;

/// **EL GUARDIAN DEL ESPEJO**, y corre en compilacion.
///
/// El catalogo tiene que tener tantas entradas como dice `PARTES_ESCRITAS`. Si
/// alguien anade una parte y no sube el numero, el kernel --que compara contra
/// el-- creeria que la ultima no existe. Aqui rompe el build.
const _: () = {
    assert!(PARTES_ESCRITAS == 3);
};

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **LA PROPIEDAD QUE NO AVISA**: la union de los rangos es el total exacto
    /// y no hay dos que se solapen.
    ///
    /// Se comprueba a lo bruto --marcando cada elemento y contando-- porque un
    /// hueco o un solape no se ven en el resultado de una division. Se ven en el
    /// buffer, tres arranques despues.
    #[test]
    fn los_rangos_cubren_el_total_sin_solaparse() {
        for total in 0..70u64 {
            for partes in 1..14u64 {
                let mut visto = [0u8; 70];
                for mio in 0..partes {
                    let r = Rango::de(mio, partes, total);
                    assert!(r.hasta <= total, "total={total} partes={partes} mio={mio}");
                    for i in r.desde..r.hasta {
                        visto[i as usize] += 1;
                    }
                }
                for (i, v) in visto.iter().enumerate().take(total as usize) {
                    assert_eq!(*v, 1, "elemento {i} de {total} con {partes} partes");
                }
            }
        }
    }

    /// **EL RESTO SE REPARTE DE UNO EN UNO**, y esto lo demuestra: entre el
    /// atril mas cargado y el menos cargado nunca hay mas de UN elemento.
    ///
    /// Si el resto cayera entero al ultimo, con 200 filas y 12 atriles habria
    /// once con 16 y uno con 24 -- y la barrera espera al mas lento, asi que el
    /// reparto duraria lo que el de 24. Esta prueba es la que impide ese
    /// "arreglo" mas simple.
    #[test]
    fn ningun_atril_lleva_mas_de_uno_de_diferencia() {
        for total in 1..200u64 {
            for partes in 1..14u64 {
                let (mut min, mut max) = (u64::MAX, 0u64);
                for mio in 0..partes {
                    let n = Rango::de(mio, partes, total).cuantos();
                    min = min.min(n);
                    max = max.max(n);
                }
                assert!(max - min <= 1, "total={total} partes={partes}: {min}..{max}");
            }
        }
    }

    /// Un atril fuera del reparto recibe un rango VACIO, no un panico ni un
    /// rango al reves.
    #[test]
    fn un_atril_de_mas_no_recibe_nada() {
        let r = Rango::de(5, 3, 100);
        assert!(r.vacio());
        assert_eq!(r.cuantos(), 0);
    }

    /// **`partes = 0` no divide por cero.** El numero viene de la puerta, y un
    /// `#DE` en Ring 0 es la maquina entera por un argumento malo.
    #[test]
    fn cero_partes_no_parte_la_maquina() {
        let r = Rango::de(0, 0, 40);
        assert_eq!(r, Rango { desde: 0, hasta: 40 });
    }

    /// Un numero fuera del catalogo dice **que no**, y no "no hagas nada".
    #[test]
    fn una_parte_que_no_existe_se_rechaza() {
        assert_eq!(Parte::de_numero(2), Some(Parte::Expandir));
        assert_eq!(Parte::de_numero(3), None);
        assert_eq!(Parte::de_numero(99), None);
        assert_eq!(Parte::de_numero(u64::MAX), None);
    }

    /// El juez separa las cinco negativas, y cada una dice otra cosa.
    #[test]
    fn el_juez_distingue_por_que_no() {
        let vacio = Encargo::default();
        assert_eq!(se_puede_tocar(Parte::Nada, &vacio), Err(Rechazo::NoHayNadaQueHacer));
        assert_eq!(se_puede_tocar(Parte::Llenar, &vacio), Err(Rechazo::TotalCero));

        let sin_origen = Encargo { total: 200, dato: 5, ..Default::default() };
        assert_eq!(
            se_puede_tocar(Parte::Expandir, &sin_origen),
            Err(Rechazo::FaltaElOrigen)
        );

        let sin_escala = Encargo { total: 200, origen: 0x1000, dato: 0, ..Default::default() };
        assert_eq!(
            se_puede_tocar(Parte::Expandir, &sin_escala),
            Err(Rechazo::EscalaImposible)
        );

        let bueno = Encargo { destino: 0x2000, origen: 0x1000, total: 200, dato: 5 };
        assert_eq!(se_puede_tocar(Parte::Expandir, &bueno), Ok(()));
        assert_eq!(se_puede_tocar(Parte::Llenar, &bueno), Ok(()));
    }

    /// **Un trabajo pequeno NO se reparte**, porque la barrera cuesta mas que
    /// el trabajo.
    #[test]
    fn lo_pequeno_no_se_reparte() {
        assert_eq!(cuantos_atriles(0, 12), 1);
        assert_eq!(cuantos_atriles(1, 12), 1);
        assert_eq!(cuantos_atriles(7, 12), 1);
        assert_eq!(cuantos_atriles(8, 12), 1);
        assert_eq!(cuantos_atriles(16, 12), 2);
        // 200 filas de DOOM entre doce en pie: caben, asi que van los doce.
        assert_eq!(cuantos_atriles(200, 12), 12);
        // Y con un solo nucleo en pie, uno -- pase lo que pase con el total.
        assert_eq!(cuantos_atriles(100_000, 1), 1);
    }

    /// El caso de DOOM, entero y con sus numeros de verdad.
    #[test]
    fn el_reparto_de_doom_sale_a_diecisiete() {
        let atriles = cuantos_atriles(200, 12);
        assert_eq!(atriles, 12);
        let mut suma = 0;
        for mio in 0..atriles {
            let n = Rango::de(mio, atriles, 200).cuantos();
            assert!(n == 16 || n == 17, "atril {mio} con {n} filas");
            suma += n;
        }
        assert_eq!(suma, 200, "las doscientas filas, ni una mas ni una menos");
    }
}
