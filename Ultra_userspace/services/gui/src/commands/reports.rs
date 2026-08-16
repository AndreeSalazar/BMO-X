//! `info`, `cpu`, `mem` -- los datos del sistema, escritos a la rejilla.
//!
//! Leer un contador no ejerce ningun poder, asi que esto vive en Ring 3 y pide
//! los datos por `OP_INFO` como cualquier otro proceso.

use bmo_userland as bmo;

use crate::scene::output::{Output, INK_GOOD, INK_ECHO, INK_ERR, INK_PLAIN};
use crate::scene::OUT_COLS;

// -- Los informes del sistema --------------------------------------------
//
// Se pintan aqui, en Ring 3, con datos que el kernel contesta por `OP_INFO`.
// El kernel da enteros; las unidades, los porcentajes, las barras y el color
// son de este lado.

/// Un rotulo de seccion, para que el informe no sea un muro de renglones.
pub(crate) fn section(s: &mut Output, title: &[u8]) {
    s.with_ink(INK_ECHO);
    s.text(b"  ");
    s.text(title);
    s.byte(b' ');
    // Una regla hasta el margen: cuesta nada y separa de verdad.
    let used_one = 3 + title.len();
    for _ in used_one..OUT_COLS.saturating_sub(2) {
        s.byte(b'-');
    }
    s.byte(b'\n');
    s.with_ink(INK_PLAIN);
}

/// Un renglon `label ....... value`, con la etiqueta a ancho fijo.
pub(crate) fn label(s: &mut Output, name: &[u8]) {
    s.text(b"    ");
    s.text(name);
    for _ in name.len()..14 {
        s.byte(b' ');
    }
}

/// Una fila de la tabla de consumo: `que`, el valor a la DERECHA, y la unidad.
///
/// Las tres columnas van a ancho fijo porque una tabla en la que los numeros no
/// estan alineados no es una tabla: es una lista con guiones. El valor va a la
/// derecha (`dec_right`) para que las unidades y las decenas caigan una debajo
/// de otra y se puedan comparar dos volcados de un vistazo.
fn fila(s: &mut Output, que: &[u8], valor: u64, unidad: &[u8], nota: &[u8]) {
    s.text(b"    ");
    s.text(que);
    for _ in que.len()..16 {
        s.byte(b' ');
    }
    s.dec_right(valor, 9);
    s.byte(b' ');
    s.text(unidad);
    if !nota.is_empty() {
        for _ in unidad.len()..8 {
            s.byte(b' ');
        }
        s.with_ink(INK_ECHO);
        s.text(nota);
        s.with_ink(INK_PLAIN);
    }
    s.byte(b'\n');
}

/// Igual, pero para un numero con UN decimal guardado en milesimas: los vatios
/// llegan en milivatios y `57432` se lee como `57.4`.
fn fila_mili(s: &mut Output, que: &[u8], milis: u64, unidad: &[u8], nota: &[u8]) {
    s.text(b"    ");
    s.text(que);
    for _ in que.len()..16 {
        s.byte(b' ');
    }
    s.dec_right(milis / 1000, 7);
    s.byte(b'.');
    s.dec((milis % 1000) / 100);
    s.byte(b' ');
    s.text(unidad);
    if !nota.is_empty() {
        for _ in unidad.len()..8 {
            s.byte(b' ');
        }
        s.with_ink(INK_ECHO);
        s.text(nota);
        s.with_ink(INK_PLAIN);
    }
    s.byte(b'\n');
}

/// Un renglon de separacion DENTRO de una seccion. Ver `report_consumo`.
fn subregla(s: &mut Output, titulo: &[u8]) {
    s.with_ink(INK_ECHO);
    s.text(b"    ");
    s.text(titulo);
    s.byte(b' ');
    let usado = 5 + titulo.len();
    for _ in usado..OUT_COLS.saturating_sub(4) {
        s.byte(b'.');
    }
    s.byte(b'\n');
    s.with_ink(INK_PLAIN);
}

/// **QUE PROGRAMA SE ESTA COMIENDO LA RAM, uno por fila.**
///
/// # De donde salen estos numeros, que ya existian y no los leia nadie
///
/// `INFO_MEM_QUIEN_PID/BYTES/PETICIONES` estan en el ABI desde que el kernel
/// aprendio a repartir memoria, y **ningun programa los habia pedido nunca**.
/// El indice viaja empaquetado con el campo --`campo | (ranura << 8)`-- porque
/// por la puerta de `INFO` cabe un numero y no una estructura; se enumera
/// pidiendo 0, 1, 2... hasta que el pid conteste `0`.
///
/// ** Y las ranuras que se enumeran son las OCUPADAS: los agujeros de la tabla
/// del kernel son suyos y no se ven desde aqui.
///
/// # Para que sirve de verdad
///
/// Es el instrumento de la fuga que se cerro el 14-08. Un programa que muere
/// tiene que **desaparecer de esta tabla**; si su fila sigue ahi, su memoria no
/// volvio. Y la columna `peticiones` distingue *"pidio un bloque grande"* de
/// *"esta pidiendo sin parar"*, que es la diferencia entre un juego y una fuga.
///
/// [!] **Aqui no hay nombres, y hay que decirlo**: el kernel guarda el pid, no
/// como se llamaba el `.bex`. Poner el nombre es una tabla mas en el kernel y un
/// campo `INFO_TXT` nuevo, o sea tocar los TRES lados del contrato. Queda
/// escrito como lo que es -- lo siguiente, no un olvido.
#[inline(never)]
pub(crate) fn report_apps(s: &mut Output) {
    section(s, b"apps con memoria pedida");

    s.with_ink(INK_ECHO);
    s.text(b"    ranura   pid        MiB   peticiones\n");
    s.text(b"    ------   ---   --------   ----------\n");
    s.with_ink(INK_PLAIN);

    let mut n = 0u64;
    let mut vistos = 0u64;
    let mut suma = 0u64;
    while n < 32 {
        let pid = bmo::info(bmo::INFO_MEM_QUIEN_PID | (n << 8));
        if pid == 0 {
            break;
        }
        let bytes = bmo::info(bmo::INFO_MEM_QUIEN_BYTES | (n << 8));
        let pet = bmo::info(bmo::INFO_MEM_QUIEN_PETICIONES | (n << 8));
        s.text(b"    ");
        s.dec_right(n, 6);
        s.dec_right(pid, 6);
        s.dec_right(bytes / (1024 * 1024), 11);
        s.dec_right(pet, 13);
        s.byte(b'\n');
        suma += bytes;
        vistos += 1;
        n += 1;
    }

    if vistos == 0 {
        s.with_ink(INK_ECHO);
        s.text(b"    ningun programa tiene memoria pedida ahora mismo\n");
        s.with_ink(INK_PLAIN);
    } else {
        s.with_ink(INK_ECHO);
        s.text(b"    ------   ---   --------   ----------\n");
        s.with_ink(INK_PLAIN);
        s.text(b"    total ");
        s.dec_right(vistos, 4);
        s.text(b" apps");
        s.dec_right(suma / (1024 * 1024), 7);
        s.text(b" MiB\n");
    }
}

/// **EL CONSUMO, EN UNA TABLA Y DE UNA VEZ.**
///
/// # Por que existe si `cpu` y `mem` ya dicen casi todo
///
/// Porque lo dicen **repartido y en prosa**. `cpu` explica la maquina, `mem`
/// explica la RAM, y para saber "que esta gastando esto ahora mismo" hay que
/// leer dos informes y juntarlos a ojo. Lo pidio el dueno con esas palabras:
/// *"detallar el consumo... cuantos nucleos y hilos, y W tambien, y otros mas,
/// en orden como tablas para facilitar, con separacion"*.
///
/// Las filas van a ancho fijo y con el numero a la derecha **para que dos
/// volcados se puedan comparar poniendolos uno debajo del otro**. Ese es el uso
/// real: lanzar DOOM, matarlo, y mirar si la RAM volvio a su sitio.
///
/// [!] Los tres bloques van separados por su propia regla y no por una linea en
/// blanco: una tabla de veinte filas seguidas no se lee, y las lineas en blanco
/// se pierden al volcar a texto.
#[inline(never)]
pub(crate) fn report_consumo(s: &mut Output) {
    section(s, b"consumo");

    subregla(s, b"cpu");
    let hilos = bmo::info(bmo::INFO_CPU_HILOS);
    let vivos = bmo::info(bmo::INFO_SMP_VIVOS);
    fila(s, b"nucleos", bmo::info(bmo::INFO_CPU_NUCLEOS), b"fisicos", b"");
    fila(s, b"hilos", hilos, b"logicos", b"");
    // `SMP_VIVOS` cuenta los APs, o sea SIN el BSP; el que mira quiere el total.
    fila(s, b"en pie", vivos + 1, b"de", b"");
    let hz = bmo::info(bmo::INFO_CPU_HZ_REAL);
    if hz > 0 {
        fila(s, b"reloj ahora", hz / 1_000_000, b"MHz", b"medido por MPERF/APERF");
    }
    fila(s, b"reloj base", bmo::info(bmo::INFO_TSC_HZ) / 1_000_000, b"MHz", b"el TSC");
    let mw = bmo::info(bmo::INFO_CPU_MW_PAQUETE);
    if mw > 0 {
        fila_mili(s, b"gasta paquete", mw, b"W", b"los nucleos + fabric + memoria + L3");
        let mwn = bmo::info(bmo::INFO_CPU_MW_NUCLEO_ACTUAL);
        if mwn > 0 {
            fila_mili(s, b"gasta nucleo", mwn, b"W", b"solo ESTE");
        }
    }

    subregla(s, b"memoria");
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let libre = bmo::info(bmo::INFO_RAM_LIBRE);
    fila(s, b"total", total / (1024 * 1024), b"MiB", b"");
    fila(s, b"usada", total.saturating_sub(libre) / (1024 * 1024), b"MiB", b"");
    fila(s, b"libre", libre / (1024 * 1024), b"MiB", b"la fila que tiene que VOLVER");
    fila(s, b"marcos libres", bmo::info(bmo::INFO_RAM_MARCOS_LIBRES), b"de", b"");
    fila(s, b"marcos", bmo::info(bmo::INFO_RAM_MARCOS), b"de 4 KiB", b"");
    fila(
        s,
        b"a Ring 3",
        bmo::info(bmo::INFO_MEM_ENTREGADA) / (1024 * 1024),
        b"MiB",
        b"HISTORICO de la sesion, no lo de ahora",
    );
    fila(s, b"kernel", bmo::info(bmo::INFO_KERNEL_BYTES) / 1024, b"KiB", b"");

    subregla(s, b"tareas");
    let ranuras = bmo::info(bmo::INFO_TAREAS_TOTAL);
    fila(s, b"en uso", ranuras, b"", b"");
    fila(s, b"libres", bmo::info(bmo::INFO_TAREAS_LIBRES), b"", b"");
    fila(s, b"listas", bmo::info(bmo::INFO_TAREAS_LISTAS), b"", b"");
    fila(s, b"lanzados", bmo::info(bmo::INFO_PROGRAMAS), b"", b"desde el arranque");
    fila(s, b"ticks", bmo::info(bmo::INFO_TICKS), b"", b"");
}

#[inline(never)]
pub(crate) fn report_cpu(s: &mut Output) {
    let mut buf = [0u8; 64];

    section(s, b"procesador");
    let n = bmo::info_texto(bmo::INFO_TXT_CPU_VENDOR, &mut buf);
    label(s, b"fabricante");
    s.text(&buf[..n]);
    s.byte(b'\n');

    let n = bmo::info_texto(bmo::INFO_TXT_CPU_NOMBRE, &mut buf);
    label(s, b"modelo");
    s.text(&buf[..n]);
    s.byte(b'\n');

    let n = bmo::info_texto(bmo::INFO_TXT_UARCH, &mut buf);
    label(s, b"uarch");
    s.text(&buf[..n]);
    let n2 = bmo::info_texto(bmo::INFO_TXT_FAMILIA, &mut buf);
    if n2 > 0 {
        s.text(b"   familia ");
        s.text(&buf[..n2]);
    }
    s.byte(b'\n');

    label(s, b"nucleos");
    s.dec(bmo::info(bmo::INFO_CPU_NUCLEOS));
    s.text(b" fisicos / ");
    s.dec(bmo::info(bmo::INFO_CPU_HILOS));
    s.text(b" hilos\n");

    // ** QUE SABE MEDIR ESTE PERFIL, antes de ensenar ninguna medida.
    //
    // Va PRIMERO a proposito. Las filas de abajo pueden salir vacias por dos
    // motivos que se ven igual --el silicio no lo expone, o aun no hay dos
    // lecturas-- y sin esta linea el que mira no puede distinguirlos.
    //
    // Es la cadena que pidio el dueno, leida de arriba abajo: el PERFIL declara
    // que se puede medir, el lector lo lee, y **la terminal ensena lo que el
    // perfil esta reflejando** en vez de suponerlo.
    let sensors = bmo::info(bmo::INFO_CPU_SENSORES);
    label(s, b"mide");
    if sensors == 0 {
        s.with_ink(INK_ECHO);
        s.text(b"nada: este perfil no declara sensores");
        s.with_ink(INK_PLAIN);
    } else {
        if sensors & 1 != 0 {
            s.text(b"frecuencia real");
        }
        if sensors & 3 == 3 {
            s.text(b" + ");
        }
        if sensors & 2 != 0 {
            s.text(b"consumo");
        }
        s.with_ink(INK_ECHO);
        s.text(b"   (lo declara el perfil)");
        s.with_ink(INK_PLAIN);
    }
    s.byte(b'\n');

    // Hz -> GHz con dos decimales, con enteros. El TSC es la frecuencia MEDIDA
    // en el arranque, no el numero de la etiqueta de la caja.
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    label(s, b"tsc");
    s.dec(hz / 1_000_000_000);
    s.byte(b'.');
    let frac = (hz % 1_000_000_000) / 10_000_000;
    if frac < 10 {
        s.byte(b'0');
    }
    s.dec(frac);
    s.text(b" GHz   (medido)\n");

    // ** Y A QUE VA AHORA, que es otra pregunta.
    //
    // El TSC de arriba es el reloj de REFERENCIA: se midio al arrancar y no
    // cambia nunca. Este es el nucleo de verdad, y en un Zen 3 se mueve entre
    // 3,7 y 4,6 GHz segun cuantos esten trabajando. Los dos juntos son lo que
    // convierte "esta al 100%" en "esta al 100% Y ADEMAS a 4,6 GHz".
    //
    // [!] Es una MEDIDA: sale de restar dos lecturas de MPERF/APERF, asi que el
    // numero es la velocidad **desde la ultima vez que se pregunto**. Pedir
    // `info` dos veces seguidas mide el rato entre las dos.
    let actual = bmo::info(bmo::INFO_CPU_HZ_REAL);
    label(s, b"ahora");
    if actual == 0 {
        // Cero no es cero hercios: es "no se puede medir". Decirlo con palabras
        // evita que alguien lea un 0.00 GHz y crea que el CPU esta parado.
        s.with_ink(INK_ECHO);
        s.text(b"sin MPERF/APERF, o aun sin dos lecturas");
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    } else {
        s.dec(actual / 1_000_000_000);
        s.byte(b'.');
        let f2 = (actual % 1_000_000_000) / 10_000_000;
        if f2 < 10 {
            s.byte(b'0');
        }
        s.dec(f2);
        s.text(b" GHz   ");
        s.with_ink(INK_ECHO);
        if actual > hz {
            s.text(b"(boost)");
        } else if actual + 200_000_000 < hz {
            s.text(b"(bajando)");
        } else {
            s.text(b"(en base)");
        }
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }

    // ** Y LO QUE CUESTA TENERLO ASI.
    //
    // Va pegado a la frecuencia a proposito: los dos numeros juntos son la frase
    // entera. "4,6 GHz" solo dice que va rapido; "4,6 GHz y 88 W" dice que va
    // rapido Y lo que cuesta -- y esa segunda mitad es la que AXION necesita
    // para decidir si apagar nucleos vale la pena.
    //
    // Hasta hoy la seccion 5 de AXION_MAESTRO.md decia que once obreros girando
    // consumen "como si trabajaran": una afirmacion sin numero al lado. Con esta
    // fila, `smp stop` tiene un antes y un despues.
    let mw = bmo::info(bmo::INFO_CPU_MW_PAQUETE);
    let mwn = bmo::info(bmo::INFO_CPU_MW_NUCLEO_ACTUAL);
    label(s, b"gasta");
    if mw == 0 {
        s.with_ink(INK_ECHO);
        s.text(b"sin RAPL, o aun sin dos lecturas");
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    } else {
        s.dec(mw / 1000);
        s.byte(b'.');
        s.dec((mw % 1000) / 100);
        s.text(b" W paquete");
        if mwn > 0 {
            s.text(b" / ");
            s.dec(mwn / 1000);
            s.byte(b'.');
            s.dec((mwn % 1000) / 100);
            s.text(b" W ESTE nucleo");
        }
        s.with_ink(INK_ECHO);
        // La resta se dice porque no es obvia: lo que va del nucleo al paquete
        // es Infinity Fabric, controlador de memoria y L3 -- y ese consumo NO
        // baja aunque se apaguen nucleos.
        s.text(b"   (el paquete son los 6 + fabric + memoria + L3)");
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    }

    // -- SMP, y lo que cuesta --------------------------------------------
    //
    // Los nucleos en pie y los choques de cerrojo van en el MISMO informe a
    // proposito. Un panel que solo ensena "12 de 12" cuenta la mitad bonita:
    // la otra mitad es si esos once obreros estan peleandose con el kernel
    // por dentro, y ese numero tiene que ser cero.
    let alive_count = bmo::info(bmo::INFO_SMP_VIVOS);
    label(s, b"smp");
    if alive_count == 0 {
        s.text(b"solo el BSP");
        s.with_ink(INK_ECHO);
        s.text(b"   (`smp all` levanta los demas)");
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    } else {
        s.with_ink(INK_GOOD);
        s.dec(alive_count + 1);
        s.with_ink(INK_PLAIN);
        s.text(b" nucleos en pie de ");
        s.dec(bmo::info(bmo::INFO_CPU_HILOS));
        s.byte(b'\n');
    }

    let collisions = bmo::info(bmo::INFO_SPIN_CHOQUES);
    label(s, b"cerrojos");
    if collisions == 0 {
        s.with_ink(INK_GOOD);
        s.text(b"0 choques");
        s.with_ink(INK_ECHO);
        s.text(b"   (lo correcto: nadie pelea)");
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    } else {
        // No es una cifra de rendimiento: es que alguien entro en el kernel
        // desde otro nucleo. Se pinta como lo que es.
        s.with_ink(INK_ERR);
        s.dec(collisions);
        s.text(b" CHOQUES");
        s.with_ink(INK_PLAIN);
        s.text(b"   espera mayor ");
        s.dec(bmo::info(bmo::INFO_SPIN_PICO));
        s.text(b" vueltas\n");
    }

    // * Y la otra mitad de lo mismo: cuando una tarea muere, el kernel dice
    // haber recuperado todo lo suyo. Esta fila es quien lo COMPRUEBA.
    //
    // Un numero distinto de cero no acusa al programa que murio: acusa al
    // KERNEL. Va aqui, al lado de los cerrojos, porque son la misma clase de
    // dato -- el sistema comprobandose a si mismo.
    let leaks = bmo::info(bmo::INFO_FUGAS);
    label(s, b"fugas");
    if leaks == 0 {
        s.with_ink(INK_GOOD);
        s.text(b"0");
        s.with_ink(INK_ECHO);
        s.text(b"   (los muertos devolvieron todo)");
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    } else {
        s.with_ink(INK_ERR);
        s.dec(leaks);
        s.text(b" RECURSOS SIN DEVOLVER");
        s.with_ink(INK_PLAIN);
        s.text(b"   escribe `fallo`\n");
    }
}

#[inline(never)]
pub(crate) fn report_memory(s: &mut Output) {
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let free_one = bmo::info(bmo::INFO_RAM_LIBRE);
    let used = total.saturating_sub(free_one);

    section(s, b"memoria");
    label(s, b"total");
    s.size(total);
    s.text(b"   ");
    s.dec_right(bmo::info(bmo::INFO_RAM_MARCOS), 8);
    s.text(b" marcos de 4 KiB\n");

    label(s, b"usada");
    s.size(used);
    s.text(b"   ");
    s.bar(used, total, 24);
    s.byte(b' ');
    s.pct(used, total);
    s.byte(b'\n');

    label(s, b"libre");
    s.size(free_one);
    s.text(b"   ");
    s.dec_right(bmo::info(bmo::INFO_RAM_MARCOS_LIBRES), 8);
    s.text(b" marcos\n");

    // El tamano REAL del kernel en RAM, medido hasta el final de su .bss.
    label(s, b"kernel");
    s.size(bmo::info(bmo::INFO_KERNEL_BYTES));
    s.text(b"   en 0x400000\n");

    // * Lo que Ring 3 ha PEDIDO. Las cuatro filas de arriba las sabe el kernel
    // porque la memoria la reparte el; esta solo se mueve si un programa
    // ejercio `KIND_MEMORIA`. Por eso vale como prueba: es el kernel diciendo
    // que entrego, no el programa diciendo que recibio.
    //
    // En cero se dice EXPRESAMENTE que nadie ha pedido, en vez de pintar un
    // `0 B` que se lee igual que "no lo se".
    let asked = bmo::info(bmo::INFO_MEM_ENTREGADA);
    label(s, b"a Ring 3");
    if asked == 0 {
        s.text(b"ningun programa ha pedido memoria\n");
    } else {
        s.size(asked);
        s.text(b"   pedida con KIND_MEMORIA\n");
    }
}

#[inline(never)]
pub(crate) fn report_system(s: &mut Output) {
    s.with_ink(INK_ECHO);
    s.text(b"  BMO-X - informe del sistema\n");
    s.with_ink(INK_PLAIN);

    report_cpu(s);
    report_memory(s);

    section(s, b"tareas");
    let total = bmo::info(bmo::INFO_TAREAS_TOTAL);
    let free = bmo::info(bmo::INFO_TAREAS_LIBRES);
    let slots = total + free;
    label(s, b"ranuras");
    s.dec(total);
    s.text(b" en uso de ");
    s.dec(slots);
    s.text(b"   ");
    s.bar(total, slots, 24);
    s.byte(b'\n');
    label(s, b"listas");
    s.dec(bmo::info(bmo::INFO_TAREAS_LISTAS));
    s.text(b"   ticks ");
    s.dec(bmo::info(bmo::INFO_TICKS));
    s.byte(b'\n');
    label(s, b"programas");
    let vistos = bmo::info(bmo::INFO_PROGRAMAS);
    let forgotten = bmo::info(bmo::INFO_PROGRAMAS_OLVIDADOS);
    s.dec(vistos + forgotten);
    s.text(b" lanzados");
    if forgotten > 0 {
        s.text(b"   (");
        s.dec(forgotten);
        s.text(b" ya no caben en la bitacora)");
    }
    s.byte(b'\n');

    section(s, b"disco");
    label(s, b"disco");
    if bmo::info(bmo::INFO_DISCO_LISTO) != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"listo");
    } else {
        s.with_ink(INK_ERR);
        s.text(b"sin disco");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    label(s, b"datos");
    if bmo::info(bmo::INFO_DATOS_MONTADO) != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"montado para escritura");
    } else {
        // La linea que decide si el File I/O de COBOL puede funcionar. Decirlo
        // aqui ahorra buscar el fallo en el programa.
        s.with_ink(INK_ERR);
        s.text(b"NO montado: sin esto no hay OPEN ni WRITE");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}


/// ** LA AUTOPSIA del ultimo fallo de Ring 3, tal como la redacto el kernel.
///
/// No se formatea nada aqui a proposito: el informe **ya viene escrito** desde
/// Ring 0, renglon a renglon. Y eso no es pereza, es donde tiene que estar --
/// el unico que sabe el vector, el codigo de error y el `cr2` es quien atendio
/// la excepcion, y volver a interpretarlos en Ring 3 seria tener dos sitios
/// donde equivocarse sobre el mismo fallo.
///
/// Aqui solo se pinta, y se pinta en ROJO, que es lo que es.
#[inline(never)]
pub(crate) fn report_autopsy(s: &mut Output) {
    section(s, b"ultimo fallo");
    let total = bmo::autopsia_total();
    if total == 0 {
        s.with_ink(INK_GOOD);
        s.text(b"    ningun fallo de Ring 3 desde el arranque\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    let rows = bmo::autopsia_renglones(0);
    let mut buf = [0u8; 96];
    for f in 0..rows {
        let n = bmo::autopsia_linea(0, f, &mut buf);
        s.text(b"    ");
        // El titulo en rojo y el cuerpo normal: lo que se busca de un vistazo
        // es CUAL fue y cuando, no el `rsp`.
        if f == 0 {
            s.with_ink(INK_ERR);
        }
        s.text(&buf[..n]);
        if f == 0 {
            s.with_ink(INK_PLAIN);
        }
        s.byte(b'\n');
    }
    if total > 1 {
        s.with_ink(INK_ECHO);
        s.text(b"    (van ");
        s.dec(total);
        s.text(b" desde el arranque; se guardan las 4 ultimas)\n");
        s.with_ink(INK_PLAIN);
    }
}

/// **EL INFORME DE LA RED.** `red` entero, o una sola pregunta con argumento.
///
/// === Por que este informe se lee de abajo arriba ===
///
/// Porque asi se depura un cable. Las cuatro preguntas van en el orden en que
/// se caen, y cada una **solo tiene sentido si la anterior dijo si**:
///
/// ```text
///   hay TARJETA?      no -> el problema es el PCI o el BAR, y nada mas importa
///   hay ENLACE?       no -> es el cable, el switch o el otro extremo
///   estamos ESCUCHANDO?  no -> el receptor no esta armado. `net rx` en Ring 0
///   llegan TRAMAS?    no -> hay enlace, escuchamos, y nadie habla
/// ```
///
/// ** Un panel que contestara *"red: 0"* obligaria a adivinar cual de las cuatro
/// fallo. Ese es todo el motivo de que sean siete campos y no uno con banderas.
///
/// === Y el `PHYstatus` va CRUDO ===
///
/// El driver ya tomo esa decision y aqui se respeta: *"se guarda sin interpretar
/// ademas de interpretado -- el dia que un bit no cuadre, el byte entero es la
/// prueba y las funciones son la opinion"*. Un diagnostico que solo ensena la
/// opinion no ayuda el dia que la opinion falle.
///
/// [!] **Nada de esto transmite.** Son campos de informe: mirar la red no es un
/// privilegio. `CR.TE` sigue apagado en el kernel y no se pone un byte en el
/// cable desde aqui.
///
/// [!] Y los valores son la foto del ARRANQUE, no del instante: el kernel cachea
/// la identidad a proposito para que repintar un panel no toque el BAR de la NIC
/// sesenta veces por segundo. Quien relee es la orden `net` del shell de Ring 0.
#[inline(never)]
pub(crate) fn report_net(s: &mut Output, what: &[u8]) {
    let present = bmo::info(bmo::INFO_NET_PRESENTE) != 0;
    let mac = bmo::info(bmo::INFO_NET_MAC);
    let mbit = bmo::info(bmo::INFO_NET_MEGABITS);
    let phy = bmo::info(bmo::INFO_NET_PHY_CRUDO);
    let armed = bmo::info(bmo::INFO_NET_RX_ARMADO) != 0;
    let frames_rx = bmo::info(bmo::INFO_NET_RX_TRAMAS);

    // Una sola pregunta, para cuando ya sabes cual quieres.
    if what == b"mac" {
        label(s, b"MAC");
        if present { mac_hex(s, mac); } else { s.text(b"no hay tarjeta"); }
        s.byte(b'\n');
        return;
    }
    if what == b"link" {
        label(s, b"enlace");
        link(s, present, mbit);
        s.byte(b'\n');
        return;
    }
    if what == b"frames" {
        label(s, b"tramas");
        s.dec(frames_rx);
        if !armed { s.text(b"   (el receptor NO esta armado)"); }
        s.byte(b'\n');
        return;
    }
    if what == b"phy" {
        label(s, b"PHYstatus");
        s.text(b"0x");
        s.hex(phy, 2);
        s.text(b"   (crudo, sin interpretar)");
        s.byte(b'\n');
        return;
    }

    section(s, b"RED");

    // 1. Hay tarjeta? Si no, lo demas no significa nada y se dice.
    label(s, b"tarjeta");
    if !present {
        s.text(b"NINGUNA reconocida en el PCI");
        s.byte(b'\n');
        s.text(b"    (sin tarjeta, el resto del informe no significa nada)\n");
        return;
    }
    let vd = bmo::info(bmo::INFO_NET_VENDOR_DEVICE);
    s.text(b"vendor:device 0x");
    s.hex(vd, 8);
    // El unico nombre que se traduce, porque es el que hay en esta placa y
    // reconocerlo de un vistazo ahorra buscarlo.
    if vd == 0x10EC_8168 { s.text(b"   (Realtek RTL8168)"); }
    s.byte(b'\n');

    let pci = bmo::info(bmo::INFO_NET_PCI);
    label(s, b"en el bus");
    s.dec((pci >> 16) & 0xFF);
    s.byte(b':');
    s.dec((pci >> 8) & 0xFF);
    s.byte(b'.');
    s.dec(pci & 0xFF);
    s.byte(b'\n');

    label(s, b"MAC");
    mac_hex(s, mac);
    s.byte(b'\n');

    // 2. Hay enlace?
    label(s, b"enlace");
    link(s, present, mbit);
    s.byte(b'\n');

    label(s, b"PHYstatus");
    s.text(b"0x");
    s.hex(phy, 2);
    s.text(b"   (crudo: la prueba, no la opinion)\n");

    // 3. Estamos escuchando? 4. Llega algo?
    label(s, b"receptor");
    if armed { s.text(b"ARMADO"); } else { s.text(b"apagado   (net rx en Ring 0)"); }
    s.byte(b'\n');

    label(s, b"tramas");
    s.dec(frames_rx);
    if armed && frames_rx == 0 {
        s.text(b"   (escuchando y nadie habla todavia)");
    }
    s.byte(b'\n');

    // ** Y lo que NO hace, dicho aqui y no en un README.
    s.text(b"    transmitir: CERRADO a proposito (CR.TE apagado)\n");
}

/// Los seis bytes con dos puntos, del mas significativo al menos.
fn mac_hex(s: &mut Output, mac: u64) {
    let mut i = 6;
    while i > 0 {
        i -= 1;
        s.hex((mac >> (i * 8)) & 0xFF, 2);
        if i > 0 { s.byte(b'-'); }
    }
}

/// `ARRIBA, 100 Mbit` o `ABAJO`. El cero de megabits **es** la respuesta.
fn link(s: &mut Output, present: bool, mbit: u64) {
    if !present {
        s.text(b"no hay tarjeta");
    } else if mbit == 0 {
        s.text(b"ABAJO   (sin cable, o el otro extremo apagado)");
    } else {
        s.text(b"ARRIBA, ");
        s.dec(mbit);
        s.text(b" Mbit");
    }
}

// -- EL CENSO DE EXTENSIONES ---------------------------------------------
//
// ** Por que este informe existe aqui y no solo en el shell de Ring 0:
// porque a ese shell no se vuelve. `ext` se escribio como orden del kernel, y
// una vez arranca el escritorio el rescate se niega --con razon-- a echar al
// que sostiene la casa. O sea que era una tabla correcta que su dueno no podia
// mirar. Lo dijo el con estas palabras: *"escribi el ext y no conoce"*.
//
// ** Y por que se agrupa por ESTADO y no por familia, al reves que el panel del
// kernel: porque la tinta de esta rejilla va POR LINEA (ver `output.rs`). Con
// una linea por familia, "lo usa", "no lo usa" y "CONFLICTO" caerian en el
// mismo renglon y tendrian que compartir color -- o sea que el color no diria
// nada. Agrupando por estado, cada renglon tiene UN significado y puede tener
// SU color. La restriccion de la rejilla eligio el diseno, y eligio bien.

/// Los nombres de una mascara, envueltos a la anchura de la rejilla.
///
/// El nombre lo da el KERNEL (`INFO_TXT_EXT_NOMBRE | i << 8`): aqui no hay una
/// copia de treinta y seis cadenas que se desincronice el dia que alguien
/// anada una fila al censo.
fn ext_grupo(s: &mut Output, titulo: &[u8], tinta: u8, n: u64, mascara: u64, notas: bool) -> u32 {
    let mut cuantos = 0u32;
    let mut col = 0usize;
    let mut buf = [0u8; 32];
    let mut nota = [0u8; 96];

    for i in 0..n {
        if mascara & (1u64 << i) == 0 {
            continue;
        }
        let ln = bmo::info_texto(bmo::INFO_TXT_EXT_NOMBRE | (i << 8), &mut buf);
        if ln == 0 {
            continue;
        }
        if cuantos == 0 {
            s.with_ink(tinta);
            // ** La etiqueta es de la LISTA, no de la tabla de motivos.
            //
            // Pintarla siempre metia 18 espacios delante de la PRIMERA fila de
            // motivos, que ademas se pone su propia sangria de 4: la primera
            // salia a 22 y las otras treinta a 4. Se vio en el Ryzen el 16-08
            // --`SSE4.1` desplazado y el resto alineado-- y no antes, porque un
            // renglon torcido no lo caza ningun test de este arbol.
            if !notas {
                label(s, titulo);
                col = 18;
            }
        }
        cuantos += 1;

        // Una fila por extension cuando se piden los motivos: el motivo es una
        // frase, y dos frases en un renglon no se leen. Sin motivos van
        // seguidas, que es como se mira una lista de banderas.
        if notas {
            // ** El motivo se ENVUELVE, no se corta.
            //
            // La primera version lo truncaba a lo que quedara de renglon, y la
            // foto del Ryzen del 16-08 lo enseno entero: "sem-asm no sabe V",
            // "en vez de un buc", "TODA pagina que BMO mapea es ej". Media
            // frase no es una version corta de la frase: es otra frase, y
            // encima una que parece que el sistema se quedo a medias.
            //
            // La sangria es 20 --cuatro de margen y el nombre a 16-- y no 34:
            // el motivo mas largo del censo mide 68 caracteres, asi que asi
            // cabe entero en un renglon de 88. Medido, no estimado.
            s.text(b"    ");
            s.text(&buf[..ln]);
            for _ in ln..16 {
                s.byte(b' ');
            }
            let nn = bmo::info_texto(bmo::INFO_TXT_EXT_NOTA | (i << 8), &mut nota);
            envolver(s, &nota[..nn], 20, OUT_COLS - 22);
            continue;
        }

        // +2 por el espacio de separacion. Si no cabe, se sigue debajo
        // alineado con la primera: una lista que desborda el margen se pierde
        // por la derecha sin decirlo.
        if col + ln + 2 >= OUT_COLS - 2 {
            s.byte(b'\n');
            label(s, b"");
            col = 18;
        }
        s.text(&buf[..ln]);
        s.byte(b' ');
        s.byte(b' ');
        col += ln + 2;
    }
    if cuantos > 0 && !notas {
        s.byte(b'\n');
    }
    s.with_ink(INK_PLAIN);
    cuantos
}

/// Escribe `texto` cortando por ESPACIOS, con las continuaciones sangradas.
///
/// Cortar por palabras y no por caracteres es la diferencia entre una frase que
/// sigue debajo y una frase partida a mitad de palabra. `ancho` es lo que cabe
/// contando desde `sangria`.
fn envolver(s: &mut Output, texto: &[u8], sangria: usize, ancho: usize) {
    let mut i = 0usize;
    let mut primera = true;
    while i < texto.len() {
        if !primera {
            for _ in 0..sangria {
                s.byte(b' ');
            }
        }
        if texto.len() - i <= ancho {
            s.text(&texto[i..]);
            s.byte(b'\n');
            return;
        }
        // El ultimo espacio que cabe. Si no hay ninguno --una palabra mas larga
        // que el renglon-- se corta en seco: es lo unico que se puede hacer, y
        // es mejor que un bucle que no avanza.
        let mut corte = ancho;
        while corte > 0 && texto[i + corte] != b' ' {
            corte -= 1;
        }
        if corte == 0 {
            corte = ancho;
        }
        s.text(&texto[i..i + corte]);
        s.byte(b'\n');
        i += corte;
        while i < texto.len() && texto[i] == b' ' {
            i += 1;
        }
        primera = false;
    }
}

/// `ext` -- que ofrece este silicio y que coge BMO.
#[inline(never)]
pub(crate) fn report_ext(s: &mut Output) {
    let n = bmo::info(bmo::INFO_CPU_EXT_N);
    if n == 0 {
        s.with_ink(INK_ERR);
        s.text(b"    este kernel no sabe censar extensiones (campo 0x31 vacio)\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    let hay = bmo::info(bmo::INFO_CPU_EXT_HAY);
    let usa = bmo::info(bmo::INFO_CPU_EXT_USA);

    section(s, b"extensiones: que ofrece el silicio");

    // El orden es el de la decision, no el del manual: primero lo que se
    // aprovecha, luego lo que esta ahi sin usar --que es la lista para la que
    // el censo existe-- y al final lo que no hay.
    let usadas = ext_grupo(s, b"BMO usa", INK_GOOD, n, hay & usa, false);
    let sobra = ext_grupo(s, b"hay, sin usar", INK_PLAIN, n, hay & !usa, false);
    let no_hay = ext_grupo(s, b"no hay", INK_PLAIN, n, !hay & !usa, false);

    // ** El conflicto: USADA Y NO DECLARADA. No es una curiosidad, es una
    // instruccion que dara #UD la primera vez que se ejecute. Va en rojo y va
    // sola, y si no hay ninguna esta linea NO sale: un renglon que dice "0
    // conflictos" en cada volcado deja de leerse a la tercera vez.
    let conflictos = ext_grupo(s, b"CONFLICTO", INK_ERR, n, usa & !hay, false);

    label(s, b"total");
    s.dec(usadas as u64);
    s.text(b" usadas de ");
    s.dec((usadas + sobra) as u64);
    s.text(b" presentes, ");
    s.dec(no_hay as u64);
    s.text(b" ausentes, de ");
    s.dec(n);
    s.text(b" censadas\n");

    // -- Los cuatro que tienen que ser cero -------------------------------
    //
    // Tres de ellos NO se pueden deducir de las mascaras: son sobre la TABLA y
    // no sobre el silicio. Un panel que solo supiera de conflictos diria que
    // todo va bien mientras una fila esta sin motivo escrito.
    let av = bmo::info(bmo::INFO_CPU_EXT_AVERIAS);
    let (conf, mudas) = (av & 0xFFFF, (av >> 16) & 0xFFFF);
    let (rep, sin_sitio) = ((av >> 32) & 0xFFFF, (av >> 48) & 0xFFFF);
    let averias = conf + mudas + rep + sin_sitio;
    s.with_ink(if averias == 0 { INK_GOOD } else { INK_ERR });
    // ** `label` rellena hasta 14 y esta etiqueta mide 15, asi que no quedaba
    // ni un espacio: en el Ryzen salio `tiene que ser 0conflictos 0`. Pegado,
    // el 0 del rotulo se lee como parte del primer numero -- justo el dato que
    // esta fila existe para vigilar.
    s.text(b"    tiene que ser 0   ");
    s.text(b"conflictos ");
    s.dec(conf);
    s.text(b"   mudas ");
    s.dec(mudas);
    s.text(b"   repetidas ");
    s.dec(rep);
    s.text(b"   sin sitio ");
    s.dec(sin_sitio);
    s.byte(b'\n');
    s.with_ink(INK_PLAIN);

    // ** La columna que convierte el censo en una decision. Sin ella esto es
    // trivia: la pregunta no es "que tiene el CPU", es "que me daria".
    if sobra > 0 {
        section(s, b"lo que hay y no se coge, y lo que daria");
        ext_grupo(s, b"", INK_PLAIN, n, hay & !usa, true);
    }
    if conflictos > 0 {
        s.with_ink(INK_ERR);
        s.text(b"    un CONFLICTO es una instruccion que dara #UD en esta maquina\n");
        s.with_ink(INK_PLAIN);
    }
}
