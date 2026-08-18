//! **Que ventana es cual** -- el id que maneja el foco, con tipo y con nombre.
//!
//! === Por que esto es un TIPO y no seis constantes ===
//!
//! Hasta el 2026-08-18 esto eran seis `const W_*: u8` en `desktop/mod.rs`, y
//! dos de ellas valian 3: `W_CPU` y `W_SOUND`. Nada dejo de compilar, porque
//! para `bmo_input::Foco` un id es un `u8` y todos los `u8` son igual de
//! validos. Lo que hacia se cuenta entero en `Ventana::Sound`.
//!
//! El numero repetido era el sintoma. **La pieza rota era que el id fuera un
//! `u8` desnudo**, y eso no fallaba en un sitio sino en seis, porque cada
//! ventana nueva habia que acordarse de darla de alta en seis listas escritas
//! a mano:
//!
//! ```text
//!   la constante            podia repetirse sin que nadie avisara
//!   switcher::name          la tabla de nombres, que MINTIO dos veces
//!   el bucle de repintado   `[W_RUN, W_DATA, W_CABINA, W_SOUND]`, sin las
//!                           vitales
//!   el `match` que pinta    con un `_ => {}` al final que se traga el olvido
//!   el `at()` del raton     con un `_ =>` que hacia pasar por Ejecutar
//!                           cualquier id que no conociera
//!   Alt+flechas             con otro `_ => {}`
//! ```
//!
//! Seis sitios que hay que acordarse de ampliar son seis sitios que un dia no
//! se amplian. **Un `enum` los convierte en seis sitios que no compilan**: el
//! compilador prohibe dos discriminantes iguales (E0081), y un `match` sin
//! `_` sobre este tipo obliga a nombrar la ventana nueva antes de arrancar.
//!
//! === Y por que `Foco` sigue hablando en `u8` ===
//!
//! Porque **la politica no sabe que es una ventana**, y eso no es un descuido:
//! es lo que la hace probable. `bmo_input` corre tests de verdad y el
//! compositor no puede --es `no_std`/`no_main` para un target sin sistema
//! operativo--, asi que la politica vive donde se puede ejecutar y aqui solo
//! se le pregunta.
//!
//! Un `u8` que cruza esa frontera esta bien; un `u8` suelto paseandose por
//! ocho ficheros del compositor es lo que costo el 3 repetido. Asi que la
//! frontera se estrecha a un sitio: [`Focus`], que traduce, y [`Ventana::id`],
//! que es la unica funcion de todo el compositor que convierte una ventana en
//! un numero.

use bmo_input::Modo;

/// Una ventana del escritorio.
///
/// El orden de los numeros no significa nada --el foco no los compara-- salvo
/// que **son distintos**, que es justo lo que aqui se esta comprando.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum Ventana {
    /// La terminal. Arranca con el foco y es la unica que no se cierra.
    Run = 0,
    /// F12 -- Datos, el explorador de ESTRATOS.
    Data = 1,
    /// F11 -- CABINA, la consola del kernel.
    Cabina = 2,
    /// F7 -- las vitales del CPU. Ver `scene::vitals`.
    Cpu = 3,
    /// F8 -- la memoria, con QUIEN se la esta comiendo.
    Mem = 4,
    /// F10 -- el sonido. Ver `scene::sound`.
    ///
    /// ** ES 5 Y NO 3, Y ESE ES EL MOTIVO DE ESTE FICHERO.
    ///
    /// Valia 3 --el mismo id que `Cpu`-- desde el dia que nacieron F7 y F8
    /// (2026-08-12): las dos ventanas nuevas cogieron los dos numeros libres
    /// mirando las lineas de ARRIBA, y la del sonido estaba escrita debajo,
    /// fuera de la vista.
    ///
    /// Lo que hacia es peor que no compilar. `Foco` guarda las ventanas **por
    /// id**, asi que dos ventanas con el mismo id eran UNA para la politica:
    /// abrir la segunda no anadia fila --`open` no duplica, y Alt+Tab no podia
    /// llegar a ella-- y cerrar cualquiera de las dos le quitaba la fila **a
    /// las dos**. Con Sonido y CPU abiertas, cerrar CPU con F7 dejaba la
    /// ventana de sonido en la pantalla y fuera de la lista: sus notas `Z..M`
    /// muertas, ESC ya no la cerraba, el asa ya no la arrastraba, y un clic
    /// tampoco la rescataba --`clic_en` mueve una ventana que ENCUENTRA, no
    /// mete una nueva--. La unica salida era F10 dos veces.
    Sound = 5,
}

impl Ventana {
    /// Todas, **de abajo arriba**: este array ES el z-order por defecto, y de
    /// el sale el repintado que deja la pantalla como estaba.
    ///
    /// Antes era una lista escrita a mano en `keys/mod.rs` que se quedo en
    /// cuatro: las vitales no estaban, y lo unico que las salvaba de no verse
    /// era que se repintan solas cada 15 fotogramas. Aqui no se puede olvidar
    /// una, porque el tamano del array lo cuenta el compilador.
    pub(crate) const TODAS: [Ventana; 6] = [
        Ventana::Run,
        Ventana::Data,
        Ventana::Cabina,
        Ventana::Sound,
        Ventana::Cpu,
        Ventana::Mem,
    ];

    /// El numero que entiende `bmo_input::Foco`.
    ///
    /// **La unica funcion del compositor que convierte una ventana en un id.**
    /// Todo lo demas habla en `Ventana`, y por eso ya no se puede escribir un
    /// 3 donde iba un 5.
    pub(crate) fn id(self) -> u8 {
        self as u8
    }

    /// Y de vuelta. `None` para un id que no es de nadie -- que hoy no puede
    /// pasar, porque los unicos ids que entran los pone `id()`.
    pub(crate) fn de_id(id: u8) -> Option<Ventana> {
        Some(match id {
            0 => Ventana::Run,
            1 => Ventana::Data,
            2 => Ventana::Cabina,
            3 => Ventana::Cpu,
            4 => Ventana::Mem,
            5 => Ventana::Sound,
            _ => return None,
        })
    }

    /// El nombre que ensena el conmutador de Alt+Tab.
    ///
    /// ** SIN `_` A PROPOSITO. Esta tabla ya mintio dos veces --CABINA y
    /// Sonido salieron como `?` por no ampliarla, y la de CPU se anunciaba
    /// como "Sonido" por el id compartido--. El sintoma de una tabla que se
    /// queda corta es suave y por eso dura: Alt+Tab funciona, conmuta bien, y
    /// solo miente en el nombre.
    ///
    /// Con el `match` exhaustivo eso deja de depender de que alguien se
    /// acuerde: **una ventana nueva no arranca hasta que tiene nombre.**
    pub(crate) fn nombre(self) -> &'static str {
        match self {
            Ventana::Run => "Ejecutar",
            Ventana::Data => "Datos (ESTRATOS)",
            Ventana::Cabina => "CABINA (kernel)",
            Ventana::Cpu => "CPU",
            Ventana::Mem => "Memoria",
            Ventana::Sound => "Sonido",
        }
    }
}

/// **El foco, hablando en ventanas.**
///
/// Es `bmo_input::Foco` con los ids traducidos y nada mas: ni una decision
/// vive aqui. La politica --quien recibe la tecla, que hace Alt+Tab, que
/// significa `Fijo`-- esta en `bmo_input`, se prueba alli con veinte tests, y
/// este envoltorio existe para que el compositor no tenga que escribir un
/// numero para preguntarle nada.
///
/// * Los nombres de los metodos son los mismos que los de `Foco`. Es aposta:
/// asi quien lea `focus.es_para(...)` aqui y luego abra `foco.rs` encuentra la
/// misma palabra, y lo unico que cambia entre los dos sitios es el tipo del
/// argumento -- que es exactamente lo que este fichero anade.
pub(crate) struct Focus(bmo_input::Foco);

impl Focus {
    pub(crate) fn nuevo() -> Self {
        Self(bmo_input::Foco::nuevo())
    }

    /// Es de esta ventana la tecla que acaba de llegar?
    pub(crate) fn es_para(&self, v: Ventana) -> bool {
        self.0.es_para(v.id())
    }

    /// Quien tiene el foco, o `None` si no hay ninguna abierta.
    ///
    /// ** ES LA PREGUNTA, y `es_para` es la respuesta a una a la vez. Sin
    /// esto, "quien manda?" solo se puede averiguar preguntando por cada
    /// ventana **por su nombre** -- y eso es una lista escrita a mano, que es
    /// de donde salio el `W_SOUND` que valia 3. `top_now` era seis ramas
    /// `abierta && es_para(v)` de las que **como mucho una puede ser cierta**,
    /// porque el foco es UNO: seis preguntas para leer un campo.
    ///
    /// * `bmo_input::Foco` tambien tiene `delante()`, y NO se asoma aqui a
    /// proposito: hoy devuelve exactamente lo mismo que `actual()` --las dos
    /// leen `orden[0]`-- asi que asomarla seria ofrecer dos nombres para una
    /// sola respuesta. La pregunta que su documentacion promete --*quien se ve
    /// delante*-- no la puede contestar una MRU: el z-order de verdad lo lleva
    /// el compositor en `top_before`, y la politica no lo conoce.
    pub(crate) fn actual(&self) -> Option<Ventana> {
        self.0.actual().and_then(Ventana::de_id)
    }

    pub(crate) fn open(&mut self, v: Ventana) {
        self.0.open(v.id());
    }

    pub(crate) fn close(&mut self, v: Ventana) {
        self.0.close(v.id());
    }

    pub(crate) fn clic_en(&mut self, v: Ventana) {
        self.0.clic_en(v.id());
    }

    pub(crate) fn puntero_en(&mut self, v: Ventana) {
        self.0.puntero_en(v.id());
    }

    /// La resaltada en el conmutador: la que RECIBIRA el foco al soltar Alt.
    pub(crate) fn pointed_at(&self) -> Option<Ventana> {
        self.0.pointed_at().and_then(Ventana::de_id)
    }

    pub(crate) fn pointed_index(&self) -> usize {
        self.0.pointed_index()
    }

    /// La lista MRU **en ids**, que es lo que el conmutador pinta.
    ///
    /// Es el unico sitio donde un `u8` sale de aqui, y no se quita sin copiar
    /// la lista entera a un buffer de `Ventana` para pintarla y tirarla. Quien
    /// la pinta la traduce con `Ventana::de_id`.
    pub(crate) fn lista(&self) -> &[u8] {
        self.0.lista()
    }

    pub(crate) fn abiertas(&self) -> usize {
        self.0.abiertas()
    }

    pub(crate) fn conmutar(&mut self) {
        self.0.conmutar();
    }

    pub(crate) fn conmutar_atras(&mut self) {
        self.0.conmutar_atras();
    }

    pub(crate) fn soltar_conmutador(&mut self) {
        self.0.soltar_conmutador();
    }

    pub(crate) fn modo(&self) -> Modo {
        self.0.modo()
    }

    pub(crate) fn poner_modo(&mut self, m: Modo) {
        self.0.poner_modo(m);
    }
}
