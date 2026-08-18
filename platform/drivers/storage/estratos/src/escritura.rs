//! **El log de escritura** -- paso 5 del section 10, la parte donde se pierden datos.
//!
//! === Lo que hay aqui, y lo que NO ===
//!
//! Aqui **no se escribe un sector**. Esto es la maquina de estados de una
//! transaccion: quien reserva bloques, en que orden salen las escrituras, donde
//! va la barrera y cual de los dos superbloques se pisa.
//!
//! La E/S la hace el kernel, que es quien tiene el dispositivo. Esa separacion
//! no es ceremonia: es lo que permite **probar en el anfitrion la parte que
//! cuesta datos**, sin un disco delante y sin arriesgar el Kingston de nadie.
//! Lo que el diseno llama "aqui empieza lo serio" es exactamente el ORDEN, y el
//! orden es lo que se prueba aqui.
//!
//! === Por que es una maquina de estados y no un plan ===
//!
//! Lo natural seria devolver una lista de escrituras. No se puede: esta crate
//! es `no_std` **sin `alloc`**, y un plan son varios KiB por bloque. No hay
//! `Vec` que devolver.
//!
//! Y resulta que la restriccion mejora el diseno. Una lista se puede reordenar
//! por accidente; una maquina de estados **no deja**: el superbloque no se
//! puede pedir antes de la barrera porque el metodo devuelve un error, no
//! porque alguien se acuerde de llamar en orden.
//!
//! === La secuencia, y por que esa ===
//!
//! ```text
//!   1. datos, atributos y nodos      en la punta del log
//!   2. el estrato nuevo              con su suma
//!   3. * BARRERA (FLUSH CACHE)       esperar al plato, no a la cache
//!   4. el superbloque ALTERNO        generacion +1 -- ESTO es el commit
//! ```
//!
//! El punto de no retorno es el paso 4 y cabe en **un solo sector**, que es la
//! unidad que el disco garantiza atomica. Antes de el, el volumen es
//! exactamente el de antes; despues, el nuevo. No hay estado intermedio
//! observable -- que es la definicion de una transaccion.
//!
//! * **La barrera no es opcional.** Un SSD que contesta "ya esta" con el dato
//! en su cache convierte todo esto en decoracion: si el corte llega entre el 3
//! y el 4, el superbloque nuevo apunta a un estrato que no llego al plato, y
//! eso es peor que no haber escrito -- es un volumen que se monta y miente.

use crate::espacio::Ocupacion;
use crate::{Superblock, SUPER_A_BLOCK, SUPER_B_BLOCK};

/// Por que una transaccion no puede seguir.
///
/// Son pocas y cada una manda a hacer algo distinto, que es la regla de
/// siempre: un unico "no se pudo" manda a buscar donde no es.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rechazo {
    /// El volumen paso del 95 %. Ver [`crate::espacio`]: no es una degradacion,
    /// es que hace falta sitio **para escribir la transaccion que libera sitio**.
    SinSitio,
    /// Lo que se pide no cabe en lo que queda.
    NoCabe,
    /// El volumen no nacio en este disco. El gate de identidad del section 5.
    OtroDisco,
    /// Se pidio algo fuera de orden. El caso que importa: el superbloque antes
    /// de la barrera.
    FueraDeOrden,
}

impl Rechazo {
    pub fn name(self) -> &'static str {
        match self {
            Rechazo::SinSitio => "el volumen esta al 95%: solo lectura",
            Rechazo::NoCabe => "no queda sitio para lo que se pide",
            Rechazo::OtroDisco => "este volumen no nacio en este disco",
            Rechazo::FueraDeOrden => "el commit no puede ir antes de la barrera",
        }
    }
}

/// En que punto de la secuencia va la transaccion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fase {
    /// Reservando y escribiendo bloques. Nada apunta a ellos todavia: **un
    /// corte aqui deja basura inofensiva**, y por eso esta fase no tiene
    /// vuelta atras que dar.
    Datos,
    /// Todo lo de datos esta mandado al disco; falta que el disco lo confirme.
    Barrera,
    /// Confirmado. Ya se puede pisar el superbloque.
    Commit,
    /// Hecha. El volumen tiene una generacion mas.
    Cerrada,
}

/// Una transaccion de ESTRATOS.
///
/// Se abre con el superbloque montado, se reservan bloques, se cierra la fase
/// de datos, se hace la barrera, y se pide el superbloque nuevo. Cada paso
/// fuera de orden devuelve [`Rechazo::FueraDeOrden`].
#[derive(Debug, Clone, Copy)]
pub struct Transaccion {
    /// Donde va la punta del log. Solo avanza.
    log_head: u64,
    /// Donde estaba al abrir. Si la transaccion se abandona, el volumen sigue
    /// siendo el de este punto -- porque el superbloque no se toco.
    log_head_inicial: u64,
    total: u64,
    generacion: u64,
    /// El superbloque tal como estaba. El commit parte de EL: un superbloque
    /// nuevo de cero pierde el `disk_id`, y con el la identidad del volumen.
    origen: Superblock,
    /// Cual de las dos copias esta en uso AHORA. Se escribe la otra.
    copia_en_uso: u64,
    fase: Fase,
}

impl Transaccion {
    /// Abre una transaccion sobre el volumen montado.
    ///
    /// `copia_en_uso` es el bloque del superbloque que gano al montar (el de
    /// generacion mas alta): 0 o 1. `identidad_ok` es el gate del section 5 -- un
    /// volumen clonado a otro disco **no se escribe por accidente**.
    pub fn open(sb: &Superblock, copia_en_uso: u64, identidad_ok: bool) -> Result<Self, Rechazo> {
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

    /// Cuantos bloques lleva reservados esta transaccion.
    pub fn reservados(&self) -> u64 {
        self.log_head - self.log_head_inicial
    }

    /// Reserva `cuantos` bloques consecutivos y devuelve el primero.
    ///
    /// El asignador entero es esto: un puntero que solo avanza. No hay lista de
    /// huecos porque **nada se sobreescribe**, asi que no hay huecos que
    /// recorrer hasta que exista el recolector.
    ///
    /// * Se comprueba contra el total en cada llamada, no solo al abrir. Una
    /// transaccion grande puede empezar cabiendo y dejar de caber a mitad, y
    /// pasarse aqui significa escribir fuera de la particion -- encima de lo que
    /// haya detras.
    pub fn reserve(&mut self, cuantos: u64) -> Result<u64, Rechazo> {
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

    /// Se acabaron los datos. A partir de aqui no se reserva nada mas.
    pub fn cerrar_datos(&mut self) -> Result<(), Rechazo> {
        if self.fase != Fase::Datos {
            return Err(Rechazo::FueraDeOrden);
        }
        self.fase = Fase::Barrera;
        Ok(())
    }

    /// El disco confirmo que todo lo anterior esta en el plato.
    ///
    /// Lo llama quien hizo el `FLUSH CACHE` **y comprobo que salio bien**.
    /// Llamarlo sin haberlo hecho no rompe nada aqui y lo rompe todo en el
    /// disco: es el unico punto de esta maquina que no se puede verificar desde
    /// dentro.
    pub fn barrera_hecha(&mut self) -> Result<(), Rechazo> {
        if self.fase != Fase::Barrera {
            return Err(Rechazo::FueraDeOrden);
        }
        self.fase = Fase::Commit;
        Ok(())
    }

    /// El commit: en que bloque va el superbloque nuevo, y como queda.
    ///
    /// Devuelve `(bloque_destino, superbloque)`. **Escribir eso es el punto de
    /// no retorno**, y hasta que se escriba el volumen sigue siendo el de antes.
    ///
    /// * El destino es siempre **la copia que NO esta en uso**. Si se pisara la
    /// que manda, un corte a mitad de ese sector dejaria el volumen sin ningun
    /// superbloque valido -- o sea, sin volumen.
    pub fn commit(&mut self, estrato: crate::BlockPtr) -> Result<(u64, Superblock), Rechazo> {
        if self.fase != Fase::Commit {
            return Err(Rechazo::FueraDeOrden);
        }
        let destino = if self.copia_en_uso == SUPER_A_BLOCK {
            SUPER_B_BLOCK
        } else {
            SUPER_A_BLOCK
        };
        // * Se parte del superbloque QUE HABIA y se cambian tres campos.
        //
        // Construirlo de cero con `Superblock::new` perdia el `disk_id` --el
        // gate de identidad grabado en el volumen-- y lo dejaba en ceros. El
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

    /// Abandona la transaccion.
    ///
    /// No hay nada que deshacer, y ese es el regalo de no sobreescribir: los
    /// bloques escritos quedan en el log sin que nada los apunte. Son basura --
    /// se recuperan cuando exista el recolector-- pero **el volumen sigue
    /// entero**, porque el superbloque no se toco.
    pub fn abandonar(&mut self) {
        self.fase = Fase::Cerrada;
        self.log_head = self.log_head_inicial;
    }
}

// == ** LAS TRES PIEZAS DE GUARDAR UN FICHERO ===============================
//
// La transaccion de arriba sabe **cuando** se puede escribir; esto sabe **que**
// bytes hay que poner. Son dos cosas distintas y por eso son dos mitades:
// `sellar` usa la primera con la segunda vacia -- un commit sin datos.
//
// === Por que esto vive aqui y no en el kernel ===
//
// Porque son funciones puras sobre buffers: entran un nombre y unos bytes, sale
// un bloque ya formado. Ni disco, ni reservas, ni orden. Y por eso **se prueban
// en el anfitrion**, que en un sistema de ficheros no es comodidad: aqui un bug
// no da un fault en pantalla, se lleva el trabajo de alguien.
//
// Es el mismo reparto que el descenso por el arbol, que el kernel y el
// formateador comparten desde el primer dia.
//
// === Lo que estas tres NO hacen, dicho ===
//
// No reservan bloques, no escriben, no deciden donde va nada. Quien tenga el
// disco pide sitio a la [`Transaccion`], llama a estas para llenar cada bloque,
// y cierra. Aqui no se puede corromper un volumen porque desde aqui no se
// alcanza ninguno.
//
// [!] Y **un objeto por bloque**, sin compartir. El formateador del anfitrion
// empaqueta varios objetos pequenos en un bloque y hace bien --tiene el volumen
// entero delante-- pero eso pide un asignador con estado, y el primer fichero
// que se escriba desde la maquina no lo necesita: gasta 4 KiB donde caben 560 y
// **es correcto**. Compartir bloque es una optimizacion con su propia prueba, no
// un requisito del formato.

use crate::objects::{
    Attr, BlockPtr, Entrada, Nodo, Tipo, ATTR_DATOS, ATTR_ENTRADAS,
    BLOQUE, ENTRADA_LEN, NODO_LEN, RESIDENTE_MAX,
};
use crate::FormatError;

/// Cuantas entradas caben en el bloque de `:entradas` de un directorio.
///
/// ** 36, y ese es el techo de ficheros por carpeta mientras el atributo viva en
/// UN bloque sin indireccion. No es un limite del formato --`Attr::en_bloques`
/// admite cuatro niveles-- es el limite de esta version, y se dice en vez de
/// descubrirse el dia 37.
pub const ENTRADAS_POR_BLOQUE: usize = BLOQUE / ENTRADA_LEN;

/// **El nodo de un fichero pequeno**, con su contenido DENTRO.
///
/// ** Hasta [`RESIDENTE_MAX`] bytes no gastan un bloque de datos: viven en el
/// atributo, o sea dentro del propio nodo. Un fichero de 30 bytes en un sistema
/// clasico ocupa un bloque entero de 4 KiB y hace falta un salto mas para
/// leerlo; aqui no hay ni bloque ni salto.
///
/// Devuelve `NoCabe` si el contenido pasa de ahi -- **y no se parte en bloques a
/// escondidas**: eso es otra funcion, con su arbol y sus niveles, y mezclarlas
/// haria que el llamante no supiera cuantos bloques va a necesitar.
pub fn nodo_de_fichero(datos: &[u8]) -> Result<[u8; NODO_LEN], FormatError> {
    if datos.len() > RESIDENTE_MAX {
        return Err(FormatError::BadField);
    }
    let a = Attr::residente(ATTR_DATOS, datos)?;
    Ok(Nodo::nuevo(Tipo::Archivo).con(a)?.encode())
}

/// **El bloque de `:entradas` de un directorio, con una entrada MAS.**
///
/// `previas` son los bytes del atributo tal como estan hoy --puede venir vacio
/// en un directorio recien nacido-- y `dst` recibe el bloque nuevo entero.
/// Devuelve cuantos BYTES utiles tiene, que es lo que hay que declarar en el
/// atributo.
///
/// === Por que se copia todo y no se anade al final ===
///
/// Porque ESTRATOS **no sobreescribe**: el bloque viejo sigue donde estaba y lo
/// alcanza el estrato anterior. Escribir la entrada nueva encima del bloque de
/// ayer seria romper el historial, que es exactamente lo que este sistema de
/// ficheros existe para no hacer. Copiar y crecer hacia adelante ES el diseno.
///
/// ** Y un nombre repetido se RECHAZA en vez de sustituir. Sustituir seria una
/// decision de politica --que hacer con el fichero viejo-- y esto es formato: la
/// toma quien llama, que es el unico que sabe si el usuario dijo "sobrescribe".
pub fn entradas_con(
    previas: &[u8],
    nombre: &str,
    nodo: BlockPtr,
    dst: &mut [u8; BLOQUE],
) -> Result<usize, FormatError> {
    let cuantas = previas.len() / ENTRADA_LEN;
    if previas.len() % ENTRADA_LEN != 0 {
        return Err(FormatError::BadField);
    }
    if cuantas + 1 > ENTRADAS_POR_BLOQUE {
        return Err(FormatError::BadField);
    }
    // El nombre se valida ANTES de copiar nada: fallar a mitad dejaria el buffer
    // con medio directorio dentro y el llamante creyendo que no paso nada.
    let nueva = Entrada::nueva(nombre, nodo)?;
    for i in 0..cuantas {
        let e = Entrada::decode(&previas[i * ENTRADA_LEN..(i + 1) * ENTRADA_LEN])?;
        if e.nombre_str() == nombre {
            return Err(FormatError::BadField);
        }
    }
    *dst = [0u8; BLOQUE];
    dst[..previas.len()].copy_from_slice(previas);
    let fin = cuantas * ENTRADA_LEN;
    dst[fin..fin + ENTRADA_LEN].copy_from_slice(&nueva.encode());
    Ok(fin + ENTRADA_LEN)
}

/// **El nodo de un directorio** que apunta a ese bloque de entradas.
///
/// `entradas` es el puntero al bloque que acaba de llenar [`entradas_con`], y
/// `bytes` lo que aquella devolvio. Niveles 0: **la raiz ES el dato**, sin
/// indireccion -- con 36 entradas por bloque, un nivel mas es para el dia que
/// una carpeta pase de 36 ficheros.
pub fn nodo_de_directorio(entradas: BlockPtr, bytes: u64) -> Result<[u8; NODO_LEN], FormatError> {
    let a = Attr::en_bloques(ATTR_ENTRADAS, bytes, 0, entradas)?;
    Ok(Nodo::nuevo(Tipo::Directorio).con(a)?.encode())
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
        let mut t = Transaccion::open(&volumen(2, 1000), SUPER_A_BLOCK, true).unwrap();
        assert_eq!(t.fase(), Fase::Datos);
        assert_eq!(t.reserve(3).unwrap(), 2);
        assert_eq!(t.reserve(1).unwrap(), 5);
        assert_eq!(t.reservados(), 4);
        t.cerrar_datos().unwrap();
        t.barrera_hecha().unwrap();
        let (destino, sb) = t.commit(puntero()).unwrap();
        assert_eq!(destino, SUPER_B_BLOCK, "se escribe la copia que NO manda");
        assert_eq!(sb.generation, 42);
        assert_eq!(sb.log_head, 6);
    }

    /// * La propiedad que sostiene todo: **el commit no puede adelantarse a la
    /// barrera**. Si se pudiera, un corte entre medias dejaria un superbloque
    /// apuntando a un estrato que nunca llego al plato -- un volumen que se
    /// monta y miente, que es peor que no haber escrito.
    ///
    /// Y no depende de que nadie se acuerde: el metodo devuelve error.
    #[test]
    fn el_commit_no_puede_ir_antes_de_la_barrera() {
        let mut t = Transaccion::open(&volumen(2, 1000), SUPER_A_BLOCK, true).unwrap();
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
    /// sin ningun superbloque valido si el corte llega a mitad de ese sector.
    #[test]
    fn el_superbloque_nuevo_va_siempre_a_la_otra_copia() {
        for (en_uso, esperado) in [(SUPER_A_BLOCK, SUPER_B_BLOCK), (SUPER_B_BLOCK, SUPER_A_BLOCK)] {
            let mut t = Transaccion::open(&volumen(2, 1000), en_uso, true).unwrap();
            t.cerrar_datos().unwrap();
            t.barrera_hecha().unwrap();
            assert_eq!(t.commit(puntero()).unwrap().0, esperado);
        }
    }

    /// Tras la barrera ya no se reserva. Un bloque escrito despues del `FLUSH`
    /// no esta garantizado en el plato cuando el superbloque lo apunte.
    #[test]
    fn no_se_reserva_despues_de_cerrar_los_datos() {
        let mut t = Transaccion::open(&volumen(2, 1000), SUPER_A_BLOCK, true).unwrap();
        t.cerrar_datos().unwrap();
        assert_eq!(t.reserve(1), Err(Rechazo::FueraDeOrden));
    }

    /// * El limite se comprueba en CADA reserva, no solo al abrir. Una
    /// transaccion puede empezar cabiendo y dejar de caber a mitad, y pasarse
    /// aqui es escribir fuera de la particion -- encima de lo que haya detras.
    #[test]
    fn reservar_mas_de_lo_que_queda_se_rechaza_a_mitad() {
        let mut t = Transaccion::open(&volumen(2, 100), SUPER_A_BLOCK, true).unwrap();
        assert!(t.reserve(90).is_ok());
        assert_eq!(t.reserve(20), Err(Rechazo::NoCabe));
        // Y el rechazo NO consume: lo que cabia sigue cabiendo.
        assert!(t.reserve(8).is_ok());
        assert_eq!(t.reserve(1), Err(Rechazo::NoCabe));
    }

    /// Una reserva que desbordaria el `u64` se rechaza en vez de dar la vuelta.
    /// Con la vuelta, `fin` sale pequeno, la comprobacion pasa, y se escribe en
    /// el bloque 3 creyendo que es el 18 trillones.
    #[test]
    fn una_reserva_absurda_no_da_la_vuelta_al_contador() {
        let mut t = Transaccion::open(&volumen(4, 1000), SUPER_A_BLOCK, true).unwrap();
        assert_eq!(t.reserve(u64::MAX), Err(Rechazo::NoCabe));
    }

    /// El gate de identidad del section 5: un volumen clonado a otro disco no se
    /// escribe por accidente. Se puede leer; escribir es otra cosa.
    #[test]
    fn un_volumen_de_otro_disco_no_se_abre_para_escribir() {
        assert_eq!(
            Transaccion::open(&volumen(2, 1000), SUPER_A_BLOCK, false).err(),
            Some(Rechazo::OtroDisco)
        );
    }

    /// Al 95 % no se abre. Ver `espacio.rs`: hace falta sitio para escribir la
    /// transaccion que libera sitio.
    #[test]
    fn un_volumen_al_95_no_admite_una_transaccion() {
        assert_eq!(
            Transaccion::open(&volumen(950, 1000), SUPER_A_BLOCK, true).err(),
            Some(Rechazo::SinSitio)
        );
        assert!(Transaccion::open(&volumen(949, 1000), SUPER_A_BLOCK, true).is_ok());
    }

    /// * Abandonar no deshace nada, y no hace falta: los bloques escritos
    /// quedan sin que nada los apunte. El volumen sigue entero porque el
    /// superbloque **no se toco**. Es el regalo de no sobreescribir.
    #[test]
    fn abandonar_deja_el_volumen_como_estaba() {
        let mut t = Transaccion::open(&volumen(2, 1000), SUPER_A_BLOCK, true).unwrap();
        t.reserve(50).unwrap();
        t.abandonar();
        assert_eq!(t.reservados(), 0);
        assert_eq!(t.fase(), Fase::Cerrada);
    }

    /// * El commit conserva el `disk_id` y todo lo que la transaccion no
    /// decidio.
    ///
    /// Construir el superbloque de cero lo dejaba en ceros, y el sintoma seria
    /// de los peores: se escribe bien **una vez**, y al siguiente arranque el
    /// gate de identidad da falso y ESTRATOS se monta en solo lectura **para
    /// siempre**, sin que nada explique por que. Un commit cambia lo que la
    /// transaccion decidio y nada mas.
    #[test]
    fn el_commit_conserva_la_identidad_del_volumen() {
        let sb = volumen(2, 1000);
        let mut t = Transaccion::open(&sb, SUPER_A_BLOCK, true).unwrap();
        t.cerrar_datos().unwrap();
        t.barrera_hecha().unwrap();
        let (_, nuevo) = t.commit(puntero()).unwrap();
        assert_eq!(nuevo.disk_id, sb.disk_id, "el disk_id no puede perderse");
        assert_eq!(nuevo.block_size, sb.block_size);
        assert_eq!(nuevo.total_blocks, sb.total_blocks);
        assert_eq!(nuevo.version, sb.version);
    }

    /// Y una transaccion cerrada no se puede reutilizar: pedirle un commit mas
    /// escribiria una generacion repetida, y entonces las dos copias del
    /// superbloque dirian el mismo numero y ninguna ganaria.
    #[test]
    fn una_transaccion_cerrada_no_admite_otro_commit() {
        let mut t = Transaccion::open(&volumen(2, 1000), SUPER_A_BLOCK, true).unwrap();
        t.cerrar_datos().unwrap();
        t.barrera_hecha().unwrap();
        assert!(t.commit(puntero()).is_ok());
        assert_eq!(t.commit(puntero()), Err(Rechazo::FueraDeOrden));
    }
}
#[cfg(test)]
mod guardar {
    use super::*;
    use crate::objects::{Nodo, ATTR_DATOS, ATTR_ENTRADAS};

    fn ptr(lba: u64, datos: &[u8]) -> BlockPtr {
        BlockPtr::nuevo(lba, 0, datos)
    }

    /// ** LA CASILLA QUE VALE: se escribe un fichero y se VUELVE A LEER.
    ///
    /// No se comprueba que los bytes esten "bien puestos" --eso seria comprobar
    /// el encoder contra si mismo-- sino que el DECODIFICADOR de siempre, el que
    /// usan el kernel y el formateador, saca lo que se metio.
    #[test]
    fn un_fichero_pequeno_cabe_dentro_de_su_nodo_y_vuelve_entero() {
        let texto = b"hola desde BMO-X";
        let bytes = nodo_de_fichero(texto).unwrap();
        let n = Nodo::decode(&bytes).unwrap();
        let a = n.attr(ATTR_DATOS).expect("el nodo tiene :datos");
        assert!(a.es_residente(), "16 bytes NO deben gastar un bloque");
        assert_eq!(a.datos_residentes().unwrap(), texto);
        assert_eq!(a.size, texto.len() as u64);
    }

    /// El limite se dice, y se dice ANTES: 96 bytes entran, 97 se rechazan.
    ///
    /// ** Y se rechaza en vez de partirse en bloques a escondidas. Partir es
    /// otra operacion --con su arbol y sus niveles-- y hacerla aqui dejaria al
    /// que llama sin saber cuantos bloques tiene que reservar.
    #[test]
    fn el_contenido_residente_tiene_un_techo_y_no_se_parte_solo() {
        assert!(nodo_de_fichero(&[b'x'; RESIDENTE_MAX]).is_ok());
        assert!(nodo_de_fichero(&[b'x'; RESIDENTE_MAX + 1]).is_err());
    }

    /// Un directorio vacio mas una entrada: una entrada, y con su nombre.
    #[test]
    fn una_entrada_en_un_directorio_recien_nacido() {
        let hijo = ptr(30, b"nodo");
        let mut b = [0u8; BLOQUE];
        let usados = entradas_con(&[], "nota.txt", hijo, &mut b).unwrap();
        assert_eq!(usados, ENTRADA_LEN);
        let e = Entrada::decode(&b[..ENTRADA_LEN]).unwrap();
        assert_eq!(e.nombre_str(), "nota.txt");
        assert_eq!(e.nodo.lba, 30);
    }

    /// ** LO QUE HABIA SIGUE AHI. Es la casilla del copy-on-write: el bloque
    /// nuevo lleva lo de ayer Y lo de hoy, porque el de ayer no se toca.
    #[test]
    fn las_entradas_de_antes_sobreviven_a_la_nueva() {
        let mut viejo = [0u8; BLOQUE];
        let n1 = entradas_con(&[], "uno.txt", ptr(10, b"a"), &mut viejo).unwrap();
        let mut nuevo = [0u8; BLOQUE];
        let n2 = entradas_con(&viejo[..n1], "dos.txt", ptr(11, b"b"), &mut nuevo).unwrap();
        assert_eq!(n2, 2 * ENTRADA_LEN);
        assert_eq!(Entrada::decode(&nuevo[..ENTRADA_LEN]).unwrap().nombre_str(), "uno.txt");
        let seg = &nuevo[ENTRADA_LEN..2 * ENTRADA_LEN];
        assert_eq!(Entrada::decode(seg).unwrap().nombre_str(), "dos.txt");
        // Y el de ayer intacto: en ESTRATOS nadie sobreescribe.
        assert_eq!(Entrada::decode(&viejo[..ENTRADA_LEN]).unwrap().nombre_str(), "uno.txt");
    }

    /// ** UN NOMBRE REPETIDO SE RECHAZA, no sustituye.
    ///
    /// Sustituir seria una decision de POLITICA --que hacer con el fichero
    /// viejo-- y esto es formato. La toma quien llama, que es el unico que sabe
    /// si el usuario dijo "sobrescribe".
    #[test]
    fn el_mismo_nombre_dos_veces_no_pasa_de_aqui() {
        let mut b = [0u8; BLOQUE];
        let n = entradas_con(&[], "nota.txt", ptr(10, b"a"), &mut b).unwrap();
        let mut b2 = [0u8; BLOQUE];
        assert!(entradas_con(&b[..n], "nota.txt", ptr(11, b"b"), &mut b2).is_err());
    }

    /// El techo de la carpeta se dice en vez de descubrirse el dia 37.
    #[test]
    fn una_carpeta_llena_lo_dice_en_vez_de_pisar_el_bloque_siguiente() {
        let mut b = [0u8; BLOQUE];
        let mut usados = 0usize;
        for i in 0..ENTRADAS_POR_BLOQUE {
            let mut sig = [0u8; BLOQUE];
            // Nombres distintos sin `alloc`: dos letras contadas a mano. En un
            // crate `no_std` un `format!` no existe, y ese es el punto.
            let nom = [b'f', b'a' + (i / 26) as u8, b'a' + (i % 26) as u8];
            let nombre = core::str::from_utf8(&nom).unwrap();
            usados = entradas_con(&b[..usados], nombre, ptr(100 + i as u64, b"x"), &mut sig).unwrap();
            b = sig;
        }
        assert_eq!(usados, ENTRADAS_POR_BLOQUE * ENTRADA_LEN);
        let mut sig = [0u8; BLOQUE];
        assert!(
            entradas_con(&b[..usados], "sobra", ptr(999, b"x"), &mut sig).is_err(),
            "la entrada 37 tiene que rebotar, no escribir fuera del bloque"
        );
    }

    /// Y el directorio entero: nodo -> bloque de entradas -> el fichero.
    /// Es el camino que recorrera el lector de verdad.
    #[test]
    fn el_arbol_completo_se_puede_recorrer_de_vuelta() {
        let texto = b"dos lineas";
        let nodo_f = nodo_de_fichero(texto).unwrap();
        let p_fichero = ptr(40, &nodo_f);

        let mut ents = [0u8; BLOQUE];
        let usados = entradas_con(&[], "leeme.txt", p_fichero, &mut ents).unwrap();
        let p_ents = BlockPtr::nuevo(41, 0, &ents[..usados]);

        let nodo_d = nodo_de_directorio(p_ents, usados as u64).unwrap();
        let dir = Nodo::decode(&nodo_d).unwrap();
        let a = dir.attr(ATTR_ENTRADAS).expect("el directorio tiene :entradas");
        assert!(!a.es_residente(), "las entradas van en bloque: 112 > 96");
        assert_eq!(a.size, usados as u64);
        assert_eq!(a.levels, 0, "sin indireccion: la raiz ES el dato");
        assert_eq!(a.raiz().unwrap().lba, 41);

        // Y desde la entrada se llega al fichero, que es el punto entero.
        let e = Entrada::decode(&ents[..ENTRADA_LEN]).unwrap();
        assert_eq!(e.nombre_str(), "leeme.txt");
        assert_eq!(e.nodo.lba, 40);
        assert!(e.nodo.verifica(&nodo_f), "el puntero comprueba lo que apunta");
        let f = Nodo::decode(&nodo_f).unwrap();
        assert_eq!(f.attr(ATTR_DATOS).unwrap().datos_residentes().unwrap(), texto);
    }
}
