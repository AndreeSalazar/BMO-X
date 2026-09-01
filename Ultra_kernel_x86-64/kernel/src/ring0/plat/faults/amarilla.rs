//! **CARRIL AMARILLO** -- cambia cada vez que una pantalla azul ensena algo.
//!
//! [carril]  AMARILLO  el nombre del fichero ya lo decia; la etiqueta lo hace comprobable
//!
//! [cuesta]  MAQUINA -- por herencia: corre con la maquina ya rota y no
//!           puede tomar un cerrojo ni asignar memoria. Colgarse aqui cambia
//!           un volcado legible por una maquina muda.
//!
//! [riesgo]  SILENCIO ESPEJO
//!           SILENCIO -- equivocarse aqui NO falla: **imprime**. Y una linea
//!                       equivocada manda la investigacion al sitio que no es.
//!           ESPEJO   -- cada campo tiene que cuadrar con lo que dicen
//!                       `scheduler`, `cabina` y la autopsia de Ring 3.
//!
//! [prueba]  bmo-fisica-juicio
//!
//! # *** POR QUE ESTO ES AMARILLO Y NO VERDE, SIENDO "SOLO IMPRIMIR"
//!
//! Porque **cambia constantemente y sus fallos cuestan dias**. En una sola
//! semana:
//!
//! ```text
//!    25-08  la pantalla presentaba CINCO CEROS como hechos
//!    30-08  decia `pila de HILO DEL KERNEL` y su comentario juraba que decia
//!           CUAL -- dos azules gastadas sin saberlo, con el `rsp` delante
//!    30-08  la autopsia de Ring 3 cortaba la ultima linea EN SILENCIO
//! ```
//!
//! > Un informe que se equivoca no falla: **convence**.
//!
//! Ese es el carril amarillo: no es peligroso de ejecutar, es peligroso de
//! creer, y se toca a menudo.

use super::verde::{
    Informe, Line, FALLO_BARRA, FALLO_DATO, FALLO_FONDO, FALLO_SEGUNDOS, FALLO_TEXTO,
    FALLO_TITULO,
};
use crate::ring0::dev::console::serial_write;


/// Terminal fault reporter. Draws to the top of the dashboard log (rows that
/// stay visible) so a Ring 3 crash is unmistakable instead of a silent hang.
/// Informe terminal de un fallo de Ring 0: pantalla completa, y reinicia.
///
/// Antes pintaba quince renglones apretados en las filas del panel y se
/// quedaba en `hlt` para siempre. Dos problemas: con la pantalla cedida a
/// Ring 3 el informe quedaba flotando sobre el escritorio de otro, y una
/// maquina congelada obliga a alguien a levantarse a pulsar el boton -- o se
/// queda muerta hasta que alguien la encuentre.
pub(super) extern "C" fn fault_report(vector: u64, error: u64, rip: u64, cr2: u64, fault_rsp: u64) -> ! {
    // Antes de pintar, CR3 del kernel: un fallo tomado bajo un CR3 de usuario
    // no mapea el framebuffer y el primer pixel daria #PF dentro de este mismo
    // manejador -- recursion infinita y pantalla congelada en vez de informe.
    let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
    if kpml4 != 0 {
        crate::ring0::mm::vmm::switch_to(kpml4);
    }
    // -- En INGLES, y solo ASCII --
    //
    // No es una preferencia de estilo. Esta pantalla se lee EN UNA FOTO, y su
    // trabajo entero es que un caracter no se confunda con otro. El espanol
    // mete tildes y enye, y esos glifos viven en la tabla de extras Latin-1 del
    // font: son los que peor se distinguen a 8 px y los primeros que se rompen
    // si algo va mal con la fuente. El ingles cabe en ASCII puro.
    //
    // Ademas los campos que van debajo (vec, err, rip, cr2, rsp) ya son ingles
    // por ser los nombres del hardware, asi que la pantalla deja de estar a
    // medias en dos idiomas.
    //
    // El resto del sistema sigue en espanol: comentarios, CABINA, el shell. Lo
    // que cambia es SOLO lo que se fotografia.
    let name = match vector {
        6 => "#UD invalid opcode",
        8 => "#DF double fault",
        13 => "#GP general protection",
        14 => "#PF page fault",
        _ => "unknown exception",
    };
    // Ultima entrada de la bitacora antes de detener la maquina.
    crate::ring0::cabina::panic_ev("ring0", name, rip);

    let mut inf = Informe::nuevo();

    let mut l = Line::new();
    l.s("vec=0x"); l.hex(vector, 2);
    l.s("  err=0x"); l.hex(error, 8);
    inf.push(l);

    // *** `err` EN PALABRAS, Y NO SOLO EN HEXADECIMAL (2026-08-26).
    //
    // El volcado del Ryzen decia `err=0x00000002` y ese numero contesta TRES
    // preguntas de golpe -- pero solo si quien lo mira se sabe la tabla de
    // memoria. Un dato que hay que descodificar a mano en una foto es un dato
    // que se lee mal a las dos de la manana.
    //
    // Y para un `#PF` las tres deciden a donde se va a mirar:
    //
    //    bit 0  presente     0 = la pagina NO ESTA. 1 = esta y no se permitio
    //    bit 1  escritura    1 = ESCRIBIENDO. 0 = leyendo
    //    bit 2  usuario      1 = fue Ring 3. 0 = fue el KERNEL
    //    bit 4  instruccion  1 = era buscar codigo, no un dato
    if vector == 14 {
        let mut l = Line::new();
        l.s("  ");
        l.s(if error & 1 == 0 { "no-presente" } else { "proteccion" });
        l.s(if error & 2 != 0 { "  ESCRIBIENDO" } else { "  leyendo" });
        l.s(if error & 4 != 0 { "  desde Ring 3" } else { "  desde el KERNEL" });
        if error & 16 != 0 {
            l.s("  (buscando codigo)");
        }
        inf.push(l);
    }

    let mut l = Line::new();
    l.s("rip=0x"); l.hex(rip, 16);
    // ** UN `rip` DE CERO NO ES UNA DIRECCION: ES UN "NO LO SE".
    //
    // El 26-08 el Ryzen enseno `rip=0x0` junto a un `err` que decia
    // **escribiendo, y no buscando codigo**. Las dos cosas no pueden ser
    // ciertas a la vez: para escribir hace falta una instruccion, y una
    // instruccion en la direccion cero se habria traido primero -- lo que
    // habria puesto el bit 4.
    //
    // *** Asi que ese cero no era donde fallo: era lo que el manejador
    // encontro al mirar. **Y un cero sin etiqueta se lee como un dato**, que
    // es lo que mando a buscar un salto a la direccion cero que nunca hubo.
    if rip == 0 {
        l.s("   <- CERO NO ES UNA DIRECCION: no se pudo leer");
    }
    inf.push(l);

    let mut l = Line::new();
    l.s("cr2=0x"); l.hex(cr2, 16);
    inf.push(l);

    // El RSP de la instruccion que fallo. Para el iretq que entra en CPL3
    // deberia ser la pila alta de la tarea; si es otra cosa, este numero dice
    // donde estaba el CPU de verdad.
    let mut l = Line::new();
    l.s("rsp=0x"); l.hex(fault_rsp, 16);
    // ** Y DE QUIEN ES ESA PILA, que es la pregunta siguiente y no se contestaba.
    //
    // El volcado del 26-08 traia `rsp=0xFFFF800000B84C50`. Eso es el physmap, o
    // sea la pila de un HILO DEL KERNEL (`spawn_kernel` las direcciona asi) --
    // no la de una tarea de Ring 3. Ese dato estaba delante y **habia que
    // saberselo**: el rango del physmap no viene escrito en la pantalla.
    //
    // *** Y sin saber CUAL hilo, "un hilo del kernel" no acota nada. Ahora lo
    // dice: hay dos, y el que late cada 4 ms es el del bus.
    l.s("   ");
    l.s(if fault_rsp >= 0xFFFF_8000_0000_0000 { "pila de HILO DEL KERNEL" } else { "pila baja" });
    // *** Y DE CUAL, QUE ES LO QUE ESTE COMENTARIO YA PROMETIA Y NO HACIA.
    //
    // ** Arriba pone *"Ahora lo dice: hay dos, y el que late cada 4 ms es el
    // del bus"*. No lo decia. Dos pantallas azules --26-08 y 30-08-- salieron
    // con `pila de HILO DEL KERNEL` a secas, teniendo el `rsp` delante y la
    // tabla de tareas con `stack_phys` en cada fila desde siempre.
    //
    // *** Y no es lo mismo que la linea de abajo. `corria tid=` es **lo que el
    // planificador cree**; esto es **sobre que pila estaba el CPU**. Hoy las dos
    // salieron distintas --`tid=05 (Ring 3)` sobre una pila de kernel-- y esa
    // diferencia no es un error de ninguna de las dos: es el hallazgo.
    if let Some((duenno, user)) = crate::ring0::task::scheduler::duenno_de_pila(fault_rsp) {
        l.s(" de tid=");
        l.hex(duenno as u64, 2);
        l.s(if user { " (Ring 3)" } else { " (Ring 0)" });
    } else if fault_rsp >= 0xFFFF_8000_0000_0000 {
        // [!] Que no sea de nadie tambien es una respuesta, y de las caras: una
        // pila del physmap que no pertenece a ninguna tarea viva significa que
        // el `rsp` esta pisado, o que su tarea ya se reciclo debajo.
        // ** Y SI SE SABE DE QUIEN FUE, SE DICE. (2026-08-31)
        //
        // `de NADIE VIVO` era cierto y era un callejon: decia que la pila no
        // tiene duena y no decia quien la solto. La morgue del planificador
        // guarda las ocho ultimas liberadas con su tid y su tick, asi que la
        // pregunta *"quien pisa aqui?"* se contesta en la propia pantalla, sin
        // tener que ir a CABINA con la maquina parada.
        l.s(" -- de NADIE VIVO");
        if let Some((tid, tick)) = crate::ring0::task::scheduler::fue_de_quien(fault_rsp) {
            l.s(", fue de tid=");
            l.hex(tid as u64, 2);
            l.s(" liberada en tick ");
            l.hex(tick, 8);
        } else {
            // [!] Y si NO se reconoce hay que decir cuantas van: con mas pilas
            // liberadas que fichas, "no lo reconoce" significa "puede que se
            // haya ido por el anillo", que no es lo mismo que "no fue `reap`".
            // Un veredicto con dos lecturas no cierra nada.
            l.s(" (morgue: ");
            l.hex(crate::ring0::task::scheduler::pilas_liberadas(), 2);
            l.s(" liberadas)");
        }
    }
    inf.push(l);

    let (tid, es_user) = crate::ring0::task::scheduler::quien_corre();
    let mut l = Line::new();
    l.s("corria tid="); l.hex(tid as u64, 2);
    l.s(if es_user { "  (Ring 3)" } else { "  (Ring 0)" });
    inf.push(l);

    // Ultimo cambio a tarea de usuario: el contexto que entrego el
    // planificador, su back-pointer EN ESE INSTANTE, y la misma ranura releida
    // AHORA. b valido + n cero => lo pisaron entre el cambio y el epilogo.
    let snap = crate::ring0::task::scheduler::switch_snap();
    let live = if snap[0] != 0 {
        unsafe { ((snap[0] + crate::ring0::plat::trap::XSAVE_AREA as u64) as *const u64).read_volatile() }
    } else {
        0
    };
    let mut l = Line::new();
    l.s("sw"); l.hex(snap[3], 2);
    l.s(" c="); l.hex(snap[0], 12);
    l.s(" b="); l.hex(snap[1], 12);
    l.s(" n="); l.hex(live, 12);
    inf.push(l);

    // La ultima escritura del RPC en un frame ajeno. Si el contexto que
    // revento es ese, la ruta culpable es esa; si no, queda descartada.
    let ue = crate::ring0::obj::endpoint::last_write();
    let mut l = Line::new();
    l.s("rpc t="); l.hex(ue[0], 2);
    l.s(" ctx="); l.hex(ue[1], 12);
    l.s(" gpr="); l.hex(ue[2], 12);
    inf.push(l);

    // GS partido en dos: los MSR contra la direccion del PerCpu que deberian
    // tener. Si difieren, algun camino movio GS despues de init_bsp.
    let (gsb, kgs, pcaddr) = crate::ring0::task::percpu::gs_diag();
    let mut l = Line::new();
    l.s("gs b="); l.hex(gsb, 12);
    l.s(" k="); l.hex(kgs, 12);
    l.s(" pc="); l.hex(pcaddr, 12);
    inf.push(l);

    let mut l = Line::new();
    l.s("ticks="); l.hex(crate::ring0::plat::timer::ticks(), 8);
    inf.push(l);

    // Si ese RSP cae en un rango plausible, los 5 operandos del iretq que el
    // CPU intento cargar. Basura aqui => el planificador entrego un contexto
    // podrido; coherentes => el problema es el destino.
    // *** Y AQUI ESTABA LA MITAD DEL PROBLEMA DE LEER EL VOLCADO DEL 26-08.
    //
    // Estas cinco palabras solo SON un marco de `iretq` si el fallo ocurrio en
    // el epilogo de un cambio de contexto. Si el kernel revienta en cualquier
    // otro sitio --dentro de un hilo, en mitad de una funcion-- lo que hay en
    // `rsp` son variables locales, y leerlas como un marco es leer ruido.
    //
    // El Ryzen enseno esto:
    //
    //    iq rip=000000000000 cs=0000 ss=0000
    //
    // ** `cs=0000` es IMPOSIBLE en un marco de verdad: el selector de codigo
    // nunca es nulo. O sea que esas dos lineas no decian "el contexto estaba
    // corrupto" --que es como se leen-- decian **"esto no era un marco"**.
    //
    // *** Cinco ceros presentados como hechos mandan a buscar una corrupcion
    // que no existe. Es el mismo fallo que el `=1100` de la bitacora con otra
    // cara: **el instrumento contestando algo que no sabe.**
    let mapped = fault_rsp >= 0xFFFF_8000_0000_0000
        || (fault_rsp >= 0x1000 && fault_rsp < 0x1_0000_0000);
    if mapped {
        let p = fault_rsp as *const u64;
        let (irip, ics, irfl, irsp, iss) = unsafe {
            (
                p.read_volatile(),
                p.add(1).read_volatile(),
                p.add(2).read_volatile(),
                p.add(3).read_volatile(),
                p.add(4).read_volatile(),
            )
        };
        // El unico juez barato que distingue un marco de cinco palabras
        // cualesquiera: **el selector de codigo**. La GDT de esta casa es
        // `[0]=nulo [1]=codigo0 [2]=datos0 [3]=datos3 [4]=codigo3`, asi que un
        // `cs` de un marco valido solo puede ser `0x08` o `0x23`. Cualquier
        // otra cosa --y sobre todo el cero-- significa que ahi no hay marco.
        let parece_marco = ics == 0x08 || ics == 0x23;
        if parece_marco {
            let mut l = Line::new();
            l.s("iq rip="); l.hex(irip, 12);
            l.s(" cs="); l.hex(ics, 4);
            l.s(" ss="); l.hex(iss, 4);
            inf.push(l);
            let mut l = Line::new();
            l.s("iq rsp="); l.hex(irsp, 12);
            l.s(" rfl="); l.hex(irfl, 6);
            inf.push(l);
        } else {
            // ** Y se DICE que no se mira, en vez de callar. Una linea que
            // falta se lee como "no habia nada que decir"; esta dice "hay algo
            // ahi y NO es lo que estas lineas saben leer", que manda a otro
            // sitio -- al hilo, no al planificador.
            let mut l = Line::new();
            l.s("iq: en rsp no hay marco de iretq (cs=0x"); l.hex(ics, 4);
            l.s("). El fallo no es de un cambio de contexto");
            inf.push(l);
        }
    }

    pantalla_de_fallo(name, &inf)
}


/// Pinta el informe a pantalla completa, cuenta atras, y reinicia.
///
/// * Usa `hay_fb_crudo`, no `has_fb`: si un proceso Ring 3 tenia cedida la
/// pantalla, un fallo de kernel **se la quita**. La maquina se esta muriendo y
/// esto es lo unico que va a quedar.
///
/// * Y reinicia en vez de quedarse en `hlt` para siempre. Un kernel congelado
/// obliga a alguien a levantarse y pulsar el boton; peor aun, si pasa mientras
/// nadie mira, la maquina se queda muerta hasta que alguien la encuentre.
pub(super) fn pantalla_de_fallo(titulo: &str, informe: &Informe) -> ! {
    use crate::ring0::core::splash as sp;

    if !crate::info::hay_fb_crudo() {
        // Sin pantalla no hay nada que pintar, pero el reinicio sigue siendo
        // mejor que el congelado.
        crate::ring0::plat::reinicio::ahora();
    }

    let w = unsafe { crate::info::FB_WIDTH };
    let h = unsafe { crate::info::FB_HEIGHT };
    sp::fallo_fondo(FALLO_FONDO);

    let x = (w / 12).max(48);
    let mut y = (h / 6).max(60);

    sp::fallo_texto_grande(x, y, "BMO-X has stopped", FALLO_TITULO, 2);
    y += sp::ALTO_LINEA * 3;

    sp::fallo_texto(
        x,
        y,
        "A Ring 0 fault cannot be isolated: the kernel is the floor",
        FALLO_TEXTO,
    );
    y += sp::ALTO_LINEA;
    sp::fallo_texto(x, y, "everything else stands on. This is what is known:", FALLO_TEXTO);
    y += sp::ALTO_LINEA * 2;

    sp::fallo_texto(x, y, titulo, FALLO_TITULO);
    y += sp::ALTO_LINEA * 2;

    for i in 0..informe.n {
        sp::fallo_texto(x, y, informe.lineas[i].as_str(), FALLO_DATO);
        y += sp::ALTO_LINEA;
    }

    // -- Cuenta atras --
    let barra_y = h - h / 8;
    let barra_w = w - x * 2;
    let alto = 10u32;
    sp::fallo_texto(
        x,
        barra_y - sp::ALTO_LINEA - 8,
        "Rebooting. If you want the photo, this is your window.",
        FALLO_TEXTO,
    );

    let hz = crate::ring0::task::scheduler::tsc_freq();
    if hz == 0 {
        // Sin TSC calibrado no hay cuenta atras honesta. Se pinta la barra
        // llena y se reinicia: mentir con una barra que no mide nada seria
        // peor que no tenerla.
        sp::fallo_rect(x, barra_y, barra_w, alto, FALLO_BARRA);
        for _ in 0..80_000_000u64 {
            core::hint::spin_loop();
        }
        crate::ring0::plat::reinicio::ahora();
    }

    let inicio = crate::ring0::task::scheduler::rdtsc();
    let total = hz * FALLO_SEGUNDOS;
    // La barra llena, UNA vez. A partir de aqui solo se borra lo que mengua.
    sp::fallo_rect(x, barra_y, barra_w, alto, FALLO_BARRA);
    let mut anterior = barra_w;
    loop {
        let pasado = crate::ring0::task::scheduler::rdtsc().wrapping_sub(inicio);
        if pasado >= total {
            break;
        }
        // La barra MENGUA: se ve cuanto queda, no cuanto ha pasado.
        let restante = ((total - pasado) as u128 * barra_w as u128 / total as u128) as u32;
        // * Repintar por DANO, no la barra entera.
        //
        // Antes este bucle borraba y redibujaba los ~1200 px de la barra en
        // CADA vuelta, tan rapido como el CPU pudiera: decenas de miles de
        // pasadas por segundo sobre memoria de video sin cache y sin ninguna
        // sincronizacion con el refresco del panel. Lo que se ve entonces no es
        // que el framebuffer sea debil: es que el panel captura la barra a
        // medio reescribir, y muestra una banda de la pasada anterior mezclada
        // con la nueva. Un LCD refresca 60 veces por segundo; escribirle 40.000
        // no lo hace ir mas rapido, lo hace ensenar basura.
        //
        // Ahora solo se borra la tira que acaba de desaparecer, y solo cuando
        // el ancho cambia de pixel entero. Es el mismo principio que el cursor
        // del compositor --repintar el dano, no la escena-- y aqui se nota mas
        // porque no hay nada mas en pantalla que lo disimule.
        if restante < anterior {
            sp::fallo_rect(x + restante, barra_y, anterior - restante, alto, FALLO_FONDO);
            anterior = restante;
        }
    }
    crate::ring0::plat::reinicio::ahora();
}

