//! La rejilla de salida y su historial con scroll.
//!
//! 200 filas guardadas, ventana de 16. Lo que sale por arriba **no se pierde**:
//! se mira con la rueda o con RePag/AvPag.

use bmo_userland as bmo;

use super::*;

// -- La salida -----------------------------------------------------------

/// Rejilla de caracteres con desplazamiento. Es lo minimo que se puede llamar
/// terminal: sin colores por celda, sin secuencias de escape, sin scrollback
/// mas alla de lo que se ve.
///
/// Deliberado. Un emulador de terminal completo --ANSI, cursor direccionable,
/// regiones-- es una pila entera, y hoy lo unico que hay al otro lado son
/// programas que escriben lineas. Cuando algo pida mas, se anade; adivinarlo
/// ahora seria escribir codigo que nadie ejercita.
// -- La tinta ------------------------------------------------------------
//
// El color va POR LINEA y no por celda. Una rejilla de atributos costaria
// 88x16 bytes para decir dieciseis veces lo mismo: este terminal escribe
// lineas enteras --el eco de un comando, un mensaje de error, la salida de un
// programa-- y nunca media linea de un color y media de otro.
//
// Antes el unico color lo daba `f == s.fila`, o sea "la fila donde esta el
// cursor". Eso pinta de otro color la ULTIMA linea escrita, que casi nunca es
// la que te interesa: si un programa imprime diez lineas, la marcada es la
// decima y no el comando que lo lanzo.

/// Salida corriente de un programa.
pub(crate) const TINTA_NORMAL: u8 = 0;
/// El comando que escribiste. Es el ancla para leer hacia abajo.
pub(crate) const TINTA_ECO: u8 = 1;
/// Algo salio mal.
pub(crate) const TINTA_MAL: u8 = 2;
/// Algo salio bien y merece verse.
pub(crate) const TINTA_BIEN: u8 = 3;

pub(crate) fn color_tinta(t: u8) -> u32 {
    match t {
        TINTA_ECO => SAL_ECO,
        TINTA_MAL => TEXTO_MAL,
        TINTA_BIEN => TEXTO_BIEN,
        _ => SAL_TEXTO,
    }
}

pub(crate) struct Salida {
    /// El historial entero. Se ESCRIBE aqui; la ventana visible es un trozo.
    pub(crate) celdas: [[u8; SAL_COLS]; SAL_HIST],
    /// Con que color se pinta cada fila.
    pub(crate) tinta: [u8; SAL_HIST],
    /// Con que color se escribe a partir de ahora.
    pub(crate) tinta_actual: u8,
    pub(crate) fila: usize,
    pub(crate) col: usize,
    /// Cuantas filas se ha subido el usuario. 0 = pegado abajo, viendo lo
    /// ultimo. Escribir algo nuevo vuelve abajo, como cualquier terminal: si no,
    /// el programa hablaria y nadie lo veria.
    pub(crate) vista: usize,
    /// Hay algo nuevo que pintar. Repintar la rejilla entera cada fotograma
    /// serian 88x16 glifos por vuelta sobre memoria de video sin cache.
    pub(crate) sucia: bool,
    /// Cuantas lineas se han CERRADO desde que arranco el terminal. Solo sube.
    ///
    /// * El indice de fila no sirve para acordarse de un sitio: en cuanto el
    /// historial se llena, `fila` se queda clavada en la ultima y las de
    /// debajo se van desplazando. Guardar "empece en la fila 187" y volver a
    /// mirar ahi un minuto despues senala a otra linea.
    ///
    /// Un contador que solo sube si sirve: la diferencia entre dos marcas es
    /// **cuantas lineas se escribieron entre medias**, y eso no se mueve
    /// aunque el historial se desplace veinte veces.
    pub(crate) escritas: usize,
    /// Cuantas filas del historial tienen CONTENIDO ahora mismo, contando la
    /// que se esta escribiendo. Nunca pasa de [`SAL_HIST`].
    ///
    /// Es distinto de `escritas` y hacen falta las dos: aquel dice *cuanto se
    /// ha escrito nunca* --y por eso sirve de marca--, esta dice *cuanto queda
    /// guardado*. Un `clear` no puede tocar el primero sin que las marcas
    /// viejas se vuelvan del futuro, pero si tiene que poner el segundo a cero
    /// -- si no, volcar el historial escupiria doscientas lineas en blanco.
    vivas: usize,
}

impl Salida {
    pub(crate) fn nueva() -> Self {
        Self {
            celdas: [[b' '; SAL_COLS]; SAL_HIST],
            tinta: [TINTA_NORMAL; SAL_HIST],
            vista: 0,
            tinta_actual: TINTA_NORMAL,
            // ** SE ESCRIBE EN LA ULTIMA FILA, no en la primera.
            //
            // Aqui vivia el bug que hacia que `ls` "no mostrara nada": el
            // ESCRITOR empezaba arriba (`fila = 0`) y el LECTOR
            // (`pintar_salida`) ensena **las ultimas** `SAL_ROWS` filas del
            // historial, o sea `celdas[184..200]`. Los dos miraban extremos
            // opuestos del mismo buffer.
            //
            // Consecuencia exacta: las **184 primeras lineas que escribiera
            // cualquier programa eran invisibles**. `ls` escupe una docena, asi
            // que no llegaba ni de lejos -- el comando corria, la linea de
            // estado decia `listo`, y la rejilla se quedaba en blanco. Un fallo
            // que se ve como "no hace nada" y en realidad es "lo hace donde no
            // se mira".
            //
            // Llego con el historial con scroll (`8ee091e2`): la ventana paso a
            // ser un trozo de 200 filas y **el escritor se quedo donde estaba**,
            // que era correcto cuando la rejilla eran 16 filas y punto.
            //
            // Con `fila` en la ultima, `salto()` siempre entra por la rama de
            // `desplazar()`: la linea nueva esta SIEMPRE abajo del todo y
            // siempre dentro de la ventana. Que es como se comporta cualquier
            // terminal.
            fila: SAL_HIST - 1,
            col: 0,
            sucia: true,
            escritas: 0,
            // La linea en curso ya cuenta: esta vacia, pero es una fila del
            // historial y no un hueco.
            vivas: 1,
        }
    }

    /// Donde estamos ahora, para poder volver. Ver [`Salida::escritas`].
    pub(crate) fn marca(&self) -> usize {
        self.escritas
    }

    /// Las filas del historial escritas **desde una marca**, como rango
    /// inclusivo de indices en `celdas`.
    ///
    /// Se recorta a lo que de verdad queda guardado: si desde la marca han
    /// pasado mas de [`SAL_HIST`] lineas --o si hubo un `clear` en medio--, esas
    /// lineas ya no estan y se devuelven solo las que hay. Prometer un rango
    /// mas largo daria filas en blanco que parecerian salida vacia del
    /// programa, que es justo la conclusion equivocada.
    pub(crate) fn filas_desde(&self, marca: usize) -> (usize, usize) {
        // +1 por la linea en curso, que aun no ha cerrado pero ya tiene texto.
        let cerradas = self.escritas.saturating_sub(marca);
        let cuantas = (cerradas + 1).min(self.vivas);
        (self.fila + 1 - cuantas, self.fila)
    }

    /// Todo lo que queda guardado, sin las filas en blanco de arriba.
    pub(crate) fn filas_todas(&self) -> (usize, usize) {
        (self.fila + 1 - self.vivas, self.fila)
    }

    /// Una fila **sin la cola de espacios**. Un volcado con las 88 columnas
    /// rellenas es ilegible y pesa cuatro veces mas de lo que dice.
    pub(crate) fn linea(&self, f: usize) -> &[u8] {
        let fila = &self.celdas[f];
        let mut n = fila.len();
        while n > 0 && fila[n - 1] == b' ' {
            n -= 1;
        }
        &fila[..n]
    }

    /// A partir de aqui se escribe con esta tinta. La fila en curso se marca
    /// ya: quien cambia el color antes de escribir espera que valga para lo
    /// que va a escribir, no para lo siguiente.
    pub(crate) fn con_tinta(&mut self, t: u8) {
        self.tinta_actual = t;
        self.tinta[self.fila] = t;
    }

    /// Sube todo una linea y deja la ultima en blanco. La tinta viaja con su
    /// linea: si no, al desplazarse el color se quedaria marcando la fila de
    /// otro.
    pub(crate) fn desplazar(&mut self) {
        for f in 1..SAL_HIST {
            self.celdas[f - 1] = self.celdas[f];
            self.tinta[f - 1] = self.tinta[f];
        }
        self.celdas[SAL_HIST - 1] = [b' '; SAL_COLS];
        self.tinta[SAL_HIST - 1] = self.tinta_actual;
    }

    /// Sube o baja la ventana. Positivo = hacia atras en el tiempo.
    ///
    /// Se topa sola en los dos extremos: no se puede subir mas alla de lo
    /// guardado ni bajar mas alla de lo ultimo. Un scroll que se sale ensena
    /// filas en blanco y parece que se ha perdido todo.
    pub(crate) fn mover_vista(&mut self, filas: i32) {
        let tope = SAL_HIST - SAL_ROWS;
        let nueva = (self.vista as i32 + filas).clamp(0, tope as i32) as usize;
        if nueva != self.vista {
            self.vista = nueva;
            self.sucia = true;
        }
    }

    pub(crate) fn salto(&mut self) {
        self.escritas += 1;
        self.vivas = (self.vivas + 1).min(SAL_HIST);
        self.col = 0;
        // Escribir devuelve la vista abajo: si no, el programa hablaria y el
        // usuario seguiria mirando el pasado sin enterarse.
        self.vista = 0;
        if self.fila + 1 >= SAL_HIST {
            self.desplazar();
        } else {
            self.fila += 1;
        }
        self.tinta[self.fila] = self.tinta_actual;
        self.sucia = true;
    }

    pub(crate) fn byte(&mut self, b: u8) {
        match b {
            b'\n' => self.salto(),
            // El retorno de carro solo, sin avance: se ignora. Un programa que
            // escribe "\r\n" no debe producir dos saltos.
            b'\r' => {}
            // Tabulador a la siguiente parada de 8.
            b'\t' => {
                let next = (self.col / 8 + 1) * 8;
                while self.col < next.min(SAL_COLS) {
                    self.celdas[self.fila][self.col] = b' ';
                    self.col += 1;
                }
                if self.col >= SAL_COLS {
                    self.salto();
                }
                self.sucia = true;
            }
            // Los no imprimibles se tiran en vez de dibujarse como basura.
            c if c < 0x20 => {}
            c => {
                if self.col >= SAL_COLS {
                    self.salto();
                }
                self.celdas[self.fila][self.col] = c;
                self.col += 1;
                self.sucia = true;
            }
        }
    }

    pub(crate) fn texto(&mut self, s: &[u8]) {
        for &b in s {
            self.byte(b);
        }
    }

    pub(crate) fn limpiar(&mut self) {
        self.celdas = [[b' '; SAL_COLS]; SAL_HIST];
        self.tinta = [TINTA_NORMAL; SAL_HIST];
        self.vista = 0;
        self.tinta_actual = TINTA_NORMAL;
        // La misma fila que en `nueva`, y por el mismo motivo: si `clear`
        // devolviera el cursor arriba, el bug volveria solo despues de limpiar
        // -- que es la clase de fallo que aparece una vez cada mil y no se
        // reproduce nunca.
        self.fila = SAL_HIST - 1;
        self.col = 0;
        self.sucia = true;
        // `escritas` NO se reinicia: sigue contando desde que arranco el
        // terminal. Ponerlo a cero haria que una marca tomada antes del `clear`
        // pareciera del futuro y la resta se diera la vuelta. Lo que si se
        // reinicia es `vivas` -- ya no queda nada guardado -- y asi un volcado
        // justo despues de limpiar escribe un archivo vacio y no doscientas
        // lineas de espacios.
        self.escritas += 1;
        self.vivas = 1;
    }

    // -- Lo que hace falta para PINTAR un informe ------------------------
    //
    // Todo esto es de Ring 3 a proposito. El kernel contesta enteros crudos y
    // no sabe nada de KiB, de porcentajes ni de barras: un kernel que decide
    // como se ve un numero es un kernel con opiniones sobre la interfaz.

    /// Un entero en decimal.
    pub(crate) fn dec(&mut self, mut v: u64) {
        if v == 0 {
            self.byte(b'0');
            return;
        }
        let mut d = [0u8; 20];
        let mut n = 0;
        while v > 0 {
            d[n] = b'0' + (v % 10) as u8;
            v /= 10;
            n += 1;
        }
        while n > 0 {
            n -= 1;
            self.byte(d[n]);
        }
    }

    /// Un entero alineado a la DERECHA en `ancho` columnas. Es lo que hace que
    /// una columna de numeros se lea de un vistazo en vez de bailar.
    /// **Hexadecimal, con `digitos` fijos y en mayusculas.**
    ///
    /// Se anadio con la red: una MAC y un `PHYstatus` se leen en hexadecimal en
    /// Windows, en un switch y en la etiqueta pegada a la tarjeta. Darlos en
    /// decimal obligaria a convertirlos a mano para compararlos con cualquiera
    /// de los tres, que es justo lo que un diagnostico no debe pedir.
    ///
    /// El ancho es fijo a proposito: `0x0A` y `0x A` alineados en columna se
    /// comparan de un vistazo, y `0xA` suelto no.
    pub(crate) fn hex(&mut self, v: u64, digitos: usize) {
        const D: &[u8; 16] = b"0123456789ABCDEF";
        let mut i = digitos;
        while i > 0 {
            i -= 1;
            self.byte(D[((v >> (i * 4)) & 0xF) as usize]);
        }
    }

    pub(crate) fn dec_der(&mut self, v: u64, ancho: usize) {
        let mut cifras = 1;
        let mut t = v;
        while t >= 10 {
            t /= 10;
            cifras += 1;
        }
        for _ in cifras..ancho {
            self.byte(b' ');
        }
        self.dec(v);
    }

    /// Bytes en la unidad que toca, con un decimal: `15.9 GiB`.
    ///
    /// Se hace con enteros, dividiendo y sacando el resto -- igual que el
    /// decimal de COBOL. Aqui no hay coma flotante y no hace falta.
    pub(crate) fn tamano(&mut self, bytes: u64) {
        const U: [&[u8]; 5] = [b"B", b"KiB", b"MiB", b"GiB", b"TiB"];
        let mut i = 0;
        let mut v = bytes;
        while v >= 1024 && i < 4 {
            v /= 1024;
            i += 1;
        }
        // El primer decimal, sin flotantes: el resto de la ultima division.
        let frac = if i == 0 {
            0
        } else {
            let divisor = 1u64 << (10 * i);
            ((bytes % divisor) * 10 / divisor) % 10
        };
        self.dec(v);
        if i > 0 {
            self.byte(b'.');
            self.byte(b'0' + frac as u8);
        }
        self.byte(b' ');
        self.texto(U[i]);
    }

    /// Un porcentaje entero, sin dividir por cero.
    pub(crate) fn pct(&mut self, parte: u64, total: u64) {
        if total == 0 {
            self.texto(b"--");
        } else {
            self.dec(parte.saturating_mul(100) / total);
        }
        self.byte(b'%');
    }

    /// Una barra de ocupacion de `ancho` columnas: `[####------]`.
    ///
    /// Con caracteres y no con pixeles porque la salida ES una rejilla de
    /// caracteres: una barra dibujada aparte se quedaria quieta cuando el log
    /// desplaza, y una barra que miente de sitio es peor que no tenerla.
    pub(crate) fn barra(&mut self, parte: u64, total: u64, ancho: usize) {
        let llenas = if total == 0 {
            0
        } else {
            (parte.saturating_mul(ancho as u64) / total) as usize
        };
        self.byte(b'[');
        for i in 0..ancho {
            self.byte(if i < llenas { b'#' } else { b'-' });
        }
        self.byte(b']');
    }
}


pub(crate) fn pintar_salida(p: &bmo::Pantalla, c: &Caja, s: &Salida) {
    // Fondo entero de la rejilla y encima las filas. Es un rectangulo de
    // 704x256 px: nada comparado con la pantalla, y evita tener que llevar la
    // cuenta de que celda cambio.
    p.rect(
        c.salida_x,
        c.salida_y,
        SAL_COLS as u32 * bmo::GLIFO_ANCHO,
        c.salida_alto(),
        CAJA_FONDO,
    );
    // La ventana: las ultimas SAL_ROWS filas, corridas hacia atras por `vista`.
    let base = SAL_HIST - SAL_ROWS - s.vista;
    for f in 0..SAL_ROWS {
        let color = color_tinta(s.tinta[base + f]);
        p.texto_bytes(
            c.salida_x,
            c.salida_y + f as u32 * bmo::GLIFO_ALTO,
            &s.celdas[base + f],
            color,
        );
    }
    // Y si se ha subido, DECIRLO. Una ventana que ensena el pasado sin avisar
    // se confunde con una que se quedo colgada.
    if s.vista > 0 {
        let x = c.salida_x + (SAL_COLS as u32 - 18) * bmo::GLIFO_ANCHO;
        p.texto(x, c.salida_y, "-- historial --", ACENTO);
    }
}
