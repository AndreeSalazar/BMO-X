//! BLAKE3 â€” implementaciÃ³n nativa no_std para BMO.
//!
//! Spec: https://github.com/BLAKE3-team/BLAKE3-specs (v20211102).
//!
//! Soporta hash arbitrariamente largo en modo Ã¡rbol (chunks de 1024 B
//! combinados pairwise hasta llegar a la raÃ­z). Single-thread, suficiente
//! para verificar secciones BEF al cargar (~1 GB/s en Zen 3).

//! ## Por qué esto es una crate y no un módulo de `bmo-abi`
//!
//! ESTRATOS exige **un solo algoritmo de hash en todo el sistema**: contenido,
//! firmas y verificación tienen que hablar el mismo idioma, o dos capas
//! calculan sumas distintas del mismo bloque y nadie se entera hasta que un
//! archivo no cuadra.
//!
//! Vivía dentro de `bmo-abi`, que arrastra `alloc` — y en Ring 0 no hay
//! reservas de memoria. El sistema de ficheros del kernel no puede depender de
//! toda la ABI para calcular una suma. Así que el hash sale a su propia crate,
//! sin dependencias, y `bmo-abi` pasa a reexportarla: **la implementación
//! sigue siendo una sola**, que es lo que importaba.

#![cfg_attr(not(test), no_std)]
#![allow(dead_code)]

// Los alias de `bmo-abi` no viajan aquí: son de la ABI, y esta crate no
// depende de nada a propósito. Se definen localmente para no tocar ni una
// línea del algoritmo al mudarlo.
#[allow(non_camel_case_types)] type bx_u8 = u8;
#[allow(non_camel_case_types)] type bx_u32 = u32;
#[allow(non_camel_case_types)] type bx_u64 = u64;

// â”€â”€â”€ Constantes â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
const OUT_LEN: usize = 32;
const KEY_LEN: usize = 32;
const BLOCK_LEN: usize = 64;
const CHUNK_LEN: usize = 1024;

// Flags
const CHUNK_START: bx_u32 = 1 << 0;
const CHUNK_END: bx_u32 = 1 << 1;
const PARENT: bx_u32 = 1 << 2;
const ROOT: bx_u32 = 1 << 3;

// IV (idÃ©ntico a SHA-256)
const IV: [bx_u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];

// PermutaciÃ³n de mensaje aplicada antes de cada ronda (6 veces).
const MSG_PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

// â”€â”€â”€ G function + rounds â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
#[inline(always)]
fn g(state: &mut [bx_u32; 16], a: usize, b: usize, c: usize, d: usize, mx: bx_u32, my: bx_u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

#[inline(always)]
fn round(state: &mut [bx_u32; 16], m: &[bx_u32; 16]) {
    // Column rounds
    g(state, 0, 4, 8, 12, m[0], m[1]);
    g(state, 1, 5, 9, 13, m[2], m[3]);
    g(state, 2, 6, 10, 14, m[4], m[5]);
    g(state, 3, 7, 11, 15, m[6], m[7]);
    // Diagonal rounds
    g(state, 0, 5, 10, 15, m[8], m[9]);
    g(state, 1, 6, 11, 12, m[10], m[11]);
    g(state, 2, 7, 8, 13, m[12], m[13]);
    g(state, 3, 4, 9, 14, m[14], m[15]);
}

#[inline]
fn permute(m: &mut [bx_u32; 16]) {
    let original = *m;
    for i in 0..16 {
        m[i] = original[MSG_PERMUTATION[i]];
    }
}

/// CompresiÃ³n de un bloque de 64 bytes. Devuelve el state de 16 palabras.
fn compress(
    chaining: &[bx_u32; 8],
    block: &[bx_u32; 16],
    counter: bx_u64,
    block_len: bx_u32,
    flags: bx_u32,
) -> [bx_u32; 16] {
    let mut state: [bx_u32; 16] = [
        chaining[0],
        chaining[1],
        chaining[2],
        chaining[3],
        chaining[4],
        chaining[5],
        chaining[6],
        chaining[7],
        IV[0],
        IV[1],
        IV[2],
        IV[3],
        counter as bx_u32,
        (counter >> 32) as bx_u32,
        block_len,
        flags,
    ];
    let mut m = *block;
    round(&mut state, &m); // 1
    permute(&mut m);
    round(&mut state, &m); // 2
    permute(&mut m);
    round(&mut state, &m); // 3
    permute(&mut m);
    round(&mut state, &m); // 4
    permute(&mut m);
    round(&mut state, &m); // 5
    permute(&mut m);
    round(&mut state, &m); // 6
    permute(&mut m);
    round(&mut state, &m); // 7

    // XOR del state superior con el inferior (extended output truncado).
    for i in 0..8 {
        state[i] ^= state[i + 8];
        state[i + 8] ^= chaining[i];
    }
    state
}

/// Convierte un buffer de 64 bytes a un array de 16 palabras LE.
fn words_from_bytes(bytes: &[u8; BLOCK_LEN]) -> [bx_u32; 16] {
    let mut out = [0u32; 16];
    for i in 0..16 {
        let off = i * 4;
        out[i] = u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]]);
    }
    out
}

// â”€â”€â”€ Chunk state â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
struct ChunkState {
    chaining: [bx_u32; 8],
    chunk_counter: bx_u64,
    block: [bx_u8; BLOCK_LEN],
    block_len: bx_u8,
    blocks_compressed: bx_u8,
    flags: bx_u32,
}

impl ChunkState {
    fn new(key: &[bx_u32; 8], chunk_counter: bx_u64, flags: bx_u32) -> Self {
        Self {
            chaining: *key,
            chunk_counter,
            block: [0; BLOCK_LEN],
            block_len: 0,
            blocks_compressed: 0,
            flags,
        }
    }

    fn len(&self) -> usize {
        self.blocks_compressed as usize * BLOCK_LEN + self.block_len as usize
    }

    fn start_flag(&self) -> bx_u32 {
        if self.blocks_compressed == 0 {
            CHUNK_START
        } else {
            0
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            // Si el bloque actual estÃ¡ lleno, comprimirlo.
            if self.block_len as usize == BLOCK_LEN {
                let block_words = words_from_bytes(&self.block);
                let new_state = compress(
                    &self.chaining,
                    &block_words,
                    self.chunk_counter,
                    BLOCK_LEN as bx_u32,
                    self.flags | self.start_flag(),
                );
                self.chaining = [
                    new_state[0],
                    new_state[1],
                    new_state[2],
                    new_state[3],
                    new_state[4],
                    new_state[5],
                    new_state[6],
                    new_state[7],
                ];
                self.blocks_compressed += 1;
                self.block = [0; BLOCK_LEN];
                self.block_len = 0;
            }
            // Copiar tantos bytes como quepan en el bloque actual.
            let want = BLOCK_LEN - self.block_len as usize;
            let take = core::cmp::min(want, input.len());
            self.block[self.block_len as usize..][..take].copy_from_slice(&input[..take]);
            self.block_len += take as u8;
            input = &input[take..];
        }
    }

    /// Cierra el chunk actual y devuelve su CV (chaining value de 8 palabras).
    /// Si `is_root` es true, devuelve directamente el hash final del root.
    fn output(&self, is_root: bool) -> [bx_u32; 8] {
        let mut block_words = [0u32; 16];
        for i in 0..16 {
            let off = i * 4;
            block_words[i] = u32::from_le_bytes([
                self.block[off],
                self.block[off + 1],
                self.block[off + 2],
                self.block[off + 3],
            ]);
        }
        let mut flags = self.flags | self.start_flag() | CHUNK_END;
        if is_root {
            flags |= ROOT;
        }
        let state = compress(
            &self.chaining,
            &block_words,
            self.chunk_counter,
            self.block_len as bx_u32,
            flags,
        );
        [
            state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7],
        ]
    }
}

/// Combina dos CVs en un nodo padre, devolviendo el CV combinado.
fn parent_cv(
    left: &[bx_u32; 8],
    right: &[bx_u32; 8],
    key: &[bx_u32; 8],
    flags: bx_u32,
    is_root: bool,
) -> [bx_u32; 8] {
    let mut block = [0u32; 16];
    block[..8].copy_from_slice(left);
    block[8..].copy_from_slice(right);
    let mut f = flags | PARENT;
    if is_root {
        f |= ROOT;
    }
    let state = compress(key, &block, 0, BLOCK_LEN as bx_u32, f);
    [
        state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7],
    ]
}

/// Hasher BLAKE3 stateful â€” modo "regular hash" (sin keyed/derive).
pub struct Hasher {
    chunk: ChunkState,
    /// Stack de CVs pendientes de combinar.
    cv_stack: [[bx_u32; 8]; 54], // 54 niveles â†’ input mÃ¡ximo 2^54 chunks
    cv_stack_len: u8,
}

impl Hasher {
    pub fn new() -> Self {
        Self {
            chunk: ChunkState::new(&IV, 0, 0),
            cv_stack: [[0u32; 8]; 54],
            cv_stack_len: 0,
        }
    }

    fn push_cv(&mut self, cv: [bx_u32; 8], total_chunks: bx_u64) {
        // Lo combinamos pairwise mientras los dos topes pertenezcan al mismo subÃ¡rbol.
        let mut new_cv = cv;
        let mut total = total_chunks;
        while total & 1 == 0 {
            let left = self.cv_stack[self.cv_stack_len as usize - 1];
            self.cv_stack_len -= 1;
            new_cv = parent_cv(&left, &new_cv, &IV, 0, false);
            total >>= 1;
        }
        self.cv_stack[self.cv_stack_len as usize] = new_cv;
        self.cv_stack_len += 1;
    }

    pub fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            if self.chunk.len() == CHUNK_LEN {
                let cv = self.chunk.output(false);
                let chunks_so_far = self.chunk.chunk_counter + 1;
                self.push_cv(cv, chunks_so_far);
                self.chunk = ChunkState::new(&IV, chunks_so_far, 0);
            }
            let want = CHUNK_LEN - self.chunk.len();
            let take = core::cmp::min(want, input.len());
            self.chunk.update(&input[..take]);
            input = &input[take..];
        }
    }

    pub fn finalize(self) -> [u8; OUT_LEN] {
        // Caso 1: solo un chunk (no hay nada en el stack).
        if self.cv_stack_len == 0 {
            return cv_to_bytes(self.chunk.output(true));
        }
        // Caso 2: vaciar el stack hacia la raÃ­z.
        let mut output_cv = self.chunk.output(false);
        let mut idx = self.cv_stack_len as usize;
        while idx > 0 {
            idx -= 1;
            let is_root = idx == 0;
            output_cv = parent_cv(&self.cv_stack[idx], &output_cv, &IV, 0, is_root);
        }
        cv_to_bytes(output_cv)
    }
}

#[inline]
fn cv_to_bytes(cv: [bx_u32; 8]) -> [u8; OUT_LEN] {
    let mut out = [0u8; OUT_LEN];
    for i in 0..8 {
        out[i * 4..i * 4 + 4].copy_from_slice(&cv[i].to_le_bytes());
    }
    out
}

/// Hash one-shot â€” atajo para hashear un buffer Ãºnico.
pub fn hash(bytes: &[u8]) -> [u8; OUT_LEN] {
    let mut h = Hasher::new();
    h.update(bytes);
    h.finalize()
}

#[cfg(test)]
mod vectores_oficiales {
    use super::*;

    // Vectores de prueba OFICIALES de BLAKE3, tomados de
    // https://github.com/BLAKE3-team/BLAKE3/blob/master/test_vectors/test_vectors.json
    //
    // Por que esto importa mas que cualquier otro test del repo: esta
    // implementacion se escribio a mano y NADIE habia comprobado nunca que
    // produjera los hashes correctos. Y de ella cuelga todo — las firmas del
    // BEF, el gate de bmo-verify, las sumas del superbloque de ESTRATOS y el
    // direccionamiento por contenido. Un fallo aqui no da un error: da un
    // sistema entero que se cree integro y no lo es.
    //
    // La entrada de longitud N son los bytes 0,1,2,...,250,0,1,... (ciclo de
    // 251, que es primo para que ningun tamano de bloque se alinee con el).
    fn entrada(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i % 251) as u8).collect()
    }

    fn hex(bytes: &[u8]) -> String {
        let mut s = String::new();
        for b in bytes {
            s.push_str(&format!("{:02x}", b));
        }
        s
    }

    /// (longitud de entrada, primeros 32 bytes del hash en hex)
    const VECTORES: &[(usize, &str)] = &[
        (0,      "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"),
        (1,      "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213"),
        (2,      "7b7015bb92cf0b318037702a6cdd81dee41224f734684c2c122cd6359cb1ee63"),
        (3,      "e1be4d7a8ab5560aa4199eea339849ba8e293d55ca0a81006726d184519e647f"),
        (4,      "f30f5ab28fe047904037f77b6da4fea1e27241c5d132638d8bedce9d40494f32"),
        (5,      "b40b44dfd97e7a84a996a91af8b85188c66c126940ba7aad2e7ae6b385402aa2"),
        (6,      "06c4e8ffb6872fad96f9aaca5eee1553eb62aed0ad7198cef42e87f6a616c844"),
        (7,      "3f8770f387faad08faa9d8414e9f449ac68e6ff0417f673f602a646a891419fe"),
        (8,      "2351207d04fc16ade43ccab08600939c7c1fa70a5c0aaca76063d04c3228eaeb"),
        // 63/64/65: el borde del BLOQUE de compresion (64 B).
        (63,     "e9bc37a594daad83be9470df7f7b3798297c3d834ce80ba85d6e207627b7db7b"),
        (64,     "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98"),
        (65,     "de1e5fa0be70df6d2be8fffd0e99ceaa8eb6e8c93a63f2d8d1c30ecb6b263dee"),
        // 1023/1024/1025: el borde del CHUNK (1024 B), donde arranca el modo
        // arbol. Es el sitio donde una implementacion a mano se rompe.
        (1023,   "10108970eeda3eb932baac1428c7a2163b0e924c9a9e25b35bba72b28f70bd11"),
        (1024,   "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7"),
        (1025,   "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444"),
        // Varios niveles de arbol.
        (2048,   "e776b6028c7cd22a4d0ba182a8bf62205d2ef576467e838ed6f2529b85fba24a"),
        (2049,   "5f4d72f40d7a5f82b15ca2b2e44b1de3c2ef86c426c95c1af0b6879522563030"),
        (3072,   "b98cb0ff3623be03326b373de6b9095218513e64f1ee2edd2525c7ad1e5cffd2"),
        (4096,   "015094013f57a5277b59d8475c0501042c0b642e531b0a1c8f58d2163229e969"),
        (102400, "bc3e3d41a1146b069abffad3c0d44860cf664390afce4d9661f7902e7943e085"),
    ];

    #[test]
    fn coincide_con_los_vectores_oficiales() {
        for &(n, esperado) in VECTORES {
            let obtenido = hex(&hash(&entrada(n)));
            assert_eq!(obtenido, esperado, "BLAKE3 falla con entrada de {} bytes", n);
        }
    }

    #[test]
    fn alimentarlo_a_trozos_da_lo_mismo_que_de_una_vez() {
        // El fallo mas probable de un hash escrito a mano no es la funcion de
        // compresion: es el BUFFER. Si `update` no acumula bien los restos
        // entre llamadas, hashear "abc" de una vez y en tres trozos da
        // resultados distintos — y en un sistema de ficheros los datos SIEMPRE
        // llegan a trozos.
        for &(n, esperado) in VECTORES {
            if n == 0 { continue; }
            for trozo in [1usize, 7, 63, 64, 65, 1024, 1031] {
                if trozo > n { continue; }
                let datos = entrada(n);
                let mut h = Hasher::new();
                for parte in datos.chunks(trozo) {
                    h.update(parte);
                }
                assert_eq!(hex(&h.finalize()), esperado,
                    "BLAKE3 falla con {} bytes alimentados en trozos de {}", n, trozo);
            }
        }
    }
}
