//! TOCAR MEMORIA -- el monton, los anchos, y el primer framebuffer.
//!
//! Que un programa pueda pedir un bloque, repartirlo, y escribir dentro con el
//! ancho que toca. La puerta se abrio en F4a y al otro lado no habia manos:
//! esto es las manos.

use super::*;
// ===================================================================
//  ** F4b -- LA MEMORIA. La puerta se abrio y al otro lado no habia manos.
// ===================================================================
//
//  F4a dejo a INTI pidiendole un bloque al kernel y **sin poder tocarlo**: no
//  habia forma de leer ni escribir una direccion. Un lenguaje de sistema al que
//  le falta eso no es un lenguaje de sistema, es una calculadora con syscalls.

/// Escribir y volver a leer. Lo minimo, y lo que no estaba.
#[test]
fn una_direccion_se_escribe_y_se_lee() {
    let f = "\
perfil llano
usa memoria

funcion principal devuelve entero32
    crudo
        escribe_natural64(0x200000, 12345)
        devuelve lee_natural64(0x200000)
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 12345);
}

/// ** Un byte se lee ENTERO y sin basura detras.
///
/// Se lee con `movzx` y no con un `mov` de 8 bits, y la diferencia importa: el
/// `mov` dejaria intactos los 56 bits de arriba, asi que el resultado traeria
/// lo que hubiera antes en el registro. **Y funcionaria casi siempre** -- solo
/// fallaria cuando el registro viniera sucio, que es cuando ya nadie mira.
///
/// Por eso el test ensucia el registro a proposito antes de leer: escribe un
/// numero grande, lo lee, y luego lee un byte.
#[test]
fn un_byte_se_lee_entero_y_sin_arrastrar_lo_de_antes() {
    let f = "\
perfil llano
usa memoria

funcion principal devuelve entero32
    crudo
        escribe_natural64(0x200000, 0x1122334455667788)
        sucio = lee_natural64(0x200000)
        escribe_natural8(0x300000, 200)
        devuelve lee_natural8(0x300000) + sucio - sucio
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 200);
}

/// ** LA PRUEBA DE F4b: el programa le PIDE memoria al kernel y la USA.
///
/// El camino entero, y cada paso es uno que no existia hace dos commits:
///
///   1. cruza la puerta para pedir un bloque       (F4a)
///   2. recoge el HANDLE, no el codigo             (el fallo que destapo esto)
///   3. vuelve a cruzar para preguntar por su base
///   4. escribe en esa direccion                   (F4b)
///   5. la lee
///   6. y sale por la puerta con lo que leyo       (F4a)
///
/// ** Y `mi_tarea` es un nombre, no un `-2`. Un programa que escribiera el
/// numero crudo compilaria igual y no se entenderia nunca mas.
#[test]
fn el_programa_pide_memoria_al_kernel_y_la_usa() {
    let f = "\
perfil llano
usa bmo
usa memoria

funcion principal devuelve entero32
    crudo
        bloque = invoca_valor(mi_tarea, 0x15, 4096, 0, 0)
        base = invoca_valor(bloque, 0x01, 0, 0, 0)
        escribe_natural64(base, 4321)
        devuelve lee_natural64(base)
";
    let m = arranca(f);
    assert_eq!(
        m.syscalls.last().unwrap().arg0,
        4321,
        "lo que se escribio en la memoria del kernel es lo que se leyo"
    );
    assert_eq!(m.memoria_entregada(), 4096, "y el kernel entrego lo pedido");
}

/// ** `invoca_valor` recoge el VALOR, no el codigo. Corriendo, no leyendo.
///
/// Este es el fallo que F4a se llevo puesto sin enterarse: la puerta contesta
/// DOS cosas a la vez --el codigo en un registro y el valor en otro-- y el
/// emisor leia el mismo para los dos.
///
/// El sintoma habria sido perfecto para no encontrarlo nunca: `invoca_valor`
/// devolvia el codigo, que en el caso bueno vale CERO. O sea que todo puntero
/// pedido al kernel habria valido cero, que es exactamente lo que devuelve un
/// kernel que dice que no.
#[test]
fn invoca_valor_recoge_el_valor_y_no_el_codigo() {
    let comun = "\
perfil llano
usa bmo

funcion principal devuelve entero32
    devuelve ";

    // El codigo de una peticion que sale bien es 0.
    let codigo = format!("{}invoca(mi_tarea, 0x15, 4096, 0, 0)\n", comun);
    assert_eq!(arranca(&codigo).syscalls.last().unwrap().arg0, 0);

    // El valor es un handle, y un handle no es cero.
    let valor = format!("{}invoca_valor(mi_tarea, 0x15, 4096, 0, 0)\n", comun);
    let h = arranca(&valor).syscalls.last().unwrap().arg0;
    assert_ne!(h, 0, "un handle de memoria no puede ser cero");
}

/// Y los dos leen de registros distintos, dicho donde se decide.
#[test]
fn la_puerta_tiene_dos_registros_de_respuesta() {
    let t = Taller::nuevo();
    assert_ne!(
        t.puerta.codigo, t.puerta.valor,
        "el codigo y el valor no pueden volver por el mismo sitio"
    );
    assert_eq!(t.puerta.recogida(Some("valor")), t.puerta.valor);
    assert_eq!(t.puerta.recogida(Some("codigo")), t.puerta.codigo);
    // Lo desconocido se trata como codigo: es lo unico seguro.
    assert_eq!(t.puerta.recogida(None), t.puerta.codigo);
}

// ===================================================================
//  ** F4c -- EL MONTON, y en piezas
// ===================================================================
//
//  Peticion de Eddi: *"si MONTON es monolitico = modular, para poder evitar
//  problemas o choques. INTI como siempre modular"*.
//
//  Y la primera consecuencia de tomarselo en serio fue **descubrir que yo me
//  habia equivocado**: dije que el monton estaba bloqueado por las variables de
//  modulo. Lo esta un monton MONOLITICO, el de C, que guarda su estado en una
//  global escondida.
//
//  Uno modular no lo necesita: **el estado del monton vive DENTRO del monton**.
//
//      monton + 0   libre   la primera direccion sin repartir
//      monton + 8   fin     la primera que ya no es suya
//      monton + 16  ...     desde aqui se reparte
//
//  ** Y eso no es un apano para esquivar una funcionalidad que falta: es mejor.
//  Un `malloc` con estado global es autoridad ambiente -- cualquiera reparte de
//  lo mismo sin haberlo pedido. `pide(monton, n)` tiene la forma de una
//  capability: **para repartir de un monton hay que tenerlo**.
//
//  Las piezas, y la unica frontera entre ellas es la tabla de arriba:
//
//      origen.inti    habla con el kernel   y NO sabe repartir
//      reparto.inti   sabe repartir         y NO habla con el kernel

const CON_MONTON: &str = "\
perfil llano
usa monton

funcion principal devuelve entero32
";

/// ** LA PRUEBA DE F4c: el monton se pide, se reparte, y las cuentas salen.
#[test]
fn el_monton_reparte_y_los_trozos_no_se_pisan() {
    let f = format!(
        "{}{}{}{}",
        CON_MONTON,
        "    m = monton_nuevo(4096)
",
        "    a = pide(m, 8)
    b = pide(m, 8)
",
        "    devuelve b - a
"
    );
    // Ocho bytes pedidos, dieciseis de distancia: alineado, y sin solaparse.
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 16);
}

/// Lo repartido se puede USAR. Que es de lo que iba todo esto.
#[test]
fn en_lo_que_reparte_el_monton_se_puede_escribir() {
    let f = format!(
        "{}{}{}{}{}",
        CON_MONTON,
        "    m = monton_nuevo(4096)
",
        "    a = pide(m, 8)
    b = pide(m, 8)
",
        "    crudo
        escribe_natural64(a, 111)
",
        "        escribe_natural64(b, 222)
        devuelve lee_natural64(a) + lee_natural64(b)
"
    );
    // Si `a` y `b` se solaparan, esto daria 444.
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 333);
}

/// ** Un monton que se acaba dice que NO, y no reparte lo que no tiene.
///
/// Es la mitad que se olvida de todo asignador, y la que convierte un fallo de
/// memoria en una corrupcion silenciosa cuando falta: sin esta comprobacion,
/// `pide` devolveria una direccion **fuera del bloque** y el programa
/// escribiria en la memoria de otro.
#[test]
fn un_monton_lleno_contesta_cero() {
    let f = format!(
        "{}    m = monton_nuevo(4096)\n    devuelve pide(m, 100000)\n",
        CON_MONTON
    );
    assert_eq!(arranca(&f).syscalls.last().unwrap().arg0, 0);
}

/// Y lo que queda baja segun se reparte, que es como se comprueba que reparte
/// de verdad en vez de devolver direcciones sueltas.
#[test]
fn lo_que_queda_baja_segun_se_reparte() {
    let antes = format!("{}    m = monton_nuevo(4096)\n    devuelve queda_en(m)\n", CON_MONTON);
    let despues = format!(
        "{}    m = monton_nuevo(4096)\n    a = pide(m, 8)\n    devuelve queda_en(m)\n",
        CON_MONTON
    );
    let a = arranca(&antes).syscalls.last().unwrap().arg0;
    let d = arranca(&despues).syscalls.last().unwrap().arg0;
    assert_eq!(a, 4096 - 16, "la cabecera del monton ocupa 16");
    assert_eq!(d, a - 16, "y un trozo de 8 se lleva 16 por la alineacion");
}

/// El monton pide su memoria al KERNEL, no a una zona inventada.
#[test]
fn el_monton_sale_de_la_puerta() {
    let f = format!("{}    m = monton_nuevo(4096)\n    devuelve 0\n", CON_MONTON);
    let m = arranca(&f);
    assert_eq!(m.memoria_entregada(), 4096);
}

/// ** Y las piezas siguen siendo piezas: `usa monton` trae DOS ficheros, y el
/// orden en que llegan no lo elige el sistema de ficheros.
///
/// Sin el orden fijo, dos compilaciones del mismo fuente darian dos binarios
/// distintos -- y entonces "este .bex es el que audite" deja de poder decirse.
#[test]
fn el_monton_llega_en_piezas_y_en_orden() {
    let piezas = bmo_inti_front::tablas::Runtime::traer(&bmo_mods::Roots::find(), "monton");
    let nombres: Vec<&str> = piezas.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(nombres, vec!["origen.inti", "reparto.inti"]);
}

/// Un `usa` que no es una pieza no trae nada, y eso no es un error.
#[test]
fn un_usa_que_no_es_una_pieza_no_trae_nada() {
    let r = bmo_mods::Roots::find();
    assert!(bmo_inti_front::tablas::Runtime::traer(&r, "x86_64").is_empty());
    assert!(bmo_inti_front::tablas::Runtime::traer(&r, "bmo").is_empty());
    // Y un nombre que intente salirse del sitio no busca en ningun lado.
    assert!(bmo_inti_front::tablas::Runtime::traer(&r, "../monton").is_empty());
}

// ===================================================================
//  ** F5a -- LOS CUATRO ANCHOS, y el primer framebuffer
// ===================================================================
//
//  Faltaban el de 16 y el de 32. Estaba declarado en la tabla con su motivo
//  --`bmo_lower` no traia los ayudantes-- y se anadieron ALLI, que es donde
//  tenian que estar.
//
//  El de 32 no es uno mas: **es el que escribe un pixel**.

/// Cada ancho guarda y devuelve lo suyo, ni un bit mas.
#[test]
fn los_cuatro_anchos_van_y_vuelven() {
    for (bits, valor) in [
        (8u32, 200u64),
        (16, 60000),
        (32, 4000000000),
        (64, 12345678901234),
    ] {
        let f = format!(
            "perfil llano
usa memoria

funcion principal devuelve entero32
{}{}{}",
            "    crudo
",
            format!("        escribe_natural{}(0x200000, {})
", bits, valor),
            format!("        devuelve lee_natural{}(0x200000)
", bits)
        );
        assert_eq!(
            arranca(&f).syscalls.last().unwrap().arg0,
            valor,
            "ancho de {} bits",
            bits
        );
    }
}

/// ** Y lo de al lado NO se toca. Es la mitad que se olvida de un `escribe`.
///
/// Un `escribe_natural8` que en realidad escribiera cuatro bytes pasaria el
/// test de arriba tan campante -- lee lo mismo que escribio-- y **se llevaria
/// por delante los tres bytes siguientes**. En un array eso es el elemento de
/// al lado, y el fallo aparece en otra parte del programa.
#[test]
fn escribir_un_ancho_no_pisa_lo_de_al_lado() {
    let f = "\
perfil llano
usa memoria

funcion principal devuelve entero32
    crudo
        escribe_natural64(0x200000, 0xFFFFFFFFFFFFFFFF)
        escribe_natural8(0x200000, 0)
        devuelve lee_natural64(0x200000)
";
    // Solo el byte bajo a cero: quedan siete bytes de unos.
    assert_eq!(
        arranca(f).syscalls.last().unwrap().arg0,
        0xFFFF_FFFF_FFFF_FF00
    );
}

#[test]
fn escribir_dos_bytes_no_pisa_los_otros_seis() {
    let f = "\
perfil llano
usa memoria

funcion principal devuelve entero32
    crudo
        escribe_natural64(0x200000, 0xFFFFFFFFFFFFFFFF)
        escribe_natural16(0x200000, 0)
        devuelve lee_natural64(0x200000)
";
    assert_eq!(
        arranca(f).syscalls.last().unwrap().arg0,
        0xFFFF_FFFF_FFFF_0000
    );
}

/// ** EL PRIMER FRAMEBUFFER DE INTI.
///
/// Pide memoria al kernel, la reparte con su propio monton, y **rellena
/// pixeles de 32 bits en un bucle**. Que es, quitando el nombre bonito, lo que
/// hace un motor grafico en su linea mas caliente.
///
/// Aqui se juntan las cuatro piezas de hoy y ninguna sobra:
///
///   F4a  arranca solo y sale por la puerta
///   F4b  toca memoria
///   F4c  el monton se la reparte
///   F5a  y el ancho de 32 es el que cabe un pixel
#[test]
fn inti_rellena_una_pantalla_de_pixeles() {
    let f = "\
perfil llano
usa monton
usa memoria

funcion pinta(pantalla es natural64, cuantos es natural64, color es natural64)
    crudo
        cambiante i = 0
        repite mientras i < cuantos
            escribe_natural32(pantalla + i * 4, color)
            i = i + 1

funcion principal devuelve entero32
    m = monton_nuevo(4096)
    p = pide(m, 64)
    pinta(p, 16, 65280)
    crudo
        devuelve lee_natural32(p + 40)
";
    // El pixel 10 de 16, y ninguno se escribio dos veces ni se quedo sin
    // escribir: si el bucle contara mal, este seria cero.
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 65280);
}

/// Y el ultimo pixel se escribe, que es donde se ve si el bucle se queda corto.
#[test]
fn el_ultimo_pixel_tambien_se_pinta() {
    let f = "\
perfil llano
usa monton
usa memoria

funcion pinta(pantalla es natural64, cuantos es natural64, color es natural64)
    crudo
        cambiante i = 0
        repite mientras i < cuantos
            escribe_natural32(pantalla + i * 4, color)
            i = i + 1

funcion principal devuelve entero32
    m = monton_nuevo(4096)
    p = pide(m, 64)
    pinta(p, 16, 7)
    crudo
        devuelve lee_natural32(p + 60)
";
    assert_eq!(arranca(f).syscalls.last().unwrap().arg0, 7);
}
