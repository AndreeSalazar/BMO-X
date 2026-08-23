//! **INTRINSICS AND THE KERNEL DOOR**: what is not translated is invoked.
//!
//! === Why this is a file of its own ===
//!
//! An intrinsic is not a C function: it is a NAME the compiler recognises and
//! substitutes with specific instructions, with no body to compile and no call
//! to link. `__rdtsc()` is two instructions; `bmo_valor(..)` is loading five
//! registers and crossing the door.
//!
//! That puts them on the boundary of the language: **they are the system
//! surface as seen from C**, and whoever touches them is changing the contract
//! with the kernel, not the semantics of the language. Having them in their own
//! file makes that difference obvious the moment you open it.
//!
//! [!] And a trap already paid for lives here: the door stub is a CALLABLE
//! (`syscall; ret`). Inlining it removes the `call` and **keeps the `ret`**: the
//! whole function returns as soon as the syscall comes back, and everything
//! behind it never runs.

use super::*;

impl Codegen {
    /// LA FUSION sem-asm<->C: emite un intrinseco de la tabla.
    /// Evalua cada argumento, lo apila, y lo vuelca al registro que dicta
    /// la tabla justo antes de los bytes de la instruccion. Bytes EXACTOS,
    /// sin caja negra: si el nombre o la aridad no cuadran -> error, no adivina.
    pub(super) fn emit_intrinsic(&mut self, name: &str, args: &[Expr]) {
        // * `__va_arg(i)` -- el argumento variadico numero `i`, contando desde 0
        // despues de los que tienen nombre.
        //
        // No sale de la tabla de sem-asm porque no es una instruccion del CPU:
        // es aritmetica sobre el marco de pila, y depende de CUANTOS parametros
        // con nombre tiene la funcion que lo pregunta.
        //
        // Y es aritmetica y no ABI porque BMO C pasa los argumentos **por la
        // pila**, de derecha a izquierda. En la convencion de registros de
        // SysV esto obligaria a volcar seis registros en el prologo y a llevar
        // dos cursores (registros y pila); aqui los argumentos ya estan
        // seguidos en memoria y el numero `i` es un desplazamiento. La
        // convencion mas vieja resulto ser la que hace los varargs triviales.
        //
        // El indice es de EJECUCION, no una constante: sin eso no se puede
        // recorrer los argumentos en un bucle, que es justo lo que hace un
        // `vsprintf` -- y un `vsprintf` es lo que pide `I_Error(fmt, ...)`.
        if name == "va_arg" {
            if args.len() != 1 {
                self.errors.push(
                    "__va_arg(i) espera UN argumento: el indice del variadico".into());
                return;
            }
            if !self.es_variadica {
                self.errors.push(
                    "__va_arg() en una funcion que no declara '...': no hay argumentos \
                     variadicos que leer".into());
                return;
            }
            self.emit_expr(&args[0]);                       // rax = i
            let base = 16 + self.ranuras_con_nombre * 8;    // primer variadico
            // lea rdx, [rbp + base]
            self.code.extend_from_slice(&[0x48, 0x8D, 0x95]);
            self.code.extend_from_slice(&base.to_le_bytes());
            // mov rax, [rdx + rax*8]
            self.code.extend_from_slice(&[0x48, 0x8B, 0x04, 0xC2]);
            return;
        }

        // * `__va_list()` -- la DIRECCION del primer argumento variadico.
        //
        // # Por que `__va_arg(i)` no bastaba
        //
        // Un indice solo sirve DENTRO de la funcion que declara el `...`:
        // describe una posicion en SU marco de pila. En cuanto se le pasa a
        // otra funcion --que es lo que hace `va_start(ap, s); vsnprintf(buf, n,
        // s, ap)`, o sea el patron entero de la familia `v*`-- el numero ya no
        // apunta a nada, porque la funcion de destino tiene su propio marco.
        //
        // Un PUNTERO si viaja. Y aqui es un puntero y no una estructura de dos
        // cursores como en SysV, porque BMO C pasa los argumentos **por la
        // pila y seguidos**: la lista variadica ya ES un array en memoria, y el
        // `va_list` de C es su primera casilla. La convencion mas vieja vuelve
        // a salir ganando, igual que en `__va_arg`.
        //
        // No lleva el `last` que pide el estandar (`va_start(ap, last)`) porque
        // aqui no hace falta: el compilador sabe cuantos parametros con nombre
        // tiene la funcion. La macro de `<stdarg.h>` se lo come y llama a esto.
        if name == "va_list" {
            if !args.is_empty() {
                self.errors
                    .push("__va_list() no lleva argumentos: es la direccion del primer variadico".into());
                return;
            }
            if !self.es_variadica {
                self.errors.push(
                    "__va_list() en una funcion que no declara '...': no hay argumentos \
                     variadicos a los que apuntar".into());
                return;
            }
            let base = 16 + self.ranuras_con_nombre * 8;
            // lea rax, [rbp + base]
            self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
            self.code.extend_from_slice(&base.to_le_bytes());
            return;
        }
        let Some(def) = self.intrinsics.get(name) else {
            self.errors.push(format!(
                "intrinsic __{name}() no existe en la tabla sem-asm (tables/arch/x86_64/intrinsics.toml)"));
            return;
        };
        if args.len() != def.args.len() {
            self.errors.push(format!(
                "intrinsic __{name}() espera {} argumento(s), recibio {}",
                def.args.len(), args.len()));
            return;
        }
        let bytes = def.bytes.clone();
        let arg_regs = def.args.clone();
        let returns = def.returns.clone();

        // === * LA RUTA SSE: un argumento en `xmm` NO PASA POR LA PILA ===
        //
        // La tabla compartida crecio el 2026-08-22 con `sqrtsd`, `minsd` y
        // `maxsd` --las trajo INTI, que es quien las pedia-- y este camino no se
        // entero. El volcado de abajo es un `pop` a un registro ENTERO, asi que
        // `xmm1` cayo en el `_ =>` de `emit_pop_to_reg` y la matriz de
        // conformidad se puso roja. **La tabla crecio y el segundo consumidor
        // no**, que es el patron y no el accidente.
        //
        // ** Y hacerlo por la pila no era la version corta: un `double` no vive
        // en `rax`. Evaluar el argumento por el camino entero lo convertiria a
        // entero ANTES de la instruccion, y `__maxsd(1.5, 2.5)` daria `2.0` en
        // vez de `2.5` -- algo que compila y no hace lo que dice, con la firma
        // detras.
        //
        // El camino bueno ya existia al lado. `floats.rs::emit_fbinop` tiene la
        // forma EXACTA de estas filas --deja `a` en xmm0, `b` en xmm1 y suelta
        // los bytes de `<op>sd xmm0, xmm1`-- porque es lo que ya hacen `addsd`
        // y sus tres hermanas desde el primer dia.
        if arg_regs.iter().any(|r| r.starts_with("xmm")) {
            self.emit_intrinseco_sse(name, &arg_regs, &bytes, args);
            return;
        }

        // 1) evaluar cada argumento a rax y apilarlo (orden de aparicion)
        for a in args {
            self.emit_expr(a);
            self.code.push(0x50); // push rax
        }
        // 2) volcar a los registros destino, en REVERSA (el tope es el ultimo
        //    arg). Cada destino es un registro DISTINTO (rax/rcx/rdx) -> pop
        //    directo sin pisarse.
        for reg in arg_regs.iter().rev() {
            self.emit_pop_to_reg(reg);
        }
        // 3) los bytes exactos de la instruccion
        self.code.extend_from_slice(&bytes);
        // 4) normalizar el valor de retorno a rax
        self.emit_intrinsic_return(returns.as_deref());
    }

    /// * Un intrinseco cuyos argumentos van en `xmm`: por la ruta de coma
    /// flotante, no por la pila.
    ///
    /// Las formas admitidas son las que la tabla tiene hoy: un argumento en
    /// `xmm0` (el destino es el mismo operando, como `sqrtsd`), o dos en
    /// `xmm0`+`xmm1` en ese orden.
    ///
    /// ** Cualquier otra cosa **se dice**. Una fila con `xmm2`, o que mezcle un
    /// entero con un flotante, no tiene camino aqui todavia -- y emitir lo mas
    /// parecido seria exactamente el fallo que este fichero existe para no
    /// tener. El mensaje dice donde esta la fila, que es lo unico util cuando
    /// esto salte dentro de seis meses.
    fn emit_intrinseco_sse(&mut self, name: &str, regs: &[String], bytes: &[u8], args: &[Expr]) {
        match regs {
            [uno] if uno == "xmm0" => {
                self.emit_fexpr_operand(&args[0]);
                self.code.extend_from_slice(bytes);
            }
            [a, b] if a == "xmm0" && b == "xmm1" => {
                self.emit_fbinop(&args[0], &args[1], bytes);
            }
            _ => self.errors.push(format!(
                "intrinsic __{name}(): la ruta SSE de BMO C sabe emitir [xmm0] y                  [xmm0, xmm1]; esta fila pide {regs:?}                  (tables/arch/x86_64/intrinsics.toml)"
            )),
        }
    }

    /// Saca el tope de la pila al registro destino de un argumento.
    ///
    /// Los nombres de 64 bits (`rdi`, `rsi`, `r10`, `r8`) no estaban, y esa
    /// ausencia era justo la que dejaba `syscall` fuera del lenguaje: la
    /// convencion de la puerta congelada pasa los argumentos por ahi, asi que
    /// sin estos registros no habia forma de escribir la llamada en C. Solo
    /// existian los de los puertos de E/S (`dx`, `al`) y los de `rdmsr`.
    pub(super) fn emit_pop_to_reg(&mut self, reg: &str) {
        match reg {
            "rax" | "eax" | "ax" | "al" => self.code.push(0x58),  // pop rax
            "rcx" | "ecx" | "cx" | "cl" => self.code.push(0x59),  // pop rcx
            "rdx" | "edx" | "dx"        => self.code.push(0x5A),  // pop rdx
            "rbx" => self.code.push(0x5B),
            "rsi" | "esi" | "si" => self.code.push(0x5E),
            "rdi" | "edi" | "di" => self.code.push(0x5F),
            // r8..r11 llevan REX.B: el `pop` corto solo alcanza los ocho
            // registros clasicos.
            "r8"  => self.code.extend_from_slice(&[0x41, 0x58]),
            "r9"  => self.code.extend_from_slice(&[0x41, 0x59]),
            "r10" => self.code.extend_from_slice(&[0x41, 0x5A]),
            "r11" => self.code.extend_from_slice(&[0x41, 0x5B]),
            "u64_edx_eax" => {
                // valor de 64 bits en rax -> edx:eax (para wrmsr)
                self.code.push(0x58);                              // pop rax
                self.code.extend_from_slice(&[0x48, 0x89, 0xC2]); // mov rdx, rax
                self.code.extend_from_slice(&[0x48, 0xC1, 0xEA, 0x20]); // shr rdx, 32
            }
            _ => self.errors.push(format!("registro de argumento desconocido: {reg}")),
        }
    }

    /// Deja el resultado del intrinseco limpio en rax segun de donde salga.
    pub(super) fn emit_intrinsic_return(&mut self, returns: Option<&str>) {
        match returns {
            Some("u64_edx_eax") => {
                self.code.extend_from_slice(&[0x48, 0xC1, 0xE2, 0x20]); // shl rdx, 32
                self.code.extend_from_slice(&[0x48, 0x09, 0xD0]);       // or rax, rdx
            }
            Some("al") => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]), // movzx rax, al
            Some("ax") => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0xC0]), // movzx rax, ax
            // La puerta devuelve DOS cosas: el codigo en rax y el valor en
            // rdx. Quien pide el valor se lleva rdx a rax, que es donde este
            // codegen espera todo resultado.
            Some("rdx") => self.code.extend_from_slice(&[0x48, 0x89, 0xD0]), // mov rax, rdx
            // "eax": escribir eax en modo 64-bit ya deja rax con el alto en cero
            _ => {}
        }
    }

    // =============== Ruta SSE: floats en xmm0 (doble precision) ===============
    // C tradicional oculta si un valor es float; aqui el codegen lo SABE y lo
    // rutea por xmm. Se computa todo en double; `float` (f32) se convierte en
    // los bordes (load/store). Registro de trabajo: xmm0; scratch: xmm1.

    pub(super) fn emit_mov_eax_syscall(&mut self, nr: u32) {
        self.code.extend_from_slice(&[0xB8]);
        self.code.extend_from_slice(&nr.to_le_bytes());
        self.emit_call_to_syscall_stub();
    }

    /// La puerta del kernel desde el codigo emitido.
    ///
    /// === * Por que `Ring0Kernel` NO compila, y lo que emitia ===
    ///
    /// Emitia `0F 05 C3` -- `syscall; ret` -- con el comentario *"los mismos 3
    /// bytes, sin relocacion"*. Y era falso de una forma que no se ve leyendo:
    /// el stub de Ring 3 es un **llamable** (`syscall; ret` al que se llega con
    /// un `call`, y el `ret` devuelve al llamante). Poniendolo en linea se
    /// quita el `call` y **se queda el `ret`**: la funcion entera retorna en
    /// cuanto vuelve el syscall, y todo lo que hubiera detras no se ejecuta.
    ///
    /// Y hay una segunda razon, mas de fondo: **`syscall` desde Ring 0 no tiene
    /// sentido**. Carga CS y SS de `IA32_STAR` y salta a `LSTAR`; desde CPL0 eso
    /// es reentrar en el manejador del kernel con la pila del kernel. Codigo de
    /// Ring 0 no pide servicios -- los llama.
    ///
    /// No lo cazo nadie porque **nadie construye este perfil**: es alcanzable
    /// solo por `compile_with_target`, y en todo el arbol nada lo pasa. Un
    /// camino muerto que emite bytes incorrectos es peor que uno que no existe,
    /// porque el dia que alguien lo use el fallo no se parecera a su causa.
    pub(super) fn emit_call_to_syscall_stub(&mut self) {
        if self.target == TargetProfile::Ring0Kernel {
            self.errors.push(
                "no se compila C para Ring 0: `syscall` desde CPL0 reentra en el \
                 manejador del kernel, y el codigo de Ring 0 no pide servicios, \
                 los llama. Si algun dia hace falta, lo que toca es emitir la \
                 LLAMADA DIRECTA a la funcion del kernel, no la puerta"
                    .to_string(),
            );
        } else {
            // Ring 3: call __bmo_syscall_stub via E8 rel32
            self.code.extend_from_slice(&[0xE8]);
            self.call_relocs.push(CallReloc { offset: self.code.len(), target: "__bmo_syscall_stub".to_string() });
            self.code.extend_from_slice(&[0, 0, 0, 0]);
        }
    }
}
