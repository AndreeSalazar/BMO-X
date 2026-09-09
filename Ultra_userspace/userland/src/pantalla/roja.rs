//! **CARRIL ROJO** -- MOVER PIXELES. Lo unico de `Pantalla` que escribe memoria
//! cruda por puntero, y con la cuenta metida en un registro.
//!
//! [carril]  ROJO      todo lo demas de esta carpeta apunta, cuenta o compone
//!                     letras. Los pixeles los mueve ESTE fichero, y los mueve
//!                     con una instruccion que no comprueba nada.
//!
//! [cuesta]  MAQUINA -- `rep stosd` con un `rcx` de mas no da error: ESCRIBE. Y
//!           el lienzo es un bloque de `KIND_MEMORIA` pedido CONTIGUO, o sea que
//!           "lo que hay detras" es memoria de alguien. Este precio ya esta
//!           pagado una vez y por eso `limpiar` lleva un `min`: `pixeles()` --el
//!           area que mapeo el KERNEL-- puede ser mayor que `stride x alto`.
//!
//! [riesgo]  AJENO SILENCIO
//!           AJENO -- la velocidad de esto no la pone el codigo: la pone el
//!                    microcodigo del CPU (ERMSB) y el bus hasta la VRAM.
//!                    `PERFIL/CPU.txt` declara Zen 3, y ahi `rep movsb` es la
//!                    ruta rapida; en un CPU de antes de 2012 esto seria
//!                    CORRECTO Y LENTO, que es un fallo que no se ve.
//!           SILENCIO -- una copia que mueve de menos no peta: deja pixeles
//!                    viejos en la pantalla. Es el modo de fallo mas caro de
//!                    cazar que tiene un compositor, y por eso la cuenta de
//!                    bytes sale de las cajas y no de un acumulador a mano.
//!
//! # *** EL CUELLO DE BOTELLA, Y ERA UNA PALABRA: `volatile` (2026-09-09)
//!
//! Este fichero movia pixeles asi, y en los DOS sentidos:
//!
//! ```text
//!    volcar   read + write_volatile de 8 bytes    ->  2 px por escritura
//!    rect     write_volatile de 4 bytes           ->  1 px por escritura
//!    limpiar  lo mismo, 2.073.600 veces seguidas
//! ```
//!
//! *** **Y `volatile` le PROHIBE al compilador tocar ese bucle.** No lo puede
//! vectorizar, no lo puede convertir en `rep movsb`, no puede juntar dos
//! escrituras en una: la semantica de `volatile` es *"emite exactamente esto,
//! tantas veces como lo escribi"*. O sea que **el bucle que se lee es el bucle
//! que corre**, y no hay optimizador que lo salve.
//!
//! ```text
//!    volcar la pantalla entera   1920x1080 -> 1.036.800 escrituras de 8 B
//!    limpiar la pantalla entera             -> 2.073.600 escrituras de 4 B
//! ```
//!
//! ** Y el panel es memoria WRITE-COMBINING, que es justo donde eso mas duele:
//! el CPU junta escrituras seguidas en rafagas de linea de cache (64 B), o sea
//! que una linea se llena con OCHO escrituras de 8 bytes. Lo que se meta en
//! medio la suelta a medias, y una linea WC parcial no es una escritura mas
//! lenta: son varias transacciones de bus donde iba a haber una.
//!
//! ## Lo contraintuitivo, y es que la casa YA LO SABIA
//!
//! ```text
//!    toolchain/forge/bmo-lower/src/memoria.rs   `memcpy` y `memset` de TODO
//!                                               programa de C son `rep movsb`
//!                                               y `rep stosb` desde el 13-08
//!    platform/abi/.../objetos.rs, FB_OP_BYTES   "es lo que hace falta para
//!                                               llenar la pantalla entera con
//!                                               un `rep stosd` sin multiplicar
//!                                               nada"
//! ```
//!
//! *** El ABI declara un campo **cuyo comentario dice para que existe**, y el
//! compositor --el unico programa de esta casa que mueve megabytes por
//! fotograma-- era el ultimo sitio que seguia moviendolos a mano. Un `.bex` de C
//! copiaba mas rapido que el escritorio.
//!
//! Y la medida que dejo escrita aquella tanda: el bucle byte a byte daba **214
//! MB/s a base de `mov al`**. El blit de aqui se midio en **~300 MB/s**. Son la
//! misma cifra y no es casualidad: es el mismo error, escrito dos veces.
//!
//! ## Por que un `asm!` es MAS seguro que el `volatile` que sustituye
//!
//! Un `asm!` sin `nomem` ni `pure` es una BARRERA de memoria para el
//! optimizador: no puede mover escrituras al otro lado ni suponer que sabe que
//! hay detras del puntero. Es una garantia **mas fuerte** que `volatile`, no mas
//! debil. Lo que se pierde es que el compilador ya no cuenta los pixeles por ti
//! -- y por eso las dos funciones de abajo son cortas, sin ramas, y tienen la
//! cuenta en un unico sitio.
//!
//! ## Y lo que esto NO es: no es zero copy, es una copia BARATA
//!
//! [!] Zero copy de verdad es el **page flip**: no mover nada y cambiar la
//! direccion que lee el escaner de video. Sigue bloqueado --tras
//! `ExitBootServices` el GOP no existe-- y es el escalon 8 de `LA_RAM.md`.
//!
//! > Mientras el hardware obligue a copiar, el trabajo es que la copia obligada
//! > cueste UNA instruccion y no un millon.

use crate::*;

// -- LAS DOS INSTRUCCIONES -------------------------------------------------
//
// * `cld` delante, y cuesta un byte. `rep` avanza hacia adelante SOLO si `DF`
// esta a cero.
//
// *** Y AQUI HAY UNA CORRECCION MEDIDA, no una precaucion. `bmo-lower/memoria.rs`
// razono lo mismo el 13-08 y lo cerro con *"nadie en BMO emite `std`, asi que
// en la practica sobra siempre"*. **Eso es falso en este binario**, y se
// comprobo desensamblandolo el 09-09:
//
// ```text
//    4008a400: fd        std                     <- dentro de `memmove`
//    4008a401: f3 a4     rep movsb               <- su camino HACIA ATRAS
//    ...
//    4008a41e: fc        cld                     <- lo devuelve antes del ret
// ```
//
// `compiler_builtins` trae un `memmove` que copia hacia atras cuando las zonas
// se solapan, y para eso pone `DF`. Lo restaura bien --la ventana es de tres
// instrucciones-- asi que hoy no hay fallo. Pero la frase de la que colgaba
// "sobra siempre" resulto no ser cierta, y con ella se cae el argumento entero.
//
// > Un `cld` que sobra cuesta un byte. Uno que falta cuesta una copia que
// > escribe hacia atras, y solo cuando alguien ha llamado a `memmove` antes.

// -- LA PRUEBA, y son las de esta casa: se desensamblo -----------------------
//
// El 09-09, sobre el `director` recien enlazado:
//
//    rep movsb    2 sitios con `cld` delante  -> los dos caminos de `volcar`
//    rep stosd  172 sitios, TODOS con `cld`   -> `rect` y `limpiar`, ya
//                                                incrustados por el compilador
//                                                en cada sitio que los llama
//
// ** Los 172 son la medida que importa: el compilador metio `rect` entero en
// cada uno de sus llamadores, o sea que el escritorio no llama a una rutina de
// relleno -- LLEVA LA INSTRUCCION PUESTA donde pinta.
//
// Y que salgan los 172 con su `cld` es lo que dice que no queda ni un bucle
// viejo escondido: si alguno hubiera sobrevivido, la cuenta no cuadraria.

/// **Rellena `n` pixeles seguidos con el mismo color.**
///
/// # Safety
/// `destino .. destino+n` tiene que estar mapeado y ser escribible. Aqui no se
/// comprueba nada: la cuenta la pone quien llama.
#[inline]
unsafe fn rellenar(destino: *mut u32, color: u32, n: usize) {
    if n == 0 {
        return;
    }
    unsafe {
        core::arch::asm!(
            "cld",
            "rep stosd",
            inout("rdi") destino => _,
            inout("rcx") n => _,
            in("eax") color,
            options(nostack),
        );
    }
}

/// **Copia `n` pixeles seguidos.** Las dos zonas no pueden solaparse.
///
/// Va en BYTES (`rep movsb`) y no en dwords (`rep movsd`) a proposito: la ruta
/// rapida del microcodigo esta en la variante de byte --se llama ERMSB por
/// algo-- y mueve una linea de cache por ciclo. `movsd` no la tiene.
///
/// # Safety
/// Las dos zonas mapeadas, `n` pixeles enteros, y **sin solape**.
#[inline]
unsafe fn copiar(destino: *mut u32, origen: *const u32, n: usize) {
    if n == 0 {
        return;
    }
    unsafe {
        core::arch::asm!(
            "cld",
            "rep movsb",
            inout("rdi") destino => _,
            inout("rsi") origen => _,
            inout("rcx") n * 4 => _,
            options(nostack),
        );
    }
}

impl Pantalla {
    /// Rellenar la pantalla entera.
    ///
    /// Solo para el primer pintado. Repetirlo por fotograma seria recorrer
    /// varios MB de memoria sin cache: un pase de diapositivas. Lo que se
    /// repinta en un bucle es el DANO, no la pantalla.
    ///
    /// ** Es UNA instruccion desde el 09-09, y es literalmente lo que el
    /// comentario de `FB_OP_BYTES` llevaba un mes prediciendo.
    pub fn limpiar(&self, color: u32) {
        // * El tope es el MENOR de los dos, y esto no es prudencia de mas.
        //
        // `pixeles()` mide el area que mapeo el KERNEL, que puede ser mas
        // grande que `stride x alto` (redondeos, padding del firmware). El
        // lienzo mide exactamente `stride x alto`. Recorrer el primero
        // escribiendo en el segundo se sale del bloque de `KIND_MEMORIA` y pisa
        // lo que haya detras -- y como el bloque se pidio contiguo y el
        // asignador da lo siguiente que encuentre, "lo que haya detras" es
        // memoria de alguien.
        //
        // [!] Con `rep stosd` este `min` deja de ser prudente y pasa a ser LA
        // cota: un bucle de mas escribia un pixel de mas por vuelta y se podia
        // notar; una cuenta de mas en `rcx` se lleva el bloque de al lado de un
        // tiron y sin pasar por ninguna comprobacion.
        let n = self.pixeles().min((self.stride as usize) * (self.alto as usize));
        unsafe { rellenar(self.lienzo, color, n) };
        self.marcar(0, 0, self.ancho, self.alto);
    }

    /// Un rectangulo, recortado a la pantalla. Es la unica primitiva de dibujo
    /// que hay, y con ella se hace un escritorio entero: fondo, barra,
    /// ventanas, bordes. Lo demas son estas mismas llamadas puestas en orden.
    ///
    /// ** El arbol del director la llama en **155 sitios**, asi que su coste por
    /// pixel no es un detalle: es el precio del escritorio.
    pub fn rect(&self, x: u32, y: u32, ancho: u32, alto: u32, color: u32) {
        let x1 = (x.saturating_add(ancho)).min(self.ancho);
        let y1 = (y.saturating_add(alto)).min(self.alto);
        if x >= x1 || y >= y1 {
            return;
        }
        // Se marca UNA vez, con las medidas ya recortadas, en vez de un pixel
        // por vuelta: un rectangulo de pantalla completa son millones de
        // llamadas a `marcar` que darian exactamente la misma caja.
        self.marcar(x, y, x1 - x, y1 - y);
        let stride = self.stride as usize;
        let ancho = (x1 - x) as usize;
        // [!] De fila en fila y NO de un tiron: las filas de un rectangulo no
        // son contiguas salvo que ocupe el stride entero, y ese caso ya lo cubre
        // `limpiar`. Una sola llamada con `alto*ancho` pintaria una banda
        // diagonal -- el fallo clasico de confundir el ancho con el stride, y
        // aqui costaria pisar el final del bloque.
        let mut fila = y as usize;
        while fila < y1 as usize {
            unsafe { rellenar(self.lienzo.add(fila * stride + x as usize), color, ancho) };
            fila += 1;
        }
    }

    /// **Empuja a la pantalla lo que se acaba de pintar.**
    ///
    /// * Esto es la otra mitad del write-combining, y sin ella el WC no es una
    /// optimizacion: es un bug.
    ///
    /// Con memoria WC el CPU **acumula** las escrituras en un bufer y las suelta
    /// cuando se llena o cuando algo le obliga. Eso es lo que hace que pintar
    /// sea rapido -- y tambien lo que hace que lo pintado **no llegue** al panel
    /// si el fotograma acaba con el bufer a medias. El escaner de video lee la
    /// memoria, no el bufer.
    ///
    /// El sintoma, dicho por quien lo sufrio: *"cuando muevo el raton tengo que
    /// apuntar bien para que me pinte las escrituras"*. No era el raton: era que
    /// mover el raton genera mas escrituras, el bufer se llenaba, y al vaciarse
    /// aparecia de golpe el texto que se habia tecleado antes.
    ///
    /// `sfence` ordena: nada de lo de despues se ve antes que lo de antes. Es
    /// una instruccion, se hace **una vez por fotograma**, y convierte el WC en
    /// lo que promete.
    ///
    /// * Con doble bufer esto **ademas vuelca**: primero la copia del lienzo al
    /// panel, despues la barrera. Ese orden es el unico que sirve -- la barrera
    /// tiene que cerrar las escrituras del volcado, no las de antes.
    #[inline]
    pub fn vaciar(&self) {
        self.volcar();
        unsafe { core::arch::asm!("sfence", options(nostack, preserves_flags)) };
    }

    /// **Copia al panel lo sucio del lienzo**, y deja la caja vacia.
    ///
    /// [!!] **ESTA FUNCION ENTERA ES PROVISIONAL.** Con un driver de pantalla no
    /// se copia nada: se cambia la direccion que lee el escaner de video y ya
    /// esta (page flip). Es el escalon 8 de `docs/identidad/LA_RAM.md`, y su
    /// bloqueante es que tras `ExitBootServices` el GOP no existe.
    ///
    /// Mover pixeles con el CPU **es trabajo de la GPU hecho por quien no
    /// toca**. Que ahora cueste una instruccion por fila no lo convierte en la
    /// forma correcta: lo convierte en la forma barata de la forma incorrecta.
    ///
    /// Sin doble bufer no hay nada que copiar: lo pintado ya esta en el panel.
    /// Igual se limpia la caja, porque llevarla puesta sin volcar seria mentir
    /// sobre lo que queda pendiente.
    pub fn volcar(&self) {
        let sucias = self.sucio.replace(crate::sin_gpu::sucio::Sucias::nueva());
        if self.lienzo == self.panel || sucias.vacia() {
            return;
        }
        let stride = self.stride as usize;
        for &(x0, y0, x1, y1) in sucias.cajas() {
            let ancho = (x1 - x0) as usize;
            let alto = (y1 - y0) as usize;
            let off = (y0 as usize) * stride + x0 as usize;
            // *** LA CAJA DE PANTALLA COMPLETA ES **UNA SOLA** INSTRUCCION.
            //
            // Cuando la caja ocupa el stride entero, sus filas SI son contiguas
            // en memoria y no hay razon para cortarlas: un `rep movsb` de 8,3 MB
            // de un tiron. Y ese es exactamente el caso que estaba medido y que
            // dolia -- el volcado de pantalla completa de **27,6 ms**, que se
            // come un fotograma y medio de los 16,7 que da el escaner.
            //
            // ** Y no es un caso raro: es cerrar una ventana, mover una ventana,
            // y el primer `vaciar` despues de `activar_doble_bufer`.
            if x0 == 0 && ancho == stride {
                unsafe { copiar(self.panel.add(off), self.lienzo.add(off), ancho * alto) };
                continue;
            }
            let mut fila = 0usize;
            while fila < alto {
                let o = off + fila * stride;
                unsafe { copiar(self.panel.add(o), self.lienzo.add(o), ancho) };
                fila += 1;
            }
        }
        self.anotar(&sucias);
    }

    /// **Asegura que lo escrito se puede LEER.** Llamar antes de [`Self::read`].
    ///
    /// * Existe por el Ep. 25, y hace dos cosas distintas segun donde se dibuje:
    ///
    /// - **Con doble bufer**: nada. El lienzo es RAM normal y cacheada, asi que
    ///   una lectura ve lo que se acaba de escribir. El problema no existe.
    /// - **Sin doble bufer**: `sfence`. Se esta leyendo memoria WC, y una
    ///   lectura de WC **no esta ordenada** contra las escrituras pendientes en
    ///   el bufer: sin barrera devuelve la pantalla de hace un fotograma.
    ///
    /// Que sea un no-op en el camino bueno es justo la gracia: el doble bufer no
    /// arregla el ghosting, lo hace **imposible**.
    #[inline]
    pub fn sincronizar_lectura(&self) {
        if self.lienzo == self.panel {
            unsafe { core::arch::asm!("sfence", options(nostack, preserves_flags)) };
        }
    }

    /// Que hay AHORA en un pixel. Fuera de la pantalla, negro.
    ///
    /// * Se lee del LIENZO, que es donde se ha dibujado. Eso es lo que permite
    /// dibujar el cursor del raton **encima de cualquier cosa**: se guarda lo
    /// que habia debajo y se devuelve al moverlo. Sin esto hay que preguntarle a
    /// un modelo de la escena que deberia haber, y ese modelo se queda corto en
    /// cuanto aparece una ventana que no conoce: el cursor deja agujeros con el
    /// color del fondo por donde pasa.
    ///
    /// Sin doble bufer esto lee memoria de video, que es cara y ademas exige
    /// [`Self::sincronizar_lectura`] antes. Con doble bufer es RAM normal.
    #[inline]
    pub fn read(&self, x: u32, y: u32) -> u32 {
        if x >= self.ancho || y >= self.alto {
            return 0;
        }
        unsafe {
            self.lienzo
                .add((y as usize) * (self.stride as usize) + x as usize)
                .read_volatile()
        }
    }
}
