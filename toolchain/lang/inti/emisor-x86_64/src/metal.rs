//! `metal` -- UN NOMBRE DE INTI SE VUELVE LOS BYTES DE UNA INSTRUCCION.
//!
//! ## Por que soy un fichero y no un trozo del de al lado (L6b)
//!
//! Porque contesto *"que hay detras de un nombre que trae `usa x86_64`"*, y esa
//! pregunta la contestan DOS TABLAS que no contesta nadie mas:
//!
//! ```text
//!    arch/x86_64/inti.toml   como se llama en INTI   `cuenta_unos` -> `popcnt`
//!    arch/x86_64/intrinsics  que bytes son           `popcnt` -> F3 0F B8 C0
//! ```
//!
//! ** La segunda la comparte con BMO C, y ese es el punto: los bytes se declaran
//! UNA vez. La primera es de INTI porque el nombre es del lenguaje.
//!
//! Que esto sea un fichero y no unas lineas dentro del despacho es lo que hace
//! visible el reparto -- y el reparto es justamente lo que fallaba antes de F5d,
//! cuando aqui no habia nada y la rama estaba vacia.

use bmo_inti_front::ir::{FuncionIr, Instr, Temporal, Valor};
use bmo_lower::x86;

use crate::marco::Marco;
use crate::{carga, guarda_temporal, Taller, IZQ};

/// EL METAL: un nombre de INTI se vuelve los bytes de una instruccion.
///
/// ## ** Que estaba roto, dicho sin adornos
///
/// Esta funcion no existia, y en su sitio habia `Instr::Metal { .. } => {}`.
/// La tabla de x86-64 llevaba desde F2b con **setenta y tantos nombres** --los
/// puertos de E/S, los registros de control, las atomicas, las cuentas de
/// bits-- y ni uno solo llegaba a un byte. Un `lee_reloj()` compilaba, pasaba
/// el analisis de nombres, pasaba el de perfiles, pasaba el gate, y devolvia lo
/// que hubiera en el registro de trabajo.
///
/// Es el fallo que este proyecto persigue desde el principio: **la pieza que se
/// calcula bien y no la lee nadie**. Aqui la leia el frontend entero y se caia
/// en la ultima linea.
///
/// ## Las dos tablas, y por que son dos
///
/// ```text
///    arch/x86_64/inti.toml   como se llama en INTI   `cuenta_unos` -> `popcnt`
///    arch/x86_64/intrinsics  que bytes son           `popcnt` -> F3 0F B8 C0
/// ```
///
/// La segunda la comparte con BMO C, y ese es el punto: **los bytes se declaran
/// una vez**. La primera es de INTI porque el nombre es del lenguaje.
///
/// ## Lo que hace cuando NO puede
///
/// Lo apunta y sigue. No emite nada plausible, y no calla: el nombre entra en
/// `sin_emitir`, que viaja hasta CABINA y hasta un test que lo exige vacio para
/// la tabla entera. Un intrinseco que no se puede emitir es una fila mal
/// escrita, y una fila mal escrita en una tabla de driver se descubre en metal
/// y seis meses tarde si nadie la cuenta.
pub(crate) fn metal(
    out: &mut Vec<u8>,
    nombre: &str,
    argumentos: &[Valor],
    destino: Option<bmo_inti_front::ir::Temporal>,
    marco: &Marco,
    taller: &Taller,
    sin_emitir: &mut Vec<String>,
) {
    let Some(maquina) = taller.maquina.as_ref() else {
        sin_emitir.push(format!("{}: no hay tabla de maquina", nombre));
        return;
    };
    let Some(instruccion) = maquina.instruccion(nombre) else {
        sin_emitir.push(format!("{}: la maquina no dice que instruccion es", nombre));
        return;
    };
    let Some(intrinsecos) = taller.intrinsecos.as_ref() else {
        sin_emitir.push(format!("{}: no hay tabla de bytes", nombre));
        return;
    };
    let Some(def) = intrinsecos.get(instruccion) else {
        sin_emitir.push(format!(
            "{}: `{}` no esta en intrinsics.toml",
            nombre, instruccion
        ));
        return;
    };
    if argumentos.len() != def.args.len() {
        // ** Y esto NO es un aviso cosmetico. Emitir la instruccion con un
        // registro sin cargar la ejecuta con lo que hubiera dentro: un
        // `escribe_puerto` con el puerto sin poner habla con un aparato que no
        // es. Se prefiere no emitir y decirlo.
        sin_emitir.push(format!(
            "{}: pide {} argumento(s) y le dieron {}",
            nombre,
            def.args.len(),
            argumentos.len()
        ));
        return;
    }

    // 1. Cada argumento a SU registro.
    //
    // ** Se puede cargar en orden y sin apilar --que es lo que hace BMO C--
    // porque el asignador ya se freno: `marco.rs` no reparte registros en una
    // funcion que tiene un `Metal`, exactamente igual que con una llamada. Sin
    // ese freno, cargar en `rdx` podria pisar un temporal que vive alli.
    for (i, a) in argumentos.iter().enumerate() {
        // ** El caso raro primero, porque es el que existe de verdad: hay
        // instrucciones que reciben un valor de 64 bits PARTIDO en dos
        // registros de 32. Nacieron antes de que hubiera registros de 64 y el
        // silicio nunca las cambio.
        //
        // Cargarlo en uno solo escribe la mitad baja y deja la alta con lo que
        // hubiera. En un registro de control del CPU, esa mitad alta son bits
        // que encienden cosas.
        if def.args[i] == "u64_edx_eax" {
            carga(out, IZQ, a, marco);
            x86::mov_r64_r64(out, 2, IZQ);
            x86::shr_r64_imm8(out, 2, 32);
            continue;
        }
        match registro_llamado(&def.args[i]) {
            Some(r) => carga(out, r, a, marco),
            None => {
                sin_emitir.push(format!(
                    "{}: no se en que registro va `{}`",
                    nombre, def.args[i]
                ));
                return;
            }
        }
    }

    // 2. Los bytes EXACTOS de la tabla. Ni uno escrito aqui.
    out.extend_from_slice(&def.bytes);

    // 3. Y el valor, donde este emisor espera todo resultado.
    recoge_de(out, def.returns.as_deref());

    if let Some(d) = destino {
        guarda_temporal(out, IZQ, d, marco);
    }
}

/// **QUE REGISTROS PISA una funcion por sus instrucciones de maquina.**
///
/// ## ** Por que esto existe, y lo que costaba no tenerlo
///
/// El asignador de registros se frenaba entero --cero temporales en registro--
/// en cuanto una funcion tenia UNA instruccion de maquina. El comentario del
/// freno decia *"una llamada puede pisar estos tres registros"*, y eso es verdad
/// de una llamada: puede pisar lo que quiera.
///
/// ** Pero una instruccion de maquina NO es una llamada. `rdtsc` pisa `rdx` y el
/// de trabajo, y nada mas -- **y eso ya esta escrito en `intrinsics.toml`**, en
/// las mismas filas que el emisor usa para emitirla. El freno pagaba el precio
/// de la peor sin mirar la tabla que tenia al lado.
///
/// Lo destapo el Ryzen: el bucle de la sonda costo ~47 ticks por vuelta cuando
/// deberia costar tres o cuatro, y la causa medida fue que el contador vivia en
/// la pila. **Un numero que solo el metal contesta, apuntando a una decision del
/// compilador.**
///
/// ## Lo que se cuenta como pisado, y por que de mas
///
/// ```text
///    los `args`     ahi se cargan los operandos, asi que se pierden
///    el `returns`   ahi sale el resultado
///    `rdx` SIEMPRE  porque `recoge_de` lo usa para juntar los de 64 bits
///                   partidos en dos, y eso no esta en la fila del intrinseco
/// ```
///
/// ** `rdx` de mas es a proposito: es la unica sobre-aproximacion, y quitarla
/// pediria que la tabla dijera algo que hoy no dice. Un registro de menos es
/// lentitud; uno de mas es un valor pisado -- y de las dos, sobra la primera.
pub(crate) fn registros_que_pisa(f: &FuncionIr, taller: &Taller) -> Vec<u8> {
    let mut pisa = vec![2u8]; // rdx, por `recoge_de`
    let (Some(maquina), Some(intrinsecos)) = (taller.maquina.as_ref(), taller.intrinsecos.as_ref())
    else {
        // Sin tablas no se sabe, y no saber se paga con el freno entero: es
        // la respuesta segura, no la comoda.
        return (0..16).collect();
    };
    for i in &f.instrucciones {
        let Instr::Metal { nombre, .. } = i else {
            continue;
        };
        let Some(def) = maquina.instruccion(nombre).and_then(|x| intrinsecos.get(x)) else {
            // Un nombre que no se sabe emitir tampoco se sabe acotar.
            return (0..16).collect();
        };
        for a in &def.args {
            if let Some(r) = registro_llamado(a) {
                pisa.push(r);
            }
        }
        if let Some(r) = def.returns.as_deref().and_then(registro_llamado) {
            pisa.push(r);
        }
    }
    pisa.sort_unstable();
    pisa.dedup();
    pisa
}

/// El numero de registro que hay detras de un nombre de `intrinsics.toml`.
///
/// ** Los nombres de la tabla son los del ENSAMBLADOR --`al`, `dx`, `eax`-- y
/// no los de INTI, porque la tabla la comparten cinco lenguajes. `al`, `ax`,
/// `eax` y `rax` son el mismo registro visto con cuatro anchos, y para saber
/// cual cargar da igual el ancho: la instruccion ya lo fija.
pub(crate) fn registro_llamado(nombre: &str) -> Option<u8> {
    Some(match nombre {
        "rax" | "eax" | "ax" | "al" => 0,
        "rcx" | "ecx" | "cx" | "cl" => 1,
        "rdx" | "edx" | "dx" | "dl" => 2,
        "rbx" | "ebx" | "bx" | "bl" => 3,
        "rsp" => 4,
        "rbp" => 5,
        "rsi" | "esi" | "si" => 6,
        "rdi" | "edi" | "di" => 7,
        "r8" => 8,
        "r9" => 9,
        "r10" => 10,
        "r11" => 11,
        _ => return None,
    })
}

/// Deja el resultado de la instruccion donde el emisor lo espera.
///
/// ** El caso que importa es el primero, y es de silicio: hay instrucciones
/// viejas que parten un valor de 64 bits en DOS registros de 32, porque nacieron
/// antes de que existieran los de 64. Recogerlo de uno solo devuelve la mitad
/// baja -- y la mitad baja de un contador de ciclos parece un numero perfecto.
pub(crate) fn recoge_de(out: &mut Vec<u8>, devuelve: Option<&str>) {
    match devuelve {
        Some("u64_edx_eax") => {
            x86::shl_r64_imm8(out, 2, 32);
            x86::or_r64_r64(out, IZQ, 2);
        }
        // Un byte o dos: el resto del registro puede traer basura de antes, asi
        // que se extiende con ceros en vez de dejarla.
        Some("al") => out.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]),
        Some("ax") => out.extend_from_slice(&[0x48, 0x0F, 0xB7, 0xC0]),
        // La puerta contesta el valor por otro registro. Aqui no se cruza la
        // puerta, pero hay instrucciones que dejan el resultado ahi.
        Some("rdx") | Some("edx") => x86::mov_r64_r64(out, IZQ, 2),
        // "eax"/"rax": ya esta donde tiene que estar. Escribir la mitad baja de
        // un registro en esta maquina pone la alta a cero, asi que tampoco hay
        // que limpiar.
        Some(_) => {}
        // ** Y la que NO devuelve nada --`hlt`, `cli`, una barrera-- deja un
        // CERO, no lo que hubiera.
        //
        // No es cosmetica: `x = para()` no tiene sentido y aun asi se puede
        // escribir. Con basura, el programa sigue con un numero que parece
        // valido; con cero, hace lo mismo siempre y se puede reproducir. Entre
        // dos cosas mal, la que no cambia entre ejecuciones.
        None => x86::zero_r32(out, IZQ),
    }
}
