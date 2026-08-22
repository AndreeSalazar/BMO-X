//! LAS TRES FRASES con las que se define INTI, hechas assert.
//!
//! Sintaxis de Python, control de ensamblador, fuera del syscall. Se usan
//! enteras para describir el lenguaje, asi que tienen que poder fallar enteras.

use super::*;
// ===================================================================
//  ** LAS TRES FRASES CON LAS QUE SE DEFINE INTI, hechas assert
// ===================================================================
//
//  Eddi lo dice asi: *"INTI es inspiracion de Python en sintaxis, pero nivel de
//  rendimiento de ASM, y fuera del syscall"*.
//
//  Son tres afirmaciones distintas y **dos de ellas se pueden comprobar aqui
//  mismo**. La tercera --la sintaxis-- no se mide con un test: se mide leyendo,
//  y para eso esta el censo.
//
//  ** Por que estan juntas en un bloque en vez de repartidas: porque la frase se
//  usa entera para describir el lenguaje, y una frase que se usa entera tiene
//  que poder fallar entera. Si algun dia una de estas deja de ser verdad, lo
//  honesto es dejar de decirla.

/// Un programa que CALCULA no cruza la puerta ni una vez.
///
/// ## ** Que se esta comprobando de verdad
///
/// Que la aritmetica de INTI no pasa por ningun runtime que a su vez hable con
/// el kernel. En Python `2 + 2` recorre el despacho de objetos; aqui son dos
/// instrucciones y la puerta ni aparece en los bytes.
///
/// Y no es una perogrullada: el maestro tiene un numero para esto --**969
/// ciclos** cuesta cruzar la puerta contra **20** una llamada-- y toda la
/// arquitectura del lenguaje se decidio con el delante. Este test es lo que
/// impide que esa decision se erosione sin que nadie lo note.
#[test]
fn un_programa_que_calcula_no_cruza_la_puerta() {
    let f = "\
perfil llano

funcion media(a es entero64, b es entero64) devuelve entero64
    cambiante t = 0
    cambiante i = 0
    repite mientras i < 10
        t = t + a * i + b
        i = i + 1
    devuelve t entre 10
";
    let e = emitido(f);
    // `0F 05` es la puerta. En un programa que solo cuenta, no puede estar.
    assert!(
        !e.codigo.windows(2).any(|w| w == [0x0F, 0x05]),
        "un programa que solo calcula esta cruzando la puerta"
    );
    // Y ademas corre y da el numero: sin esto seria un test que aprueba un
    // binario vacio, que efectivamente no cruza ninguna puerta.
    assert_eq!(ejecuta(f, 2, 3), (0..10).map(|i| 2 * i + 3).sum::<u64>() / 10);
}

/// **EL BUCLE MAS CALIENTE QUE INTI SABE ESCRIBIR HOY, sin una sola llamada.**
///
/// ## ** Esta es la frase de "nivel de ASM", y aqui esta lo que significa
///
/// No significa *"va tan rapido como el ensamblador que escribiria un experto"*
/// -- eso es medible y todavia no esta medido. Significa algo mas estrecho y
/// que si se puede comprobar: **entre el fuente y la instruccion no hay nadie**.
/// Ni despacho, ni contador de referencias, ni una llamada por elemento.
///
/// Ese es exactamente el techo que Python no puede levantar, y no por lentitud
/// del interprete: `x + y` alli **es** una llamada, y lo seguiria siendo
/// compilado. Aqui el bucle entero son saltos y aritmetica.
#[test]
fn el_bucle_de_pixeles_no_llama_a_nadie() {
    let f = "\
perfil llano
usa memoria

funcion pinta(pantalla es bufer de natural32, cuantos es entero64, color es entero64)
    cambiante i = 0
    repite mientras i < cuantos
        crudo
            pantalla[i] = color
        i = i + 1
";
    let e = emitido(f);
    assert!(
        !e.codigo.windows(2).any(|w| w == [0x0F, 0x05]),
        "el bucle cruza la puerta"
    );
    // ** Y ninguna LLAMADA, que es la mitad que de verdad importa: un
    // rasterizador que llama una vez por pixel tiene un techo que ninguna
    // optimizacion posterior levanta.
    //
    // Se cuenta en la IR y NO buscando el byte de la instruccion. El primer
    // intento buscaba `E8` suelto en el codigo y fallaba: ese byte aparece
    // dentro de cualquier inmediato o desplazamiento que lo lleve. Un test que
    // da falsos positivos se desactiva en una semana, y entonces ya no vigila
    // nada -- es la misma leccion que `agnostico.rs` aprendio con `rsi` dentro
    // de `conversion`.
    assert_eq!(llamadas_de(f), 0, "el bucle llama a alguien por pixel");
}


/// ** Y LA MEDIDA, que es lo que convierte la frase en un numero.
///
/// Cuantas comprobaciones anti-UB lleva ese mismo bucle, y cuantas instrucciones
/// de maquina. Los dos numeros van a CABINA en cada compilacion, asi que **se
/// pueden seguir en el tiempo**: el dia que alguien anada una comprobacion de
/// mas en el sitio equivocado, el numero sube y se ve.
///
/// La seccion 6.3 del maestro dice que comprobar cuesta ~1%. Esto no lo mide
/// --medirlo pide el Ryzen-- pero dice **contra que** se va a medir, que es lo
/// unico que se puede saber hoy sin hardware.
#[test]
fn el_precio_de_no_tener_ub_esta_contado() {
    let f = "\
perfil llano

funcion suma(a es entero64, b es entero64) devuelve entero64
    devuelve a + b * 2
";
    // Dos operaciones que se pueden pasar de la cuenta, dos comprobaciones.
    // Ni una de mas: comparar no puede desbordar y no la lleva.
    assert_eq!(reglas_de(f), 2);

    let g = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve logico\n    devuelve a < b\n";
    assert_eq!(reglas_de(g), 0, "una comparacion no puede salirse");
}

/// La tercera pata: **fuera del syscall no quiere decir sin acceso al sistema**.
///
/// ** Es la distincion que el maestro llama "control no es privilegio", y aqui
/// se ve en bytes: el mismo compilador emite un programa sin puerta (arriba) y
/// uno con puerta (este), y **la diferencia es una linea del fuente** -- `usa
/// bmo` -- no una bandera del compilador ni una palabra clave.
///
/// Quitar esa fila de `modulos.toml` apaga la puerta sin tocar una linea de
/// Rust. Eso es lo que significa que la puerta no sea sintaxis.
#[test]
fn la_puerta_llega_por_una_linea_del_fuente_y_no_por_otra_via() {
    let sin = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a + b\n";
    let con = "\
perfil llano
usa bmo

funcion f(a es entero64, b es entero64) devuelve entero64
    devuelve invoca(a, b, 0, 0, 0)
";
    let hay = |src: &str| emitido(src).codigo.windows(2).any(|w| w == [0x0F, 0x05]);
    assert!(!hay(sin), "sin `usa bmo` no puede haber puerta");
    assert!(hay(con), "con `usa bmo` tiene que haberla");
}
