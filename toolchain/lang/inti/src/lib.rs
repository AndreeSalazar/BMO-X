//! # INTI -- el lenguaje de BMO-X
//!
//! *"Es como lo hizo C pero para Unix, pero BMO-X tendra su INTI."*
//!
//! Frontend: de texto a arbol. El **porque** de cada decision esta en
//! `docs/maestro/INTI_MAESTRO.md`; **lo que se escribe**, en
//! `toolchain/lang/inti/GRAMATICA.md`; y **como se decidio partir el
//! compilador**, en `ARQUITECTURA.md`, que esta al lado de este fichero.
//!
//! ## Los modulos, y por que son estos
//!
//! El corte no es por fases del compilador --eso saldria igual en cualquier
//! libro--, sino por **lo que cada pieza tiene que poder decir sin nombrar a
//! las demas**:
//!
//! ```text
//!    aviso      el mensaje de cuatro partes y los codigos estables.
//!               No sabe que existe INTI. Se prueba solo.
//!
//!    palabras   el vocabulario, leido de `tables/lang/inti/palabras.toml`.
//!               Es lo que hace que el idioma sea una columna y no un fork.
//!
//!    lexico     de bytes a piezas. No conoce la gramatica.
//!      pieza      los datos, sin logica: los lee todo el mundo
//!      sangria    el margen, que es lo unico con estado del barrido
//!
//!    arbol      la forma de un programa. Cero decisiones.
//!    sintaxis   aplica la gramatica. No sabe si los nombres existen.
//!    perfil     `llano` contra `pleno`. No emite un byte.
//!    nombres    quien es cada nombre, y si se puede cambiar.
//!    ir         del arbol a instrucciones. NO nombra ninguna maquina.
//!    cabina     lo que INTI le cuenta al sistema, en la capa `Lang`.
//! ```
//!
//! OJO: **Lo que este crate NO enlaza todavia**: `bmo-abi`, `bmo-lower` y
//! `bmo-verify`, que es lo que enlazan los otros cuatro frontends. F1 no emite
//! bytes -- entra texto y sale un arbol. Atar el frontend a la forma del
//! emisor antes de tener nada que emitir es el orden que este proyecto evita.
//!
//! ## Lo que hay hoy
//!
//! ```text
//!    F1a  lexico completo                      <- esto
//!    F1b  arbol + sintaxis (de piezas a arbol)
//!    F2   INTI LLANO a `.bex` nativo
//! ```

pub mod arbol;
pub mod arquitectura;
pub mod cabina;
pub mod ir;
pub mod aviso;
pub mod lexico;
pub mod nombres;
pub mod palabras;
pub mod perfil;
pub mod sintaxis;
pub mod disposicion;
pub mod tipos;
pub mod tablas;

pub use aviso::{Aviso, Cosecha, Sitio};
pub use lexico::{Clase, Pieza, Signo};
pub use palabras::{Simbolo, Vocabulario};
pub use arbol::{Modulo, Perfil};

/// Barre un fuente con el vocabulario que traiga el sistema.
///
/// El vocabulario se busca en las raices de `bmo-mods` (`$BMO_MODS` -> `mods/`
/// -> `tables/`) y, si no aparece en ninguna, se usa el que viaja dentro. Un
/// compilador que no arranca porque falta un fichero de datos es peor que uno
/// que arranca con lo que traia -- pero **cual de las dos cosas paso se puede
/// preguntar**, con `palabras::Vocabulario::cargar`.
pub fn barrer(fuente: &str) -> Cosecha<Vec<Pieza>> {
    let raices = bmo_mods::Roots::find();
    let (vocab, _origen) = Vocabulario::cargar(&raices);
    match vocab {
        Ok(v) => lexico::barrer(fuente, &v),
        // Solo pasa si alguien rompio la tabla incrustada, y entonces el
        // compilador no tiene idioma con el que hablar.
        Err(e) => panic!("palabras.toml esta roto y ni el respaldo carga: {}", e),
    }
}

/// Lee un fuente entero: barrido + gramatica.
///
/// Los avisos de las dos fases salen **juntos y en orden**. Es la razon de que
/// `Cosecha` no sea un `Result`: si el barrido encuentra tres cosas y la
/// gramatica dos, el que escribe quiere ver las cinco.
pub fn leer(fuente: &str) -> Cosecha<Modulo> {
    let raices = bmo_mods::Roots::find();
    let (vocab, _) = Vocabulario::cargar(&raices);
    let v = match vocab {
        Ok(v) => v,
        Err(e) => panic!("palabras.toml esta roto y ni el respaldo carga: {}", e),
    };
    let piezas = lexico::barrer(fuente, &v);
    let mut arbol = sintaxis::leer(&piezas.valor, &v);
    let mut avisos = piezas.avisos;
    avisos.append(&mut arbol.avisos);
    Cosecha::con(arbol.valor, avisos)
}


/// Lee un fuente **y le trae las piezas de INTI que pidio con un `usa`**.
///
/// ## Que es esto exactamente, y que NO es
///
/// Es INCLUSION: las declaraciones de `runtime/monton/*.inti` se meten en el
/// mismo modulo que las del usuario, y salen en el mismo `.bex`.
///
/// OJO: **No es enlazado, y la diferencia se paga.** Diez programas que usen el
/// monton llevan diez copias del monton. Es exactamente lo que se le critica a
/// Go en la seccion 13c del maestro, asi que decirlo aqui es lo minimo.
///
/// La respuesta buena ya esta escrita en esa misma seccion y no es "enlazar
/// mejor": el runtime es codigo que no cambia, o sea **congelado**, y lo
/// congelado en BMO-X se PRESTA en vez de copiarse (`MEM_OP_OFRECER`). El dia
/// que exista compilacion separada, esta funcion pasa a resolver nombres en vez
/// de pegar fuentes, y el segundo programa que arranque no paga el monton otra
/// vez.
///
/// Se hace asi hoy porque la alternativa era tener el monton escrito, probado y
/// **sin forma de usarlo**, que en este proyecto cuenta como no tenerlo.
///
/// ## El precio que si se cobra ya
///
/// Una pieza traida se compila con SUS `usa`, y esos `usa` acaban en la lista
/// del modulo. O sea que `usa monton` deja a mano los nombres de `memoria`, que
/// el fichero no pidio. Es una fuga, esta marcada, y desaparece con lo mismo
/// que lo anterior.
pub fn armar(fuente: &str) -> Cosecha<Modulo> {
    let raices = bmo_mods::Roots::find();
    let (vocab, _) = Vocabulario::cargar(&raices);
    let v = match vocab {
        Ok(v) => v,
        Err(e) => panic!("palabras.toml esta roto y ni el respaldo carga: {}", e),
    };

    let piezas = lexico::barrer(fuente, &v);
    let mut arbol = sintaxis::leer(&piezas.valor, &v);
    let mut avisos = piezas.avisos;
    avisos.append(&mut arbol.avisos);

    // Lo que el fuente pidio, en el orden en que lo pidio.
    let pedidos: Vec<String> = arbol.valor.usa.iter().map(|(n, _)| n.clone()).collect();
    for nombre in pedidos {
        for (fichero, texto) in tablas::Runtime::traer(&raices, &nombre) {
            let p = lexico::barrer(&texto, &v);
            let mut a = sintaxis::leer(&p.valor, &v);
            // ** Una pieza del runtime que no compila NO se traga en silencio.
            //
            // Sus avisos salen con los del usuario, y el `fichero` va en el
            // texto porque el sitio que traen apunta a lineas de OTRO fuente:
            // sin eso, el que compila ve "linea 12" y mira su linea 12.
            for mut x in p.avisos.into_iter().chain(a.avisos.drain(..)) {
                x.que_paso = format!("en la pieza `{}`: {}", fichero, x.que_paso);
                avisos.push(x);
            }
            arbol.valor.declaraciones.append(&mut a.valor.declaraciones);
            arbol.valor.usa.extend(a.valor.usa);
        }
    }

    Cosecha::con(arbol.valor, avisos)
}

/// El fuente entero: barrido, gramatica y perfil.
///
/// Es lo mas lejos que llega INTI hoy. Los avisos de las tres fases salen
/// juntos y en orden, que es lo que `Cosecha` existe para permitir.
pub fn comprobar(fuente: &str) -> Cosecha<perfil::Informe> {
    let raices = bmo_mods::Roots::find();
    // El vocabulario lo carga `armar`: aqui ya no hace falta.
    let mut arbol = armar(fuente);

    // `usa x86_64` trae los nombres de una maquina. Un `usa` que no sea una
    // arquitectura conocida no es un error: sera `usa entrada`, que es REX.
    let maquinas: Vec<arquitectura::Maquina> = arbol
        .valor
        .usa
        .iter()
        .filter_map(|(n, _)| arquitectura::Maquina::buscar(&raices, n))
        .collect();

    let modulos = tablas::Modulos::cargar(&raices);
    let mut perfiles = perfil::comprobar(
        &arbol.valor,
        &perfil::Catalogo::cargar(&raices),
        &maquinas,
        &modulos,
    );

    // Los nombres que traen los `usa`: los de las maquinas declaradas y los de
    // los modulos de REX.
    let mut extra: Vec<String> = maquinas
        .iter()
        .flat_map(|m| m.nombres_que_trae())
        .collect();
    for (n, _) in &arbol.valor.usa {
        extra.extend(modulos.trae(n).iter().cloned());
    }
    // ** Y las CONSTANTES del ABI. `mi_tarea` y las operaciones de la puerta se
    // escriben como un nombre cualquiera, y el descenso las resuelve contra la
    // tabla -- pero quien busca nombres desconocidos no las conocia.
    extra.extend(modulos.constantes());
    // ** Y las conversiones, que se escriben como una llamada.
    //
    // `flotante64(n)` no es una funcion y aun asi tiene que EXISTIR para el
    // analisis de nombres, o lo denuncia como una falta de ortografia. Los
    // nombres salen de `medidas.toml`, que es la misma tabla que decide como se
    // baja: una sola lista, no dos que puedan discrepar.
    extra.extend(disposicion::Medidas::cargar(&raices).conversiones());
    let mut nombres =
        nombres::comprobar(&arbol.valor, &nombres::Comun::cargar(&raices), &extra);

    // La cuarta: cuanto mide cada cosa y donde esta cada campo.
    let mut plano = disposicion::comprobar(&arbol.valor, disposicion::Medidas::cargar(&raices));

    // ** Y la quinta: que lo que se opera junto se pueda operar junto.
    //
    // Va DESPUES del plano y no en paralelo, porque necesita sus respuestas --y
    // es el unico analisis que depende de otro--. No es una excepcion a que los
    // analisis no se miren entre ellos: `tipos` no mira a `disposicion`, mira al
    // PLANO, que es un dato ya calculado. La diferencia es la misma que entre
    // llamar a alguien y leer lo que dejo escrito.
    let mut tipos = tipos::comprobar(&arbol.valor, &plano.valor);

    let mut avisos = std::mem::take(&mut arbol.avisos);
    avisos.append(&mut perfiles.avisos);
    avisos.append(&mut nombres.avisos);
    avisos.append(&mut plano.avisos);
    avisos.append(&mut tipos.avisos);
    Cosecha::con(perfiles.valor, avisos)
}

/// El camino entero, y lo que CABINA necesita saber al final.
///
/// ** Una sola llamada y el sistema tiene la foto: que fallo, donde, a que
/// maquina se ata el programa y lo que paga por no tener comportamiento
/// indefinido.
///
/// Es la peticion de Eddi -- *"CABINA va a estar vigilando a INTI por completo,
/// porque es el PRINCIPAL para decir y senalar que fallo, para asi mejorar en
/// avances"* -- y lo importante es que **no se manda solo lo que fallo**. Los
/// numeros van tambien, porque un numero se puede seguir en el tiempo y una
/// queja no.
pub fn informar(fuente: &str, fichero: &str) -> (cabina::Parte, Vec<cabina_core::Event>) {
    let raices = bmo_mods::Roots::find();
    // El vocabulario lo carga `armar`: aqui ya no hace falta.
    let mut arbol = armar(fuente);

    let maquinas: Vec<arquitectura::Maquina> = arbol
        .valor
        .usa
        .iter()
        .filter_map(|(n, _)| arquitectura::Maquina::buscar(&raices, n))
        .collect();

    let modulos = tablas::Modulos::cargar(&raices);
    let mut perfiles = perfil::comprobar(
        &arbol.valor,
        &perfil::Catalogo::cargar(&raices),
        &maquinas,
        &modulos,
    );
    let mut extra: Vec<String> = maquinas.iter().flat_map(|m| m.nombres_que_trae()).collect();
    for (n, _) in &arbol.valor.usa {
        extra.extend(modulos.trae(n).iter().cloned());
    }
    // ** Y las CONSTANTES del ABI. `mi_tarea` y las operaciones de la puerta se
    // escriben como un nombre cualquiera, y el descenso las resuelve contra la
    // tabla -- pero quien busca nombres desconocidos no las conocia.
    extra.extend(modulos.constantes());
    // ** Y las conversiones, que se escriben como una llamada.
    //
    // `flotante64(n)` no es una funcion y aun asi tiene que EXISTIR para el
    // analisis de nombres, o lo denuncia como una falta de ortografia. Los
    // nombres salen de `medidas.toml`, que es la misma tabla que decide como se
    // baja: una sola lista, no dos que puedan discrepar.
    extra.extend(disposicion::Medidas::cargar(&raices).conversiones());
    let mut nombres_ = nombres::comprobar(&arbol.valor, &nombres::Comun::cargar(&raices), &extra);

    let plano = disposicion::comprobar(&arbol.valor, disposicion::Medidas::cargar(&raices));
    let metal = ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let ir = ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal).valor;

    let parte = cabina::Parte {
        fichero: fichero.to_string(),
        perfil: arbol.valor.perfil.nombre().to_string(),
        arquitecturas: perfiles.valor.arquitecturas.clone(),
        bloques_crudo: perfiles.valor.bloques_crudo,
        comprobaciones: ir.comprobaciones(),
        funciones: ir.funciones.len(),
        instrucciones: ir.instrucciones(),
        // ** Vacio, y NO por olvido: esta funcion no emite un byte. Lo que no
        // llego a emitirse solo lo sabe quien emitio, y este camino se queda en
        // la IR. Poner un cero aqui diria "no falto nada", que es una respuesta
        // distinta de "no lo se".
        sin_emitir: Vec::new(),
    };

    let mut avisos = std::mem::take(&mut arbol.avisos);
    avisos.append(&mut perfiles.avisos);
    avisos.append(&mut nombres_.avisos);

    let eventos = cabina::eventos(&parte, &avisos);
    (parte, eventos)
}
