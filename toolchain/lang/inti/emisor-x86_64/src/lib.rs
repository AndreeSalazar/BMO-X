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
pub mod marco;
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

    if m.funciones.iter().any(|f| f.nombre == arranque::PRINCIPAL) {
        let hueco = arranque::emitir(&mut salida.codigo, &taller.puerta, IZQ);
        salida
            .huecos_de_llamada
            .push((hueco, arranque::PRINCIPAL.to_string()));
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
    /// **Donde acabo el bloque de trampa de cada regla**: `(codigo, offset,
    /// longitud)`, con el offset DENTRO de la seccion de codigo.
    ///
    /// ** Este dato solo lo tiene quien emite, y hasta hoy se tiraba. Sin el, la
    /// afirmacion *"este binario atrapa"* no se puede contrastar con nada: el
    /// bloque esta en los bytes y **nadie sabe donde**.
    katanas: Vec<(u64, usize, usize)>,
}

fn emitir_funcion(f: &FuncionIr, out: &mut Vec<u8>, taller: &Taller) -> Cuenta {
    // ** Los que la maquina pisa por sus propias instrucciones se quitan del
    // reparto; el resto sigue disponible. Antes se apagaba el reparto ENTERO en
    // cuanto habia una, y eso es pagar el precio de una llamada por algo que la
    // tabla acota en una fila.
    let pisados = metal::registros_que_pisa(f, taller);
    let libres: Vec<u8> = taller
        .temporales
        .iter()
        .copied()
        .filter(|r| !pisados.contains(r))
        .collect();
    let marco = Marco::con_registros(f, &libres);
    let mut cuenta = Cuenta {
        en_registros: marco.en_registros(),
        en_pila: f.temporales as usize - marco.en_registros(),
        ..Default::default()
    };
    let mut comprobaciones = 0usize;
    let mut salida_huecos: Vec<(usize, String)> = Vec::new();
    let mut sin_emitir: Vec<String> = Vec::new();
    let mut reubicaciones: Vec<(usize, u32)> = Vec::new();

    // Prologo.
    out.push(0x55); // push rbp
    x86::mov_r64_r64(out, 5, 4); // mov rbp, rsp
    let tam = marco.size();
    if tam > 0 {
        if tam <= 127 {
            x86::sub_r64_imm8(out, 4, tam as i8);
        } else {
            // Marcos grandes: se emite el inmediato de 32 bits a mano porque
            // `bmo_lower` no trae ese helper todavia.
            out.extend_from_slice(&[0x48, 0x81, 0xEC]);
            out.extend_from_slice(&(tam as u32).to_le_bytes());
        }
    }

    // ** Los parametros llegan en registros y las locales viven en el marco,
    // asi que lo primero que hace toda funcion es bajarlos.
    //
    // El orden de esos registros es la convencion de llamada de esta maquina, y
    // por eso esta linea solo puede existir en este crate: el frontend tiene
    // prohibido saber que existe algo llamado "registro de argumento".
    for i in 0..f.parametros.min(6) as usize {
        mov_a_marco(out, marco.local(Local(i as u32)), ARGUMENTOS[i]);
    }

    // Los saltos se rellenan al final, cuando se sabe donde cayo cada etiqueta.
    let mut sitios_de_etiqueta: Vec<(u32, usize)> = Vec::new();
    let mut huecos: Vec<(usize, u32)> = Vec::new();
    // A donde saltan las comprobaciones que fallan.
    // ** Cada hueco lleva SU codigo, y eso es lo que hacia falta para que
    // hubiera mas de una regla. Con un solo destino de trampa, atrapar por
    // dividir entre cero habria devuelto E1001 -- el codigo de desbordar -- y
    // el programa habria dicho que le paso otra cosa.
    let mut huecos_de_atrapa: Vec<(usize, u64)> = Vec::new();

    for i in &f.instrucciones {
        match i {
            Instr::Etiqueta(e) => sitios_de_etiqueta.push((e.0, out.len())),

            Instr::Mueve { destino, origen } => {
                carga(out, IZQ, origen, &marco);
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            Instr::Guarda { destino, valor } => {
                carga(out, IZQ, valor, &marco);
                mov_a_marco(out, marco.local(*destino), IZQ);
            }

            Instr::Binaria {
                destino,
                op,
                clase,
                izquierda,
                derecha,
            } => {
                carga(out, IZQ, izquierda, &marco);
                carga(out, DER, derecha, &marco);
                // ** La clase viene DE LA IR, no se adivina aqui. Los ocho
                // bytes de un flotante y los de un entero son indistinguibles,
                // asi que un emisor que lo decidiera mirando el valor acertaria
                // casi siempre -- que es peor que fallar siempre.
                match clase {
                    Clase::Entero => binaria(out, *op),
                    Clase::Flotante => flotante(out, *op),
                }
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            // ** LA CONVERSION, que es la unica vez que los bits CAMBIAN.
            //
            // Todo lo demas de este emisor mueve ocho bytes de un sitio a otro
            // sin tocarlos. Aqui no: `5` y `5.0` no comparten un solo bit, y
            // hay una instruccion que los traduce. De ahi que sea una
            // instruccion de la IR y no un `mov` con otro nombre.
            Instr::Convierte {
                destino,
                valor,
                desde,
                hacia,
            } => {
                carga(out, IZQ, valor, &marco);
                match (desde, hacia) {
                    (Clase::Entero, Clase::Flotante) => {
                        x86::cvtsi2sd_de_r64(out, IZQ);
                        x86::movq_r64_de_xmm(out, IZQ, 0);
                    }
                    (Clase::Flotante, Clase::Entero) => {
                        x86::movq_xmm_de_r64(out, 0, IZQ);
                        x86::cvttsd2si_r64(out, IZQ);
                    }
                    // De entero a entero y de flotante a flotante, los bits ya
                    // estan. Estrechar --de `entero64` a `entero8`-- es otra
                    // cosa y todavia no se pide: el ancho de una local lo
                    // reparte el marco, no la conversion.
                    _ => {}
                }
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            Instr::Unaria { destino, op, valor } => {
                carga(out, IZQ, valor, &marco);
                match op {
                    bmo_inti_front::arbol::OpUno::Menos => x86::neg_r64(out, IZQ),
                    bmo_inti_front::arbol::OpUno::No => {
                        // `no x` sobre un logico: comparar con cero y quedarse
                        // con el bit de igualdad.
                        x86::test_r64_r64(out, IZQ, IZQ);
                        out.extend_from_slice(&[0x0F, 0x94, 0xC0]); // sete al
                        out.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx
                    }
                }
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            // ** La regla, en bytes.
            Instr::Comprueba { que, sobre, contra, .. } => {
                comprobaciones += 1;
                let codigo: u64 = que.codigo()[1..].parse().unwrap_or(0);
                match que {
                    Comprobacion::Desborde => {
                        // `jo` mira la bandera que la propia suma dejo puesta:
                        // la comprobacion no vuelve a calcular nada, solo
                        // pregunta. Por eso cuesta lo que cuesta.
                        out.extend_from_slice(&[0x0F, 0x80]);
                        huecos_de_atrapa.push((out.len(), codigo));
                        out.extend_from_slice(&[0, 0, 0, 0]);
                    }

                    // ** REGLA 3, y son cuatro instrucciones.
                    //
                    // La IR ya la coloco ANTES de la division y le paso el
                    // DIVISOR, que era la pieza que faltaba: mirar el resultado
                    // no sirve de nada porque dividir entre cero no deja
                    // resultado, deja una excepcion del procesador.
                    Comprobacion::EntreCero => {
                        carga(out, IZQ, sobre, &marco);
                        x86::test_r64_r64(out, IZQ, IZQ);
                        out.extend_from_slice(&[0x0F, 0x84]); // jz
                        huecos_de_atrapa.push((out.len(), codigo));
                        out.extend_from_slice(&[0, 0, 0, 0]);
                    }

                    // *** LA REGLA 1 ESCONDIDA DENTRO DE UNA DIVISION.
                    //
                    // `-2^63 entre -1` no cabe en 64 bits. Es la unica de las
                    // cinco que mira DOS valores, y por eso `Comprueba` lleva un
                    // `contra`: el cociente solo se sale cuando el dividendo es
                    // el minimo Y el divisor es -1.
                    //
                    // El camino que no atrapa paga una comparacion y un salto
                    // que no salta: al segundo `cmp` solo se entra si el divisor
                    // es exactamente -1, que casi nunca.
                    Comprobacion::Cociente => {
                        let Some(divisor) = contra else {
                            // Sin el segundo valor no hay nada que comprobar, y
                            // callar aqui seria emitir una regla que aprueba
                            // todo. Se dice y no se emite.
                            sin_emitir.push(
                                "la regla del cociente llego sin su segundo valor".to_string(),
                            );
                            continue;
                        };
                        carga(out, DER, divisor, &marco);
                        x86::cmp_r64_imm32(out, DER, -1);
                        let al_final = x86::salto_corto(out, 0x75); // jne
                        carga(out, IZQ, sobre, &marco);
                        x86::mov_r64_imm64(out, 2, i64::MIN as u64);
                        x86::cmp_r64_r64(out, IZQ, 2);
                        out.extend_from_slice(&[0x0F, 0x84]); // je -> atrapa
                        huecos_de_atrapa.push((out.len(), codigo));
                        out.extend_from_slice(&[0, 0, 0, 0]);
                        x86::cierra_salto_corto(out, al_final);
                    }

                    // ** REGLA 12 -- cabe este numero en tantos bytes?
                    Comprobacion::Conversion(bytes) => {
                        regla_doce(out, sobre, *bytes, &marco, &mut huecos_de_atrapa, codigo);
                    }

                    // ** LA 2 SIGUE SIN EMITIRSE, y ahora esta sola con su
                    // motivo -- que es distinto del que tenian las otras dos.
                    //
                    // Las otras dos no salian por un fallo de sitio: la IR las
                    // ponia detras de la operacion, donde ya no habia nada que
                    // mirar. Esta no sale porque **no hay contra que
                    // comprobar**: un `bufer de T` es una direccion y no lleva
                    // su longitud dentro. Por eso indexarlo pide `crudo`.
                    //
                    // La comprobacion nace con `lista de T` de `pleno`, que SI
                    // lleva la suya. No es deuda de este fichero: es una que
                    // espera a un tipo que todavia no existe.
                    Comprobacion::Indice => {
                        comprobaciones -= 1;
                    }
                }
            }

            Instr::Devuelve(v) => {
                if let Some(v) = v {
                    carga(out, IZQ, v, &marco);
                }
                epilogo(out);
            }

            Instr::Salta(e) => {
                out.push(0xE9);
                huecos.push((out.len(), e.0));
                out.extend_from_slice(&[0, 0, 0, 0]);
            }

            Instr::SaltaSi { cond, falso, .. } => {
                carga(out, IZQ, cond, &marco);
                x86::test_r64_r64(out, IZQ, IZQ);
                // Si es cero, al camino falso; si no, sigue.
                out.extend_from_slice(&[0x0F, 0x84]);
                huecos.push((out.len(), falso.0));
                out.extend_from_slice(&[0, 0, 0, 0]);
            }

            Instr::Llama {
                destino,
                que,
                argumentos,
            } => {
                // ** Y antes de nada: es esto una llamada, o es LA PUERTA?
                //
                // La diferencia no la decide una palabra del lenguaje. La
                // decide una fila de `modulos.toml` que el usuario pidio con
                // `usa bmo` -- y por eso `invoca` nunca fue palabra clave, que
                // era la condicion que Eddi puso dos veces.
                //
                // Aqui se ve entera: quitar esa fila de la tabla apaga la
                // puerta sin tocar una linea de este fichero.
                if let Valor::Nombre(n) = que {
                    if taller.abre_la_puerta(n) {
                        let p = &taller.puerta;
                        for (i, a) in argumentos.iter().enumerate().take(p.caben()) {
                            carga(out, p.argumentos[i], a, &marco);
                        }
                        // Solo hay una puerta. Ese es el congelamiento de los
                        // dos syscalls, visto desde el unico sitio donde se
                        // notaria si dejara de ser verdad.
                        x86::mov_r32_imm32(out, p.numero, NR_INVOKE);
                        x86::syscall(out);
                        // ** Y de DONDE se recoge no lo decide la instruccion:
                        // lo decide el nombre. La misma puerta contesta un
                        // codigo y un valor a la vez, por registros distintos.
                        if let Some(d) = destino {
                            let de = p.recogida(taller.recoge.recoge(n));
                            guarda_temporal(out, de, *d, &marco);
                        }
                        continue;
                    }
                }

                // Los argumentos van a los registros que dice la convencion.
                //
                // ** Y aqui se ve para que sirve el freno del asignador: como
                // una funcion con llamadas no reparte registros, ningun
                // argumento puede estar viviendo en `rdi` cuando toca cargar el
                // siguiente. Cargarlos en orden es seguro **porque el reparto
                // se apago**, no por suerte.
                for (i, a) in argumentos.iter().enumerate().take(6) {
                    carga(out, ARGUMENTOS[i], a, &marco);
                }

                match que {
                    Valor::Nombre(n) => {
                        out.push(0xE8); // call rel32
                        salida_huecos.push((out.len(), n.clone()));
                        out.extend_from_slice(&[0, 0, 0, 0]);
                    }
                    otro => {
                        // Una llamada a un valor --una funcion guardada en una
                        // variable-- pide `call reg`. Se deja sin emitir en vez
                        // de emitir algo que salta a donde no debe.
                        let _ = otro;
                    }
                }

                // Lo que devuelve viene en el registro de retorno.
                if let Some(d) = destino {
                    guarda_temporal(out, IZQ, *d, &marco);
                }
            }
            // ** TOCAR MEMORIA. La IR pide "lee 8 bytes de esta direccion" y
            // aqui se elige la instruccion. Ese es el reparto entero: el ancho
            // en bytes es agnostico, el opcode no.
            Instr::Lee {
                destino,
                direccion,
                ancho,
            } => {
                carga(out, IZQ, direccion, &marco);
                match ancho {
                    // Un byte se lee con `movzx` y no con un `mov` de 8 bits:
                    // el `mov` dejaria intactos los 56 bits de arriba, asi que
                    // el resultado traeria basura de lo que hubiera antes en el
                    // registro. Lo peor es que funcionaria casi siempre.
                    1 => x86::movzx_r32_byte_at_reg(out, IZQ, IZQ),
                    2 => x86::movzx_r32_word_at_reg(out, IZQ, IZQ),
                    // ** El de 32 no lleva `movzx` y no es un olvido: escribir
                    // la mitad baja de un registro **pone a cero la mitad
                    // alta** en 64 bits. Por debajo de 32, el silicio conserva
                    // lo que hubiera, y por eso 8 y 16 si lo necesitan.
                    4 => x86::mov_r32_at_reg(out, IZQ, IZQ),
                    8 => x86::mov_r64_at_reg(out, IZQ, IZQ),
                    // Un ancho que no esta en la tabla no puede llegar aqui.
                    // Si llegara, devolver cero es lo unico honesto: dejar el
                    // registro como estaba seria mentir con la direccion
                    // dentro, y esa mentira parece un puntero valido.
                    _ => x86::zero_r32(out, IZQ),
                }
                guarda_temporal(out, IZQ, *destino, &marco);
            }
            // *** LA DIRECCION DE UNA TABLA CONGELADA (2026-08-22).
            //
            // Se emite un `mov reg, imm64` con el inmediato **a cero**, y se
            // apunta donde quedo: la direccion de verdad no se sabe hasta que el
            // cargador coloque `RoData`, y la rellena una reubicacion.
            //
            // ** Y esto es una INSTRUCCION de la IR, no un `Valor`, justamente
            // para que este sea el UNICO sitio del emisor que tiene que apuntar
            // una reubicacion. Con un `Valor::Congelado`, los veintitres sitios
            // que cargan un valor tendrian que acordarse -- y el dia que alguien
            // anadiera el veinticuatro, la tabla se cargaria con un cero.
            Instr::Direccion { destino, congelado } => {
                x86::mov_r64_imm64(out, IZQ, 0);
                // El inmediato son los ocho ultimos bytes de lo que se acaba de
                // emitir. Contarlo desde el principio del `mov` obligaria a
                // saber si el REX esta o no.
                reubicaciones.push((out.len() - 8, *congelado));
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            Instr::Escribe {
                direccion,
                valor,
                ancho,
            } => {
                // *** LOS DOS PUEDEN VIVIR EN EL REGISTRO DEL OTRO, y entonces
                // no hay orden que valga.
                //
                // Aqui habia esto escrito: *"el valor primero: cargar la
                // direccion antes lo perderia si los dos vivieran en el mismo
                // sitio"*. Vio la mitad del problema y arreglo la mitad.
                //
                // La otra mitad: si la DIRECCION vive en `DER`, cargar el valor
                // ahi primero la machaca **antes de leerla**. Y si ademas el
                // valor vive en `IZQ`, cualquiera de los dos ordenes destroza al
                // otro:
                //
                // ```text
                //    valor en IZQ, direccion en DER
                //    mov rcx, rax   -> el valor pisa la direccion
                //    mov rax, rcx   -> y ahora los dos son el valor
                // ```
                //
                // ** Lo encontro un programa de verdad --el escritor de PNG de
                // `ejemplos/`-- y no una prueba: hacen falta DOS operaciones
                // antes de la escritura para que los dos registros esten
                // ocupados a la vez. Con una sola, cualquiera de los dos ordenes
                // funciona, y por eso el banco no lo vio nunca.
                //
                // El caso cruzado se resuelve con un intercambio; los demas, con
                // el orden que no pisa.
                let en_izq = |v: &Valor| {
                    matches!(v, Valor::Temporal(t) if marco.sitio(*t) == Sitio::Registro(IZQ))
                };
                let en_der = |v: &Valor| {
                    matches!(v, Valor::Temporal(t) if marco.sitio(*t) == Sitio::Registro(DER))
                };
                if en_izq(valor) && en_der(direccion) {
                    // Cruzados: un `xchg` y los dos quedan donde toca.
                    x86::xchg_r64_r64(out, IZQ, DER);
                } else if en_der(direccion) {
                    // La direccion ya esta donde estorba: se salva primero.
                    carga(out, IZQ, direccion, &marco);
                    carga(out, DER, valor, &marco);
                } else {
                    // El caso corriente, y el que ya estaba bien.
                    carga(out, DER, valor, &marco);
                    carga(out, IZQ, direccion, &marco);
                }
                match ancho {
                    1 => x86::mov_byte_at_reg_from_low(out, IZQ, DER),
                    2 => x86::mov_word_at_reg_from_r16(out, IZQ, DER),
                    4 => x86::mov_at_reg_from_r32(out, IZQ, DER),
                    8 => x86::mov_at_reg_from_r64(out, IZQ, DER),
                    _ => {}
                }
            }
            // El metal se emite cuando el emisor lea `intrinsics.toml`. Se deja
            // marcado, no escondido.
            Instr::Metal {
                destino,
                nombre,
                argumentos,
            } => {
                metal(
                    out,
                    nombre,
                    argumentos,
                    *destino,
                    &marco,
                    taller,
                    &mut sin_emitir,
                );
            }
        }
    }

    // Toda funcion acaba volviendo, aunque el fuente no lo diga.
    epilogo(out);

    // ** EL SITIO AL QUE VAN LAS COMPROBACIONES QUE FALLAN -- uno POR CODIGO.
    //
    // Antes habia uno solo con `1001` escrito a mano, porque solo se emitia una
    // regla. Con dos, ese destino unico habria contado una mentira concreta:
    // atrapar por dividir entre cero habria devuelto E1001 --desbordamiento-- y
    // el programa habria dicho que le paso otra cosa.
    //
    // Van al final de la funcion y no al lado de cada comprobacion **a
    // proposito**: el camino que se recorre siempre es el que no atrapa, y
    // meterle un bloque de cinco instrucciones en medio lo llena de saltos.
    // Aqui el coste de la regla en el camino normal es UNA instruccion que casi
    // nunca salta -- que es de donde sale el 1% de la seccion 6.3.
    let mut codigos: Vec<u64> = huecos_de_atrapa.iter().map(|(_, c)| *c).collect();
    codigos.sort_unstable();
    codigos.dedup();
    for codigo in codigos {
        let atrapa = out.len();
        // El codigo se pone en el registro de retorno y se vuelve. Cuando haya
        // errores como datos de verdad, esto construira el valor de error.
        x86::mov_r64_imm64(out, IZQ, codigo);
        epilogo(out);
        // ** Y se APUNTA DONDE QUEDO. Es el unico momento en toda la compilacion
        // en que se sabe: dentro de un instante estos bytes son indistinguibles
        // del resto del codigo.
        cuenta.katanas.push((codigo, atrapa, out.len() - atrapa));
        for (h, c) in huecos_de_atrapa.iter().filter(|(_, c)| *c == codigo) {
            let _ = c;
            let rel = (atrapa as i64 - (*h as i64 + 4)) as i32;
            out[*h..*h + 4].copy_from_slice(&rel.to_le_bytes());
        }
    }

    // Y ahora los saltos.
    for (hueco, etiqueta) in huecos {
        let destino = sitios_de_etiqueta
            .iter()
            .find(|(e, _)| *e == etiqueta)
            .map(|(_, off)| *off)
            .unwrap_or(out.len());
        let rel = (destino as i64 - (hueco as i64 + 4)) as i32;
        out[hueco..hueco + 4].copy_from_slice(&rel.to_le_bytes());
    }

    cuenta.comprobaciones = comprobaciones;
    cuenta.huecos_de_llamada = salida_huecos;
    cuenta.reubicaciones = reubicaciones;
    cuenta.sin_emitir = sin_emitir;
    cuenta
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
            Instr::Direccion { .. } => {}
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
    if !e.congelados.is_empty() {
        let mut rodata: Vec<u8> = Vec::new();
        let mut donde: Vec<u64> = Vec::with_capacity(e.congelados.len());
        for c in &e.congelados {
            // Alineadas a ocho: una tabla de `entero64` leida a medias es lenta
            // en el mejor caso y una excepcion en el peor.
            while rodata.len() % 8 != 0 {
                rodata.push(0);
            }
            donde.push(rodata.len() as u64);
            // *** UN TEXTO LLEVA CABECERA DE OBJETO; UNA TABLA, NO.
            //
            // Y la pone AQUI y no el frontend a proposito: la forma de un objeto
            // del monton la declara `bmo_abi::dynobj`, y el frontend no enlaza
            // `bmo-abi` --lo dice la cabecera de su `Cargo.toml`, y es la linea
            // que le deja no saber de bytes--. Escribir alli veinticuatro bytes
            // a mano seria una segunda declaracion del mismo contrato, que es
            // como acaban discrepando.
            //
            // ** `congelado` y no `nacer`: el bit 63 puesto, INMORTAL. Un
            // literal no se cuenta, no se libera y ademas vive en una seccion de
            // solo lectura -- las tres cosas dicen lo mismo y ninguna sobra.
            if matches!(c.clase, ClaseCongelada::Texto) {
                let n = c.bytes.len() as u64;
                let mut cab = vec![0u8; dynobj_texto::CABECERA_LEN];
                // `type_index` = 0 mientras no exista el mapa de tipos.
                //
                // [!] Cero significa *"el `TypeMap` no existe"*, no *"el tipo
                // cero"*. `SectionKind::TypeMap = 0x10` es el quinto hueco
                // declarado y vacio del formato, y `lista.rs` ya lo dejo dicho.
                // Inventar aqui una numeracion propia es como se consiguen dos
                // numeraciones el dia que llegue la de verdad.
                dynobj_texto::congelado(&mut cab, 0, n)
                    .expect("la cabecera de un texto siempre cabe en su propio tamano");
                rodata.extend_from_slice(&cab);
            }
            rodata.extend_from_slice(&c.bytes);
        }
        b.add_section(BefSection::rodata(rodata));

        // ** `SeccionAbs64` y no `Abs64`: no hay simbolo de por medio, hay una
        // POSICION dentro de otra seccion de este mismo binario. Y ojo con la
        // trampa que el propio formato deja escrita -- los codigos de seccion de
        // una reubicacion **no son los de `SectionKind`**: aqui `2` es rodata.
        let relocs: Vec<Relocation> = e
            .reubicaciones
            .iter()
            .filter_map(|(off, i)| {
                donde.get(*i as usize).map(|d| Relocation {
                    offset: *off as u64,
                    symbol_idx: 2, // rodata, en la numeracion de las reubicaciones
                    kind: RelocationKind::SeccionAbs64 as u8,
                    target_section: 0, // el hueco vive en el codigo
                    _pad: [0; 2],
                    addend: *d as i64,
                })
            })
            .collect();
        if !relocs.is_empty() {
            b.add_section(BefSection::relocs(relocs));
        }
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
