//! **LA DISPOSICION, del lado del codegen: colocarla y COTEJARLA.**
//!
//! [carril]  ROJO      un offset mal puesto no da error: da el campo de al lado
//!
//! [cuesta]  DATO -- de aqui salen los offsets con los que se emiten cargas y
//!           guardados. Equivocarse escribe en el sitio equivocado con el
//!           tamano equivocado, y el programa sigue corriendo.
//!
//! [riesgo]  ESPEJO
//!           ESPEJO -- esta es la SEGUNDA cuenta de la disposicion; la primera
//!                     la hace el frontend. Que sean dos es a proposito, y por
//!                     eso el cotejo vive aqui: dos cuentas que nadie contrasta
//!                     no son una comprobacion doble, son dos oportunidades de
//!                     equivocarse.
//!
//! # Por que este fichero existe
//!
//! Salio de `codegen/mod.rs` el 2026-09-02, y lo pidio L6a: aquel fichero esta
//! en la lista del trinquete y **solo puede encoger**. Anadir el cotejo lo hizo
//! crecer 34 lineas de codigo, y la regla contesto que no.
//!
//! ** Y tenia razon por el motivo de fondo, no por la aritmetica: colocar un
//! agregado y comprobar que la colocacion cuadra son **un solo concepto**, y
//! estaban repartidos entre una funcion sin vecinos y ninguna parte. Aqui se
//! leen juntos, que es lo que L6a persigue.

use super::*;

impl Codegen {
    /// Segunda de las tres copias que habia de la regla de disposicion. Ahora
    /// las tres llaman a `bmo_abi::types::disposicion`, que es donde esta
    /// escrita -- y con sus tests.
    ///
    /// Que el codegen la recalcule en vez de recibirla del parser **no es
    /// duplicacion**: es lo que hace que un frontend distinto (C++) que ya
    /// calculo offsets para sus nodos `Field` no pueda imponer una
    /// disposicion propia sin que se note.
    pub(super) fn build_struct_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut d = bmo_abi::types::Disposicion::nueva();
        for m in members {
            let sz = self.type_stack_size(&m.typ);
            layout.push((m.name.clone(), d.coloca(sz, self.type_align(&m.typ)), sz));
        }
        self.struct_layouts.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), d.total());
        self.struct_aligns.insert(name.to_string(), d.alineado());
    }

    pub(super) fn build_union_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut d = bmo_abi::types::DisposicionUnion::nueva();
        for m in members {
            let sz = self.type_stack_size(&m.typ);
            layout.push((m.name.clone(), d.coloca(sz, self.type_align(&m.typ)), sz));
        }
        self.struct_layouts.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), d.total());
        self.struct_aligns.insert(name.to_string(), d.alineado());
    }

    /// **El alineado de un tipo**, que no es su tamano en cuanto deja de ser
    /// un escalar.
    ///
    /// Las tres filas que no son la trivial son las que importan, y las tres
    /// aparecen en las estructuras que DOOM lee del WAD:
    ///
    /// - un **array** se alinea como su elemento, no como el conjunto. `char
    ///   name[8]` se alinea a 1, aunque mida 8 igual que un puntero.
    /// - un **agregado** se alinea como el mas exigente de sus miembros, que
    ///   es lo que `Disposicion::alineado()` fue acumulando al colocarlos.
    /// - un **puntero** siempre a 8, mida lo que mida lo apuntado.
    ///
    /// [!] El `unwrap_or(8)` de la rama del agregado es el mismo suelo que usa
    /// [`Self::type_stack_size`]: un struct que todavia no se ha colocado.
    /// Conservar los dos suelos iguales es lo que evita que tamano y alineado
    /// se contradigan a mitad de una disposicion.
    pub(super) fn type_align(&self, typ: &TypeSpec) -> u32 {
        match typ {
            TypeSpec::Array(t, _) => self.type_align(t),
            TypeSpec::StructRef(name) | TypeSpec::UnionRef(name) => {
                self.struct_aligns.get(name).copied().unwrap_or(8)
            }
            TypeSpec::Ptr(_) => 8,
            otro => bmo_abi::types::alineado_de(self.type_stack_size(otro)),
        }
    }

    pub(super) fn type_stack_size(&self, typ: &TypeSpec) -> u32 {
        match typ {
            TypeSpec::Void => 0,
            TypeSpec::Char | TypeSpec::UnsignedChar => 1,
            TypeSpec::Short | TypeSpec::UnsignedShort => 2,
            TypeSpec::Int | TypeSpec::UnsignedInt => 4,
            TypeSpec::Long | TypeSpec::UnsignedLong | TypeSpec::LongLong | TypeSpec::UnsignedLongLong => 8,
            TypeSpec::Float => 4,
            TypeSpec::Double => 8,
            TypeSpec::Ptr(_) => 8,
            TypeSpec::Array(t, n) => self.type_stack_size(t) * n,
            TypeSpec::StructRef(name) | TypeSpec::UnionRef(name) => {
                self.struct_sizes.get(name).copied().unwrap_or(8)
            }
        }
    }

    /// **EL COTEJO: que las dos disposiciones digan lo mismo, o que se note.**
    ///
    /// # Por que esto no sobraba, y por que hasta hoy no servia
    ///
    /// La cabecera de [`Self::build_struct_layout`] justifica que el codegen
    /// recalcule la disposicion en vez de recibirla: es lo que impide que un
    /// frontend distinto imponga la suya *"sin que se note"*.
    ///
    /// ** El argumento es bueno y **la implementacion no lo cumplia**: se
    /// calculaban dos disposiciones y no se comparaba ninguna. Dos cuentas que
    /// nadie contrasta no son una comprobacion doble: son dos oportunidades de
    /// equivocarse. Y ya paso -- el 2026-08-13 divergieron y lo destapo un bug,
    /// no un guardian.
    ///
    /// [!] Un `disposiciones` vacio **no es un fallo**: significa que ese
    /// frontend no declara la suya, y entonces manda la del codegen. Lo que si
    /// es un fallo es declararla y que no cuadre.
    pub(super) fn cotejar_disposicion(&self, program: &Program) -> Result<()> {
        for (nombre, suya) in &program.disposiciones {
            let mia = match self.struct_layouts.get(nombre) {
                Some(m) => m,
                // El frontend coloco un agregado que este codegen no vio. No
                // se juzga aqui: si alguien lo usa, fallara por su nombre.
                None => continue,
            };
            let tam = self.struct_sizes.get(nombre).copied().unwrap_or(0);
            let ali = self.struct_aligns.get(nombre).copied().unwrap_or(0);
            if suya.tamano != tam || suya.alineado != ali {
                return Err(CError::new(0, format!(
                    "disposicion de `{}`: el frontend dice tamano {} alineado {}, \
                     y el codegen calcula {} y {}",
                    nombre, suya.tamano, suya.alineado, tam, ali
                )));
            }
            if suya.campos.len() != mia.len() {
                return Err(CError::new(0, format!(
                    "disposicion de `{}`: el frontend coloca {} campo(s) y el codegen {}",
                    nombre, suya.campos.len(), mia.len()
                )));
            }
            for ((cn, co, cs), (mn, mo, ms)) in suya.campos.iter().zip(mia.iter()) {
                if cn != mn || co != mo || cs != ms {
                    return Err(CError::new(0, format!(
                        "disposicion de `{}`: el frontend pone `{}` en +{} midiendo {}, \
                         y el codegen pone `{}` en +{} midiendo {}",
                        nombre, cn, co, cs, mn, mo, ms
                    )));
                }
            }
        }
        Ok(())
    }
}
