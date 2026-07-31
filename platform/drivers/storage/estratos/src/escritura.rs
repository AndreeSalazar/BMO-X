//! **El log de escritura** — paso 5 del §10, la parte donde se pierden datos.
//!
//! ═══ Lo que hay aquí, y lo que NO ═══
//!
//! Aquí **no se escribe un sector**. Esto es la máquina de estados de una
//! transacción: quién reserva bloques, en qué orden salen las escrituras, dónde
//! va la barrera y cuál de los dos superbloques se pisa.
//!
//! La E/S la hace el kernel, que es quien tiene el dispositivo. Esa separación
//! no es ceremonia: es lo que permite **probar en el anfitrión la parte que
//! cuesta datos**, sin un disco delante y sin arriesgar el Kingston de nadie.
//! Lo que el diseño llama "aquí empieza lo serio" es exactamente el ORDEN, y el
//! orden es lo que se prueba aquí.
//!
//! ═══ Por qué es una máquina de estados y no un plan ═══
//!
//! Lo natural sería devolver una lista de escrituras. No se puede: esta crate
//! es `no_std` **sin `alloc`**, y un plan son varios KiB por bloque. No hay
//! `Vec` que devolver.
//!
//! Y resulta que la restricción mejora el diseño. Una lista se puede reordenar
//! por accidente; una máquina de estados **no deja**: el superbloque no se
//! puede pedir antes de la barrera porque el método devuelve un error, no
//! porque alguien se acuerde de llamar en orden.
//!
//! ═══ La secuencia, y por qué esa ═══
//!
//! ```text
//!   1. datos, atributos y nodos      en la punta del log
//!   2. el estrato nuevo              con su suma
//!   3. ★ BARRERA (FLUSH CACHE)       esperar al plato, no a la caché
//!   4. el superbloque ALTERNO        generación +1 — ESTO es el commit
//! ```
//!
//! El punto de no retorno es el paso 4 y cabe en **un solo sector**, que es la
//! unidad que el disco garantiza atómica. Antes de él, el volumen es
//! exactamente el de antes; después, el nuevo. No hay estado intermedio
//! observable — que es la definición de una transacción.
//!
//! ★ **La barrera no es opcional.** Un SSD que contesta "ya está" con el dato
//! en su caché convierte todo esto en decoración: si el corte llega entre el 3
//! y el 4, el superbloque nuevo apunta a un estrato que no llegó al plato, y
//! eso es peor que no haber escrito — es un volumen que se monta y miente.

use crate::espacio::Ocupacion;
use crate::{Superblock, SUPER_A_BLOCK, SUPER_B_BLOCK};

/// Por qué una transacción no puede seguir.
///
/// Son pocas y cada una manda a hacer algo distinto, que es la regla de
/// siempre: un único "no se pudo" manda a buscar donde no es.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rechazo {
    /// El volumen pasó del 95 %. Ver [`crate::espacio`]: no es una degradación,
    /// es que hace falta sitio **para escribir la transacción que libera sitio**.
    SinSitio,
    /// Lo que se pide no cabe en lo que queda.
    NoCabe,
    /// El volumen no nació en este disco. El gate de identidad del §5.
    OtroDisco,
    /// Se pidió algo fuera de orden. El caso que importa: el superbloque antes
    /// de la barrera.
    FueraDeOrden,
}

impl Rechazo {
    pub fn nombre(self) -> &'static str {
        match self {
            Rechazo::SinSitio => "el volumen esta al 95%: solo lectura",
            Rechazo::NoCabe => "no queda sitio para lo que se pide",
            Rechazo::OtroDisco => "este volumen no nacio en este disco",
            Rechazo::FueraDeOrden => "el commit no puede ir antes de la barrera",
        }
    }
}

/// En qué punto de la secuencia va la transacción.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fase {
    /// Reservando y escribiendo bloques. Nada apunta a ellos todavía: **un
    /// corte aquí deja basura inofensiva**, y por eso esta fase no tiene
    /// vuelta atrás que dar.
    Datos,
    /// Todo lo de datos está mandado al disco; falta que el disco lo confirme.
    Barrera,
    /// Confirmado. Ya se puede pisar el superbloque.
    Commit,
    /// Hecha. El volumen tiene una generación más.
    Cerrada,
}

/// Una transacción de ESTRATOS.
///
/// Se abre con el superbloque montado, se reservan bloques, se cierra la fase
/// de datos, se hace la barrera, y se pide el superbloque nuevo. Cada paso
/// fuera de orden devuelve [`Rechazo::FueraDeOrden`].
#[derive(Debug, Clone, Copy)]
pub struct Transaccion {
    /// Dónde va la punta del log. Sólo avanza.
    log_head: u64,
    /// Dónde estaba al abrir. Si la transacción se abandona, el volumen sigue
    /// siendo el de este punto — porque el superbloque no se tocó.
    log_head_inicial: u64,
    total: u64,
    generacion: u64,
    /// El superbloque tal como estaba. El commit parte de EL: un superbloque
    /// nuevo de cero pierde el `disk_id`, y con el la identidad del volumen.
    origen: Superblock,
    /// Cuál de las dos copias está en uso AHORA. Se escribe la otra.
    copia_en_uso: u64,
    fase: Fase,
}

impl Transaccion {
    /// Abre una transacción sobre el volumen montado.
    ///
    /// `copia_en_uso` es el bloque del superbloque que ganó al montar (el de
    /// generación más alta): 0 o 1. `identidad_ok` es el gate del §5 — un
    /// volumen clonado a otro disco **no se escribe por accidente**.
    pub fn abrir(sb: &Superblock, copia_en_uso: u64, identidad_ok: bool) -> Result<Self, Rechazo> {
        if !identidad_ok {
            return Err(Rechazo::OtroDisco);
        }
        let oc = Ocupacion::de(sb.log_head, sb.total_blocks, sb.block_size);
        if !oc.nivel().admite_escritura() {
            return Err(Rechazo::SinSitio);
        }
        Ok(Self {
            log_head: sb.log_head,
            log_head_inicial: sb.log_head,
            total: sb.total_blocks,
            generacion: sb.generation,
            origen: *sb,
            copia_en_uso,
            fase: Fase::Datos,
        })
    }

    pub fn fase(&self) -> Fase {
        self.fase
    }

    /// Cuántos bloques lleva reservados esta transacción.
    pub fn reservados(&self) -> u64 {
        self.log_head - self.log_head_inicial
    }

    /// Reserva `cuantos` bloques consecutivos y devuelve el primero.
    ///
    /// El asignador entero es esto: un puntero que sólo avanza. No hay lista de
    /// huecos porque **nada se sobreescribe**, así que no hay huecos que
    /// recorrer hasta que exista el recolector.
    ///
    /// ★ Se comprueba contra el total en cada llamada, no sólo al abrir. Una
    /// transacción grande puede empezar cabiendo y dejar de caber a mitad, y
    /// pasarse aquí significa escribir fuera de la partición — encima de lo que
    /// haya detrás.
    pub fn reservar(&mut self, cuantos: u64) -> Result<u64, Rechazo> {
        if self.fase != Fase::Datos {
            return Err(Rechazo::FueraDeOrden);
        }
        if cuantos == 0 {
            return Ok(self.log_head);
        }
        let fin = self.log_head.checked_add(cuantos).ok_or(Rechazo::NoCabe)?;
        if fin > self.total {
            return Err(Rechazo::NoCabe);
        }
        let primero = self.log_head;
        self.log_head = fin;
        Ok(primero)
    }

    /// Se acabaron los datos. A partir de aquí no se reserva nada más.
    pub fn cerrar_datos(&mut self) -> Result<(), Rechazo> {
        if self.fase != Fase::Datos {
            return Err(Rechazo::FueraDeOrden);
        }
        self.fase = Fase::Barrera;
        Ok(())
    }

    /// El disco confirmó que todo lo anterior está en el plato.
    ///
    /// Lo llama quien hizo el `FLUSH CACHE` **y comprobó que salió bien**.
    /// Llamarlo sin haberlo hecho no rompe nada aquí y lo rompe todo en el
    /// disco: es el único punto de esta máquina que no se puede verificar desde
    /// dentro.
    pub fn barrera_hecha(&mut self) -> Result<(), Rechazo> {
        if self.fase != Fase::Barrera {
            return Err(Rechazo::FueraDeOrden);
        }
        self.fase = Fase::Commit;
        Ok(())
    }

    /// El commit: en qué bloque va el superbloque nuevo, y cómo queda.
    ///
    /// Devuelve `(bloque_destino, superbloque)`. **Escribir eso es el punto de
    /// no retorno**, y hasta que se escriba el volumen sigue siendo el de antes.
    ///
    /// ★ El destino es siempre **la copia que NO está en uso**. Si se pisara la
    /// que manda, un corte a mitad de ese sector dejaría el volumen sin ningún
    /// superbloque válido — o sea, sin volumen.
    pub fn commit(&mut self, estrato: crate::BlockPtr) -> Result<(u64, Superblock), Rechazo> {
        if self.fase != Fase::Commit {
            return Err(Rechazo::FueraDeOrden);
        }
        let destino = if self.copia_en_uso == SUPER_A_BLOCK {
            SUPER_B_BLOCK
        } else {
            SUPER_A_BLOCK
        };
        // ★ Se parte del superbloque QUE HABIA y se cambian tres campos.
        //
        // Construirlo de cero con `Superblock::new` perdia el `disk_id` —el
        // gate de identidad grabado en el volumen— y lo dejaba en ceros. El
        // sintoma seria de los peores: el volumen se escribe bien una vez, y al
        // siguiente arranque `identidad_ok` da falso y ESTRATOS se monta en
        // solo lectura para siempre, sin que nada explique por que.
        //
        // Un commit cambia lo que la transaccion decidio y NADA mas.
        let mut sb = self.origen;
        sb.generation = self.generacion + 1;
        sb.log_head = self.log_head;
        sb.estrato = estrato;
        self.fase = Fase::Cerrada;
        Ok((destino, sb))
    }

    /// Abandona la transacción.
    ///
    /// No hay nada que deshacer, y ése es el regalo de no sobreescribir: los
    /// bloques escritos quedan en el log sin que nada los apunte. Son basura —
    /// se recuperan cuando exista el recolector— pero **el volumen sigue
    /// entero**, porque el superbloque no se tocó.
    pub fn abandonar(&mut self) {
        self.fase = Fase::Cerrada;
        self.log_head = self.log_head_inicial;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BlockPtr;

    fn volumen(log_head: u64, total: u64) -> Superblock {
        let mut sb = Superblock::new([7u8; 32], total);
        sb.log_head = log_head;
        sb.generation = 41;
        sb
    }

    fn puntero() -> BlockPtr {
        BlockPtr { lba: 9, off: 0, len: 64, hash: [1u8; 32] }
    }

    #[test]
    fn una_transaccion_normal_recorre_las_cuatro_fases() {
        let mut t = Transaccion::abrir(&volumen(2, 1000), SUPER_A_BLOCK, true).unwrap();
        assert_eq!(t.fase(), Fase::Datos);
        assert_eq!(t.reservar(3).unwrap(), 2);
        assert_eq!(t.reservar(1).unwrap(), 5);
        assert_eq!(t.reservados(), 4);
        t.cerrar_datos().unwrap();
        t.barrera_hecha().unwrap();
        let (destino, sb) = t.commit(puntero()).unwrap();
        assert_eq!(destino, SUPER_B_BLOCK, "se escribe la copia que NO manda");
        assert_eq!(sb.generation, 42);
        assert_eq!(sb.log_head, 6);
    }

    /// ★ La propiedad que sostiene todo: **el commit no puede adelantarse a la
    /// barrera**. Si se pudiera, un corte entre medias dejaría un superbloque
    /// apuntando a un estrato que nunca llegó al plato — un volumen que se
    /// monta y miente, que es peor que no haber escrito.
    ///
    /// Y no depende de que nadie se acuerde: el método devuelve error.
    #[test]
    fn el_commit_no_puede_ir_antes_de_la_barrera() {
        let mut t = Transaccion::abrir(&volumen(2, 1000), SUPER_A_BLOCK, true).unwrap();
        assert_eq!(t.commit(puntero()), Err(Rechazo::FueraDeOrden));
        t.cerrar_datos().unwrap();
        assert_eq!(
            t.commit(puntero()),
            Err(Rechazo::FueraDeOrden),
            "cerrar los datos no es la barrera"
        );
        t.barrera_hecha().unwrap();
        assert!(t.commit(puntero()).is_ok());
    }

    /// Se escribe SIEMPRE la copia alterna. Pisar la que manda deja el volumen
    /// sin ningún superbloque válido si el corte llega a mitad de ese sector.
    #[test]
    fn el_superbloque_nuevo_va_siempre_a_la_otra_copia() {
        for (en_uso, esperado) in [(SUPER_A_BLOCK, SUPER_B_BLOCK), (SUPER_B_BLOCK, SUPER_A_BLOCK)] {
            let mut t = Transaccion::abrir(&volumen(2, 1000), en_uso, true).unwrap();
            t.cerrar_datos().unwrap();
            t.barrera_hecha().unwrap();
            assert_eq!(t.commit(puntero()).unwrap().0, esperado);
        }
    }

    /// Tras la barrera ya no se reserva. Un bloque escrito después del `FLUSH`
    /// no está garantizado en el plato cuando el superbloque lo apunte.
    #[test]
    fn no_se_reserva_despues_de_cerrar_los_datos() {
        let mut t = Transaccion::abrir(&volumen(2, 1000), SUPER_A_BLOCK, true).unwrap();
        t.cerrar_datos().unwrap();
        assert_eq!(t.reservar(1), Err(Rechazo::FueraDeOrden));
    }

    /// ★ El límite se comprueba en CADA reserva, no sólo al abrir. Una
    /// transacción puede empezar cabiendo y dejar de caber a mitad, y pasarse
    /// aquí es escribir fuera de la partición — encima de lo que haya detrás.
    #[test]
    fn reservar_mas_de_lo_que_queda_se_rechaza_a_mitad() {
        let mut t = Transaccion::abrir(&volumen(2, 100), SUPER_A_BLOCK, true).unwrap();
        assert!(t.reservar(90).is_ok());
        assert_eq!(t.reservar(20), Err(Rechazo::NoCabe));
        // Y el rechazo NO consume: lo que cabía sigue cabiendo.
        assert!(t.reservar(8).is_ok());
        assert_eq!(t.reservar(1), Err(Rechazo::NoCabe));
    }

    /// Una reserva que desbordaría el `u64` se rechaza en vez de dar la vuelta.
    /// Con la vuelta, `fin` sale pequeño, la comprobación pasa, y se escribe en
    /// el bloque 3 creyendo que es el 18 trillones.
    #[test]
    fn una_reserva_absurda_no_da_la_vuelta_al_contador() {
        let mut t = Transaccion::abrir(&volumen(4, 1000), SUPER_A_BLOCK, true).unwrap();
        assert_eq!(t.reservar(u64::MAX), Err(Rechazo::NoCabe));
    }

    /// El gate de identidad del §5: un volumen clonado a otro disco no se
    /// escribe por accidente. Se puede leer; escribir es otra cosa.
    #[test]
    fn un_volumen_de_otro_disco_no_se_abre_para_escribir() {
        assert_eq!(
            Transaccion::abrir(&volumen(2, 1000), SUPER_A_BLOCK, false).err(),
            Some(Rechazo::OtroDisco)
        );
    }

    /// Al 95 % no se abre. Ver `espacio.rs`: hace falta sitio para escribir la
    /// transacción que libera sitio.
    #[test]
    fn un_volumen_al_95_no_admite_una_transaccion() {
        assert_eq!(
            Transaccion::abrir(&volumen(950, 1000), SUPER_A_BLOCK, true).err(),
            Some(Rechazo::SinSitio)
        );
        assert!(Transaccion::abrir(&volumen(949, 1000), SUPER_A_BLOCK, true).is_ok());
    }

    /// ★ Abandonar no deshace nada, y no hace falta: los bloques escritos
    /// quedan sin que nada los apunte. El volumen sigue entero porque el
    /// superbloque **no se tocó**. Es el regalo de no sobreescribir.
    #[test]
    fn abandonar_deja_el_volumen_como_estaba() {
        let mut t = Transaccion::abrir(&volumen(2, 1000), SUPER_A_BLOCK, true).unwrap();
        t.reservar(50).unwrap();
        t.abandonar();
        assert_eq!(t.reservados(), 0);
        assert_eq!(t.fase(), Fase::Cerrada);
    }

    /// ★ El commit conserva el `disk_id` y todo lo que la transacción no
    /// decidió.
    ///
    /// Construir el superbloque de cero lo dejaba en ceros, y el síntoma sería
    /// de los peores: se escribe bien **una vez**, y al siguiente arranque el
    /// gate de identidad da falso y ESTRATOS se monta en solo lectura **para
    /// siempre**, sin que nada explique por qué. Un commit cambia lo que la
    /// transacción decidió y nada más.
    #[test]
    fn el_commit_conserva_la_identidad_del_volumen() {
        let sb = volumen(2, 1000);
        let mut t = Transaccion::abrir(&sb, SUPER_A_BLOCK, true).unwrap();
        t.cerrar_datos().unwrap();
        t.barrera_hecha().unwrap();
        let (_, nuevo) = t.commit(puntero()).unwrap();
        assert_eq!(nuevo.disk_id, sb.disk_id, "el disk_id no puede perderse");
        assert_eq!(nuevo.block_size, sb.block_size);
        assert_eq!(nuevo.total_blocks, sb.total_blocks);
        assert_eq!(nuevo.version, sb.version);
    }

    /// Y una transacción cerrada no se puede reutilizar: pedirle un commit más
    /// escribiría una generación repetida, y entonces las dos copias del
    /// superbloque dirían el mismo número y ninguna ganaría.
    #[test]
    fn una_transaccion_cerrada_no_admite_otro_commit() {
        let mut t = Transaccion::abrir(&volumen(2, 1000), SUPER_A_BLOCK, true).unwrap();
        t.cerrar_datos().unwrap();
        t.barrera_hecha().unwrap();
        assert!(t.commit(puntero()).is_ok());
        assert_eq!(t.commit(puntero()), Err(Rechazo::FueraDeOrden));
    }
}
