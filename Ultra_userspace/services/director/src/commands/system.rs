//! **Commands that ask the KERNEL**: `info`, `cpu`, `mem`, `autopsia`, `smp`,
//! `red`, `audio`, `reboot`.
//!
//! None of them exercises a privilege -- counting RAM is not power. They come
//! down through `OP_INFO` and its siblings and are painted here, which is
//! where the screen is. Several of these used to exist ONLY in the Ring 0
//! shell, and two shells with two vocabularies are two products.

use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::commands::reports::{report_autopsy, report_cpu, report_memory, report_net, report_system};
use crate::scene::output::{INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK, INK_DIM};
use crate::text::decimal;
use crate::paint_output;

/// * `sella` YA NO VIVE AQUI, y esto lo dice.
///
/// La orden se mudo a la ventana de ESTRATOS porque
/// el verbo vive donde vive el objeto. Borrarla y
/// contestar "no lo conozco" habria sido correcto y
/// cruel: estaba escrita en la linea de ayuda de
/// ayer, en dos documentos y en la costumbre del
/// dueno. **Una funcion que se muda sin dejar nota se
/// convierte en una funcion que desaparecio.**
pub(crate) fn seal_moved(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    dsk.out.grid.text(b"  sellar se mudo a la ventana de ESTRATOS.\n");
    dsk.out.grid.with_ink(INK_GOOD);
    dsk.out.grid.text(b"  F12  ->  TAB  ->  tecla S\n");
    dsk.out.grid.with_ink(INK_PLAIN);
    dsk.out.grid.text(b"  ahi se ve el volumen mientras se sella, que es\n");
    dsk.out.grid.text(b"  donde tiene sentido: la generacion sube delante.\n");
    paint_status(&p, &dsk.run_box, "esta en F12", INK);
    dsk.field.n = 0;
    After::Settle
}

/// ** `audio` -- paso 0 de docs/maestro/AUDIO_MAESTRO.md.
///
/// La orden existia SOLO en el shell de Ring 0 y el
/// dueno la escribio aqui, que es donde se trabaja.
/// Contesto "no es un comando ni una ruta" y la
/// prueba se quedo sin hacer. Dos shells con dos
/// vocabularios distintos son dos productos.
/// ** LA RED -- siete campos que hasta hoy no
/// cruzaban a Ring 3. Mismo criterio que `audio`: la
/// orden existia SOLO en el shell de Ring 0, y dos
/// shells con dos vocabularios son dos productos.
/// **`estratos escribe <nombre> <texto>`** -- el primer fichero que BMO-X
/// guarda en SU sistema de ficheros.
///
/// === Por que el sustantivo va delante ===
///
/// Porque ya hay un `escribe` y va a la FAT32. Dos ordenes con el mismo verbo
/// escribiendo en dos volumenes distintos es como se guarda algo donde no se
/// queria -- y aqui uno de los dos volumenes es el que Windows sabe leer y el
/// otro no. Mismo criterio que `disco`: nombrar el objeto antes de darle una
/// orden.
///
/// === Lo que cuesta, dicho ANTES ===
///
/// Cuatro bloques: el nodo del fichero, las entradas nuevas, el directorio nuevo
/// y el estrato. Los tres ultimos no son del fichero, son **la version nueva del
/// arbol** -- en ESTRATOS no se toca nada, se copia lo que cambia. Se dice
/// porque `log_head` sube de cuatro en cuatro y quien mire la ocupacion tiene
/// que saber por que.
///
/// [!] Y el techo de hoy son **96 bytes**: lo que cabe dentro del nodo. Mas
/// grande pide un arbol de bloques con sus niveles, que es otra funcion y otra
/// tanda. Se dice aqui en vez de dejar que el kernel conteste un cero.
pub(crate) fn estratos_escribe(
    dsk: &mut Desktop,
    p: &bmo::Pantalla,
    nombre: &[u8],
    texto: &[u8],
) -> After {
    let s = &mut dsk.out.grid;
    if texto.len() > bmo::estratos::ES_GESTO_MAX as usize {
        s.with_ink(INK_ERR);
        s.text(b"  no cabe: hoy un fichero de ESTRATOS entra en 96 bytes\n");
        s.with_ink(INK_PLAIN);
        s.text(b"  (mas grande pide un arbol de bloques, y es otra tanda)\n");
        paint_status(p, &dsk.run_box, "no cabe", INK_DIM);
        dsk.field.n = 0;
        return After::Settle;
    }
    // El aviso va ANTES de llamar, como en `smp` y en el recorte: la llamada
    // escribe cuatro bloques y hace dos barreras, y un mensaje escrito despues
    // no explica una espera que ya paso.
    s.text(b"  escribiendo en ESTRATOS (4 bloques y dos barreras)...\n");
    paint_output(p, &dsk.run_box, &dsk.out.grid);
    p.volcar();

    let g = bmo::estratos::crear_fichero(nombre, texto);
    let s = &mut dsk.out.grid;
    if g == 0 {
        s.with_ink(INK_ERR);
        s.text(b"  NO se guardo\n");
        s.with_ink(INK_PLAIN);
        // Los cuatro que se ven en la practica. El motivo exacto lo dice el
        // kernel en F11: aqui no se adivina cual fue.
        s.text(b"  el nombre ya existe / la carpeta esta llena (36) /\n");
        s.text(b"  la escritura esta cerrada / no hay volumen.  F11 dice cual.\n");
    } else {
        s.with_ink(INK_GOOD);
        s.text(b"  GUARDADO. generacion ");
        s.dec(g);
        s.with_ink(INK_PLAIN);
        s.text(b"
  F12 lo ensena, y tras reiniciar tiene que seguir ahi.\n");
    }
    paint_status(p, &dsk.run_box, "estratos", INK_DIM);
    dsk.field.n = 0;
    dsk.field.cur = 0;
    After::NextKey
}

pub(crate) fn net(dsk: &mut Desktop, _p: &bmo::Pantalla, what: &[u8]) -> After {
    // *** `net rx` ARMA EL RECEPTOR, y desde aqui (2026-08-24).
    //
    // ** Hasta hoy esto solo informaba y el panel mandaba al shell de Ring 0.
    // Y al shell de Ring 0 no se vuelve: un camino que solo existe alli es un
    // camino que el dueno de su propia maquina no puede tomar.
    //
    // [!] Y no es "el escritorio toca la NIC": es `bmo::red::armar()`, o sea
    // Ring 3 PIDE y el kernel DECIDE -- la misma forma que el disco. Ninguna
    // operacion de esa puerta puede transmitir: `CR.TE` se queda apagado.
    if what == b"rx" {
        match bmo::red::armar() {
            bmo::red::Armado::Ok => {
                let n = bmo::red::sondear();
                dsk.out.grid.with_ink(INK_GOOD);
                dsk.out.grid.text(b"  receptor ARMADO
");
                dsk.out.grid.with_ink(INK_PLAIN);
                dsk.out.grid.text(b"  tramas en esta vuelta: ");
                dsk.out.grid.dec(n);
                dsk.out.grid.text(b"
");
                if n == 0 {
                    // ** CERO EN LA PRIMERA VUELTA ES LO ESPERADO, y decirlo es
                    // lo que impide que el minuto siguiente se gaste buscando un
                    // fallo en un driver que funciona.
                    dsk.out.grid.with_ink(INK_PLAIN);
                    dsk.out.grid.text(b"  cero de momento es normal: el anillo se acaba de armar.
");
                    dsk.out.grid.text(b"  vuelve a escribir `net rx` en unos segundos.
");
                    dsk.out.grid.with_ink(INK_PLAIN);
                }
            }
            // ** Cada motivo por separado, porque mandan a mirar sitios
            // distintos. "No funciona" no dice cual de los tres.
            bmo::red::Armado::SinEnlace => {
                dsk.out.grid.with_ink(INK_PLAIN);
                dsk.out.grid.text(b"  el enlace esta ABAJO: enchufa el cable antes de armar nada.
");
                dsk.out.grid.with_ink(INK_PLAIN);
            }
            bmo::red::Armado::NoArma => {
                dsk.out.grid.text(b"  el receptor no se pudo armar -- CABINA dice por que.
");
            }
            bmo::red::Armado::SinTarjeta => {
                dsk.out.grid.text(b"  no hay tarjeta que este kernel sepa leer.
");
            }
            bmo::red::Armado::Raro(v) => {
                dsk.out.grid.text(b"  el kernel contesto algo que no conozco: ");
                dsk.out.grid.dec(v);
                dsk.out.grid.text(b"
");
            }
        }
        return After::Settle;
    }
    report_net(&mut dsk.out.grid, what);
    After::Settle
}

/// **`audio [silencio | calla]`** -- el aparato, y el TUBO.
///
/// # Los dos numeros que hay que mirar, y por que estos
///
/// `AUDIO_MAESTRO` parte 7 lo deja escrito: **`tramas tarde` es la fila de esta
/// pagina**. Un audio que va bien y uno que chasquea se distinguen por ese
/// contador y por nada mas -- a oido son *"suena raro"* y *"suena bien"*, que no
/// es un diagnostico.
///
/// [!] Y `silencio` es TRAFICO, no configuracion: 250 latidos por segundo
/// empujando tramas al bus. Por eso se pide a proposito y no se enciende solo.
pub(crate) fn audio(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> After {
    if arg == b"silencio" || arg == b"prueba" {
        let ok = bmo::audio_tubo(1);
        dsk.out.grid.with_ink(if ok != 0 { INK_GOOD } else { INK_ERR });
        if ok != 0 {
            dsk.out.grid.text(b"  tubo ARMADO: empujando SILENCIO\n");
            dsk.out.grid.with_ink(INK_PLAIN);
            dsk.out.grid.text(b"  el silencio no puede sonar mal. Mira `audio` otra vez:\n");
            dsk.out.grid.text(b"  encoladas tiene que SUBIR y tarde quedarse en 0\n");
        } else {
            dsk.out.grid.text(b"  no hay tubo abierto que armar (mira `cabina`)\n");
            dsk.out.grid.with_ink(INK_PLAIN);
        }
        paint_status(&p, &dsk.run_box, "audio", INK_DIM);
        dsk.field.n = 0;
        return After::Settle;
    }
    if arg == b"calla" || arg == b"para" {
        bmo::audio_tubo(2);
        dsk.out.grid.text(b"  tubo callado\n");
        paint_status(&p, &dsk.run_box, "audio", INK_DIM);
        dsk.field.n = 0;
        return After::Settle;
    }

    let had_any = bmo::audio_censo();
    if had_any {
        dsk.out.grid.with_ink(INK_GOOD);
        dsk.out.grid.text(b"  aparato de reproduccion HALLADO\n");
        dsk.out.grid.with_ink(INK_PLAIN);
        dsk.out.grid.text(b"  los ocho numeros estan en F11 (canales, bits, frecuencias)\n");
        dsk.out.grid.text(b"  comparalos con lo que dice Windows del mismo audifono\n");
    } else {
        dsk.out.grid.with_ink(INK_ERR);
        dsk.out.grid.text(b"  ningun aparato de reproduccion en los puertos libres\n");
        dsk.out.grid.with_ink(INK_PLAIN);
        // La distincion que decide el siguiente paso, y por eso
        // se dice aqui y no solo en CABINA.
        dsk.out.grid.text(b"  F11 dice CUANTOS puertos se miraron: si es 0, el fallo\n");
        dsk.out.grid.text(b"  es del censo; si es >0, el aparato no es UAC1\n");
    }

    // ** EL TUBO, que es lo que decide si esto va a sonar.
    if bmo::audio_tubo(0) != 0 {
        dsk.out.grid.with_ink(INK_GOOD);
        dsk.out.grid.text(b"  TUBO ABIERTO\n");
        dsk.out.grid.with_ink(INK_PLAIN);
        dsk.out.grid.text(b"    frecuencia      ");
        dsk.out.grid.dec(bmo::audio_tubo(4));
        dsk.out.grid.text(b" Hz\n");
        dsk.out.grid.text(b"    bytes por trama ");
        dsk.out.grid.dec(bmo::audio_tubo(3));
        dsk.out.grid.text(b"\n");
        dsk.out.grid.text(b"    encoladas       ");
        dsk.out.grid.dec(bmo::audio_tubo(5));
        dsk.out.grid.text(b"\n");
        // *** LAS DOS FILAS, Y SON DISTINTAS AUNQUE SUENEN IGUAL.
        //
        //    tarde    el xHC no llego a su cita   -> el problema es del BUS
        //    huecos   nadie escribio la trama     -> el problema es de la APP
        //
        // Sin separarlas, un audio que chasquea manda a mirar el driver cuando
        // la mitad de las veces el que llega tarde es quien produce.
        let tarde = bmo::audio_tubo(6);
        dsk.out.grid.text(b"    tramas TARDE    ");
        dsk.out.grid.with_ink(if tarde == 0 { INK_GOOD } else { INK_ERR });
        dsk.out.grid.dec(tarde);
        dsk.out.grid.with_ink(INK_PLAIN);
        dsk.out.grid.text(b"   (el bus)\n");
        let huecos = bmo::audio_tubo(12);
        dsk.out.grid.text(b"    huecos          ");
        dsk.out.grid.with_ink(if huecos == 0 { INK_GOOD } else { INK_ERR });
        dsk.out.grid.dec(huecos);
        dsk.out.grid.with_ink(INK_PLAIN);
        dsk.out.grid.text(b"   (el que produce)\n");
        // ** El bufer prestado, si lo hay. Sin prestamo no se pinta: una fila
        // de ceros sobre algo que nadie ofrecio no dice nada.
        let pend = bmo::audio_tubo(11);
        if pend > 0 || bmo::audio_tubo(10) > 0 {
            dsk.out.grid.text(b"    sin entregar    ");
            dsk.out.grid.dec(pend);
            dsk.out.grid.text(b" bytes\n");
        }
        if bmo::audio_tubo(7) == 0 {
            dsk.out.grid.text(b"  escribe `audio silencio` para empujar y ver si sube\n");
        }
    } else {
        dsk.out.grid.with_ink(INK_ERR);
        dsk.out.grid.text(b"  el tubo NO esta abierto: no puede sonar nada todavia\n");
        dsk.out.grid.with_ink(INK_PLAIN);
    }
    paint_status(&p, &dsk.run_box, "audio", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn autopsy(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    report_autopsy(&mut dsk.out.grid);
    paint_status(&p, &dsk.run_box, "ultimo fallo de Ring 3", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **`cabina`** -- el anillo de eventos del kernel. Vive al lado de [`autopsy`]
/// porque los dos leen la caja negra, y son OTRA pregunta cada uno: aquel el
/// ultimo fallo de Ring 3, este todo lo que el kernel apunto.
pub(crate) fn cabina(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> After {
    // ** `radar` es OTRO panel, no un filtro mas: el anillo contesta *que paso*
    // y el barrido *cuanto hubo*. Meterlo como filtro habria dado a entender que
    // ensena un subconjunto de lo mismo, y ensena lo que el anillo YA NO TIENE.
    if arg == b"radar" || arg == b"barrido" {
        super::cabina::report_radar(&mut dsk.out.grid);
    } else {
        super::cabina::report_cabina(&mut dsk.out.grid, arg);
    }
    paint_status(&p, &dsk.run_box, "la caja negra del kernel", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn report(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    report_system(&mut dsk.out.grid);
    paint_status(&p, &dsk.run_box, "informe del sistema", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn cpu(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    report_cpu(&mut dsk.out.grid);
    paint_status(&p, &dsk.run_box, "procesador", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn ext(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    super::reports::report_ext(&mut dsk.out.grid);
    paint_status(&p, &dsk.run_box, "extensiones", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn apps(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    super::reports::report_apps(&mut dsk.out.grid);
    paint_status(&p, &dsk.run_box, "apps", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn consumo(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    super::reports::report_consumo(&mut dsk.out.grid);
    paint_status(&p, &dsk.run_box, "consumo", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

pub(crate) fn memory(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    report_memory(&mut dsk.out.grid);
    paint_status(&p, &dsk.run_box, "memoria", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// * El aviso va ANTES y se VUELCA antes, porque la
/// llamada bloquea hasta un segundo entero mientras
/// el kernel manda INIT+SIPI a cada nucleo. Un
/// mensaje escrito despues de volver no explica
/// nada: para entonces la espera ya paso, y lo que
/// el dueno habria visto es un escritorio congelado
/// sin motivo.
/// **`banda` -- el ancho de banda de la memoria, y lo que decide el modelo.**
///
/// # Por que un BARRIDO y no un numero
///
/// Un modelo de lenguaje lee sus pesos **enteros por cada token**, asi que
/// `tokens/s = ancho de banda / bytes del modelo`. Y un solo nucleo de este CPU
/// **no satura la DDR4**: tiene un numero limitado de fallos de cache en vuelo.
/// Lo que hay que buscar no es un maximo, es **donde la curva deja de subir**.
///
/// *** Y por eso hacen falta los obreros. Con `1 de 12` en pie esto mide un
/// nucleo y ya, que es la mitad larga de la respuesta. `smp all` primero.
pub(crate) fn banda(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    let mut b = [0u8; 10];
    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"  BANDA ------------------------------------------------------
");
    dsk.out.grid.with_ink(INK_PLAIN);

    let (vivos, _, _) = bmo::smp_censo(0);
    if vivos == 0 {
        // ** No es un error, es la mitad de la medida faltando. Y se dice
        // ANTES de gastar medio segundo reservando 256 MiB para un barrido que
        // solo va a tener un punto.
        dsk.out.grid.with_ink(INK_ERR);
        dsk.out.grid.text(b"    solo el BSP en pie: esto mediria UN nucleo
");
        dsk.out.grid.with_ink(INK_PLAIN);
        dsk.out.grid.text(b"    escribe `smp all` y vuelve. Un nucleo NO satura la RAM.
");
        paint_status(&p, &dsk.run_box, "banda: faltan obreros", INK_DIM);
        dsk.field.n = 0;
        dsk.field.cur = 0;
        return After::NextKey;
    }

    dsk.out.grid.text(b"    reservando el banco (256 MiB)...
");
    paint_output(&p, &dsk.run_box, &dsk.out.grid);
    p.volcar();
    let bytes = bmo::banda_preparar();
    if bytes == 0 {
        dsk.out.grid.with_ink(INK_ERR);
        dsk.out.grid.text(b"    el banco no llega a 4x el L3: mediria CACHE, no RAM
");
        dsk.out.grid.with_ink(INK_PLAIN);
        dsk.out.grid.text(b"    F11 dice el motivo exacto
");
        paint_status(&p, &dsk.run_box, "banda: sin banco", INK_DIM);
        dsk.field.n = 0;
        dsk.field.cur = 0;
        return After::NextKey;
    }
    dsk.out.grid.text(b"    banco         ");
    let k = decimal(bytes / (1024 * 1024), &mut b);
    dsk.out.grid.text(&b[..k]);
    dsk.out.grid.text(b" MiB   (el L3 son 32: no cabe, y esa es la idea)
");
    dsk.out.grid.text(b"    midiendo (esto tarda)...
");
    paint_output(&p, &dsk.run_box, &dsk.out.grid);
    p.volcar();

    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"    partes        MB/s        x el de 1 parte
");
    dsk.out.grid.with_ink(INK_PLAIN);

    // Los mismos puntos que declara el kernel: 1, 2, 4, 6, 8, 12 partes.
    const PARTES: [u64; 6] = [1, 2, 4, 6, 8, 12];
    let mut base = 0u64;
    let mut techo = 0u64;
    for i in 0..PARTES.len() {
        let mb = bmo::banda_punto(i as u32);
        if mb == 0 {
            continue;
        }
        if base == 0 {
            base = mb;
        }
        if mb > techo {
            techo = mb;
        }
        dsk.out.grid.text(b"    ");
        let k = decimal(PARTES[i], &mut b);
        for _ in k..4 { dsk.out.grid.byte(b' '); }
        dsk.out.grid.text(&b[..k]);
        dsk.out.grid.text(b"       ");
        let k = decimal(mb, &mut b);
        for _ in k..6 { dsk.out.grid.byte(b' '); }
        dsk.out.grid.text(&b[..k]);
        dsk.out.grid.text(b"      ");
        let x100 = mb * 100 / base;
        let k = decimal(x100 / 100, &mut b);
        dsk.out.grid.text(&b[..k]);
        dsk.out.grid.byte(b'.');
        if x100 % 100 < 10 { dsk.out.grid.byte(b'0'); }
        let k = decimal(x100 % 100, &mut b);
        dsk.out.grid.text(&b[..k]);
        dsk.out.grid.byte(b'\n');


        paint_output(&p, &dsk.run_box, &dsk.out.grid);
        p.volcar();
    }

    if techo == 0 {
        dsk.out.grid.with_ink(INK_ERR);
        dsk.out.grid.text(b"    ni un punto valido. F11 dice por que.
");
        dsk.out.grid.with_ink(INK_PLAIN);
        paint_status(&p, &dsk.run_box, "banda: sin medida", INK_DIM);
        dsk.field.n = 0;
        dsk.field.cur = 0;
        return After::NextKey;
    }

    // *** Y LA TRADUCCION, que es a lo que se venia. TECHO y no prediccion:
    // supone leer los pesos a la velocidad maxima de la maquina, y un motor de
    // verdad se queda entre el 60% y el 80%. Lo que SI es cierto es que no
    // puede pasarlo -- y por eso sirve para elegir el modelo ANTES de escribir
    // el motor.
    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"    techo de tokens/s  (un modelo lee sus pesos ENTEROS por token)
");
    dsk.out.grid.with_ink(INK_PLAIN);
    const MODELOS: [(&[u8], u64); 3] = [
        (b"1B en 4 bits ( 700 MB)", 700),
        (b"3B en 4 bits (1700 MB)", 1700),
        (b"7B en 4 bits (3700 MB)", 3700),
    ];
    for (nombre, mb_modelo) in MODELOS {
        dsk.out.grid.text(b"      ");
        dsk.out.grid.text(nombre);
        dsk.out.grid.text(b"  ->  ");
        let x100 = techo * 100 / mb_modelo;
        let k = decimal(x100 / 100, &mut b);
        dsk.out.grid.text(&b[..k]);
        dsk.out.grid.byte(b'.');
        if x100 % 100 < 10 { dsk.out.grid.byte(b'0'); }
        let k = decimal(x100 % 100, &mut b);
        dsk.out.grid.text(&b[..k]);
        dsk.out.grid.text(b" tokens/s
");
    }
    dsk.out.grid.with_ink(INK_PLAIN);
    dsk.out.grid.text(b"    TECHO, no prediccion: un motor real se queda en el 60-80%
");
    dsk.out.grid.with_ink(INK_PLAIN);

    paint_status(&p, &dsk.run_box, "banda", INK_DIM);
    dsk.field.n = 0;
    dsk.field.cur = 0;
    After::NextKey
}

pub(crate) fn smp(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> After {
    // * El CONTROL, y el reparto de quien decide:
    // aqui solo se traduce lo que el dueno escribio
    // a un numero. `smp` a secas censa y no toca
    // nada -- que sea el caso por defecto es la
    // diferencia entre un mando y un boton.
    // Los dos mandos que no son un numero: parar y
    // medir. Se resuelven aqui y salen, porque no
    // comparten NADA con el camino de despertar.
    // `stop` y `test` son los nombres que el dueno
    // pidio; `parar` y `prueba` siguen valiendo. Un
    // alias cuesta cuatro bytes y evita el unico
    // fallo de una orden bien escrita: no acordarse
    // de como se llamaba.
    if arg == b"parar" || arg == b"para" || arg == b"stop" {
        bmo::smp_parar();
        dsk.out.grid.text(b"  obreros parados (vuelven a hlt)\n");
        // ** Y LO QUE VA A PASAR DESPUES, DICHO AQUI.
        //
        // El dueno escribio `smp stop`, luego `smp`,
        // y leyo `12 de 12`. Las dos lineas eran
        // ciertas y juntas decian una mentira. Lo
        // que faltaba no era un numero distinto:
        // era avisar de que ese numero cuenta otra
        // cosa.
        dsk.out.grid.text(b"  [!] seguiran contando como \"en pie\": encendidos, no trabajando\n");
        dsk.out.grid.text(b"      `smp all` los vuelve a poner a trabajar\n");
        paint_output(&p, &dsk.run_box, &dsk.out.grid);
        paint_status(&p, &dsk.run_box, "smp", INK_DIM);
        dsk.field.n = 0;
        dsk.field.cur = 0;
        return After::NextKey;
    }
    if arg == b"prueba" || arg == b"bench" || arg == b"test" {
        dsk.out.grid.text(b"  midiendo reparto (esto tarda)...\n");
        paint_output(&p, &dsk.run_box, &dsk.out.grid);
        p.volcar();
        let x100 = bmo::smp_prueba();
        let mut b = [0u8; 10];
        dsk.out.grid.with_ink(if x100 >= 150 { INK_GOOD } else { INK_ERR });
        dsk.out.grid.text(b"  aceleracion: ");
        let k = decimal(x100 / 100, &mut b);
        dsk.out.grid.text(&b[..k]);
        dsk.out.grid.text(b".");
        // Los dos decimales, con su cero delante:
        // "8.4" y "8.04" no son el mismo numero.
        if x100 % 100 < 10 {
            dsk.out.grid.text(b"0");
        }
        let k = decimal(x100 % 100, &mut b);
        dsk.out.grid.text(&b[..k]);
        dsk.out.grid.text(b"x   (F11 trae los ticks)\n");
        dsk.out.grid.with_ink(INK_PLAIN);
        if x100 == 0 {
            dsk.out.grid.text(b"  0 = falto una parte: el numero no vale\n");
        }
        paint_status(&p, &dsk.run_box, "smp", INK_DIM);
        dsk.field.n = 0;
        dsk.field.cur = 0;
        return After::NextKey;
    }
    let how_many = if arg.is_empty() {
        0
    } else if arg == b"all" || arg == b"todos" {
        u32::MAX
    } else {
        let mut v = 0u32;
        let mut ok = false;
        for &b in arg {
            if b >= b'0' && b <= b'9' {
                v = v.saturating_mul(10) + (b - b'0') as u32;
                ok = true;
            } else {
                ok = false;
                break;
            }
        }
        // Un argumento que no se entiende NO se
        // interpreta como "todos": eso convertiria
        // un dedazo en once INIT+SIPI.
        if ok { v } else { 0 }
    };
    if how_many == 0 {
        dsk.out.grid.text(b"  censando (no se despierta a nadie)\n");
    } else {
        dsk.out.grid.text(b"  despertando nucleos (esto tarda)...\n");
    }
    paint_output(&p, &dsk.run_box, &dsk.out.grid);
    p.volcar();
    let (alive, expected, stopped) = bmo::smp_censo(how_many);
    dsk.out.grid.with_ink(if alive == expected {
        INK_GOOD
    } else {
        INK_ERR
    });
    dsk.out.grid.text(b"  nucleos en pie: ");
    let mut b = [0u8; 10];
    let k = decimal((alive + 1) as u64, &mut b);
    dsk.out.grid.text(&b[..k]);
    dsk.out.grid.text(b" de ");
    let k = decimal((expected + 1) as u64, &mut b);
    dsk.out.grid.text(&b[..k]);
    dsk.out.grid.text(b"   (F11 lo cuenta entero)\n");
    dsk.out.grid.with_ink(INK_PLAIN);
    // ** LA MITAD QUE FALTABA DEL CENSO.
    //
    // "En pie" cuenta nucleos que contestaron al
    // SIPI, y ese numero no baja al pararlos --
    // correctamente: salir del reset no es trabajar.
    // Pero leido solo, dice que `smp stop` no hizo
    // nada. Ahora se dicen las dos cosas.
    if stopped {
        dsk.out.grid.with_ink(INK_ERR);
        dsk.out.grid.text(b"  [!] pero estan PARADOS: en pie no es trabajando\n");
        dsk.out.grid.with_ink(INK_PLAIN);
        dsk.out.grid.text(b"      `smp all` los vuelve a poner a trabajar\n");
    }
    // La guia va donde se necesita: justo despues
    // de censar, que es cuando uno se pregunta
    // "y ahora como los enciendo?". Un atajo que
    // solo vive en la documentacion no existe.
    // ** LAS CINCO, NO SOLO DOS (2026-08-11).
    //
    // Aqui se decian `smp all` y `smp N` y se
    // callaban `prueba` y `parar`, que son las dos
    // unicas que HACEN algo interesante. El dueno
    // lo dijo con todas las letras: *"el smp no me
    // salen mensajes de recomendacion"*.
    //
    // Son cinco lineas y caben. Una orden con
    // subordenes que no las dice obliga a buscar en
    // `help`, y a `help` se va cuando uno ya se
    // rindio.
    if how_many == 0 {
        dsk.out.grid.text(b"  smp all      despierta todos    smp 3   solo tres\n");
        dsk.out.grid.text(b"  smp test     reparte una cuenta y mide la aceleracion\n");
        dsk.out.grid.text(b"  smp stop     los duerme. [!] sin IPI NO vuelven\n");
        dsk.out.grid.text(b"  F11 dice en que esta cada nucleo y cual gira en vacio\n");
    }
    paint_status(&p, &dsk.run_box, "smp", INK_DIM);
    dsk.field.n = 0;
    dsk.field.cur = 0;
    After::Settle
}

/// Se pinta ANTES de pedirlo: la llamada no vuelve,
/// asi que un mensaje despues no lo veria nadie. Y
/// que quede escrito distingue "reinicio pedido" de
/// "se colgo" en la foto. `Pantalla` escribe directo
/// al framebuffer, asi que al volver de `text` ya
/// esta en el cristal: no hay nada que vaciar.
pub(crate) fn reboot(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    dsk.out.grid.text(b"  reiniciando...\n");
    paint_status(&p, &dsk.run_box, "reiniciando", INK_DIM);
    bmo::reiniciar();
}

/// **`placa` -- lo que el firmware le cuenta a BMO-X.**
///
/// ## Por que esto vive aqui y no solo en Ring 0 (2026-08-24)
///
/// Se cablo en el shell de Ring 0 y el dueno, que vive en el escritorio, recibio
/// *"no es un comando ni una ruta"*. **Un camino que solo existe alli es un
/// camino que el dueno de su propia maquina no puede tomar.**
///
/// ** Contesta y no concede: no cambia nada del firmware ni de la placa.
///
/// [!] Y la fila que hay que mirar NO es cuantas tablas hay: es cuantas **no
/// pasan su suma de comprobacion**. En una placa sana es cero, y si no lo es lo
/// que falla no es la placa -- es el mapeo de esas direcciones fisicas.
pub(crate) fn placa(dsk: &mut Desktop, _p: &bmo::Pantalla) -> After {
    let n = bmo::placa_cuantas();
    if n == 0 {
        dsk.out.grid.text(b"  sin XSDT que leer: el firmware no dio un RSDP de ACPI 2.0+
");
        return After::Settle;
    }
    let mut aml = 0u64;
    let mut malas = 0u64;
    for i in 0..n {
        let v = bmo::placa_tabla(i);
        if v == 0 {
            continue;
        }
        let creible = v & (1 << 32) != 0;
        let programa = v & (1 << 33) != 0;
        if !creible { malas += 1; }
        if programa { aml += 1; }
        dsk.out.grid.text(if !creible {
            b"  [!] "
        } else if programa {
            b"  AML "
        } else {
            b"      "
        });
        // Los cuatro caracteres de la firma, tal cual vinieron.
        let f = ((v & 0xFFFF_FFFF) as u32).to_le_bytes();
        dsk.out.grid.text(&f);
        dsk.out.grid.text(b"
");
    }
    dsk.out.grid.text(b"  tablas: ");
    dsk.out.grid.dec(n);
    dsk.out.grid.text(b"   AML (no se ejecutan): ");
    dsk.out.grid.dec(aml);
    dsk.out.grid.text(b"   sin suma valida: ");
    dsk.out.grid.dec(malas);
    dsk.out.grid.text(b"
");

    let ecam = bmo::placa_ecam();
    if ecam == 0 {
        dsk.out.grid.text(b"  sin MCFG: la config de PCIe se queda en 256 B por funcion.
");
    } else {
        dsk.out.grid.text(b"  PCIe config en memoria: 0x");
        dsk.out.grid.hex(ecam, 8);
        dsk.out.grid.text(b"   (4096 B por funcion)
");
    }
    let iommu = bmo::placa_iommu();
    if iommu == 0 {
        // *** Y esto no es una carencia menor: una capability dice que puede
        // hacer un PROCESO, y no dice NADA de lo que puede hacer un APARATO.
        dsk.out.grid.with_ink(INK_PLAIN);
        dsk.out.grid.text(b"  [!] sin IVRS: nada limita adonde escribe un aparato con DMA.
");
        dsk.out.grid.with_ink(INK_PLAIN);
    } else {
        dsk.out.grid.text(b"  IOMMU: registros en 0x");
        dsk.out.grid.hex(iommu, 8);
        dsk.out.grid.text(b"   (leerla no la enciende)
");
    }
    dsk.out.grid.with_ink(INK_PLAIN);
    dsk.out.grid.text(b"  el AML es un PROGRAMA de la placa. BMO-X se perfila: no lo ejecuta.
");
    dsk.out.grid.with_ink(INK_PLAIN);
    After::Settle
}

/// **EL CENSO HILO A HILO, CON SU NOMBRE.**
///
/// *** Peticion del dueno (2026-08-24): *"en `smp all` me gustaria que detalles
/// TODO con nombres CORE y THREAD asi para no decir x12, eso es mentir si pongo
/// asi"*.
///
/// Y tiene razon. **"12 de 12" presenta doce cosas como si fueran doce
/// iguales**, y no lo son: son SEIS nucleos con dos hilos cada uno. Un hilo SMT
/// no es medio nucleo ni es un nucleo -- es un sitio mas para meter trabajo en
/// el MISMO nucleo, y cuanto rinde depende de si la faena deja huecos.
///
/// Es la misma queja que la de la aceleracion, en otro sitio: **un numero sin el
/// perfil al lado no se puede juzgar.**
fn tabla_de_hilos(s: &mut crate::scene::output::Output) {
    let hilos = bmo::info(bmo::INFO_CPU_HILOS) as u32;
    if hilos == 0 || hilos > 64 {
        return;
    }
    let mut cores = 0u32;
    let mut threads = 0u32;
    let mut trabajando = 0u32;
    let mut ultimo_fisico = u32::MAX;

    for id in 0..hilos {
        let (estado, tipo, fisico, _hpc) = bmo::smp_hilo(id);
        // ** Una linea en blanco entre nucleos fisicos: es lo que hace que se
        // VEA que los hermanos van de dos en dos, sin tener que contarlos.
        if fisico != ultimo_fisico {
            ultimo_fisico = fisico;
            s.text(b"    CORE ");
            s.dec(fisico as u64);
            s.byte(b'\n');
        }
        s.text(b"      ");
        match tipo {
            1 => { cores += 1; s.text(b"CORE   "); }
            2 => { threads += 1; s.text(b"THREAD "); }
            _ => s.text(b"?      "),
        }
        s.text(b"#");
        s.dec(id as u64);
        s.text(b"  ");
        match estado {
            0 => { trabajando += 1; s.with_ink(INK_GOOD); s.text(b"MAESTRO (el BSP)"); s.with_ink(INK_PLAIN); }
            1 => { trabajando += 1; s.with_ink(INK_GOOD); s.text(b"obrero, EN PIE"); s.with_ink(INK_PLAIN); }
            // ** "Dormido" y "en pie" se cuentan distinto A PROPOSITO. El dueno
            // escribio `smp stop`, luego `smp`, y leyo "12 de 12": las dos
            // lineas eran ciertas y juntas decian una mentira.
            2 => s.text(b"PARADO (sin IPI no vuelve)"),
            3 => s.text(b"AUSENTE -- no contesto al llamarlo"),
            _ => s.text(b"?"),
        }
        s.byte(b'\n');
    }

    // *** Y EL RESUMEN QUE NO ES UNA `x`.
    s.text(b"    = ");
    s.dec(cores as u64);
    s.text(b" CORE + ");
    s.dec(threads as u64);
    s.text(b" THREAD, y ");
    s.dec(trabajando as u64);
    s.text(b" trabajando\n");
    s.text(b"    [!] un THREAD no es medio CORE: es otro sitio para meter\n");
    s.text(b"        trabajo en el MISMO nucleo. Lo que rinde depende de si\n");
    s.text(b"        la faena deja huecos -- `smp test` da los DOS numeros.\n");
}
