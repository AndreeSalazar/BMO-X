//! **La lista de inicializacion y la construccion de la base** -- el paso 4 de
//! `BRECHA.md` que faltaba (2026-09-18).
//!
//! === El orden, que es del estandar y no de quien escribe ===
//!
//! [class.base.init] fija lo que hace un constructor ANTES de su cuerpo, y en
//! este orden:
//!
//!   1. construir la BASE -- con el constructor que nombre la lista
//!      (`: Cuenta(saldo)`) o, si no la nombra, con el suyo sin argumentos;
//!   2. apuntar el `vptr` a la tabla de ESTA clase (lo emite el descenso);
//!   3. los miembros de la lista, **en el orden en que estan DECLARADOS**, no en
//!      el que se escriben;
//!   4. el cuerpo.
//!
//! El 2 va despues del 1 a proposito: mientras corre el constructor de la base,
//! el objeto todavia ES la base, y una llamada virtual desde ahi tiene que ir a
//! la version de la base. Hasta hoy el `vptr` del derivado se ponia antes de
//! llamar a nada, y esa llamada iba al derivado -- a un objeto a medio hacer.
//!
//! === Lo que faltaba sin que nada lo dijera ===
//!
//! `class Ahorro : public Cuenta { }` sin constructor, con `Cuenta` que SI lo
//! tiene: el constructor implicito de `Ahorro` tiene que construir su `Cuenta`.
//! No existia, asi que un `Ahorro` nacia con la base sin construir y el
//! compilador no decia nada. Es la misma familia que el destructor de la base
//! del 17-09 (ver `descenso.rs`), del otro lado de la vida del objeto.
//!
//! Vive aqui y no en `parser.rs` porque aquel esta en la linea base de L6a y
//! solo puede encoger -- el mismo corte que `sobrecarga.rs`.

use super::*;

impl Parser {
    /// Elige el constructor de `cls` para estos argumentos.
    ///
    /// `None` significa *"esta clase no tiene constructor"*, que es legal y
    /// deja el objeto sin inicializar -- igual que un `struct` de C. Pedir
    /// argumentos a una clase sin constructor si es error. Desde el 2026-09-18
    /// una clase sin constructor cuya base SI tiene recibe uno implicito
    /// (`ctor_implicito`), asi que `None` ya no deja una base sin construir.
    ///
    /// Vivia en `parser.rs`; vino aqui con el resto de la construccion.
    pub(super) fn resolver_ctor(&self, cls: &str, args: &[Expr]) -> Result<Option<String>, CppError> {
        let Some(info) = self.clases.get(cls) else {
            return Err(self.err(format!("la clase `{cls}` no esta definida")));
        };
        if info.constructores.is_empty() {
            if !args.is_empty() {
                return Err(self.err(format!(
                    "`{cls}` no tiene constructor, asi que no acepta argumentos")));
            }
            return Ok(None);
        }
        let firmas = info.constructores.clone();
        let tipos = self.tipos_de(args, cls)?;
        Ok(Some(self.resolver(cls, &firmas, &tipos)?.simbolo.clone()))
    }

    /// Vuelta 1: la lista se SALTA, como el cuerpo. Sus expresiones se leen en
    /// la vuelta 2, cuando la disposicion de la clase ya existe.
    pub(super) fn saltar_lista_ini(&mut self) -> Result<(), CppError> {
        let mut profundidad = 0i32;
        loop {
            match self.peek() {
                Token::OpenParen => profundidad += 1,
                Token::CloseParen => profundidad -= 1,
                Token::OpenBrace if profundidad == 0 => return Ok(()),
                Token::Eof => return Err(self.err("la lista de inicializacion no llega nunca al cuerpo `{`")),
                _ => {}
            }
            self.avanzar();
        }
    }

    /// Vuelta 2: lo que un constructor hace antes de su cuerpo. Se llama con
    /// `this` y los parametros ya en el ambito.
    pub(super) fn iniciales(&mut self, clase: &str) -> Result<Iniciales, CppError> {
        let mut ini = if *self.peek() == Token::Colon { self.lista_ini(clase)? } else { Iniciales::default() };
        if ini.base.is_none() {
            ini.base = self.base_por_defecto(clase)?;
        }
        Ok(ini)
    }

    /// `: Base(a), x(0), y(a + 1)`.
    fn lista_ini(&mut self, clase: &str) -> Result<Iniciales, CppError> {
        self.exige(&Token::Colon)?;
        let info = self.clases.get(clase).cloned()
            .ok_or_else(|| self.err(format!("la clase `{clase}` no esta definida")))?;
        let base = info.base.clone();
        // Solo se inicializan aqui los miembros PROPIOS: los de la base ya los
        // inicializo el constructor de la base, que corre antes.
        let heredados: Vec<String> = base.as_ref()
            .and_then(|b| self.clases.get(b))
            .map(|p| p.campos.iter().map(|(n, _, _)| n.clone()).collect())
            .unwrap_or_default();
        let mut ini = Iniciales::default();
        let mut miembros: Vec<(usize, Expr)> = Vec::new();
        let mut nombrados: Vec<String> = Vec::new();
        loop {
            let n = match self.avanzar() {
                Token::Ident(n) => n,
                otro => return Err(self.err(format!(
                    "se esperaba la base o un miembro en la lista de inicializacion y vino {otro:?}"))),
            };
            if nombrados.contains(&n) {
                return Err(self.err(format!("`{n}` aparece dos veces en la lista de inicializacion")));
            }
            nombrados.push(n.clone());
            self.exige(&Token::OpenParen)?;
            let mut args = Vec::new();
            if *self.peek() != Token::CloseParen {
                loop {
                    args.push(self.asignacion()?);
                    if !self.come(&Token::Comma) { break; }
                }
            }
            self.exige(&Token::CloseParen)?;

            if Some(&n) == base.as_ref() {
                // La sobrecarga la resuelve la misma regla que `Base b(args);`.
                ini.base = self.resolver_ctor(&n, &args)?.map(|s| (s, args));
            } else if heredados.contains(&n) {
                return Err(self.err(format!(
                    "`{n}` es un campo de la base: lo inicializa el constructor de la base, \
                     no la lista de `{clase}`")));
            } else if let Some(i) = info.campos.iter().position(|(m, _, _)| *m == n && m != VPTR) {
                let (_, off, t) = info.campos[i].clone();
                if matches!(t, TypeSpec::ClassRef(_)) {
                    return Err(self.pendiente("inicializar en la lista un miembro que es un objeto", 5));
                }
                let valor = match args.len() {
                    // `x()` es value-initialization: el cero de su tipo.
                    0 if matches!(t, TypeSpec::Float | TypeSpec::Double) => Expr::FloatLit(0.0),
                    0 => Expr::Int(0),
                    1 => args.pop().unwrap(),
                    k => return Err(self.err(format!(
                        "el miembro `{n}` se inicializa con UN valor, y la lista le da {k}"))),
                };
                miembros.push((i, Expr::AssignArrow(Box::new(Expr::This), n, off, t, Box::new(valor))));
            } else {
                return Err(self.err(format!("`{n}` no es la base ni un miembro de `{clase}`")));
            }
            if !self.come(&Token::Comma) { break; }
        }
        // ** El orden de DECLARACION, no el de la lista ([class.base.init]/13).
        miembros.sort_by_key(|(i, _)| *i);
        ini.miembros = miembros.into_iter().map(|(_, e)| e).collect();
        Ok(ini)
    }

    /// La base que nadie nombro: se construye con su constructor SIN
    /// argumentos, y si no tiene uno asi es un error -- no se deja sin
    /// construir en silencio.
    fn base_por_defecto(&self, clase: &str) -> Result<Option<(String, Vec<Expr>)>, CppError> {
        let Some(b) = self.clases.get(clase).and_then(|c| c.base.clone()) else { return Ok(None) };
        if self.clases.get(&b).map_or(true, |c| c.constructores.is_empty()) {
            return Ok(None);
        }
        match self.resolver_ctor(&b, &[]) {
            Ok(s) => Ok(s.map(|s| (s, Vec::new()))),
            Err(_) => Err(self.err(format!(
                "`{clase}` no construye a su base `{b}` por su nombre, y `{b}` no tiene constructor \
                 sin argumentos: escribe `{clase}(...) : {b}(...) {{ ... }}`"))),
        }
    }

    /// El constructor IMPLICITO de una clase que no declara ninguno y cuya base
    /// SI tiene: solo construye la base. Se registra como una firma mas, asi
    /// que `D d;` lo encuentra por la sobrecarga de siempre.
    pub(super) fn ctor_implicito(&mut self, clase: &str) -> Result<Option<Method>, CppError> {
        if self.clases.get(clase).map_or(true, |c| !c.constructores.is_empty()) {
            return Ok(None);
        }
        let Some(base) = self.base_por_defecto(clase)? else { return Ok(None) };
        let simbolo = crate::mangling::constructor(&self.espacios, clase, &[]);
        self.retornos.insert(simbolo.clone(), TypeSpec::Void);
        if let Some(c) = self.clases.get_mut(clase) {
            c.constructores.push(Firma { params: Vec::new(), ret: TypeSpec::Void, simbolo });
        }
        Ok(Some(Method {
            name: clase.to_string(), ret_type: TypeSpec::Void, params: Vec::new(),
            body: Vec::new(), is_virtual: false, is_override: false, is_const: false,
            access: Access::Public, class_name: clase.to_string(),
            iniciales: Iniciales { base: Some(base), miembros: Vec::new() },
        }))
    }
}
