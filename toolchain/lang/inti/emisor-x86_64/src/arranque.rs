//! El `crt0` de INTI: quien llama a `principal`, y quien recoge lo que deja.
//!
//! ## El punto 1 de los cinco, y el unico que no se puede saltar nadie
//!
//! La seccion 13c del maestro separa las cinco cosas que se llaman "runtime".
//! Esta es la primera: **alguien tiene que llamar a `principal` y hacer algo con
//! lo que devuelva**. C la tiene, Rust la tiene, y `llano` --que presume de no
//! necesitar runtime-- tambien. No hay lenguaje sin ella, porque una funcion que
//! nadie llama no se ejecuta.
//!
//! Son unas decenas de bytes, y por eso *"C no tiene runtime"* es una frase que
//! se puede decir sin mentir del todo.
//!
//! ## Lo que este arranque NO hace, y en que se nota
//!
//! ```text
//!    poner la pila a cero      no: la pone el kernel al cargar el .bex
//!    limpiar la BSS            no: el cargador entrega paginas a cero
//!    montar un monton          no: eso es `pleno`, y llega despues
//!    arrancar un planificador  NUNCA: una tarea de INTI es una tarea del
//!                              sistema, no una tarea verde de nadie
//! ```
//!
//! ** Las cuatro son la misma respuesta dicha cuatro veces: **el runtime de
//! INTI es delgado porque el sistema operativo es tuyo**. Go se trae su
//! planificador puesto porque el sistema de debajo no le da lo que quiere; aqui
//! el de debajo hace lo que se le pide.
//!
//! ## Y como termina
//!
//! Por la puerta. No hay `exit()` de biblioteca, no hay `atexit`, no hay una
//! capa que traduzca: el ultimo acto de un programa de INTI es **un `invoca`
//! sobre su propia tarea**, identico al que habria escrito el programa a mano.
//!
//! Eso es lo que hace que INTI sea el lenguaje de este sistema y no un lenguaje
//! portado a el: no termina *"como termina un proceso"*, termina como termina un
//! proceso **de BMO-X**.

use crate::puerta::Puerta;
use bmo_abi::syscalls::surface::{CURRENT_TASK, NR_INVOKE, TASK_OP_EXIT};
use bmo_lower::x86;

/// El registro que el arranque usa para llevar la direccion del slot.
///
/// [!] `rcx`, y NO uno de los de argumento: el arranque ya tiene ocupados los
/// suyos con lo que va a pasarle a la puerta, y machacar uno aqui pondria a
/// morir la tarea con un codigo inventado.
const DONDE: u8 = 1;

/// El nombre por el que empieza todo. No es una palabra del lenguaje: es una
/// fila mas, y por eso esta aqui y no en la gramatica.
pub const PRINCIPAL: &str = "principal";

/// Quien monta el monton de la tarea. Tambien es un nombre, no una palabra.
pub const MONTON_NUEVO: &str = "monton_nuevo";

/// **Cuanto mide el monton de una tarea recien nacida.** (2026-08-23)
///
/// Decision de Eddi: **4096**, una pagina. Y es un numero y no una tabla
/// todavia a proposito -- el dia que haya un segundo caso (una tarea que pida
/// mas al arrancar) se muda a `[reparto]`, con el resto de lo que el arranque
/// decide. Mover un dato a una tabla antes de que exista el segundo caso es como
/// se acaba con tres tablas que dicen lo mismo.
pub const MONTON_DE_LA_TAREA: u32 = 4096;

/// **Con que codigo muere una tarea que no consiguio su monton.**
///
/// Decision de Eddi: *"si falla que muera"*. Y muere AQUI, antes de `principal`,
/// que es la unica forma de que se note:
///
/// ** Si se dejara seguir, el primer `texto + texto` reservaria sobre un monton
/// cero, `pide` devolveria 0, `junta` devolveria 0, y el programa seguiria con
/// un texto que no existe. El fallo aparecerria **paginas mas adelante** y sin
/// relacion visible con su causa.
///
/// Es la misma regla que `monton_nuevo` ya tenia escrita: *"quien pide 4 KiB y
/// recibe 0 tiene que enterarse ahi, y no dos funciones mas adelante"*.
pub const SIN_MONTON: u64 = 1004;

/// Emite el arranque al principio de `out` y devuelve el hueco del `call` a
/// `principal`, para que lo rellene quien reparte los sitios del modulo.
///
/// `retorno` es el registro por el que una funcion de esta maquina devuelve.
///
/// OJO: hoy llega como parametro porque el emisor lo tiene en una constante.
/// Su sitio de verdad es `[reparto]` de la tabla, al lado de `trabajo` y
/// `temporales`, y el dia que se mude este parametro desaparece. Se dice en vez
/// de dejarlo escrito a mano y callarse.
/// Lo que el arranque deja pendiente de que otro rellene.
pub struct Pendiente {
    /// El `call` a `principal`.
    pub principal: usize,
    /// El `call` a `monton_nuevo`, si se monto monton.
    pub monton_nuevo: Option<usize>,
    /// El inmediato que apunta al slot del monton en la seccion `Data`.
    pub slot_del_monton: Option<usize>,
}

pub fn emitir(out: &mut Vec<u8>, p: &Puerta, retorno: u8, con_monton: bool) -> Pendiente {
    let mut monton_nuevo = None;
    let mut slot_del_monton = None;

    // === 0. EL MONTON DE LA TAREA, y solo si alguien lo pidio ===
    //
    // ** "Solo si" importa: montar un monton cuesta DOS cruces de la puerta, y
    // un programa de `llano` que no toca objetos no tiene por que pagarlos. Se
    // sabe mirando la IR --si nadie emitio `MontonDeLaTarea`, no hace falta--
    // y no por el perfil, que seria adivinar.
    if con_monton {
        x86::mov_r32_imm32(out, p.argumentos[0], MONTON_DE_LA_TAREA);
        out.push(0xE8); // call monton_nuevo
        monton_nuevo = Some(out.len());
        out.extend_from_slice(&[0, 0, 0, 0]);

        // Si devolvio 0, la tarea MUERE aqui. Ver `SIN_MONTON`.
        x86::test_r64_r64(out, retorno, retorno);
        let hay = x86::salto_corto(out, 0x75); // jnz
        if p.caben() >= 3 {
            x86::mov_r64_imm64(out, p.argumentos[2], SIN_MONTON);
        }
        x86::mov_r64_imm64(out, p.argumentos[0], CURRENT_TASK);
        x86::mov_r32_imm32(out, p.argumentos[1], TASK_OP_EXIT as u32);
        x86::mov_r32_imm32(out, p.numero, NR_INVOKE);
        x86::syscall(out);
        out.extend_from_slice(&[0xEB, 0xFE]); // por si la puerta vuelve
        x86::cierra_salto_corto(out, hay);

        // Y al slot. La direccion de `Data` no se sabe al emitir --la elige el
        // cargador-- asi que va un inmediato a cero y una reubicacion.
        x86::mov_r64_imm64(out, DONDE, 0);
        slot_del_monton = Some(out.len() - 8);
        // `mov [DONDE], retorno`
        out.push(0x48 | ((retorno >> 3) & 1) << 2 | ((DONDE >> 3) & 1));
        out.push(0x89);
        out.push(((retorno & 7) << 3) | (DONDE & 7));
    }

    // 1. Llamar a `principal`. El destino no se sabe todavia: se sabra cuando
    //    todas las funciones tengan sitio.
    out.push(0xE8); // call rel32
    let hueco = out.len();
    out.extend_from_slice(&[0, 0, 0, 0]);

    // 2. ** Lo que devolvio, AL SITIO DEL ARGUMENTO, y antes de tocar nada mas.
    //
    //    El orden importa y no es evidente: el registro de retorno y el del
    //    numero de la puerta son **el mismo** en esta maquina. Cargar el numero
    //    primero se comeria el codigo de salida, y el programa terminaria
    //    siempre con exito -- que es la clase de fallo que no se ve nunca,
    //    porque solo se nota cuando algo ya fue mal.
    if p.caben() >= 3 {
        x86::mov_r64_r64(out, p.argumentos[2], retorno);
    }

    // 3. Sobre quien, y que.
    x86::mov_r64_imm64(out, p.argumentos[0], CURRENT_TASK);
    x86::mov_r32_imm32(out, p.argumentos[1], TASK_OP_EXIT as u32);

    // 4. Por que puerta. Solo hay una, y ese es el congelamiento.
    x86::mov_r32_imm32(out, p.numero, NR_INVOKE);
    x86::syscall(out);

    // 5. Si la puerta devuelve, no se sigue.
    //
    //    Un programa que sobrevive a su propia salida es un fallo del kernel, y
    //    lo que no puede hacer es ponerse a ejecutar lo que hubiera detras.
    out.extend_from_slice(&[0xEB, 0xFE]); // jmp -2

    Pendiente {
        principal: hueco,
        monton_nuevo,
        slot_del_monton,
    }
}
