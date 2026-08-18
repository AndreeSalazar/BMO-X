//! **`guia` -- por donde empezar.** La orden que faltaba, y la pidio quien lo
//! escribio todo.
//!
//! === Por que existe, dicho con las palabras del dueno ===
//!
//! *"la verdad no hay archivos y eso es algo que me puse a pensar... eso tendria
//! que poner guias, ironicamente yo como creador no se usar"*.
//!
//! ** Y no era torpeza suya: era que el sistema no lo decia. El malentendido
//! concreto tenia respuesta exacta y no estaba escrita en ningun sitio --
//! `ls` lista la **FAT32**, no ESTRATOS; ESTRATOS sale vacio porque **todavia no
//! sabe guardar un fichero**, no porque se hayan perdido. Nadie podia deducir
//! eso de la pantalla.
//!
//! === Por que NO es un tercer catalogo ===
//!
//! Esta casa acaba de matar dos listas de ordenes que se pudrieron --`presta` en
//! la ayuda, `sella` en la pista de la ventana-- y una guia escrita como lista
//! seria la tercera.
//!
//! Asi que esta guia va por **TAREAS**, no por ordenes: *"quiero ver la maquina",
//! "quiero mirar el disco", "quiero correr algo"*. Las tareas de un sistema
//! operativo cambian cada muchos meses; su lista de verbos, cada semana. La
//! lista entera vive en `ayuda` y en un solo sitio.
//!
//! [!] Y el ultimo bloque es el que de verdad hacia falta: **lo que todavia NO
//! se puede**. Un sistema que solo anuncia lo que hace deja al que lo usa
//! buscando durante media hora algo que no existe -- que es exactamente lo que
//! acaba de pasar.

use bmo_userland as bmo;

use super::reports::section;
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ECHO, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};

pub(crate) fn guia(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    let s = &mut dsk.out.grid;
    section(s, b"por donde empezar");
    s.text(b"    esto no es Linux ni Windows: no hay usuarios, ni paquetes, ni\n");
    s.text(b"    root. Hay capabilities, y lo que no te dieron no existe.\n");
    s.with_ink(INK_ECHO);
    s.text(b"  1  VER LA MAQUINA\n");
    s.with_ink(INK_PLAIN);
    s.text(b"       info            RAM, CPU, tareas y disco de un vistazo\n");
    s.text(b"       consumo         que esta gastando AHORA, en tabla\n");
    s.with_ink(INK_ECHO);
    s.text(b"  2  MIRAR EL DISCO\n");
    s.with_ink(INK_PLAIN);
    s.text(b"       disco           que aparato es y cuanto queda\n");
    s.text(b"       disco trim      devolverle al SSD lo que ya no usa nadie\n");
    s.with_ink(INK_ECHO);
    s.text(b"  3  ABRIR LO QUE HAY\n");
    s.with_ink(INK_PLAIN);
    s.text(b"       ls              que archivos hay        ls datos\n");
    s.text(b"       cat             que hay dentro          cat datos/movim.txt\n");
    s.with_ink(INK_ECHO);
    s.text(b"  4  CORRER UN PROGRAMA\n");
    s.with_ink(INK_PLAIN);
    s.text(b"       cobol/2/banco.bex     escribe la ruta y Enter\n");
    s.text(b"       c/ray.bex             pide pantalla: se le presta sola\n");
    s.text(b"       apps/doom.bex         o su icono del escritorio\n");
    s.with_ink(INK_ECHO);
    s.text(b"  5  DEJAR CONSTANCIA\n");
    s.with_ink(INK_PLAIN);
    s.text(b"       write datos/nota.txt hola     crea un archivo\n");
    s.text(b"       save                          vuelca esta salida a un .txt\n");
    s.with_ink(INK_ECHO);
    s.text(b"  6  LAS DOS VENTANAS\n");
    s.with_ink(INK_PLAIN);
    s.text(b"       F11  CABINA     lo que el kernel vio, con su gravedad\n");
    s.text(b"       F12  ESTRATOS   el almacen propio; la tecla S sella\n");
    s.with_ink(INK_GOOD);
    s.text(b"  GUARDAR EN ESTRATOS, el almacen propio\n");
    s.with_ink(INK_PLAIN);
    s.text(b"    estratos escribe nota.txt hola    lo guarda DE VERDAD\n");
    s.text(b"    F12 lo ensena, y tras reiniciar tiene que seguir ahi.\n");
    s.with_ink(INK_GOOD);
    s.text(b"  LO QUE TODAVIA NO SE PUEDE, para no buscarlo\n");
    s.with_ink(INK_PLAIN);
    s.text(b"    en ESTRATOS un fichero entra en 96 bytes y una carpeta en 36:\n");
    s.text(b"    lo mas grande pide un arbol de bloques, y esa es otra tanda.\n");
    s.text(b"    Y los de ls, cat y write son de la FAT32 (datos/, cobol/, c/),\n");
    s.text(b"    que es la particion que Windows tambien sabe leer.\n");
    s.with_ink(INK_ECHO);
    s.text(b"    `ayuda` tiene la lista entera de ordenes.\n");
    s.with_ink(INK_PLAIN);
    paint_status(p, &dsk.run_box, "por donde empezar", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}
