//! **LA HUELLA: lo que ya esta pintado no se vuelve a pintar.**
//!
//! # La regla, y por que necesitaba una PIEZA
//!
//! `scene::testigo` lleva desde agosto haciendo exactamente esto: guarda su
//! ultimo estado en un `static` y **vuelve sin pintar si no cambio nada**. Es la
//! regla correcta y esta escrita en su cabecera.
//!
//! *** Y era una COSTUMBRE DE UN FICHERO, no una regla. El pulso y el volcado
//! --sus dos vecinos en la misma barra, escritos despues-- repintan sus 650
//! pixeles de ancho en cada fotograma que pinte, digan lo que digan. Nadie lo
//! decidio: simplemente el que vino despues no copio lo que hacia el de al lado.
//!
//! > La primera ley de esta casa dice que un eje sin juez es prosa. Una regla
//! > que vive en la cabeza del que escribio UN fichero es exactamente eso.
//!
//! Asi que la regla se convierte en un tipo. Quien quiera saltarsela ahora tiene
//! que hacerlo a proposito, que es toda la diferencia.
//!
//! # Como se usa
//!
//! ```text
//!    static mut HUELLA: Huella = Huella::nueva();
//!
//!    let firma = (a as u64) | ((b as u64) << 32);   // lo que se va a pintar
//!    if !cambio(&mut HUELLA, firma) { return; }     // ya esta en pantalla
//! ```
//!
//! # *** LA TRAMPA, y esta pagada: hay que saber OLVIDAR
//!
//! Un panel que solo se repinta al cambiar **desaparece** el dia que alguien
//! pinta la barra entera por debajo: el sitio queda vacio y la huella sigue
//! diciendo *"ya lo puse"*. `testigo` lo aprendio y dejo escrito el porque:
//!
//! > Un hueco vacio donde estaba la luz se lee como *"no hay problema"*, que es
//! > la peor cosa que puede decir un instrumento que se borro.
//!
//! ** Por eso el olvido tiene UN SOLO SITIO --[`super::olvidar_la_barra`]-- y no
//! una llamada suelta por chip. Tres llamadas repartidas es como se anade un
//! cuarto chip y se olvida la suya, y ese fallo no se ve: se ve un hueco.
//!
//! # *** LA EXCEPCION, Y ES LO QUE HACE QUE ESTO SEA UNA REGLA
//!
//! **El pulso NO lleva huella, a proposito.** Su aguja es la prueba de que el
//! bucle da vueltas, y su propio carril amarillo lo dejo escrito el 08-09:
//!
//! > *"la aguja avanza SIEMPRE, y por eso el modulo entero repinta siempre. Es
//! > lo contrario de lo que hace el testigo, y es a proposito: aqui lo que se
//! > ensena no es el valor, es que HAYA LATIDO."*
//!
//! ** Un instrumento de vida que se calla cuando no cambia nada es un
//! instrumento que se calla cuando el bucle se muere. Justo el fallo que costo
//! una vuelta al metal descubrir, y que la aguja existe para tapar.
//!
//! *** Y por **L4** --una regla se prueba diciendo que NO-- esta excepcion es lo
//! que convierte la costumbre en regla: una que no sabe nombrar donde no se
//! aplica no es una regla, es un habito con suerte.
//!
//! # Lo que esta pieza NO promete
//!
//! ```text
//!    [ ] no ahorra mucho HOY: la barra repinta 3-20 veces por segundo, o sea
//!        ~1 MB/s. Es real y es poco, y decirlo vale mas que venderlo
//!    [ ] lo que evita es el precio del EXITO: a 60 fotogramas por segundo los
//!        mismos tres chips serian 3,7 MB/s de pintar lo que ya estaba
//!    [ ] y no sabe si la firma es la buena. Una firma que se deja un campo
//!        fuera congela el chip -- SILENCIO puro. Por eso cada una se escribe
//!        al lado de lo que pinta, y no aqui
//! ```

/// Lo ultimo que un chip dejo en pantalla, resumido en un numero.
///
/// ** Es `Option` y no un centinela, y esa es la unica decision del tipo: con un
/// valor reservado --`0`, `u64::MAX`-- existe una firma legitima que se
/// confundiria con *"nunca pintado"*, y ese fotograma no se dibujaria. `None` no
/// es un `u64`, asi que el caso no puede darse. La casa prefiere lo imposible a
/// lo improbable.
#[derive(Clone, Copy)]
pub(crate) struct Huella(Option<u64>);

impl Huella {
    pub(crate) const fn nueva() -> Self {
        Huella(None)
    }
}

/// **Hay que pintar?** Deja apuntada la firma nueva si la respuesta es si.
///
/// Se toma `&mut` en vez de ser un metodo sobre un `static`: quien llama tiene
/// que conseguir la referencia, y en un `static mut` eso obliga a `addr_of_mut!`
/// -- que es justo el gesto que hace visible que se esta tocando estado global.
#[inline]
pub(crate) fn cambio(h: &mut Huella, firma: u64) -> bool {
    if h.0 == Some(firma) {
        return false;
    }
    h.0 = Some(firma);
    true
}

/// **Olvida lo pintado**: la barra se repinto por debajo y el sitio esta vacio.
#[inline]
pub(crate) fn olvidar(h: &mut Huella) {
    h.0 = None;
}
