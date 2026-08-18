//! **UN NIVEL de la ruta**: donde esta, como se llama, y QUE HAY DENTRO.
//!
//! === Por que existe este fichero ===
//!
//! El cursor guardaba la ruta como una pila de punteros y **un solo listado**:
//! el del nodo donde estabas. Con eso se pinta una lista, que es lo que habia.
//!
//! No se pinta un ARBOL. Un panel de arbol --el de la izquierda de cualquier
//! explorador-- ensena a la vez los hijos de la raiz, los del nivel siguiente y
//! los del siguiente, con la rama por la que has bajado abierta. Eso son varios
//! listados vivos al mismo tiempo, y la pila solo tenia el ultimo.
//!
//! * **La correccion NO es "un cursor por ventana".** El aviso que habia escrito
//! en `userland/src/estratos.rs` decia que el dia que hubiera dos clientes esto
//! pediria un handle por cliente. Ese dia no ha llegado: el arbol de la
//! izquierda y la rejilla de la derecha **no son dos recorridos**, son EL MISMO
//! recorrido mirado a dos profundidades. Lo que faltaba no era otro cursor: era
//! que el cursor no se olvidara de por donde ha pasado.
//!
//! Dos cursores independientes seguiran haciendo falta el dia que haya dos
//! ventanas mirando sitios distintos. No es hoy, y adelantarlo costaria tabla,
//! ciclo de vida y revocacion para un problema que nadie tiene.
//!
//! === Y de paso, el martillo sobre el disco ===
//!
//! `hijo_tipo`, `hijo_bytes`, `hijo_atributos` y `hijo_firmado` contestaban
//! **leyendo un bloque cada una**, porque el dato vive en el NODO y la entrada
//! del directorio solo guarda el nombre y a donde apunta. El panel las llama
//! por fila, y el panel se repinta al mover el raton.
//!
//! Con veinte filas visibles eso son cuarenta lecturas de bloque por
//! repintado, y arrastrar la ventana repinta. Un panel que mira no puede ser un
//! martillo sobre el disco -- es el mismo argumento que ya tiene escrito el
//! lanzador de iconos: *se hace UNA VEZ, ninguna cambia mientras la maquina
//! esta encendida*.
//!
//! Aqui se lee el detalle de cada hijo **al listar el nivel**, que es la unica
//! vez que puede haber cambiado. Repintar deja de tocar el disco.
//!
//! === Lo que esto cuesta, dicho ===
//!
//! Un nivel ocupa unos 8 KiB --7 de listado crudo y 1 de detalle-- y hay
//! `HONDO_MAX` niveles, asi que la pila entera son unos 130 KiB de `.bss`.
//!
//! No es poco y no se esconde. Es del orden de lo que este mismo modulo ya
//! tiene apartado para verificar una firma (256 KiB), y compra dos cosas que no
//! se pueden comprar de otra manera: el panel de arbol, y que mover el raton
//! encima de una ventana deje de leer del disco.

use super::*;

/// Lo que se guarda del nombre de cada nivel de la ruta, PARA ENSENARLO.
///
/// Un nombre mas largo se recorta al pintar la miga de pan y no pasa nada: el
/// nombre entero sigue en la entrada del padre, que es de donde se abre.
pub const LEVEL_NAME: usize = 32;

/// Lo que cuesta un salto al disco, leido UNA VEZ.
///
/// Todo lo de aqui vive en el nodo del hijo, no en la entrada del directorio
/// que lo nombra. Por eso cada campo era una lectura de bloque, y por eso estan
/// juntos: se traen los cuatro con el mismo viaje.
#[derive(Clone, Copy)]
pub(crate) struct Detalle {
    /// Bytes del contenido. Un directorio contesta lo que ocupa su LISTA de
    /// entradas, que es un dato distinto de lo que hay dentro.
    pub bytes: u64,
    /// Cuantos atributos lleva. Es el numero que dice que ESTRATOS no es un
    /// sistema de carpetas: un nodo es un conjunto de atributos.
    pub atributos: u8,
    /// `0` archivo, `1` directorio, `2` no se pudo leer.
    ///
    /// El `2` **no es lo mismo que archivo** y por eso no es un `bool`:
    /// confundirlos pinta una caja para algo que no existe.
    pub tipo: u8,
    /// Lleva `:firma`? Dice si LA LLEVA, no si cuadra -- comprobarlo exige leer
    /// el contenido entero y eso se pide a mano (`cursor::verify`).
    pub firmado: bool,
}

impl Detalle {
    /// Lo que se contesta de un hijo que no se pudo leer. `tipo = 2` es la
    /// respuesta honesta: no se sabe que es.
    pub const ILEGIBLE: Self = Self { bytes: 0, atributos: 0, tipo: 2, firmado: false };
}

/// Un nivel de la ruta, con su listado YA LEIDO.
///
/// [!] Esto mide unos 8 KiB y **nunca se devuelve por valor**. Se rellena en
/// sitio a traves de `&mut self`, viviendo donde ya vive: dentro del `static`
/// de la pila. Un constructor que lo devolviera lo montaria en la pila de quien
/// llama, y 8 KiB en el marco de un manejador de syscall es como se desborda
/// una pila de kernel. Es la misma razon por la que el `Launcher` de Ring 3
/// lleva `#[inline(never)]`, medida en el Ryzen.
pub(crate) struct Nivel {
    /// Donde esta el nodo de este nivel.
    pub ptr: Option<BlockPtr>,
    /// **Y el nodo ya leido.** Es lo que hace que subir no toque el disco.
    ///
    /// Guardar el puntero y no el nodo obligaba a `subir()` a traerlo otra vez
    /// --una lectura de bloque por cada vez que se sube una carpeta-- para
    /// contestar algo que ya se supo al bajar. Un nodo son unos cientos de
    /// bytes contra los 8 KiB que este mismo nivel ya guarda de listado: al
    /// lado de lo que hay aqui, es gratis.
    pub nodo: Option<Nodo>,
    /// Como se llama. Se anota AL BAJAR y no se reconstruye despues: un
    /// `BlockPtr` sabe DONDE esta un nodo y no sabe como se llama -- el nombre
    /// vive en la entrada del padre, no en el hijo.
    pub nombre: [u8; LEVEL_NAME],
    pub nombre_len: usize,
    /// Por que hijo se bajo desde aqui, si se bajo por alguno.
    ///
    /// * Lo pide el panel de arbol y no se puede deducir sin el: para saber que
    /// rama esta abierta habria que comparar el nombre del nivel de abajo
    /// contra los de este, o sea reconstruir con cadenas algo que se sabia en
    /// el momento de bajar. Anotarlo al pasar cuesta ocho bytes.
    pub elegido: Option<usize>,
    /// Las entradas crudas de este nivel.
    pub buf: [u8; MAX_ENTRIES * ENTRADA_LEN],
    pub cuantas: usize,
    /// Se quedo el listado corto? Un directorio truncado en silencio se ve
    /// igual que uno con pocos archivos.
    pub truncado: bool,
    /// El detalle de cada hijo, en el mismo orden que `buf`.
    pub detalle: [Detalle; MAX_ENTRIES],
}

impl Nivel {
    pub const VACIO: Self = Self {
        ptr: None,
        nodo: None,
        nombre: [0; LEVEL_NAME],
        nombre_len: 0,
        elegido: None,
        buf: [0; MAX_ENTRIES * ENTRADA_LEN],
        cuantas: 0,
        truncado: false,
        detalle: [Detalle::ILEGIBLE; MAX_ENTRIES],
    };

    /// La entrada numero `i` de este nivel.
    pub fn entrada(&self, i: usize) -> Option<Entrada> {
        if i >= self.cuantas {
            return None;
        }
        Entrada::decode(&self.buf[i * ENTRADA_LEN..(i + 1) * ENTRADA_LEN]).ok()
    }

    /// El detalle del hijo `i`, o `ILEGIBLE` si no hay tal hijo.
    ///
    /// Devuelve un valor y no un `Option` a proposito: quien pregunta es el
    /// otro lado de un syscall, que no tiene forma de expresar "no hay". El
    /// `tipo = 2` ya es esa respuesta y es la misma que daria un nodo roto --
    /// las dos significan *no se puede decir que es esto*.
    pub fn detalle(&self, i: usize) -> Detalle {
        if i >= self.cuantas {
            return Detalle::ILEGIBLE;
        }
        self.detalle[i]
    }

    /// Se apunta el nombre con el que se bajo a este nivel.
    ///
    /// Se recorta a `LEVEL_NAME` si no cabe. Recortar un nombre PARA ENSENARLO
    /// no es lo mismo que recortarlo para abrirlo: aqui se pinta una miga de
    /// pan, no se resuelve una ruta, y el nombre completo sigue en la entrada
    /// del padre.
    pub fn nombrar(&mut self, b: &[u8]) {
        let k = b.len().min(LEVEL_NAME);
        self.nombre[..k].copy_from_slice(&b[..k]);
        self.nombre_len = k;
    }

    /// **Lista el nodo de este nivel y lee el detalle de cada hijo.**
    ///
    /// Aqui es donde se paga el disco, y se paga entero: una lectura por el
    /// listado y una por cada hijo. A cambio, todo lo que pregunte despues
    /// --tipos, tamanos, atributos, firmas-- se contesta de memoria.
    ///
    /// ** El coste esta ACOTADO por `MAX_ENTRIES` y no por el tamano del
    /// directorio: un directorio mas grande se lista truncado y lo dice, asi
    /// que esto son como mucho 65 lecturas y nunca "las que haya".
    pub fn listar(&mut self) -> bool {
        // El nodo se COPIA fuera antes de tocar el buffer: los dos viven en
        // este mismo struct y prestarlos a la vez no compila. `Nodo` es `Copy`
        // y son unos cientos de bytes.
        let Some(n) = self.nodo else {
            self.cuantas = 0;
            self.truncado = false;
            self.releer_detalle();
            return false;
        };
        let ok = match listar_en(&n, &mut self.buf) {
            Some((c, t)) => {
                self.cuantas = c.min(MAX_ENTRIES);
                self.truncado = t;
                true
            }
            // Un archivo no tiene `:entries`, y eso no es un fallo: es que no
            // tiene hijos. Se contesta cero y se sigue.
            None => {
                self.cuantas = 0;
                self.truncado = false;
                true
            }
        };
        self.releer_detalle();
        ok
    }

    /// Trae el nodo de cada hijo y se queda con lo que la ventana pregunta.
    ///
    /// [!] Un hijo ilegible hace que `walk::nodo` avise por CABINA. Antes ese
    /// aviso salia en cada repintado --o sea, mientras arrastrabas la ventana--
    /// y ahora sale una vez por navegacion. Un fallo real se sigue viendo; lo
    /// que se deja de ver es el mismo fallo sesenta veces por segundo.
    fn releer_detalle(&mut self) {
        for i in 0..self.cuantas {
            self.detalle[i] = match self.entrada(i) {
                Some(e) => match super::nodo(&e.nodo) {
                    Some(n) => Detalle {
                        bytes: {
                            let cual = if n.tipo == Tipo::Directorio {
                                bmo_estratos::objects::ATTR_ENTRADAS
                            } else {
                                bmo_estratos::objects::ATTR_DATOS
                            };
                            n.attr(cual).map(|a| a.size).unwrap_or(0)
                        },
                        atributos: n.attrs().count() as u8,
                        tipo: if n.tipo == Tipo::Directorio { 1 } else { 0 },
                        firmado: n.attr(bmo_estratos::objects::ATTR_FIRMA).is_some(),
                    },
                    None => Detalle::ILEGIBLE,
                },
                None => Detalle::ILEGIBLE,
            };
        }
        // Lo que quedo por encima del listado se limpia: si no, un directorio
        // largo seguido de uno corto dejaria el detalle del anterior asomando
        // por debajo, que es peor que no tener detalle.
        for i in self.cuantas..MAX_ENTRIES {
            self.detalle[i] = Detalle::ILEGIBLE;
        }
    }
}

