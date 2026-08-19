//! **PARTIR UN FLUJO EN BLOQUES, y construir su arbol.** El espejo de `read`.
//!
//! === Que resuelve ===
//!
//! Un fichero de ESTRATOS media **como mucho 96 bytes**: lo que entra dentro
//! del propio nodo. `nodo_de_fichero` rechazaba lo demas, y hacia bien -- partir
//! es otra operacion, con su arbol y sus niveles, y mezclarlas dejaria al que
//! llama sin saber cuantos bloques va a necesitar.
//!
//! Esta es esa otra operacion. El formato admitia cuatro niveles de indireccion
//! desde el primer dia (`Attr::en_bloques`) y **nadie los habia escrito nunca**:
//! el unico que construia arboles era el formateador del anfitrion, con `Vec`.
//!
//! === ** SIN `alloc`, Y ESO MANDA EN EL DISENO ===
//!
//! El constructor del anfitrion junta todos los punteros de un nivel en un
//! `Vec` y despues los agrupa. Aqui no hay `Vec`, y tampoco se puede fingir con
//! un buffer grande: un fichero de 2 GiB tiene medio millon de bloques de datos,
//! o sea 24 MiB solo de punteros de nivel 1.
//!
//! Asi que **el arbol se construye segun pasan los datos**, con UN bloque de
//! indice por nivel. Cada vez que uno se llena se cierra, se escribe, y su
//! puntero sube al de arriba -- que puede llenarse a su vez y repetir. Un
//! acarreo, como sumar a mano.
//!
//! Es la misma forma que `read::descender`, que tambien lleva un buffer por
//! nivel y por la misma razon. Uno baja y el otro sube.
//!
//! === Por que el PLAN va antes que los bytes ===
//!
//! Porque quien escribe tiene que **reservar antes de escribir**, y para
//! reservar hay que saber cuanto. [`plan_de`] contesta eso sin tocar un byte de
//! contenido: cuantos bloques de datos, cuantos de indice en cada nivel, y
//! cuantos niveles. Con eso, el sitio de cada bloque esta decidido antes de
//! empezar y no hace falta preguntarle a nadie donde va el siguiente.
//!
//! ```text
//!   base + 0                     ..  los bloques de DATOS
//!   base + datos                 ..  los indices de nivel 1
//!   base + datos + n1            ..  los de nivel 2
//!   ...                              y la raiz es el ultimo de todos
//! ```

use crate::objects::{BlockPtr, BLOQUE, NIVELES_MAX, PTRS_POR_BLOQUE, PTR_LEN};
use crate::FormatError;

/// Cuantas filas tiene el reparto: los datos mas un indice por nivel.
pub const FILAS: usize = NIVELES_MAX + 1;

/// **Lo que va a costar guardar un flujo**, decidido antes de escribir nada.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Plan {
    /// Bajo cuantos niveles de indireccion queda el contenido.
    pub niveles: u8,
    /// Cuantos bloques hay en cada fila. `por_nivel[0]` son los de DATOS.
    pub por_nivel: [u64; FILAS],
    /// La suma, que es lo que hay que reservar.
    pub total: u64,
}

/// **El reparto para `size` bytes.** `None` si no cabe ni con el tope.
///
/// ** El `None` es "no cabe" y no un arbol truncado en silencio. Con cuatro
/// niveles son unos 200 TiB, mas de lo que cabe en el disco, asi que en la
/// practica solo lo contesta un `size` corrupto -- y entonces vale mas pararse.
pub fn plan_de(size: u64) -> Option<Plan> {
    if size == 0 {
        return None;
    }
    let bloque = BLOQUE as u64;
    let datos = size.div_ceil(bloque);
    let mut por_nivel = [0u64; FILAS];
    por_nivel[0] = datos;
    let mut niveles = 0usize;
    let mut n = datos;
    // Se sube mientras haga falta mas de un bloque para nombrar el nivel de
    // abajo. Cuando queda uno, ESE es la raiz y no hay mas que subir.
    while n > 1 {
        n = n.div_ceil(PTRS_POR_BLOQUE as u64);
        niveles += 1;
        if niveles >= FILAS {
            return None;
        }
        por_nivel[niveles] = n;
    }
    let mut total = 0u64;
    for v in por_nivel.iter() {
        total += *v;
    }
    Some(Plan { niveles: niveles as u8, por_nivel, total })
}

/// **Construye el arbol de un flujo segun van pasando sus trozos.**
///
/// Se le empujan los trozos de datos EN ORDEN --de `BLOQUE` bytes, y el ultimo
/// lo que quede-- y el va escribiendo bloques por medio de `poner`. Al final,
/// [`cerrar`](Arbol::cerrar) entrega la raiz.
///
/// [!] `indice` tiene que traer **un bloque por nivel de indireccion**. Es el
/// mismo trato que `read::descender` con su `scratch`, y por la misma razon: sin
/// un buffer propio, el nivel de abajo machacaria el del padre a medio llenar.
pub struct Arbol<'a> {
    plan: Plan,
    /// Donde va el siguiente bloque de cada fila. Sale del plan, asi que el
    /// sitio de cada bloque esta decidido antes de escribir el primero.
    cursor: [u64; FILAS],
    indice: &'a mut [[u8; BLOQUE]],
    /// Cuantos punteros lleva metidos el indice de cada nivel.
    lleno: [usize; FILAS],
    raiz: Option<BlockPtr>,
    /// Cuantos trozos de datos se han empujado ya.
    trozos: u64,
}

impl<'a> Arbol<'a> {
    /// Empieza un arbol que escribira a partir de `base`.
    ///
    /// `indice` necesita `plan.niveles` bloques. Se comprueba aqui y no al
    /// llenarlos: quedarse sin buffer a mitad dejaria medio arbol escrito.
    pub fn nuevo(plan: Plan, base: u64, indice: &'a mut [[u8; BLOQUE]]) -> Result<Self, FormatError> {
        if indice.len() < plan.niveles as usize {
            return Err(FormatError::SinScratch);
        }
        let mut cursor = [0u64; FILAS];
        let mut donde = base;
        for k in 0..FILAS {
            cursor[k] = donde;
            donde += plan.por_nivel[k];
        }
        Ok(Self { plan, cursor, indice, lleno: [0; FILAS], raiz: None, trozos: 0 })
    }

    /// **Mete el siguiente trozo de datos.** Escribe su bloque y lo cuelga.
    ///
    /// El trozo puede ser mas corto que un bloque -- el ultimo casi siempre lo
    /// es. El puntero guarda su `len`, asi que el lector devuelve exactamente
    /// estos bytes y no el relleno.
    pub fn empujar(
        &mut self,
        datos: &[u8],
        poner: &mut dyn FnMut(u64, &[u8]) -> bool,
    ) -> Result<(), FormatError> {
        if datos.is_empty() || datos.len() > BLOQUE {
            return Err(FormatError::BadField);
        }
        if self.trozos >= self.plan.por_nivel[0] {
            // Mas trozos de los que el plan dijo: pararse. Seguir escribiria
            // encima del primer bloque de indice, que ya tiene sitio asignado.
            return Err(FormatError::BadField);
        }
        let lba = self.cursor[0];
        if !poner(lba, datos) {
            return Err(FormatError::Io);
        }
        self.cursor[0] += 1;
        self.trozos += 1;
        let p = BlockPtr::nuevo(lba, 0, datos);
        self.colgar(1, p, poner)
    }

    /// Cuelga `ptr` del nivel `nivel`, cerrando y subiendo lo que haga falta.
    ///
    /// Es un bucle y no una recursion a proposito: el acarreo puede llegar
    /// hasta arriba, y una recursion con un `&mut dyn FnMut` por el medio es mas
    /// dificil de leer que este `loop`.
    fn colgar(
        &mut self,
        mut nivel: usize,
        mut ptr: BlockPtr,
        poner: &mut dyn FnMut(u64, &[u8]) -> bool,
    ) -> Result<(), FormatError> {
        loop {
            // Por encima del ultimo nivel no hay a quien colgarse: ese puntero
            // ES la raiz. Con `niveles = 0` esto pasa en el primer trozo, que es
            // justo lo que dice el formato -- un flujo de un bloque tiene su
            // bloque de datos por raiz.
            if nivel > self.plan.niveles as usize {
                self.raiz = Some(ptr);
                return Ok(());
            }
            let hueco = self.lleno[nivel] * PTR_LEN;
            self.indice[nivel - 1][hueco..hueco + PTR_LEN].copy_from_slice(&ptr.encode());
            self.lleno[nivel] += 1;
            if self.lleno[nivel] < PTRS_POR_BLOQUE {
                return Ok(());
            }
            ptr = self.cerrar_nivel(nivel, poner)?;
            nivel += 1;
        }
    }

    /// Escribe el indice a medias del nivel `nivel` y devuelve su puntero.
    fn cerrar_nivel(
        &mut self,
        nivel: usize,
        poner: &mut dyn FnMut(u64, &[u8]) -> bool,
    ) -> Result<BlockPtr, FormatError> {
        let usados = self.lleno[nivel] * PTR_LEN;
        let lba = self.cursor[nivel];
        // ** Se escribe SOLO lo usado, y el puntero lo dice en su `len`. El
        // lector saca `len / PTR_LEN` punteros, asi que un indice a medias no
        // necesita relleno nulo -- y no tenerlo es lo que hace que su suma
        // dependa solo de los punteros que hay de verdad.
        if !poner(lba, &self.indice[nivel - 1][..usados]) {
            return Err(FormatError::Io);
        }
        let p = BlockPtr::nuevo(lba, 0, &self.indice[nivel - 1][..usados]);
        self.cursor[nivel] += 1;
        self.lleno[nivel] = 0;
        Ok(p)
    }

    /// **Cierra lo que quede a medias y entrega la raiz.**
    pub fn cerrar(
        mut self,
        poner: &mut dyn FnMut(u64, &[u8]) -> bool,
    ) -> Result<BlockPtr, FormatError> {
        if self.trozos != self.plan.por_nivel[0] {
            // Menos trozos de los que el plan dijo: el arbol quedaria corto y el
            // fichero se leeria a medias sin que nada fallara.
            return Err(FormatError::BadField);
        }
        let mut nivel = 1usize;
        while nivel <= self.plan.niveles as usize {
            if self.lleno[nivel] > 0 {
                let p = self.cerrar_nivel(nivel, poner)?;
                self.colgar(nivel + 1, p, poner)?;
            }
            nivel += 1;
        }
        self.raiz.ok_or(FormatError::BadField)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::RESIDENTE_MAX;
    use crate::read::{descender, Fuente};

    /// Un volumen en memoria, indexado por lba.
    struct Memoria {
        bloques: Vec<[u8; BLOQUE]>,
    }

    impl Memoria {
        fn nueva(n: usize) -> Self {
            Self { bloques: vec![[0u8; BLOQUE]; n] }
        }
    }

    impl Fuente for Memoria {
        fn bloque(&mut self, lba: u64, dst: &mut [u8; BLOQUE]) -> bool {
            match self.bloques.get(lba as usize) {
                Some(b) => {
                    dst.copy_from_slice(b);
                    true
                }
                None => false,
            }
        }
    }

    /// Escribe `datos` con el constructor y lo vuelve a leer con `descender`.
    ///
    /// ** LA CASILLA QUE VALE. No se comprueba que los bytes esten "bien
    /// puestos" --eso seria comprobar el constructor contra si mismo-- sino que
    /// **el lector de siempre**, el que usan el kernel y el formateador, saca lo
    /// que se metio.
    fn ida_y_vuelta(datos: &[u8]) -> Vec<u8> {
        let plan = plan_de(datos.len() as u64).expect("cabe");
        let mut m = Memoria::nueva(plan.total as usize);
        let mut indice = vec![[0u8; BLOQUE]; plan.niveles as usize];
        let raiz = {
            let mut a = Arbol::nuevo(plan, 0, &mut indice).unwrap();
            let mut poner = |lba: u64, b: &[u8]| {
                m.bloques[lba as usize] = [0u8; BLOQUE];
                m.bloques[lba as usize][..b.len()].copy_from_slice(b);
                true
            };
            for trozo in datos.chunks(BLOQUE) {
                a.empujar(trozo, &mut poner).unwrap();
            }
            a.cerrar(&mut poner).unwrap()
        };
        let mut scratch = vec![[0u8; BLOQUE]; FILAS + 1];
        let mut fuera = Vec::new();
        descender(&mut m, &raiz, plan.niveles, &mut scratch, &mut |t| {
            fuera.extend_from_slice(t);
            true
        })
        .unwrap();
        fuera
    }

    /// Un bloque justo: sin indireccion, el bloque de datos ES la raiz.
    #[test]
    fn un_bloque_no_necesita_indice() {
        let plan = plan_de(BLOQUE as u64).unwrap();
        assert_eq!(plan.niveles, 0);
        assert_eq!(plan.total, 1);
        let datos: Vec<u8> = (0..BLOQUE).map(|i| (i % 251) as u8).collect();
        assert_eq!(ida_y_vuelta(&datos), datos);
    }

    /// Un byte mas que un bloque ya pide un nivel, y son TRES bloques: dos de
    /// datos y uno de indice.
    #[test]
    fn pasar_de_un_bloque_pide_un_nivel() {
        let plan = plan_de(BLOQUE as u64 + 1).unwrap();
        assert_eq!(plan.niveles, 1);
        assert_eq!(plan.por_nivel[0], 2);
        assert_eq!(plan.por_nivel[1], 1);
        assert_eq!(plan.total, 3);
    }

    /// ** EL TECHO DE 96 BYTES, SUPERADO Y COMPROBADO POR EL LECTOR.
    ///
    /// Justo lo que `nodo_de_fichero` rechazaba: un contenido que no cabe dentro
    /// del nodo. Se parte, se cuelga y vuelve entero.
    #[test]
    fn lo_que_no_cabia_en_el_nodo_ahora_vuelve_entero() {
        let datos: Vec<u8> = (0..RESIDENTE_MAX * 4).map(|i| (i % 253) as u8).collect();
        assert!(datos.len() > RESIDENTE_MAX);
        assert_eq!(ida_y_vuelta(&datos), datos);
    }

    /// Un trozo final CORTO vuelve corto, no relleno de ceros.
    ///
    /// Es lo que separa un fichero de su bloque: el puntero guarda el `len`, asi
    /// que el lector devuelve los bytes que se metieron y no los 4096 del sitio
    /// donde viven.
    #[test]
    fn el_ultimo_trozo_vuelve_con_su_tamano_y_no_relleno() {
        let datos: Vec<u8> = (0..BLOQUE + 7).map(|i| (i % 199) as u8).collect();
        let vuelta = ida_y_vuelta(&datos);
        assert_eq!(vuelta.len(), BLOQUE + 7);
        assert_eq!(vuelta, datos);
    }

    /// Mas de un indice lleno: el acarreo sube solo.
    ///
    /// `PTRS_POR_BLOQUE + 1` bloques de datos no caben en un indice, asi que
    /// hacen falta dos de nivel 1 y uno de nivel 2. Es el caso que la version
    /// con `Vec` resolvia agrupando; aqui lo resuelve el acarreo.
    #[test]
    fn cuando_un_indice_se_llena_el_acarreo_sube_un_nivel() {
        let bloques = PTRS_POR_BLOQUE + 1;
        let plan = plan_de((bloques * BLOQUE) as u64).unwrap();
        assert_eq!(plan.niveles, 2);
        assert_eq!(plan.por_nivel[0], bloques as u64);
        assert_eq!(plan.por_nivel[1], 2);
        assert_eq!(plan.por_nivel[2], 1);
        // Y se lee entero. Son 348 KiB, que en una prueba de anfitrion sobran.
        let datos: Vec<u8> = (0..bloques * BLOQUE).map(|i| (i % 241) as u8).collect();
        assert_eq!(ida_y_vuelta(&datos), datos);
    }

    /// El plan y `niveles_para` tienen que decir lo mismo.
    ///
    /// ** Son dos cuentas distintas de la misma cosa --una por capacidad, otra
    /// contando bloques-- y estaban en dos ficheros. Que coincidan no se supone:
    /// se comprueba, porque el dia que se separen el sintoma es un arbol escrito
    /// con un `levels` que el lector no espera.
    #[test]
    fn el_plan_y_niveles_para_dicen_lo_mismo() {
        for size in [1u64, 4095, 4096, 4097, 100_000, 348_160, 348_161, 1_000_000] {
            let plan = plan_de(size).unwrap();
            assert_eq!(
                plan.niveles,
                crate::objects::niveles_para(size).unwrap(),
                "size {size}"
            );
        }
    }

    /// Cero bytes no es un flujo: se dice, en vez de devolver un plan de cero
    /// bloques que nadie sabria escribir.
    #[test]
    fn cero_bytes_no_tiene_plan() {
        assert!(plan_de(0).is_none());
    }
}
