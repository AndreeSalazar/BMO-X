//! **QUE se dibuja.** Una lista de trazos, en orden de pintado, sin saber a
//! donde van.
//!
//! ## Por que esto se saco de `rust.rs`
//!
//! El emisor mezclaba dos preguntas en cada funcion: *que hay que dibujar* y
//! *como se escribe eso en Rust*. Mientras hubo un solo destino no se notaba.
//! Con tres --Rust, el recurso BEF, y el reflejo en PPM-- cada uno habria vuelto
//! a deducir lo primero, y **tres deducciones de la misma cosa son tres sitios
//! donde puede salir distinta**.
//!
//! Ahora la lista se calcula UNA vez y los destinos la escriben. `rust.rs` no
//! decide nada: traduce.
//!
//! ## * Y es lo que hace diagnosticable un fotograma
//!
//! Un recorte que falla no da un error: da basura en pantalla, o un trozo que no
//! se repinta. Con la lista fuera, la pregunta *"por que este fotograma salio
//! mal"* deja de ser una lectura del generado y pasa a ser **filtrar una lista y
//! mirarla** -- que es lo que hace `recorte::dentro`, y lo que prueban sus
//! pruebas sin arrancar nada.

use bmo_maqueta_layout::{Frame, Laid, Rect};

/// Un trazo: lo unico que BMO-X sabe hacer, dicho en dos formas.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Trazo {
    /// Un rectangulo macizo.
    Rect { r: Rect, color: u32 },
    /// Letras. `r` es donde empiezan y cuanto ocupan.
    Texto { r: Rect, texto: String, color: u32 },
}

impl Trazo {
    /// El area que toca. Es lo unico que hace falta para recortar.
    pub fn area(&self) -> Rect {
        match self {
            Trazo::Rect { r, .. } | Trazo::Texto { r, .. } => *r,
        }
    }
}

/// Cuando se dibuja este trazo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Estado {
    /// Siempre.
    Reposo,
    /// Solo mientras el puntero esta encima de la caja.
    Encima,
}

/// Un trazo, de quien es, y cuando toca.
///
/// `de` no es decoracion: va al comentario del codigo generado y es por donde se
/// sigue un fotograma raro hasta la caja que lo causo.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Orden {
    pub trazo: Trazo,
    pub de: String,
    pub estado: Estado,
}

/// La lista entera, en orden de pintado -- que es el orden del fichero.
pub fn lista(l: &Laid) -> Vec<Orden> {
    let mut out = Vec::new();
    for f in l.all() {
        let de = nombre_de(f);
        trazos_de(f, Estado::Reposo, &de, &mut out);
        trazos_de(f, Estado::Encima, &de, &mut out);
    }
    out
}

/// Los trazos de una caja en un estado. Devuelve nada si en ese estado no
/// cambia -- una caja sin `:hover` no aporta ni una orden a `Encima`.
fn trazos_de(f: &Frame, estado: Estado, de: &str, out: &mut Vec<Orden>) {
    let s = match estado {
        Estado::Reposo => f.style,
        Estado::Encima => match f.hover {
            Some(h) => h,
            None => return,
        },
    };
    // Sin `id` no hay forma de pedir el realce de esta caja, asi que sus
    // ordenes de `Encima` no las podria disparar nadie.
    if estado == Estado::Encima && f.id.is_none() {
        return;
    }

    let mut push = |trazo| {
        out.push(Orden {
            trazo,
            de: de.to_string(),
            estado,
        })
    };

    // El borde primero y el fondo encima: dos rects concentricos, que es como
    // lo escribe `calc.rs` a mano. Cuatro tiras darian el mismo dibujo y cuatro
    // veces mas ordenes que recortar.
    if let Some(color) = s.border_color {
        if s.border_width > 0 {
            push(Trazo::Rect { r: f.rect, color });
        }
    }
    if let Some(color) = s.background {
        let d = s.border_width;
        push(Trazo::Rect {
            r: Rect {
                x: f.rect.x + d as i32,
                y: f.rect.y + d as i32,
                w: f.rect.w.saturating_sub(d * 2),
                h: f.rect.h.saturating_sub(d * 2),
            },
            color,
        });
    }
    if let (Some(t), Some(r), Some(color)) = (&f.text, f.text_at, s.color) {
        push(Trazo::Texto {
            r,
            texto: t.clone(),
            color,
        });
    }
}

/// **Una region que se puede pulsar**, y como se llama.
///
/// # Por que vive aqui y no en el emisor que la escribe
///
/// Por lo mismo que `lista`: es una respuesta a *"que hay"*, no a *"como se
/// escribe"*. Si cada destino dedujera por su cuenta que cajas son pulsables,
/// serian **tres deducciones de la misma cosa** -- y el dia que una se
/// desviara, el boton estaria en un sitio en el codigo generado y en otro en el
/// recurso, con el mismo `.maqueta` de origen.
///
/// [!] Y OJO CON EL NOMBRE: esto **no** es `bmo-golpe`, que es la RESTA --
/// convertir un clic de pantalla en un clic de app-- y vive en Ring 3. Esto es
/// *donde* se puede pulsar y *como se llama*, decidido en el anfitrion. Dos
/// preguntas, dos sitios, y la palabra se repite porque el plan la usa asi.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Golpe {
    pub r: Rect,
    /// El `#id` de la caja, con la almohadilla incluida.
    pub nombre: String,
}

/// Las regiones pulsables: **las cajas que tienen `#id`, y solo esas**.
///
/// * El criterio no es un invento de aqui: es el mismo que usa `trazos_de` para
/// decidir si una caja aporta ordenes de `Encima`. Una caja sin `id` no puede
/// recibir su realce **porque nadie puede nombrarla para pedirlo**, y por la
/// misma razon nadie puede recibir su pulsacion.
///
/// > Si las dos reglas se separaran, habria cajas que se iluminan al pasar por
/// > encima y no hacen nada al pulsarlas.
pub fn golpes(l: &Laid) -> Vec<Golpe> {
    let mut out = Vec::new();
    for f in l.all() {
        if let Some(id) = &f.id {
            out.push(Golpe {
                r: f.rect,
                nombre: format!("#{id}"),
            });
        }
    }
    out
}

/// Como se llama una caja: su `id`, su isla, o su etiqueta.
pub fn nombre_de(f: &Frame) -> String {
    if let Some(id) = &f.id {
        return format!("#{id}");
    }
    if let Some(n) = &f.island {
        return format!("isla {n}");
    }
    f.tag.name().to_string()
}
