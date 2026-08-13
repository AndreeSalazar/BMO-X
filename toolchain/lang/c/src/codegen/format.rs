//! **`printf`**: the only part of the compiler that emits an INTERPRETER.
//!
//! === Why this is a file of its own, and it is the clearest case here ===
//!
//! Everything else BMO C emits is a translation: an expression in the source
//! becomes instructions that compute it. `printf` is not. `printf` has **two
//! modes, and they are two different programs**:
//!
//! - If the format is known at compile time --the normal case-- it is walked
//!   HERE and the resolved bytes are emitted: `write_const` puts the text in as
//!   immediates inside the instructions themselves. There is no interpreter.
//! - If the format is a VARIABLE, the template does not exist until the program
//!   runs, so the loop that walks it has to be emitted.
//!
//! So this file holds a formatter written twice, once in Rust and once in
//! machine code, **and the two have to agree**. That is the property that earns
//! it a file: it is the only subsystem in the codegen with an obligation to be
//! consistent with itself.
//!
//! [!] And that is why the `printf` tests EXECUTE. A formatter that produces
//! wrong digits looks perfectly healthy in a byte dump.

use super::*;

impl Codegen {
    /// `printf(fmt, args...)` -- la L2 de C sobre la libreria de formateo.
    ///
    /// Antes esto empujaba los argumentos a la pila y llamaba a un
    /// `bmo_printf` **importado de `userland_ring3`**: un simbolo que en BMO
    /// nadie resuelve, porque no hay enlazado dinamico de una libc. El
    /// programa compilaba y luego saltaba a una direccion sin parchear.
    ///
    /// Ahora el formateo se emite EN LINEA: cada trozo literal baja por la
    /// puerta de consola y cada conversion evalua su argumento y llama al
    /// emisor correspondiente de `bmo_lower::fmt`. Sin runtime, sin
    /// importaciones, sin dependencias del cargador.
    ///
    /// Lo especifico de C --que significa `%d`, que `%x` va en minusculas,
    /// que `%%` es un porcentaje-- se decide aqui. La libreria solo sabe
    /// convertir un numero en digitos.
    /// **La superficie de biblioteca que se emite en linea.**
    ///
    /// Devuelve `Some(())` si `name` era una de ellas y ya se emitio.
    ///
    /// * Cada una carga sus argumentos en registros y llama al emisor de L1.
    /// El orden importa: se evalua el ultimo primero y se apila, porque
    /// evaluar el segundo argumento puede machacar el registro donde estaba el
    /// primero -- un `memcpy(a, f(x), n)` con `f` llamando a otra cosa es el
    /// caso que lo destapa, y no se destapa en las pruebas faciles.
    pub(super) fn emitir_biblioteca(&mut self, name: &str, args: &[Expr]) -> Option<()> {
        use bmo_lower::memoria;
        use bmo_lower::x86;
        // ** UNA DEFINICION PROPIA GANA A LA DE BIBLIOTECA, y solo para la
        // familia de la memoria.
        //
        // Es lo que deja existir al monton de Ring 3: `<stdlib.h>` define
        // `malloc` y `free` en C de verdad --sobre UN bloque de
        // `KIND_MEMORIA`-- y si esta emision se adelantara, ese cuerpo no se
        // llamaria nunca. Es el mismo trato que ya tiene `printf` cuando el
        // formato no es literal: el codegen se aparta y llama al de C.
        //
        // Acotado a estos dos a proposito. La regla general --"cualquier
        // funcion definida en la unidad gana"-- es la correcta en C y
        // probablemente sea lo siguiente, pero cambiarla de golpe altera
        // silenciosamente que emite un programa que redefina `abs` o `strlen`,
        // y eso pide su propia tanda con sus filas.
        if matches!(name, "malloc" | "free") && self.known_functions.contains(name) {
            return None;
        }
        match (name, args.len()) {
            // * `memcpy` YA NO ESTA AQUI, y su ausencia es el cambio.
            //
            // Cae por el camino de llamada normal y su cuerpo lo pone
            // `SINTETIZABLES`: emitido UNA vez, alcanzado con `call rel32`. Se
            // eligio este y no otro porque es el que mas se repite --por
            // `memcpy` pasa el blit de cada fotograma-- y porque su cuerpo no
            // tiene estado, asi que compartirlo no cambia lo que hace.
            //
            // `memmove` se queda en linea, y no por simetria descuidada: se
            // llama poco, y tocar los dos a la vez habria mezclado en un solo
            // cambio la conversion y un riesgo que no hacia falta correr.
            //
            // [!] Y de paso queda anotado lo que se vio al mirar esto: este arm
            // le da a `memmove` el MISMO `copiar`, que avanza de principio a
            // fin. Para solapamiento con `dst > src` eso corrompe --es
            // exactamente lo que `memmove` promete y `memcpy` no--, asi que
            // `memmove` hoy es un `memcpy` con otro nombre. **No lo arregla
            // este cambio** y esta sin arreglar, dicho aqui para que no se
            // cuente como hecho.
            ("memmove", 3) => {
                self.cargar_tres(args, x86::RDI, x86::RSI, x86::RCX);
                // ** `mover`, no `copiar`. Antes compartia emision con `memcpy`,
                // o sea que era un `memcpy` con otro nombre: copiaba siempre de
                // frente y corrompia el unico caso para el que `memmove`
                // existe. Ver `memoria::mover`.
                memoria::mover(&mut self.code);
                // Devuelve el destino, que sigue en la pila porque el bucle se
                // llevo rdi por delante.
                self.soltar_tres();
                Some(())
            }
            // `strncmp` y `memcmp` comparten emision y se distinguen en UN bool:
            // si el terminador corta o no. Ver `memoria::comparar_n`.
            ("abs", 1) => {
                self.emit_expr(&args[0]);
                memoria::absoluto(&mut self.code);
                Some(())
            }
            // -- malloc / free ----------------------------------------
            //
            // * Cada `malloc` es **una peticion al kernel**, no un trozo de un
            // monton. Y eso NO es un atajo: es lo que hay hoy, dicho como es.
            //
            // El kernel entrega bloques enteros y no sabe repartirlos -- a
            // proposito, porque el asignador es politica y la politica vive en
            // Ring 3. Un monton de verdad (bump + listas libres) se escribe
            // encima de `bmo::Memoria`, y ese es el siguiente paso.
            //
            // **Limite declarado**: el kernel acepta CUATRO peticiones por
            // proceso, porque no hay forma de devolver memoria y ese numero es
            // el de fugas posibles. Un quinto `malloc` devuelve **0**, que es
            // lo que un programa de C ya sabe comprobar. Falla pronto y con
            // un valor que significa algo, en vez de agotar la RAM callando.
            //
            // Para el caso que motivo todo esto --DOOM pide su bloque UNA vez y
            // se lo administra con `Z_Zone`-- esto es exactamente suficiente.
            // * Los dos saltos de aqui van por ETIQUETA, y no es cosmetica.
            //
            // La primera version los emitio con desplazamientos contados a
            // mano, y el primero se quedo **seis bytes corto**: `jnz +0x1D`
            // cuando el camino hasta el `xor rax,rax` mide 35. O sea que
            // cuando el kernel RECHAZABA la peticion --la quinta, o una
            // demasiado grande-- el salto caia **dentro** del `jnz` siguiente y
            // el CPU seguia por la mitad de una instruccion.
            //
            // Y el detalle que lo hace peor: la rama buena estaba bien. Un
            // `malloc` que funciona cuatro veces y descarrila a la quinta pasa
            // por "el tope se cumple" en cualquier prueba que no llegue a la
            // quinta. Lo cazo el emulador con `opcode 0x05 no emitido por BMO`
            // -- que es la firma de haber aterrizado a media instruccion.
            //
            // Contar bytes a mano es escribir un enlazador en la cabeza cada
            // vez que alguien anade una instruccion en medio. Las etiquetas ya
            // estaban aqui; solo habia que usarlas.
            // ** `bmo_bloque_pedir(bytes)` es LA MISMA EMISION con otro nombre,
            // y ese nombre es lo que permite escribir un asignador.
            //
            // `malloc` de C tiene que poder ser un monton escrito en Ring 3, y
            // un monton necesita pedirle al kernel el bloque grande **sin pasar
            // por `malloc`**, que es justo lo que el estaria implementando. Con
            // los dos nombres, la peticion cruda al kernel deja de estar
            // escondida detras de la palabra `malloc`:
            //
            //     malloc              lo que un programa de C llama
            //     bmo_bloque_pedir    una peticion a `KIND_MEMORIA`, y nada mas
            //
            // Que sean el mismo brazo no es pereza: **es la afirmacion de que
            // hoy `malloc` sin `<stdlib.h>` ES la peticion cruda**, con su tope
            // de cuatro. `examples/memoria_C.c` prueba ese contrato y no incluye
            // nada, asi que lo sigue probando.
            //
            // [!] Y va SIN los dos guiones bajos aunque sea del sistema: un
            // nombre que empieza por `__` lo desvia el codegen a la tabla de
            // intrinsecos de sem-asm --antes de llegar aqui-- y falla con
            // "no existe en la tabla". Ese prefijo esta tomado.
            ("malloc", 1) | ("bmo_bloque_pedir", 1) => {
                use bmo_sem_asm::x86_64::Reg;
                let sin_bloque = self.fresh_label();
                let fin = self.fresh_label();
                self.emit_expr(&args[0]);                          // rax = bytes
                self.emit_asm(|a| { a.mov_reg(Reg::Rdx, Reg::Rax).unwrap(); });
                // rdi = CURRENT_TASK, rsi = OP_MEMORIA_PEDIR
                self.emit_asm(|a| { a.mov_imm64(Reg::Rdi, 0xFFFF_FFFF_FFFF_FFFE).unwrap(); });
                self.emit_asm(|a| { a.mov_imm64(Reg::Rsi, 0x15).unwrap(); });
                self.code.extend_from_slice(&[0xB8, 0, 0, 0, 0]);  // mov eax, NR_INVOKE(0)
                self.emit_call_to_syscall_stub();
                // El handle vuelve en rdx (`value`); rax lleva el codigo.
                // Si el codigo no es 0, no hay bloque: se devuelve 0.
                self.code.extend_from_slice(&[0x85, 0xC0]);        // test eax, eax
                self.emit_jnz_reloc(sin_bloque);
                // * El handle a la pila ANTES de la segunda llamada, que pisa
                // `rdx`. Es el dato que `fread` necesita y que hasta ahora se
                // perdia justo aqui: se usaba para pedir la base y se tiraba.
                self.code.push(0x52);                              // push rdx
                // Segunda llamada: MEM_OP_BASE sobre el handle.
                self.emit_asm(|a| { a.mov_reg(Reg::Rdi, Reg::Rdx).unwrap(); });
                self.emit_asm(|a| { a.mov_imm64(Reg::Rsi, 0x01).unwrap(); });
                self.code.extend_from_slice(&[0x48, 0x31, 0xD2]);  // xor rdx, rdx
                self.code.extend_from_slice(&[0xB8, 0, 0, 0, 0]);  // mov eax, NR_INVOKE
                self.emit_call_to_syscall_stub();
                // El `pop` va ANTES del test, y eso no es estilo: por el camino
                // de fallo se salta a `sin_bloque`, y saltar con algo aun en la
                // pila la descuadra para el resto de la funcion.
                self.code.extend_from_slice(&[0x41, 0x58]);        // pop r8 (el handle)
                self.code.extend_from_slice(&[0x85, 0xC0]);        // test eax, eax
                self.emit_jnz_reloc(sin_bloque);
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]);  // mov rax, rdx (la base)
                // * PUBLICAR EL BLOQUE. Sin esto `fread` no puede existir.
                //
                // El kernel solo acepta escribir dentro de un bloque que el
                // concedio, y para pedirselo hay que darle SU handle y un
                // desplazamiento. `malloc` es el unico que tiene las dos cosas
                // --el handle vino en la primera llamada, la base en la segunda--
                // y hasta ahora tiraba el handle en cuanto sacaba la base.
                //
                // Se guardan en dos globales **solo si el programa las
                // declaro** (las trae `<bmo/archivo.h>`). Un programa que no
                // lee ficheros no paga ni un byte por esto, que es la razon de
                // preguntar por el nombre en vez de emitirlas siempre.
                self.publicar_bloque();
                self.emit_jmp_reloc(fin);
                self.resolve_label(sin_bloque);
                self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);  // xor rax, rax
                self.resolve_label(fin);
                Some(())
            }
            // `free` NO devuelve nada al kernel -- no hay forma, y decirlo aqui
            // vale mas que emitir una llamada que no haria nada. El bloque vive
            // hasta que el proceso muere, y entonces se destruye su espacio de
            // direcciones entero.
            //
            // Se acepta porque el codigo ajeno lo llama y quitarlo a mano de
            // 35.000 lineas no es una opcion. Evalua su argumento, por si tiene
            // efectos secundarios.
            ("free", 1) => {
                self.emit_expr(&args[0]);
                self.emit_xor_eax();
                Some(())
            }
            _ => None,
        }
    }

    /// Tres argumentos a tres registros, evaluando de derecha a izquierda.
    ///
    /// * Los tres se dejan EN LA PILA y los registros se cargan **leyendo**,
    /// no sacando. La primera version los sacaba con `pop` y apilaba el
    /// destino dos veces para poder devolverlo -- y eso desalineaba los tres
    /// `pop`: `memset` acababa con el valor de relleno en el registro del
    /// contador. Salio como `-16,-16,-16` donde tenia que salir `65,65,65`.
    ///
    /// Leyendo con desplazamiento no hay orden que cuadrar: cada argumento
    /// esta donde se puso. Y quien llama limpia con [`Self::soltar_tres`], que
    /// es lo que faltaba tambien -- la version de `pop` dejaba dos valores
    /// vivos en la pila por cada `memcpy`, y eso no se ve hasta que un bucle
    /// hace mil.
    pub(super) fn cargar_tres(&mut self, args: &[Expr], r0: u8, r1: u8, r2: u8) {
        self.emit_expr(&args[2]);
        self.code.push(0x50); // push n        -> [rsp+16]
        self.emit_expr(&args[1]);
        self.code.push(0x50); // push src      -> [rsp+8]
        self.emit_expr(&args[0]);
        self.code.push(0x50); // push dst      -> [rsp]
        self.mov_desde_pila(r0, 0);
        self.mov_desde_pila(r1, 8);
        self.mov_desde_pila(r2, 16);
    }

    /// Recupera el destino en `rax` y tira los otros dos. Cierra a
    /// [`Self::cargar_tres`].
    pub(super) fn soltar_tres(&mut self) {
        self.code.push(0x58);                               // pop rax (dst)
        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 16]); // add rsp, 16
    }

    /// `mov <r64>, [rsp+disp8]`.
    pub(super) fn mov_desde_pila(&mut self, reg: u8, disp: u8) {
        self.code.push(0x48 | if reg >= 8 { 0x04 } else { 0 }); // REX.W (+R)
        self.code.push(0x8B);
        self.code.push(0x44 | ((reg & 7) << 3)); // modrm: [SIB + disp8]
        self.code.push(0x24);                    // SIB: base = rsp
        self.code.push(disp);
    }

    pub(super) fn emit_printf_variadic(&mut self, args: &[Expr]) {
        let Expr::StringLit(format) = &args[0] else {
            // * EL FORMATO NO ES UN LITERAL -> al formateador de EJECUCION.
            //
            // Esto era un error hasta hoy, y era el sitio exacto donde se
            // paraba el unity build de DOOM: `printf(message, demoversion,
            // ...)` en `g_game.c`. El emisor de aqui recorre la plantilla **al
            // compilar** --por eso los literales caben dentro de las
            // instrucciones y no hace falta libreria-- y eso no se puede hacer
            // con una plantilla que llega en un puntero.
            //
            // La respuesta no es meterle un interprete al codegen: es llamar al
            // formateador que ya existe escrito en C, en `<stdio.h>`. Ahi se
            // lee, se corrige y **tiene anchura de verdad**, que este emisor no
            // tiene.
            self.emit_printf_en_ejecucion(&args[0], &args[1..]);
            return;
        };
        let format = format.clone();
        let va_args: Vec<Expr> = args[1..].to_vec();
        let mut next_arg = 0usize;
        let mut literal: Vec<u8> = Vec::new();

        // * **TODOS los argumentos se evaluan ANTES de escribir un solo byte.**
        //
        // Antes no: el emisor recorria la plantilla y evaluaba cada argumento
        // al llegar a su `%`, intercalado con la salida de los literales. Con
        // argumentos sin efectos daba igual, pero `printf("[%d]", f())` con `f`
        // imprimiendo sacaba `[` **antes** que lo de `f` -- y en C estandar
        // todos los argumentos se evaluan antes de entrar en la llamada.
        //
        // Lo destapo la matriz de C++ al probar RAII: un destructor que
        // imprime es justo un argumento con efectos. Es la clase de diferencia
        // que solo aparece al portar codigo de otro, y entonces ya no se sabe
        // de donde viene.
        //
        // Se guardan en la PILA y no en ranuras del marco a proposito: los
        // ayudantes de `bmo_lower::fmt` y `console` estan **equilibrados en
        // rsp** (cada `sub rsp` tiene su `add rsp`), asi que un offset
        // relativo a rsp sigue valiendo entre una conversion y la siguiente.
        // Y asi no hay que reservar sitio en el prologo para algo que solo
        // vive dentro de un `printf`.
        let n = va_args.len();
        for a in &va_args {
            self.emit_expr(a);
            self.code.push(0x50); // push rax
        }

        let chars: Vec<char> = format.chars().collect();
        let mut i = 0usize;
        while i < chars.len() {
            if chars[i] != '%' {
                let mut buf = [0u8; 4];
                literal.extend_from_slice(chars[i].encode_utf8(&mut buf).as_bytes());
                i += 1;
                continue;
            }

            // * BANDERAS, ANCHURA y PRECISION -- se leen y NO se aplican.
            //
            // `printf("block:%p size:%7i tag:%3i\n", ...)` de `z_zone.c`. Antes
            // el `7` se tomaba por la conversion y el mensaje decia
            // *"'%7' aun no se compila"*, acusando a un digito que es la
            // anchura.
            //
            // [!] Y esto es una APROXIMACION, dicha aqui y no escondida: el
            // numero sale bien y **sin rellenar**, asi que una tabla alineada
            // sale desalineada. Alinear pide saber cuantos caracteres va a
            // ocupar el numero ANTES de escribirlo, y los formateadores
            // sintetizados no lo devuelven. Se acepta porque la alternativa es
            // no compilar el fichero, y porque el valor -- que es lo que un
            // programa hace con el -- es correcto.
            let mut j = i + 1;
            while j < chars.len() && matches!(chars[j], '-' | '+' | ' ' | '#' | '0') {
                j += 1;
            }
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j < chars.len() && chars[j] == '.' {
                j += 1;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
            }
            // Saltar los modificadores de longitud: en BMO todo entero viaja
            // en 64 bits, asi que `%ld` y `%d` producen lo mismo.
            while j < chars.len() && matches!(chars[j], 'l' | 'h' | 'z' | 'j' | 't') {
                j += 1;
            }
            let Some(&conversion) = chars.get(j) else {
                self.errors
                    .push("'%' al final del formato de printf".to_string());
                return;
            };

            if conversion == '%' {
                literal.push(b'%');
                i = j + 1;
                continue;
            }

            // Todo lo literal acumulado sale ANTES de la conversion.
            if !literal.is_empty() {
                bmo_lower::console::write_const(&mut self.code, &literal);
                literal.clear();
            }

            if next_arg >= n {
                self.errors.push(format!(
                    "printf: '%{conversion}' no tiene argumento correspondiente"
                ));
                return;
            }
            // El valor ya esta calculado en la pila: el primero empujado es el
            // que queda mas arriba, asi que el i-esimo esta en `n-1-i`.
            self.emit_cargar_de_pila(n - 1 - next_arg);
            next_arg += 1;

            // * Aqui estaba el formateador ENTERO, en linea, en cada `%`.
            //
            // Un `printf("%d %d %d")` se llevaba tres copias del mismo
            // conversor de entero a decimal, y no habia programa que no
            // pagara eso: `printf` es la funcion que todos usan. Ahora es un
            // `call` de cinco bytes al cuerpo que puso `SINTETIZABLES`.
            match conversion {
                'd' | 'i' => self.emit_call_sintetizada("__bmo_fmt_i64"),
                'u' => self.emit_call_sintetizada("__bmo_fmt_u64_dec"),
                // `%p` es una direccion, y una direccion se lee en hexadecimal:
                // es el MISMO conversor que `%x`, no uno nuevo. Se escribe como
                // una fila propia --y no como `'x' | 'p'`-- porque el dia que
                // lleve el `0x` delante, ese dia cambia aqui y solo aqui.
                'x' | 'p' => self.emit_call_sintetizada("__bmo_fmt_u64_hex"),
                'c' => self.emit_call_sintetizada("__bmo_fmt_char"),
                's' => self.emit_call_sintetizada("__bmo_fmt_cstr"),
                other => {
                    self.errors.push(format!(
                        "printf: '%{other}' aun no se compila (se compilan \
                         %d %i %u %x %c %s %%; los flotantes necesitan la ruta SSE)"
                    ));
                    return;
                }
            }
            i = j + 1;
        }

        if !literal.is_empty() {
            bmo_lower::console::write_const(&mut self.code, &literal);
        }

        // Devolver la pila. Va DESPUES del ultimo literal, no antes: entre
        // medias todavia se leen ranuras relativas a rsp.
        self.emit_soltar_pila(n);

        if next_arg < n {
            self.errors.push(format!(
                "printf: sobran {} argumento(s) para el formato dado",
                n - next_arg
            ));
        }
    }

    /// `printf(fmt, ...)` cuando `fmt` **no se sabe hasta ejecutar**.
    ///
    /// # La pieza que lo hace corto: la pila YA es un `va_list`
    ///
    /// BMO C pasa los argumentos por la pila y seguidos, asi que empujarlos en
    /// orden INVERSO deja en memoria, de menor a mayor direccion, exactamente
    /// `arg0, arg1, arg2...` -- que es la forma que tiene una lista variadica
    /// aqui (ver `<stdarg.h>`). O sea que el `va_list` que hay que pasarle al
    /// formateador **es `rsp`**, sin copiar nada a ningun sitio.
    ///
    /// [!] Y por eso se empujan al reves que en la ruta de formato literal, que
    /// los quiere en el otro orden porque los lee por indice con
    /// [`Self::emit_cargar_de_pila`]. Dos ordenes distintos en la misma
    /// funcion: cada uno es el que necesita su lector.
    ///
    /// # Lo que hace falta que exista
    ///
    /// `bmo_formatear`, que **no** es sintetizada: es C de verdad y vive en
    /// `<stdio.h>`. Si no esta incluido no se compila un `call` a la nada, se
    /// dice cual es la linea que falta -- que es la diferencia entre un error
    /// que se arregla en diez segundos y uno que manda a leer el codegen.
    pub(super) fn emit_printf_en_ejecucion(&mut self, fmt: &Expr, va_args: &[Expr]) {
        if !self.known_functions.contains("bmo_formatear") {
            self.errors.push(
                "printf con el formato calculado en ejecucion necesita el formateador: \
                 anade #include <stdio.h> (ahi vive `bmo_formatear`, y de paso trae \
                 snprintf, sprintf y la familia v*)"
                    .to_string(),
            );
            return;
        }
        let n = va_args.len();
        for a in va_args.iter().rev() {
            self.emit_expr(a);
            self.code.push(0x50); // push rax
        }
        // rax = rsp -> el va_list. Se captura ANTES de empujar los argumentos
        // de la llamada, que es lo unico que hay que no equivocar aqui.
        self.code.extend_from_slice(&[0x48, 0x89, 0xE0]); // mov rax, rsp
        self.code.push(0x50); // push rax        -> ap    (4o parametro)
        self.emit_expr(fmt);
        self.code.push(0x50); // push fmt        -> (3o)
        self.emit_asm(|a| { a.mov_imm64(bmo_sem_asm::x86_64::Reg::Rax, 0).unwrap(); });
        self.code.push(0x50); // push 0          -> lim   (2o)
        self.code.push(0x50); // push 0          -> dst   (1o), y 0 = a la consola
        self.code.extend_from_slice(&[0xE8]);
        self.call_relocs.push(CallReloc {
            offset: self.code.len(),
            target: "bmo_formatear".to_string(),
        });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
        // Los cuatro de la llamada mas los variadicos que quedan debajo.
        self.emit_soltar_pila(4 + n);
    }

    /// Empuja un argumento de coma flotante como el PATRON DE BITS que el
    /// callee espera encontrar en su ranura.
    ///
    /// `estrecho` = el parametro es `float` (cuatro bytes) y no `double`. Esa
    /// distincion no es cosmetica: el callee lee su ranura con `movss` o con
    /// `movsd` segun lo que declaro, y meter ocho bytes donde va a leer cuatro
    /// le da la mitad baja de la mantisa como si fuera el numero entero.
    ///
    /// La conversion desde un argumento ENTERO sale gratis: `emit_fexpr` ya
    /// sabe subir un entero a `xmm0` con `cvtsi2sd`, que es lo que C manda
    /// hacer con `fabs(3)`.
    pub(super) fn emit_empuja_flotante(&mut self, arg: &Expr, estrecho: bool) {
        self.emit_fexpr(arg); // el valor, en xmm0, como double
        if estrecho {
            self.code.extend_from_slice(&[0xF2, 0x0F, 0x5A, 0xC0]); // cvtsd2ss xmm0,xmm0
            self.code.extend_from_slice(&[0x66, 0x0F, 0x7E, 0xC0]); // movd eax, xmm0
        } else {
            self.code.extend_from_slice(&[0x66, 0x48, 0x0F, 0x7E, 0xC0]); // movq rax, xmm0
        }
        self.code.push(0x50); // push rax
    }

    /// `mov rax, [rsp + slot*8]` -- lee un argumento ya calculado.
    pub(super) fn emit_cargar_de_pila(&mut self, slot: usize) {
        let disp = (slot * 8) as i64;
        if disp <= 127 {
            // 48 8B 44 24 disp8
            self.code.extend_from_slice(&[0x48, 0x8B, 0x44, 0x24, disp as u8]);
        } else {
            // 48 8B 84 24 disp32
            self.code.extend_from_slice(&[0x48, 0x8B, 0x84, 0x24]);
            self.code.extend_from_slice(&(disp as u32).to_le_bytes());
        }
    }

    /// `add rsp, ranuras*8` -- suelta los argumentos guardados.
    pub(super) fn emit_soltar_pila(&mut self, ranuras: usize) {
        if ranuras == 0 { return; }
        let bytes = (ranuras * 8) as i64;
        if bytes <= 127 {
            self.code.extend_from_slice(&[0x48, 0x83, 0xC4, bytes as u8]);
        } else {
            self.code.extend_from_slice(&[0x48, 0x81, 0xC4]);
            self.code.extend_from_slice(&(bytes as u32).to_le_bytes());
        }
    }
    /// `printf("literal")` -- la L2 de C sobre la puerta generica (L1).
    ///
    /// Lo especifico de C que se resuelve AQUI y en ningun otro sitio: que la
    /// cadena es un literal ya escapado por el lexer y que `\n` va pegado al
    /// final. Los bytes resultantes se los entrega a `bmo_lower::console`,
    /// que no sabe que existe C.
    ///
    /// Antes esto emitia `lea rdi,[str]; mov esi,len; syscall 0x1F0`: un
    /// numero plano que el kernel no despacha, pasando ademas un PUNTERO,
    /// que la superficie congelada rechaza por diseno. No imprimia nada en
    /// hardware. La cadena ya no necesita vivir en `.rodata`: viaja como
    /// inmediatos dentro de las propias instrucciones.
    pub(super) fn emit_printf(&mut self, s: &str, newline: bool) {
        let text = if newline { let mut t = s.to_string(); t.push('\n'); t } else { s.to_string() };
        bmo_lower::console::write_const(&mut self.code, text.as_bytes());
    }
}
