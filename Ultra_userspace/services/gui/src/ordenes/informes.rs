//! `info`, `cpu`, `mem` -- los datos del sistema, escritos a la rejilla.
//!
//! Leer un contador no ejerce ningun poder, asi que esto vive en Ring 3 y pide
//! los datos por `OP_INFO` como cualquier otro proceso.

use bmo_userland as bmo;

use crate::escena::salida::{Salida, TINTA_BIEN, TINTA_ECO, TINTA_MAL, TINTA_NORMAL};
use crate::escena::SAL_COLS;

// -- Los informes del sistema --------------------------------------------
//
// Se pintan aqui, en Ring 3, con datos que el kernel contesta por `OP_INFO`.
// El kernel da enteros; las unidades, los porcentajes, las barras y el color
// son de este lado.

/// Un rotulo de seccion, para que el informe no sea un muro de renglones.
pub(crate) fn seccion(s: &mut Salida, titulo: &[u8]) {
    s.con_tinta(TINTA_ECO);
    s.texto(b"  ");
    s.texto(titulo);
    s.byte(b' ');
    // Una regla hasta el margen: cuesta nada y separa de verdad.
    let usado = 3 + titulo.len();
    for _ in usado..SAL_COLS.saturating_sub(2) {
        s.byte(b'-');
    }
    s.byte(b'\n');
    s.con_tinta(TINTA_NORMAL);
}

/// Un renglon `etiqueta ....... valor`, con la etiqueta a ancho fijo.
pub(crate) fn etiqueta(s: &mut Salida, name: &[u8]) {
    s.texto(b"    ");
    s.texto(name);
    for _ in name.len()..14 {
        s.byte(b' ');
    }
}

pub(crate) fn informe_cpu(s: &mut Salida) {
    let mut buf = [0u8; 64];

    seccion(s, b"procesador");
    let n = bmo::info_texto(bmo::INFO_TXT_CPU_VENDOR, &mut buf);
    etiqueta(s, b"fabricante");
    s.texto(&buf[..n]);
    s.byte(b'\n');

    let n = bmo::info_texto(bmo::INFO_TXT_CPU_NOMBRE, &mut buf);
    etiqueta(s, b"modelo");
    s.texto(&buf[..n]);
    s.byte(b'\n');

    let n = bmo::info_texto(bmo::INFO_TXT_UARCH, &mut buf);
    etiqueta(s, b"uarch");
    s.texto(&buf[..n]);
    let n2 = bmo::info_texto(bmo::INFO_TXT_FAMILIA, &mut buf);
    if n2 > 0 {
        s.texto(b"   familia ");
        s.texto(&buf[..n2]);
    }
    s.byte(b'\n');

    etiqueta(s, b"nucleos");
    s.dec(bmo::info(bmo::INFO_CPU_NUCLEOS));
    s.texto(b" fisicos / ");
    s.dec(bmo::info(bmo::INFO_CPU_HILOS));
    s.texto(b" hilos\n");

    // Hz -> GHz con dos decimales, con enteros. El TSC es la frecuencia MEDIDA
    // en el arranque, no el numero de la etiqueta de la caja.
    let hz = bmo::info(bmo::INFO_TSC_HZ);
    etiqueta(s, b"tsc");
    s.dec(hz / 1_000_000_000);
    s.byte(b'.');
    let frac = (hz % 1_000_000_000) / 10_000_000;
    if frac < 10 {
        s.byte(b'0');
    }
    s.dec(frac);
    s.texto(b" GHz   (medido)\n");

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
    let real = bmo::info(bmo::INFO_CPU_HZ_REAL);
    etiqueta(s, b"ahora");
    if real == 0 {
        // Cero no es cero hercios: es "no se puede medir". Decirlo con palabras
        // evita que alguien lea un 0.00 GHz y crea que el CPU esta parado.
        s.con_tinta(TINTA_ECO);
        s.texto(b"sin MPERF/APERF, o aun sin dos lecturas");
        s.con_tinta(TINTA_NORMAL);
        s.byte(b'\n');
    } else {
        s.dec(real / 1_000_000_000);
        s.byte(b'.');
        let f2 = (real % 1_000_000_000) / 10_000_000;
        if f2 < 10 {
            s.byte(b'0');
        }
        s.dec(f2);
        s.texto(b" GHz   ");
        s.con_tinta(TINTA_ECO);
        if real > hz {
            s.texto(b"(boost)");
        } else if real + 200_000_000 < hz {
            s.texto(b"(bajando)");
        } else {
            s.texto(b"(en base)");
        }
        s.con_tinta(TINTA_NORMAL);
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
    let mwn = bmo::info(bmo::INFO_CPU_MW_NUCLEOS);
    etiqueta(s, b"gasta");
    if mw == 0 {
        s.con_tinta(TINTA_ECO);
        s.texto(b"sin RAPL, o aun sin dos lecturas");
        s.con_tinta(TINTA_NORMAL);
        s.byte(b'\n');
    } else {
        s.dec(mw / 1000);
        s.byte(b'.');
        s.dec((mw % 1000) / 100);
        s.texto(b" W paquete");
        if mwn > 0 {
            s.texto(b" / ");
            s.dec(mwn / 1000);
            s.byte(b'.');
            s.dec((mwn % 1000) / 100);
            s.texto(b" W nucleos");
        }
        s.con_tinta(TINTA_ECO);
        // La resta se dice porque no es obvia: lo que va del nucleo al paquete
        // es Infinity Fabric, controlador de memoria y L3 -- y ese consumo NO
        // baja aunque se apaguen nucleos.
        s.texto(b"   (el resto: fabric + memoria + L3)");
        s.con_tinta(TINTA_NORMAL);
        s.byte(b'\n');
    }

    // -- SMP, y lo que cuesta --------------------------------------------
    //
    // Los nucleos en pie y los choques de cerrojo van en el MISMO informe a
    // proposito. Un panel que solo ensena "12 de 12" cuenta la mitad bonita:
    // la otra mitad es si esos once obreros estan peleandose con el kernel
    // por dentro, y ese numero tiene que ser cero.
    let vivos = bmo::info(bmo::INFO_SMP_VIVOS);
    etiqueta(s, b"smp");
    if vivos == 0 {
        s.texto(b"solo el BSP");
        s.con_tinta(TINTA_ECO);
        s.texto(b"   (`smp all` levanta los demas)");
        s.con_tinta(TINTA_NORMAL);
        s.byte(b'\n');
    } else {
        s.con_tinta(TINTA_BIEN);
        s.dec(vivos + 1);
        s.con_tinta(TINTA_NORMAL);
        s.texto(b" nucleos en pie de ");
        s.dec(bmo::info(bmo::INFO_CPU_HILOS));
        s.byte(b'\n');
    }

    let choques = bmo::info(bmo::INFO_SPIN_CHOQUES);
    etiqueta(s, b"cerrojos");
    if choques == 0 {
        s.con_tinta(TINTA_BIEN);
        s.texto(b"0 choques");
        s.con_tinta(TINTA_ECO);
        s.texto(b"   (lo correcto: nadie pelea)");
        s.con_tinta(TINTA_NORMAL);
        s.byte(b'\n');
    } else {
        // No es una cifra de rendimiento: es que alguien entro en el kernel
        // desde otro nucleo. Se pinta como lo que es.
        s.con_tinta(TINTA_MAL);
        s.dec(choques);
        s.texto(b" CHOQUES");
        s.con_tinta(TINTA_NORMAL);
        s.texto(b"   espera mayor ");
        s.dec(bmo::info(bmo::INFO_SPIN_PICO));
        s.texto(b" vueltas\n");
    }

    // * Y la otra mitad de lo mismo: cuando una tarea muere, el kernel dice
    // haber recuperado todo lo suyo. Esta fila es quien lo COMPRUEBA.
    //
    // Un numero distinto de cero no acusa al programa que murio: acusa al
    // KERNEL. Va aqui, al lado de los cerrojos, porque son la misma clase de
    // dato -- el sistema comprobandose a si mismo.
    let fugas = bmo::info(bmo::INFO_FUGAS);
    etiqueta(s, b"fugas");
    if fugas == 0 {
        s.con_tinta(TINTA_BIEN);
        s.texto(b"0");
        s.con_tinta(TINTA_ECO);
        s.texto(b"   (los muertos devolvieron todo)");
        s.con_tinta(TINTA_NORMAL);
        s.byte(b'\n');
    } else {
        s.con_tinta(TINTA_MAL);
        s.dec(fugas);
        s.texto(b" RECURSOS SIN DEVOLVER");
        s.con_tinta(TINTA_NORMAL);
        s.texto(b"   escribe `fallo`\n");
    }
}

pub(crate) fn informe_memoria(s: &mut Salida) {
    let total = bmo::info(bmo::INFO_RAM_TOTAL);
    let libre = bmo::info(bmo::INFO_RAM_LIBRE);
    let usada = total.saturating_sub(libre);

    seccion(s, b"memoria");
    etiqueta(s, b"total");
    s.tamano(total);
    s.texto(b"   ");
    s.dec_der(bmo::info(bmo::INFO_RAM_MARCOS), 8);
    s.texto(b" marcos de 4 KiB\n");

    etiqueta(s, b"usada");
    s.tamano(usada);
    s.texto(b"   ");
    s.barra(usada, total, 24);
    s.byte(b' ');
    s.pct(usada, total);
    s.byte(b'\n');

    etiqueta(s, b"libre");
    s.tamano(libre);
    s.texto(b"   ");
    s.dec_der(bmo::info(bmo::INFO_RAM_MARCOS_LIBRES), 8);
    s.texto(b" marcos\n");

    // El tamano REAL del kernel en RAM, medido hasta el final de su .bss.
    etiqueta(s, b"kernel");
    s.tamano(bmo::info(bmo::INFO_KERNEL_BYTES));
    s.texto(b"   en 0x400000\n");

    // * Lo que Ring 3 ha PEDIDO. Las cuatro filas de arriba las sabe el kernel
    // porque la memoria la reparte el; esta solo se mueve si un programa
    // ejercio `KIND_MEMORIA`. Por eso vale como prueba: es el kernel diciendo
    // que entrego, no el programa diciendo que recibio.
    //
    // En cero se dice EXPRESAMENTE que nadie ha pedido, en vez de pintar un
    // `0 B` que se lee igual que "no lo se".
    let pedida = bmo::info(bmo::INFO_MEM_ENTREGADA);
    etiqueta(s, b"a Ring 3");
    if pedida == 0 {
        s.texto(b"ningun programa ha pedido memoria\n");
    } else {
        s.tamano(pedida);
        s.texto(b"   pedida con KIND_MEMORIA\n");
    }
}

pub(crate) fn informe_sistema(s: &mut Salida) {
    s.con_tinta(TINTA_ECO);
    s.texto(b"  BMO-X - informe del sistema\n");
    s.con_tinta(TINTA_NORMAL);

    informe_cpu(s);
    informe_memoria(s);

    seccion(s, b"tareas");
    let total = bmo::info(bmo::INFO_TAREAS_TOTAL);
    let libres = bmo::info(bmo::INFO_TAREAS_LIBRES);
    let ranuras = total + libres;
    etiqueta(s, b"ranuras");
    s.dec(total);
    s.texto(b" en uso de ");
    s.dec(ranuras);
    s.texto(b"   ");
    s.barra(total, ranuras, 24);
    s.byte(b'\n');
    etiqueta(s, b"listas");
    s.dec(bmo::info(bmo::INFO_TAREAS_LISTAS));
    s.texto(b"   ticks ");
    s.dec(bmo::info(bmo::INFO_TICKS));
    s.byte(b'\n');
    etiqueta(s, b"programas");
    let vistos = bmo::info(bmo::INFO_PROGRAMAS);
    let olvidados = bmo::info(bmo::INFO_PROGRAMAS_OLVIDADOS);
    s.dec(vistos + olvidados);
    s.texto(b" lanzados");
    if olvidados > 0 {
        s.texto(b"   (");
        s.dec(olvidados);
        s.texto(b" ya no caben en la bitacora)");
    }
    s.byte(b'\n');

    seccion(s, b"disco");
    etiqueta(s, b"disco");
    if bmo::info(bmo::INFO_DISCO_LISTO) != 0 {
        s.con_tinta(TINTA_BIEN);
        s.texto(b"listo");
    } else {
        s.con_tinta(TINTA_MAL);
        s.texto(b"sin disco");
    }
    s.con_tinta(TINTA_NORMAL);
    s.byte(b'\n');
    etiqueta(s, b"datos");
    if bmo::info(bmo::INFO_DATOS_MONTADO) != 0 {
        s.con_tinta(TINTA_BIEN);
        s.texto(b"montado para escritura");
    } else {
        // La linea que decide si el File I/O de COBOL puede funcionar. Decirlo
        // aqui ahorra buscar el fallo en el programa.
        s.con_tinta(TINTA_MAL);
        s.texto(b"NO montado: sin esto no hay OPEN ni WRITE");
    }
    s.con_tinta(TINTA_NORMAL);
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
pub(crate) fn informe_autopsia(s: &mut Salida) {
    seccion(s, b"ultimo fallo");
    let total = bmo::autopsia_total();
    if total == 0 {
        s.con_tinta(TINTA_BIEN);
        s.texto(b"    ningun fallo de Ring 3 desde el arranque\n");
        s.con_tinta(TINTA_NORMAL);
        return;
    }
    let filas = bmo::autopsia_renglones(0);
    let mut buf = [0u8; 96];
    for f in 0..filas {
        let n = bmo::autopsia_linea(0, f, &mut buf);
        s.texto(b"    ");
        // El titulo en rojo y el cuerpo normal: lo que se busca de un vistazo
        // es CUAL fue y cuando, no el `rsp`.
        if f == 0 {
            s.con_tinta(TINTA_MAL);
        }
        s.texto(&buf[..n]);
        if f == 0 {
            s.con_tinta(TINTA_NORMAL);
        }
        s.byte(b'\n');
    }
    if total > 1 {
        s.con_tinta(TINTA_ECO);
        s.texto(b"    (van ");
        s.dec(total);
        s.texto(b" desde el arranque; se guardan las 4 ultimas)\n");
        s.con_tinta(TINTA_NORMAL);
    }
}
