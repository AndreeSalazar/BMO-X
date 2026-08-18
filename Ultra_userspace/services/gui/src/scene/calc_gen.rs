//! GENERADO POR MAQUETA DESDE `toolchain/tools/maqueta/pruebas/calc.maqueta` -- NO EDITAR A MANO.
//!
//! Lo que se edita es el `.maqueta`. Cambiar esto es escribir una verdad
//! que la siguiente compilacion borra.
//!
//! Todas las coordenadas son relativas al origen que se pase, asi que
//! este modulo no sabe donde esta la ventana -- igual que no lo sabia el
//! `.maqueta`.

#![allow(clippy::identity_op, clippy::erasing_op)]
// [!] `dead_code` aparte, y con motivo: este modulo ofrece la superficie
// ENTERA de la maquetacion --pintar, recortar, realzar, golpear, las
// islas-- y cual de esas usa la app es cosa de la app. Recortar lo que
// hoy no se llama obligaria a regenerar el dia que alguien lo use.
#![allow(dead_code)]

use bmo_userland as bmo;

/// El tamano que MAQUETA dedujo del arbol. Nadie lo escribio.
pub const ANCHO: u32 = 322;
pub const ALTO: u32 = 446;

/// Pinta la maquetacion entera con su esquina superior izquierda en
/// `(ox, oy)`. El orden es el del fichero, que ES el orden de pintado.
pub fn pintar(p: &bmo::Pantalla, ox: u32, oy: u32) {
    // div
    p.rect(ox + 0, oy + 0, 322, 446, 0x00333D52);
    p.rect(ox + 2, oy + 2, 318, 442, 0x00182434);
    // isla visor
    p.rect(ox + 8, oy + 8, 306, 40, 0x00161C28);
    // #k_c
    p.rect(ox + 8, oy + 54, 72, 72, 0x002B3B52);
    p.texto(ox + 40, oy + 82, "C", 0x00E6EDF6);
    // #k_div
    p.rect(ox + 86, oy + 54, 72, 72, 0x003A5878);
    p.texto(ox + 118, oy + 82, "/", 0x00E6EDF6);
    // #k_mul
    p.rect(ox + 164, oy + 54, 72, 72, 0x003A5878);
    p.texto(ox + 196, oy + 82, "*", 0x00E6EDF6);
    // #k_sub
    p.rect(ox + 242, oy + 54, 72, 72, 0x003A5878);
    p.texto(ox + 274, oy + 82, "-", 0x00E6EDF6);
    // #k_7
    p.rect(ox + 8, oy + 132, 72, 72, 0x002B3B52);
    p.texto(ox + 40, oy + 160, "7", 0x00E6EDF6);
    // #k_8
    p.rect(ox + 86, oy + 132, 72, 72, 0x002B3B52);
    p.texto(ox + 118, oy + 160, "8", 0x00E6EDF6);
    // #k_9
    p.rect(ox + 164, oy + 132, 72, 72, 0x002B3B52);
    p.texto(ox + 196, oy + 160, "9", 0x00E6EDF6);
    // #k_add
    p.rect(ox + 242, oy + 132, 72, 72, 0x003A5878);
    p.texto(ox + 274, oy + 160, "+", 0x00E6EDF6);
    // #k_4
    p.rect(ox + 8, oy + 210, 72, 72, 0x002B3B52);
    p.texto(ox + 40, oy + 238, "4", 0x00E6EDF6);
    // #k_5
    p.rect(ox + 86, oy + 210, 72, 72, 0x002B3B52);
    p.texto(ox + 118, oy + 238, "5", 0x00E6EDF6);
    // #k_6
    p.rect(ox + 164, oy + 210, 72, 72, 0x002B3B52);
    p.texto(ox + 196, oy + 238, "6", 0x00E6EDF6);
    // #k_1
    p.rect(ox + 8, oy + 288, 72, 72, 0x002B3B52);
    p.texto(ox + 40, oy + 316, "1", 0x00E6EDF6);
    // #k_2
    p.rect(ox + 86, oy + 288, 72, 72, 0x002B3B52);
    p.texto(ox + 118, oy + 316, "2", 0x00E6EDF6);
    // #k_3
    p.rect(ox + 164, oy + 288, 72, 72, 0x002B3B52);
    p.texto(ox + 196, oy + 316, "3", 0x00E6EDF6);
    // #k_eq
    p.rect(ox + 242, oy + 288, 72, 72, 0x004C9BE8);
    p.texto(ox + 274, oy + 316, "=", 0x00E6EDF6);
    // #k_0
    p.rect(ox + 8, oy + 366, 72, 72, 0x002B3B52);
    p.texto(ox + 40, oy + 394, "0", 0x00E6EDF6);
    // #k_dot
    p.rect(ox + 86, oy + 366, 72, 72, 0x002B3B52);
    p.texto(ox + 118, oy + 394, ".", 0x00E6EDF6);
}

/// Repinta SOLO lo que cae dentro de `(cx, cy, cw, ch)`, en coordenadas
/// de pantalla. Para devolver el fondo de un area sin repintarlo todo.
///
/// ** Por que existe, con el numero: devolver fondo preguntando el color
/// PIXEL A PIXEL cuesta ~325.000 escrituras por borrado, que a los
/// ~300 MB/s medidos en el Ryzen son 4,33 ms -- la cuarta parte de un
/// fotograma de 60 Hz, y arrastrar hace uno por evento de raton. Esto son
/// unas pocas llamadas a `rect`, que escriben por filas.
///
/// Los rectangulos se RECORTAN; el texto entra entero o no entra, porque
/// medio glifo no se puede pintar.
pub fn pintar_en(p: &bmo::Pantalla, ox: u32, oy: u32, cx: u32, cy: u32, cw: u32, ch: u32) {
    // div
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 0, oy + 0, 322, 446) {
        p.rect(x, y, w, h, 0x00333D52);
    }
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 2, oy + 2, 318, 442) {
        p.rect(x, y, w, h, 0x00182434);
    }
    // isla visor
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 8, oy + 8, 306, 40) {
        p.rect(x, y, w, h, 0x00161C28);
    }
    // #k_c
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 8, oy + 54, 72, 72) {
        p.rect(x, y, w, h, 0x002B3B52);
    }
    if cruza(cx, cy, cw, ch, ox + 40, oy + 82, 8, 16) {
        p.texto(ox + 40, oy + 82, "C", 0x00E6EDF6);
    }
    // #k_div
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 86, oy + 54, 72, 72) {
        p.rect(x, y, w, h, 0x003A5878);
    }
    if cruza(cx, cy, cw, ch, ox + 118, oy + 82, 8, 16) {
        p.texto(ox + 118, oy + 82, "/", 0x00E6EDF6);
    }
    // #k_mul
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 164, oy + 54, 72, 72) {
        p.rect(x, y, w, h, 0x003A5878);
    }
    if cruza(cx, cy, cw, ch, ox + 196, oy + 82, 8, 16) {
        p.texto(ox + 196, oy + 82, "*", 0x00E6EDF6);
    }
    // #k_sub
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 242, oy + 54, 72, 72) {
        p.rect(x, y, w, h, 0x003A5878);
    }
    if cruza(cx, cy, cw, ch, ox + 274, oy + 82, 8, 16) {
        p.texto(ox + 274, oy + 82, "-", 0x00E6EDF6);
    }
    // #k_7
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 8, oy + 132, 72, 72) {
        p.rect(x, y, w, h, 0x002B3B52);
    }
    if cruza(cx, cy, cw, ch, ox + 40, oy + 160, 8, 16) {
        p.texto(ox + 40, oy + 160, "7", 0x00E6EDF6);
    }
    // #k_8
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 86, oy + 132, 72, 72) {
        p.rect(x, y, w, h, 0x002B3B52);
    }
    if cruza(cx, cy, cw, ch, ox + 118, oy + 160, 8, 16) {
        p.texto(ox + 118, oy + 160, "8", 0x00E6EDF6);
    }
    // #k_9
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 164, oy + 132, 72, 72) {
        p.rect(x, y, w, h, 0x002B3B52);
    }
    if cruza(cx, cy, cw, ch, ox + 196, oy + 160, 8, 16) {
        p.texto(ox + 196, oy + 160, "9", 0x00E6EDF6);
    }
    // #k_add
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 242, oy + 132, 72, 72) {
        p.rect(x, y, w, h, 0x003A5878);
    }
    if cruza(cx, cy, cw, ch, ox + 274, oy + 160, 8, 16) {
        p.texto(ox + 274, oy + 160, "+", 0x00E6EDF6);
    }
    // #k_4
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 8, oy + 210, 72, 72) {
        p.rect(x, y, w, h, 0x002B3B52);
    }
    if cruza(cx, cy, cw, ch, ox + 40, oy + 238, 8, 16) {
        p.texto(ox + 40, oy + 238, "4", 0x00E6EDF6);
    }
    // #k_5
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 86, oy + 210, 72, 72) {
        p.rect(x, y, w, h, 0x002B3B52);
    }
    if cruza(cx, cy, cw, ch, ox + 118, oy + 238, 8, 16) {
        p.texto(ox + 118, oy + 238, "5", 0x00E6EDF6);
    }
    // #k_6
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 164, oy + 210, 72, 72) {
        p.rect(x, y, w, h, 0x002B3B52);
    }
    if cruza(cx, cy, cw, ch, ox + 196, oy + 238, 8, 16) {
        p.texto(ox + 196, oy + 238, "6", 0x00E6EDF6);
    }
    // #k_1
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 8, oy + 288, 72, 72) {
        p.rect(x, y, w, h, 0x002B3B52);
    }
    if cruza(cx, cy, cw, ch, ox + 40, oy + 316, 8, 16) {
        p.texto(ox + 40, oy + 316, "1", 0x00E6EDF6);
    }
    // #k_2
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 86, oy + 288, 72, 72) {
        p.rect(x, y, w, h, 0x002B3B52);
    }
    if cruza(cx, cy, cw, ch, ox + 118, oy + 316, 8, 16) {
        p.texto(ox + 118, oy + 316, "2", 0x00E6EDF6);
    }
    // #k_3
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 164, oy + 288, 72, 72) {
        p.rect(x, y, w, h, 0x002B3B52);
    }
    if cruza(cx, cy, cw, ch, ox + 196, oy + 316, 8, 16) {
        p.texto(ox + 196, oy + 316, "3", 0x00E6EDF6);
    }
    // #k_eq
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 242, oy + 288, 72, 72) {
        p.rect(x, y, w, h, 0x004C9BE8);
    }
    if cruza(cx, cy, cw, ch, ox + 274, oy + 316, 8, 16) {
        p.texto(ox + 274, oy + 316, "=", 0x00E6EDF6);
    }
    // #k_0
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 8, oy + 366, 72, 72) {
        p.rect(x, y, w, h, 0x002B3B52);
    }
    if cruza(cx, cy, cw, ch, ox + 40, oy + 394, 8, 16) {
        p.texto(ox + 40, oy + 394, "0", 0x00E6EDF6);
    }
    // #k_dot
    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 86, oy + 366, 72, 72) {
        p.rect(x, y, w, h, 0x002B3B52);
    }
    if cruza(cx, cy, cw, ch, ox + 118, oy + 394, 8, 16) {
        p.texto(ox + 118, oy + 394, ".", 0x00E6EDF6);
    }
}

/// La parte de un rectangulo que cae dentro del limite. `None` si no se
/// tocan -- y tocarse por el borde NO es tocarse: `[x0, x1)`, medio
/// abierto, la misma regla que el recorte de `bmo-dibujo`. Si el borde
/// contara, cada reparacion repintaria una fila de mas y se veria como
/// una costura.
fn corte(cx: u32, cy: u32, cw: u32, ch: u32, x: u32, y: u32, w: u32, h: u32)
    -> Option<(u32, u32, u32, u32)>
{
    let x0 = if x > cx { x } else { cx };
    let y0 = if y > cy { y } else { cy };
    let x1 = if x + w < cx + cw { x + w } else { cx + cw };
    let y1 = if y + h < cy + ch { y + h } else { cy + ch };
    if x1 > x0 && y1 > y0 {
        Some((x0, y0, x1 - x0, y1 - y0))
    } else {
        None
    }
}

/// Se tocan? Para el texto, que es atomico.
fn cruza(cx: u32, cy: u32, cw: u32, ch: u32, x: u32, y: u32, w: u32, h: u32) -> bool {
    corte(cx, cy, cw, ch, x, y, w, h).is_some()
}

/// Repinta la caja `id` con sus colores de `:hover`. Llamalo cuando el
/// puntero entre, y `pintar` cuando salga.
pub fn realce(p: &bmo::Pantalla, ox: u32, oy: u32, id: &str) {
    if id == "k_c" {
        p.rect(ox + 8, oy + 54, 72, 72, 0x004B637E);
        p.texto(ox + 40, oy + 82, "C", 0x00E6EDF6);
        return;
    }
    if id == "k_div" {
        p.rect(ox + 86, oy + 54, 72, 72, 0x005A80A8);
        p.texto(ox + 118, oy + 82, "/", 0x00E6EDF6);
        return;
    }
    if id == "k_mul" {
        p.rect(ox + 164, oy + 54, 72, 72, 0x005A80A8);
        p.texto(ox + 196, oy + 82, "*", 0x00E6EDF6);
        return;
    }
    if id == "k_sub" {
        p.rect(ox + 242, oy + 54, 72, 72, 0x005A80A8);
        p.texto(ox + 274, oy + 82, "-", 0x00E6EDF6);
        return;
    }
    if id == "k_7" {
        p.rect(ox + 8, oy + 132, 72, 72, 0x004B637E);
        p.texto(ox + 40, oy + 160, "7", 0x00E6EDF6);
        return;
    }
    if id == "k_8" {
        p.rect(ox + 86, oy + 132, 72, 72, 0x004B637E);
        p.texto(ox + 118, oy + 160, "8", 0x00E6EDF6);
        return;
    }
    if id == "k_9" {
        p.rect(ox + 164, oy + 132, 72, 72, 0x004B637E);
        p.texto(ox + 196, oy + 160, "9", 0x00E6EDF6);
        return;
    }
    if id == "k_add" {
        p.rect(ox + 242, oy + 132, 72, 72, 0x005A80A8);
        p.texto(ox + 274, oy + 160, "+", 0x00E6EDF6);
        return;
    }
    if id == "k_4" {
        p.rect(ox + 8, oy + 210, 72, 72, 0x004B637E);
        p.texto(ox + 40, oy + 238, "4", 0x00E6EDF6);
        return;
    }
    if id == "k_5" {
        p.rect(ox + 86, oy + 210, 72, 72, 0x004B637E);
        p.texto(ox + 118, oy + 238, "5", 0x00E6EDF6);
        return;
    }
    if id == "k_6" {
        p.rect(ox + 164, oy + 210, 72, 72, 0x004B637E);
        p.texto(ox + 196, oy + 238, "6", 0x00E6EDF6);
        return;
    }
    if id == "k_1" {
        p.rect(ox + 8, oy + 288, 72, 72, 0x004B637E);
        p.texto(ox + 40, oy + 316, "1", 0x00E6EDF6);
        return;
    }
    if id == "k_2" {
        p.rect(ox + 86, oy + 288, 72, 72, 0x004B637E);
        p.texto(ox + 118, oy + 316, "2", 0x00E6EDF6);
        return;
    }
    if id == "k_3" {
        p.rect(ox + 164, oy + 288, 72, 72, 0x004B637E);
        p.texto(ox + 196, oy + 316, "3", 0x00E6EDF6);
        return;
    }
    if id == "k_eq" {
        p.rect(ox + 242, oy + 288, 72, 72, 0x006CC3FF);
        p.texto(ox + 274, oy + 316, "=", 0x00E6EDF6);
        return;
    }
    if id == "k_0" {
        p.rect(ox + 8, oy + 366, 72, 72, 0x004B637E);
        p.texto(ox + 40, oy + 394, "0", 0x00E6EDF6);
        return;
    }
    if id == "k_dot" {
        p.rect(ox + 86, oy + 366, 72, 72, 0x004B637E);
        p.texto(ox + 118, oy + 394, ".", 0x00E6EDF6);
        return;
    }
}

/// Que `id` hay bajo `(px, py)`, con la maquetacion puesta en `(ox, oy)`.
///
/// ** Sale de la MISMA pasada que `pintar`, asi que no hay una segunda
/// aritmetica que pueda discrepar: el boton que se dibuja aqui responde
/// aqui, por construccion y no por cuidado.
pub fn golpe(ox: u32, oy: u32, px: u32, py: u32) -> Option<&'static str> {
    if px >= ox + 8 && px < ox + 80 && py >= oy + 54 && py < oy + 126 {
        return Some("k_c");
    }
    if px >= ox + 86 && px < ox + 158 && py >= oy + 54 && py < oy + 126 {
        return Some("k_div");
    }
    if px >= ox + 164 && px < ox + 236 && py >= oy + 54 && py < oy + 126 {
        return Some("k_mul");
    }
    if px >= ox + 242 && px < ox + 314 && py >= oy + 54 && py < oy + 126 {
        return Some("k_sub");
    }
    if px >= ox + 8 && px < ox + 80 && py >= oy + 132 && py < oy + 204 {
        return Some("k_7");
    }
    if px >= ox + 86 && px < ox + 158 && py >= oy + 132 && py < oy + 204 {
        return Some("k_8");
    }
    if px >= ox + 164 && px < ox + 236 && py >= oy + 132 && py < oy + 204 {
        return Some("k_9");
    }
    if px >= ox + 242 && px < ox + 314 && py >= oy + 132 && py < oy + 204 {
        return Some("k_add");
    }
    if px >= ox + 8 && px < ox + 80 && py >= oy + 210 && py < oy + 282 {
        return Some("k_4");
    }
    if px >= ox + 86 && px < ox + 158 && py >= oy + 210 && py < oy + 282 {
        return Some("k_5");
    }
    if px >= ox + 164 && px < ox + 236 && py >= oy + 210 && py < oy + 282 {
        return Some("k_6");
    }
    if px >= ox + 8 && px < ox + 80 && py >= oy + 288 && py < oy + 360 {
        return Some("k_1");
    }
    if px >= ox + 86 && px < ox + 158 && py >= oy + 288 && py < oy + 360 {
        return Some("k_2");
    }
    if px >= ox + 164 && px < ox + 236 && py >= oy + 288 && py < oy + 360 {
        return Some("k_3");
    }
    if px >= ox + 242 && px < ox + 314 && py >= oy + 288 && py < oy + 360 {
        return Some("k_eq");
    }
    if px >= ox + 8 && px < ox + 80 && py >= oy + 366 && py < oy + 438 {
        return Some("k_0");
    }
    if px >= ox + 86 && px < ox + 158 && py >= oy + 366 && py < oy + 438 {
        return Some("k_dot");
    }
    None
}

/// Esta `(px, py)` dentro de la maquetacion?
pub fn dentro(ox: u32, oy: u32, px: u32, py: u32) -> bool {
    px >= ox && px < ox + ANCHO && py >= oy && py < oy + ALTO
}

/// Los huecos que rellena otro proceso: nombre, x, y, ancho, alto.
///
/// Relativos al origen. Una isla es una superficie de `PLAN_DIRECTOR.md`
/// vista desde la maqueta: aqui solo se dice DONDE va.
pub const ISLAS: [(&str, u32, u32, u32, u32); 1] = [
    ("visor", 8, 8, 306, 40),
];

/// El rect de una isla por su nombre: x, y, ancho, alto.
pub fn isla(nombre: &str) -> Option<(u32, u32, u32, u32)> {
    let mut k = 0;
    while k < ISLAS.len() {
        let (n, x, y, w, h) = ISLAS[k];
        if n == nombre {
            return Some((x, y, w, h));
        }
        k += 1;
    }
    None
}

/// Repinta el fondo de una isla, para borrar lo que hubiera dentro.
///
/// ** Existe para que quien rellena la isla NO tenga que saber su color.
/// Copiarlo en Rust seria una segunda verdad, y el dia que cambie el
/// `.maqueta` una de las dos se quedaria vieja sin avisar.
pub fn limpiar_isla(p: &bmo::Pantalla, ox: u32, oy: u32, nombre: &str) {
    if nombre == "visor" {
        p.rect(ox + 8, oy + 8, 306, 40, 0x00161C28);
        return;
    }
}
