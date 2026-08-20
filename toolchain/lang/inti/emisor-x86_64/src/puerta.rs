//! La puerta del sistema: de la receta de la tabla a registros de verdad.
//!
//! ## Por que esto es un fichero y no seis lineas dentro del emisor
//!
//! Porque cruzar la puerta es lo unico que hace un programa de INTI que **no
//! puede comprobarse leyendo el binario**. Un `si` que no bifurca se ve
//! corriendolo; una puerta cruzada con el cuarto argumento en el registro
//! equivocado **corre igual de bien** y le entrega al kernel un numero que no
//! es. El kernel valida la capability, no la mano de quien la puso.
//!
//! Asi que el sitio donde se decide eso merece un fichero con su nombre, y
//! merece que la decision **no se tome aqui**: se lee.
//!
//! ## El reparto, otra vez
//!
//! ```text
//!   QUE se puede pedir   lang/inti/modulos.toml, [bmo]   agnostico
//!   COMO se cruza aqui   arch/x86_64/inti.toml, [puerta] de esta maquina
//!   QUE bytes son eso    bmo_lower::x86::syscall         de esta maquina
//! ```
//!
//! Este fichero es el que junta los tres, y **ninguno de los tres esta escrito
//! dentro de el**.

use bmo_inti_front::arquitectura::Maquina;

/// La receta de la puerta, ya traducida a numeros de registro.
#[derive(Debug, Clone)]
pub struct Puerta {
    /// Donde va el numero de la puerta.
    pub numero: u8,
    /// Por donde van los argumentos, en orden.
    pub argumentos: Vec<u8>,
    /// Por donde vuelve la respuesta.
    pub resultado: u8,
}

/// El respaldo, para cuando la tabla no esta a mano.
///
/// ** Existe por lo mismo que el vocabulario tiene respaldo, y aqui se puede
/// escribir sin pedir permiso a nadie: este crate ES el de esta maquina.
///
/// OJO al cuarto: `r10` y no `rcx`. La instruccion `syscall` machaca `rcx` con
/// la direccion de vuelta en el silicio, antes de que el kernel mire nada. El
/// motivo entero esta en `[puerta]` de la tabla, que es donde se puede
/// contrastar con el manual.
const RESPALDO_ARGUMENTOS: [u8; 6] = [7, 6, 2, 10, 8, 9]; // rdi rsi rdx r10 r8 r9
const RESPALDO_NUMERO: u8 = 0; // rax
const RESPALDO_RESULTADO: u8 = 0; // rax

impl Puerta {
    /// La receta que diga la maquina; el respaldo si no la dice o si nombra un
    /// registro que esta maquina no conoce.
    ///
    /// ** Un nombre desconocido tira la receta ENTERA y no solo esa fila. Media
    /// receta buena es lo peor de las tres opciones: emitiria una puerta que
    /// cruza con cuatro argumentos en su sitio y el quinto en cualquier parte.
    pub fn de(m: Option<&Maquina>) -> Self {
        let receta = match m.and_then(|m| m.puerta()) {
            Some(r) => r,
            None => return Self::respaldo(),
        };
        let maquina = m.expect("si hay receta hay maquina");

        let uno = |n: &str| maquina.registro(n);
        let (numero, resultado) = match (uno(&receta.numero), uno(&receta.resultado)) {
            (Some(a), Some(b)) => (a, b),
            _ => return Self::respaldo(),
        };
        let mut argumentos = Vec::with_capacity(receta.argumentos.len());
        for n in &receta.argumentos {
            match uno(n) {
                Some(r) => argumentos.push(r),
                None => return Self::respaldo(),
            }
        }

        Self {
            numero,
            argumentos,
            resultado,
        }
    }

    pub fn respaldo() -> Self {
        Self {
            numero: RESPALDO_NUMERO,
            argumentos: RESPALDO_ARGUMENTOS.to_vec(),
            resultado: RESPALDO_RESULTADO,
        }
    }

    /// Cuantos argumentos caben por la puerta antes de tener que pasarlos de
    /// otra forma. Hoy nadie pide mas: `invoca` son cinco y la puerta seis.
    pub fn caben(&self) -> usize {
        self.argumentos.len()
    }
}
