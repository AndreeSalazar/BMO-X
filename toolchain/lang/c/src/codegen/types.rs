//! **THE QUESTIONS YOU ASK A TYPE**: is it floating point? is it unsigned?
//!
//! === Why this is a file of its own, and it is a rule ===
//!
//! Because the codegen computes EVERYTHING in `rax`, which means **the type is
//! not in the value: it has to be asked for**. Every time an operation behaves
//! differently depending on the type, the choice comes through here.
//!
//! Today there are two axes --floating point and signedness-- and the two
//! functions are written as carbon copies on purpose: same shape, same arms, in
//! the same file. **The third axis that shows up gets written the same way and
//! right next to them.**
//!
//! === *** Y EL TERCER EJE LLEGO: EL ANCHO (2026-09-04) ===
//!
//! Esta cabecera lo dejo escrito y se cumplio al pie de la letra. El eje nuevo
//! es **cuantos BITS ocupa el resultado**, y llego por donde llegan todos aqui:
//! una operacion que se comportaba distinta segun el tipo y no lo preguntaba.
//!
//! Todas las cuentas se emiten con `REX.W`, o sea en 64 bits. Para una cuenta
//! que se GUARDA en un `unsigned int` da igual --guardar en 32 recorta-- pero
//! DOOM la hace de una vez:
//!
//! ```c
//!    angle2 = (angle2 + ANG90) >> ANGLETOFINESHIFT;
//! ```
//!
//! El `>>` veia la suma todavia en 64 bits, con el acarreo que tenia que
//! haberse perdido, y devolvia **9212** donde tocaba 1020. Ese numero entra en
//! `viewangletox[]`, que tiene 4096 entradas: de ahi `x1=7 x2=0` --al reves-- y
//! de ahi el `Bad R_RenderWallRange` que estuvo una semana matando la partida.
//!
//! ** El eje del ancho se distingue de los otros dos en una cosa: los dos
//! primeros eligen QUE INSTRUCCION emitir, y este elige si emitir UNA MAS.
//!
//! === ** Why they are together, with a name and a date ===
//!
//! `expr_is_float` had been written forever. When it turned out on 2026-08-13
//! that all four unsigned operations were emitting the signed instruction, the
//! `Shr` arm said so in writing: *"an unsigned type would want `shr`; today the
//! codegen does not carry that distinction this far"*.
//!
//! And that was false -- the distinction did reach here, and `expr_is_unsigned`
//! was written by copying its neighbour. **They were 900 lines apart**, and
//! that distance is what made it look like a big job instead of a twin
//! function.
//!
//! ** The general lesson, and it is Eddi's: in a monolith some defects are
//! SMALL NEEDLES. Split the file and the same defect becomes a BIG needle -- in
//! a 134-line file holding exactly two sibling functions, a missing third one
//! is impossible not to see.

use super::*;

impl Codegen {
    pub(super) fn var_type_of(&self, name: &str) -> Option<TypeSpec> {
        self.var_offsets.get(name).map(|&(_, ref t)| t.clone())
            .or_else(|| self.global_offsets.get(name).map(|&(_, ref t)| t.clone()))
    }

    pub(super) fn is_float_ty(t: &TypeSpec) -> bool { matches!(t, TypeSpec::Float | TypeSpec::Double) }

    /// Un tipo que sobrevive SIN SIGNO a las promociones enteras de C.
    ///
    /// [!] `unsigned char` y `unsigned short` **no estan**, y no es un olvido:
    /// C11 6.3.1.1 dice que promocionan a `int` con signo, porque un `int` puede
    /// con todos sus valores. Meterlos aqui haria que `(unsigned short)0xFFFF /
    /// -1` diera un numero enorme en vez de lo que dice el estandar.
    ///
    /// Y en la practica da igual para el resultado --un valor de 16 bits nunca
    /// tiene el bit 63 puesto en `rax`-- pero la regla se escribe como es, no
    /// como se nota.
    pub(super) fn is_unsigned_ty(t: &TypeSpec) -> bool {
        matches!(
            t,
            TypeSpec::UnsignedInt | TypeSpec::UnsignedLong | TypeSpec::UnsignedLongLong
        )
    }

    /// **Esta expresion produce un valor sin signo?**
    ///
    /// # Por que hacia falta, y por que solo se notaba en 64 bits
    ///
    /// El codegen calcula TODO en `rax`, o sea en 64 bits. Un `unsigned int`
    /// con el bit 31 puesto se carga con `mov eax` y llega a `rax`
    /// **extendido con ceros**: el bit 63 vale 0, asi que `sar` y `shr` dan lo
    /// mismo y `idiv` y `div` tambien. Por eso `unsigned int` acertaba por
    /// casualidad y nadie lo vio.
    ///
    /// Con un `unsigned long` de 64 bits el bit 63 SI es del valor, y ahi las
    /// cuatro operaciones que miran el signo se equivocaban a la vez:
    ///
    /// ```text
    ///   (unsigned long)0x8000000000000000 >> 60   daba 18446744073709551608
    ///   ...                               / 2     daba un negativo enorme
    ///   ...                               % 10    idem
    ///   ...                               > 1     daba 0
    /// ```
    ///
    /// ** El arm de `Shr` lo CONFESABA en prosa: *"un tipo sin signo querria
    /// `shr`; hoy el codegen no arrastra esa distincion hasta aqui"*. Y era
    /// falso: la distincion llegaba --`var_type_of` existe, y `Field`, `Arrow`
    /// e `IndexPtr` traen su `TypeSpec` dentro-- lo que faltaba era preguntar.
    /// Un fallo confesado en prosa sigue siendo un fallo.
    ///
    /// Gemela de [`Self::expr_is_float`] a proposito: misma forma, mismos
    /// brazos. Cuando aparezca un tercer eje de tipo, que se escriba igual.
    pub(super) fn expr_is_unsigned(&self, e: &Expr) -> bool {
        match e {
            Expr::Var(n) => self.var_type_of(n).map_or(false, |t| Self::is_unsigned_ty(&t)),
            Expr::Cast(t, _) => Self::is_unsigned_ty(t),
            Expr::Field(_, _) | Expr::Arrow(_, _) => crate::tipos::tipo_de(self, e).map_or(false, |t| Self::is_unsigned_ty(&t)),
            Expr::IndexPtr(base, _) => self.pointee_type(base).map_or(false, |t| Self::is_unsigned_ty(&t)),
            // En una operacion binaria basta con que UNO sea sin signo: es la
            // conversion aritmetica usual de C, que arrastra el resultado al
            // tipo sin signo.
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b)
            | Expr::Mod(a, b) | Expr::BitAnd(a, b) | Expr::BitOr(a, b) | Expr::BitXor(a, b) => {
                self.expr_is_unsigned(a) || self.expr_is_unsigned(b)
            }
            // [!] En un desplazamiento manda SOLO el izquierdo. El derecho es
            // una cuenta, no un operando de la conversion usual: `1u << x` es
            // sin signo aunque `x` sea `int`, y `1 << u` es CON signo aunque
            // `u` no lo sea.
            Expr::Shl(a, _) | Expr::Shr(a, _) => self.expr_is_unsigned(a),
            Expr::BitNot(a) => self.expr_is_unsigned(a),
            Expr::Conditional(_, a, b) => self.expr_is_unsigned(a) || self.expr_is_unsigned(b),
            Expr::Call(n, _) => self
                .firmas
                .get(n)
                .map_or(false, |(_, ret)| Self::is_unsigned_ty(ret)),
            _ => false,
        }
    }

    /// Esta expresion produce un valor de punto flotante?
    pub(super) fn expr_is_float(&self, e: &Expr) -> bool {
        match e {
            Expr::FloatLit(_) => true,
            Expr::Var(n) => self.var_type_of(n).map_or(false, |t| Self::is_float_ty(&t)),
            Expr::Cast(t, _) => Self::is_float_ty(t),
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) =>
                self.expr_is_float(a) || self.expr_is_float(b),
            Expr::Neg(a) => self.expr_is_float(a),
            Expr::Field(_, _) | Expr::Arrow(_, _) => crate::tipos::tipo_de(self, e).map_or(false, |t| Self::is_float_ty(&t)),
            Expr::IndexPtr(base, _) => self.pointee_type(base).map_or(false, |t| Self::is_float_ty(&t)),
            Expr::Conditional(_, a, b) => self.expr_is_float(a) || self.expr_is_float(b),
            // * Una LLAMADA es flotante si su funcion devuelve un flotante.
            //
            // Faltaba, y hacia que `d = id(2.5)` guardara basura aunque las dos
            // mitades funcionaran por separado: el valor volvia en `xmm0` --el
            // retorno flotante lleva funcionando desde el principio-- y quien
            // llamaba lo iba a buscar a `rax`. El sintoma es un numero
            // cualquiera, no un error.
            Expr::Call(n, _) => self
                .firmas
                .get(n)
                .map_or(false, |(_, ret)| Self::is_float_ty(ret)),
            // ** Y un INTRINSECO no tiene firma: la suya vive en la TABLA.
            //
            // `sqrtsd`, `minsd` y `maxsd` declaran `returns = "xmm0"`. Sin esta
            // rama, `double m = __maxsd(a, b)` iria a buscar el valor a `rax` --
            // el mismo fallo que cuenta el parrafo de arriba, con la misma
            // pinta: **un numero cualquiera, no un error**.
            //
            // Y va contra la tabla y no contra una lista de tres nombres, para
            // que la cuarta fila que devuelva `xmm0` no tenga que acordarse de
            // pasar por aqui.
            Expr::Intrinsic(n, _) => self
                .intrinsics
                .get(n)
                .map_or(false, |d| d.returns.as_deref() == Some("xmm0")),
            _ => false,
        }
    }

    /// **EL RECORTE A 32 BITS, entre un operador y el siguiente.**
    ///
    /// # *** El bug que costo una semana de DOOM
    ///
    /// Todas las cuentas se emiten con `REX.W`, o sea en registros de 64 bits.
    /// Para una cuenta que se GUARDA en un `unsigned int` da igual: guardar en
    /// 32 bits recorta. Pero DOOM no guarda -- lo hace de una vez:
    ///
    /// ```c
    ///    angle2 = (angle2 + ANG90) >> ANGLETOFINESHIFT;
    /// ```
    ///
    /// El `>>` veia la suma **todavia en 64 bits**, con el acarreo que tenia
    /// que haberse perdido. Con `0xDFE00000 + 0x40000000` eso da un numero de
    /// treinta y tres bits, y el desplazamiento devolvia **9212** donde tocaba
    /// 1020. Ese numero entra en `viewangletox[]`, que tiene 4096 entradas: de
    /// ahi salio `x1=7 x2=0` --al reves-- y de ahi el `Bad R_RenderWallRange`.
    ///
    /// ** La aritmetica de angulos de DOOM **es** envolvente: sumar 270 grados a
    /// 180 tiene que dar 90. Sin el recorte, no hay angulos.
    ///
    /// # Por que solo en cuatro operadores
    ///
    /// `Add`, `Sub`, `Mul` y `Shl` son los unicos que pueden producir mas bits
    /// de los que entraron. `Div`, `Mod` y los bit a bit no pueden, y meterles
    /// un recorte seria pagar dos instrucciones por una imposibilidad.
    ///
    /// [!] Y el signo importa: `mov eax,eax` rellena de ceros y `movsxd` con el
    /// bit de signo. Elegir mal aqui convierte un `-1` en cuatro mil millones,
    /// que es el mismo fallo con otro traje. Quien lo decide es `tipos.rs`, que
    /// es el UNICO juez de tipos de este compilador.
    pub(super) fn recortar_a_32(&mut self, e: &Expr) {
        match crate::tipos::recorte_de(self, e) {
            // `mov eax, eax`: escribir en un registro de 32 bits pone a cero la
            // mitad de arriba. Es el idioma de x86-64 para "olvida el acarreo".
            Some(true) => self.code.extend_from_slice(&[0x89, 0xC0]),
            // `movsxd rax, eax`: ensancha con el bit de signo.
            Some(false) => self.code.extend_from_slice(&[0x48, 0x63, 0xC0]),
            // 64 bits, un puntero, o un tipo que el juez no sabe nombrar. No se
            // recorta: recortar lo que no cabe en 32 seria PERDER datos, que es
            // el bug contrario y peor.
            None => {}
        }
    }

    /// **UN DESPLAZAMIENTO, que toca los TRES ejes de este fichero a la vez.**
    ///
    /// Por eso vive aqui y no en el despachador: es la unica operacion que
    /// pregunta por el signo Y por el ancho en la misma linea.
    ///
    /// ```text
    ///    el SIGNO del izquierdo   ->  `sar` (copia el bit) o `shr` (ceros)
    ///    el ANCHO del resultado   ->  si hay que recortar despues
    /// ```
    ///
    /// [!] Y en las dos preguntas **manda el operando IZQUIERDO a solas**: en C
    /// el tipo de `a << b` no depende de `b`. `1u >> x` es sin signo aunque `x`
    /// sea `int`. Es la unica operacion binaria que se salta la conversion
    /// usual, y por eso es la que mas veces se ha emitido mal.
    ///
    /// ** A la izquierda no hay dos versiones: `shl` y `sal` son la misma
    /// instruccion. A la derecha si.
    pub(super) fn emit_desplazamiento(
        &mut self,
        a: &Expr,
        b: &Expr,
        izquierda: bool,
        entero: &Expr,
    ) {
        // shr rax,cl (/5) sin signo, sar rax,cl (/7) con el.
        let cola = if izquierda {
            0xE0
        } else if self.expr_is_unsigned(a) {
            0xE8
        } else {
            0xF8
        };
        self.emit_binop(a, b, &[
            0x48, 0x89, 0xC1, // mov rcx, rax   -> cuenta = b
            0x48, 0x89, 0xD0, // mov rax, rdx   -> valor  = a
            0x48, 0xD3, cola,
        ]);
        // Desplazar a la izquierda saca bits por arriba, y en un registro de 64
        // esos bits SOBREVIVEN. `1 << 31` en un `int` es negativo; sin recorte
        // salia positivo y valia dos mil millones. A la derecha no hace falta
        // --no se pueden ganar bits-- pero se pregunta igual: quien decide es
        // `recortar_a_32`, y un sitio que decide es mejor que dos que suponen.
        self.recortar_a_32(entero);
    }
}
