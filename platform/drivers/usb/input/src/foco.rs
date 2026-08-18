//! **El foco**: quien recibe las teclas cuando hay mas de una ventana.
//!
//! === Como se llama esto de verdad ===
//!
//! Lo que la gente conoce como "Alt+Tab" son en realidad cuatro conceptos con
//! nombre propio, y separarlos es lo que evita el lio:
//!
//! - **Foco** (*input focus*) -- que ventana recibe el teclado. Es UNA.
//! - **Pila MRU** (*most-recently-used*) -- el orden en que se usaron. Es lo que
//!   Alt+Tab recorre, y por eso pulsarlo dos veces te devuelve a la anterior.
//! - **Z-order** -- el orden de apilamiento en pantalla. **No es lo mismo**: se
//!   parecen tanto que casi todo el mundo los mezcla, y entonces sale un
//!   gestor donde levantar una ventana le da el teclado sin querer.
//! - ***Focus stealing*** -- que una ventana recien abierta te robe el teclado a
//!   mitad de una frase. Tiene antidoto con nombre propio: *focus stealing
//!   prevention*.
//!
//! === Por que vive en `bmo_input` y no en el compositor ===
//!
//! Porque **enrutar entrada es el oficio de este crate**, y porque aqui se
//! puede probar. El compositor es `no_std`/`no_main` para un target sin
//! sistema operativo: no corre un test. Una politica de foco que nadie ejecuta
//! es una politica que se descubre rota mirando la pantalla.
//!
//! Aqui no se dibuja ni se sabe que es un pixel: se contesta "de quien es esta
//! tecla?". El dibujo es del que tiene la pantalla.
//!
//! === Los tres modos ===
//!
//! ```text
//!   Normal     lo que espera cualquiera: abrir una ventana le da el foco,
//!              y Alt+Tab recorre la pila MRU.
//!   Fijo       nadie roba: abrir una ventana NO cambia el foco.
//!              Es `focus stealing prevention`, y sirve cuando estas
//!              escribiendo algo largo y no quieres que nada te lo quite.
//!   Puntero    el foco sigue al raton (`focus-follows-mouse`), como en las X
//!              de toda la vida. Sin clic: pasar por encima basta.
//! ```

/// Cuantas ventanas puede haber. Fijo y pequeno a proposito: aqui no hay
/// allocator, y un escritorio con mas de ocho ventanas no es este proyecto.
pub const MAX_VENTANAS: usize = 8;

/// Que politica sigue el foco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Abrir una ventana le da el foco. Alt+Tab recorre la MRU.
    Normal,
    /// Abrir una ventana **no** toca el foco: *focus stealing prevention*.
    Fijo,
    /// El foco sigue al puntero: *focus-follows-mouse*.
    Puntero,
}

impl Modo {
    pub fn name(self) -> &'static str {
        match self {
            Modo::Normal => "normal",
            Modo::Fijo => "fijo (nadie roba el teclado)",
            Modo::Puntero => "sigue al puntero",
        }
    }

    /// El nombre con lo que hace puesto detras, para una linea de estado.
    ///
    /// Es texto de PANTALLA y por eso es Latin-1 puro: la consola de BMO es de
    /// un byte por caracter a proposito, y un guion largo saldria de basura.
    pub fn nombre_largo(self) -> &'static str {
        match self {
            Modo::Normal => "foco: normal - Alt+Tab recorre las ventanas",
            Modo::Fijo => "foco: fijo - ninguna ventana nueva roba el teclado",
            Modo::Puntero => "foco: sigue al puntero - pasar por encima basta",
        }
    }

    /// El siguiente, para conmutar con una tecla.
    pub fn next(self) -> Modo {
        match self {
            Modo::Normal => Modo::Fijo,
            Modo::Fijo => Modo::Puntero,
            Modo::Puntero => Modo::Normal,
        }
    }
}

/// El gestor de foco.
///
/// Guarda las ventanas **en orden MRU**: `orden[0]` es la que tiene el foco,
/// `orden[1]` la anterior, y asi. Cerrar una la saca; abrir una la mete donde
/// el modo diga.
#[derive(Debug, Clone, Copy)]
pub struct Foco {
    /// Identificadores de ventana, en orden de uso reciente.
    orden: [u8; MAX_VENTANAS],
    n: usize,
    modo: Modo,
    /// Indice que el conmutador esta senalando mientras `Alt` sigue pulsado.
    ///
    /// * Es lo que hace que Alt+Tab se comporte como en cualquier sistema: la
    /// pila **no se reordena hasta que sueltas Alt**. Reordenar en cada Tab
    /// haria que Alt+Tab+Tab volviera al principio en vez de avanzar, porque
    /// cada paso moveria la de destino al frente y la siguiente seria la que
    /// acabas de dejar.
    pointing_at: Option<usize>,
}

impl Default for Foco {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Foco {
    pub const fn nuevo() -> Self {
        Self {
            orden: [0; MAX_VENTANAS],
            n: 0,
            modo: Modo::Normal,
            pointing_at: None,
        }
    }

    pub fn modo(&self) -> Modo {
        self.modo
    }

    pub fn poner_modo(&mut self, m: Modo) {
        self.modo = m;
    }

    /// Cuantas ventanas hay abiertas.
    pub fn abiertas(&self) -> usize {
        self.n
    }

    /// Las ventanas en orden MRU. La primera es la del foco.
    pub fn lista(&self) -> &[u8] {
        &self.orden[..self.n]
    }

    /// Quien esta DELANTE del todo, tenga o no el foco de escritura.
    ///
    /// ** LA REGLA QUE SEPARA LAS DOS COSAS, y salio de una sesion en metal:
    ///
    /// > Las teclas de **ESCRITURA** son de la ventana con el FOCO.
    /// > Las teclas de **NAVEGACION** son de la ventana que se VE DELANTE.
    ///
    /// El 2026-08-09 el dueno abrio CABINA con F11 y RePag/AvPag no movieron
    /// nada, aunque el pie de la ventana los anuncia. El motivo no era un fallo:
    /// era la politica funcionando -- **abrir no es enfocar**, asi que las
    /// teclas seguian yendo a Ejecutar, que estaba detras y usa RePag/AvPag para
    /// su propio historial.
    ///
    /// Las dos conductas son correctas por separado y juntas dan una ventana que
    /// promete algo que no hace. Se separan aqui: una `f` escrita sigue cayendo
    /// donde el foco diga --robarsela seria mucho peor-- pero un RePag mueve **lo
    /// que estas mirando**, que es lo que cualquiera espera al verlo.
    pub fn delante(&self) -> Option<u8> {
        self.orden[..self.n].first().copied()
    }

    /// Esta `ventana` delante del todo?
    pub fn esta_delante(&self, ventana: u8) -> bool {
        self.delante() == Some(ventana)
    }

    /// Quien tiene el foco, o `None` si no hay ninguna abierta.
    ///
    /// * **Esto NO cambia mientras conmutas.** Con Alt pulsado, lo resaltado es
    /// una *propuesta* --[`Foco::pointed_at`]-- y el teclado sigue yendo a donde
    /// iba hasta que sueltas. Son dos preguntas distintas y mezclarlas se nota:
    /// una tecla escrita a mitad de un Alt+Tab acabaria en una ventana que
    /// todavia no habias elegido.
    pub fn actual(&self) -> Option<u8> {
        if self.n == 0 {
            return None;
        }
        Some(self.orden[0])
    }

    /// La que esta resaltada en el conmutador: la que RECIBIRA el foco si
    /// sueltas Alt ahora. Sin conmutar es la que ya lo tiene.
    pub fn pointed_at(&self) -> Option<u8> {
        if self.n == 0 {
            return None;
        }
        Some(self.orden[self.pointed_index()])
    }

    /// Su posicion en la lista, que es lo que el conmutador necesita para
    /// resaltar una fila. Se da hecho: calcularlo fuera obliga a buscar el id
    /// en la lista, y dos ventanas con el mismo id resaltarian la que no es.
    pub fn pointed_index(&self) -> usize {
        self.pointing_at.unwrap_or(0)
    }

    /// Es de esta ventana la tecla que acaba de llegar?
    ///
    /// La pregunta que hace el compositor en cada tecla, y la razon de que este
    /// modulo exista: sin ella, cada ventana mira todas las teclas y decide por
    /// su cuenta -- que es como dos ventanas acaban respondiendo a la misma.
    pub fn es_para(&self, ventana: u8) -> bool {
        self.actual() == Some(ventana)
    }

    fn posicion(&self, ventana: u8) -> Option<usize> {
        self.orden[..self.n].iter().position(|&v| v == ventana)
    }

    /// Se acabo la conmutacion sin elegir.
    ///
    /// * Hace falta en TODO lo que mueve la lista --abrir, cerrar, un clic-- y no
    /// es cosmetico: `pointing_at` es un **indice**, no un id. Insertar o quitar
    /// una ventana desplaza las filas por debajo del resaltado, asi que un
    /// indice que sobrevive a un cambio de lista senala a otra ventana. Es el
    /// clasico de guardar una posicion en vez de una identidad.
    pub fn cancelar_conmutacion(&mut self) {
        self.pointing_at = None;
    }

    /// Abre una ventana. Si ya estaba, no se duplica.
    ///
    /// En `Normal` se lleva el foco; en `Fijo` entra **detras** de la que lo
    /// tiene, que es exactamente *focus stealing prevention*: aparece, se ve,
    /// y no te quita el teclado a mitad de una frase.
    pub fn open(&mut self, ventana: u8) {
        if let Some(p) = self.posicion(ventana) {
            if self.modo != Modo::Fijo {
                self.al_frente(p);
            }
            return;
        }
        if self.n >= MAX_VENTANAS {
            return;
        }
        self.cancelar_conmutacion();
        // Hueco al principio y la nueva delante.
        let destino = if self.modo == Modo::Fijo && self.n > 0 { 1 } else { 0 };
        let mut i = self.n;
        while i > destino {
            self.orden[i] = self.orden[i - 1];
            i -= 1;
        }
        self.orden[destino] = ventana;
        self.n += 1;
    }

    /// Cierra una ventana. El foco pasa a la siguiente de la MRU, que es la
    /// ultima que se uso antes -- y no a una cualquiera.
    pub fn close(&mut self, ventana: u8) {
        let Some(p) = self.posicion(ventana) else { return };
        for i in p..self.n - 1 {
            self.orden[i] = self.orden[i + 1];
        }
        self.n -= 1;
        self.cancelar_conmutacion();
    }

    fn al_frente(&mut self, p: usize) {
        // Traer una al frente es elegir: lo que el conmutador estuviera
        // proponiendo deja de valer.
        self.cancelar_conmutacion();
        if p == 0 {
            return;
        }
        let v = self.orden[p];
        let mut i = p;
        while i > 0 {
            self.orden[i] = self.orden[i - 1];
            i -= 1;
        }
        self.orden[0] = v;
    }

    /// Un `Tab` con Alt pulsado: senala la siguiente.
    ///
    /// No reordena nada todavia -- ver [`Foco::soltar_conmutador`].
    pub fn conmutar(&mut self) {
        if self.n < 2 {
            return;
        }
        let actual = self.pointing_at.unwrap_or(0);
        self.pointing_at = Some((actual + 1) % self.n);
    }

    /// Igual pero hacia atras, para `Alt+Shift+Tab`.
    pub fn conmutar_atras(&mut self) {
        if self.n < 2 {
            return;
        }
        let actual = self.pointing_at.unwrap_or(0);
        self.pointing_at = Some((actual + self.n - 1) % self.n);
    }

    /// Se esta conmutando ahora mismo? El compositor lo usa para saber si
    /// tiene que pintar la ventanita del conmutador.
    pub fn conmutando(&self) -> bool {
        self.pointing_at.is_some()
    }

    /// Se solto Alt: la senalada pasa al frente **de verdad**.
    ///
    /// Aqui es donde la pila se reordena, y es lo que hace que pulsar Alt+Tab
    /// dos veces seguidas te devuelva a donde estabas: la primera vez mueve A
    /// al frente dejando B segunda, y la segunda mueve B al frente otra vez.
    pub fn soltar_conmutador(&mut self) {
        if let Some(p) = self.pointing_at.take() {
            self.al_frente(p);
        }
    }

    /// El puntero entro en una ventana. Solo hace algo en modo `Puntero`.
    pub fn puntero_en(&mut self, ventana: u8) {
        if self.modo != Modo::Puntero {
            return;
        }
        if let Some(p) = self.posicion(ventana) {
            self.al_frente(p);
        }
    }

    /// Alguien hizo clic en una ventana. En `Normal` y `Fijo` esto SI da el
    /// foco: *click-to-focus*. Es la forma explicita de pedirlo, y por eso
    /// funciona hasta en modo `Fijo` -- ahi lo que se impide es que una ventana
    /// se lo tome **sin que nadie se lo pida**.
    pub fn clic_en(&mut self, ventana: u8) {
        if let Some(p) = self.posicion(ventana) {
            self.al_frente(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EJECUTAR: u8 = 0;
    const DATOS: u8 = 1;
    const TERCERA: u8 = 2;

    #[test]
    fn sin_ventanas_no_hay_foco() {
        let f = Foco::nuevo();
        assert_eq!(f.actual(), None);
        assert!(!f.es_para(EJECUTAR));
    }

    #[test]
    fn abrir_una_ventana_le_da_el_foco() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        assert_eq!(f.actual(), Some(EJECUTAR));
        f.open(DATOS);
        assert_eq!(f.actual(), Some(DATOS));
        assert!(!f.es_para(EJECUTAR), "una tecla tiene UN destino");
    }

    /// * *Focus stealing prevention*: en modo `Fijo`, una ventana que se abre
    /// aparece pero **no te quita el teclado**. Es lo que quieres cuando estas
    /// escribiendo algo largo.
    #[test]
    fn en_modo_fijo_una_ventana_nueva_no_roba_el_teclado() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.poner_modo(Modo::Fijo);
        f.open(DATOS);
        assert_eq!(f.actual(), Some(EJECUTAR), "no puede robarlo");
        assert_eq!(f.abiertas(), 2, "pero SI esta abierta");
        // Y pedirlo a mano sigue funcionando: lo que se impide es tomarlo sin
        // que nadie lo pida.
        f.clic_en(DATOS);
        assert_eq!(f.actual(), Some(DATOS));
    }

    /// * La propiedad que define Alt+Tab y la que mas se implementa mal:
    /// **pulsarlo dos veces te devuelve a donde estabas**.
    ///
    /// Sale sola de reordenar al SOLTAR y no en cada Tab. Reordenando en cada
    /// paso, el segundo Tab llevaria a la que acabas de dejar y nunca se
    /// avanzaria mas alla de dos.
    #[test]
    fn alt_tab_dos_veces_vuelve_a_la_anterior() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.open(DATOS); // el foco esta en DATOS

        f.conmutar();
        f.soltar_conmutador();
        assert_eq!(f.actual(), Some(EJECUTAR));

        f.conmutar();
        f.soltar_conmutador();
        assert_eq!(f.actual(), Some(DATOS), "y vuelta");
    }

    /// Con Alt pulsado, cada Tab avanza uno. La pila NO se toca hasta soltar.
    #[test]
    fn con_alt_pulsado_cada_tab_avanza_uno() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.open(DATOS);
        f.open(TERCERA); // MRU: TERCERA, DATOS, EJECUTAR

        f.conmutar();
        assert_eq!(f.pointed_at(), Some(DATOS));
        f.conmutar();
        assert_eq!(f.pointed_at(), Some(EJECUTAR));
        f.conmutar();
        assert_eq!(f.pointed_at(), Some(TERCERA), "da la vuelta");
        f.soltar_conmutador();
        assert_eq!(f.lista(), &[TERCERA, DATOS, EJECUTAR], "la MRU no cambio");
    }

    /// * Lo resaltado es una PROPUESTA. Mientras Alt sigue pulsado, una tecla
    /// suelta va a la ventana de siempre -- no a la que estas mirando y todavia
    /// no has elegido.
    #[test]
    fn mientras_conmutas_las_teclas_siguen_yendo_a_la_de_antes() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.open(DATOS); // el foco esta en DATOS

        f.conmutar();
        assert_eq!(f.pointed_at(), Some(EJECUTAR), "eso es lo que se resalta");
        assert_eq!(f.actual(), Some(DATOS), "y esto es lo que recibe la tecla");
        assert!(f.es_para(DATOS));

        f.soltar_conmutador();
        assert_eq!(f.actual(), Some(EJECUTAR), "ahora si");
    }

    #[test]
    fn se_puede_conmutar_hacia_atras() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.open(DATOS);
        f.open(TERCERA);
        f.conmutar_atras();
        assert_eq!(f.pointed_at(), Some(EJECUTAR), "la ultima de la pila");
    }

    /// * `pointing_at` es un INDICE, no un id: abrir una ventana desplaza las
    /// filas y el resaltado se quedaria apuntando a otra. Abrir cancela la
    /// conmutacion, y entonces soltar Alt no puede llevarte a una ventana que
    /// no elegiste.
    #[test]
    fn abrir_una_ventana_mientras_conmutas_no_deja_el_indice_colgado() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.open(DATOS);
        f.conmutar();
        assert!(f.conmutando());

        f.open(TERCERA);
        assert!(!f.conmutando(), "la conmutacion se cancela");
        assert_eq!(f.actual(), Some(TERCERA));
        f.soltar_conmutador();
        assert_eq!(f.actual(), Some(TERCERA), "soltar Alt ya no mueve nada");
    }

    /// Un clic es elegir a mano, y elegir cancela lo que el conmutador
    /// estuviera proponiendo.
    #[test]
    fn un_clic_cancela_la_conmutacion() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.open(DATOS);
        f.open(TERCERA);
        f.conmutar();
        f.clic_en(EJECUTAR);
        assert!(!f.conmutando());
        assert_eq!(f.actual(), Some(EJECUTAR));
    }

    /// Cerrar la ultima no deja un foco fantasma: sin ventanas, ninguna tecla
    /// es de nadie.
    #[test]
    fn cerrar_la_ultima_deja_el_escritorio_sin_foco() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.close(EJECUTAR);
        assert_eq!(f.abiertas(), 0);
        assert_eq!(f.actual(), None);
        assert_eq!(f.pointed_at(), None);
        assert!(!f.es_para(EJECUTAR));
    }

    /// Y esconder la que tiene el foco lo pasa a la otra: es lo que hace que
    /// `Ctrl+Alt` con Datos abierta deje el teclado en Datos y no en el vacio.
    #[test]
    fn esconder_la_del_foco_se_lo_pasa_a_la_que_queda() {
        let mut f = Foco::nuevo();
        f.open(DATOS);
        f.open(EJECUTAR);
        f.close(EJECUTAR);
        assert_eq!(f.actual(), Some(DATOS));
    }

    /// Con una sola ventana no hay nada que conmutar, y sobre todo no se puede
    /// quedar senalando fuera de la lista.
    #[test]
    fn con_una_sola_ventana_conmutar_no_hace_nada() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.conmutar();
        assert_eq!(f.actual(), Some(EJECUTAR));
        assert!(!f.conmutando());
    }

    /// * Al cerrar, el foco va a **la ultima que se uso**, no a una cualquiera.
    /// Es la diferencia entre volver a lo que estabas haciendo y aterrizar en
    /// una ventana al azar.
    #[test]
    fn al_cerrar_el_foco_va_a_la_ultima_usada() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.open(TERCERA);
        f.open(DATOS); // MRU: DATOS, TERCERA, EJECUTAR
        f.close(DATOS);
        assert_eq!(f.actual(), Some(TERCERA));
    }

    #[test]
    fn abrir_dos_veces_la_misma_no_la_duplica() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.open(DATOS);
        f.open(EJECUTAR);
        assert_eq!(f.abiertas(), 2);
        assert_eq!(f.actual(), Some(EJECUTAR), "y se la trae al frente");
    }

    /// ** DOS VENTANAS CON EL MISMO ID SON UNA SOLA VENTANA.
    ///
    /// No es un fallo de aqui --un id es un `u8` y este modulo no sabe que hay
    /// detras-- pero es la trampa que se cobro el compositor: `W_CPU` y
    /// `W_SOUND` valian los dos 3 desde el 2026-08-12, y nada dejo de
    /// compilar. Se fija como test porque el sintoma no se parece a la causa:
    /// la segunda ventana no entra en la lista --y Alt+Tab no puede llegar a
    /// ella-- y cerrar UNA deja a la otra en la pantalla y sin teclado, de
    /// donde no la saca ni un clic.
    ///
    /// La regla para quien reparta ids: **son identidades, no etiquetas.**
    #[test]
    fn dos_ventanas_con_el_mismo_id_son_una_sola() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.open(TERCERA);
        // La "otra" ventana, que reusa el id sin saberlo.
        f.open(TERCERA);
        assert_eq!(f.abiertas(), 2, "la segunda no entra: `open` no duplica");

        // Se cierra UNA de las dos, y la que sigue abierta pierde el teclado.
        f.close(TERCERA);
        assert!(!f.es_para(TERCERA), "la que queda se quedo sin foco");
        f.clic_en(TERCERA);
        assert!(
            !f.es_para(TERCERA),
            "y el clic no la rescata: `clic_en` mueve una ventana que ENCUENTRA",
        );
    }

    /// `focus-follows-mouse`: pasar por encima basta, sin clic. Y en los otros
    /// modos el puntero NO mueve el foco -- que es justo lo que espera quien no
    /// pidio ese modo.
    #[test]
    fn el_foco_sigue_al_puntero_solo_en_ese_modo() {
        let mut f = Foco::nuevo();
        f.open(DATOS);
        f.open(EJECUTAR);
        f.puntero_en(DATOS);
        assert_eq!(f.actual(), Some(EJECUTAR), "en Normal el raton no manda");

        f.poner_modo(Modo::Puntero);
        f.puntero_en(DATOS);
        assert_eq!(f.actual(), Some(DATOS));
    }

    #[test]
    fn los_modos_rotan_y_vuelven() {
        assert_eq!(Modo::Normal.next(), Modo::Fijo);
        assert_eq!(Modo::Fijo.next(), Modo::Puntero);
        assert_eq!(Modo::Puntero.next(), Modo::Normal);
    }

    /// Cerrar una que no esta abierta no rompe nada ni descuadra la cuenta.
    #[test]
    fn cerrar_una_que_no_estaba_no_hace_nada() {
        let mut f = Foco::nuevo();
        f.open(EJECUTAR);
        f.close(TERCERA);
        assert_eq!(f.abiertas(), 1);
        assert_eq!(f.actual(), Some(EJECUTAR));
    }

    /// ** LA FILA DEL 2026-08-09: **delante** y **foco** son dos preguntas, y
    /// en modo Fijo dan respuestas DISTINTAS.
    ///
    /// Abrir una ventana en Fijo la pone delante sin darle el teclado. Si las
    /// teclas de navegacion se pidieran por foco, la ventana que estas MIRANDO
    /// no responderia a su propio pie de pagina -- que es lo que le paso a
    /// CABINA con RePag/AvPag.
    #[test]
    fn delante_y_foco_no_son_lo_mismo() {
        let mut f = Foco::nuevo();
        f.open(1);
        assert!(f.esta_delante(1) && f.es_para(1), "con una sola, coinciden");

        f.poner_modo(Modo::Fijo);
        f.open(2);
        // En Fijo la nueva NI se pone delante NI recibe el teclado: entra
        // detras de la que estaba. Se fija aqui porque es justo lo que hace que
        // "esta delante" y "la estoy viendo" NO sean la misma pregunta -- el
        // compositor la PINTA encima igualmente al abrirla.
        assert!(!f.esta_delante(2), "en Fijo entra detras");
        assert!(f.esta_delante(1), "la de antes sigue delante");
        assert!(f.es_para(1), "y el teclado se queda donde estaba");
        assert!(!f.es_para(2), "abrir no es enfocar, y eso no cambia");
    }

    /// Y sin ventanas no hay nadie delante. Un `unwrap` aqui seria un panico en
    /// el arranque, antes de que exista una sola ventana.
    #[test]
    fn sin_ventanas_no_hay_nadie_delante() {
        let f = Foco::nuevo();
        assert_eq!(f.delante(), None);
        assert!(!f.esta_delante(0));
    }
}
