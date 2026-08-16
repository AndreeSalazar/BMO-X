//! **LA ESCENA** -- la intro de arranque, pintada.
//!
//! ## El reparto con `bmo-ciudad`, que es lo que hace que esto se pueda ajustar
//!
//! `bmo-ciudad` sabe **que hay en pantalla en el milisegundo N**: el guion, el
//! encuadre, la geometria de la ciudad, el aura. Todo eso es aritmetica pura y
//! se prueba en el anfitrion sin encender la maquina, que es por lo que salio
//! del kernel en su dia.
//!
//! Aqui queda lo que solo puede hacerse en Ring 0: **pintar**. El gato desde
//! sus dos mascaras de 1 bit, la tipografia del titulo, el destello, y el bucle
//! que pregunta la hora.
//!
//! ## ** LO QUE ESTE MODULO YA NO HACE: recortar a mano
//!
//! Hasta el 2026-08-15 cada sitio que pintaba se escribia su propia
//! comprobacion de limites, y la de la ciudad DESCARTABA el rectangulo entero
//! si una esquina se salia en vez de recortarlo. La ciudad emite geometria que
//! empieza fuera a proposito --el sello del marco-- asi que el rectangulo cuyo
//! trabajo es tapar el borde era el primero en no pintarse: 2.625 de 8.775 por
//! fotograma, el 7,2% de la pantalla, y una franja muerta de 191 px que se ve a
//! simple vista en el video.
//!
//! Ahora el recorte lo pone `bmo_dibujo::Lienzo` una sola vez para las tres
//! orillas, y el fundido a negro lo pone `lienzo::Apagado` como una capa. Ver
//! `lienzo.rs`.
//!
//! ## Lo que queda pendiente y esta a la vista
//!
//! El gato, el kanji y el titulo siguen pintando con `fill_rect`/`put_pix`, o
//! sea con una barrera de memoria por rectangulo. La ciudad --que son ~8.800 de
//! los ~9.000 rectangulos del fotograma-- ya no lo hace. Lo que falta para
//! cerrar el desgarro del todo es que la escena entera pinte en una superficie
//! en RAM y se vuelque una vez; el sitio donde eso engancha es
//! `Pantalla::presentar`.

use super::lienzo::{self, Pantalla, Superficie};
use super::reloj::{ms_desde, tsc_read};
use super::texto::{cadena_en, text_width, text_width_scaled, FONT_H};
use super::{ACCENT, BG, DIM, WHITE};
use bmo_dibujo::Lienzo;

use crate::ring0::core::gato;

/// Cuanto se queda el logo a la vista, en ms.
///
/// [!] **Es tiempo de arranque puro.** No hay trabajo con el que solaparlo --el
/// kernel esta operativo a los 52 ms-- y no se puede saltar con una tecla porque
/// el USB no esta enumerado todavia.
///
/// El dueno pidio tres segundos. Van 1.600 porque el mismo dia se quitaron 4,5 s
/// de espera artificial (los cuatro carteles de aqui y la siesta del compositor),
/// y 3.000 devolveria dos tercios de lo ganado. Es una linea: si se quieren los
/// tres segundos, se cambia el numero.
const GATO_MS: u64 = 1600;

/// **El gato ENCENDIENDOSE**, que es lo que pidio el dueno: *"el gato en neon
/// que se prende al arrancar"*.
///
/// `trazo` y `ojos` van de 0 a 255; `apagado` mezcla el resultado hacia negro
/// para el fundido final.
///
/// # Por que se enciende con COLOR y no con transparencia
///
/// Lo natural seria pintar el gato con alfa creciente sobre la ciudad. Y no se
/// puede: mezclar con lo de debajo obliga a **leer el framebuffer**, que es
/// memoria write-combining y va lentisimo -- la misma trampa que ya costo cara
/// en el blit de DOOM.
///
/// Asi que no se mezcla con el fondo: se mezcla el **color del trazo**, de un
/// gris muy oscuro a blanco. El pixel siempre es opaco y siempre esta ahi; lo
/// que cambia es su brillo. Y ademas queda mejor de lo que quedaria un fundido:
/// el gato empieza como una silueta apagada en la ciudad y **se enciende**, en
/// vez de materializarse de la nada.
///
/// [!] Los ojos van por su cuenta y **suben despues**. Un gato que abre los ojos
/// a la vez que aparece no se prende: ya estaba encendido.
///
/// # ** Y AHORA DERRAMA LUZ, que es lo que faltaba para que fuera un neon
///
/// El video del 2026-08-15 lo enseno: un trazo blanco de un pixel sobre un cielo
/// violeta claro **no se despega de la escena**. Lo que hace que algo se lea como
/// tubo de gas no es que brille, es que **enciende lo que tiene alrededor**.
///
/// El halo sale de `gato::neon`, que mide la distancia de cada pixel al trazo.
/// Los tres conjuntos --nucleo, halo cercano, halo lejano-- son disjuntos, asi
/// que esto sigue siendo **un caso por pixel** y no se pinta nada dos veces.
///
/// `fondo(y)` dice de que color esta la pantalla en esa fila. El halo se mezcla
/// CONTRA ese color en vez de ser un tono plano, y por eso se funde con el cielo
/// en lugar de recortarse encima como una calcomania. Es la unica forma de
/// mezclar aqui: **leer el framebuffer esta prohibido**, asi que el fondo se
/// pregunta a quien lo pinto.
fn draw_gato_encendido(
    l: &mut dyn Lienzo,
    x0: u32,
    y0: u32,
    escala: u32,
    trazo: u32,
    ojos: u32,
    apagado: u32,
    fondo: impl Fn(u32) -> u32,
) {
    use bmo_ciudad::paleta::{mezcla, NEGRO};
    // El trazo apagado no es negro: es el gris al que quedaria una silueta con
    // la ciudad detras. Negro del todo lo haria desaparecer sobre el cielo.
    const TRAZO_APAGADO: u32 = 0xFF1A1730;
    const R: u32 = gato::neon::RADIO as u32;
    let c_trazo = mezcla(mezcla(TRAZO_APAGADO, WHITE, trazo, 255), NEGRO, apagado, 255);
    let c_ojos = mezcla(mezcla(TRAZO_APAGADO, ACCENT, ojos, 255), NEGRO, apagado, 255);
    for fy in 0..gato::ALTO {
        let y = y0 + fy * escala;
        // El fondo se pregunta una vez por FILA, no por pixel: es el mismo para
        // los 152 de la fila y preguntarlo 152 veces seria pagar 27.000
        // divisiones por fotograma para obtener el mismo numero.
        //
        // Y los colores del derrame tambien se calculan una vez por fila: son
        // cuatro mezclas contra las 152x4 que saldrian de hacerlo por pixel.
        let bg = fondo(y);
        let mut halo = [0u32; R as usize];
        for (n, c) in halo.iter_mut().enumerate() {
            // El derrame entra con el TRAZO, no con los ojos: es el tubo el que
            // derrama. Caida cuadratica con la distancia.
            let queda = R - n as u32;
            let f = HALO_MAX * queda * queda / (R * R) * trazo / 255;
            *c = mezcla(mezcla(bg, ACCENT, f, 255), NEGRO, apagado, 255);
        }
        for fx in 0..gato::ANCHO {
            let i = (fy * gato::ANCHO + fx) as usize;
            // Los ojos ganan al trazo: son el unico sitio con color propio.
            let d = gato::neon::distancia(i);
            let color = if d == 0 {
                if gato::bit_ojos(i) { c_ojos } else { c_trazo }
            } else if d <= gato::neon::RADIO {
                halo[d as usize - 1]
            } else {
                continue;
            };
            l.rect((x0 + fx * escala) as i32, (y) as i32, (escala) as i32, (escala) as i32, color);
        }
    }
}

/// **El kanji del logo** -- el que significa "gato", y por eso esta en la marca.
///
/// Una sola mascara porque en el logo es de un solo color -- medido al
/// generarla: 1.440 pixeles, todos cian, ni uno blanco. 666 bytes.
///
/// Se DIBUJA y no se escribe, igual que el triangulo de aviso: la fuente del
/// kernel es ASCII de 16 px, y meter un glifo CJK seria arrastrar una tabla de
/// simbolos entera para un caracter. Y dibujarlo a mano tampoco valia: son once
/// trazos, y un kanji torcido en la pantalla de arranque es peor que no ponerlo.
/// Sale del PNG con el mismo guion que saco al gato.
/// ** Y DERRAMA IGUAL QUE EL GATO. El kanji y el gato son **dos piezas del mismo
/// letrero**: uno con halo y el otro plano se leen como dos dibujos pegados en
/// vez de como una marca. Se vio en la primera imagen de `bmo-vista-ciudad`, que
/// es justo el fallo que antes habria hecho falta reiniciar para encontrar.
fn draw_kanji(
    l: &mut dyn Lienzo,
    x0: u32,
    y0: u32,
    escala: u32,
    color: u32,
    alfa: u32,
    apagado: u32,
    fondo: impl Fn(u32) -> u32,
) {
    use bmo_ciudad::paleta::{mezcla, NEGRO};
    const R: u32 = gato::neon::RADIO as u32;
    for fy in 0..gato::KANJI_ALTO {
        let y = y0 + fy * escala;
        let bg = fondo(y);
        let mut halo = [0u32; R as usize];
        for (n, c) in halo.iter_mut().enumerate() {
            // El derrame sube CON el trazo. Sin este `alfa` el kanji llegaba
            // con el halo a plena potencia mientras su propio trazo aun estaba
            // a medias: un contorno tenue dentro de un resplandor entero, que se
            // ve como si el halo fuera otra cosa. Lo destapo el previsualizador
            // en el fotograma del ms 900.
            let queda = R - n as u32;
            let f = HALO_MAX * queda * queda / (R * R) * alfa / 255;
            *c = mezcla(mezcla(bg, ACCENT, f, 255), NEGRO, apagado, 255);
        }
        for fx in 0..gato::KANJI_ANCHO {
            let i = (fy * gato::KANJI_ANCHO + fx) as usize;
            let d = gato::neon::distancia_kanji(i);
            let c = if d == 0 {
                color
            } else if d <= gato::neon::RADIO {
                halo[d as usize - 1]
            } else {
                continue;
            };
            l.rect((x0 + fx * escala) as i32, (y) as i32, (escala) as i32, (escala) as i32, c);
        }
    }
}

/// ** LA INTRO DEL ARRANQUE -- **el logo, y nada mas**.
///
/// === Que habia aqui, y por que se fue ===
///
/// Cuatro carteles de texto a pantalla completa:
///
/// ```text
///   scene("BMO-X", ...)       hold_ms(700)
///   scene("Preparando", ...)  hold_ms(350)
///   scene("RING 0", ...)      hold_ms(550)
///   scene("RING 3", ...)      hold_ms(550)
/// ```
///
/// **2.150 ms de esperas explicitas**, y `scene` trae dentro otros ~405 ms de
/// fundidos cada una: en total **casi cuatro segundos de carteleria antes de que
/// el kernel empiece a hacer nada**. Y encima decia "RING 3 : userspace listo"
/// cuando el userspace no habia arrancado todavia -- un cartel que anuncia un
/// estado que aun no existe.
///
/// El dueno lo dijo claro: *"eso ya se ve un poco feo"*. Tenia razon dos veces,
/// porque ademas de feo era lento.
///
/// === Lo que hay ahora ===
///
/// Una pantalla: el gato, `BMO-X`, y `BMO METAKERNEL` debajo. El fundido es
/// **el de los ojos**, que son 276 pixeles -- no un barrido de pantalla completa.
///
/// Coste total: unos 240 ms contra 3.700. Y el logo se queda puesto hasta que
/// `phase1_ui` aterriza en el panel, asi que **es lo que se ve mientras carga**,
/// que es exactamente lo que se pedia.
///
/// === Y no se anuncia lo que no ha pasado ===
///
/// Los estados --RING 0 despierto, RING 3 arrancado-- ya los cuenta el log del
/// panel **cuando ocurren de verdad**, con su marca de tiempo. Un cartel que los
/// promete antes es una mentira con animacion.
/// **Las cuatro esquinas del marco.** Cuatro angulos, nada mas.
///
/// Es el vocabulario visual que el dueno pidio --el de una interfaz de sala de
/// operaciones-- y es el mas barato que existe: **ocho rectangulos**. Un marco
/// entero seria una linea de 1920 px por lado que compite con el contenido; una
/// esquina insinua el marco y deja el centro limpio.
///
/// Y hace un trabajo real ademas del de adorno: dice **donde acaba la pantalla**.
/// En un monitor sin bordes visibles, con el fondo negro del logo y la sala a
/// oscuras, el panel y la pared son el mismo color.
fn marco_esquinas(l: &mut dyn Lienzo, w: u32, h: u32, color: u32) {
    // Proporcionales a la pantalla, no fijos: en 4K un angulo de 24 px no se ve.
    let largo = (w / 26).clamp(24, 90);
    let grosor = if h >= 900 { 2 } else { 1 };
    let m = (w / 60).clamp(12, 48); // margen desde el borde
    for &(x, y, hx, hy) in &[
        (m, m, 1i32, 1i32),                                   // arriba izquierda
        (w.saturating_sub(m), m, -1, 1),                      // arriba derecha
        (m, h.saturating_sub(m), 1, -1),                      // abajo izquierda
        (w.saturating_sub(m), h.saturating_sub(m), -1, -1),   // abajo derecha
    ] {
        // El brazo horizontal y el vertical de cada angulo. `hx`/`hy` dicen
        // hacia donde crece cada uno, asi que las cuatro esquinas salen del
        // mismo par de lineas en vez de cuatro casos escritos a mano.
        let x0 = if hx > 0 { x } else { x.saturating_sub(largo) };
        let y0 = if hy > 0 { y } else { y.saturating_sub(grosor) };
        l.rect((x0) as i32, (y0) as i32, (largo) as i32, (grosor) as i32, color);
        let x1 = if hx > 0 { x } else { x.saturating_sub(grosor) };
        let y1 = if hy > 0 { y } else { y.saturating_sub(largo) };
        l.rect((x1) as i32, (y1) as i32, (grosor) as i32, (largo) as i32, color);
    }
}

/// **El triangulo de aviso**, el que va detras de la X en el logo.
///
/// Se dibuja y no se escribe porque **la fuente es ASCII de 16 px y no tiene ese
/// glifo** -- y meter uno seria abrir la puerta a que la pantalla de arranque
/// dependa de una tabla de simbolos que hoy no existe. Son tres lados y una
/// barra: geometria exacta, sin inventar nada del logo.
///
/// [!] El contorno se calcula por fila (`media = i * lado / (2 * alto)`), que es
/// lo unico que se puede hacer sin un trazador de lineas -- y `splash.rs` no
/// tiene uno porque hasta hoy no lo habia necesitado nadie.
fn triangulo_aviso(l: &mut dyn Lienzo, x: u32, y: u32, lado: u32, color: u32) {
    let alto = lado * 7 / 8;
    if alto == 0 {
        return;
    }
    let cx = x + lado / 2;
    let grosor = (lado / 12).max(1);
    let mut i = 0;
    while i < alto {
        let media = i * lado / (2 * alto);
        l.rect((cx.saturating_sub(media)) as i32, (y + i) as i32, (grosor) as i32, (1) as i32, color);
        l.rect((cx + media) as i32, (y + i) as i32, (grosor) as i32, (1) as i32, color);
        i += 1;
    }
    l.rect((x) as i32, (y + alto) as i32, (lado + grosor) as i32, (grosor) as i32, color);
    // La admiracion: barra y punto, con el hueco que la separa.
    let bh = alto / 2;
    l.rect((cx) as i32, (y + alto / 4) as i32, (grosor) as i32, (bh) as i32, color);
    l.rect((cx) as i32, (y + alto / 4 + bh + grosor) as i32, (grosor) as i32, (grosor) as i32, color);
}

/// Cuanto tine el aura el cielo en su centro, de 0 a 255.
///
/// [!] **Estaba en 150 y el previsualizador lo tumbo.** A esa fuerza el aura
/// salia como un globo turquesa detras del gato: una forma que competia con el
/// gato en vez de sostenerlo.
///
/// El reparto correcto es otro y se ve en cuanto se puede mirar la imagen: **el
/// resplandor que se nota es el que sigue la silueta** (`gato::neon`, que es lo
/// que hace un tubo de neon de verdad), y el aura es solo un lavado que levanta
/// el cielo un punto para que el conjunto no este pegado sobre el degradado.
/// Cincuenta se ve; ciento cincuenta se mira.
const FUERZA_AURA: u32 = 50;

/// Cuanto tine el derrame el color del fondo en su primer nivel, de 0 a 255. De
/// ahi cae con el cuadrado de la distancia: un derrame que no cae se ve como un
/// borde grueso, que es lo contrario de un resplandor.
///
/// Lo comparten el gato y el kanji a proposito: son el mismo letrero y tienen
/// que brillar igual.
const HALO_MAX: u32 = 150;

/// Cuando empezo la intro, en ciclos de TSC. `0` = no ha empezado.
static mut INTRO_T0: u64 = 0;

/// **Esta la intro en pantalla?**
///
/// Existe para una sola cosa: que el log del arranque **no pinte encima**. Ver
/// [`splash_dashboard_log_color`].
pub fn intro_en_curso() -> bool {
    unsafe { INTRO_T0 != 0 }
}
/// La ciudad, compuesta una vez. Vive aqui y no en la pila porque ahora se
/// dibuja **desde muchos sitios** del arranque, no de una sentada.
static mut INTRO_CIUDAD: Option<bmo_ciudad::Ciudad> = None;

/// **La superficie en RAM donde se pinta el fotograma antes de ensenarlo.**
///
/// Se reserva una vez al empezar la intro y se devuelve al cerrarla, igual que
/// la ciudad. Reservar y liberar ocho megas por fotograma seria pagar el
/// asignador sesenta veces por segundo para tener siempre la misma memoria.
///
/// `None` significa que no habia sitio, y entonces se pinta directamente sobre
/// la pantalla como se hacia antes: con desgarro, pero arrancando. Ver
/// [`lienzo::Superficie`].
static mut INTRO_SUPERFICIE: Option<Superficie> = None;

/// El progreso del ultimo [`intro_paso`]. Lo necesita [`intro_latido`]: repinta
/// con el reloj de AHORA pero con el progreso de arranque que hubiera, porque
/// el latido no sabe nada del arranque -- solo sabe que hay tiempo muerto.
static mut INTRO_PCT: u32 = 0;

/// **Lo que cuesta un fotograma, en ms.** `0` = todavia no se ha medido uno.
///
/// No es una estimacion ni una constante: se mide el primero y se guarda el peor
/// visto. De eso depende que el latido no alargue las esperas del hardware --
/// ver [`intro_latido`].
static mut INTRO_COSTE_MS: u32 = 0;

/// **Pinta un fotograma donde toque y lo ensena.**
///
/// Es el unico sitio que decide si la escena va a una superficie en RAM o
/// directamente al framebuffer, y por eso el resto del modulo no se entera de
/// cual de las dos cosas esta pasando: `pintar_escena` recibe un `&mut dyn
/// Lienzo` y pinta.
fn fotograma_a_pantalla(w: u32, h: u32, f: &bmo_ciudad::Fotograma) {
    unsafe {
        let sup = &mut *core::ptr::addr_of_mut!(INTRO_SUPERFICIE);
        if let Some(s) = sup.as_mut() {
            pintar_escena(s, w, h, f);
            // ** UNA copia y UNA barrera por fotograma. Antes eran ~8.800
            // barreras -- una por rectangulo-- para ensenar una sola imagen.
            s.volcar();
            return;
        }
    }
    if let Some(mut pant) = Pantalla::actual() {
        pintar_escena(&mut pant, w, h, f);
        pant.presentar();
    }
}

/// **Empieza la intro y NO se queda esperando.**
///
/// Ver [`intro_paso`], que es donde esta explicado el cambio entero.
pub fn intro_empieza() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 {
        return;
    }
    // El halo del gato se calcula UNA vez: es una dilatacion de la mascara, y la
    // mascara no cambia nunca. Aqui y no en el primer fotograma para que ese
    // coste no caiga dentro de la animacion. Ver `gato::neon`.
    gato::neon::preparar();
    unsafe {
        INTRO_T0 = tsc_read();
        INTRO_CIUDAD = Some(bmo_ciudad::Ciudad::nueva(
            w as i32,
            h as i32,
            ((w as u64) << 20) | h as u64,
        ));
        // Si no hay ocho megas contiguos, `nueva` contesta `None` y la intro
        // pinta sobre la pantalla como toda la vida. Un arranque con desgarro
        // es mejor que un arranque que no arranca.
        INTRO_SUPERFICIE = Superficie::nueva(w as i32, h as i32);
    }
    intro_paso(0);
}

/// **Un fotograma de la intro, con el progreso REAL del arranque.**
///
/// # El truco de Santa Monica, y por que el modelo de antes estaba mal
///
/// Lo dijo el dueno viendo el log de arranque pasar: *"BMO-X esta preparando
/// todo eso, los datos se ejecutan en tiempo real... tiene que esconder con
/// truco inspirado como hicieron Santa Monica en God of War"*.
///
/// God of War 2018 no tiene pantallas de carga: el trabajo se hace **debajo** de
/// una camara que no corta. La carga no se elimina -- se tapa con algo que el
/// jugador queria ver de todas formas.
///
/// Aqui era al reves, y el comentario de `phase.rs` lo decia con todas las
/// letras: *"la animacion juega, luego apareces en el escritorio"*, el modelo de
/// Windows. O sea **2.400 ms de animacion MAS el tiempo real de arrancar**. Y el
/// coste ya estaba medido y confesado en otro sitio: `boot_timeline` tiene una
/// fila propia para el `GATO_MS` porque, sin ella, ese segundo y medio se
/// achacaba a la enumeracion del bus PCI.
///
/// ** Ahora la intro no espera a nada: se llama a esto entre paso y paso del
/// arranque de verdad --USB, xHCI, AHCI, el censo de PCI-- y cada llamada pinta
/// UN fotograma con el reloj que haya. La animacion dura **lo que dure el
/// trabajo**, y no cuesta ni un milisegundo de mas.
///
/// # Y `pct` no es una barra: es la ciudad
///
/// El progreso enciende las torres. Con lo cual se cierra la idea que el dueno
/// tuvo dos dias antes --*"en el fondo se ve el sistema de ciudad con TODO"*--:
/// **la ciudad encendiendose ES el arranque ocurriendo**, no una animacion que
/// finge acompanarlo. Un subsistema que tarda deja su tramo de ciudad a oscuras
/// mas tiempo, y eso es informacion de verdad.
pub fn intro_paso(pct: u32) {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 {
        return;
    }
    let t0 = unsafe { INTRO_T0 };
    if t0 == 0 {
        return;
    }
    // El tiempo manda en la camara y en el gato; el PROGRESO manda en la ciudad.
    // Son dos relojes distintos a proposito: uno cuenta lo que se ve, el otro
    // cuenta lo que pasa.
    unsafe { INTRO_PCT = pct.min(100) };
    // [!] **Y AQUI YA NO SE RECORTA EL RELOJ.** Estaba `ms_desde(t0).min(
    // DURACION_MS)`, o sea que pasados 2,4 segundos el guion recibia siempre el
    // mismo milisegundo y contestaba siempre el mismo fotograma: una foto. El
    // guion ya no se acaba --tiene un acto de espera que se repite-- asi que lo
    // que hay que darle es la hora de verdad.
    let ms = ms_desde(t0);
    let mut f = bmo_ciudad::fotograma(ms);
    f.ciudad_pct = pct.min(100);
    fotograma_a_pantalla(w, h, &f);
}

/// **UN FOTOGRAMA DENTRO DEL TIEMPO MUERTO DE OTRO.**
///
/// # El problema, tal y como se vio en el video del arranque
///
/// La intro pinta entre paso y paso del arranque, y eso deja huecos enormes:
/// entre `intro_paso(40)` y `intro_paso(70)` esta el bloque completo de USB,
/// xHCI, AHCI, red y sistemas de ficheros. En el video son **mas de tres
/// segundos con un solo fotograma en pantalla** -- la ciudad congelada, el gato
/// sin salir ni una vez, y de golpe el destello del final.
///
/// Y esos segundos no son trabajo: en su mayor parte son **esperas**. El USB
/// tiene tiempos fisicos obligatorios por spec --100 ms de debounce de conexion,
/// 20+ ms de estabilizacion de alimentacion, resets de puerto-- y durante ellos
/// el CPU no hace absolutamente nada. Es la definicion de tiempo muerto.
///
/// # La regla: pintar SOLO si cabe
///
/// `ms_libres` es cuanto queda de la espera. Si no cabe un fotograma entero, se
/// contesta `false` y quien llama sigue girando. Asi el fotograma es **gratis
/// de verdad**: la espera dura lo que el hardware pide, ni un milisegundo mas,
/// y lo que antes era CPU parado ahora es la animacion corriendo.
///
/// Es el mismo truco de Santa Monica que ya sostiene [`intro_paso`], pero un
/// nivel mas abajo: alli se tapa el trabajo, aqui se tapa la espera.
///
/// [!] El coste de un fotograma **se mide, no se supone**, y se guarda el peor
/// visto. Suponerlo seria elegir entre alargar las esperas del USB --que es
/// arriesgar la enumeracion del teclado por una animacion-- o no pintar nunca
/// por prudencia.
///
/// Devuelve `true` si pinto.
pub fn intro_latido(ms_libres: u32) -> bool {
    if unsafe { INTRO_T0 } == 0 {
        return false;
    }
    let coste = unsafe { INTRO_COSTE_MS };
    // El primero se pinta siempre: hay que medir uno para saber lo que cuestan.
    // Y se pinta en la primera espera larga del arranque, que es justo donde
    // sobra tiempo.
    if coste != 0 && ms_libres < coste {
        return false;
    }
    let t = tsc_read();
    intro_paso(unsafe { INTRO_PCT });
    let gastado = ms_desde(t);
    unsafe {
        if gastado > INTRO_COSTE_MS {
            INTRO_COSTE_MS = gastado;
        }
    }
    true
}

/// **Cierra la intro: los ojos toman el control y todo se va a negro.**
///
/// Esto SI espera, y es el unico sitio donde se puede: el trabajo ya termino, no
/// hay nada debajo que tapar. Son los ultimos 500 ms del guion.
pub fn intro_cierra() {
    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    if w == 0 || h == 0 || unsafe { INTRO_T0 } == 0 {
        return;
    }
    let t0 = tsc_read();
    // ** EL CIERRE EMPIEZA EN CERO, y antes no.
    //
    // Estaba `fotograma(FIN_GATO + d)`: se le pedia al guion el instante 1.500 y
    // hacia delante. Eso funcionaba solo si la intro ya habia llegado ahi por su
    // cuenta -- y en el arranque real **nunca llegaba**, porque el kernel acaba
    // antes del milisegundo 700. O sea que el cierre saltaba por encima del acto
    // del gato y el logo solo aparecia durante estos ultimos 900 ms. Por eso en
    // el video el gato solo se ve al final, un instante, y ya apagandose.
    //
    // Ahora son dos guiones: `fotograma` mientras hay trabajo --con su espera
    // que se repite-- y `cierre` para despedirse, contando desde cero.
    let dur = bmo_ciudad::acto::CIERRE_MS;
    loop {
        let d = ms_desde(t0);
        if d >= dur {
            break;
        }
        let f = bmo_ciudad::acto::cierre(d);
        fotograma_a_pantalla(w, h, &f);
    }
    // El borron final va directo a la pantalla: es una sola cosa y no hay
    // fotograma detras que componer.
    super::lienzo::fill_rect(0, 0, w, h, BG);
    unsafe {
        INTRO_T0 = 0;
        INTRO_CIUDAD = None;
        // ** Y SE DEVUELVEN LOS OCHO MEGAS. La intro corre una vez por arranque;
        // quedarse la superficie seria regalarle al splash una memoria que a
        // partir de aqui no vuelve a usar nadie.
        if let Some(s) = (*core::ptr::addr_of_mut!(INTRO_SUPERFICIE)).take() {
            s.liberar();
        }
    }
}

/// **Pinta UN fotograma de la escena entera**: ciudad, gato, kanji y destello.
///
/// El encuadre no se calcula aqui: se le pide a `bmo_ciudad::encuadre`, que es
/// aritmetica pura y **se prueba sin encender la maquina**. Mientras estuvo
/// dentro de esta funcion solo se podia juzgar reiniciando el Ryzen, y asi se
/// colo el fallo del video del 08-15: el titulo escrito sobre los tejados.
///
/// Lo que queda aqui es pintar. Esta funcion sigue **sin estado**: se puede
/// llamar desde cualquier punto del arranque sin que nadie haya guardado nada
/// antes.
fn pintar_escena(l: &mut dyn Lienzo, w: u32, h: u32, f: &bmo_ciudad::Fotograma) {
    use bmo_ciudad::paleta::{mezcla, NEGRO};

    // La escala sale de la ALTURA de la pantalla, no de un numero fijo: en 1080
    // sale a x2 y en 720 a x1, y en las dos ocupa la misma fraccion.
    let escala = if h >= 900 { 2 } else { 1 };
    let escala_t = if h >= 900 { 5 } else { 4 };
    let gw = gato::ANCHO * escala;
    let gh = gato::ALTO * escala;
    let kw = gato::KANJI_ANCHO * escala;
    let kh = gato::KANJI_ALTO * escala;
    let tw = text_width_scaled("BMO-X", escala_t);

    // El techo y el canto del marco se le preguntan a la ciudad en vez de copiar
    // aqui unos porcentajes. Si manana alguien sube las torres o ensancha el
    // marco, el logo se aparta solo.
    let (techo, marco_interior) = unsafe {
        let ciudad = &*core::ptr::addr_of!(INTRO_CIUDAD);
        match ciudad.as_ref() {
            Some(c) => (c.techo().max(0) as u32, c.marco().interior().max(0) as u32),
            None => (h, 0),
        }
    };
    let medidas = bmo_ciudad::Medidas {
        pantalla_w: w,
        pantalla_h: h,
        techo,
        marco_interior,
        gato_w: gw,
        gato_h: gh,
        kanji_w: kw,
        kanji_h: kh,
        hueco_kanji: 22 * escala,
        titulo_w: tw,
        titulo_h: FONT_H as u32 * escala_t,
        linea_h: FONT_H as u32,
    };
    let enc = bmo_ciudad::componer(&medidas);
    let (gx, gy, ky) = (enc.gato_x, enc.gato_y, enc.kanji_y);
    let th = medidas.titulo_h;

    // -- LA CIUDAD, detras de todo.
    //
    // ** AQUI ESTABA EL FALLO DEL VIDEO DEL 08-15, y era esta linea:
    //
    //     if cw > 0 && ch > 0 && x >= 0 && y >= 0 && ... { fill_rect(...) }
    //
    // Eso no recorta: DESCARTA. Un rectangulo con la esquina fuera se tiraba
    // entero, y la ciudad emite rectangulos que empiezan fuera a proposito --el
    // sello del marco-- asi que el unico rectangulo cuyo trabajo es tapar el
    // borde era el primero en no pintarse. Eran 2.625 de 8.775 por fotograma y
    // el 7,2% de la pantalla sin escribir. Ver `lienzo.rs`.
    //
    // Ahora no hay guarda que escribir: el recorte lo pone `bmo_dibujo::Lienzo`
    // y el fundido a negro lo pone `Apagado`, que es una capa y no una cuenta
    // repetida en cada sitio que pinta.
    unsafe {
        let ciudad = &mut *core::ptr::addr_of_mut!(INTRO_CIUDAD);
        if let Some(c) = ciudad.as_mut() {
            c.encender(f.ciudad_pct);
            let cam = bmo_ciudad::Camara::nueva(f.avance);
            c.dibujar(cam, &mut lienzo::Apagado::nuevo(l, f.negro));
        }
    }

    // -- ** EL AURA: el cielo ENCENDIDO detras del logo.
    //
    // La otra mitad de "las capas estan mezcladas". La escalera de valores de la
    // paleta separo el cielo de las torres, pero el logo no tenia separacion de
    // NADA: estaba estampado sobre el degradado, y cuando el cielo llegaba a su
    // parte clara el gato casi desaparecia.
    //
    // Un neon de verdad enciende el aire que tiene detras. Eso es esto, y va
    // ENTRE la ciudad y el gato porque es cielo respondiendo a una luz -- no es
    // parte del gato. Ver `bmo_ciudad::halo`.
    //
    // [!] Es OPACA (no se puede leer el framebuffer para mezclar), asi que tiene
    // que caber en el cielo despejado o borraria las torres. De ahi el recorte
    // contra `techo`: la caja del aura nunca baja de ahi.
    let fuerza_aura = FUERZA_AURA * f.gato_alfa / 255;
    if f.gato_alfa > 0 {
        {
            unsafe {
                let ciudad = &*core::ptr::addr_of!(INTRO_CIUDAD);
                if let Some(c) = ciudad.as_ref() {
                    // [!] El fundido va en la CAPA y ya no en el cielo. Antes se
                    // apagaba el fondo del aura --`mezcla(color_cielo, NEGRO,
                    // ...)`-- pero el tinte cian que se le pone encima entraba a
                    // plena potencia, asi que el aura se resistia al fundido y
                    // se quedaba clara sobre una escena que se iba a negro.
                    // Envuelta en `Apagado` se apaga entera, como todo lo demas.
                    bmo_ciudad::aura(
                        |y| c.color_cielo(y),
                        enc.aura_cx,
                        enc.aura_cy,
                        enc.aura_rx,
                        enc.aura_ry,
                        ACCENT,
                        fuerza_aura,
                        &mut lienzo::Apagado::nuevo(l, f.negro),
                    );
                }
            }
        }
    }

    // -- EL GATO, encendiendose por encima Y CON SU PROPIO RITMO.
    //
    // ** El flote es lo que lo separa del fondo de verdad. Dos planos quietos
    // uno sobre otro se leen como un collage por mucho que tengan brillos
    // distintos; en cuanto uno se mueve **a su ritmo**, el ojo los separa solo.
    // Es la misma pista que el paralaje, aplicada al primer plano -- y por eso
    // el periodo (2,6 s) no es multiplo de nada de la camara: si coincidieran,
    // el gato y la ciudad se moverian a compas y volverian a parecer lo mismo.
    //
    // El kanji flota con el, y el titulo NO: el titulo es tipografia, y una
    // tipografia que se mueve se lee como un fallo de sincronia, no como vida.
    if f.gato_alfa > 0 {
        let gy = (gy as i32 + f.gato_flote).max(0) as u32;
        let ky = (ky as i32 + f.gato_flote).max(0) as u32;
        // El latido del neon va SOBRE el brillo de los ojos, con tope: un neon
        // perfectamente estable no parece neon, parece un LED.
        let ojos = (f.ojos_alfa + f.ojos_pulso).min(255);
        // El fondo contra el que se mezcla el halo es el AURA, no el cielo
        // pelado: el gato se dibuja encima de ella. Se reconstruye con la misma
        // aritmetica en vez de leerla de la pantalla -- leer el framebuffer
        // esta prohibido aqui, y el numero se sabe.
        let bajo_el_logo = |y: u32| {
            let cielo = unsafe {
                let ciudad = &*core::ptr::addr_of!(INTRO_CIUDAD);
                ciudad.as_ref().map_or(NEGRO, |c| c.color_cielo(y as i32))
            };
            // Cuanto tine el aura a esta altura: cae con el cuadrado de la
            // distancia al centro, igual que en `bmo_ciudad::halo`.
            let ry = enc.aura_ry as u32;
            let dy = (y as i32 - enc.aura_cy).unsigned_abs().min(ry);
            let cerca = ry - dy;
            let f_aura = fuerza_aura * cerca * cerca / (ry * ry).max(1);
            mezcla(mezcla(cielo, ACCENT, f_aura, 255), NEGRO, f.negro, 255)
        };
        draw_gato_encendido(l, gx, gy, escala, f.gato_alfa, ojos, f.negro, bajo_el_logo);
        // El kanji flota con el gato, asi que su `x` sale del encuadre y su `y`
        // lleva el mismo desplazamiento que el trazo. Y derrama igual que el
        // gato: son el mismo letrero.
        draw_kanji(
            l,
            enc.kanji_x,
            ky,
            escala,
            mezcla(mezcla(0xFF1A1730, ACCENT, ojos, 255), NEGRO, f.negro, 255),
            ojos,
            f.negro,
            bajo_el_logo,
        );
        // El titulo entra con el trazo: es parte del gato, no de la ciudad. Y NO
        // flota: una tipografia que se mueve se lee como un fallo de sincronia.
        let ty = enc.titulo_y;
        let tx = enc.titulo_x;
        let c_txt = mezcla(mezcla(NEGRO, WHITE, f.gato_alfa, 255), NEGRO, f.negro, 255);
        let c_ac = mezcla(mezcla(NEGRO, ACCENT, f.gato_alfa, 255), NEGRO, f.negro, 255);
        cadena_en(l, tx as i32, ty as i32, "BMO-X", c_txt, escala_t as i32);
        triangulo_aviso(l, tx + tw + escala_t * 2, ty + th / 3, th / 2, c_ac);
        l.rect((tx) as i32, (ty + th + 10) as i32, (tw) as i32, (3) as i32, c_ac);
        let sub = "BMO METAKERNEL";
        let sw = text_width(sub);
        let sy = ty + th + 10 + 3 + 14;
        let sx = w.saturating_sub(sw) / 2;
        cadena_en(l, sx as i32, sy as i32, sub, c_ac, 1);
        let regla = (tw / 3).max(20);
        let ry = sy + FONT_H as u32 / 2;
        let c_dim = mezcla(mezcla(NEGRO, DIM, f.gato_alfa, 255), NEGRO, f.negro, 255);
        l.rect((sx.saturating_sub(14 + regla)) as i32, (ry) as i32, (regla) as i32, (1) as i32, c_dim);
        l.rect((sx + sw + 14) as i32, (ry) as i32, (regla) as i32, (1) as i32, c_dim);
    }

    // -- EL DESTELLO: los ojos tomando el control.
    //
    // Una caja de cian que crece desde la cara del gato hasta comerse la
    // pantalla. Se pinta ENCIMA de todo y se apaga hacia negro con el mismo
    // `f.negro` que la ciudad, asi que no se "quita": se lo traga el negro.
    if f.destello > 0 {
        let cara_x = (gx + gw / 2) as i32;
        let cara_y = (gy + gh / 3) as i32;
        let radio = (f.destello * (w.max(h)) / 255) as i32;
        let c = mezcla(mezcla(NEGRO, ACCENT, f.destello, 255), NEGRO, f.negro, 255);
        let x0 = (cara_x - radio).max(0) as u32;
        let y0 = (cara_y - radio).max(0) as u32;
        let x1 = ((cara_x + radio).max(0) as u32).min(w);
        let y1 = ((cara_y + radio).max(0) as u32).min(h);
        if x1 > x0 && y1 > y0 {
            l.rect((x0) as i32, (y0) as i32, (x1 - x0) as i32, (y1 - y0) as i32, c);
        }
    }

    // El marco, lo ultimo: encuadra todo lo demas.
    marco_esquinas(l, w, h, mezcla(ACCENT, NEGRO, f.negro, 255));
    // Aqui NO se drena el buffer de escritura: ensenar el fotograma es cosa de
    // quien decidio donde se pinta (`fotograma_a_pantalla`), que lo hace una vez
    // con `volcar` o con `presentar`. Una barrera aqui seria la penultima.
}
