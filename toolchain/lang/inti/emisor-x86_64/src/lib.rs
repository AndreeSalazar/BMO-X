//! # INTI para x86-64 -- de la IR a bytes
//!
//! El unico crate de INTI que puede nombrar una maquina, y por eso vive fuera
//! del frontend: `bmo-inti-front/tests/agnostico.rs` prohibe alli lo que aqui
//! se hace en cada linea.
//!
//! ## La frontera, dicha entera
//!
//! ```text
//!    bmo-inti-front        texto -> piezas -> arbol -> analisis -> IR
//!                          NO nombra ninguna maquina. Hay un test.
//!
//!    bmo-inti-x86-64       IR -> bytes -> .bex
//!    (esto)                nombra x86-64 en cada linea, porque ES x86-64.
//! ```
//!
//! El dia de otra arquitectura, `emisor-aarch64/` al lado y **el frontend no se
//! toca**. Esa es la mitad B de la portabilidad de la seccion 7 del maestro,
//! convertida en dos carpetas.
//!
//! ## ** Y lo que se emite que ningun otro lenguaje emite
//!
//! Las comprobaciones. Una suma de INTI baja a **dos** cosas:
//!
//! ```text
//!    add   rax, rcx        la suma
//!    jo    <atrapa>        y la regla 1, que en C no existe
//! ```
//!
//! Ese `jo` es el "sin comportamiento indefinido" en bytes. La seccion 6.3 dice
//! que cuesta ~1%; aqui esta la instruccion que se va a medir.
//!
//! ## ** F3: los temporales viven en registros
//!
//! Desde el 2026-08-19. Y el cambio ocurrio **en `marco.rs` y en tres lineas de
//! este fichero**, que es lo que `LINAJE.md` habia prometido. Se pudo porque la
//! IR ya traia los temporales.
//!
//! ## ** Las llamadas, desde el 2026-08-19
//!
//! Una funcion de INTI llama a otra de INTI. Es la pieza que desbloquea todo lo
//! demas, porque **todo runtime son llamadas**.
//!
//! Y los destinos se resuelven **al final del modulo**: una funcion puede
//! llamar a otra declarada mas abajo, y resolver sobre la marcha obligaria a
//! ordenar las funciones por quien llama a quien -- imposible en cuanto dos se
//! llaman entre si.
//!
//! ## OJO: Lo que hoy NO hace, dicho por delante
//!
//! - **No llama fuera del modulo**: una funcion de biblioteca pide enlazado, y
//!   el hueco se deja marcado en vez de inventarle una direccion.
//! - **No emite `pleno`**: texto, listas y tablas piden monton.
//!
//! O sea: **INTI LLANO con aritmetica entera y control**. Que es justo lo que
//! hace falta para que el primer `.bex` exista y pase el gate.

pub mod arranque;
pub mod barrido;
pub mod funcion;
use funcion::emitir_funcion;
mod marco;
mod metal;
mod operaciones;
mod reglas;
pub mod puerta;

use bmo_abi::bef::katanas::{self, Katana};
use bmo_abi::dynobj::texto as dynobj_texto;
use bmo_abi::bef::relocations::{Relocation, RelocationKind};
use bmo_abi::bef::sections::SectionKind;
use bmo_abi::bef::writer::{BefBuilder, BefSection};
use bmo_abi::syscalls::surface::NR_INVOKE;
use bmo_inti_front::ir::{
    Clase, ClaseCongelada, Comprobacion, Const, FuncionIr, Instr, Local, ModuloIr, Valor,
};
use bmo_lower::x86;
use marco::{Marco, Sitio};
use metal::metal;
use operaciones::{binaria, flotante};
use reglas::regla_doce;
use puerta::Puerta;

/// Los dos registros de trabajo.
///
/// Dos bastan mientras todo viva en la pila: uno para cada lado de una
/// operacion binaria. Cuando llegue el asignador de registros esto desaparece,
/// y ese es justo el cambio que la IR con temporales hace posible.
pub(crate) const IZQ: u8 = 0; // rax
pub(crate) const DER: u8 = 1; // rcx

/// Por donde llegan y se mandan los argumentos, en orden.
///
/// Es la convencion de llamada de esta maquina, y por eso esta linea solo puede
/// existir en este crate: el frontend tiene prohibido saber que existe algo
/// llamado "registro de argumento".
const ARGUMENTOS: [u8; 6] = [7, 6, 2, 1, 8, 9]; // rdi, rsi, rdx, rcx, r8, r9

/// Lo que sale de emitir un modulo.
pub struct Emitido {
    pub codigo: Vec<u8>,
    /// Donde empieza cada funcion dentro del codigo.
    pub inicios: Vec<(String, usize)>,
    /// Cuantas comprobaciones anti-UB se emitieron **en bytes**.
    ///
    /// ** No es el mismo numero que `ModuloIr::comprobaciones()`: aquel cuenta
    /// las que la IR pidio y este las que llegaron al binario. El dia que haya
    /// eliminacion de comprobaciones, **la diferencia entre los dos numeros es
    /// exactamente lo que el optimizador quito**, y se podra leer sin creerselo.
    pub comprobaciones: usize,
    /// Cuantos temporales viven en un registro, y cuantos en la pila.
    ///
    /// ** Es el numero de F3: si el segundo baja y el primero sube, el
    /// asignador esta haciendo su trabajo. Y como sale del emisor y no de una
    /// estimacion, se puede seguir en el tiempo -- igual que los `crudo`.
    pub en_registros: usize,
    pub en_pila: usize,
    /// **Las tablas congeladas del modulo**, tal y como van a `RoData`.
    pub congelados: Vec<bmo_inti_front::ir::Congelado>,
    /// Los huecos del codigo que hay que rellenar con la direccion del **slot
    /// del monton de la tarea**, que vive en la seccion `Data`.
    ///
    /// ** Lista aparte de `reubicaciones` --las de `RoData`-- porque apuntan a
    /// otra seccion, y mezclarlas obligaria a llevar el destino en cada entrada
    /// para distinguirlas. Dos listas cortas dicen mas que una larga con una
    /// etiqueta.
    pub reubicaciones_del_monton: Vec<usize>,
    /// Donde hay que escribir la direccion de cada tabla: `(offset en el codigo,
    /// indice del congelado)`. Se convierten en reubicaciones del `.bex`.
    pub reubicaciones: Vec<(usize, u32)>,
    /// **LAS KATANAS**: `(codigo, offset, longitud)` de cada bloque de trampa,
    /// con el offset dentro de `codigo`.
    ///
    /// *** Es lo que convierte *"este binario atrapa"* de afirmacion en algo que
    /// se puede ir a mirar. Va a la seccion `Katanas 0x16` del `.bex`.
    pub katanas: Vec<(u64, usize, usize)>,
    /// Llamadas cuyo destino todavia no se sabia al emitirlas.
    ///
    /// ** Se resuelven al final del modulo y no sobre la marcha, porque una
    /// funcion puede llamar a otra **declarada mas abajo**. Resolver segun se
    /// emite obligaria a ordenar las funciones por quien llama a quien -- y eso
    /// es imposible en cuanto dos se llaman entre si.
    huecos_de_llamada: Vec<(usize, String)>,
    /// Lo que se pidio emitir y NO se pudo, con su motivo.
    ///
    /// ## ** Por que esto es un campo publico y no un `panic!`
    ///
    /// Porque un intrinseco que no sale **no rompe la compilacion**: el resto
    /// del programa esta bien y el `.bex` se escribe. Sin una lista, la unica
    /// senal seria que el binario hace otra cosa en metal -- y en una tabla de
    /// driver eso se descubre seis meses tarde.
    ///
    /// Va a CABINA con su numero, y hay un test que lo exige VACIO para la
    /// tabla entera de la maquina. Esa es la matriz de conformidad que el
    /// comentario de `Intrinsics::names()` llevaba pidiendo desde que se
    /// escribio.
    pub sin_emitir: Vec<String>,
    /// Si este modulo trae su propio arranque.
    ///
    /// ** Es la diferencia entre un programa y una biblioteca, y la decide un
    /// dato del fuente --que exista `principal`-- y no una bandera de la linea
    /// de ordenes. Un `.bex` que arranca por accidente y otro que no arranca
    /// porque faltaba un flag son el mismo fallo con dos caras.
    pub arranca: bool,
}

/// Lo que el emisor lee ANTES de escribir un byte.
///
/// ** Aqui esta el permiso que este crate tiene y el frontend no: puede decir
/// "x86_64" en voz alta. Es lo que justifica que sea un crate aparte -- y por
/// eso carga SU tabla, y no una generica que le pasen desde fuera.
///
/// Se lee una vez por modulo. Por funcion seria releer TOML en cada funcion del
/// programa; una vez por proceso obligaria a un estatico, que es la clase de
/// cosa que hace que dos compilaciones dentro del mismo proceso no den lo mismo.
pub struct Taller {
    /// Como se cruza la puerta aqui.
    pub puerta: Puerta,
    /// Los nombres que abren esa puerta.
    ///
    /// ** Salen de `modulos.toml`, que es AGNOSTICO: `invoca` se llama igual en
    /// toda maquina. Lo unico que este crate aporta es donde van sus
    /// argumentos. Si la lista viviera aqui escrita a mano, cada emisor nuevo
    /// tendria que acordarse de copiarla -- y el dia que se anadiera una
    /// operacion a la puerta, se acordaria uno solo.
    pub nombres_de_puerta: Vec<String>,
    /// Que recoge cada nombre de la puerta. Agnostico: sale de `modulos.toml`.
    pub recoge: bmo_inti_front::tablas::Modulos,
    /// Los registros que el asignador puede repartir, leidos de `[reparto]`.
    pub temporales: Vec<u8>,
    /// La tabla de ESTA maquina: como se llama en INTI cada instruccion.
    pub maquina: Option<bmo_inti_front::arquitectura::Maquina>,
    /// Y los bytes que hay detras de cada nombre de instruccion.
    ///
    /// ** Es LA MISMA tabla que lee BMO C, y ese es el punto entero: los bytes
    /// se declaran una vez. Dos declaraciones de los mismos bytes acaban
    /// discrepando, y la que discrepe sera la que nadie prueba.
    pub intrinsecos: Option<bmo_sem_asm::Intrinsics>,
}

impl Taller {
    pub fn nuevo() -> Self {
        let raices = bmo_mods::Roots::find();
        let maquina = bmo_inti_front::arquitectura::Maquina::buscar(&raices, "x86_64");
        let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
        let temporales = maquina
            .as_ref()
            .map(|m| m.temporales())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| marco::RESPALDO.to_vec());
        Self {
            puerta: Puerta::de(maquina.as_ref()),
            nombres_de_puerta: modulos.trae("bmo").to_vec(),
            recoge: modulos,
            temporales,
            maquina,
            intrinsecos: bmo_sem_asm::Intrinsics::load_x86_64().ok(),
        }
    }

    fn abre_la_puerta(&self, nombre: &str) -> bool {
        self.nombres_de_puerta.iter().any(|n| n == nombre)
    }
}

/// Emite un modulo entero con las tablas de esta maquina.
pub fn emitir(m: &ModuloIr) -> Emitido {
    emitir_con(m, &Taller::nuevo())
}

/// Emite un modulo entero.
pub fn emitir_con(m: &ModuloIr, taller: &Taller) -> Emitido {
    let mut salida = Emitido {
        codigo: Vec::new(),
        inicios: Vec::new(),
        katanas: Vec::new(),
        congelados: Vec::new(),
        reubicaciones_del_monton: Vec::new(),
        reubicaciones: Vec::new(),
        comprobaciones: 0,
        en_registros: 0,
        en_pila: 0,
        huecos_de_llamada: Vec::new(),
        sin_emitir: Vec::new(),
        arranca: false,
    };

    // ** El arranque va PRIMERO, y solo si hay a quien llamar.
    //
    // Primero porque el kernel entra por el principio del codigo y el `.bex`
    // declara `entry_offset = 0`. Y "solo si" porque un modulo sin `principal`
    // es una biblioteca -- y una biblioteca que arranca sola no es una
    // comodidad, es un fallo.
    // ** Las tablas congeladas viajan tal cual: la IR ya las tiene en bytes y
    // este crate solo decide DONDE van. Convertirlas aqui seria decidir dos
    // veces lo mismo.
    salida.congelados = m.congelados.clone();

    // *** Y LOS LITERALES DE LISTA QUE NO SE PUDIERON CONSTRUIR, SE DICEN.
    //
    // Un `[1, 2, 3]` sin tipo escrito baja a `nada`, porque **el ancho del
    // elemento sale del TIPO** y una expresion suelta no sabe adonde va. Es una
    // carencia real y con arreglo conocido --escribir el tipo-- asi que se
    // cuenta en vez de callar.
    //
    // ** Es la leccion de `Const::Texto`, que bajo a un cero durante meses: lo
    // unico que impidio que se olvidara fue que el emisor lo confesaba con un
    // numero.
    let sin_ancho: usize = m.funciones.iter().map(|f| f.sin_ancho).sum();
    if sin_ancho > 0 {
        salida.sin_emitir.push(format!(
            "{} literal(es) de lista sin construir: no se sabe cuanto mide su elemento.              Escribe el tipo del destino, como `notas es lista de entero64 = [1, 2, 3]`",
            sin_ancho
        ));
    }

    // ** SE MIRA LA IR, NO EL PERFIL. Montar un monton cuesta dos cruces de la
    // puerta, y un programa que no toca objetos no tiene por que pagarlos.
    // Preguntarselo al perfil seria adivinar; preguntarselo a la IR es leer.
    let necesita_monton = m
        .funciones
        .iter()
        .flat_map(|f| f.instrucciones.iter())
        .any(|i| matches!(i, Instr::MontonDeLaTarea { .. }));

    if m.funciones.iter().any(|f| f.nombre == arranque::PRINCIPAL) {
        let p = arranque::emitir(&mut salida.codigo, &taller.puerta, IZQ, necesita_monton);
        salida
            .huecos_de_llamada
            .push((p.principal, arranque::PRINCIPAL.to_string()));
        if let Some(h) = p.monton_nuevo {
            salida
                .huecos_de_llamada
                .push((h, arranque::MONTON_NUEVO.to_string()));
        }
        if let Some(h) = p.slot_del_monton {
            salida.reubicaciones_del_monton.push(h);
        }
        salida.arranca = true;
    }

    for f in &m.funciones {
        salida.inicios.push((f.nombre.clone(), salida.codigo.len()));
        let cuenta = emitir_funcion(f, &mut salida.codigo, taller);
        salida.comprobaciones += cuenta.comprobaciones;
        salida.en_registros += cuenta.en_registros;
        salida.en_pila += cuenta.en_pila;
        salida.katanas.extend(cuenta.katanas);
        // ** Los huecos YA vienen en coordenadas del MODULO: `emitir_funcion`
        // escribe sobre `salida.codigo`, no sobre un buffer propio, asi que
        // `out.len()` de ahi dentro ya es absoluto.
        //
        // *** La primera version les sumaba el inicio de la funcion y quedaban
        // apuntando mas alla del hueco. No lo canto ninguna prueba: lo canto
        // mirar los bytes del `.ibex` y ver que ahi no habia ocho ceros. Es la
        // misma comprobacion que hacen las katanas, y por eso ellas nunca lo
        // tuvieron: usan `out.len()` tal cual desde el primer dia.
        salida.reubicaciones.extend(cuenta.reubicaciones.iter().copied());
        salida.huecos_de_llamada.extend(cuenta.huecos_de_llamada);
        salida
            .reubicaciones_del_monton
            .extend(cuenta.reubicaciones_del_monton);
        salida.sin_emitir.extend(cuenta.sin_emitir);
        // ** Y los nombres que se van a bajar a un cero, con el suyo delante.
        for n in nombres_sueltos(f) {
            salida.sin_emitir.push(format!(
                "`{}`: el emisor no sabe que es y lo baja a un CERO",
                n
            ));
        }
    }

    // *** EL POZO DE TEXTOS SE CALCULA Y NO LO LEE NADIE (2026-08-22).
    //
    // *** RESUELTO EL 2026-08-23. Lo que decia aqui, y ya no es verdad:
    //
    //     `ir::bajar` interna cada literal de texto en `ModuloIr::textos` -- los
    //     deduplica, les da un indice, y deja escrito que "se comparte, y por
    //     eso puede prestarse congelado". Y este emisor **no lo mira ni una
    //     vez**: `Const::Texto(i)` baja a un cero.
    //
    // Era la firma de fallo de siempre --la pieza que se calcula bien y no la
    // lee nadie-- y el aviso se quedo escrito porque no habia adonde llevarlos.
    // Ahora si: **un literal de texto ES un congelado**, va a `RoData` con su
    // cabecera de objeto y el codigo llega a el por una reubicacion, igual que
    // una tabla constante.
    //
    // ** Y el pozo no era un segundo mecanismo: existia porque `RoData` no
    // existia. La seccion 10.2 del maestro ya los tenia juntos.

    // Ahora si: todas las funciones tienen sitio, asi que todas las llamadas
    // tienen destino.
    let huecos = std::mem::take(&mut salida.huecos_de_llamada);
    for (hueco, nombre) in huecos {
        let destino = salida
            .inicios
            .iter()
            .find(|(n, _)| *n == nombre)
            .map(|(_, off)| *off);
        // ** UNA LLAMADA SIN DESTINO SE APUNTA, que es lo que este comentario
        // decia y no hacia.
        //
        // Decia *"se deja marcada en vez de inventarle una direccion"*, y la
        // dejaba en CERO sin marcar nada. Un `call` con desplazamiento cero
        // salta a la instruccion siguiente: no revienta, no se queja, y devuelve
        // lo que hubiera en el registro de retorno.
        //
        // Y eso es exactamente lo que hoy le pasa a `usa archivo`, `usa
        // superficie` y los demas modulos de REX: **traen los nombres, el
        // analisis los aprueba, y la llamada no va a ninguna parte**. Un
        // programa que guarda un fichero compilaba, corria, y no guardaba nada.
        //
        // Hace falta enlazado para arreglarlo de verdad. Hasta entonces, lo
        // unico honesto es que se sepa.
        match destino {
            Some(d) => {
                let rel = (d as i64 - (hueco as i64 + 4)) as i32;
                salida.codigo[hueco..hueco + 4].copy_from_slice(&rel.to_le_bytes());
            }
            None => salida.sin_emitir.push(format!(
                "{}: la llamada no tiene destino -- no esta en este modulo y no hay enlazado",
                nombre
            )),
        }
    }

    salida
}

/// Lo que una funcion aprende mientras se emite.
///
/// Se DEVUELVE en vez de escribirse sobre la marcha porque el codigo esta
/// prestado mientras se emite. No es una pelea con el prestamo: es la senal de
/// que emitir y contabilizar son dos cosas, y mezclarlas fue lo primero que
/// probe.
#[derive(Default)]
struct Cuenta {
    comprobaciones: usize,
    en_registros: usize,
    en_pila: usize,
    huecos_de_llamada: Vec<(usize, String)>,
    /// Lo que se pidio emitir y NO se pudo, con el motivo.
    ///
    /// ** Existe porque la alternativa es lo que habia: no emitir nada y callar.
    /// Un intrinseco que no sale no rompe la compilacion --el resto del programa
    /// esta bien-- asi que sin una lista no se entera nadie hasta que el binario
    /// hace otra cosa en metal.
    sin_emitir: Vec<String>,
    /// **Donde queda el hueco de la direccion de cada tabla congelada**:
    /// `(offset del inmediato, indice del congelado)`.
    reubicaciones: Vec<(usize, u32)>,
    /// Los huecos que apuntan al slot del monton, en la seccion `Data`.
    reubicaciones_del_monton: Vec<usize>,
    /// **Donde acabo el bloque de trampa de cada regla**: `(codigo, offset,
    /// longitud)`, con el offset DENTRO de la seccion de codigo.
    ///
    /// ** Este dato solo lo tiene quien emite, y hasta hoy se tiraba. Sin el, la
    /// afirmacion *"este binario atrapa"* no se puede contrastar con nada: el
    /// bloque esta en los bytes y **nadie sabe donde**.
    katanas: Vec<(u64, usize, usize)>,
}

/// **EL BARRIDO, COMO GATE: ninguna operacion sin su regla.**
///
/// ## Lo que anade sobre `exige_katanas`
///
/// Aquella comprueba que las reglas DECLARADAS estan donde dice. Esta pregunta
/// al reves y cierra la otra mitad: **por cada operacion que pide regla, esta la
/// suya?** Una mesa vacia pasa la primera y no pasa esta.
///
/// ## *** Y cuando NO puede afirmar nada, NO rechaza
///
/// Si el barrido se atasca --un intrinseco de un byte dentro de un `crudo`, una
/// instruccion que este lector todavia no conoce-- la respuesta correcta es
/// **callar**, no negar. Son tres cosas distintas:
///
/// ```text
///    completo + sin descubiertas   -> pasa
///    completo + alguna descubierta -> NO PASA, y se dice cual y donde
///    atascado                      -> pasa, porque no se ha demostrado nada
/// ```
///
/// ** Un verificador que rechaza lo que no entiende es un verificador que se
/// apaga en una semana, y entonces no verifica nada. Es la misma regla que
/// impidio comprobar el techo de `crudo` con un barrido de bytes: **absolver se
/// puede sin certeza; condenar no**.
fn auditar(e: &Emitido) -> bmo_verify::Verdict {
    let taller = Taller::nuevo();
    let maquina: Vec<Vec<u8>> = match &taller.intrinsecos {
        Some(t) => t
            .names()
            .iter()
            .filter_map(|n| t.get(n).map(|d| d.bytes.clone()))
            .collect(),
        None => Vec::new(),
    };
    let b = barrido::recorrer_con(&e.codigo, &maquina);
    if !b.completo() {
        return bmo_verify::Verdict::Ok;
    }
    let trampas: Vec<(u64, usize)> = e.katanas.iter().map(|(c, o, _)| (*c, *o)).collect();
    let malas = barrido::descubiertas(&b, &e.inicios, &trampas);
    if malas.is_empty() {
        return bmo_verify::Verdict::Ok;
    }
    // ** El motivo lleva el byte, no un recuento. Quien lo lea tiene que poder
    // ir al sitio: un "faltan 3 reglas" manda a buscar.
    bmo_verify::Verdict::Rejected(
        malas
            .iter()
            .map(|d| {
                format!(
                    "la operacion del byte {} pide la regla {} y en su funcion no hay                      ningun salto que vaya a un bloque de esa regla",
                    d.off, d.regla
                )
            })
            .collect(),
    )
}

/// **LOS NOMBRES QUE LLEGAN SUELTOS Y SE BAJAN A UN CERO.**
///
/// ## Por que existe, y lo que costaba no tenerla
///
/// `carga()` acaba en `Valor::Nombre(_) => zero_r32`. Es lo unico que puede
/// hacer --no sabe que es ese nombre-- y **lo hacia callandose**.
///
/// *** Eso convirtio `maximo = 100` en cero durante toda la vida del lenguaje:
/// la IR tiraba `Decl::Constante` con un `{}`, el nombre llegaba aqui suelto, y
/// salia un binario que compilaba limpio, pasaba el gate, salia FIRMADO y valia
/// cero -- con el ejemplo escrito en `GRAMATICA.md`.
///
/// ** Arreglar las constantes cierra ESE caso. Esta funcion cierra la CLASE: un
/// nombre que el emisor no sabe resolver no se convierte en un numero en
/// silencio, se dice. Es la misma decision que `sin_emitir` para los
/// intrinsecos, aplicada a los valores.
///
/// ## Lo que NO cuenta
///
/// El destino de una llamada. `Instr::Llama { que: Valor::Nombre(f) }` es lo
/// normal: asi se llama a una funcion, y se resuelve al final del modulo con los
/// huecos. Contarlo aqui llenaria el informe de ruido y entrenaria a no mirarlo.
fn nombres_sueltos(f: &FuncionIr) -> Vec<String> {
    let mut sueltos = Vec::new();
    let mut mira = |v: &Valor, sueltos: &mut Vec<String>| {
        if let Valor::Nombre(n) = v {
            sueltos.push(n.clone());
        }
    };
    for i in &f.instrucciones {
        // ** SIN COMODIN, y por lo que le paso a `marco.rs` el 22-08: un `_ =>`
        // dejo `Lee` y `Escribe` fuera del recuento de vivos y costo dos
        // temporales en el mismo registro. Aqui la lista tiene que crecer con la
        // IR, y sin comodin no se puede olvidar.
        match i {
            Instr::Mueve { origen, .. } => mira(origen, &mut sueltos),
            Instr::Binaria {
                izquierda, derecha, ..
            } => {
                mira(izquierda, &mut sueltos);
                mira(derecha, &mut sueltos);
            }
            Instr::Unaria { valor, .. } => mira(valor, &mut sueltos),
            Instr::Comprueba { sobre, contra, .. } => {
                mira(sobre, &mut sueltos);
                if let Some(c) = contra {
                    mira(c, &mut sueltos);
                }
            }
            Instr::Convierte { valor, .. } => mira(valor, &mut sueltos),
            // `que` es el DESTINO de la llamada: ahi un nombre es lo normal.
            Instr::Llama { argumentos, .. } => {
                for a in argumentos {
                    mira(a, &mut sueltos);
                }
            }
            // No lleva ningun `Valor`: su dato es un indice de tabla.
            Instr::Direccion { .. }
            | Instr::MontonDeLaTarea { .. }
            | Instr::DireccionDeLocal { .. } => {}
            Instr::Lee { direccion, .. } => mira(direccion, &mut sueltos),
            Instr::Escribe {
                direccion, valor, ..
            } => {
                mira(direccion, &mut sueltos);
                mira(valor, &mut sueltos);
            }
            Instr::Metal { argumentos, .. } => {
                for a in argumentos {
                    mira(a, &mut sueltos);
                }
            }
            Instr::Guarda { valor, .. } => mira(valor, &mut sueltos),
            Instr::SaltaSi { cond, .. } => mira(cond, &mut sueltos),
            Instr::Devuelve(Some(v)) => mira(v, &mut sueltos),
            Instr::Etiqueta(_) | Instr::Salta(_) | Instr::Devuelve(None) => {}
        }
    }
    sueltos.sort();
    sueltos.dedup();
    sueltos
}

fn epilogo(out: &mut Vec<u8>) {
    x86::mov_r64_r64(out, 4, 5); // mov rsp, rbp
    out.push(0x5D); // pop rbp
    out.push(0xC3); // ret
}

pub(crate) fn carga(out: &mut Vec<u8>, reg: u8, v: &Valor, marco: &Marco) {
    match v {
        Valor::Const(Const::Entero(n)) => x86::mov_r64_imm64(out, reg, *n as u64),
        // ** Un flotante se carga como lo que es: OCHO BYTES. La conversion de
        // "3.5" a esos bytes la hizo la IR una vez; aqui ya no hay decimal que
        // valga, hay un inmediato.
        //
        // Y de ahi sale gratis media Regla 11: el mismo fuente da el mismo
        // patron de bits en cualquier emisor, porque el que convierte es el
        // frontend y no la maquina.
        Valor::Const(Const::Flotante(bits)) => x86::mov_r64_imm64(out, reg, *bits),
        Valor::Const(Const::Logico(b)) => x86::mov_r64_imm64(out, reg, u64::from(*b)),
        Valor::Const(Const::Nada) => x86::zero_r32(out, reg),
        // Un decimal exacto no cabe en un inmediato: lo construye el runtime.
        Valor::Const(Const::Decimal(_)) | Valor::Const(Const::Texto(_)) => {
            x86::zero_r32(out, reg)
        }
        Valor::Local(l) => mov_de_marco(out, reg, marco.local(*l)),
        // ** F3: si el temporal vive en un registro, esto es un `mov` entre
        // registros en vez de una lectura de memoria. Ese es el 2-4x, y cabe en
        // estas tres lineas porque la IR ya traia los temporales.
        Valor::Temporal(t) => match marco.sitio(*t) {
            Sitio::Registro(r) => {
                if r != reg {
                    x86::mov_r64_r64(out, reg, r);
                }
            }
            Sitio::Pila(disp) => mov_de_marco(out, reg, disp),
        },
        // Una funcion o algo de un `usa`: lo resuelve el enlazado, que todavia
        // no existe.
        Valor::Nombre(_) => x86::zero_r32(out, reg),
    }
}

pub(crate) fn guarda_temporal(
    out: &mut Vec<u8>,
    reg: u8,
    t: bmo_inti_front::ir::Temporal,
    marco: &Marco,
) {
    match marco.sitio(t) {
        Sitio::Registro(r) => {
            if r != reg {
                x86::mov_r64_r64(out, r, reg);
            }
        }
        Sitio::Pila(disp) => mov_a_marco(out, disp, reg),
    }
}

/// `mov reg, [rbp+disp]`
fn mov_de_marco(out: &mut Vec<u8>, reg: u8, disp: i32) {
    out.push(0x48);
    out.push(0x8B);
    out.push(0x85 | (reg << 3));
    out.extend_from_slice(&disp.to_le_bytes());
}

/// `mov [rbp+disp], reg`
fn mov_a_marco(out: &mut Vec<u8>, disp: i32, reg: u8) {
    out.push(0x48);
    out.push(0x89);
    out.push(0x85 | (reg << 3));
    out.extend_from_slice(&disp.to_le_bytes());
}

/// **Los bytes de `RoData` y donde cae cada congelado dentro.**
///
/// ## Por que es una funcion y no diez lineas dentro de `empaquetar`
///
/// Porque tiene DOS clientes y **los dos tienen que ver exactamente lo mismo**:
/// el `.ibex` que se escribe al disco, y el banco de pruebas que ejecuta en el
/// emulador.
///
/// *** Hasta el 2026-08-23 el banco NO tenia esto, y la consecuencia era peor de
/// lo que parece: `ejecuta_en` corre el codigo crudo, sin secciones y sin
/// reubicaciones, asi que **una tabla congelada se leia de la direccion cero**.
/// Ninguna prueba unitaria habia visto nunca una tabla de verdad -- las que
/// existian miraban los BYTES del `.ibex`, que es otra pregunta.
///
/// Lo destapo el decimal: `POTENCIAS` daba numeros al azar y el codigo estaba
/// bien. Duplicar esta disposicion en el banco habria dado dos layouts que se
/// separan el dia que uno cambie; por eso se comparte.
pub fn rodata_de(e: &Emitido) -> (Vec<u8>, Vec<u64>) {
    let mut rodata: Vec<u8> = Vec::new();
    let mut donde: Vec<u64> = Vec::with_capacity(e.congelados.len());
    for c in &e.congelados {
        // Alineadas a ocho: una tabla de `entero64` leida a medias es lenta en
        // el mejor caso y una excepcion en el peor.
        while rodata.len() % 8 != 0 {
            rodata.push(0);
        }
        donde.push(rodata.len() as u64);
        // *** UN TEXTO LLEVA CABECERA DE OBJETO; UNA TABLA, NO.
        //
        // Y la pone AQUI y no el frontend a proposito: la forma de un objeto del
        // monton la declara `bmo_abi::dynobj`, y el frontend no enlaza `bmo-abi`
        // -- es la linea que le deja no saber de bytes.
        //
        // ** `congelado` y no `nacer`: el bit 63 puesto, INMORTAL. Un literal no
        // se cuenta, no se libera, y ademas vive en una seccion de solo lectura.
        if matches!(c.clase, ClaseCongelada::Texto) {
            let n = c.bytes.len() as u64;
            let mut cab = vec![0u8; dynobj_texto::CABECERA_LEN];
            // `type_index` = 0 mientras no exista el mapa de tipos.
            // Cero significa "el `TypeMap` no existe", no "el tipo cero".
            dynobj_texto::congelado(&mut cab, 0, n)
                .expect("la cabecera de un texto siempre cabe en su propio tamano");
            rodata.extend_from_slice(&cab);
        }
        rodata.extend_from_slice(&c.bytes);
    }
    (rodata, donde)
}

/// Envuelve el codigo en un `.bex` y **lo pasa por el gate**.
///
/// Ningun `.bex` del sistema se escribe sin pasar por `bmo-verify`: es el unico
/// checkpoint comun, y aqui no se abre un quinto camino que lo esquive.
pub fn empaquetar(e: &Emitido, manifiesto: Option<&str>) -> Result<Vec<u8>, String> {
    let mut b = BefBuilder::new();
    // Por donde entra el kernel. El arranque es lo primero que se emitio, asi
    // que es cero -- pero se dice, porque un cero que coincide con el valor por
    // defecto no distingue "decidido" de "olvidado".
    b.entry_offset = 0;
    b.add_section(BefSection::code(e.codigo.clone()));

    // *** LAS TABLAS CONGELADAS, EN `RoData` -- y NO dentro del codigo.
    //
    // Meterlas en `Code` habria sido mas corto: no harian falta reubicaciones y
    // la direccion se sabria al emitir. **Y habria roto el barrido lineal**, que
    // es lo que hace que un `.ibex` se pueda recorrer de principio a fin -- la
    // exclusividad tecnica de INTI, escrita en `barrido.rs`.
    //
    // Un binario de C mete datos entre las instrucciones y por eso no se puede
    // recorrer. Que INTI no lo haga es la restriccion que le paga esa propiedad,
    // y aqui es donde se respeta o se pierde.
    // *** LA SECCION `Data`: OCHO BYTES, y son el monton de la tarea.
    //
    // Es la primera vez que INTI emite una seccion ESCRIBIBLE. `RoData` la
    // rellena el compilador y no cambia; esta nace a cero y **la escribe el
    // arranque** con lo que le devuelva `monton_nuevo`.
    //
    // ** Ocho bytes y no una pagina: lo que vive aqui es UNA DIRECCION, no el
    // monton. El monton lo da el kernel y vive donde el kernel diga.
    //
    // [!] La numeracion de las reubicaciones NO es la de `SectionKind`: alli
    // `Data = 0x03`, y aqui es **1** (`0` = code, `1` = data, `2` = rodata), que
    // es lo que `relocations.rs` deja escrito. Cruzar las dos daria un binario
    // que carga y escribe el monton encima del codigo.
    let mut relocs_data: Vec<Relocation> = Vec::new();
    if !e.reubicaciones_del_monton.is_empty() {
        b.add_section(BefSection::data(vec![0u8; 8]));
        relocs_data = e
            .reubicaciones_del_monton
            .iter()
            .map(|off| Relocation {
                offset: *off as u64,
                symbol_idx: 1, // data, en la numeracion de las reubicaciones
                kind: RelocationKind::SeccionAbs64 as u8,
                target_section: 0, // el hueco vive en el codigo
                _pad: [0; 2],
                addend: 0,
            })
            .collect();
    }

    if !e.congelados.is_empty() {
        let (rodata, donde) = rodata_de(e);
        b.add_section(BefSection::rodata(rodata));

        // ** `SeccionAbs64` y no `Abs64`: no hay simbolo de por medio, hay una
        // POSICION dentro de otra seccion de este mismo binario. Y ojo con la
        // trampa que el propio formato deja escrita -- los codigos de seccion de
        // una reubicacion **no son los de `SectionKind`**: aqui `2` es rodata.
        relocs_data.extend(e.reubicaciones.iter().filter_map(|(off, i)| {
            donde.get(*i as usize).map(|d| Relocation {
                offset: *off as u64,
                symbol_idx: 2, // rodata, en la numeracion de las reubicaciones
                kind: RelocationKind::SeccionAbs64 as u8,
                target_section: 0, // el hueco vive en el codigo
                _pad: [0; 2],
                addend: *d as i64,
            })
        }));
    }

    // ** LAS REUBICACIONES VAN JUNTAS Y AL FINAL, y esto era un fallo esperando.
    //
    // Estaban DENTRO del `if` de las tablas congeladas, asi que un programa que
    // pidiera el monton de la tarea **y no tuviera ni una tabla** se habria
    // llevado su seccion `Data` sin la reubicacion que la alcanza: el inmediato
    // se quedaria a cero y el monton estaria en la direccion 0.
    //
    // Compilaria, pasaria el gate, y moriria al primer `texto + texto`.
    if !relocs_data.is_empty() {
        b.add_section(BefSection::relocs(relocs_data));
    }

    // ** LO QUE EL BINARIO DICE DE SI MISMO, y llega HECHO.
    //
    // Este crate no sabe que es un perfil, ni que es una pieza, ni que es
    // `crudo`. Recibe un texto y lo mete en su seccion -- por la misma regla que
    // le prohibe al frontend saber que existe un "registro de argumento".
    //
    // El `Option` no es una comodidad: hay bancos que emiten bytes sin compilar
    // un modulo, y para esos no hay modulo del que declarar nada. Poner un
    // manifiesto vacio ahi diria "este binario no trae piezas", que es una
    // respuesta distinta de "no lo se".
    //
    // ** La bandera `HAS_MANIFEST` NO se pone aqui: la enciende `build()` al ver
    // la seccion. Un productor que se acuerda es un productor que un dia no se
    // acuerda.
    // ** DECLARAR ES UN ACTO, y por eso las dos vistas van juntas.
    //
    // El manifiesto es el TOML para humanos; la mesa de katanas es la version
    // que se lee sin parser. **Dos vistas del mismo hecho** -- la misma regla
    // que `requisitos.rs` dejo escrita para Ring 0.
    //
    // *** Y por eso van dentro del MISMO `if`. La primera vez se escribieron
    // separadas y el `.bex` sin manifiesto crecio igual: un binario que no
    // declara su perfil pero si donde corta. Medio declarado no es una postura,
    // es un descuido con forma de decision.
    //
    // Sin manifiesto, `empaquetar` produce exactamente lo que producia antes de
    // P1 -- y hay una prueba que lo fija en 8.752 bytes sobre la sonda de
    // verdad, para que esa linea base no se pueda mover sin querer.
    if let Some(t) = manifiesto {
        b.add_section(BefSection::manifest_toml(t.as_bytes().to_vec()));

        // Por cada regla, su codigo y DONDE esta su bloque de trampa. Es lo que
        // convierte *"este binario atrapa"* en algo que se puede ir a mirar:
        // sin esto el bloque esta en los bytes y **nadie sabe donde**.
        //
        // Va aunque este vacia. Cero katanas es la respuesta honesta de un
        // binario sin reglas, y es distinta de no traer tabla, que es no decir
        // nada -- la primera se puede contrastar y la segunda no.
        let filas: Vec<Katana> = e
            .katanas
            .iter()
            .map(|(codigo, off, len)| Katana {
                codigo: *codigo as u32,
                offset: *off as u32,
                longitud: *len as u32,
            })
            .collect();
        let tabla = katanas::construir(&filas).map_err(|f| f.nombre().to_string())?;
        // ** Y se revisa CONTRA EL CODIGO antes de escribirla, no despues.
        //
        // Aqui es donde el emisor puede equivocarse de verdad: un offset mal
        // apuntado da una tabla que dice que la trampa esta donde hay otra cosa.
        // Comprobarlo despues seria dejar el fichero escrito con la mentira
        // dentro.
        katanas::revisar(&tabla, e.codigo.len()).map_err(|f| f.nombre().to_string())?;
        b.add_section(BefSection::new(SectionKind::Katanas, tabla));
    }


    let bytes = b.build().map_err(|x| x.to_string())?;

    // ** EL GATE SE EXIGE A SI MISMO LO QUE ACABA DE PROMETER.
    //
    // Si se paso un manifiesto, el `.bex` TIENE que traerlo. Sin esta linea, el
    // dia que alguien rompa el cableado --un `None` que se cuela, una seccion
    // que no se anade-- saldria un binario correcto por dentro y **mudo por
    // fuera**, con el gate diciendo que todo esta bien.
    //
    // Es la clase de fallo que este proyecto ya conoce: el que no rompe nada y
    // sobrevive. Cuesta una comparacion y se cierra aqui.
    // ** Y desde S2, tambien QUE LA MESA CUADRE CON EL CODIGO.
    //
    // *** Esta es la linea que hace que la mesa de katanas valga algo. Sin ella
    // la tabla es una lista de numeros que nadie ha contrastado con nada -- y un
    // offset mal apuntado saldria firmado, diciendo que la trampa esta donde hay
    // otra cosa.
    //
    // El compilador se lo exige a SI MISMO: si algun dia el emisor se equivoca
    // de offset, **no se escribe el fichero**. Es la unica postura que se
    // sostiene, porque quien escribe la tabla es tambien quien podria mentir sin
    // querer.
    let veredicto = if manifiesto.is_some() {
        match bmo_verify::declaracion::exige_manifiesto(&bytes) {
            bmo_verify::Verdict::Ok => match bmo_verify::declaracion::exige_katanas(&bytes) {
                bmo_verify::Verdict::Ok => auditar(e),
                malo => malo,
            },
            malo => malo,
        }
    } else {
        bmo_verify::verify(&bytes)
    };

    match veredicto {
        bmo_verify::Verdict::Ok => Ok(bytes),
        bmo_verify::Verdict::Rejected(motivos) => Err(motivos.join("; ")),
    }
}

#[cfg(test)]
mod pruebas;
