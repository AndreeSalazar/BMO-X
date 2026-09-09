//! **DECIDIR: lo que el compilador SABE sin emitir un byte.**
//!
//! # *** LA REGLA DE ESTA CARPETA, Y ES TODA SU RAZON DE SER
//!
//! > **El emisor no decide.** Si hay que elegir entre dos secuencias de bytes,
//! > la eleccion se toma ANTES, aqui, y en una funcion **pura**.
//!
//! Un generador de codigo tiene dos trabajos que se parecen y no lo son:
//!
//! ```text
//!    DECIDIR   que hay que emitir      -> aritmetica, formas, tamanos
//!              es PURO: entra un AST, sale un numero. Se prueba en el
//!              anfitrion, en milisegundos, sin CPU y sin emulador
//!    EMITIR    los bytes               -> una tabla de opcodes
//!              no elige nada: recibe la decision ya tomada
//!    COLOCAR   donde va cada cosa      -> el `.bex`
//! ```
//!
//! ** Mezclados, **una optimizacion es un parche sobre bytes** y no hay forma de
//! probarla sin arrancar la maquina. Separados, una optimizacion es una funcion
//! que devuelve `Some(8)` en vez de `None`, y eso se prueba con un `assert_eq!`.
//!
//! # Por que la regla nacio, y con fecha
//!
//! El **2026-09-09** se desensamblo el bucle interior de la expansion de DOOM:
//! **35 instrucciones para escribir 8 bytes**, con un `imulq` dentro para
//! multiplicar `1 x 8` -- el tamano de un `unsigned long long`, conocido al
//! compilar, calculado en cada vuelta.
//!
//! *** Y el plegador de constantes **ya existia**: `constante_de` lleva meses
//! resolviendo `1 << 16` y `-.867*FRACUNIT` para las tablas de DOOM. Estaba
//! completo, probado y **el emisor no le preguntaba**.
//!
//! ```text
//!    lo que faltaba NO era la maquinaria
//!    era que el que emite bytes supiera A QUIEN PREGUNTAR
//! ```
//!
//! Esa es exactamente la clase de fallo que una carpeta con nombre evita: el
//! que va a emitir un `imul` abre esta puerta y ve que la respuesta ya esta.
//!
//! # Los carriles, y por que son DOS y no tres
//!
//! ```text
//!    roja.rs      EL PLEGADO    un numero mal plegado es un numero mal, en
//!                               silencio, en todas las tablas    [cuesta] DATO
//!    amarilla.rs  LA IMAGEN     paginas y regiones: si falla, el programa
//!                               no carga o un global cae mal     [cuesta] TAREA
//! ```
//!
//! No hay verde porque **aqui no hay nada que se pueda tocar sin miedo**: todo
//! lo de esta carpeta acaba dentro de un numero que el programa usa. Inventar un
//! verde para tener los tres seria decir que algo es seguro porque falta un
//! fichero, y esta casa ya retiro una carpeta de carriles por menos.
//!
//! # [!] LO QUE ESTA CARPETA NO ES
//!
//! ```text
//!    [ ] no es un optimizador. Es donde VIVEN las decisiones; que el emisor
//!        las use es cosa suya, y hoy solo usa dos
//!    [ ] no es un asignador de registros. Eso es `C4` de
//!        `docs/plan/PLAN_EL_CODEGEN.md` y es otro proyecto
//!    [ ] y no se puede llamar desde aqui al emisor. Si algo de esta carpeta
//!        necesitara `&mut self` para contestar, es que no era una decision:
//!        era emision disfrazada
//! ```

pub(super) mod amarilla;
pub(super) mod roja;
