//! `aviso::codigos` -- los numeros, y la promesa de que no cambian.
//!
//! Un codigo de error es una **direccion publica**: alguien lo va a buscar, lo
//! va a pegar en un mensaje, lo va a poner en un comentario de su codigo. Por
//! eso se reservan aqui, en un sitio, y por eso **un numero retirado no se
//! reutiliza jamas** -- reciclarlo haria que una busqueda vieja diera una
//! respuesta nueva y equivocada, que es peor que no dar ninguna.
//!
//! Tres familias, y la letra dice **cuando** te enteras:
//!
//! ```text
//!    E0xxx   no compila            te enteras al escribir
//!    E1xxx   atrapa en ejecucion   te enteras al correr, y como DATO
//!    A2xxx   aviso                 compila, y aun asi hay algo que decir
//! ```
//!
//! Los `E1xxx` viven aqui aunque el frontend no los emita nunca: son el
//! contrato que el runtime tendra que cumplir, y tenerlos en la misma lista es
//! lo que impide que dos fases inventen dos numeraciones. Es el bug de
//! `INFO_CPU_HZ_REAL` escrito encima de `INFO_FUGAS`, evitado por delante.

/// Un codigo de la lista. Es un tipo y no un `u16` suelto para que no se pueda
/// pasar un numero cualquiera donde va un codigo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Codigo(pub &'static str);

impl std::fmt::Display for Codigo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

// ===================================================================
//  E0xxx -- no compila
// ===================================================================

/// Falta `perfil` en la primera linea util. No hay perfil por defecto.
pub const FALTA_PERFIL: Codigo = Codigo("E0001");
/// `cambiante` en el nivel superior: lo de arriba se congela al cargar.
pub const CAMBIANTE_ARRIBA: Codigo = Codigo("E0002");
/// Perfil desconocido: solo hay `llano` y `pleno`.
pub const PERFIL_RARO: Codigo = Codigo("E0003");

/// Un tabulador donde va la sangria.
pub const TABULADOR: Codigo = Codigo("E0010");
/// Comilla simple. En INTI no existe.
pub const COMILLA_SIMPLE: Codigo = Codigo("E0011");
/// La sangria no es multiplo de cuatro.
pub const SANGRIA_RARA: Codigo = Codigo("E0012");
/// Un texto que empieza y no acaba antes del final de la linea.
pub const TEXTO_SIN_CERRAR: Codigo = Codigo("E0013");
/// Una barra de escape seguida de algo que no es escape.
pub const ESCAPE_RARO: Codigo = Codigo("E0014");
/// Un caracter que no es de este lenguaje.
pub const SIGNO_DESCONOCIDO: Codigo = Codigo("E0015");
/// Un numero mal escrito (dos puntos decimales, un `0x` sin digitos...).
pub const NUMERO_RARO: Codigo = Codigo("E0016");
/// Se cierra un parentesis que nadie abrio, o al reves.
pub const PAREJA_ROTA: Codigo = Codigo("E0017");

/// Un numero literal que no cabe en 64 bits.
///
/// ** Existe porque durante un dia no existio: `0xFFFFFFFFFFFFFFFF` --que es un
/// `natural64` perfectamente valido-- no cabia en el `i64` con el que se
/// guardaban los literales, la conversion fallaba, y **el numero se convertia
/// en CERO sin una sola queja**.
///
/// Lo primero se arreglo: los literales guardan el PATRON DE BITS, asi que
/// cualquier `natural64` cabe. Este codigo es para lo segundo -- lo que de
/// verdad no quepa en 64 bits tiene que decirlo, porque un cero silencioso en
/// una mascara o en una capability es de los fallos que no se encuentran
/// mirando el programa: el programa esta bien.
pub const NUMERO_ENORME: Codigo = Codigo("E0018");

/// En `llano` hay que decir la medida: `numero` no existe alli.
pub const FALTA_TAMANO: Codigo = Codigo("E0020");
/// Un `quiza T` usado sin mirarlo antes.
pub const QUIZA_SIN_MIRAR: Codigo = Codigo("E0021");
/// Conversion implicita entre tipos. Aqui se piden con nombre.
pub const SIN_CONVERSION: Codigo = Codigo("E0022");

/// Reasignar un nombre que no es `cambiante`.
pub const NO_ES_CAMBIANTE: Codigo = Codigo("E0030");
/// Declarar sin valor. No existe la sintaxis, y por eso la regla 4 es gratis.
pub const SIN_VALOR: Codigo = Codigo("E0031");
/// Tocar un valor congelado.
pub const CONGELADO: Codigo = Codigo("E0032");
/// Cambiar un parametro que no se declaro `cambiante`.
pub const PARAMETRO_FIJO: Codigo = Codigo("E0033");

/// La condicion de un `si` no es `logico`. No hay veracidad.
pub const CONDICION_NO_LOGICA: Codigo = Codigo("E0040");
/// Modificar la coleccion que se esta recorriendo.
pub const MUTA_ITERANDO: Codigo = Codigo("E0050");
/// Ignorar un resultado que puede fallar.
pub const ERROR_IGNORADO: Codigo = Codigo("E0060");

/// Usar algo que asigna memoria dentro del perfil `llano`.
pub const LLANO_NO_ADMITE: Codigo = Codigo("E0070");
/// `crudo` dentro del perfil `pleno`.
pub const CRUDO_EN_PLENO: Codigo = Codigo("E0071");
/// Tocar el metal fuera de un bloque `crudo`.
pub const METAL_SIN_CRUDO: Codigo = Codigo("E0072");

/// Un perfil que el compilador todavia no sabe bajar a bytes.
///
/// ** No es una prohibicion del lenguaje: es el compilador diciendo lo que NO
/// SABE hacer, que es distinto y hay que distinguirlo. `perfil pleno` es
/// legitimo y esta especificado entero; lo que falta es su runtime.
///
/// Hasta el 2026-08-22 no se decia, y un fichero de `pleno` salia como un
/// `.bex` firmado de 768 bytes que devolvia ceros. El gate decia que si sobre
/// algo que no hacia nada -- y una firma sobre eso es peor que no tener firma.
pub const PERFIL_SIN_BYTES: Codigo = Codigo("E0073");
/// **Lo que `llano` no admite por lo que CUESTA**, no por lo que le falta.
///
/// ** `numero` es decimal exacto y su suma cuesta 5-20 veces una entera de 64
/// bits. No es que no se sepa cuanto mide --el maestro lo tiene decidido:
/// coeficiente de 128 bits mas escala-- es que `llano` escribe drivers y ahi
/// ese precio no se paga sin decirlo.
///
/// *** Existe separado de `E0020` porque el motivo es otro, y un motivo
/// equivocado manda a buscar lo que no falta. Tercera pareja de la misma
/// familia: **prohibido** / **no se hacerlo todavia** (`E0073`) / **cuesta y
/// aqui no se paga**.
pub const CUESTA_DEMASIADO: Codigo = Codigo("E0074");
/// Cruzar algo mutable a otra tarea.
pub const CRUZA_MUTABLE: Codigo = Codigo("E0080");

/// Indice constante fuera de rango: se ve al compilar, no se deja para luego.
pub const INDICE_VISIBLE: Codigo = Codigo("E0090");
/// Herencia. No hay.
pub const SIN_HERENCIA: Codigo = Codigo("E0100");
/// Una funcion dentro de otra. No hay, y el motivo es del perfil: una captura
/// hay que guardarla en algun sitio, y en `llano` no hay monton. Tenerlas solo
/// en `pleno` serian dos lenguajes con una gramatica.
pub const SIN_FUNCION_ANIDADA: Codigo = Codigo("E0101");
/// Un nombre que no esta declarado ni lo trae ningun `usa`.
pub const NOMBRE_DESCONOCIDO: Codigo = Codigo("E0110");

// ===================================================================
//  E1xxx -- atrapa en ejecucion, y llega como dato
// ===================================================================

/// La suma, resta o multiplicacion se paso de la cuenta.
pub const DESBORDE: Codigo = Codigo("E1001");
/// Indice fuera de rango, calculado en ejecucion.
pub const INDICE: Codigo = Codigo("E1002");
/// Division entre cero.
pub const ENTRE_CERO: Codigo = Codigo("E1003");
/// Un prestamo que sobrevivio a lo prestado.
pub const PRESTAMO_MUERTO: Codigo = Codigo("E1005");
/// Convertir un flotante que no cabe en el entero de destino.
pub const CONVERSION: Codigo = Codigo("E1012");

// ===================================================================
//  A2xxx -- avisos
// ===================================================================

/// Desplazamiento constante mayor que el ancho del tipo: da cero, y se dice.
pub const DESPLAZA_DE_MAS: Codigo = Codigo("A2007");
/// Un identificador con letras fuera del ASCII. Se permite y se avisa.
pub const NOMBRE_NO_ASCII: Codigo = Codigo("A2010");

/// Todos los codigos, para el test que comprueba que ninguno se repite.
///
/// Se escribe a mano a proposito: una lista generada no habria detectado el
/// duplicado que motivo este test, porque el generador habria repetido el
/// error.
pub const TODOS: &[Codigo] = &[
    FALTA_PERFIL,
    CAMBIANTE_ARRIBA,
    PERFIL_RARO,
    TABULADOR,
    COMILLA_SIMPLE,
    SANGRIA_RARA,
    TEXTO_SIN_CERRAR,
    ESCAPE_RARO,
    SIGNO_DESCONOCIDO,
    NUMERO_RARO,
    PAREJA_ROTA,
    FALTA_TAMANO,
    QUIZA_SIN_MIRAR,
    SIN_CONVERSION,
    NO_ES_CAMBIANTE,
    SIN_VALOR,
    CONGELADO,
    PARAMETRO_FIJO,
    CONDICION_NO_LOGICA,
    MUTA_ITERANDO,
    ERROR_IGNORADO,
    LLANO_NO_ADMITE,
    CRUDO_EN_PLENO,
    METAL_SIN_CRUDO,
    CRUZA_MUTABLE,
    INDICE_VISIBLE,
    SIN_HERENCIA,
    SIN_FUNCION_ANIDADA,
    NOMBRE_DESCONOCIDO,
    DESBORDE,
    INDICE,
    ENTRE_CERO,
    PRESTAMO_MUERTO,
    CONVERSION,
    DESPLAZA_DE_MAS,
    NOMBRE_NO_ASCII,
];

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::collections::HashSet;

    /// Dos codigos con el mismo numero es el bug de `INFO_CPU_HZ_REAL` escrito
    /// encima de `INFO_FUGAS`: no falla, miente.
    #[test]
    fn ningun_codigo_se_repite() {
        let mut vistos = HashSet::new();
        for c in TODOS {
            assert!(vistos.insert(c.0), "codigo repetido: {}", c.0);
        }
    }

    /// La familia se lee en la primera letra y el resto son cuatro digitos.
    /// Sin esto, un `E42` se colaria y el lector perderia la pista de si es
    /// de compilacion o de ejecucion.
    #[test]
    fn la_forma_del_codigo_es_ley() {
        for c in TODOS {
            let s = c.0;
            assert_eq!(s.len(), 5, "codigo con forma rara: {}", s);
            let familia = &s[0..1];
            assert!(familia == "E" || familia == "A", "familia rara: {}", s);
            assert!(
                s[1..].chars().all(|d| d.is_ascii_digit()),
                "codigo no numerico: {}",
                s
            );
        }
    }

    /// La segunda cifra dice la familia y tiene que coincidir con la letra.
    #[test]
    fn la_letra_y_el_millar_estan_de_acuerdo() {
        for c in TODOS {
            let millar = &c.0[1..2];
            match &c.0[0..1] {
                "E" => assert!(millar == "0" || millar == "1", "{} no es 0 ni 1", c.0),
                "A" => assert_eq!(millar, "2", "{} deberia ser A2xxx", c.0),
                _ => unreachable!(),
            }
        }
    }
}

/// Un campo que no existe, o algo que no tiene campos.
pub const CAMPO_DESCONOCIDO: Codigo = Codigo("E0120");

/// No se sabe cuanto mide algo que hay que medir.
///
/// ** Es el aviso que sustituye a un comportamiento peor: antes de que
/// existiera `disposicion`, `p.x` se bajaba a `p` --el campo se ignoraba, sin
/// una queja-- y `a[i]` daba la direccion en vez del valor. Compilaba, corria,
/// y hacia otra cosa.
pub const SIN_MEDIDA: Codigo = Codigo("E0121");

/// Un campo de registro sin tipo.
pub const CAMPO_SIN_TIPO: Codigo = Codigo("E0122");

/// Una operacion que no existe para la coma flotante.
///
/// ** Los bits de un `flotante64` no son un numero: son signo, exponente y
/// mantisa metidos en ocho bytes. `f | 1` no enciende un bit de nada -- toca el
/// exponente, y el resultado es otro flotante cualquiera.
///
/// Que este DENUNCIADO y no simplemente "no soportado" es la diferencia: sin el
/// aviso, el emisor no tendria que emitir para ese caso, no emitiria nada, y el
/// programa compilaria y daria basura. Es el mismo agujero que F5b cerro en los
/// campos, visto en otro sitio.
///
/// Y hay una salida escrita: quien quiera los bits DE VERDAD los pide por su
/// nombre, y entonces esta pidiendo un entero, que es lo que son.
pub const FLOTANTE_SIN_BITS: Codigo = Codigo("E0123");
