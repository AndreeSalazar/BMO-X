//! `ir::descenso` -- COMO SE LLEGA A UNA INSTRUCCION.
//!
//! ## Por que soy un fichero y no un trozo del de al lado (L6a)
//!
//! El corte ya estaba escrito, y lo escribio `forma.rs` cuando SE partio a si
//! mismo:
//!
//! ```text
//!    forma      QUE es una instruccion       -> tipos, y cero decisiones
//!    descenso   COMO se llega a una          -> recorre el arbol y decide
//! ```
//!
//! Faltaba la otra mitad. `mod.rs` se comio el `Descenso` entero --el recorrido,
//! los locales, los temporales, las once decisiones de bajada-- y llego a 1.416
//! lineas. El guardian L6a lo tumbo el 2026-08-23.
//!
//! ** Y el reparto es MECANICO, no un rediseno: se movio texto. Lo que queda en
//! `mod.rs` es la puerta de entrada --`bajar`, `bajar_con`,
//! `metal_que_declara`-- y las funciones libres que deciden que regla toca. Lo
//! que vive aqui es quien recorre.
//!
//! [!] Es la SEGUNDA vez que este modulo se parte por la misma razon, y la
//! segunda vez el corte estaba nombrado de antes. Un fichero que crece por los
//! lados casi siempre sabe por donde se rompe: lo dice su propio indice.

use super::*;

pub(super) struct Descenso<'t> {
    pub(super) instrucciones: Vec<Instr>,
    pub(super) siguiente_temporal: u32,
    pub(super) siguiente_etiqueta: u32,
    pub(super) locales: Vec<String>,
    pub(super) textos: &'t mut Vec<String>,
    /// Los congelados del modulo, para poder ANADIR el de un literal de texto.
    ///
    /// ** Es `&mut` y las tablas constantes no lo son porque las tablas se
    /// conocen enteras antes de bajar nada --son declaraciones-- y un literal
    /// aparece **dentro de una expresion**, que es aqui.
    pub(super) congelados: &'t mut Vec<crate::ir::forma::Congelado>,
    /// Indice del pozo de textos -> indice en `congelados`.
    ///
    /// [!] No es la identidad y no se puede suponer: en `congelados` ya viven
    /// las tablas constantes, que se declararon antes. Suponerlo cargaria la
    /// direccion de una tabla creyendo que era un texto -- que compila.
    pub(super) textos_congelados: &'t mut Vec<u32>,
    /// **Las constantes CONGELADAS de este modulo.**
    ///
    /// ** Se resuelven aqui, en la IR, y no en el emisor: una constante vale lo
    /// mismo en toda maquina. Es la misma decision que ya estaba tomada para las
    /// constantes del ABI unas lineas mas abajo.
    pub(super) congeladas: &'t std::collections::HashMap<String, Const>,
    /// Las tablas congeladas, por nombre -> indice en `ModuloIr::congelados`.
    pub(super) tablas: &'t std::collections::HashMap<String, u32>,
    /// Donde salta un `corta` y donde un `continua`, de fuera a dentro.
    pub(super) bucles: Vec<(Etiqueta, Etiqueta)>,
    pub(super) tabla: &'t crate::tablas::Modulos,
    pub(super) plano: &'t crate::disposicion::Plano,
    /// ** Que perfil, y hace falta por UNA sola pregunta: que es `3.5`.
    ///
    /// En `llano` es un `flotante64` --binario, ocho bytes-- porque `decimal`
    /// esta prohibido alli por no decir su medida. En `pleno` es un decimal
    /// EXACTO, y convertirlo a binario para "ya tenerlo hecho" perderia la
    /// exactitud que el lenguaje promete en la portada.
    ///
    /// El mismo caracter en el fuente, dos cosas distintas, y lo decide una
    /// palabra escrita en la primera linea del fichero. Es la unica vez que el
    /// perfil cambia lo que SIGNIFICA algo en vez de lo que se permite.
    pub(super) perfil: crate::arbol::Perfil,
    /// Los nombres que son UNA INSTRUCCION de la maquina, no una funcion.
    ///
    /// ## ** Por que llegan de fuera y no se buscan aqui
    ///
    /// Porque este modulo no puede nombrar una maquina, y ese es todo el
    /// asunto. La lista se monta arriba, a partir de lo que el FUENTE declaro
    /// con `usa` -- asi que el nombre de la maquina lo escribio el usuario, no
    /// el compilador.
    ///
    /// ** Y sin esta lista pasaba lo peor que puede pasar: `lee_reloj()` se
    /// bajaba a una LLAMADA a un simbolo que no existe. Compilaba, pasaba el
    /// analisis de nombres --porque el nombre existe en la tabla--, pasaba el
    /// de perfiles, y el binario saltaba a la nada. La tabla de la maquina
    /// estaba entera y no la leia nadie a la hora de emitir.
    pub(super) metal: &'t [String],
    /// Los tipos declarados de la funcion que se esta bajando.
    pub(super) tipos: std::collections::HashMap<String, crate::arbol::Tipo>,
    /// Cuanto mide cada local, en el mismo orden que `locales`.
    pub(super) medidas_locales: Vec<u32>,
    /// **Cuantos literales de lista se quedaron sin construir** por no saber el
    /// ancho de su elemento.
    ///
    /// ** Se cuenta en vez de callar. Un `[1, 2, 3]` que baja a `nada` es
    /// exactamente la firma de fallo que este proyecto persigue -- y `Const::
    /// Texto` estuvo bajando a un cero durante meses precisamente porque el
    /// emisor lo confesaba y nadie mas.
    pub(super) sin_ancho: usize,
}

impl<'t> Descenso<'t> {
    pub(super) fn nueva(
        textos: &'t mut Vec<String>,
        congelados: &'t mut Vec<crate::ir::forma::Congelado>,
        textos_congelados: &'t mut Vec<u32>,
        congeladas: &'t std::collections::HashMap<String, Const>,
        tablas: &'t std::collections::HashMap<String, u32>,
        tabla: &'t crate::tablas::Modulos,
        plano: &'t crate::disposicion::Plano,
        perfil: crate::arbol::Perfil,
        metal: &'t [String],
    ) -> Self {
        Self {
            instrucciones: Vec::new(),
            siguiente_temporal: 0,
            siguiente_etiqueta: 0,
            locales: Vec::new(),
            textos,
            congelados,
            textos_congelados,
            congeladas,
            tablas,
            bucles: Vec::new(),
            tabla,
            plano,
            perfil,
            metal,
            tipos: std::collections::HashMap::new(),
            sin_ancho: 0,
            medidas_locales: Vec::new(),
        }
    }

    /// **Construye una `lista de T` a partir de su literal**, si se puede.
    ///
    /// Devuelve `None` cuando no hay tipo escrito de donde sacar el ancho, y
    /// entonces el camino normal sigue -- que hoy no emite la lista y lo dice.
    ///
    /// ```text
    ///    m = monton de la tarea
    ///    l = lista_nueva(m, cuantos, ancho)
    ///    agrega(l, x0, ancho)
    ///    agrega(l, x1, ancho)
    /// ```
    ///
    /// ** La capacidad es EXACTA --los elementos que hay-- y no un numero con
    /// holgura. Un literal se escribe entero: si despues crece, crecer es otra
    /// operacion y ya tiene su propio "no cabe". Reservar de mas "por si acaso"
    /// seria una politica de crecimiento metida donde no toca.
    /// **Construye una `tabla de A a B` a partir de su literal**, si se puede.
    ///
    /// ```text
    ///    m = monton de la tarea
    ///    t = tabla_nueva(m, capacidad)
    ///    pon(t, clave0, valor0)
    ///    pon(t, clave1, valor1)
    /// ```
    ///
    /// *** LA CAPACIDAD ES EL DOBLE DE LAS PAREJAS, y aqui si hay holgura -- al
    /// reves que en la lista, donde es exacta.
    ///
    /// ** El motivo no es "por si crece": es que una tabla llena **no termina de
    /// buscar**. Con el doble, la ocupacion queda al 50% y la sonda lineal
    /// encuentra un hueco enseguida. Ajustarla seria pagar colisiones en cada
    /// busqueda para ahorrar unos bytes una vez.
    ///
    /// [!] Y el minimo es 2: una tabla de una ranura no admitiria ni una pareja,
    /// porque siempre queda una libre.
    pub(super) fn tabla_literal(&mut self, destino: &Expr, valor: &Expr) -> Option<Valor> {
        let Expr::Tabla(pares, _) = valor else {
            return None;
        };
        // Que el destino sea una tabla es lo unico que hace falta saber: la
        // clave es un `texto` y el valor cabe en una palabra.
        if !matches!(
            self.plano.tipo_de(destino, &self.tipos),
            Some(crate::arbol::Tipo::Tabla(_, _))
        ) {
            return None;
        }
        let capacidad = (pares.len() as i64 * 2).max(2);

        let m = self.temporal();
        self.pon(Instr::MontonDeLaTarea { destino: m });
        let t = self.temporal();
        self.pon(Instr::Llama {
            destino: Some(t),
            que: Valor::Nombre("tabla_nueva".to_string()),
            argumentos: vec![Valor::Temporal(m), Valor::Const(Const::Entero(capacidad))],
        });
        // [!] CLAVE Y LUEGO VALOR, en el orden escrito. La Regla 8 fija el orden
        // de evaluacion, y una tabla literal no puede tener otro que el de al
        // lado por el hecho de ser una tabla.
        for (k, v) in pares {
            let ck = self.expresion(k);
            let cv = self.expresion(v);
            self.pon(Instr::Llama {
                destino: None,
                que: Valor::Nombre("pon".to_string()),
                argumentos: vec![Valor::Temporal(t), ck, cv],
            });
        }
        Some(Valor::Temporal(t))
    }

    pub(super) fn lista_literal(&mut self, destino: &Expr, valor: &Expr) -> Option<Valor> {
        let Expr::Lista(elementos, _) = valor else {
            return None;
        };
        // El ancho sale del tipo del DESTINO, que es el unico sitio donde esta
        // escrito. Un `Expr::Lista` no lo lleva y no puede llevarlo.
        let ancho = self.ancho_de_lista(destino)?;

        let m = self.temporal();
        self.pon(Instr::MontonDeLaTarea { destino: m });
        let l = self.temporal();
        self.pon(Instr::Llama {
            destino: Some(l),
            que: Valor::Nombre("lista_nueva".to_string()),
            argumentos: vec![
                Valor::Temporal(m),
                Valor::Const(Const::Entero(elementos.len() as i64)),
                Valor::Const(Const::Entero(ancho as i64)),
            ],
        });
        // [!] EN ORDEN, y de izquierda a derecha. `agrega` pone al final, asi
        // que el orden de estas llamadas ES el orden de la lista -- y ademas es
        // el orden en que la Regla 8 dice que se evaluan los elementos. Las dos
        // cosas coinciden aqui por suerte, y se escribe para que el dia que
        // dejen de coincidir alguien lo vea.
        for x in elementos {
            let v = self.expresion(x);
            self.pon(Instr::Llama {
                destino: None,
                que: Valor::Nombre("agrega".to_string()),
                argumentos: vec![
                    Valor::Temporal(l),
                    v,
                    Valor::Const(Const::Entero(ancho as i64)),
                ],
            });
        }
        Some(Valor::Temporal(l))
    }

    /// **Si esto es una `lista de T`, cuanto mide su elemento.**
    ///
    /// ** El ancho sale del TIPO y no de la lista: `lista de entero64` mide
    /// ocho y `lista de natural8` mide uno, y el objeto en el monton no lo
    /// guarda. La tabla de tipos --`SectionKind::TypeMap = 0x10`-- sigue siendo
    /// el quinto hueco declarado y vacio del formato, asi que mientras tanto la
    /// dependencia se lleva VISIBLE: por la firma de `sitio_de`.
    pub(super) fn ancho_de_lista(&self, e: &Expr) -> Option<u32> {
        match self.plano.tipo_de(e, &self.tipos)? {
            crate::arbol::Tipo::Lista(dentro) => self.plano.medida_de(&dentro),
            _ => None,
        }
    }

    /// Es esta expresion un `numero`? -- el decimal exacto de `pleno`.
    pub(super) fn es_numero(&self, e: &Expr) -> bool {
        matches!(
            self.plano.tipo_de(e, &self.tipos),
            Some(crate::arbol::Tipo::Nombre(ref n)) if n == "numero" || n == "decimal"
        )
    }

    /// Es esta expresion un `texto`?
    ///
    /// ** Se pregunta sobre el ARBOL y no sobre el valor ya bajado, por lo mismo
    /// que la clase y el signo: un valor no dice de que tipo era.
    pub(super) fn es_texto(&self, e: &Expr) -> bool {
        matches!(
            self.plano.tipo_de(e, &self.tipos),
            Some(crate::arbol::Tipo::Nombre(ref n)) if n == "texto"
        )
    }

    /// La direccion de un sitio de memoria escrito con `.` o con `[]`.
    ///
    /// ** Devolver la DIRECCION y no el valor es lo que deja usar la misma
    /// cuenta para leer y para escribir. `p.x` y `p.x = 3` calculan
    /// exactamente lo mismo; lo unico que cambia es la instruccion de despues.
    ///
    /// `None` si no se sabe la disposicion -- y entonces no se emite nada,
    /// porque `disposicion` ya lo denuncio. Emitir "algo" para un programa que
    /// esta mal es como un compilador acaba produciendo binarios plausibles.
    pub(super) fn direccion_de(&mut self, e: &Expr) -> Option<(Valor, u32)> {
        match e {
            Expr::Campo { que, nombre, .. } => {
                let t = self.plano.tipo_de(que, &self.tipos)?;
                let crate::arbol::Tipo::Nombre(r) = t else {
                    return None;
                };
                let hueco = self.plano.registro(&r)?.campo(nombre)?;
                let (desplazamiento, medida) = (hueco.desplazamiento, hueco.medida);
                let base = self.expresion(que);
                let t = self.temporal();
                self.pon(Instr::Binaria {
                    destino: t,
                    op: Op::Suma,
                    // Una direccion es un entero. Siempre. Aunque lo que haya al
                    // final sea un flotante: `p.x` suma bytes, no numeros.
                    clase: Clase::Entero,
                    // ** Y SIN SIGNO, que es lo mismo por otro lado: una
                    // direccion no puede ser negativa, y compararla con signo
                    // parte el espacio de memoria en dos mundos en cuanto pasa
                    // del bit 63.
                    sin_signo: true,
                    izquierda: base,
                    derecha: Valor::Const(Const::Entero(desplazamiento as i64)),
                });
                Some((Valor::Temporal(t), medida))
            }
            Expr::Indice { que, indice, .. } => {
                let t = self.plano.tipo_de(que, &self.tipos)?;
                let (_, medida) = self.plano.elemento(&t)?;
                let base = self.expresion(que);
                let i = self.expresion(indice);
                // indice * medida
                let paso = self.temporal();
                self.pon(Instr::Binaria {
                    destino: paso,
                    op: Op::Por,
                    clase: Clase::Entero,
                    sin_signo: true, // aritmetica de direcciones: ver arriba
                    izquierda: i,
                    derecha: Valor::Const(Const::Entero(medida as i64)),
                });
                let t = self.temporal();
                self.pon(Instr::Binaria {
                    destino: t,
                    op: Op::Suma,
                    clase: Clase::Entero,
                    sin_signo: true,
                    izquierda: base,
                    derecha: Valor::Temporal(paso),
                });
                Some((Valor::Temporal(t), medida))
            }
            _ => None,
        }
    }

    pub(super) fn temporal(&mut self) -> Temporal {
        let t = Temporal(self.siguiente_temporal);
        self.siguiente_temporal += 1;
        t
    }

    pub(super) fn etiqueta(&mut self) -> Etiqueta {
        let e = Etiqueta(self.siguiente_etiqueta);
        self.siguiente_etiqueta += 1;
        e
    }

    pub(super) fn local(&mut self, nombre: &str) -> Local {
        match self.locales.iter().position(|n| n == nombre) {
            Some(i) => Local(i as u32),
            None => {
                self.locales.push(nombre.to_string());
                // *** Y SE APUNTA CUANTO MIDE, que hasta hoy no hacia falta.
                //
                // El emisor le daba UNA PALABRA a cada local. Valia mientras
                // todo lo que vivia en una cupiera en ocho bytes -- y `numero`
                // mide 16, asi que su segunda mitad se habria comido la de al
                // lado **en silencio**.
                //
                // ** La medida sale del TIPO, no de la maquina, y por eso la
                // sabe el frontend: `disposicion` ya la calculo. Lo que sigue
                // siendo del emisor es donde cae cada una.
                let medida = self
                    .tipos
                    .get(nombre)
                    .and_then(|t| self.plano.medida_de(t))
                    .unwrap_or(0);
                self.medidas_locales.push(medida);
                Local((self.locales.len() - 1) as u32)
            }
        }
    }

    /// **Una local sin nombre, de `medida` bytes.**
    ///
    /// ** Hace falta desde que una operacion puede producir un valor que no cabe
    /// en un registro: `a + b` de dos `numero` da 16 bytes, y esos tienen que
    /// vivir en algun sitio del marco. Un temporal no sirve -- un temporal es
    /// una palabra, y esa es toda su definicion.
    ///
    /// [!] El nombre lleva un caracter que el lexer no deja escribir, asi que no
    /// puede chocar con una local del programa. Es la misma treta que el pozo de
    /// textos usa para sus congelados, y por el mismo motivo.
    pub(super) fn local_anonima(&mut self, medida: u32) -> Local {
        let nombre = format!(" tmp{}", self.locales.len());
        self.locales.push(nombre);
        self.medidas_locales.push(medida);
        Local((self.locales.len() - 1) as u32)
    }

    pub(super) fn busca_local(&self, nombre: &str) -> Option<Local> {
        self.locales
            .iter()
            .position(|n| n == nombre)
            .map(|i| Local(i as u32))
    }

    pub(super) fn pon(&mut self, i: Instr) {
        self.instrucciones.push(i);
    }

    pub(super) fn funcion(mut self, f: &arbol::Funcion) -> FuncionIr {
        // Los tipos escritos de esta funcion. Sin esto, `p.x` no sabe de que
        // registro es `p` -- y esa es toda la informacion que hace falta.
        // *** CON DEDUCCION, y esto era un agujero de medio dia (2026-08-23).
        //
        // Aqui ponia `tipos_de(f)` -- solo los tipos ESCRITOS. Asi que la
        // deduccion que se construyo esta manana la usaba `disposicion` para
        // COMPROBAR y no la usaba la IR para EMITIR: **dos respuestas distintas a
        // "de que tipo es esto" dentro del mismo compilador.**
        //
        // ** Y se noto donde se tenia que notar: `a = "ho"` seguido de `a + b`
        // no bajaba a `junta`, porque aqui `a` no tenia tipo. La comprobacion
        // decia que si y el codigo decia que no -- y el que manda es el codigo.
        self.tipos = crate::disposicion::tipos_de_con(f, Some(self.plano));
        for p in &f.parametros {
            self.local(&p.nombre);
        }
        self.bloque(&f.cuerpo);
        FuncionIr {
            nombre: f.nombre.clone(),
            parametros: f.parametros.len() as u32,
            locales: self.locales.len() as u32,
            medidas_locales: self.medidas_locales,
            temporales: self.siguiente_temporal,
            instrucciones: self.instrucciones,
            sin_ancho: self.sin_ancho,
        }
    }

    fn bloque(&mut self, b: &Bloque) {
        for s in b {
            self.sentencia(s);
        }
    }

    fn sentencia(&mut self, s: &Sent) {
        match s {
            Sent::Asigna { destino, valor, .. } => {
                // *** `notas es lista de entero64 = [1, 2, 3]` (2026-08-23).
                //
                // Se construye AQUI y no en `expresion` por una razon que no es
                // de comodidad: **el ancho del elemento sale del TIPO, y una
                // expresion no sabe adonde va**. `[1, 2, 3]` a secas no dice si
                // sus elementos miden uno, cuatro u ocho -- y sin ese numero no
                // hay lista que reservar.
                //
                // ** La alternativa era deducir el elemento de los literales, y
                // eso tiene reglas propias que nadie ha escrito: que mide
                // `[1, 2.5]`? La deduccion de este compilador ya dejo esa fila
                // fuera a proposito, con su motivo.
                //
                // Asi que se construye donde el tipo ESTA ESCRITO, y donde no lo
                // este sigue sin construirse -- y `expresion` lo dice.
                if let Some(v) = self
                    .lista_literal(destino, valor)
                    .or_else(|| self.tabla_literal(destino, valor))
                {
                    if let Expr::Nombre(n, _) = destino {
                        let l = self.local(n);
                        self.pon(Instr::Guarda { destino: l, valor: v });
                    }
                    return;
                }
                let v = self.expresion(valor);
                match destino {
                    Expr::Nombre(n, _) => {
                        let l = self.local(n);
                        self.pon(Instr::Guarda {
                            destino: l,
                            valor: v,
                        });
                    }
                    // ** `p.x = 3` y `a[i] = 3`: la MISMA cuenta que al leer, y
                    // por eso comparten `direccion_de`. Lo unico que cambia es
                    // la instruccion del final.
                    Expr::Campo { .. } | Expr::Indice { .. } => {
                        if let Some((direccion, ancho)) = self.direccion_de(destino) {
                            self.pon(Instr::Escribe {
                                direccion,
                                valor: v,
                                ancho,
                            });
                        }
                        // Si no se supo la direccion, no se emite nada:
                        // `disposicion` ya lo denuncio y aqui no hay nada que
                        // inventar.
                    }
                    // Asignar a otra cosa no es asignar a nada: es una forma
                    // que la gramatica no deberia haber dejado pasar.
                    _ => {}
                }
            }
            Sent::Si { ramas, sino, .. } => self.si(ramas, sino.as_ref()),
            Sent::Repite { forma, cuerpo, .. } => self.repite(forma, cuerpo),
            Sent::ParaCada { .. } => {
                // Recorrer pide saber como esta hecha una lista, y eso es el
                // runtime. Cuando exista, esto se convierte en un bucle con
                // llamadas a `siguiente`.
            }
            Sent::Devuelve { valor, .. } => {
                let v = valor.as_ref().map(|e| self.expresion(e));
                self.pon(Instr::Devuelve(v));
            }
            Sent::Falla { motivo, .. } => {
                let v = self.expresion(motivo);
                self.pon(Instr::Devuelve(Some(v)));
            }
            Sent::Corta(_) => {
                if let Some((salida, _)) = self.bucles.last().copied() {
                    self.pon(Instr::Salta(salida));
                }
            }
            Sent::Continua(_) => {
                if let Some((_, vuelta)) = self.bucles.last().copied() {
                    self.pon(Instr::Salta(vuelta));
                }
            }
            Sent::Crudo { cuerpo, .. } | Sent::Paralelo { cuerpo, .. } => self.bloque(cuerpo),
            Sent::Expresion(e) => {
                self.expresion(e);
            }
        }
    }

    fn si(&mut self, ramas: &[(Expr, Bloque)], sino: Option<&Bloque>) {
        let fin = self.etiqueta();
        for (cond, cuerpo) in ramas {
            let cierto = self.etiqueta();
            let siguiente = self.etiqueta();
            let v = self.expresion(cond);
            self.pon(Instr::SaltaSi {
                cond: v,
                cierto,
                falso: siguiente,
            });
            self.pon(Instr::Etiqueta(cierto));
            self.bloque(cuerpo);
            self.pon(Instr::Salta(fin));
            self.pon(Instr::Etiqueta(siguiente));
        }
        if let Some(c) = sino {
            self.bloque(c);
        }
        self.pon(Instr::Etiqueta(fin));
    }

    fn repite(&mut self, forma: &Repeticion, cuerpo: &Bloque) {
        let vuelta = self.etiqueta();
        let dentro = self.etiqueta();
        let salida = self.etiqueta();

        self.pon(Instr::Etiqueta(vuelta));
        match forma {
            Repeticion::Siempre => {}
            Repeticion::Mientras(cond) => {
                let v = self.expresion(cond);
                self.pon(Instr::SaltaSi {
                    cond: v,
                    cierto: dentro,
                    falso: salida,
                });
                self.pon(Instr::Etiqueta(dentro));
            }
            Repeticion::Veces(_) => {
                // Un contador pide una local anonima y un tipo entero. Se hara
                // con el emisor, que es quien sabe los anchos.
            }
        }

        self.bucles.push((salida, vuelta));
        self.bloque(cuerpo);
        self.bucles.pop();

        self.pon(Instr::Salta(vuelta));
        self.pon(Instr::Etiqueta(salida));
    }

}