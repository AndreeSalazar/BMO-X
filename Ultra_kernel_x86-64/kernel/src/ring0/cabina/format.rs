//! **FORMATTING WITHOUT `std`** -- a line built in a byte buffer.
//!
//! === Why this is a file of its own ===
//!
//! Because Ring 0 has no allocator and no `format!`, so every line CABINA
//! prints is assembled by hand into a fixed array. It is small, it is dull, and
//! it is used by every other file in this folder -- which is exactly the
//! profile of something that should be one file and not six copies.
//!
//! [!] The buffer is fixed and the writes are bounded. A formatter that can
//! overrun in a panic handler turns a diagnostic into a second fault.

use super::*;

// -- Formateo sin std (buffer de bytes) --------------------------------------

pub(crate) struct Buf {
    b: [u8; 96],
    o: usize,
}
impl Buf {
    pub(crate) fn new() -> Self { Self { b: [0u8; 96], o: 0 } }
    pub(crate) fn txt(&mut self, s: &str) {
        for &c in s.as_bytes() { if self.o < self.b.len() { self.b[self.o] = c; self.o += 1; } }
    }
    pub(crate) fn dec(&mut self, mut v: u64) {
        if v == 0 { self.txt("0"); return; }
        let mut tmp = [0u8; 20]; let mut i = 0;
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        while i > 0 { i -= 1; if self.o < self.b.len() { self.b[self.o] = tmp[i]; self.o += 1; } }
    }
    /// Decimal alineado a la derecha en `width` -- mantiene las columnas de la
    /// bitacora quietas aunque el numero crezca.
    pub(crate) fn dec_pad(&mut self, v: u64, width: usize) {
        let mut digits = 1; let mut t = v;
        while t >= 10 { t /= 10; digits += 1; }
        for _ in digits..width { self.txt(" "); }
        self.dec(v);
    }
    pub(crate) fn hex(&mut self, v: u64, digits: usize) {
        const H: &[u8; 16] = b"0123456789ABCDEF";
        for i in (0..digits).rev() {
            if self.o < self.b.len() { self.b[self.o] = H[((v >> (i * 4)) & 0xF) as usize]; self.o += 1; }
        }
    }
    /// Hex sin ceros a la izquierda -- para el `value` del evento, que puede ser
    /// una direccion MMIO o un contador de 2 digitos.
    pub(crate) fn hex_min(&mut self, v: u64) {
        if v == 0 { self.txt("0"); return; }
        let mut digits = 1;
        let mut t = v >> 4;
        while t > 0 { t >>= 4; digits += 1; }
        self.hex(v, digits);
    }
    // -- ** THE TRANSLATION, AND IT LIVES SOMEWHERE IT CAN BE RUN -----------
    //
    // The decisions -- how a size reads, where a page boundary matters, the byte
    // order of a MAC -- are in `cabina_core::legible`, with tests. They are NOT
    // here, and that is on purpose: this crate cannot be tested (`cargo test`
    // links `std`, the kernel is `no_std`), so anything living here is verified
    // by reading it. Formatting is exactly the code that is wrong QUIETLY: it
    // prints something plausible, and a plausible wrong number gets believed.
    //
    // This project has that scar already -- nine floating-point tests in the C
    // frontend are green and none of them executes.
    //
    // What stays here is only what no test can cover: putting bytes on a screen.

    /// The event value, **read the way its emitter said it should be**.
    ///
    /// `Fmt::Raw` is the historical behaviour, so every one of the two hundred
    /// call sites that has not been migrated prints exactly as it did before any
    /// of this existed.
    pub(crate) fn value_of(&mut self, ev: &Event) {
        use cabina_core::legible as leg;
        // The remaining room of this line, handed to the tested writer. Sharing
        // the cursor instead of a scratch buffer is what keeps the truncation
        // rule in ONE place: `Escritor` already drops what does not fit.
        let mut w = leg::Escritor::new(&mut self.b[self.o..]);
        match ev.fmt {
            Fmt::Raw => w.hex_min(ev.value),
            Fmt::Count | Fmt::Id => w.dec(ev.value),
            Fmt::Bytes => leg::size(&mut w, ev.value),
            Fmt::Addr => leg::address(&mut w, ev.value),
            Fmt::Millis => { w.dec(ev.value); w.txt(" ms"); }
            Fmt::Mac => leg::mac(&mut w, ev.value),
            Fmt::Bits => leg::bits(&mut w, ev.value),
        }
        let escrito = w.len();
        self.o += escrito;
    }

    /// Cuanto se lleva escrito. Lo necesita quien tiene que REPARTIR el ancho
    /// de una linea entre varias piezas antes de escribir ninguna.
    pub(crate) fn len(&self) -> usize { self.o }

    /// Texto **que cede**: escribe como mucho `max` caracteres, y si no cabe
    /// entero deja un `~` en el ultimo para que se vea que falta algo.
    ///
    /// # Por que existe, y por que la marca no es opcional
    ///
    /// Recortar en silencio es lo que convirtio una frase en una mentira: una
    /// linea cortada se lee como una linea completa, y entonces `=1100` se lee
    /// como el valor `0x1100` en vez de como *"los cuatro primeros digitos de
    /// algo que no cabia"*. Las dos cosas mandan a mirar sitios distintos.
    pub(crate) fn txt_max(&mut self, s: &str, max: usize) {
        if max == 0 { return; }
        if s.len() <= max { self.txt(s); return; }
        // `is_char_boundary` no hace falta: la consola es de un byte por
        // caracter y todos los mensajes del kernel son ASCII (regla del
        // proyecto, la vigila `ascii-sweep`). Aun asi se corta por bytes, que es
        // lo unico que esta funcion promete.
        self.txt(&s[..max - 1]);
        self.txt("~");
    }

    /// Texto a ancho fijo (recorta o rellena) -- columnas estables.
    pub(crate) fn pad(&mut self, s: &str, width: usize) {
        let n = s.len().min(width);
        self.txt(&s[..n]);
        for _ in n..width { self.txt(" "); }
    }
    pub(crate) fn as_str(&self) -> &str { core::str::from_utf8(&self.b[..self.o]).unwrap_or("") }
}
