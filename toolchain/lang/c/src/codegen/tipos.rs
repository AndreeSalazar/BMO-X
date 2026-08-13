//! **LAS PREGUNTAS SOBRE UN TIPO**: es flotante? es sin signo?
//!
//! === Por que esto es un fichero aparte, y es una regla ===
//!
//! Porque el codegen calcula TODO en `rax`, o sea que **el tipo no esta en el
//! valor: hay que preguntarlo**. Cada vez que una operacion se comporta
//! distinto segun el tipo, la eleccion pasa por aqui.
//!
//! Hoy son dos ejes --flotante y signo-- y las dos funciones estan escritas
//! calcadas a proposito: misma forma, mismos brazos, en el mismo fichero. **El
//! tercer eje que aparezca se escribe igual y al lado.**
//!
//! === ** Por que estan juntas, con nombre y fecha ===
//!
//! `expr_is_float` llevaba escrita desde siempre. Cuando el 2026-08-13 se
//! descubrio que las cuatro operaciones sin signo emitian la version con signo,
//! el arm de `Shr` decia por escrito: *"un tipo sin signo querria `shr`; hoy el
//! codegen no arrastra esa distincion hasta aqui"*.
//!
//! Y era falso -- la distincion llegaba, y `expr_is_unsigned` se escribio
//! copiando la de al lado. **Estaban a 900 lineas de distancia**, y esa
//! distancia es la que hizo que pareciera un trabajo grande en vez de una
//! funcion gemela.

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
            Expr::Field(_, _, _, t) | Expr::Arrow(_, _, _, t) => Self::is_unsigned_ty(t),
            Expr::IndexPtr(_, _, t) => Self::is_unsigned_ty(t),
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
            Expr::Field(_, _, _, t) | Expr::Arrow(_, _, _, t) => Self::is_float_ty(t),
            Expr::IndexPtr(_, _, t) => Self::is_float_ty(t),
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
            _ => false,
        }
    }
}
