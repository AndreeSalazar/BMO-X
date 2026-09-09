//! **CARRIL AMARILLO** -- LA CONTABILIDAD. Que ha cambiado, y lo que costo.
//!
//! [carril]  AMARILLO  no mueve un pixel: apunta y cuenta. Pero es lo que le
//!                     dice al carril ROJO **cuanto** copiar, asi que un error
//!                     de aqui sale por alli multiplicado.
//!
//! [cuesta]  APARATO -- si `marcar` se queda corta, lo pintado no llega nunca
//!           al panel y la pantalla se queda con la imagen de antes. El usuario
//!           no ve un error: ve un escritorio congelado, que es lo que el dueno
//!           reporto tres veces en agosto antes de que existiera el troceado.
//!
//! [riesgo]  ESPEJO SILENCIO
//!           ESPEJO -- la caja sucia es un REFLEJO de lo que se pinto, y se
//!                    mantiene a mano. Toda primitiva nueva que escriba en el
//!                    lienzo y no marque abre el agujero otra vez.
//!                    `punto_sin_comprobar` es publica y NO marca a proposito:
//!                    ese contrato esta escrito en su `# Safety` porque no hay
//!                    forma de que el compilador lo vigile.
//!           SILENCIO -- las cuentas del volcado no fallan: CONVENCEN. Un
//!                    contador equivocado manda la siguiente sesion al fichero
//!                    equivocado, y eso ya paso el 08-09 con `cuerpo`.
//!
//! # Por que la cuenta sale de las cajas y no de un acumulador
//!
//! Un acumulador a mano se puede olvidar en una rama, y entonces la estadistica
//! **miente despacio**: no da un salto, da un numero bajo que parece bueno. Los
//! bytes se derivan de `Sucias::pixeles()`, que es la misma fuente que usa el
//! carril rojo para copiar -- si los dos numeros se separan, es que la copia
//! esta mal, y eso es exactamente lo que se quiere poder ver.

use crate::*;

impl Pantalla {
    /// **Apunta que esta region ha cambiado.** Sin esto, lo pintado se queda en
    /// el lienzo y no llega nunca al panel.
    ///
    /// Las primitivas de dibujo lo hacen solas. Es publico porque
    /// [`Pantalla::punto_sin_comprobar`] no marca --es el camino caliente y no
    /// va a llevar esto dentro--, asi que quien la use tiene que marcar el.
    #[inline]
    pub fn marcar(&self, x: u32, y: u32, ancho: u32, alto: u32) {
        if ancho == 0 || alto == 0 {
            return;
        }
        // `saturating_add` y no `+`: desde el 09-09 `glifo` marca su celda de
        // 8x16 de una vez, o sea que aqui llega aritmetica de la fuente y no
        // solo de la maqueta. En release un `+` que desborda ENVUELVE, y una
        // caja envuelta es una caja pequena en la esquina de arriba: se dejaria
        // de volcar lo que si se pinto. Saturar la deja fuera de pantalla, que
        // es lo que el `if` de abajo ya sabe descartar.
        let nx1 = x.saturating_add(ancho).min(self.ancho);
        let ny1 = y.saturating_add(alto).min(self.alto);
        if x >= nx1 || y >= ny1 {
            return;
        }
        // ** VARIAS CAJAS Y NO UNA. Ver `crate::sin_gpu::sucio`.
        //
        // [!] Y esa carpeta se llama asi por algo: todo esto **desaparece con el
        // page flip**. Trocear la copia es trabajo que la CPU hace porque no hay
        // quien mueva la direccion del escaner; no es como deberia quedarse.
        //
        // Con una sola, dos cambios en esquinas opuestas --el cursor donde
        // estaba y donde esta-- unian a la pantalla ENTERA: 384 pixeles reales
        // convertidos en 2.073.600 copiados, cada fotograma, a memoria
        // write-combining. Eso era a la vez la lentitud y el parpadeo.
        let mut s = self.sucio.get();
        s.marcar((x, y, nx1, ny1));
        self.sucio.set(s);
    }

    /// Apunta lo que costo el fotograma que acaba de volcarse.
    ///
    /// Vive aparte de `volcar` desde el 09-09 y no es cosmetico: el carril rojo
    /// tiene DOS caminos de copia --la fila a fila y la de pantalla entera de un
    /// tiron-- y los dos tienen que acabar en la misma contabilidad. Un `return`
    /// nuevo en cualquiera de ellos que se saltara esto daria justo el fallo que
    /// este fichero declara como `[riesgo] SILENCIO`.
    pub(super) fn anotar(&self, sucias: &crate::sin_gpu::sucio::Sucias) {
        let bytes = sucias.pixeles() * 4;
        let v = self.volcado.get();
        // Las cajas se apuntan solo cuando este fotograma ES el peor: guardar
        // el maximo de las dos cosas por separado daria una pareja que nunca
        // ocurrio, y un numero que no paso no explica nada.
        let peor_ahora = bytes > v.peor;
        self.volcado.set(Volcado {
            fotogramas: v.fotogramas + 1,
            bytes: v.bytes + bytes,
            peor: v.peor.max(bytes),
            ultimo: bytes,
            cajas: if peor_ahora { sucias.cajas().len() as u32 } else { v.cajas },
            modo: v.modo,
        });
    }

    /// Lo que lleva costado el volcado. Ver [`Volcado`] y [`Volcador`].
    ///
    /// ** El modo se calcula AQUI y no se guarda: la unica verdad sobre si hay
    /// lienzo son los dos punteros, y una copia de ese hecho en un campo es una
    /// segunda fuente que puede quedarse vieja. Es la misma leccion que costo el
    /// `ocupada` de `endpoint.rs`, borrado el 08-09 por mentir desde el dia uno.
    pub fn volcado(&self) -> Volcado {
        let mut v = self.volcado.get();
        v.modo = if self.lienzo == self.panel { Volcador::Ninguno } else { Volcador::Directo };
        v
    }
}

/// **Como llegan los pixeles del lienzo al panel.**
///
/// === * Esta es la costura donde entra una GPU ===
///
/// La idea que la motivo era partir el compositor en `gui_CPU.bex` y
/// `gui_GPU.bex`. Eso seria **bifurcar antes de que exista la segunda
/// implementacion**: cada arreglo habria que hacerlo dos veces, que es
/// exactamente el problema que resolvio `refactor(abi): la disposicion de
/// agregados estaba escrita TRES veces`.
///
/// El corte correcto es por CAPA, y hay tres -- y desde el 09-09 son **los tres
/// ficheros de esta carpeta**, que es la prueba de que el corte era el bueno:
///
/// ```text
///   POLITICA     que ventana existe, donde va, quien tiene el foco
///                -> no cambia NUNCA con una GPU. Vive en el compositor.
///   DIBUJO       llenar el lienzo: punto, rect, texto
///                -> es CPU SIEMPRE. Una app que pinta su superficie en RAM
///                   la pinta con el CPU, tenga GPU o no.
///   VOLCADO      mover el rectangulo sucio del lienzo al panel
///                -> AQUI, y solo aqui, una GPU cambia algo.
/// ```
///
/// Por eso el contrato es esto y no un trait `Lienzo` entero: `punto` esta en
/// el camino caliente y meterle una llamada indirecta costaria en cada pixel
/// para no ganar nada. `volcar` se llama **una vez por fotograma**, asi que
/// aqui una rama no se nota -- y es donde esta el coste de verdad.
///
/// === Y antes de comprar una tarjeta, MEDIR ===
///
/// La caja de sucio ya evita casi todo el trabajo: escribir una letra vuelca
/// unos pocos KiB, no la pantalla. Ver [`Pantalla::volcado`] -- el numero va
/// primero, la tarjeta despues.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Volcador {
    /// No hay doble bufer: lo pintado ya esta en el panel. No hay nada que
    /// mover, y decirlo con su nombre vale mas que un `if` suelto.
    Ninguno,
    /// Copia con `rep movsb`: una instruccion por fila, y **una sola para la
    /// pantalla entera** cuando la caja ocupa el stride.
    ///
    /// Es lo que hay hoy y lo que corre en el Ryzen. Hasta el 09-09 era un bucle
    /// de dos pixeles por escritura `volatile`, que el compilador tenia
    /// prohibido tocar: ver la cabecera de `roja.rs`.
    Directo,
}

/// Lo que ha costado el volcado, para poder **perfilar antes de comprar nada**.
///
/// No es telemetria de adorno: la pregunta "hace falta una GPU?" solo se puede
/// contestar con estos dos numeros. Si los bytes por fotograma son pocos, una
/// tarjeta no compra nada y cuesta un ano de trabajo.
#[derive(Clone, Copy)]
pub struct Volcado {
    /// Fotogramas con algo que volcar. Los que no cambian nada no cuentan:
    /// promediar con ellos esconderia el caso caro.
    pub fotogramas: u64,
    /// Bytes movidos del lienzo al panel, en total.
    pub bytes: u64,
    /// El fotograma mas caro visto. **El peor caso importa mas que la media**:
    /// un tiron se nota, y una media buena lo esconde.
    pub peor: u64,
    /// ** LO QUE COSTO EL ULTIMO, y existe desde el 09-09 por una foto.
    ///
    /// El primer arranque con el medidor en la barra dijo `volcado 8100K
    /// cajas 1`, que es EXACTAMENTE 1920x1080x4: la pantalla entera en una caja.
    /// Y no significaba que el troceado hubiera degenerado -- es el PRIMER
    /// fotograma, el que `activar_doble_bufer` marca entero para igualar los dos
    /// bufferes.
    ///
    /// *** `peor` no baja nunca, y eso esta bien: un maximo que se olvida no es
    /// un maximo. Pero un maximo que no caduca **no sabe decir AHORA**, y esa es
    /// justo la pregunta que se hace mirando una barra de tareas. Hacen falta
    /// los dos numeros, no uno mejor.
    pub ultimo: u64,
    /// ** CAJAS SUCIAS DEL PEOR FOTOGRAMA, y es el numero que dice si el
    /// arreglo del 2026-08-12 sirvio de algo.
    ///
    /// Con la caja unica de antes esto valdria SIEMPRE 1, y `peor` seria la
    /// pantalla entera en cuanto dos cosas cambiaran lejos. Si en metal sale
    /// `cajas 2` o `3` con un `peor` pequeno, el troceado esta trabajando. Si
    /// sale `cajas 1` con un `peor` de 8 MB, degenero -- y entonces el
    /// sospechoso es `COSTE_DE_UNA_CAJA`, no el volcado.
    pub cajas: u32,
    pub modo: Volcador,
}
