//! `arquitectura` -- lo que INTI sabe de una maquina, que es solo lo que le
//! cuenta un fichero.
//!
//! ## La idea, y es de Eddi
//!
//! > *"pensaba que podrias crear una libreria `usar <la maquina>`; esa es
//! > libreria PARA ahorrar esfuerzo, una sola llamada, y asi facilitar la
//! > escritura en INTI"*
//!
//! Una linea, `usa <la maquina>`, y el programa tiene sus nombres sin declarar
//! nada.
//!
//! (La cita original decia el nombre de una maquina concreta. Se parafrasea
//! porque `tests/agnostico.rs` tampoco deja nombrarla en un comentario, y esa
//! severidad es a proposito: **si no se puede explicar este modulo sin nombrar
//! una maquina, es que el modulo sabe demasiado**. Se puede.)
//!
//! ## Y por que arregla mas de lo que parece
//!
//! **El nombre del modulo es la declaracion de que el fichero no es portable.**
//! Antes se escribia `usa metal`, que esconde la arquitectura -- el metal de
//! que maquina? Con `usa x86_64`:
//!
//! - el compilador la **cuenta**, igual que cuenta los `crudo`;
//! - compilando para otra maquina, **la linea falla con un mensaje claro** en
//!   vez de con un nombre desconocido a mitad de fichero;
//! - y el fichero se lee de arriba abajo sabiendo a que se ata.
//!
//! ## Lo que este modulo NO sabe
//!
//! **Los bytes.** Estan en `intrinsics.toml`, que es la fuente y la comparte
//! BMO C. Aqui solo llega el nombre y si alguien comprueba al otro lado. Dos
//! declaraciones de los mismos bytes acabarian discrepando -- y ademas el test
//! de agnosticismo prohibe que este crate escriba los bytes de una instruccion.
//!
//! ## ** Y por que NO hay una maquina de respaldo dentro del binario
//!
//! La hubo durante media hora: una tabla incrustada con `include_str!` para que
//! el compilador arrancara sin raices, igual que hace `palabras`. **El test de
//! agnosticismo la tumbo**, y tenia razon: para tener respaldo hay que nombrar
//! a la favorita, y nombrar a una favorita es justo lo que este lenguaje no
//! hace.
//!
//! La diferencia con las palabras es real: sin vocabulario el compilador no
//! puede ni leer una linea, asi que ahi el respaldo se gana el sitio. Sin
//! arquitectura si puede -- solo significa que ese `usa` no encuentra nada, **y
//! eso es una respuesta correcta**, no un fallo.
//!
//! Es un ejemplo pequeno de lo que la regla evita: el respaldo era comodo, no
//! era urgente, y era la puerta por la que una maquina concreta se colaba en el
//! compilador.

use std::collections::{HashMap, HashSet};

use bmo_mods::Roots;

/// Lo que INTI necesita de una maquina.
#[derive(Debug, Clone)]
pub struct Maquina {
    nombre: String,
    /// nombre en INTI -> nombre del intrinseco en `intrinsics.toml`.
    nombres: HashMap<String, String>,
    piden_crudo: HashSet<String>,
    ancho_de_puntero: u32,
    alineacion_maxima: u32,
    /// nombre -> numero, para todos los registros que la maquina declara.
    registros: HashMap<String, u8>,
    /// Los que el emisor puede repartir entre temporales, en orden.
    ///
    /// ** Sale de la tabla y no del emisor. El asignador de F3 llevaba esta
    /// lista escrita a mano en Rust durante unas horas, y no habia motivo:
    /// **anadir una instruccion es una fila de TOML, y un registro tambien
    /// deberia serlo**.
    temporales: Vec<String>,
    trabajo: Vec<String>,
    /// Como se cruza la puerta del sistema EN ESTA MAQUINA.
    ///
    /// ** Fijate en lo que este campo NO es: no es la lista de lo que se puede
    /// pedir al kernel. Eso vive en `modulos.toml`, es agnostico, y un programa
    /// lo pide con `usa bmo`. Esto es solo **donde van los argumentos aqui**.
    ///
    /// Separarlo asi es lo que deja que `invoca(cap, op, a, b, c)` sea la misma
    /// linea en toda maquina teniendo cada maquina su propia respuesta.
    puerta: Option<Puerta>,
}

/// El COMO de la puerta del sistema, leido de la tabla de la maquina.
///
/// Los nombres de los campos son agnosticos a proposito --"numero",
/// "argumentos", "resultado"-- porque la pregunta *"por donde va el tercer
/// argumento?"* la tiene toda maquina. Lo que cambia es la respuesta, y la
/// respuesta es un dato.
#[derive(Debug, Clone)]
pub struct Puerta {
    /// Donde va el numero de la puerta.
    pub numero: String,
    /// Por donde van los argumentos, en orden.
    pub argumentos: Vec<String>,
    /// Por donde vuelve la respuesta.
    pub resultado: String,
    /// Lo que la puerta se lleva por delante aunque nadie se lo pida.
    pub destruye: Vec<String>,
}

impl Maquina {
    /// Busca una maquina por nombre en las raices de `bmo-mods`.
    ///
    /// Devuelve `None` si no existe, y **eso es una respuesta util**: es lo que
    /// deja que un `usa` de una maquina que no esta diga *"no conozco esa"* en
    /// vez de dejar el fichero lleno de nombres desconocidos.
    pub fn buscar(raices: &Roots, nombre: &str) -> Option<Self> {
        // Un nombre de arquitectura no puede traer separadores: se usa para
        // construir una ruta, y `usa ../../etc` no es una arquitectura.
        if !nombre
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return None;
        }

        let rel = format!("arch/{}/inti.toml", nombre);
        let texto = raices
            .locate(&rel)
            .and_then(|p| std::fs::read_to_string(p).ok())?;
        Self::desde_texto(&texto)
    }

    fn desde_texto(t: &str) -> Option<Self> {
        let raiz: toml::Value = t.parse().ok()?;
        let nombre = raiz
            .get("meta")?
            .get("arquitectura")?
            .as_str()?
            .to_string();

        let mut nombres = HashMap::new();
        if let Some(tabla) = raiz.get("nombres").and_then(|v| v.as_table()) {
            for (k, v) in tabla {
                if let Some(s) = v.as_str() {
                    nombres.insert(k.clone(), s.to_string());
                }
            }
        }

        let piden_crudo = raiz
            .get("crudo")
            .and_then(|c| c.get("piden"))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let numero = |clave: &str, por_defecto: u32| -> u32 {
            raiz.get("maquina")
                .and_then(|m| m.get(clave))
                .and_then(|v| v.as_integer())
                .map(|n| n as u32)
                .unwrap_or(por_defecto)
        };

        let mut registros = HashMap::new();
        if let Some(t) = raiz.get("registros").and_then(|v| v.as_table()) {
            for (k, v) in t {
                if let Some(n) = v.get("n").and_then(|x| x.as_integer()) {
                    registros.insert(k.clone(), n as u8);
                }
            }
        }

        let lista = |clave: &str| -> Vec<String> {
            raiz.get("reparto")
                .and_then(|r| r.get(clave))
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        };

        Some(Self {
            nombre,
            nombres,
            piden_crudo,
            ancho_de_puntero: numero("ancho_de_puntero", 8),
            alineacion_maxima: numero("alineacion_maxima", 16),
            registros,
            temporales: lista("temporales"),
            trabajo: lista("trabajo"),
            puerta: raiz.get("puerta").and_then(|p| {
                let texto = |c: &str| p.get(c).and_then(|v| v.as_str()).map(String::from);
                let vector = |c: &str| -> Vec<String> {
                    p.get(c)
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default()
                };
                // Sin `numero` y sin `resultado` no hay puerta que emitir. Se
                // devuelve `None` en vez de rellenar huecos: un emisor que cree
                // saber cruzar y no sabe produce bytes que corren y hacen otra
                // cosa, que es peor que no compilar.
                Some(Puerta {
                    numero: texto("numero")?,
                    resultado: texto("resultado")?,
                    argumentos: vector("argumentos"),
                    destruye: vector("destruye"),
                })
            }),
        })
    }

    pub fn nombre(&self) -> &str {
        &self.nombre
    }

    /// Cuantos nombres trae `usa <esta maquina>`.
    pub fn cuantos_nombres(&self) -> usize {
        self.nombres.len()
    }

    /// Todos los nombres que trae, para que el analisis de nombres los de por
    /// declarados.
    pub fn nombres_que_trae(&self) -> Vec<String> {
        self.nombres.keys().cloned().collect()
    }

    /// Este nombre lo trae esta maquina?
    pub fn conoce(&self, nombre: &str) -> bool {
        self.nombres.contains_key(nombre)
    }

    /// Que instruccion hay detras. El compilador no la interpreta: se la pasa a
    /// quien sabe leer `intrinsics.toml`.
    pub fn instruccion(&self, nombre: &str) -> Option<&str> {
        self.nombres.get(nombre).map(|s| s.as_str())
    }

    /// Al otro lado de esto, hay alguien que comprueba?
    pub fn pide_crudo(&self, nombre: &str) -> bool {
        self.piden_crudo.contains(nombre)
    }

    /// El numero de un registro por su nombre.
    pub fn registro(&self, nombre: &str) -> Option<u8> {
        self.registros.get(nombre).copied()
    }

    /// Cuantos registros declara la maquina.
    pub fn cuantos_registros(&self) -> usize {
        self.registros.len()
    }

    /// ** Los que el emisor reparte entre temporales, ya como numeros.
    ///
    /// El emisor no decide cuales son: los LEE. El dia que `bmo_lower` traiga
    /// los prefijos de `r8`..`r15`, esta lista crece en la tabla **y no cambia
    /// una linea de codigo**.
    pub fn temporales(&self) -> Vec<u8> {
        self.temporales
            .iter()
            .filter_map(|n| self.registro(n))
            .collect()
    }

    /// Los de trabajo: donde se hace toda operacion binaria.
    pub fn trabajo(&self) -> Vec<u8> {
        self.trabajo.iter().filter_map(|n| self.registro(n)).collect()
    }

    /// Como se cruza la puerta aqui. `None` si esta maquina no lo declara --
    /// que es una maquina en la que INTI puede compilar `llano` pero no puede
    /// hablar con ningun sistema, y eso hay que poder distinguirlo.
    pub fn puerta(&self) -> Option<&Puerta> {
        self.puerta.as_ref()
    }

    pub fn ancho_de_puntero(&self) -> u32 {
        self.ancho_de_puntero
    }

    pub fn alineacion_maxima(&self) -> u32 {
        self.alineacion_maxima
    }
}
