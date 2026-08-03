//! **El compositor de BMO.** El proceso Ring 3 que es dueño de la pantalla.
//!
//! ## La caja
//!
//! No hay terminal. Había uno planeado —`apps/terminal`, doce líneas de
//! esqueleto— y se ha quitado, porque un terminal de verdad es una pila entera:
//! scrollback, PTY, señales, un intérprete, edición de línea, historial. Nada de
//! eso hace falta para lo único que hoy se quiere hacer desde la pantalla, que
//! es **arrancar un programa**.
//!
//! Así que lo que hay es una caja de una línea, como el `Win+R` de Windows.
//! Escribes una ruta, pulsas Enter, y el `.bex` corre. Es la forma más pequeña
//! de "terminal" que sigue siendo útil, y no arrastra nada de lo otro.
//!
//! ★ Y no es una API prestada de nadie: `Win+R` tampoco lo es allí. Es UI del
//! shell, y por debajo acaba llamando a lo mismo que llamaría cualquiera. Aquí
//! por debajo hay `OP_EJECUTAR` sobre `CURRENT_TASK`, que es una operación más
//! en una tabla — **el ABI de tres syscalls no se toca para esto**.
//!
//! ## Quién manda sobre el teclado
//!
//! Reclamar `KIND_INPUT` ahora cede el teclado además del ratón, y eso tiene
//! consecuencia al otro lado: mientras este proceso viva, el shell de Ring 0 no
//! lee el teclado físico. No es un reparto —los dos drenarían la misma cola y
//! se robarían letras— es una cesión. El cable serie sigue siendo del kernel,
//! que es lo que hace falta cuando esto se rompa.
//!
//! ## La tira de medida sigue
//!
//! Los seis parches de color siguen ahí abajo porque la pregunta que hacen
//! sigue abierta: en la primera foto en hardware la geometría salió exacta pero
//! los colores mucho más claros de lo que dice el código. Hasta que una foto lo
//! zanje, se quedan —
//!
//! - si el parche `0x00FF0000` sale ROJO, el formato es XRGB como creemos;
//! - si sale AZUL, los canales están al revés (BGR) y hay que voltearlos;
//! - si `0x00202020` sale gris medio en vez de casi negro, no es orden de
//!   canales: algo toca la intensidad (el panel, o el propio GOP).

#![no_std]
#![no_main]

use bmo_userland as bmo;

// ── El reparto ──────────────────────────────────────────────────────────
//
// Eran 2308 lineas en un solo fichero: colores, geometria, la rejilla de
// salida, el historial, la calculadora, los informes del sistema, el
// interprete de comandos y el bucle de fotograma. Todo junto, compartiendo
// constantes y variables locales.
//
// Dos carpetas, y la frontera es la que importa:
//
//   escena/   lo que se PINTA   — no sabe que es un comando
//   ordenes/  lo que se INTERPRETA — no sabe de que color es la ventana
//
// `main.rs` se queda con lo unico que necesita a las dos: el bucle.
mod escena;
mod ordenes;
mod texto;

use escena::calc::{pintar_calc, Calc, CalcCaja};
use escena::cursor::Bajo;
use escena::salida::{pintar_salida, Salida, TINTA_BIEN, TINTA_ECO, TINTA_MAL, TINTA_NORMAL};
use escena::*;
use ordenes::completar::{completar, motivo_archivo};
use ordenes::historial::Historial;
use ordenes::informes::{informe_cpu, informe_memoria, informe_sistema};
use ordenes::*;
use texto::{decimal, es_punto};


// ── El programa ─────────────────────────────────────────────────────────

/// Cada cuántas vueltas del bucle parpadea el cursor de escritura.
///
/// Se cuenta en fotogramas y no en tiempo porque aquí no hay reloj: los tres
/// syscalls no incluyen "qué hora es". Es un parpadeo que depende de la
/// velocidad de la máquina, y para decir "aquí se escribe" eso basta.
const PARPADEO: u32 = 12_000;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // El aviso va ANTES de reclamar: en cuanto la cesión se consuma, el kernel
    // deja de dibujar y nada de lo que se imprima después llega al panel.
    bmo::consola("reclamo pantalla y entrada\n");

    let Some(mut p) = bmo::Pantalla::reclamar() else {
        bmo::consola("sin pantalla que reclamar\n");
        bmo::salir()
    };

    // ── ★ EL DOBLE BÚFER ──
    //
    // Se pide ANTES de pintar nada, que es cuando la RAM está menos
    // fragmentada: el bloque tiene que ser contiguo en físico y son ~8 MB.
    //
    // Y se dice en los dos casos. Que no haya doble búfer **no impide arrancar**
    // —se dibuja en el panel, como siempre—, pero cambia dos cosas que se notan:
    // vuelve el riesgo de tearing y el cursor tiene que poner una barrera antes
    // de leer. Un escritorio que se degrada en silencio es un escritorio del que
    // no se puede diagnosticar nada.
    if p.activar_doble_bufer() {
        bmo::consola("doble bufer: pintando fuera de la pantalla\n");
    } else {
        bmo::consola("SIN doble bufer: no hubo bloque, pinto directo al panel\n");
    }
    // La entrada es opcional a propósito: sin ella hay escritorio, sólo que
    // quieto y mudo. Un compositor que se niega a arrancar porque falta un
    // periférico es un compositor que no arranca el día que el periférico falla.
    let entrada = bmo::Entrada::reclamar();

    // La consola de este terminal. Desde aquí, todo lo que lance escribe en
    // ESTE anillo y no en el panel del kernel — que es lo único que separaba
    // una caja de lanzar de un terminal de verdad.
    let salida_cap = bmo::Consola::crear();

    let caja = Caja::nueva(p.ancho, p.alto);

    // ── LA ENTRADA A RING 3 ──
    //
    // Antes de dibujar nada del escritorio, decir lo que acaba de pasar: el
    // userspace tiene la máquina. Hasta hoy este paso era invisible y por eso
    // un compositor muerto y un compositor que no pinta se veían igual — un
    // shell donde debía haber un escritorio.
    //
    // Y lleva las dos capabilities OPCIONALES escritas en la cara, que es lo
    // que distingue "no funciona" de "no me la dieron".
    escena::entrada::pintar(&p, entrada.is_some(), salida_cap.is_some());
    bmo::consola("entrada a Ring 3 pintada\n");

    // Fondo entero de una pasada, y encima la escena.
    p.limpiar(FONDO);
    p.rect(0, 0, p.ancho, BARRA_ALTO, BARRA);
    p.rect(16, 14, 16, 16, ACENTO);
    let mut i = 0u32;
    while (i as usize) < MEDIDA.len() {
        p.rect(
            MEDIDA_X + i * MEDIDA_LADO,
            MEDIDA_Y,
            MEDIDA_LADO,
            MEDIDA_LADO,
            MEDIDA[i as usize],
        );
        i += 1;
    }

    // Marco del pulsómetro. Si la entrada ni se pudo reclamar, sale en rojo:
    // dos fallos distintos, dos aspectos distintos.
    let marco = if entrada.is_some() { ACENTO } else { 0x00E0_4040 };
    p.rect(PULSO_X - 2, PULSO_Y - 2, PULSO_ANCHO + 4, PULSO_ALTO + 4, marco);
    p.rect(PULSO_X, PULSO_Y, PULSO_ANCHO, PULSO_ALTO, FONDO);

    pintar_caja(&p, &caja);
    let mut ruta = [0u8; RUTA_MAX];
    let mut n = 0usize;
    let mut salida = Salida::nueva();
    let mut historial = Historial::nuevo();
    // Posicion del cursor DENTRO de la linea. Sin esto solo se puede escribir
    // al final y borrar desde el final: equivocarte en la tercera letra de una
    // ruta larga obliga a borrarlo todo hasta ahi.
    let mut cur = 0usize;
    // Portapapeles. Ctrl+C copia la linea entera, Ctrl+V la pega donde este el
    // cursor. Ctrl+ARRIBA / Ctrl+ABAJO hacen lo mismo con las flechas.
    let mut porta = [0u8; RUTA_MAX];
    let mut porta_n = 0usize;
    let mut calc = Calc::nueva();
    let calc_caja = CalcCaja::nueva(&caja);
    // Flanco del botón del ratón: un clic es una BAJADA, no "el botón está
    // pulsado". Sin esto, mantener pulsado teclearía cien veces por segundo.
    let mut boton_antes = false;
    // Mientras el motor no conteste, su salida NO va a la rejilla: es el
    // resultado, no un mensaje. Se acumula aparte.
    let mut resp = [0u8; 24];
    let mut resp_n = 0usize;
    if salida_cap.is_none() {
        salida.texto(b"sin consola: la salida de los programas ira al panel del kernel\n");
    }
    pintar_campo(&p, &caja, &ruta[..n], cur, true);
    pintar_salida(&p, &caja, &salida);
    if entrada.is_some() {
        pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
    } else {
        // Decirlo, y decir por qué. Una caja que no responde y no explica nada
        // es peor que no tener caja.
        pintar_estado(&p, &caja, "sin teclado: la entrada no se pudo reclamar", TEXTO_MAL);
    }

    bmo::consola("escritorio pintado\n");

    // ── El bucle de vida ──
    //
    // No termina: si saliera, `revoke_all` devolvería la pantalla y el kernel
    // repintaría su panel encima. Un escritorio es un proceso que VIVE — y de
    // paso esto ejerce el cambio de contexto miles de veces por segundo, que es
    // justo el camino que costó una foto de madrugada.
    let (mut ax, mut ay) = (u32::MAX, u32::MAX);
    let mut pulso_previo = 0u32;
    let mut vueltas = 0u32;
    let mut caret = true;
    // Vueltas desde la última tecla. Se reinicia al escribir para que el
    // cursor esté SIEMPRE encendido mientras se teclea.
    let mut desde_tecla: u32 = 0;
    // ── El atajo: un TOQUE de Ctrl+Alt ──
    //
    // Se dispara al SOLTAR, y sólo si no llegó ningún carácter mientras
    // estaban pulsados. No es una floritura: en la distribución española
    // `Ctrl+Alt` **es** `AltGr` —lo que produce `@`, `#`, `[`, `]`, `\`, `|`
    // y `€`— así que disparar al pulsarlos rompería escribir todos esos
    // caracteres. Con el toque, `Ctrl+Alt` a secas invoca la ventana y
    // `Ctrl+Alt+2` sigue dando `@`.
    let mut combo_antes = false;
    let mut hubo_tecla_en_combo = false;
    let mut visible = true;
    // ── La consola de DATOS (F12) ──
    //
    // Una tecla de funcion no produce caracter en NINGUNA distribucion, asi que
    // no puede chocar con escribir. Es lo unico que importa en un atajo del
    // sistema, y es lo que `Ctrl+Alt` no puede ofrecer: en espanol ES AltGr.
    let caja_datos = escena::datos::CajaDatos::nueva(&p);
    // ★ ABIERTA no es lo mismo que ARRIBA. Abierta es "existe y esta dibujada";
    // arriba es "es la que tapa a la otra". Se separan porque aqui no hay
    // recorte: las ventanas se pintan enteras una encima de otra, y la ultima
    // que se pinta gana. Sin la distincion, Alt+Tab podria dejar el teclado en
    // Ejecutar con Datos delante — escribiendo en una linea que no se ve, que
    // es el mismo fallo de antes al reves.
    let mut datos_abierta = false;

    // ── La consola del KERNEL (F11) ──
    //
    // Lo que dice Ring 0, leído desde aquí. **No es "ir a Ring 0"**: este
    // proceso sigue en Ring 3 con sus capabilities contadas y lo único que hace
    // es preguntar (`TASK_OP_KLOG_*`). Ver `escena::klog`.
    //
    // Y F11 en vez de un comando por una razón de hoy: **no hace falta teclear
    // nada para abrirla**. Cuando lo que falla es el campo donde se escribe, un
    // diagnóstico que exige escribir un comando no sirve de nada.
    let caja_klog = escena::klog::CajaKlog::nueva(&p);
    let mut klog_abierta = false;
    // Cuántas líneas hacia atrás empieza la ventana. RePág/AvPág la mueven, que
    // es lo que permite llegar al PRINCIPIO del arranque — donde están las
    // respuestas de por qué algo no arrancó.
    let mut klog_desplazamiento = 0u64;

    /// Qué tecla de la calculadora tiene el puntero encima, si alguna.
    ///
    /// Se lleva como estado porque el realce sólo se repinta **cuando cambia**:
    /// repintar la calculadora entera en cada fotograma que el ratón se mueva
    /// un píxel serían veinte rectángulos y veinte glifos por vuelta para
    /// enseñar exactamente lo mismo.
    let mut calc_encima: Option<u8> = None;

    // ── El FOCO ──
    //
    // Quien recibe las teclas cuando hay mas de una ventana. La politica vive
    // en `bmo_input::foco` y se prueba ALLI (12 tests); aqui solo se le
    // pregunta y se pinta lo que decidio.
    //
    // Hacia falta ya: hasta ahora F12 se atendia arriba del todo y **todo lo
    // demas caia en Ejecutar** aunque Datos estuviera abierta. Con una tercera
    // ventana, chocan.
    const V_EJECUTAR: u8 = 0;
    const V_DATOS: u8 = 1;
    const V_KLOG: u8 = 2;
    let mut foco = bmo_input::Foco::nuevo();
    foco.abrir(V_EJECUTAR);
    let mut alt_antes = false;
    let mut conmutador_pintado = false;
    // Quién tapaba a quién en la vuelta anterior, para pintar sólo cuando
    // cambia. `datos_abierta && foco.es_para(V_DATOS)` es la cuenta entera:
    // **la que tiene el teclado es la que se ve**.
    let mut arriba_antes = V_EJECUTAR;

    // Lo que hay DEBAJO del cursor del ratón. Ver `escena::cursor::Bajo`: se
    // quita al principio del fotograma y se pone al final, y en medio se pinta.
    let mut bajo = Bajo::nuevo();
    // Estado anterior de los botones EN PANTALLA, para no repintar el testigo
    // del pulsómetro sesenta veces por segundo con el mismo color.
    let mut col_boton = FONDO;

    loop {
        vueltas = vueltas.wrapping_add(1);
        let mut repintar_campo = false;

        // ── ¿Va a pintar algo este fotograma? ──
        //
        // Hay que saberlo ANTES de pintar, porque el cursor del ratón se quita
        // al principio y se pone al final: hacerlo en todos los fotogramas
        // dejaría el puntero ausente la mitad del tiempo y en pantalla se vería
        // pálido y parpadeante. Y como leer una tecla la CONSUME, "¿hay
        // teclas?" obliga a tenerlas ya en la mano.
        //
        // Lo que se lee aquí no se interpreta aquí: esto sólo recoge.
        let mut va_a_pintar = salida.sucia || desde_tecla + 1 >= PARPADEO;

        if let Some(e) = entrada.as_ref() {
            // ── El atajo, ANTES de leer teclas ──
            let m = e.modificadores();
            let ctrl = m & bmo::MOD_CTRL != 0;
            let combo = ctrl && m & bmo::MOD_ALT != 0;
            // ★ Alt SOLO, sin Ctrl. La distincion no es cosmetica: `Ctrl+Alt`
            // **es AltGr** en espanol, y ya tiene dueno (invocar la ventana).
            // El driver ademas da el Alt DERECHO como `SC_ALTGR` con codigo
            // propio, asi que `MOD_ALT` es el izquierdo — el de Alt+Tab de toda
            // la vida.
            let alt_solo = m & bmo::MOD_ALT != 0 && !ctrl;

            // El tope no descarta: lo que no quepa se queda en el anillo del
            // kernel y llega en el fotograma siguiente. Drenar sin tope y tirar
            // el sobrante sería perder letras justo cuando se escribe rápido.
            let mut teclas = [0u8; 64];
            let mut nt = 0usize;
            while nt < teclas.len() {
                match e.tecla() {
                    Some(c) => {
                        teclas[nt] = c;
                        nt += 1;
                    }
                    None => break,
                }
            }
            let pos = e.puntero();
            let giro = e.rueda();
            let ev_raton = e.eventos().min(PULSO_ANCHO as u64) as u32;

            va_a_pintar |= nt > 0
                || giro != 0
                || pos.x != ax
                || pos.y != ay
                || (pos.botones != 0) != boton_antes
                || ev_raton != pulso_previo
                || alt_solo != alt_antes
                || combo != combo_antes;

            // A partir de aquí se PINTA, así que el cursor se aparta.
            if va_a_pintar {
                bajo.quitar(&p);
            }

            // ── Alt+Tab: el conmutador ──
            //
            // La pila se reordena al SOLTAR, no en cada Tab: eso es lo que hace
            // que pulsarlo dos veces te devuelva a donde estabas. Ver
            // `bmo_input::foco`.
            if !alt_solo && alt_antes && foco.conmutando() {
                foco.soltar_conmutador();
                let (bx, by, ba, bh) = escena::conmutador::area(&p, foco.abiertas());
                for fy in 0..bh {
                    for fx in 0..ba {
                        let (x, y) = (bx + fx, by + fy);
                        p.punto(x, y, color_escena(&caja, visible, x, y));
                    }
                }
                conmutador_pintado = false;
                // Lo que tapaba vuelve a pintarse entero, **de abajo arriba**:
                // es el unico orden que deja la pantalla como estaba. Y quien
                // va arriba lo acaba de decidir el Alt que se solto.
                //
                // ★ Con tres ventanas esto se escribe como lo que es: pintar
                // TODAS las abiertas, y la que tiene el foco la ÚLTIMA. La
                // versión de dos ventanas enumeraba los casos a mano, y con
                // tres eso son seis ramas que dicen una sola regla.
                let arriba_ahora = if klog_abierta && foco.es_para(V_KLOG) {
                    V_KLOG
                } else if datos_abierta && foco.es_para(V_DATOS) {
                    V_DATOS
                } else {
                    V_EJECUTAR
                };
                let mut pintar_una = |v: u8, repintar: &mut bool, sal: &mut escena::salida::Salida| {
                    match v {
                        V_KLOG if klog_abierta => {
                            escena::klog::pintar(&p, &caja_klog, klog_desplazamiento)
                        }
                        V_DATOS if datos_abierta => escena::datos::pintar(&p, &caja_datos),
                        V_EJECUTAR if visible => {
                            pintar_caja(&p, &caja);
                            *repintar = true;
                            sal.sucia = true;
                        }
                        _ => {}
                    }
                };
                for v in [V_EJECUTAR, V_DATOS, V_KLOG] {
                    if v != arriba_ahora {
                        pintar_una(v, &mut repintar_campo, &mut salida);
                    }
                }
                pintar_una(arriba_ahora, &mut repintar_campo, &mut salida);
                arriba_antes = arriba_ahora;
            }
            alt_antes = alt_solo;
            if combo && !combo_antes {
                hubo_tecla_en_combo = false;
            }
            if !combo && combo_antes && !hubo_tecla_en_combo {
                visible = !visible;
                if visible {
                    // Esconderla y volver a invocarla es cerrarla y abrirla
                    // para el foco. Sin esto, Alt+Tab llevaria el teclado a una
                    // ventana que no esta en la pantalla: escribirias en algo
                    // invisible, que es la peor forma de perder una linea.
                    foco.abrir(V_EJECUTAR);
                    pintar_caja(&p, &caja);
                    repintar_campo = true;
                    salida.sucia = true;
                    pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                } else {
                    foco.cerrar(V_EJECUTAR);
                    borrar_caja(&p, &caja);
                }
            }
            combo_antes = combo;

            // ── Teclado ──
            //
            // Se atienden TODAS las de la vuelta, no una por fotograma:
            // escribiendo rápido llegan varias entre vuelta y vuelta, y
            // quedarse con una sería perder letras de forma que parecería un
            // teclado malo. Ya están recogidas arriba.
            for &c in &teclas[..nt] {
                // Tab con Alto pulsado NO llega a ninguna ventana: es del
                // conmutador. Shift lo recorre al reves.
                if alt_solo && c == 0x09 {
                    if m & bmo::MOD_SHIFT != 0 {
                        foco.conmutar_atras();
                    } else {
                        foco.conmutar();
                    }
                    escena::conmutador::pintar(
                        &p,
                        foco.lista(),
                        foco.indice_señalado(),
                        foco.modo().nombre(),
                    );
                    conmutador_pintado = true;
                    continue;
                }
                // ── Alt+M: cambiar el MODO del foco ──
                //
                // Sin una tecla, los tres modos son decoracion: `Fijo` y
                // `Puntero` existirian sin forma de llegar a ellos. Va con Alt
                // por lo mismo que el Tab —`Alt` solo no produce caracter en
                // ninguna distribucion, `Ctrl+Alt` SI (es AltGr)— y se anuncia
                // en la propia ventanita, que es donde se lee el modo.
                if alt_solo && (c == b'm' || c == b'M') {
                    foco.poner_modo(foco.modo().siguiente());
                    if conmutador_pintado {
                        escena::conmutador::pintar(
                            &p,
                            foco.lista(),
                            foco.indice_señalado(),
                            foco.modo().nombre(),
                        );
                    } else if visible {
                        // Cambiarlo sin el conmutador abierto tambien tiene que
                        // verse: un modo que cambia en silencio se descubre
                        // cuando el teclado ya se fue a otra ventana.
                        pintar_estado(&p, &caja, foco.modo().nombre_largo(), ACENTO);
                    }
                    continue;
                }
                // Cualquier tecla durante el combo lo convierte en AltGr y
                // cancela el toque: el usuario estaba escribiendo, no llamando.
                if combo {
                    hubo_tecla_en_combo = true;
                }

                // ── F12 es del SISTEMA, no de una ventana ──
                //
                // Se atiende ANTES de preguntar por el foco, y tiene que ser
                // asi: un atajo que solo funciona si ya estas en la ventana que
                // abre no sirve para abrirla — y peor, no sirve para cerrarla,
                // porque para entonces el foco ya es suyo.
                //
                // ESC cierra la de arriba, que es lo que hace ESC en todas
                // partes. En Ejecutar ESC sigue borrando la linea: son dos
                // ventanas distintas y cada una contesta lo suyo.
                let conmutar_datos = if c == 0x94 {
                    Some(!datos_abierta)
                } else if c == 0x1B && datos_abierta && foco.es_para(V_DATOS) {
                    Some(false)
                } else {
                    None
                };
                if let Some(abrir) = conmutar_datos {
                    datos_abierta = abrir;
                    if abrir {
                        // Abrir es decirselo al foco y ya: en modo `Fijo` la
                        // ventana aparece y NO se lleva el teclado, y quien
                        // decide eso es la politica, no esta tecla.
                        foco.abrir(V_DATOS);
                        escena::datos::pintar(&p, &caja_datos);
                        arriba_antes = if foco.es_para(V_DATOS) { V_DATOS } else { V_EJECUTAR };
                        // En `Fijo` se ha pintado encima de una caja que sigue
                        // teniendo el teclado: hay que devolverla arriba.
                        if arriba_antes == V_EJECUTAR && visible {
                            pintar_caja(&p, &caja);
                            repintar_campo = true;
                            salida.sucia = true;
                        }
                    } else {
                        // Al cerrarla hay que devolver el fondo Y repintar
                        // lo que tapaba: la caja de Ejecutar esta debajo.
                        foco.cerrar(V_DATOS);
                        borrar_ventana(
                            &p, &caja, caja_datos.x, caja_datos.y,
                            caja_datos.ancho, caja_datos.alto, visible,
                        );
                        arriba_antes = V_EJECUTAR;
                        if visible {
                            pintar_caja(&p, &caja);
                            repintar_campo = true;
                            salida.sucia = true;
                        }
                    }
                    continue;
                }

                // ── F11: la consola del KERNEL ──
                //
                // Calcada de F12 y por los mismos motivos: se atiende ANTES de
                // preguntar por el foco, porque un atajo que sólo funciona si ya
                // estás dentro de la ventana no sirve para abrirla.
                let conmutar_klog = if c == 0x93 {
                    Some(!klog_abierta)
                } else if c == 0x1B && klog_abierta && foco.es_para(V_KLOG) {
                    Some(false)
                } else {
                    None
                };
                if let Some(abrir) = conmutar_klog {
                    klog_abierta = abrir;
                    if abrir {
                        // Se abre SIEMPRE por lo último, que es lo que se quiere
                        // ver el 90% de las veces. Para ir al arranque están
                        // RePág/AvPág.
                        klog_desplazamiento = 0;
                        foco.abrir(V_KLOG);
                        escena::klog::pintar(&p, &caja_klog, klog_desplazamiento);
                        arriba_antes = if foco.es_para(V_KLOG) { V_KLOG } else { V_EJECUTAR };
                        if arriba_antes == V_EJECUTAR && visible {
                            pintar_caja(&p, &caja);
                            repintar_campo = true;
                            salida.sucia = true;
                        }
                    } else {
                        foco.cerrar(V_KLOG);
                        borrar_ventana(
                            &p, &caja, caja_klog.x, caja_klog.y,
                            caja_klog.ancho, caja_klog.alto, visible,
                        );
                        arriba_antes = V_EJECUTAR;
                        if visible {
                            pintar_caja(&p, &caja);
                            repintar_campo = true;
                            salida.sucia = true;
                        }
                        // Si Datos estaba abierta debajo, vuelve a verse.
                        if datos_abierta {
                            escena::datos::pintar(&p, &caja_datos);
                        }
                    }
                    continue;
                }

                // RePág/AvPág dentro de la consola del kernel: recorrer el log.
                //
                // Va aquí y no en el editor de línea porque **es de esta
                // ventana**: con el foco en el kernel, esas teclas no tienen
                // nada que ver con el historial de salida de Ejecutar.
                if klog_abierta && foco.es_para(V_KLOG) && (c == 0x87 || c == 0x88) {
                    let hay = bmo::klog_lineas();
                    if c == 0x87 {
                        // Hacia atrás en el tiempo, sin pasarse del principio.
                        klog_desplazamiento = (klog_desplazamiento + 8).min(hay.saturating_sub(1));
                    } else {
                        klog_desplazamiento = klog_desplazamiento.saturating_sub(8);
                    }
                    escena::klog::pintar(&p, &caja_klog, klog_desplazamiento);
                    continue;
                }

                // ── ★ ¿DE QUIEN es esta tecla? ──
                //
                // La pregunta que faltaba, y la razon de que exista
                // `bmo_input::foco`. Hasta ahora TODA tecla se editaba en la
                // linea de Ejecutar aunque la consola de datos estuviera
                // encima: escribias en una ventana tapada, sin verlo. Con una
                // tercera, chocan.
                //
                // Ninguna abierta —todas escondidas— tampoco es "Ejecutar por
                // defecto": las teclas se descartan y vuelven al invocarla.
                if !foco.es_para(V_EJECUTAR) {
                    continue;
                }
                debug_assert!(visible, "el foco de una ventana escondida es un bug");
                // Cualquier tecla enciende el cursor y reinicia el parpadeo.
                caret = true;
                desde_tecla = 0;
                repintar_campo = true;
                match c {
                    b'\r' | b'\n' => {
                        // Eco SIEMPRE, también de lo que no se entiende: un
                        // terminal que se traga lo que escribiste deja al
                        // usuario sin saber qué llegó.
                        // El eco lleva un punto medio (0xB7) y no `>`. El `>`
                        // es la marca de Unix y este sistema no es Unix; el
                        // punto medio separa igual de bien y no arrastra la
                        // convencion de otro. Esta en la tabla de extras del
                        // font, asi que se dibuja sin tocar nada mas.
                        // El eco en su tinta y la respuesta en la normal: al
                        // mirar la rejilla, los comandos son las anclas y todo
                        // lo de debajo es lo que contestaron.
                        salida.con_tinta(TINTA_ECO);
                        salida.byte(0xB7);
                        salida.byte(b' ');
                        salida.texto(&ruta[..n]);
                        salida.byte(b'\n');
                        salida.con_tinta(TINTA_NORMAL);

                        // ¿Hay un programa vivo escuchando en esta consola?
                        // Entonces la linea NO es un comando: es SUYA. Es lo
                        // que hace cualquier shell, y sin esto un `ACCEPT` de
                        // COBOL no puede recibir nada nunca — el terminal se
                        // come la respuesta y contesta "no lo conozco".
                        //
                        // La calculadora se excluye a proposito: mientras
                        // espera al motor, ese hijo es SUYO y ya recibio sus
                        // tres lineas. Colar una mas ahi le cambiaria la
                        // cuenta a alguien que no la pidio.
                        let del_hijo = !calc.esperando
                            && salida_cap.as_ref().map(|cc| cc.hay_hijo()).unwrap_or(false);

                        if del_hijo {
                            if let Some(cc) = salida_cap.as_ref() {
                                cc.escribir(&ruta[..n]);
                                // El salto va aparte y SIEMPRE: `read_line`
                                // espera a verlo para dar la linea por
                                // cerrada. Sin el, el programa sigue
                                // esperando algo que ya escribiste.
                                cc.escribir(b"\n");
                            }
                            pintar_estado(&p, &caja, "para el programa", TEXTO_TENUE);
                            n = 0;
                            cur = 0;
                            repintar_campo = true;
                            continue;
                        }

                        // Al historial va lo que es un COMANDO. Un importe
                        // tecleado para un `ACCEPT` es un dato, y mezclarlo
                        // con las rutas ensucia la flecha arriba justo cuando
                        // hace falta repetir el comando de verdad.
                        historial.empujar(&ruta[..n]);
                        match interpretar(&ruta[..n]) {
                            Orden::Nada => {
                                pintar_estado(&p, &caja, "escribe algo", TEXTO_TENUE);
                            }
                            Orden::Listar(ruta_dir) => {
                                match bmo::Directorio::abrir(ruta_dir) {
                                    Ok(d) => {
                                        let mut cuantas = 0u32;
                                        // Tope por si un directorio enorme se
                                        // comiera el fotograma entero.
                                        while cuantas < 256 {
                                            let e = match d.siguiente() {
                                                Some(e) => e,
                                                None => break,
                                            };
                                            let mut nom = [0u8; 12];
                                            let largo = e.legible(&mut nom);
                                            // `.` y `..` no se enseñan: aqui
                                            // no hay carpeta actual a la que
                                            // volver, asi que son ruido.
                                            if es_punto(&nom[..largo]) { continue; }
                                            salida.texto(b"  ");
                                            salida.texto(&nom[..largo]);
                                            // Alinear la columna del tamaño.
                                            let mut k = largo;
                                            while k < 14 { salida.byte(b' '); k += 1; }
                                            if e.es_dir {
                                                salida.texto(b"<DIR>");
                                            } else {
                                                let mut d10 = [0u8; 10];
                                                let n10 = decimal(e.bytes as u64, &mut d10);
                                                salida.texto(&d10[..n10]);
                                            }
                                            salida.byte(b'\n');
                                            cuantas += 1;
                                        }
                                        if cuantas == 0 {
                                            salida.texto(b"  (vacio)
");
                                        }
                                        pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                    }
                                    // ★ El MOTIVO, no un "no pude" para todo.
                                    //
                                    // Esto tiraba el código con `Err(_)` y
                                    // decía siempre "no puedo abrir esa
                                    // carpeta". Cuando la tabla de directorios
                                    // del kernel se llenó, eso fue una mentira
                                    // exacta: la carpeta estaba ahí, lo que no
                                    // había era ranura. Y mandó a buscar el
                                    // fallo al disco, que estaba perfecto.
                                    //
                                    // Un error que no distingue sus causas es
                                    // un error que manda a mirar donde no es.
                                    Err(cod) => {
                                        // 25 = sin hueco, 26 = no está. Ver
                                        // `ring0/obj/directorio.rs`.
                                        let (linea, estado): (&[u8], &str) = if cod == 25 {
                                            (
                                                b"  no queda ranura de directorio en el kernel.\n",
                                                "sin ranura libre",
                                            )
                                        } else {
                                            (
                                                b"  no puedo abrir esa carpeta.\n",
                                                "carpeta no encontrada",
                                            )
                                        };
                                        salida.con_tinta(TINTA_MAL);
                                        salida.texto(linea);
                                        salida.con_tinta(TINTA_NORMAL);
                                        pintar_estado(&p, &caja, estado, TEXTO_MAL);
                                    }
                                }
                                n = 0;
                            }
                            // ── Leer un archivo ──
                            //
                            // El hermano de `ls`: aquel dice QUE hay, este
                            // enseña lo de DENTRO. Es la primera vez que un
                            // programa de Ring 3 abre un archivo del disco.
                            Orden::Leer(ruta_arch) => {
                                match bmo::Archivo::leer_de(ruta_arch) {
                                    Ok(a) => {
                                        let mut trozo = [0u8; 256];
                                        let mut total = 0usize;
                                        // El ultimo byte se guarda segun pasa:
                                        // reconstruirlo al final obligaria a
                                        // saber en que trozo cayo, y el buffer
                                        // ya se ha reutilizado.
                                        let mut ultimo = 0u8;
                                        // De 256 en 256 y con tope: un archivo
                                        // que no sea texto llenaria la rejilla
                                        // de basura y se comeria el fotograma.
                                        loop {
                                            let n = a.leer(&mut trozo);
                                            if n == 0 { break; }
                                            salida.texto(&trozo[..n]);
                                            ultimo = trozo[n - 1];
                                            total += n;
                                            if total >= 2048 {
                                                salida.texto(b"\n  ...(cortado)\n");
                                                ultimo = b'\n';
                                                break;
                                            }
                                        }
                                        if total == 0 {
                                            salida.texto(b"  (vacio)\n");
                                        } else if ultimo != b'\n' {
                                            // Sin esto, el proximo mensaje se
                                            // pega al final del archivo.
                                            salida.byte(b'\n');
                                        }
                                        a.cerrar();
                                        pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                    }
                                    Err(e) => {
                                        salida.con_tinta(TINTA_MAL);
                                        salida.texto(b"  ");
                                        salida.texto(motivo_archivo(e));
                                        salida.byte(b'\n');
                                        salida.con_tinta(TINTA_NORMAL);
                                        pintar_estado(&p, &caja, "no se pudo leer", TEXTO_MAL);
                                    }
                                }
                                n = 0;
                            }
                            // ── Escribir un archivo ──
                            //
                            // Lo que NUNCA habia pasado: un programa de Ring 3
                            // dejando algo en el disco. Hasta hoy todo lo que
                            // habia ahi lo puso el anfitrion al flashear o el
                            // kernel con su caja negra.
                            Orden::Escribir(ruta_arch, texto) => {
                                match bmo::Archivo::crear(ruta_arch) {
                                    Ok(a) => {
                                        let puestos = a.escribir(texto);
                                        // El salto final: un archivo de texto
                                        // sin el ultimo salto es el clasico
                                        // que descuadra al siguiente que lo lee.
                                        a.escribir(b"\n");
                                        // ★ Aqui es donde llega al disco. Antes
                                        // de esto no hay nada escrito.
                                        if a.cerrar() {
                                            salida.texto(b"  guardado: ");
                                            let mut d10 = [0u8; 10];
                                            let n10 = decimal(puestos as u64 + 1, &mut d10);
                                            salida.texto(&d10[..n10]);
                                            salida.texto(b" bytes\n");
                                            pintar_estado(&p, &caja, "guardado", TEXTO_BIEN);
                                        } else {
                                            salida.texto(b"  no se guardo nada.\n");
                                            pintar_estado(&p, &caja, "no se pudo guardar", TEXTO_MAL);
                                        }
                                    }
                                    Err(e) => {
                                        salida.con_tinta(TINTA_MAL);
                                        salida.texto(b"  ");
                                        salida.texto(motivo_archivo(e));
                                        salida.byte(b'\n');
                                        salida.con_tinta(TINTA_NORMAL);
                                        pintar_estado(&p, &caja, "no se pudo crear", TEXTO_MAL);
                                    }
                                }
                                n = 0;
                            }
                            // ★ La primera orden del escritorio que ESCRIBE EN
                            // EL DISCO. Ver `bmo::estratos_sellar`.
                            Orden::EstratosSellar => {
                                let g = bmo::estratos_sellar();
                                if g == 0 {
                                    salida.con_tinta(TINTA_MAL);
                                    salida.texto(b"  el sellado NO se hizo. el volumen sigue igual.\n");
                                    salida.con_tinta(TINTA_NORMAL);
                                    salida.texto(b"  el motivo esta en F11 (consola del kernel).\n");
                                    pintar_estado(&p, &caja, "no se sello", TEXTO_MAL);
                                } else {
                                    salida.con_tinta(TINTA_BIEN);
                                    salida.texto(b"  COMMIT. generacion ");
                                    let mut d = [0u8; 10];
                                    let k = decimal(g, &mut d);
                                    salida.texto(&d[..k]);
                                    salida.byte(b'\n');
                                    salida.con_tinta(TINTA_NORMAL);
                                    // La prueba de verdad no es este mensaje.
                                    salida.texto(b"  ESTRATOS acaba de escribir en el disco.\n");
                                    salida.texto(b"  F12 debe decir esa misma generacion.\n");
                                    salida.texto(b"  y tras REINICIAR debe seguir diciendola:\n");
                                    salida.texto(b"  eso es lo que prueba que llego al plato.\n");
                                    pintar_estado(&p, &caja, "sellado", TEXTO_BIEN);
                                }
                                n = 0;
                            }
                            // ★ `perf` — el número antes que la tarjeta.
                            //
                            // Se pinta ANTES de leer los contadores no: se leen
                            // aquí y se imprimen, y el fotograma que los pinta
                            // sumará el suyo. Da igual: lo que interesa es el
                            // orden de magnitud y el peor caso, no un dígito.
                            Orden::Pintado => {
                                let v = p.volcado();
                                salida.texto(b"  pintado\n");
                                salida.texto(b"    modo        ");
                                salida.texto(match v.modo {
                                    bmo::Volcador::Ninguno => b"directo al panel (SIN doble bufer)\n" as &[u8],
                                    bmo::Volcador::Directo => b"doble bufer, volcado por CPU\n",
                                });
                                salida.texto(b"    fotogramas  ");
                                let mut d = [0u8; 10];
                                let k = decimal(v.fotogramas, &mut d);
                                salida.texto(&d[..k]);
                                salida.texto(b"   con algo que mover\n");
                                if v.fotogramas > 0 {
                                    salida.texto(b"    medio      ");
                                    let k = decimal(v.bytes / v.fotogramas / 1024, &mut d);
                                    salida.texto(&d[..k]);
                                    salida.texto(b" KiB por fotograma\n");
                                    // El PEOR caso va aparte y a propósito: un
                                    // tirón se nota y una media buena lo tapa.
                                    salida.texto(b"    peor       ");
                                    let k = decimal(v.peor / 1024, &mut d);
                                    salida.texto(&d[..k]);
                                    salida.texto(b" KiB en un fotograma\n");
                                    salida.texto(b"    total      ");
                                    let k = decimal(v.bytes / 1024 / 1024, &mut d);
                                    salida.texto(&d[..k]);
                                    salida.texto(b" MiB movidos desde el arranque\n");
                                }
                                salida.con_tinta(TINTA_ECO);
                                salida.texto(b"    la caja de sucio ya recorta esto: una GPU solo\n");
                                salida.texto(b"    compra algo si estos numeros son grandes.\n");
                                salida.con_tinta(TINTA_NORMAL);
                                pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                n = 0;
                            }
                            Orden::Calculadora => {
                                calc.visible = !calc.visible;
                                if calc.visible {
                                    pintar_calc(&p, &calc_caja, &calc, calc_encima);
                                    salida.texto(b"  calculadora: la cara en Rust, el calculo en COBOL
");
                                } else {
                                    // Devolver esa zona a la escena.
                                    for f in 0..calc_caja.alto {
                                        for co in 0..calc_caja.ancho {
                                            let (px, py) = (calc_caja.x + co, calc_caja.y + f);
                                            p.punto(px, py, color_escena(&caja, visible, px, py));
                                        }
                                    }
                                }
                                pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                n = 0;
                                cur = 0;
                            }
                            Orden::Limpiar => {
                                salida.limpiar();
                                pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                n = 0;
                            }
                            Orden::Ayuda => {
                                salida.texto(b"  <ruta>       lanza un .bex   (cobol/banco.bex)\n");
                                salida.texto(b"  run <ruta>   lo mismo, como en el shell de Ring 0\n");
                                salida.texto(b"  cat <ruta>   ensena lo que hay dentro\n");
                                salida.texto(b"  write <ruta> <texto>     lo guarda\n");
                                salida.texto(b"  clear / cls  limpia esta salida\n");
                                salida.texto(b"  TAB          completa   Ctrl+A/E inicio/fin\n");
                                salida.texto(b"  Ctrl+K corta al final    Ctrl+W borra palabra\n");
                                salida.texto(b"  Ctrl+U borra linea       Ctrl+L limpia\n");
                                salida.texto(b"  info         RAM, CPU, tareas y disco\n");
                                salida.texto(b"  cpu / mem    solo esa parte del informe\n");
                                salida.texto(b"  perf         lo que cuesta pintar, medido\n");
                                salida.texto(b"  estratos sellar   ESCRIBE EN EL DISCO (commit vacio)\n");
                                salida.texto(b"  help         esto\n");
                                salida.texto(b"  reboot       reinicia la maquina\n");
                                salida.texto(b"  Ctrl+Alt     esconde o invoca esta ventana\n");
                                pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                                n = 0;
                            }
                            // Ni se intenta lanzar. Se dice lo que es y con
                            // que se abre — un mensaje sobre la FIRMA aqui
                            // manda a buscar un permiso que no hace falta.
                            Orden::NoEsPrograma(r) => {
                                salida.con_tinta(TINTA_MAL);
                                salida.texto(b"  eso no es un programa (solo .bex se lanza).\n");
                                salida.texto(b"  para verlo:  cat ");
                                salida.texto(r);
                                salida.byte(b'\n');
                                salida.con_tinta(TINTA_NORMAL);
                                pintar_estado(&p, &caja, "no es un programa: prueba lee", TEXTO_TENUE);
                                n = 0;
                            }
                            Orden::Informe => {
                                informe_sistema(&mut salida);
                                pintar_estado(&p, &caja, "informe del sistema", TEXTO_TENUE);
                                n = 0;
                            }
                            Orden::Cpu => {
                                informe_cpu(&mut salida);
                                pintar_estado(&p, &caja, "procesador", TEXTO_TENUE);
                                n = 0;
                            }
                            Orden::Memoria => {
                                informe_memoria(&mut salida);
                                pintar_estado(&p, &caja, "memoria", TEXTO_TENUE);
                                n = 0;
                            }
                            // Se pinta ANTES de pedirlo: la llamada no vuelve,
                            // asi que un mensaje despues no lo veria nadie. Y
                            // que quede escrito distingue "reinicio pedido" de
                            // "se colgo" en la foto. `Pantalla` escribe directo
                            // al framebuffer, asi que al volver de `texto` ya
                            // esta en el cristal: no hay nada que vaciar.
                            Orden::Reiniciar => {
                                salida.texto(b"  reiniciando...\n");
                                pintar_estado(&p, &caja, "reiniciando", TEXTO_TENUE);
                                bmo::reiniciar();
                            }
                            Orden::Desconocida => {
                                // El mensaje honesto. Antes se contestaba "no
                                // esta: revisa la ruta" a quien escribía
                                // `reboot`, y eso manda a buscar un archivo que
                                // nunca existió en vez de decir la verdad.
                                salida.texto(b"  no es un comando ni una ruta. escribe 'help'.\n");
                                pintar_estado(&p, &caja, "no lo conozco: prueba help", TEXTO_MAL);
                                n = 0;
                            }
                            Orden::Lanzar(objetivo) => {
                                let cap = salida_cap.as_ref().map(|c| c.cap).unwrap_or(0);
                                match bmo::ejecutar_en(objetivo, cap) {
                                    Ok(_) => {
                                        pintar_estado(&p, &caja, "lanzado", TEXTO_BIEN);
                                        // El campo se vacía al lanzar, como el
                                        // Win+R: la caja está para el SIGUIENTE
                                        // programa, no para admirar el anterior.
                                        n = 0;
                                    }
                                    Err(bmo::ERROR_NO_ESTA) => {
                                        pintar_estado(&p, &caja, "no esta: revisa la ruta", TEXTO_MAL)
                                    }
                                    Err(bmo::ERROR_GATE) => pintar_estado(
                                        &p,
                                        &caja,
                                        "rechazado: la firma no cuadra",
                                        TEXTO_MAL,
                                    ),
                                    Err(bmo::ERROR_OCUPADO) => {
                                        pintar_estado(&p, &caja, "no hay hueco ahora mismo", TEXTO_MAL)
                                    }
                                    Err(_) => {
                                        pintar_estado(&p, &caja, "no paso la admision", TEXTO_MAL)
                                    }
                                }
                            }
                        }
                        // El cursor detras de la linea, SIEMPRE. Las ramas que
                        // vacian el campo ponian `n = 0` y dejaban `cur` donde
                        // estaba: la tecla siguiente se escribia en `ruta[cur]`
                        // —fuera de lo que se dibuja— y el campo ensenaba los
                        // bytes VIEJOS del comando anterior. Escribir `2` tras
                        // `run apps/calc.bex` mostraba una `r`. Las ramas de
                        // error conservan la ruta a proposito para poder
                        // corregirla, y ahi `cur` no se mueve: por eso es un
                        // `min` y no un cero.
                        cur = cur.min(n);
                        repintar_campo = true;
                    }
                    // TAB: completar.
                    b'\t' => {
                        let antes = n;
                        n = completar(&mut ruta, n, &mut salida);
                        cur = n;
                        if n == antes {
                            pintar_estado(&p, &caja, "nada que completar", TEXTO_TENUE);
                        }
                        repintar_campo = true;
                    }
                    // Retroceso.
                    0x08 | 0x7F => {
                        if cur > 0 {
                            let mut k = cur;
                            while k < n {
                                ruta[k - 1] = ruta[k];
                                k += 1;
                            }
                            cur -= 1;
                            n -= 1;
                            repintar_campo = true;
                        }
                    }
                    // Escape: borrar la línea entera, igual que en el Win+R.
                    0x1B => {
                        n = 0;
                        cur = 0;
                        pintar_estado(&p, &caja, "listo", TEXTO_TENUE);
                        repintar_campo = true;
                    }
                    // ── El portapapeles ──
                    //
                    // Ctrl+C copia la línea entera; Ctrl+V la pega donde esté
                    // el cursor. No es un lujo: la mitad de lo que se teclea en
                    // un terminal es una variación de lo anterior, y sin copiar
                    // hay que reescribirlo todo.
                    //
                    // Ctrl+C para copiar y no para interrumpir, que es lo que
                    // significa en Unix. Aquí no hay señales que mandar, y el
                    // dedo que ya sabe Ctrl+C sabe copiar — no interrumpir.
                    0x03 => {
                        porta_n = n;
                        porta[..n].copy_from_slice(&ruta[..n]);
                        pintar_estado(&p, &caja, "copiado", TEXTO_TENUE);
                    }
                    0x16 => {
                        if porta_n > 0 && n + porta_n <= RUTA_MAX {
                            // Hueco del tamaño del pegado, y meterlo.
                            let mut k = n;
                            while k > cur {
                                ruta[k + porta_n - 1] = ruta[k - 1];
                                k -= 1;
                            }
                            ruta[cur..cur + porta_n].copy_from_slice(&porta[..porta_n]);
                            cur += porta_n;
                            n += porta_n;
                            repintar_campo = true;
                        }
                    }
                    // Ctrl+U — borra la línea. Ctrl+L — borra la salida.
                    // Los mismos que el shell de Ring 0, porque los dedos ya
                    // los tienen y un atajo que cambia entre dos ventanas del
                    // mismo sistema es peor que no tenerlo.
                    0x15 => {
                        n = 0;
                        cur = 0;
                        repintar_campo = true;
                    }
                    0x0C => {
                        salida.limpiar();
                        repintar_campo = true;
                    }
                    // FLECHA ARRIBA / ABAJO — el historial. Llegan por la misma
                    // cola que las letras, con bytes del rango C1 (0x80..0x9F)
                    // que no tienen glifo: el driver los eligió justo para que
                    // no puedan confundirse con texto.
                    // Ctrl+ARRIBA copia, Ctrl+ABAJO pega. Lo mismo que
                    // Ctrl+C / Ctrl+V, con las flechas — porque los dedos que
                    // ya andan por el historial no tienen que irse a buscar
                    // otra tecla para copiar lo que acaban de recuperar.
                    0x80 if ctrl => {
                        porta_n = n;
                        porta[..n].copy_from_slice(&ruta[..n]);
                        pintar_estado(&p, &caja, "copiado", TEXTO_TENUE);
                    }
                    0x81 if ctrl => {
                        if porta_n > 0 && n + porta_n <= RUTA_MAX {
                            let mut k = n;
                            while k > cur {
                                ruta[k + porta_n - 1] = ruta[k - 1];
                                k -= 1;
                            }
                            ruta[cur..cur + porta_n].copy_from_slice(&porta[..porta_n]);
                            cur += porta_n;
                            n += porta_n;
                            repintar_campo = true;
                        }
                    }
                    0x80 => {
                        if let Some(k) = historial.atras(&mut ruta) {
                            n = k;
                            cur = k;
                            repintar_campo = true;
                        }
                    }
                    0x81 => {
                        if let Some(k) = historial.adelante(&mut ruta) {
                            n = k;
                            cur = k;
                            repintar_campo = true;
                        }
                    }
                    // IZQUIERDA / DERECHA — mover el cursor.
                    0x82 => {
                        if cur > 0 { cur -= 1; repintar_campo = true; }
                    }
                    0x83 => {
                        if cur < n { cur += 1; repintar_campo = true; }
                    }
                    // INICIO / FIN.
                    0x84 => { cur = 0; repintar_campo = true; }
                    0x85 => { cur = n; repintar_campo = true; }
                    // ── Los atajos de edicion de linea ──
                    //
                    // Los de toda la vida en una consola: Ctrl+A al principio,
                    // Ctrl+E al final, Ctrl+K corta hasta el final, Ctrl+W
                    // borra la palabra de atras. Van ADEMAS de Inicio/Fin, que
                    // ya estaban: los dedos que vienen de un terminal buscan
                    // estos, y los que vienen de Windows buscan aquellos.
                    // Atender a los dos cuesta cuatro lineas.
                    0x01 => { cur = 0; repintar_campo = true; }
                    0x05 => { cur = n; repintar_campo = true; }
                    // Ctrl+K: tirar lo que hay del cursor al final.
                    0x0B => {
                        n = cur;
                        repintar_campo = true;
                    }
                    // Ctrl+W: borrar la palabra de atras. Primero se comen los
                    // espacios y luego las letras, que es lo que espera
                    // cualquiera que lo haya usado — si no, borrar tras un
                    // espacio no haria nada.
                    0x17 => {
                        let mut k = cur;
                        while k > 0 && ruta[k - 1] == b' ' { k -= 1; }
                        while k > 0 && ruta[k - 1] != b' ' { k -= 1; }
                        let quitados = cur - k;
                        if quitados > 0 {
                            let mut i = cur;
                            while i < n {
                                ruta[i - quitados] = ruta[i];
                                i += 1;
                            }
                            n -= quitados;
                            cur = k;
                            repintar_campo = true;
                        }
                    }
                    // SUPRIMIR — borra HACIA ADELANTE, al reves que el
                    // retroceso. Son dos teclas porque son dos intenciones.
                    0x86 => {
                        if cur < n {
                            let mut k = cur + 1;
                            while k < n { ruta[k - 1] = ruta[k]; k += 1; }
                            n -= 1;
                            repintar_campo = true;
                        }
                    }
                    // ★ PgUp / PgDn — el historial de la salida.
                    //
                    // Estaban ignoradas "explicitamente", que era honesto pero
                    // inutil: lo que salia por arriba se perdia para siempre, y
                    // en una maquina donde depurar es fotografiar la pantalla,
                    // perder la salida de un batch cuesta un arranque entero.
                    // Ahora suben y bajan la ventana sobre 200 filas guardadas.
                    0x87 => {
                        salida.mover_vista(SAL_ROWS as i32 - 1);
                    }
                    0x88 => {
                        salida.mover_vista(-(SAL_ROWS as i32 - 1));
                    }
                    // ★ F12 (0x94) NO esta aqui: se atiende arriba, antes de
                    // preguntar por el foco, porque es del sistema y no de esta
                    // ventana. Ver la conmutacion de la consola de datos.
                    //
                    // El resto de navegación se ignora, pero EXPLÍCITAMENTE:
                    // dejarlas caer al comodín las dibujaría como basura.
                    0x89..=0x9F => {}
                    // Todo lo demás imprimible, incluido el Latin-1 alto: la
                    // `ñ` llega como 0xF1 y la fuente la tiene.
                    c if c >= 0x20 => {
                        if n < RUTA_MAX {
                            // Hueco en el cursor y meter ahi: escribir en
                            // medio de una linea es lo normal, no un caso raro.
                            let mut k = n;
                            while k > cur {
                                ruta[k] = ruta[k - 1];
                                k -= 1;
                            }
                            ruta[cur] = c;
                            cur += 1;
                            n += 1;
                            repintar_campo = true;
                        }
                    }
                    _ => {}
                }
            }

            // ── Ratón ──
            // La rueda, primero: mueve el historial de la salida. Es lo que
            // pidio Eddi —"ver y scrollear"— y funciona con la rueda o con
            // PgUp/PgDn, porque un teclado siempre hay.
            // ★ La rueda se atiende MAS ABAJO, cuando ya se sabe sobre que
            // ventana esta el puntero. Antes se atendia aqui y siempre movia el
            // historial de salida: con la consola del kernel abierta y encima,
            // girar la rueda desplazaba una rejilla que ni siquiera se veia.
            //
            // Ver `bajo_el_puntero`.
            // ── Los botones de la calculadora ──
            let boton = pos.botones != 0;
            if calc.visible && boton && !boton_antes && !calc.esperando {
                if let Some(t) = calc_caja.tecla_en(pos.x, pos.y) {
                    match t {
                        b'C' => calc.limpiar(),
                        b'+' => calc.operador(1),
                        b'-' => calc.operador(2),
                        b'*' => calc.operador(3),
                        b'/' => calc.operador(4),
                        b'=' => {
                            if calc.op != 0 && calc.guardado_n > 0 && calc.n > 0 {
                                // Lanzar el MOTOR y darle los tres datos por su
                                // consola. Aqui es donde la cara deja de saber
                                // de aritmetica y empieza a saber COBOL.
                                let cap = salida_cap.as_ref().map(|c| c.cap).unwrap_or(0);
                                if bmo::ejecutar_en(b"cobol/calcgui.bex", cap).is_ok() {
                                    if let Some(cc) = salida_cap.as_ref() {
                                        cc.escribir(&calc.guardado[..calc.guardado_n]);
                                        cc.escribir(b"\n");
                                        cc.escribir(&[b'0' + calc.op]);
                                        cc.escribir(b"\n");
                                        cc.escribir(&calc.entrada[..calc.n]);
                                        cc.escribir(b"\n");
                                    }
                                    calc.esperando = true;
                                    resp_n = 0;
                                } else {
                                    pintar_estado(&p, &caja, "falta cobol/calcgui.bex", TEXTO_MAL);
                                }
                            }
                        }
                        d => calc.meter(d),
                    }
                    pintar_calc(&p, &calc_caja, &calc, calc_encima);
                }
            }
            // ── El raton tambien manda en el foco ──
            //
            // Sin esto, dos de los tres modos son decoracion: `click-to-focus`
            // no existiria y `focus-follows-mouse` no tendria quien le dijera
            // por donde va el puntero.
            //
            // ★ El orden de estas dos preguntas ES el Z-order: Datos se pinta
            // ENCIMA de Ejecutar, asi que se pregunta primero, y un clic en la
            // zona compartida es de la de arriba. `bmo_input::foco` no sabe que
            // ventana tapa a cual y no tiene por que: eso lo sabe el que pinta.
            // ★ Con TRES ventanas, el orden de las preguntas deja de caber en
            // un `if/else` escrito a mano por pares. Se pregunta primero por la
            // que está ARRIBA —sea cual sea— y después por las demás: un clic
            // en la zona compartida es siempre de la de encima, y eso es una
            // regla, no una lista de casos.
            let en = |v: u8| match v {
                V_DATOS => datos_abierta && caja_datos.contiene(pos.x, pos.y),
                V_KLOG => klog_abierta && caja_klog.contiene(pos.x, pos.y),
                _ => visible && caja.contiene(pos.x, pos.y),
            };
            let bajo_el_puntero = if en(arriba_antes) {
                Some(arriba_antes)
            } else {
                [V_KLOG, V_DATOS, V_EJECUTAR]
                    .into_iter()
                    .find(|&v| v != arriba_antes && en(v))
            };
            // ── ★ LA RUEDA VA A LA VENTANA QUE HAY DEBAJO ──
            //
            // Es lo que hace cualquier sistema y lo que la mano espera sin
            // pensarlo: se gira donde se mira. Antes iba SIEMPRE al historial
            // de salida, así que con la consola del kernel delante la rueda
            // movía una rejilla tapada — el gesto no hacía nada visible y
            // parecía que la rueda no funcionaba.
            //
            // Sin ventana debajo no se hace nada, y eso también es una
            // decisión: mandar el giro a la ventana con el foco cuando el
            // puntero está en el escritorio mueve cosas que no se están
            // mirando.
            if giro != 0 {
                match bajo_el_puntero {
                    Some(V_KLOG) => {
                        // Positivo es hacia arriba, y en un log "arriba" es
                        // hacia ATRÁS en el tiempo: el desplazamiento cuenta
                        // líneas hacia el pasado, así que suma.
                        let hay = bmo::klog_lineas();
                        let paso = (giro * 3) as i64;
                        let nuevo = klog_desplazamiento as i64 + paso;
                        klog_desplazamiento =
                            nuevo.clamp(0, hay.saturating_sub(1) as i64) as u64;
                        escena::klog::pintar(&p, &caja_klog, klog_desplazamiento);
                    }
                    Some(V_EJECUTAR) => {
                        // Tres filas por muesca: una sola se queda corta y una
                        // página entera se pasa. Es el paso de un terminal.
                        salida.mover_vista(giro * 3);
                    }
                    // La consola de datos no tiene nada que desplazar todavía:
                    // cabe entera. Cuando enseñe el árbol de nodos, aquí entra.
                    _ => {}
                }
            }

            // ── El realce de la calculadora ──
            //
            // Sólo cuando CAMBIA la tecla señalada, y sólo si la calculadora se
            // ve y no está tapada. Al salir de ella el realce se apaga, que es
            // la mitad que se olvida siempre: un botón que se queda encendido
            // cuando ya no lo señalas miente sobre dónde está el ratón.
            let encima_ahora = if calc.visible && arriba_antes == V_EJECUTAR {
                calc_caja.tecla_en(pos.x, pos.y)
            } else {
                None
            };
            if encima_ahora != calc_encima {
                calc_encima = encima_ahora;
                if calc.visible {
                    pintar_calc(&p, &calc_caja, &calc, calc_encima);
                }
            }

            if let Some(v) = bajo_el_puntero {
                // Pasar por encima: solo hace algo en modo `Puntero`, y la
                // guarda esta DENTRO de la politica — aqui solo se cuenta lo
                // que pasa, no se decide lo que significa.
                if pos.x != ax || pos.y != ay {
                    foco.puntero_en(v);
                }
                // Un clic lo pide en CUALQUIER modo, incluido `Fijo`: lo que
                // ese modo impide es que una ventana se lo tome sin que nadie
                // se lo pida, no que tu se lo des.
                if boton && !boton_antes {
                    foco.clic_en(v);
                }
            }
            boton_antes = boton;

            // ── ★ El foco arrastra el Z-order ──
            //
            // Levantar una ventana no da el teclado —eso es mezclar dos cosas y
            // es el error clasico de un gestor de ventanas—, pero **al reves si
            // vale**: la que tiene el teclado tiene que verse. Aqui no hay
            // recorte, asi que "verse" es pintarse la ultima.
            //
            // Sin esto, Alt+Tab a Ejecutar con Datos delante dejaria el teclado
            // en una linea tapada: escribirias sin ver nada. Es exactamente el
            // fallo que se acaba de arreglar, del reves.
            let arriba = if klog_abierta && foco.es_para(V_KLOG) {
                V_KLOG
            } else if datos_abierta && foco.es_para(V_DATOS) {
                V_DATOS
            } else {
                V_EJECUTAR
            };
            if arriba != arriba_antes {
                match arriba {
                    V_KLOG => escena::klog::pintar(&p, &caja_klog, klog_desplazamiento),
                    V_DATOS => escena::datos::pintar(&p, &caja_datos),
                    _ if visible => {
                        pintar_caja(&p, &caja);
                        repintar_campo = true;
                        salida.sucia = true;
                    }
                    _ => {}
                }
                arriba_antes = arriba;
            }

            // El cursor ya no se borra aquí: se pone al final del fotograma y
            // se quita al principio del siguiente, con lo que había debajo
            // guardado. Aquí sólo se apunta dónde está.
            ax = pos.x;
            ay = pos.y;

            // El pulsómetro. Se satura a propósito: interesa "late / no late",
            // no el valor exacto, y una barra que se sale de la pantalla no
            // dice nada que no diga una llena.
            if ev_raton != pulso_previo {
                p.rect(PULSO_X, PULSO_Y, ev_raton, PULSO_ALTO, ACENTO);
                pulso_previo = ev_raton;
            }
            // Los botones, encima del marco: pulsar debería verse aunque el
            // movimiento no llegue. Son dos preguntas distintas al mismo HID.
            //
            // Sólo cuando CAMBIA: repintarlo cada vuelta son 256 píxeles de
            // memoria de vídeo sin caché por fotograma para dejarlo igual, y
            // además pisaría el cursor si el puntero pasara por encima.
            let col = if pos.botones != 0 { 0x00FF_FFFF } else { FONDO };
            if col != col_boton {
                p.rect(PULSO_X + PULSO_ANCHO + 16, PULSO_Y, PULSO_ALTO, PULSO_ALTO, col);
                col_boton = col;
            }
        }

        // ── Drenar la salida de los hijos ──
        //
        // Con tope por fotograma. Un programa que escupe sin parar podría
        // quedarse con el bucle entero y congelar el cursor: es preferible que
        // la salida vaya un poco por detrás a que el escritorio deje de
        // responder. Lo que no se lea ahora sigue en el anillo del kernel.
        if let Some(c) = salida_cap.as_ref() {
            let mut buf = [0u8; 8];
            let mut vueltas = 0;
            while vueltas < 64 {
                let leidos = c.leer(&mut buf);
                if leidos == 0 {
                    break;
                }
                if calc.esperando {
                    // Todo lo que escriba el motor es la respuesta: el
                    // programa no imprime prompts a proposito.
                    for &b in &buf[..leidos] {
                        if b == b'\n' {
                            if resp_n > 0 {
                                calc.entrada = [0; 20];
                                let k = resp_n.min(calc.entrada.len());
                                calc.entrada[..k].copy_from_slice(&resp[..k]);
                                calc.n = k;
                                calc.guardado_n = 0;
                                calc.op = 0;
                                calc.esperando = false;
                                // ★ El cursor SE APARTA antes de pintar aquí.
                                //
                                // Éste es el único pintado del bucle que no
                                // dispara la ENTRADA: lo dispara el HIJO al
                                // contestar. Así que puede caer en un fotograma
                                // con `va_a_pintar` en falso — o sea con el
                                // puntero todavía en pantalla y lo que hay
                                // debajo ya guardado. Pintar encima **caduca**
                                // ese guardado, y el `quitar` de la vuelta
                                // siguiente devolvería los píxeles viejos
                                // encima del resultado recién escrito: un
                                // rectángulo fantasma sobre la calculadora.
                                //
                                // `quitar` es idempotente —si no está puesto no
                                // hace nada—, así que llamarlo aquí no cuesta
                                // nada en los fotogramas que ya lo apartaron.
                                bajo.quitar(&p);
                                pintar_calc(&p, &calc_caja, &calc, calc_encima);
                            }
                        } else if resp_n < resp.len() && b >= 0x20 {
                            resp[resp_n] = b;
                            resp_n += 1;
                        }
                    }
                } else {
                    salida.texto(&buf[..leidos]);
                }
                vueltas += 1;
            }
        }
        // ★ Y sólo en un fotograma que haya apartado el cursor. Un hijo que
        // escribe no es motivo suficiente: pintar aquí dejaría el puntero
        // enterrado bajo la rejilla y, al quitarlo, devolvería píxeles viejos
        // encima de lo recién escrito. `sucia` se queda puesto y la vuelta
        // siguiente ya empieza sabiendo que hay que pintar.
        if salida.sucia && va_a_pintar {
            // Se pinta sólo si se ve; el contenido sigue acumulándose oculto,
            // así que al invocar la ventana está todo lo que pasó mientras.
            //
            // ★ Y NO si la consola de datos está ARRIBA. Sin este guardia, el
            // fotograma siguiente repintaría la rejilla POR DEBAJO y la
            // dibujaría encima de la ventana de datos, dejándola a trozos. La
            // salida no se pierde: `sucia` se queda puesto y se pinta entera
            // cuando esta ventana vuelva a estar arriba.
            //
            // Y es ARRIBA, no ABIERTA: con Datos abierta pero detrás, la
            // rejilla se ve y tiene que seguir escribiéndose.
            if visible && arriba_antes != V_DATOS && !conmutador_pintado {
                pintar_salida(&p, &caja, &salida);
                salida.sucia = false;
            } else if !visible {
                salida.sucia = false;
            }
        }

        // El parpadeo del cursor de escritura. Sólo repinta cuando cambia de
        // estado — repintar el campo cada vuelta sería reescribir la ruta
        // miles de veces por segundo para que se vea igual.
        //
        // ★ El contador se REINICIA con cada tecla (ver el manejador). Antes
        // era `vueltas % PARPADEO`, un reloj que corría solo: si te ponías a
        // escribir justo cuando tocaba apagarlo, el cursor desaparecía a mitad
        // de la palabra y no volvía hasta la siguiente vuelta entera. Un
        // cursor que se esconde mientras escribes es lo contrario de lo que
        // un cursor existe para decir.
        desde_tecla = desde_tecla.wrapping_add(1);
        if desde_tecla >= PARPADEO {
            desde_tecla = 0;
            caret = !caret;
            repintar_campo = true;
        }
        if repintar_campo
            && va_a_pintar
            && visible
            && arriba_antes != V_DATOS
            && !conmutador_pintado
        {
            pintar_campo(&p, &caja, &ruta[..n], cur, caret);
        }

        // ★ UNA sola vez, al cerrar el primer fotograma entero. Con esto, las
        // últimas palabras que guarda el kernel dicen DÓNDE murió sin tener que
        // adivinarlo:
        //
        //   "reclamo pantalla y entrada"  -> murió en el arranque o en la intro
        //   "escritorio pintado"          -> murió sin cerrar el primer cuadro
        //   "primer fotograma completo"   -> murió ya en el bucle
        //
        // Tres mensajes que ya existían más éste, y el diagnóstico deja de ser
        // una teoría. Cuesta una línea en el log y se dice una vez en la vida
        // del proceso.
        if vueltas == 1 {
            bmo::consola("primer fotograma completo\n");
        }

        // ── El cursor del ratón, ENCIMA de todo y lo último ──
        //
        // Aquí ya no queda nada por pintar en este fotograma, así que lo que se
        // guarda debajo es lo definitivo. Ponerlo antes obligaría a que cada
        // ventana supiera esquivarlo — que es justo lo que no se puede pedir a
        // una ventana que todavía no existe.
        if ax != u32::MAX {
            // ★ QUÉ ESTÁ DICIENDO EL PUNTERO.
            //
            // Se decide aquí, al final del fotograma, porque es aquí donde ya
            // se sabe todo lo que pasó en él: qué ventana quedó arriba, dónde
            // acabó el ratón y si la calculadora está abierta.
            //
            // El orden de las preguntas es el Z-order otra vez: lo que está
            // encima manda. Un botón de la calculadora tapado por la consola
            // del kernel no puede pedir la mano — señalaría algo que no se
            // puede pulsar, que es peor que no señalar nada.
            let forma = if calc.visible
                && arriba_antes == V_EJECUTAR
                && calc_caja.tecla_en(ax, ay).is_some()
            {
                escena::cursor::Forma::Mano
            } else if visible && arriba_antes == V_EJECUTAR && caja.en_campo(ax, ay) {
                escena::cursor::Forma::Texto
            } else {
                escena::cursor::Forma::Flecha
            };
            bajo.poner(&p, ax, ay, forma);
        }

        // ★ Y ahora EMPUJARLO a la pantalla.
        //
        // El framebuffer está mapeado en write-combining: el CPU acumula las
        // escrituras y las suelta cuando el búfer se llena. Sin esta línea, lo
        // pintado en este fotograma se queda esperando a que alguien escriba
        // más — y el síntoma es exactamente el que apareció en el Ryzen:
        // teclear no pintaba nada hasta que se movía el ratón, porque mover el
        // ratón era lo que llenaba el búfer.
        //
        // Una instrucción, una vez por fotograma, al final de todo. Ver
        // `Pantalla::vaciar`.
        p.vaciar();

        bmo::ceder();
    }
}

/// Un pánico aquí no puede tumbar nada más que a este proceso: lo dice y sale
/// por la puerta normal. El kernel revoca sus capabilities —incluidas la
/// pantalla y la entrada— y sigue vivo.
///
/// ★ **Y DICE DÓNDE.** Esto era `_info` —el guion bajo delata el bug— y
/// escribía "panico en el compositor" y nada más. El escritorio se moría al
/// arrancar, dejaba la máquina en el shell de Ring 0, y el único que sabía el
/// archivo y la línea era este manejador, que los tiraba.
///
/// Se escribe en la consola del KERNEL a propósito: cuando esto corre, la
/// pantalla puede estar reclamada por nosotros y a medio pintar, así que el
/// único sitio donde el mensaje sobrevive es el panel del kernel — que es
/// justo donde se queda la máquina cuando el escritorio no arranca.
#[panic_handler]
fn panico(info: &core::panic::PanicInfo) -> ! {
    bmo::consola("panico en el compositor\n");
    if let Some(l) = info.location() {
        bmo::consola("  en ");
        bmo::consola(l.file());
        bmo::consola(":");
        // El número a mano: aquí no hay `format!` ni asignador.
        let mut buf = [0u8; 12];
        let mut n = 0usize;
        let mut v = l.line();
        if v == 0 {
            buf[0] = b'0';
            n = 1;
        } else {
            let mut d = [0u8; 12];
            let mut k = 0;
            while v > 0 {
                d[k] = b'0' + (v % 10) as u8;
                v /= 10;
                k += 1;
            }
            while k > 0 {
                k -= 1;
                buf[n] = d[k];
                n += 1;
            }
        }
        if let Ok(s) = core::str::from_utf8(&buf[..n]) {
            bmo::consola(s);
        }
        bmo::consola("\n");
    }
    bmo::salir()
}

