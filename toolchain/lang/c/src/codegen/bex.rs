//! **EL `.bex` QUE SALE**: las secciones, los simbolos y el reparto del BSS.
//!
//! [fase]     IMAGEN
//!
//! [aparece]  METAL -- lo que se arma aqui lo lee el CARGADOR. Una seccion mal
//!            medida o un simbolo con el offset de otro no fallan al compilar:
//!            fallan al arrancar, y lejos
//!
//! [carril]   AMARILLO -- hay que EJECUTAR para verlo: compila en verde y falla despues
//!            * y sale de su `[aparece]`, no de una opinion: ver toolchain/tools/fases/
//!
//! [cuesta]   DATO -- de aqui sale el FICHERO. Lo que se equivoque aqui es un
//!            `.bex` que el cargador acepta y que no dice lo que dice
//!
//! [riesgo]   AJENO -- estos bytes los lee OTRO programa (el cargador), y una
//!            vez escritos no se deshacen: el fichero ya esta en el disco
//!
//! ## *** POR QUE ESTO SALE DE `mod.rs` (2026-09-12)
//!
//! Porque **`mod.rs` declaraba UNA fase y hacia DOS**. Su cabecera dice
//! `[fase] EMISION` --emitir bytes de instrucciones-- y 360 de sus 1.738 lineas
//! eran IMAGEN: armar las secciones del BEF, construir la tabla de simbolos y
//! repartir el BSS.
//!
//! Y `[fase]` no es decoracion. El build la cuenta --*"39 de los 39 ficheros de
//! BMO C declaran su fase"*-- y `toolchain/tools/fases` la usa para contestar
//! **donde aparece un fallo de este fichero**. Un fichero que declara EMISION y
//! hace IMAGEN le miente a ese juez: un fallo de `build_bef` no aparece
//! *dentro* como dice `mod.rs`, aparece **en el metal**, al cargar.
//!
//! ** O sea que el corte no es de tamano ni de gusto: es que la etiqueta era
//! FALSA. Y la unica forma de que deje de serlo es que las dos fases vivan en
//! dos ficheros, porque la etiqueta es por FICHERO.
//!
//! [!] Y de paso `mod.rs` baja de 1.738 a 1.378 lineas. Eso es una consecuencia,
//! no el motivo -- L6a es un trinquete y no habria obligado a nada.

use super::*;

impl Codegen {
    /// * LOS CEROS NO SE GUARDAN: SE DECLARAN. La seccion `Bss`.
    ///
    /// === El numero que obligo a escribir esto ===
    ///
    /// De los **645.008 bytes** de la seccion `data` de DOOM, **582.291 eran
    /// cero**: el 90,3% de la seccion y el **44,8% del `.bex` entero**. Casi la
    /// mitad del fichero eran ceros que se guardaban en el disco, se leian del
    /// disco, se copiaban al bufer de rebote del kernel y se copiaban otra vez
    /// al espacio del proceso. Cuatro veces pagado un byte cuyo valor ya se
    /// sabia al compilar.
    ///
    /// El motivo era de una linea: este codegen metia TODOS los globales en
    /// `.data`, con o sin inicializador. La maquinaria para no hacerlo ya
    /// estaba entera y sin estrenar -- `BefBuilder::bss()` existe, el escritor
    /// ya salta las `Bss` al colocar y al volcar, y `proc.rs` ya reserva las
    /// paginas y las pone a cero (`bex.rs` acepta `file_size == 0` **solo** si
    /// la seccion es `Bss`). Faltaba quien lo pidiera.
    ///
    /// Ver `docs/identidad/LA_RAM.md`: es el escalon 0 del modelo quirofano, y va primero
    /// porque encoge todo lo demas ANTES de optimizar como se transporta.
    ///
    /// === Como se decide, y son TRES motivos para quedarse ===
    ///
    /// Un global se va a `.bss` solo si no le aplica ninguno:
    ///
    /// 1. **Sus bytes no son todos cero.** El caso obvio.
    /// 2. **El cargador ESCRIBE dentro de el** -- una relocation lo tiene como
    ///    destino de escritura. `char *p = "x"` guarda ceros en el fichero y
    ///    parece un candidato perfecto, pero su valor de verdad lo pone el
    ///    cargador: mandarlo a `.bss` seria mandar la reloc a una seccion que su
    ///    codigo de `donde` no sabe nombrar.
    /// 3. ** **Alguien apunta a el.** Esta es la que no es obvia. El codigo de
    ///    seccion de una relocation solo distingue `code`/`data`/`rodata` -- no
    ///    hay valor para `bss`. Asi que un global a cero cuya DIRECCION se
    ///    guarda en otro global (`&contador` dentro de una tabla, que en DOOM es
    ///    `doom_defaults[]` entero) tiene que quedarse donde la reloc lo sepa
    ///    nombrar. Ampliar el codigo de seccion se puede, pero toca el formato
    ///    Y el cargador del kernel, y eso es otra tanda.
    ///
    /// === Por que el espacio de offsets sigue siendo UNO ===
    ///
    /// Los anclados se colocan primero y los demas detras, en el mismo espacio
    /// de offsets. Asi `global_offsets` no necesita decir en que seccion vive
    /// cada global: se deduce de si su offset pasa de `global_data.len()`. La
    /// unica consecuencia es que `patch_all_fixups` calcula la VA con dos
    /// bases, y todo lo demas del compilador sigue sin enterarse.
    ///
    /// Las regiones incluyen el relleno de alineacion del global siguiente
    /// --que son ceros y no cambia ningun veredicto-- y por eso miden todas un
    /// multiplo de 8: el reparto nuevo sale alineado sin recalcular nada.
    pub(super) fn separar_bss(&mut self) {
        if self.global_data.is_empty() {
            return;
        }

        let mut regiones: Vec<(u32, u32, String)> = self
            .global_offsets
            .iter()
            .map(|(n, &(off, _))| (off, 0u32, n.clone()))
            .collect();
        regiones.sort_by_key(|r| r.0);
        for i in 0..regiones.len() {
            let fin = regiones
                .get(i + 1)
                .map(|r| r.0)
                .unwrap_or(self.global_data.len() as u32);
            regiones[i].1 = fin.saturating_sub(regiones[i].0);
        }
        // ** FUERA LAS REGIONES DE LONGITUD CERO, y esto no es limpieza: es el
        // bug que el gate del BEF caza si no se hace.
        //
        // `type_stack_size` devuelve 0 para un tipo cuyo tamano no conoce, asi
        // que dos globales pueden acabar EN EL MISMO OFFSET. Con eso, el mapa
        // de traduccion --que se indexa por offset-- tiene dos duenos para la
        // misma clave: si uno esta anclado y el otro no, el ultimo en escribir
        // gana y **una reloc acaba apuntando dentro de `.bss`**, que es una
        // seccion que su codigo de `donde` no sabe nombrar.
        //
        // Se cayo asi de verdad al compilar DOOM: `reloc[293]: offset 0x614d0
        // exceeds target section size`. Quitarlas es correcto ademas de
        // necesario -- una region vacia no tiene bytes, y cualquier offset que
        // la nombrara cae igual en la region que empieza donde ella acaba.
        regiones.retain(|r| r.1 > 0);
        if regiones.is_empty() {
            return;
        }

        let mut anclado = vec![false; regiones.len()];

        // Motivo 2: el cargador escribe dentro.
        let escrituras = self
            .relocs_a_cadena
            .iter()
            .map(|&(off, _)| off)
            .chain(self.relocs_a_global.iter().map(|&(off, _, _)| off))
            .chain(self.relocs_a_funcion.iter().map(|&(off, _)| off))
            .collect::<Vec<u32>>();
        for off in escrituras {
            if let Some(i) = decidir::imagen::region_de(&regiones, off) {
                anclado[i] = true;
            }
        }

        // Motivo 3: alguien apunta a el, y una reloc no sabe nombrar `.bss`.
        let destinos = self
            .relocs_a_global
            .iter()
            .filter_map(|(_, gname, _)| self.global_offsets.get(gname).map(|&(off, _)| off))
            .collect::<Vec<u32>>();
        for off in destinos {
            if let Some(i) = decidir::imagen::region_de(&regiones, off) {
                anclado[i] = true;
            }
        }

        // Motivo 1: tiene algo escrito.
        for (i, &(off, len, _)) in regiones.iter().enumerate() {
            let ini = off as usize;
            let fin = (ini + len as usize).min(self.global_data.len());
            if self.global_data[ini..fin].iter().any(|&b| b != 0) {
                anclado[i] = true;
            }
        }

        // El reparto nuevo: anclados primero, en su orden; el resto detras.
        let mut datos: Vec<u8> = Vec::with_capacity(self.global_data.len());
        let mut nuevo_de: HashMap<u32, u32> = HashMap::with_capacity(regiones.len());
        for (i, &(off, len, _)) in regiones.iter().enumerate() {
            if !anclado[i] {
                continue;
            }
            nuevo_de.insert(off, datos.len() as u32);
            let ini = off as usize;
            let fin = (ini + len as usize).min(self.global_data.len());
            datos.extend_from_slice(&self.global_data[ini..fin]);
            while datos.len() % 8 != 0 {
                datos.push(0);
            }
        }
        let data_len = datos.len() as u32;
        let mut cursor = data_len;
        for (i, &(off, len, _)) in regiones.iter().enumerate() {
            if anclado[i] {
                continue;
            }
            nuevo_de.insert(off, cursor);
            cursor += (len + 7) & !7;
        }

        // Y se traduce todo lo que hablaba de offsets viejos. Un offset puede
        // caer DENTRO de un global (una tabla se parchea por elementos), asi que
        // se traslada su region y se conserva la distancia al principio.
        let traducir = |viejo: u32| -> u32 {
            match decidir::imagen::region_de(&regiones, viejo) {
                Some(i) => nuevo_de[&regiones[i].0] + (viejo - regiones[i].0),
                None => viejo,
            }
        };
        for v in self.global_offsets.values_mut() {
            v.0 = traducir(v.0);
        }
        for r in self.relocs_a_cadena.iter_mut() {
            r.0 = traducir(r.0);
        }
        for r in self.relocs_a_global.iter_mut() {
            r.0 = traducir(r.0);
        }
        for r in self.relocs_a_funcion.iter_mut() {
            r.0 = traducir(r.0);
        }

        self.global_data = datos;
        self.bss_len = (cursor - data_len) as usize;

        // ** EL GUARDIA, y se queda aunque hoy no salte.
        //
        // La regla entera de este paso cabe en una frase: **una relocation
        // nunca escribe en `.bss`**. Si algun dia un motivo de anclaje se
        // queda corto, el sintoma sin este guardia es un `.bex` que el gate del
        // BEF rechaza con un offset en hexadecimal, o --peor, si el gate no
        // estuviera-- un cargador escribiendo ocho bytes en la pagina de otra
        // cosa. Aqui se dice con el nombre del global delante.
        let fuera: Vec<u32> = self
            .relocs_a_cadena
            .iter()
            .map(|&(off, _)| off)
            .chain(self.relocs_a_global.iter().map(|&(off, _, _)| off))
            .chain(self.relocs_a_funcion.iter().map(|&(off, _)| off))
            .filter(|&off| off >= data_len)
            .collect();
        for off in fuera {
            let quien = self
                .global_offsets
                .iter()
                .find(|(_, &(g, _))| g <= off && off < g + 8)
                .map(|(n, _)| n.clone())
                .unwrap_or_else(|| format!("offset {off}"));
            self.errors.push(format!(
                "bug del compilador: una relocation escribe en '{quien}', que quedo en .bss. \
                 Un global al que el cargador escribe tiene que quedarse en .data -- ver \
                 los tres motivos de anclaje en `separar_bss`"
            ));
        }
    }

    /// **La tabla de simbolos del `.bex`.** Ver la llamada en `build_bef`.
    ///
    /// ## El TAMANO se deduce, y por eso vale mas que `--map`
    ///
    /// `--map` da la direccion de inicio de cada funcion. Con eso, un `rip`
    /// entre dos funciones se atribuye a la de arriba **aunque caiga fuera de
    /// ella** -- en un hueco de relleno, o en una funcion que el mapa no vio.
    ///
    /// Aqui el tamano sale de la distancia a la siguiente funcion, y la ultima
    /// llega hasta el final del codigo. Con eso, quien lee puede decir *"esta
    /// direccion NO esta en ninguna funcion"*, que es una respuesta distinta y
    /// mucho mas util que un nombre equivocado.
    ///
    /// [!] `virt_addr` guarda el offset DENTRO de la seccion de codigo, no una
    /// direccion virtual, y `section_idx` dice cual es esa seccion. El
    /// compilador no decide donde se carga el programa -- eso es del cargador, y
    /// escribir aqui una direccion absoluta seria repetir una decision ajena.
    fn seccion_de_simbolos(&self) -> BefSection {
        use bmo_abi::bef::symbols::{name_hash, Symbol, SymbolBinding, SymbolKind, SymbolVisibility};

        let mut orden: Vec<(usize, &String)> =
            self.function_offsets.iter().map(|(n, off)| (*off, n)).collect();
        orden.sort();

        let mut entradas: Vec<Symbol> = Vec::with_capacity(orden.len());
        let mut cadenas: Vec<u8> = Vec::new();

        for (i, (offset, nombre)) in orden.iter().enumerate() {
            // Hasta donde llega: el principio de la siguiente, o el final del
            // codigo si es la ultima.
            let fin = orden
                .get(i + 1)
                .map(|(sig, _)| *sig)
                .unwrap_or(self.instruction_end);

            let name_off = cadenas.len() as u32;
            cadenas.extend_from_slice(nombre.as_bytes());
            cadenas.push(0); // las cadenas acaban en cero: quien lee no trae longitudes

            entradas.push(Symbol {
                name_off,
                name_hash: name_hash(nombre),
                virt_addr: *offset as u64,
                size: fin.saturating_sub(*offset) as u64,
                kind: SymbolKind::Function as u8,
                // Todos LOCAL por ahora, y es la verdad de hoy: sin enlazador no
                // hay nadie a quien exportar. Cuando exista, esto lo decide
                // `static` en el fuente y no una constante aqui.
                binding: SymbolBinding::Local as u8,
                visibility: SymbolVisibility::Default as u8,
                section_idx: 0, // la seccion de codigo se anade siempre la primera
                _reserved: 0,
            });
        }

        BefSection::symbols(entradas, cadenas)
    }

    pub(super) fn build_bef(&mut self) -> Vec<u8> {
        let all = core::mem::take(&mut self.code);
        let mut b = BefBuilder::new();

        let code_bytes = &all[..self.instruction_end];
        let rodata_bytes = &all[self.instruction_end..self.string_data_end];
        let data_bytes = &all[self.string_data_end..];

        let mut code_sec = BefSection::code(code_bytes.to_vec());
        code_sec.alignment = 4096;
        b.add_section(code_sec);

        if !rodata_bytes.is_empty() {
            let mut rodata_sec = BefSection::rodata(rodata_bytes.to_vec());
            rodata_sec.alignment = 4096;
            b.add_section(rodata_sec);
        }

        if !data_bytes.is_empty() {
            let mut data_sec = BefSection::data(data_bytes.to_vec());
            data_sec.alignment = 4096;
            b.add_section(data_sec);
        }

        // * LA SECCION `Bss`: los globales que son todo ceros. No lleva ni un
        // byte en el fichero -- solo dice cuantos hacen falta, y el cargador
        // reserva las paginas y las entrega a cero, que es lo que ya hacia con
        // cualquier seccion (`phys::zero_frame` antes de copiar).
        //
        // Va DESPUES de `data` y antes de las relocs, porque el cargador coloca
        // en el orden de la tabla y `patch_all_fixups` calculo `va_bss`
        // contando con que `.data` va justo delante.
        if self.bss_len > 0 {
            let mut bss_sec = BefSection::bss(self.bss_len as u64);
            bss_sec.alignment = 4096;
            b.add_section(bss_sec);
        }

        // * LA SECCION `Relocs`, y va DESPUES de las tres cargables a proposito:
        // sus offsets son relativos a `.data` y `.rodata`, o sea que se refiere
        // a las que ya estan puestas. Y no es cargable --`is_loadable` la
        // excluye-- asi que no ocupa una pagina en el proceso: el cargador la
        // lee del fichero, aplica lo que dice y la olvida.
        //
        // Solo se emite si hay alguna. Un `.bex` sin punteros en datos no lleva
        // seccion de relocs, igual que uno sin syscalls dejo de llevar el stub.
        if !self.relocs.is_empty() {
            let relocs = core::mem::take(&mut self.relocs);
            b.add_section(BefSection::relocs(relocs));
        }

        // ** LA SECCION `Symbols`: que funcion vive en cada offset.
        //
        // === Por que existe, y por que hasta hoy no ===
        //
        // El compilador SIEMPRE ha sabido esto: `function_offsets` lo lleva
        // dentro para resolver las llamadas. El 2026-08-13 salio por la consola
        // con `--map`, porque DOOM murio con `rip 0x400815f2` y ese numero no
        // servia para nada. Pero salir por la consola obliga a que alguien
        // recompile el programa y reste a mano cada vez.
        //
        // Escribirlo en el `.bex` cierra el circuito: el que tiene el binario
        // tiene los nombres. La autopsia del kernel puede decir
        // `SHA1_Update+0x18` sin que nadie pase nada por `--map`.
        //
        // ** Y es la primera fila de la COMPILACION SEPARADA. Un enlazador
        // necesita saber que ofrece cada objeto; esto es exactamente eso, aunque
        // hoy lo lea un depurador y no un enlazador.
        //
        // [!] `SectionKind::Symbols` y `BefSection::symbols()` llevaban escritos
        // desde que se diseno BEF y **no los usaba nadie** -- igual que
        // `Resources` antes del paquete. El sitio ya estaba; faltaba llenarlo.
        //
        // No es cargable (`is_loadable` solo mapea Code/RoData/Data/Bss), asi
        // que **no cuesta ni una pagina al proceso**: viaja en el fichero y el
        // cargador la salta.
        if !self.function_offsets.is_empty() {
            b.add_section(self.seccion_de_simbolos());
        }

        // * La bandera de la pantalla, deducida al recorrer el programa. Ver
        // `BefFlags::WANTS_SCREEN`: la pone el compilador y no el autor para que
        // diga lo que el programa HACE y no lo que promete.
        //
        // ** Y NO SE PONE si el programa ademas sabe componerse (2026-09-11).
        // Un programa con los DOS caminos --ventana si alguien compone,
        // pantalla entera si no-- lanzado desde el escritorio pide la ventana
        // y nunca la pantalla. Con la bandera puesta, el DIRECTOR se apartaba
        // ANTES de lanzarlo y se quedaba treinta segundos esperando una
        // reclamacion que no iba a llegar: pantalla negra, y luego la ventana.
        // La bandera dice lo que el programa HACE, y lo que hace un programa
        // con los dos caminos es preferir la ventana.
        if self.quiere_pantalla && !self.sabe_componerse {
            b.header.flags |= bmo_abi::bef::header::BefFlags::WANTS_SCREEN.bits();
        }

        b.entry_offset = self.entry_offset as u64;
        b.build().unwrap_or_default()
    }
}
