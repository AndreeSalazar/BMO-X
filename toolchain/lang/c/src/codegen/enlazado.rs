//! **EL ENLAZADO INTERNO**: los saltos, las llamadas y las direcciones que no
//! se conocen cuando se emiten.
//!
//! === Por que esto es un fichero aparte ===
//!
//! Todo lo que hay aqui comparte una sola forma: **emitir un hueco, apuntar
//! donde esta, y rellenarlo cuando ya se sabe la distancia**. Un `if` no sabe
//! a que byte tiene que saltar hasta que se ha emitido su cuerpo; una llamada
//! no sabe donde esta la funcion hasta que se han emitido todas.
//!
//! Repartidos entre `emit_stmt` y `emit_program`, los doce metodos parecian
//! detalles de sus llamantes. Juntos son **un mecanismo con un nombre**, y la
//! propiedad que hay que conservar se lee de una vez: *ningun desplazamiento
//! se cuenta a mano*.
//!
//! [!] Y no es teorico -- la version a mano de estos huecos, en
//! `bmo-lower/memoria.rs`, tenia **tres de cuatro saltos mal por uno**. Contar
//! bytes de instruccion es justo el trabajo que una maquina hace sin
//! equivocarse.

use super::*;

impl Codegen {
    pub(super) fn patch_all_fixups(&mut self) {
        // * EL BUFER VA APRETADO Y LOS DESPLAZAMIENTOS SE CALCULAN CON LA REGLA
        // DEL CARGADOR. Antes se rellenaba cada tramo hasta la pagina, y ese
        // relleno viajaba DENTRO DEL FICHERO.
        //
        // El problema que resolvia era real: estos `lea [rip+disp]` se contaban
        // asumiendo que los datos van PEGADOS detras del codigo, y el cargador
        // (`ring0/task/proc.rs`) hace `va_cursor = va_start + pages * PAGE`, o
        // sea que pone cada seccion en la pagina siguiente. Con el codigo a 500
        // bytes, el compilador apuntaba al byte 500 y el cargador dejaba la
        // cadena en el 4096: un `%s` leia basura EN HARDWARE.
        //
        // Rellenar hacia coincidir las dos cuentas. Pero **es la cuenta lo que
        // habia que arreglar, no el tamano del fichero**: ahora el compilador
        // modela la regla del cargador --tres sumas-- y no necesita empujar 2 642
        // bytes de `0xCC` por seccion para que el mundo cuadre.
        //
        // Lo que esto quita, MEDIDO:
        //
        //   - los seis ejemplos, de 107 184 a 84 952 bytes (-20,7%), y todo
        //     ahorro de codigo futuro deja de ser invisible bajo el relleno.
        //     `holac.bex`: 12 376 -> 8 432
        //   - el tercer `pad_to_page`, que rellenaba la seccion `data` -- la
        //     ultima, sin nada detras. Relleno por relleno.
        //
        // Lo que NO quita, y conviene tenerlo escrito con su numero porque es
        // el siguiente escalon: **el BEF sigue alineando los `file_offset` a
        // 4096**. En `holac.bex` eso son 3 952 bytes de hueco antes del codigo
        // y 2 642 antes de rodata -- o sea que **6 594 de sus 8 432 bytes son
        // agujeros**. El campo `alignment` de una seccion se usa para las dos
        // cosas a la vez, y solo la direccion VIRTUAL lo necesita: el cargador
        // copia desde `file_offset` con un `copy_nonoverlapping` al que le da
        // igual donde empiece.
        //
        // Lo que esto NO quita, y hay que decirlo: **el acoplamiento sigue
        // ahi**. El compilador conoce la regla de colocacion del cargador. La
        // solucion definitiva son relocations de verdad en el BEF, para que el
        // cargador parchee y el compilador no tenga que adivinar donde va a
        // caer nada. Esto es la mitad del camino: quita el coste, deja la deuda
        // -- y ahora el emulador SI distingue las dos cuentas, asi que la otra
        // mitad se puede escribir con red.
        let code_len = self.code.len();
        self.instruction_end = code_len;

        // Las direcciones virtuales de cada seccion, con la cuenta del
        // cargador: cada una arranca en la pagina siguiente a las que ocupa la
        // anterior. Relativas al inicio del codigo, que es lo que necesita un
        // `lea [rip+disp]`.
        let rodata_len: usize = self.strings.iter().map(|s| s.len() + 1).sum();
        let va_rodata = Self::hasta_pagina(code_len);
        let va_data = va_rodata + Self::hasta_pagina(rodata_len);

        // rodata: las cadenas. `off_en_seccion` es el offset DENTRO de rodata,
        // no dentro del bufer -- que es la distincion que este cambio introduce.
        let mut off_en_seccion = 0usize;
        for (idx, s) in self.strings.iter().enumerate() {
            for f in &self.fixups {
                if f.string_idx == idx {
                    let rip = f.lea_offset + 4;
                    let disp = (va_rodata + off_en_seccion) as i64 - rip as i64;
                    self.code[f.lea_offset..f.lea_offset + 4]
                        .copy_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            self.code.extend_from_slice(s.as_bytes());
            self.code.push(0);
            off_en_seccion += s.len() + 1;
        }
        self.string_data_end = self.code.len();

        // data y bss: los globales.
        //
        // * DOS BASES, UN ESPACIO DE OFFSETS. `separar_bss` dejo los globales a
        // cero al final, pasado `global_data.len()`, y el cargador pone `.bss`
        // en la pagina siguiente a `.data` igual que hace con todas. Asi que la
        // VA de un global es una cosa o la otra segun de que lado caiga su
        // offset -- y nadie mas en el compilador tiene que saberlo.
        let data_len = self.global_data.len();
        let va_bss = va_data + Self::hasta_pagina(data_len);
        for &(lea_offset, ref name) in &self.global_fixups {
            if let Some(&(off, _)) = self.global_offsets.get(name) {
                let va = if (off as usize) < data_len {
                    va_data + off as usize
                } else {
                    va_bss + (off as usize - data_len)
                };
                let rip = lea_offset + 4;
                let disp = va as i64 - rip as i64;
                self.code[lea_offset..lea_offset + 4]
                    .copy_from_slice(&(disp as i32).to_le_bytes());
            }
        }
        let globals = core::mem::take(&mut self.global_data);
        self.code.extend_from_slice(&globals);
        self.global_data = globals;

        // * LAS RELOCATIONS, que es lo unico que el compilador NO puede
        // resolver por su cuenta.
        //
        // Todo lo de arriba son desplazamientos: la distancia entre dos
        // secciones de la misma imagen es fija, asi que un `lea [rip+disp]` se
        // puede calcular aqui. Un PUNTERO GUARDADO EN UN DATO es otra cosa --
        // lleva la direccion absoluta, y esa depende de donde cargue el
        // programa. Se anota y la escribe el cargador.
        //
        // Los offsets van dentro de su seccion, no del bufer: el del puntero es
        // relativo a `.data` (ya lo es, sale de `global_data`) y el de la cadena
        // relativo a `.rodata`.
        let mut off_cadena: Vec<usize> = Vec::with_capacity(self.strings.len());
        let mut acc = 0usize;
        for s in &self.strings {
            off_cadena.push(acc);
            acc += s.len() + 1;
        }
        for &(off_en_data, idx) in &self.relocs_a_cadena {
            let Some(&destino) = off_cadena.get(idx) else {
                self.errors.push(format!(
                    "reloc a una cadena que no esta en la tabla (indice {idx}): esto es un bug \
                     del compilador, no del programa"
                ));
                continue;
            };
            self.relocs.push(Relocation::seccion_abs64(
                SEC_DATA,
                off_en_data as u64,
                SEC_RODATA,
                destino as i64,
            ));
        }

        // Las de GLOBAL. Se cierran aqui porque una tabla puede nombrar algo
        // declarado mas abajo que ella.
        let pendientes_g = core::mem::take(&mut self.relocs_a_global);
        for (off_en_data, gname, sumando) in pendientes_g {
            let Some(&(destino, _)) = self.global_offsets.get(&gname) else {
                self.errors.push(format!(
                    "la tabla apunta a '{gname}' y ese global no existe en esta unidad"
                ));
                continue;
            };
            self.relocs.push(Relocation::seccion_abs64(
                SEC_DATA,
                off_en_data as u64,
                SEC_DATA,
                destino as i64 + sumando,
            ));
        }

        // Y las de FUNCION, que ya se pueden cerrar: aqui los offsets del
        // codigo estan todos.
        let pendientes = core::mem::take(&mut self.relocs_a_funcion);
        for (off_en_data, fname) in pendientes {
            let Some(&destino) = self.function_offsets.get(&fname) else {
                self.errors.push(format!(
                    "la tabla apunta a '{fname}' y esa funcion no se emitio en esta unidad"
                ));
                continue;
            };
            self.relocs.push(Relocation::seccion_abs64(
                SEC_DATA,
                off_en_data as u64,
                SEC_CODE,
                destino as i64,
            ));
        }
    }

    pub(super) fn patch_goto_relocs(&mut self) {
        for (off, label) in &self.goto_relocs {
            if let Some(&target) = self.label_positions.get(label) {
                let disp = target as i32 - (*off as i32 + 4);
                self.code[*off..*off + 4].copy_from_slice(&disp.to_le_bytes());
            }
        }
    }

    /// `call rel32` a una funcion del catalogo de [`sintetizadas`], con su
    /// reloc pendiente.
    ///
    /// El nombre no se comprueba contra el catalogo aqui a proposito: si
    /// alguien se equivoca escribiendolo, [`Self::patch_call_relocs`] falla
    /// diciendo *"no existe la funcion 'X'"* con el nombre delante, que es un
    /// mejor error que un `panic` del compilador -- y ese camino ya esta probado
    /// (`una_funcion_desconocida_sigue_fallando_con_su_nombre`).
    pub(super) fn emit_call_sintetizada(&mut self, name: &str) {
        self.code.extend_from_slice(&[0xE8]);
        self.call_relocs.push(CallReloc {
            offset: self.code.len(),
            target: name.to_string(),
        });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    /// La PASADA sobre las relocs pendientes: pide a [`sintetizadas`] el cuerpo
    /// de lo que alguien llama y no esta definido, y registra su offset.
    ///
    /// El catalogo y los cuerpos NO estan aqui, y el corte es deliberado: este
    /// fichero sabe que es una reloc y que es el `Codegen`; aquel sabe que
    /// bytes implementan `strlen`. Anadir una funcion sintetizable no toca este
    /// metodo.
    ///
    /// Va ANTES de [`Self::patch_call_relocs`] y no puede ir despues: ese es
    /// quien escribe los desplazamientos, y necesita el offset ya registrado.
    pub(super) fn sintetizar_referidas(&mut self) {
        // [!] ESTE GUARDIA VALE MAS QUE UN COMENTARIO, y el motivo esta escrito
        // en la cabecera de `patch_all_fixups`: la seccion de codigo es
        // `all[..instruction_end]`, y **`rodata` es lo que viene detras**. Si
        // esta pasada se moviera despues de `patch_all_fixups`, los cuerpos
        // sintetizados caerian en `rodata`, que se mapea SIN permiso de
        // ejecucion -- y el `.bex` saltaria EN METAL.
        //
        // El banco de pruebas NO puede cazarlo: el emulador reconcatena las
        // secciones tal cual, asi que ejecutaria el cuerpo igual y los 262
        // tests seguirian verdes. O sea, exactamente la clase de fallo que solo
        // aparece con la maquina delante. Por eso se comprueba aqui.
        debug_assert_eq!(
            self.instruction_end, 0,
            "sintetizar_referidas() tiene que ir ANTES de patch_all_fixups():              si no, el cuerpo sintetizado acaba en rodata y no se puede ejecutar"
        );
        sintetizadas::inyectar(&mut self.code, &self.call_relocs, &mut self.function_offsets);
    }

    /// Escribe el destino de cada `call rel32`.
    ///
    /// * Una llamada sin destino es un ERROR, no un hueco.
    ///
    /// Antes el `if let` no tenia `else`: el desplazamiento se quedaba en 0, y
    /// `E8 00000000` es "llama a la instruccion siguiente" -- o sea, un `call`
    /// que empuja una direccion de retorno, no hace nada y vuelve. Un nombre mal
    /// escrito, o una macro con parametros que este preprocesador todavia no
    /// expande, producia un programa que compilaba y **se saltaba la llamada en
    /// silencio**.
    ///
    /// Aqui no hay enlazado que pueda rellenarlo mas tarde: no existe tabla de
    /// importaciones en la salida de este codegen, asi que todo lo que se llama
    /// tiene que estar en esta misma unidad --o en el catalogo de
    /// [`sintetizadas`], que es lo que la pasada de arriba acaba de inyectar--.
    /// La prueba de que era un descuido y no una decision esta tres funciones
    /// mas abajo: `patch_func_addr_fixups` ya reportaba exactamente este caso
    /// para los punteros a funcion.
    pub(super) fn patch_call_relocs(&mut self) {
        let mut faltan: Vec<String> = Vec::new();
        for reloc in &self.call_relocs {
            if let Some(&target_offset) = self.function_offsets.get(&reloc.target) {
                let off = reloc.offset;
                let disp = target_offset as i32 - (off as i32 + 4);
                self.code[off..off + 4].copy_from_slice(&disp.to_le_bytes());
            } else if !faltan.contains(&reloc.target) {
                faltan.push(reloc.target.clone());
            }
        }
        for name in faltan {
            self.errors.push(format!(
                "no existe la funcion '{name}' que se llama (aqui no hay enlazado: \
                 todo lo que se llama tiene que estar en esta unidad)"
            ));
        }
    }

    /// Escribe la direccion rip-relativa de cada funcion referida por un
    /// `lea rax, [rip+func]` (punteros a funcion). Mismo esquema que las
    /// call relocs: displacement dentro de la seccion de codigo.
    pub(super) fn patch_func_addr_fixups(&mut self) {
        for (off, name) in &self.func_addr_fixups {
            if let Some(&target) = self.function_offsets.get(name) {
                let disp = target as i32 - (*off as i32 + 4);
                self.code[*off..*off + 4].copy_from_slice(&disp.to_le_bytes());
            } else {
                self.errors.push(format!("no existe la funcion '{name}' cuya direccion se tomo"));
            }
        }
    }

    /// `lea rax, [rip+func]` -- deja en rax la direccion de una funcion.
    pub(super) fn emit_func_addr(&mut self, name: &str) {
        self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
        self.func_addr_fixups.push((self.code.len() - 4, name.to_string()));
    }

    /// Fija una etiqueta en la posicion actual y resuelve los saltos que ya
    /// la esperaban.
    ///
    /// El `label_offsets` es lo que faltaba: antes esta funcion SOLO
    /// parcheaba los saltos pendientes en ese instante, asi que un salto
    /// emitido DESPUES de fijar la etiqueta --es decir, todo salto hacia
    /// atras-- se quedaba con desplazamiento 0 para siempre. Eso significa
    /// "seguir a la instruccion siguiente": **ningun bucle de C daba mas de
    /// una vuelta**. `while`, `for`, `do-while`, y por tanto `break` y
    /// `continue`, ejecutaban el cuerpo exactamente una vez y salian. El
    /// binario compilaba y validaba igual.
    ///
    /// Es el mismo defecto que tenia el `IF` de COBOL, en otro lenguaje.
    pub(super) fn resolve_label(&mut self, label: u32) {
        let here = self.code.len();
        self.label_offsets.insert(label, here);
        let mut i = 0;
        while i < self.pending_relocs.len() {
            if self.pending_relocs[i].target_label == label {
                let off = self.pending_relocs[i].offset;
                let disp = here as i32 - (off as i32 + 4);
                self.code[off..off + 4].copy_from_slice(&disp.to_le_bytes());
                self.pending_relocs.swap_remove(i);
            } else { i += 1; }
        }
    }

    /// Resuelve los saltos que quedaron pendientes: los que apuntan a una
    /// etiqueta fijada ANTES de emitirlos (saltos hacia atras).
    ///
    /// Una etiqueta usada y jamas fijada es un bug del emisor: se aborta en
    /// vez de dejar un salto a ninguna parte.
    pub(super) fn patch_backward_relocs(&mut self) {
        for reloc in std::mem::take(&mut self.pending_relocs) {
            let target = *self
                .label_offsets
                .get(&reloc.target_label)
                .unwrap_or_else(|| panic!("etiqueta {} usada pero nunca fijada", reloc.target_label));
            let disp = target as i32 - (reloc.offset as i32 + 4);
            self.code[reloc.offset..reloc.offset + 4].copy_from_slice(&disp.to_le_bytes());
        }
    }

    pub(super) fn emit_jmp_reloc(&mut self, label: u32) {
        self.code.extend_from_slice(&[0xE9]);
        self.pending_relocs.push(PendingReloc { offset: self.code.len(), target_label: label });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    pub(super) fn emit_jz_reloc(&mut self, label: u32) {
        self.code.extend_from_slice(&[0x0F, 0x84]);
        self.pending_relocs.push(PendingReloc { offset: self.code.len(), target_label: label });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    pub(super) fn emit_jnz_reloc(&mut self, label: u32) {
        self.code.extend_from_slice(&[0x0F, 0x85]);
        self.pending_relocs.push(PendingReloc { offset: self.code.len(), target_label: label });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    // ---- Stack frame helpers ----
}
