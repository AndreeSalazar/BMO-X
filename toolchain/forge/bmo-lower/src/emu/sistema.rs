//! `emu::sistema` -- LO QUE CONTESTA EL SISTEMA, no lo que hace el CPU.
//!
//! ## Por que soy un fichero y no un trozo del de al lado (L6b)
//!
//! Porque son dos preguntas, y el emulador las mezclaba en un solo fichero de
//! 2.373 lineas:
//!
//! ```text
//!    emu/mod.rs      QUE HACE EL CPU      registros, memoria, banderas, y el
//!                                         despacho de instrucciones
//!    emu/sistema.rs  QUE CONTESTA EL SO   la puerta: consola, disco, memoria,
//!                                         entrada, audio -- y como se siembra
//! ```
//!
//! ** Y la senal de que el corte esta bien puesto es cual de los dos crece: el
//! de arriba crece cuando un lenguaje emite una instruccion nueva; este crece
//! cuando el SISTEMA gana una operacion. Son dos calendarios distintos, y
//! juntarlos en un fichero hacia que cada uno pareciera culpa del otro.
//!
//! ## Lo que este fichero se niega a hacer
//!
//! **Inventarse un dato.** Un `TASK_OP` que no esta modelado no devuelve cero:
//! falla. Es la misma regla que `VERDAD.md` escribe para `rdmsr`, aplicada al
//! otro lado de la puerta -- y por eso los ceros que salen de aqui son ceros de
//! verdad y no huecos.

use super::*;

impl Machine {
    /// Siembra lo que el terminal habria tecleado. El `\n` final hace falta:
    /// `read_line` espera verlo para dar la linea por cerrada, exactamente
    /// igual que en la maquina.
    pub fn poner_entrada(&mut self, texto: &str) {
        self.entrada.extend_from_slice(texto.as_bytes());
    }

    /// Siembra un archivo antes de ejecutar. Es el disco de la prueba.
    pub fn poner_archivo(&mut self, ruta: &str, datos: &[u8]) {
        self.archivos.insert(ruta.to_string(), datos.to_vec());
    }

    /// **La propia imagen del programa**, la que contesta `TASK_OP_MI_PAQUETE`.
    ///
    /// El nombre interno no se puede escribir desde el programa --lleva un byte
    /// nulo-- a proposito: si el `.bex` pudiera nombrarlo, la prueba dejaria de
    /// distinguir *"me lo dieron"* de *"lo abri yo por la ruta"*, que es justo
    /// lo que esta operacion existe para separar.
    pub fn poner_mi_paquete(&mut self, datos: &[u8]) {
        let clave = "\u{0}mi-paquete".to_string();
        self.archivos.insert(clave.clone(), datos.to_vec());
        self.mi_paquete = Some(clave);
    }

    /// Hace que guardar ESA ruta falle: el `CLOSE` contestara `0` y en el disco
    /// no quedara nada.
    ///
    /// Es el disco diciendo que no, que es lo unico que un programa puede
    /// observar. Sirve para probar que el programa **se entera** -- un `CLOSE`
    /// que siempre dice que si deja el camino del fallo sin pisar, y ese es
    /// justo el que decide si un fichero se perdio en silencio.
    pub fn fallar_al_guardar(&mut self, ruta: &str) {
        self.fallo_al_guardar.insert(ruta.to_string());
    }

    /// Lo que hay en el disco al terminar. `None` si ese archivo no existe --
    /// que es distinto de existir vacio, y en un batch bancario esa diferencia
    /// es la que separa "no se escribio" de "se escribio cero registros".
    pub fn archivo(&self, ruta: &str) -> Option<&[u8]> {
        self.archivos.get(ruta).map(|v| v.as_slice())
    }

    /// Igual, pero como texto. Comodidad para los tests.
    pub fn archivo_texto(&self, ruta: &str) -> Option<String> {
        self.archivo(ruta).map(|b| String::from_utf8_lossy(b).into_owned())
    }

    // -- Sembrar la entrada ----------------------------------------------

    /// Concede la entrada: a partir de aqui `TASK_OP_INPUT_CLAIM` funciona.
    ///
    /// Hay que pedirlo a proposito porque la entrada es **exclusiva**: sin
    /// esto, la prueba ve lo mismo que un programa lanzado mientras el
    /// compositor la tiene tomada, que es el caso que mas se equivoca al
    /// escribirlo.
    pub fn ceder_entrada(&mut self) {
        self.entrada_cedida = true;
    }

    /// Teclas que el programa ira recogiendo con `INPUT_OP_TECLA`, una por
    /// llamada. Los bytes son Latin-1 ya resueltos; para las que no tienen
    /// glifo, las constantes `TECLA_*` de `bmo_abi::syscalls::surface`.
    pub fn poner_teclas(&mut self, teclas: &[u8]) {
        self.teclas.extend_from_slice(teclas);
    }

    /// Teclas CRUDAS que el programa recogera con `INPUT_OP_EVENTO_TECLA`, una
    /// por llamada: `(scancode Set 1, pulsada)`.
    ///
    /// Es la cola del que quiere saber **que esta pulsado**, no que se
    /// escribio. Sembrar un `(sc, true)` sin su `(sc, false)` detras es
    /// legitimo y es justo el caso interesante: describe una tecla que se
    /// queda abajo.
    pub fn poner_eventos_tecla(&mut self, eventos: &[(u8, bool)]) {
        self.eventos_tecla.extend_from_slice(eventos);
    }

    /// Teclas repartidas EN EL TIEMPO: un lote por fotograma, entendiendo por
    /// fotograma cada `YIELD` que haga el programa.
    ///
    /// Es la diferencia entre probar un programa interactivo y probar una
    /// rafaga: con todo disponible de golpe, un bucle que drena el teclado ve
    /// la sesion entera en la primera vuelta y nunca llega a repintar entre
    /// pulsacion y pulsacion -- que es justo la conducta que se quiere mirar.
    ///
    /// El primer lote llega tras el primer `YIELD`; lo que deba estar ahi
    /// desde el principio va en [`Machine::poner_teclas`].
    pub fn poner_teclas_por_fotograma(&mut self, lotes: &[&[u8]]) {
        // Se guardan al reves para poder sacar el siguiente por el final, que
        // es O(1). El orden que ve el programa es el de la lista.
        for lote in lotes.iter().rev() {
            self.lotes.push(lote.to_vec());
        }
    }

    /// Suma muescas de rueda. Positivo = hacia arriba. Se acumulan hasta que
    /// alguien las lea, y leerlas las vacia.
    pub fn poner_rueda(&mut self, muescas: i32) {
        self.rueda += muescas;
        self.eventos_hid += muescas.unsigned_abs() as u64;
    }

    /// Coloca el puntero y sube el pulsometro de informes HID.
    pub fn poner_puntero(&mut self, x: u32, y: u32, botones: u8) {
        self.puntero = (x, y, botones);
        self.eventos_hid += 1;
    }

    /// Modificadores pulsados AHORA (`MOD_SHIFT`, `MOD_CTRL`...). Es estado: se
    /// queda puesto hasta que se cambie.
    pub fn poner_modificadores(&mut self, mascara: u8) {
        self.modificadores = mascara;
    }

    /// Muescas de rueda que quedan sin leer. Un programa que se olvida de
    /// drenarla las deja aqui, y la prueba puede decirlo.
    pub fn rueda_pendiente(&self) -> i32 {
        self.rueda
    }

    /// La PARTITURA: todo lo que el programa mando sonar, `(hz, ms)` en orden.
    ///
    /// Es lo unico que un banco de pruebas puede mirar de una libreria de
    /// musica, y es suficiente: si `LA4` en negra a 120 pulsos no son 440 Hz
    /// durante 425 ms, la libreria esta mal, suene el altavoz o no.
    pub fn partitura(&self) -> &[(u64, u64)] {
        &self.audio_partitura
    }

    /// Milisegundos totales que el programa dejo el altavoz sonando (sin contar
    /// los silencios). Sirve para comprobar articulacion y tempo de una frase
    /// entera sin enumerar nota por nota.
    pub fn audio_ms_sonando(&self) -> u64 {
        self.audio_partitura.iter().filter(|p| p.0 != 0).map(|p| p.1).sum()
    }

    /// Volumen que quedo puesto. 50 si nadie lo toco, igual que el crate.
    pub fn audio_volumen(&self) -> u64 {
        self.audio_volumen
    }

    /// Los volumenes pedidos, en orden. Una pieza con eco --forte y luego
    /// piano, que es como Vivaldi escribio el ritornello de "La primavera"--
    /// solo se puede comprobar mirando la SECUENCIA: el ultimo valor por si
    /// solo no distingue un eco de un volumen puesto una vez.
    pub fn volumenes(&self) -> &[u64] {
        &self.audio_volumenes
    }

    /// Despacho de la capability de sonido. Copia la semantica de
    /// `ring0/obj/audio.rs` -- sobre todo la que se nota: **el tope recorta**.
    fn audio_op(&mut self, op: u64, a0: u64, a1: u64) -> u64 {
        use bmo_abi::syscalls::surface::{
            DEVICE_SPEAKER, AUDIO_OP_DEVICES, AUDIO_OP_SILENCE, AUDIO_OP_BEEP, AUDIO_OP_VOLUME,
        };
        match op {
            // Solo el altavoz. HDA sigue sin existir, y decir aqui que si lo
            // hay seria darle al programa una respuesta que el Ryzen no da.
            AUDIO_OP_DEVICES => DEVICE_SPEAKER,
            AUDIO_OP_BEEP => {
                let hz = a0.min(20_000);
                let ms = a1.min(AUDIO_MAX_MS);
                self.audio_partitura.push((hz, ms));
                ms
            }
            AUDIO_OP_VOLUME => {
                self.audio_volumen = a0.min(100);
                self.audio_volumenes.push(self.audio_volumen);
                self.audio_volumen
            }
            AUDIO_OP_SILENCE => {
                self.audio_partitura.push((0, 0));
                0
            }
            _ => 0,
        }
    }

    /// Despacho de la capability de entrada. Copia la semantica de
    /// `ring0/obj/input.rs` -- sobre todo la que se nota: la rueda CONSUME.
    fn entrada_op(&mut self, op: u64) -> u64 {
        use bmo_abi::syscalls::surface::{
            INPUT_OP_EVENTOS, INPUT_OP_EVENTO_TECLA, INPUT_OP_MODIFICADORES, INPUT_OP_PUNTERO,
            INPUT_OP_RUEDA, INPUT_OP_TECLA,
        };
        match op {
            INPUT_OP_PUNTERO => {
                let (x, y, b) = self.puntero;
                ((x as u64) << 32) | ((y as u64) << 16) | b as u64
            }
            INPUT_OP_EVENTOS => self.eventos_hid,
            // `0x100 | byte` cuando hay una; `0` cuando no. El bit 8 es lo que
            // distingue "llego el byte 0" de "no llego nada".
            INPUT_OP_TECLA => {
                if self.teclas_cursor < self.teclas.len() {
                    let b = self.teclas[self.teclas_cursor];
                    self.teclas_cursor += 1;
                    0x100 | b as u64
                } else {
                    0
                }
            }
            INPUT_OP_MODIFICADORES => self.modificadores as u64,
            // La tecla CRUDA: `0x100 | (pulsada << 9) | scancode`, y `0` cuando
            // no queda ninguna. Cola aparte de la de caracteres, igual que en el
            // kernel: alli las dos se llenan del mismo informe HID y aqui las
            // dos las siembra la prueba.
            INPUT_OP_EVENTO_TECLA => {
                if self.eventos_tecla_cursor < self.eventos_tecla.len() {
                    let (sc, pulsada) = self.eventos_tecla[self.eventos_tecla_cursor];
                    self.eventos_tecla_cursor += 1;
                    let marca = if pulsada { 0x200 } else { 0 };
                    0x100 | marca | sc as u64
                } else {
                    0
                }
            }
            // * Consume. Dos lecturas seguidas sin girar dan cero la segunda.
            INPUT_OP_RUEDA => {
                let v = self.rueda;
                self.rueda = 0;
                v as i64 as u64
            }
            _ => 0,
        }
    }

    /// Abre o crea. Devuelve el handle (el indice + 1, para que 0 no sea uno
    /// valido) o 0 si no se pudo.
    fn archivo_abrir(&mut self, escribe: bool) -> u64 {
        let ruta = String::from_utf8_lossy(&self.ruta).into_owned();
        self.ruta.clear();
        self.abrir_ruta(ruta, escribe)
    }

    fn abrir_ruta(&mut self, ruta: String, escribe: bool) -> u64 {
        if ruta.is_empty() {
            return 0;
        }
        let datos = if escribe {
            Vec::new()
        } else {
            match self.archivos.get(&ruta) {
                Some(d) => d.clone(),
                // Abrir para leer lo que no existe FALLA. En el kernel es
                // `ERROR_NOT_THERE`; aqui es un handle nulo. Devolver uno vacio
                // haria que un `READ` de un fichero que falta pareciera un
                // fichero sin registros.
                None => return 0,
            }
        };
        self.abiertos.push(Abierto { ruta, datos, cursor: 0, escribe, vivo: true });
        self.abiertos.len() as u64
    }

    fn archivo_op(&mut self, handle: u64, op: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
        use bmo_abi::syscalls::surface::{
            ARCH_OP_CERRAR, ARCH_OP_ESCRIBIR, ARCH_OP_ESCRIBIR_DE, ARCH_OP_LEER, ARCH_OP_LEER_EN,
            ARCH_OP_LEER_LINEA,
            ARCH_OP_SALTAR, ARCH_OP_TAMANO,
        };
        let i = match (handle as usize).checked_sub(1) {
            Some(i) if i < self.abiertos.len() => i,
            _ => return 0,
        };
        if !self.abiertos[i].vivo {
            return 0;
        }
        match op {
            ARCH_OP_LEER if !self.abiertos[i].escribe => {
                let a = &mut self.abiertos[i];
                let mut w = [0u8; 8];
                let mut n = 0usize;
                while n < 7 && a.cursor < a.datos.len() {
                    w[n] = a.datos[a.cursor];
                    a.cursor += 1;
                    n += 1;
                }
                ((n as u64) << 56) | u64::from_le_bytes(w)
            }
            // Se para en el salto y lo consume. Modela EXACTAMENTE lo que
            // hace `ring0/archivo.rs`: si el emulador entregara los bytes de
            // detras del salto, un fichero de varios registros pasaria los
            // tests y daria basura en la maquina.
            ARCH_OP_LEER_LINEA if !self.abiertos[i].escribe => {
                let a = &mut self.abiertos[i];
                let mut w = [0u8; 8];
                let mut n = 0usize;
                let mut fin = 0u64;
                while n < 7 && a.cursor < a.datos.len() {
                    let b = a.datos[a.cursor];
                    a.cursor += 1;
                    if b == b'\n' {
                        fin = 1;
                        break;
                    }
                    w[n] = b;
                    n += 1;
                }
                (fin << 63) | ((n as u64) << 56) | u64::from_le_bytes(w)
            }
            ARCH_OP_ESCRIBIR if self.abiertos[i].escribe => {
                let n = (((arg0 >> 56) & 0xFF) as usize).min(7);
                let b = arg0.to_le_bytes();
                let a = &mut self.abiertos[i];
                for k in 0..n {
                    a.datos.push(b[k]);
                }
                n as u64
            }
            ARCH_OP_TAMANO => {
                let a = &self.abiertos[i];
                if a.escribe { a.datos.len() as u64 } else { (a.datos.len() - a.cursor) as u64 }
            }
            ARCH_OP_CERRAR => {
                let a = &mut self.abiertos[i];
                a.vivo = false;
                if a.escribe {
                    let (ruta, datos) = (a.ruta.clone(), a.datos.clone());
                    // El disco dice que no: no se escribe NADA y se contesta
                    // `0`. No se guarda un trozo -- un archivo a medias se
                    // parece demasiado a uno entero, que es la misma regla que
                    // sigue `close` en `ring0/archivo.rs`.
                    if self.fallo_al_guardar.contains(&ruta) {
                        return 0;
                    }
                    // * AQUI es donde llega al disco, y solo aqui. Igual que
                    // en el kernel.
                    self.archivos.insert(ruta, datos);
                }
                1
            }
            // ** `ARCH_OP_LEER_EN` -- EL BLOQUE DE GOLPE, y no siete bytes.
            //
            // Faltaba, y **contestaba exito con cero** por el `_ => 0` de abajo:
            // el cursor no se movia, `fread` devolvia 0 y `ftell` mentia. Dos
            // filas del banco estaban marcadas como pendientes por esto.
            //
            // El contrato es el del kernel y hay que copiarlo entero, porque es
            // donde vive lo interesante: **el destino no es un puntero, es una
            // CAPABILITY**. `arg0` es el handle del bloque de `KIND_MEMORIA`,
            // `arg1` el desplazamiento DENTRO de ese bloque y `arg2` cuantos
            // bytes. Comprobar que cabe es una resta contra lo que el kernel
            // entrego -- no hace falta ningun validador de punteros, que es la
            // infraestructura que aqui no existe.
            ARCH_OP_LEER_EN if !self.abiertos[i].escribe => {
                let bloque = match arg0.checked_sub(CAP_MEMORIA) {
                    Some(b) => b as usize,
                    None => return 0,
                };
                let base = match self.mem_bloques.get(bloque) {
                    Some(b) => *b,
                    None => return 0,
                };
                // El rango tiene que caber en lo entregado AL PROCESO, igual
                // que en `syscall.rs`. Un desbordamiento de la suma cae aqui.
                if arg1.checked_add(arg2).map_or(true, |fin| fin > self.mem_entregados) {
                    return 0;
                }
                let a = &self.abiertos[i];
                let quedan = a.datos.len().saturating_sub(a.cursor);
                let n = (arg2 as usize).min(quedan);
                let trozo: Vec<u8> = a.datos[a.cursor..a.cursor + n].to_vec();
                self.abiertos[i].cursor += n;
                for (k, b) in trozo.into_iter().enumerate() {
                    self.mem.insert(base + arg1 + k as u64, b);
                }
                n as u64
            }
            // ** `ARCH_OP_ESCRIBIR_DE` -- el espejo, con el mismo contrato: el
            // ORIGEN es una capability de memoria, no un puntero. Aqui el
            // buffer del archivo crece solo, que es lo que hace el kernel.
            ARCH_OP_ESCRIBIR_DE if self.abiertos[i].escribe => {
                let bloque = match arg0.checked_sub(CAP_MEMORIA) {
                    Some(b) => b as usize,
                    None => return 0,
                };
                let base = match self.mem_bloques.get(bloque) {
                    Some(b) => *b,
                    None => return 0,
                };
                if arg1.checked_add(arg2).map_or(true, |fin| fin > self.mem_entregados) {
                    return 0;
                }
                let n = arg2 as usize;
                let mut trozo: Vec<u8> = Vec::with_capacity(n);
                for k in 0..n {
                    trozo.push(*self.mem.get(&(base + arg1 + k as u64)).unwrap_or(&0));
                }
                self.abiertos[i].datos.extend_from_slice(&trozo);
                n as u64
            }
            // Mover el cursor. Se RECORTA al tamano, que es lo que hace el
            // kernel: un seek mas alla del final deja el cursor al final y lo
            // dice devolviendo donde quedo, no falla.
            ARCH_OP_SALTAR if !self.abiertos[i].escribe => {
                let a = &mut self.abiertos[i];
                let d = (arg0 as usize).min(a.datos.len());
                a.cursor = d;
                d as u64
            }
            // El modo manda: pedirle bytes a uno de escritura no es un error
            // de permisos, es una pregunta que ese objeto no responde. Se
            // enumeran para que caigan AQUI y no en el grito de abajo.
            ARCH_OP_LEER | ARCH_OP_LEER_LINEA | ARCH_OP_LEER_EN | ARCH_OP_SALTAR
            | ARCH_OP_ESCRIBIR | ARCH_OP_ESCRIBIR_DE => 0,
            // *** Y LO QUE NO CONOZCO SE GRITA.
            //
            // Aqui habia un `_ => 0`, y un cero por esta puerta significa
            // "exito, y el valor es cero": el programa cree que leyo, que
            // reservo, que sono. **Tres veces en un solo dia** mordio el mismo
            // patron --`TASK_OP_MEMORIA_PEDIR`, `KIND_AUDIO` y este mismo
            // `ARCH_OP_LEER_EN`-- y las tres veces el sintoma fue una fila
            // verde sobre algo que no existia.
            //
            // Un emulador que se calla ante lo que no conoce no es un modelo
            // incompleto: es un modelo que MIENTE, y miente en la direccion de
            // decir que todo va bien. Parar en seco convierte un dia de
            // depuracion en una linea.
            otra => panic!(
                "operacion 0x{otra:02X} sobre un handle de ARCHIVO no modelada en el emulador.                  Modelala en `emu.rs::archivo_op` con el contrato de                  `ring0/obj/archivo.rs`, o el test de arriba esta probando un                  sistema que no existe."
            ),
        }
    }

    /// `TASK_OP_MEMORIA_PEDIR` -- el bloque, o el motivo por el que no.
    ///
    /// Los dos rechazos que un programa puede provocar SOLO son los mismos que
    /// los del kernel y **con sus mismos codigos**: pedir cero o pasarse del
    /// tope (`0xE001`), y pedir una quinta vez (`0xE003`).
    ///
    /// Los otros dos no se modelan, y por el mismo motivo los dos: **aqui solo
    /// corre un proceso y la memoria es infinita**. `ERROR_NO_RAM` necesitaria
    /// RAM que fragmentar y `ERROR_NO_SLOT` necesitaria 16 procesos vivos a
    /// la vez. Fingirlos seria inventarse fallos que este emulador no puede
    /// reproducir de forma repetible -- y son exactamente el tipo de cosa que el
    /// eje 2 de la seccion FIDELIDAD dice que hay que probar en el Ryzen.
    fn memoria_pedir(&mut self, bytes: u64) -> Result<u64, u64> {
        const ERROR_TOO_BIG: u64 = 0xE001;
        const ERROR_TOO_MANY: u64 = 0xE003;

        if bytes == 0 || bytes > MEMORIA_MAX_BYTES {
            return Err(ERROR_TOO_BIG);
        }
        if self.mem_peticiones >= MEMORIA_MAX_PETICIONES {
            return Err(ERROR_TOO_MANY);
        }
        // Redondeo a paginas ARRIBA: pedir 1024 bytes entrega 4096, y el
        // siguiente bloque empieza detras de los 4096. Si esto redondeara hacia
        // abajo, dos bloques se solaparian y el emulador --memoria dispersa-- no
        // se quejaria nunca. Por eso el programa de prueba compara las bases.
        let paginas = (bytes + MEMORIA_PAGE - 1) / MEMORIA_PAGE;
        let base = self.mem_cursor;
        self.mem_cursor += paginas * MEMORIA_PAGE;
        self.mem_entregados += paginas * MEMORIA_PAGE;
        self.mem_peticiones += 1;
        self.mem_bloques.push(base);
        Ok(CAP_MEMORIA + (self.mem_bloques.len() as u64 - 1))
    }

    /// Las dos preguntas que responde un handle de memoria.
    fn memoria_op(&self, handle: u64, op: u64) -> u64 {
        use bmo_abi::syscalls::surface::{MEM_OP_BASE, MEM_OP_BYTES};
        let i = (handle - CAP_MEMORIA) as usize;
        match op {
            MEM_OP_BASE => self.mem_bloques.get(i).copied().unwrap_or(0),
            // Lo entregado al PROCESO entero, no a este bloque: es lo que
            // contesta el kernel, que lleva la cuenta por pid.
            MEM_OP_BYTES => self.mem_entregados,
            _ => 0,
        }
    }

    /// Cuantos bytes de `KIND_MEMORIA` se han entregado. Para que un test pueda
    /// comprobar lo que el programa pidio sin creerse lo que el programa dice.
    pub fn memoria_entregada(&self) -> u64 {
        self.mem_entregados
    }

    /// La puerta del kernel, modelada.
    pub(super) fn do_syscall(&mut self) {
        use bmo_abi::syscalls::surface::{
            CURRENT_TASK, NR_INVOKE, TASK_OP_ARCHIVO_ABRIR, TASK_OP_ARCHIVO_CREAR,
            TASK_OP_AUDIO_CLAIM, TASK_OP_AUDIO_RELEASE, TASK_OP_CONSOLE_READ,
            TASK_OP_CONSOLE_WRITE, TASK_OP_EXIT, TASK_OP_INPUT_CLAIM, TASK_OP_MEMORIA_PEDIR,
            TASK_OP_RUTA, TASK_OP_YIELD,
        };

        let call = ObservedSyscall {
            nr: self.regs[RAX],
            capability: self.regs[RDI],
            operation: self.regs[RSI],
            arg0: self.regs[RDX],
        };
        self.syscalls.push(call);

        assert_eq!(
            call.nr, NR_INVOKE as u64,
            "solo INVOKE cruza esta puerta (rax={:#x})",
            call.nr
        );

        if call.capability == CURRENT_TASK {
            match call.operation {
                op if op == TASK_OP_CONSOLE_WRITE => {
                    for i in 0..8 {
                        let b = ((call.arg0 >> (i * 8)) & 0xFF) as u8;
                        if b == 0 {
                            break; // NUL-stop: identico al kernel
                        }
                        self.console.push(b as char);
                    }
                }
                op if op == TASK_OP_EXIT => self.exited = true,
                // La ruta se acumula de 8 en 8 y se corta en el primer cero,
                // igual que en el kernel: un chunk final corto viene relleno.
                op if op == TASK_OP_RUTA => {
                    for i in 0..8 {
                        let b = ((call.arg0 >> (i * 8)) & 0xFF) as u8;
                        if b == 0 {
                            break;
                        }
                        self.ruta.push(b);
                    }
                }
                // La consola AL REVES: lo que el terminal habria tecleado. Se
                // siembra con `poner_entrada` y sale de 7 en 7, como en el
                // kernel. Es lo que hace testeable el `ACCEPT` de COBOL.
                op if op == TASK_OP_CONSOLE_READ => {
                    // ** NUNCA CRUZA UN SALTO DE LINEA, igual que el kernel.
                    //
                    // El porque entero esta en `ring0/obj/console.rs`,
                    // `read_entry`: sin esta regla, el que lee lineas pierde lo
                    // que venga detras del `\n` en el mismo paquete.
                    //
                    // [!] Que este emulador lo copiara MAL era lo de menos; lo
                    // grave habria sido copiarlo BIEN mientras el kernel lo
                    // hacia mal, porque entonces el banco de pruebas diria que
                    // si a un programa que en el Ryzen se equivoca.
                    let mut w = [0u8; 8];
                    let mut n = 0usize;
                    while n < 7 && self.entrada_cursor < self.entrada.len() {
                        let b = self.entrada[self.entrada_cursor];
                        self.entrada_cursor += 1;
                        w[n] = b;
                        n += 1;
                        if b == b'\n' {
                            break;
                        }
                    }
                    let v = ((n as u64) << 56) | u64::from_le_bytes(w);
                    self.finalizar_syscall(v);
                    return;
                }
                op if op == TASK_OP_ARCHIVO_ABRIR => {
                    let h = self.archivo_abrir(false);
                    self.finalizar_syscall(h);
                    return;
                }
                // `TASK_OP_MI_PAQUETE` -- la propia imagen, **sin ruta**.
                //
                // El programa no dice cual: el kernel lo sabe porque lo lanzo
                // el. Si el banco no puso ninguna, se contesta 0 -- que es lo
                // que le pasa a un binario que el kernel embebe y no viene de
                // ningun sitio.
                op if op == 0x25 => {
                    let h = match self.mi_paquete.clone() {
                        Some(clave) => self.abrir_ruta(clave, false),
                        None => 0,
                    };
                    self.finalizar_syscall(h);
                    return;
                }
                op if op == TASK_OP_ARCHIVO_CREAR => {
                    let h = self.archivo_abrir(true);
                    self.finalizar_syscall(h);
                    return;
                }
                // Reclamar la entrada. Sin `ceder_entrada()` devuelve 0, que
                // es el handle nulo: exactamente lo que ve un programa cuando
                // otro proceso la tiene tomada.
                op if op == TASK_OP_INPUT_CLAIM => {
                    let h = if self.entrada_cedida { CAP_ENTRADA } else { 0 };
                    self.finalizar_syscall(h);
                    return;
                }
                // Pedir memoria. Un rechazo NO es "handle 0": es un **codigo de
                // error en rax**, y esa es la diferencia que el emulador tiene
                // que respetar. `malloc` mira `rax` primero (`test eax,eax`), y
                // un modelo que devolviera siempre codigo 0 dejaria sin probar
                // justo la rama que decide si el tope se cumple.
                op if op == TASK_OP_MEMORIA_PEDIR => {
                    match self.memoria_pedir(call.arg0) {
                        Ok(h) => self.finalizar_syscall(h),
                        Err(code) => self.fallar_syscall(code),
                    }
                    return;
                }
                // Ceder el turno es el borde del fotograma: aqui es donde
                // "llega" lo que el usuario tecleo mientras tanto.
                // El SONIDO. Reclamarlo dos veces sin soltar tiene que fallar:
                // es la propiedad entera de un aparato exclusivo, y modelarla
                // aqui es lo que permite probarla sin encender el Ryzen.
                op if op == TASK_OP_AUDIO_CLAIM => {
                    let h = if self.audio_dueno { 0 } else { CAP_AUDIO };
                    self.audio_dueno = true;
                    self.finalizar_syscall(h);
                    return;
                }
                op if op == TASK_OP_AUDIO_RELEASE => {
                    if self.audio_dueno {
                        self.audio_dueno = false;
                        self.finalizar_syscall(0);
                    } else {
                        // No era suyo. El kernel contesta ERROR_BUSY, no OK:
                        // un "si" a quien no era dueno le haria creer que lo
                        // solto.
                        self.fallar_syscall(16);
                    }
                    return;
                }
                op if op == TASK_OP_YIELD => {
                    if let Some(lote) = self.lotes.pop() {
                        self.teclas.extend_from_slice(&lote);
                    }
                }
                _ => {}
            }
        } else if call.capability == CAP_AUDIO {
            // [!] Y **solo si sigue siendo suyo**. Un handle que funciona
            // despues de soltarlo es un uso-despues-de-liberar con otro nombre,
            // y en el kernel de verdad no resuelve porque la generacion cambio.
            // Si el emulador no modelara esto, la prueba que lo comprueba
            // pasaria con el kernel roto.
            if !self.audio_dueno {
                self.fallar_syscall(2); // ERROR_INVALID_HANDLE
                return;
            }
            let v = self.audio_op(call.operation, call.arg0, self.regs[R10]);
            self.finalizar_syscall(v);
            return;
        } else if call.capability == CAP_ENTRADA {
            let v = self.entrada_op(call.operation);
            self.finalizar_syscall(v);
            return;
        } else if call.capability >= CAP_MEMORIA {
            let v = self.memoria_op(call.capability, call.operation);
            self.finalizar_syscall(v);
            return;
        } else if call.capability != 0 {
            // Cualquier otro handle: aqui solo existen los de archivo. El
            // emulador no modela la pantalla ni el raton porque ningun codigo
            // EMITIDO los toca -- los usa el compositor, que es Rust normal.
            let v =
                self.archivo_op(call.capability, call.operation, call.arg0, self.regs[R10], self.regs[crate::x86::R8 as usize]);
            self.finalizar_syscall(v);
            return;
        }

        self.finalizar_syscall(0);
    }

    /// El epilogo comun de toda llamada.
    ///
    /// * El valor vuelve en **rdx**, no en rax. `BmoStatus` es
    /// `{code, flags, value}`: rax trae el codigo y las banderas, rdx trae el
    /// valor. Se puede leer en el stub de `userland::syscall`.
    ///
    /// Esto estaba MAL modelado: el emulador ponia `rax = 0` y no tocaba rdx,
    /// asi que ahi seguia el argumento de entrada. Por eso `console::read_line`
    /// --la puerta de `ACCEPT`-- no tiene ni un test: en el emulador habria
    /// visto siempre "no hay nada" y girado para siempre. El emulador mentia
    /// sobre la puerta, que es justo lo que no puede hacer.
    fn finalizar_syscall(&mut self, valor: u64) {
        // El silicio destruye estos dos.
        self.regs[RCX] = POISON;
        self.regs[R11] = POISON;
        self.regs[RAX] = 0; // code = 0 (ok), flags = 0
        self.regs[RDX] = valor;
    }

    /// El epilogo de una llamada que el kernel RECHAZA: codigo en `rax` y
    /// **valor envenenado** en `rdx`.
    ///
    /// Lo segundo es a proposito. Un programa que se salta la comprobacion del
    /// codigo y usa el valor igual tiene que estropearse aqui, en un test, y no
    /// en el Ryzen -- donde `rdx` traeria lo que hubiera quedado y funcionaria
    /// por casualidad las primeras veces.
    fn fallar_syscall(&mut self, code: u64) {
        self.regs[RCX] = POISON;
        self.regs[R11] = POISON;
        self.regs[RAX] = code;
        self.regs[RDX] = POISON;
    }
}
