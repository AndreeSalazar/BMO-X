//! **Las cinco comprobaciones, una prueba cada una que las tumba.**
//!
//! La regla de esta carpeta la fija `docs/metal/VERIFICACION_METAL.md` y vale igual
//! aqui: *"una prueba que solo puede salir bien no prueba nada"*. Asi que cada
//! comprobacion tiene un caso que **la rompe a proposito** y comprueba que sale
//! el motivo CONCRETO -- no que salga un error cualquiera.
//!
//! [!] Y eso es lo que separa esto de un `is_err()`: si `LasCuentasNoCaben`
//! contestara a un magico malo, el lector estaria pasando la prueba mientras
//! manda al que la lea a mirar el sitio equivocado.

use super::*;

extern crate std;
use std::vec::Vec;

/// Un constructor MINIMO, escrito a mano y no con el emisor.
///
/// *** Y eso es a proposito: si estas pruebas usaran `emit::bef`, un fallo del
/// emisor produciria bytes malos que el lector rechazaria... y el banco saldria
/// verde porque *"rechazar es lo que hace"*. **El lector se prueba contra bytes
/// que se saben buenos, no contra la salida de su pareja.**
struct Constructor {
    ancho: u16,
    alto: u16,
    trazos: Vec<[u8; TRAZO]>,
    golpes: Vec<[u8; GOLPE]>,
    cadenas: Vec<u8>,
}

impl Constructor {
    fn nueva() -> Self {
        Constructor {
            ancho: 320,
            alto: 200,
            trazos: Vec::new(),
            golpes: Vec::new(),
            cadenas: Vec::new(),
        }
    }

    /// Mete una cadena y devuelve `(offset, largo)`.
    fn cadena(&mut self, s: &[u8]) -> (u16, u16) {
        let off = self.cadenas.len() as u16;
        self.cadenas.extend_from_slice(s);
        (off, s.len() as u16)
    }

    fn rect(&mut self, x: i16, y: i16, w: u16, h: u16, color: u32) -> &mut Self {
        let mut t = [0u8; TRAZO];
        t[trazo::CLASE] = CLASE_RECT;
        t[trazo::ESTADO] = ESTADO_REPOSO;
        t[trazo::X..trazo::X + 2].copy_from_slice(&x.to_le_bytes());
        t[trazo::Y..trazo::Y + 2].copy_from_slice(&y.to_le_bytes());
        t[trazo::W..trazo::W + 2].copy_from_slice(&w.to_le_bytes());
        t[trazo::H..trazo::H + 2].copy_from_slice(&h.to_le_bytes());
        t[trazo::COLOR..trazo::COLOR + 4].copy_from_slice(&color.to_le_bytes());
        self.trazos.push(t);
        self
    }

    fn texto(&mut self, x: i16, y: i16, w: u16, h: u16, color: u32, s: &[u8]) -> &mut Self {
        let (off, len) = self.cadena(s);
        let mut t = [0u8; TRAZO];
        t[trazo::CLASE] = CLASE_TEXTO;
        t[trazo::ESTADO] = ESTADO_REPOSO;
        t[trazo::X..trazo::X + 2].copy_from_slice(&x.to_le_bytes());
        t[trazo::Y..trazo::Y + 2].copy_from_slice(&y.to_le_bytes());
        t[trazo::W..trazo::W + 2].copy_from_slice(&w.to_le_bytes());
        t[trazo::H..trazo::H + 2].copy_from_slice(&h.to_le_bytes());
        t[trazo::COLOR..trazo::COLOR + 4].copy_from_slice(&color.to_le_bytes());
        t[trazo::CAD_OFF..trazo::CAD_OFF + 2].copy_from_slice(&off.to_le_bytes());
        t[trazo::CAD_LEN..trazo::CAD_LEN + 2].copy_from_slice(&len.to_le_bytes());
        self.trazos.push(t);
        self
    }

    fn golpe(&mut self, x: i16, y: i16, w: u16, h: u16, nombre: &[u8]) -> &mut Self {
        let (off, len) = self.cadena(nombre);
        let mut g = [0u8; GOLPE];
        g[golpe::X..golpe::X + 2].copy_from_slice(&x.to_le_bytes());
        g[golpe::Y..golpe::Y + 2].copy_from_slice(&y.to_le_bytes());
        g[golpe::W..golpe::W + 2].copy_from_slice(&w.to_le_bytes());
        g[golpe::H..golpe::H + 2].copy_from_slice(&h.to_le_bytes());
        g[golpe::CAD_OFF..golpe::CAD_OFF + 2].copy_from_slice(&off.to_le_bytes());
        g[golpe::CAD_LEN..golpe::CAD_LEN + 2].copy_from_slice(&len.to_le_bytes());
        self.golpes.push(g);
        self
    }

    fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&MAGICO.to_le_bytes());
        b.extend_from_slice(&VERSION.to_le_bytes());
        b.extend_from_slice(&self.ancho.to_le_bytes());
        b.extend_from_slice(&self.alto.to_le_bytes());
        b.extend_from_slice(&(self.trazos.len() as u16).to_le_bytes());
        b.extend_from_slice(&(self.golpes.len() as u16).to_le_bytes());
        b.extend_from_slice(&(self.cadenas.len() as u16).to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes()); // reservado
        assert_eq!(b.len(), CABECERA, "la cabecera tiene que medir lo declarado");
        for t in &self.trazos {
            b.extend_from_slice(t);
        }
        for g in &self.golpes {
            b.extend_from_slice(g);
        }
        b.extend_from_slice(&self.cadenas);
        b
    }
}

fn buena() -> Vec<u8> {
    let mut c = Constructor::nueva();
    c.rect(0, 0, 320, 200, 0xFF102030);
    c.rect(10, 10, 100, 40, 0xFF445566);
    c.texto(14, 20, 90, 16, 0xFFFFFFFF, b"siete");
    c.golpe(10, 10, 100, 40, b"#boton7");
    c.bytes()
}

/// El caso bueno, y lo que trae dentro. Va primero porque **si este falla, los
/// nueve de abajo pasarian por el motivo equivocado**.
#[test]
fn una_cara_bien_hecha_se_abre_y_dice_lo_que_lleva() {
    let b = buena();
    let cara = leer(&b, 1920, 1080).expect("tiene que abrir");
    assert_eq!(cara.lienzo(), (320, 200));
    assert_eq!(cara.trazos(), 3);
    assert_eq!(cara.golpes(), 1);

    let t0 = cara.trazo(0).unwrap();
    assert_eq!(t0.clase, CLASE_RECT);
    assert_eq!((t0.x, t0.y, t0.w, t0.h), (0, 0, 320, 200));
    assert_eq!(t0.color, 0xFF102030);
    assert_eq!(t0.texto, b"", "un rect no trae letras");

    let t2 = cara.trazo(2).unwrap();
    assert_eq!(t2.clase, CLASE_TEXTO);
    assert_eq!(t2.texto, b"siete");

    let g = cara.golpe(0).unwrap();
    assert_eq!(g.nombre, b"#boton7");

    // Fuera de rango contesta `None`, no basura ni panico.
    assert!(cara.trazo(3).is_none());
    assert!(cara.golpe(1).is_none());
}

// -- LAS CINCO COMPROBACIONES, UNA A UNA ------------------------------------

/// **1a.** Cuatro bytes que no son `CARA`.
#[test]
fn comprobacion_1_un_magico_que_no_es() {
    let mut b = buena();
    b[cabecera::MAGICO] ^= 0xFF;
    assert_eq!(leer(&b, 1920, 1080).unwrap_err(), Falta::NoEsUnaCara);
}

/// **1b.** Una version futura se rechaza. **Por igualdad, no por "mayor o
/// igual"**: aceptar el futuro es prometer entender algo que no existe.
#[test]
fn comprobacion_1_una_version_que_no_entiendo() {
    let mut b = buena();
    b[cabecera::VERSION..cabecera::VERSION + 2].copy_from_slice(&2u16.to_le_bytes());
    assert_eq!(leer(&b, 1920, 1080).unwrap_err(), Falta::OtraVersion);
}

/// **1c.** El reservado sucio. Es la senal mas barata de que esto viene de otro
/// sitio: nadie escribe ahi por accidente.
#[test]
fn comprobacion_1_el_reservado_tiene_que_estar_limpio() {
    let mut b = buena();
    b[cabecera::RESERVADO] = 1;
    assert_eq!(leer(&b, 1920, 1080).unwrap_err(), Falta::ReservadoSucio);

    // Y tambien el de dentro de un trazo, que es el hueco por donde crecera el
    // formato algun dia.
    let mut b = buena();
    b[CABECERA + trazo::RESERVADO] = 0xAA;
    assert_eq!(leer(&b, 1920, 1080).unwrap_err(), Falta::ReservadoSucio);
}

/// **2.** *** LA QUE SOSTIENE A LAS DEMAS. Una cabecera que declara mas trazos
/// de los que hay bytes.
///
/// Sin esta, la 3 y la 4 leerian sus campos de mas alla del final del buffer --
/// o sea que estarian comprobando basura y diciendo que todo bien.
#[test]
fn comprobacion_2_las_cuentas_no_pueden_pedir_mas_de_lo_que_hay() {
    let mut b = buena();
    b[cabecera::N_TRAZOS..cabecera::N_TRAZOS + 2].copy_from_slice(&9999u16.to_le_bytes());
    assert_eq!(leer(&b, 1920, 1080).unwrap_err(), Falta::LasCuentasNoCaben);
}

/// **2-bis.** El mismo ataque por el bloque de cadenas.
#[test]
fn comprobacion_2_las_cadenas_tampoco() {
    let mut b = buena();
    b[cabecera::CADENAS..cabecera::CADENAS + 2].copy_from_slice(&60000u16.to_le_bytes());
    assert_eq!(leer(&b, 1920, 1080).unwrap_err(), Falta::LasCuentasNoCaben);
}

/// **3.** Una cadena que se sale del bloque de cadenas. Es el equivalente exacto
/// del `4000x4000 en 1 MiB` que el compositor ya sabe rechazar.
#[test]
fn comprobacion_3_una_cadena_que_apunta_fuera() {
    let mut b = buena();
    // El trazo 2 es el texto; se le estira el largo hasta pasarse.
    let t = CABECERA + 2 * TRAZO;
    b[t + trazo::CAD_LEN..t + trazo::CAD_LEN + 2].copy_from_slice(&5000u16.to_le_bytes());
    assert_eq!(leer(&b, 1920, 1080).unwrap_err(), Falta::CadenaFuera);
}

/// **3-bis.** Y por el nombre de un golpe, que es la otra tabla que indexa
/// cadenas. Se prueba aparte porque **son dos bucles distintos**, y arreglar uno
/// no arregla el otro.
#[test]
fn comprobacion_3_el_nombre_de_un_golpe_tambien() {
    let mut b = buena();
    let g = CABECERA + 3 * TRAZO;
    b[g + golpe::CAD_OFF..g + golpe::CAD_OFF + 2].copy_from_slice(&60000u16.to_le_bytes());
    assert_eq!(leer(&b, 1920, 1080).unwrap_err(), Falta::CadenaFuera);
}

/// **4.** Un rect que se sale del lienzo que la propia cara declara.
#[test]
fn comprobacion_4_un_trazo_fuera_del_lienzo() {
    let mut b = buena();
    let t = CABECERA + TRAZO; // el segundo rect
    b[t + trazo::W..t + trazo::W + 2].copy_from_slice(&5000u16.to_le_bytes());
    assert_eq!(leer(&b, 1920, 1080).unwrap_err(), Falta::TrazoFueraDelLienzo);
}

/// **4-bis.** Y una `x` NEGATIVA, que es el caso que un `x + w > ancho` a secas
/// deja pasar: con `x = -100` y `w = 50` la suma da `-50`, que es menor que el
/// ancho y **parece que cabe**.
#[test]
fn comprobacion_4_una_coordenada_negativa_no_cabe_aunque_la_suma_diga_que_si() {
    let mut b = buena();
    let t = CABECERA + TRAZO;
    b[t + trazo::X..t + trazo::X + 2].copy_from_slice(&(-100i16).to_le_bytes());
    b[t + trazo::W..t + trazo::W + 2].copy_from_slice(&50u16.to_le_bytes());
    assert_eq!(leer(&b, 1920, 1080).unwrap_err(), Falta::TrazoFueraDelLienzo);
}

/// **5.** El lienzo mas grande que la pantalla que hay.
///
/// La pantalla se PASA y no se supone: la misma cara es legal en 1920x1080 y
/// absurda en 640x480, y el fichero no puede ser quien diga cual hay delante.
#[test]
fn comprobacion_5_el_lienzo_no_cabe_en_esta_pantalla() {
    let b = buena(); // declara 320x200
    assert!(leer(&b, 1920, 1080).is_ok());
    assert_eq!(
        leer(&b, 200, 100).unwrap_err(),
        Falta::LienzoMasGrandeQueLaPantalla
    );
}

/// **5-bis.** Un lienzo de cero contesta `LienzoVacio` y **no**
/// `TrazoFueraDelLienzo`.
///
/// *** Esta prueba no cuida el codigo: cuida el MOTIVO. Con el orden al reves,
/// una cara con el lienzo a cero mandaria al que la lea a mirar los trazos --
/// que estan bien-- en vez de a la cabecera, que es donde esta el roto.
#[test]
fn comprobacion_5_un_lienzo_vacio_se_denuncia_por_su_nombre() {
    let mut b = buena();
    b[cabecera::ANCHO..cabecera::ANCHO + 2].copy_from_slice(&0u16.to_le_bytes());
    assert_eq!(leer(&b, 1920, 1080).unwrap_err(), Falta::LienzoVacio);
}

/// Un fichero mas corto que la cabecera. El caso mas tonto y el primero que
/// llega cuando algo se lee a medias del disco.
#[test]
fn un_fichero_que_no_llega_ni_a_la_cabecera() {
    for n in 0..CABECERA {
        let b = buena();
        assert_eq!(
            leer(&b[..n], 1920, 1080).unwrap_err(),
            Falta::NoLlegaNiALaCabecera,
            "con {n} bytes no hay cabecera"
        );
    }
}

/// **NADA ENTRA EN PANICO, VENGA LO QUE VENGA.**
///
/// Se corrompe cada byte de una cara buena, uno por uno, y lo unico que se exige
/// es que `leer` **conteste**. Es la misma prueba que `bmo-bex-gate` le hace a
/// su cabecera, y esta aqui por el mismo motivo: en Ring 3 un panico del
/// compositor no es un test rojo, es el escritorio caido.
#[test]
fn ningun_byte_corrompido_hace_estallar_al_lector() {
    let base = buena();
    for i in 0..base.len() {
        for v in [0x00u8, 0x01, 0x7F, 0x80, 0xFF] {
            let mut b = base.clone();
            b[i] = v;
            let _ = leer(&b, 1920, 1080);
        }
    }
}
