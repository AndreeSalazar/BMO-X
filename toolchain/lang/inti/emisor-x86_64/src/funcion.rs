//! `funcion` -- de una `FuncionIr` a los bytes de UNA funcion.
//!
//! ## Por que sale de `lib.rs` (L6a, 2026-08-23)
//!
//! Porque `lib.rs` hacia dos trabajos de tamano muy distinto:
//!
//! ```text
//!    lib       el MODULO: reparte sitios, resuelve llamadas, empaqueta
//!    funcion   UNA funcion: el prologo, cada instruccion, las katanas
//! ```
//!
//! El segundo son 600 lineas de un solo `match` --una rama por instruccion de
//! la IR-- y crece cada vez que el lenguaje aprende algo. El primero no.
//!
//! ** Y el censo lo llamaba `mixto`: *"a mano, hay funciones grandes entre las
//! pequenas"*. La grande era esta.
//!
//! [!] El `match` sigue CERRADO, que es lo que obliga a atender una instruccion
//! nueva en vez de dejarla caer en un comodin. Mudarlo de fichero no le quita
//! esa propiedad -- y este ano ya la ha cobrado cuatro veces.

use super::*;

pub(crate) fn emitir_funcion(f: &FuncionIr, out: &mut Vec<u8>, taller: &Taller) -> Cuenta {
    // ** Los que la maquina pisa por sus propias instrucciones se quitan del
    // reparto; el resto sigue disponible. Antes se apagaba el reparto ENTERO en
    // cuanto habia una, y eso es pagar el precio de una llamada por algo que la
    // tabla acota en una fila.
    let pisados = metal::registros_que_pisa(f, taller);
    let libres: Vec<u8> = taller
        .temporales
        .iter()
        .copied()
        .filter(|r| !pisados.contains(r))
        .collect();
    let marco = Marco::con_registros(f, &libres);
    let mut cuenta = Cuenta {
        en_registros: marco.en_registros(),
        en_pila: f.temporales as usize - marco.en_registros(),
        ..Default::default()
    };
    let mut comprobaciones = 0usize;
    let mut reubicaciones_del_monton: Vec<usize> = Vec::new();
    let mut salida_huecos: Vec<(usize, String)> = Vec::new();
    let mut sin_emitir: Vec<String> = Vec::new();
    let mut reubicaciones: Vec<(usize, u32)> = Vec::new();

    // Prologo.
    out.push(0x55); // push rbp
    x86::mov_r64_r64(out, 5, 4); // mov rbp, rsp
    let tam = marco.size();
    if tam > 0 {
        if tam <= 127 {
            x86::sub_r64_imm8(out, 4, tam as i8);
        } else {
            // Marcos grandes: se emite el inmediato de 32 bits a mano porque
            // `bmo_lower` no trae ese helper todavia.
            out.extend_from_slice(&[0x48, 0x81, 0xEC]);
            out.extend_from_slice(&(tam as u32).to_le_bytes());
        }
    }

    // ** Los parametros llegan en registros y las locales viven en el marco,
    // asi que lo primero que hace toda funcion es bajarlos.
    //
    // El orden de esos registros es la convencion de llamada de esta maquina, y
    // por eso esta linea solo puede existir en este crate: el frontend tiene
    // prohibido saber que existe algo llamado "registro de argumento".
    for i in 0..f.parametros.min(6) as usize {
        mov_a_marco(out, marco.local(Local(i as u32)), ARGUMENTOS[i]);
    }

    // Los saltos se rellenan al final, cuando se sabe donde cayo cada etiqueta.
    let mut sitios_de_etiqueta: Vec<(u32, usize)> = Vec::new();
    let mut huecos: Vec<(usize, u32)> = Vec::new();
    // A donde saltan las comprobaciones que fallan.
    // ** Cada hueco lleva SU codigo, y eso es lo que hacia falta para que
    // hubiera mas de una regla. Con un solo destino de trampa, atrapar por
    // dividir entre cero habria devuelto E1001 -- el codigo de desbordar -- y
    // el programa habria dicho que le paso otra cosa.
    let mut huecos_de_atrapa: Vec<(usize, u64)> = Vec::new();

    for i in &f.instrucciones {
        match i {
            Instr::Etiqueta(e) => sitios_de_etiqueta.push((e.0, out.len())),

            Instr::Mueve { destino, origen } => {
                carga(out, IZQ, origen, &marco);
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            Instr::Guarda { destino, valor } => {
                carga(out, IZQ, valor, &marco);
                mov_a_marco(out, marco.local(*destino), IZQ);
            }

            Instr::Binaria {
                destino,
                op,
                clase,
                sin_signo,
                izquierda,
                derecha,
            } => {
                carga(out, IZQ, izquierda, &marco);
                carga(out, DER, derecha, &marco);
                // ** La clase viene DE LA IR, no se adivina aqui. Los ocho
                // bytes de un flotante y los de un entero son indistinguibles,
                // asi que un emisor que lo decidiera mirando el valor acertaria
                // casi siempre -- que es peor que fallar siempre.
                match clase {
                    // ** Y el SIGNO viene de la IR por lo mismo que la clase:
                    // los ocho bytes de un `natural64` y los de un `entero64`
                    // son indistinguibles, asi que un emisor que lo decidiera
                    // mirando el valor acertaria casi siempre -- que es peor que
                    // fallar siempre.
                    Clase::Entero => binaria(out, *op, *sin_signo),
                    Clase::Flotante => flotante(out, *op),
                }
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            // ** LA CONVERSION, que es la unica vez que los bits CAMBIAN.
            //
            // Todo lo demas de este emisor mueve ocho bytes de un sitio a otro
            // sin tocarlos. Aqui no: `5` y `5.0` no comparten un solo bit, y
            // hay una instruccion que los traduce. De ahi que sea una
            // instruccion de la IR y no un `mov` con otro nombre.
            Instr::Convierte {
                destino,
                valor,
                desde,
                hacia,
            } => {
                carga(out, IZQ, valor, &marco);
                match (desde, hacia) {
                    (Clase::Entero, Clase::Flotante) => {
                        x86::cvtsi2sd_de_r64(out, IZQ);
                        x86::movq_r64_de_xmm(out, IZQ, 0);
                    }
                    (Clase::Flotante, Clase::Entero) => {
                        x86::movq_xmm_de_r64(out, 0, IZQ);
                        x86::cvttsd2si_r64(out, IZQ);
                    }
                    // De entero a entero y de flotante a flotante, los bits ya
                    // estan. Estrechar --de `entero64` a `entero8`-- es otra
                    // cosa y todavia no se pide: el ancho de una local lo
                    // reparte el marco, no la conversion.
                    _ => {}
                }
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            Instr::Unaria { destino, op, valor } => {
                carga(out, IZQ, valor, &marco);
                match op {
                    bmo_inti_front::arbol::OpUno::Menos => x86::neg_r64(out, IZQ),
                    bmo_inti_front::arbol::OpUno::No => {
                        // `no x` sobre un logico: comparar con cero y quedarse
                        // con el bit de igualdad.
                        x86::test_r64_r64(out, IZQ, IZQ);
                        out.extend_from_slice(&[0x0F, 0x94, 0xC0]); // sete al
                        out.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx
                    }
                }
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            // ** La regla, en bytes.
            Instr::Comprueba { que, sobre, contra, sin_signo, .. } => {
                comprobaciones += 1;
                let codigo: u64 = que.codigo()[1..].parse().unwrap_or(0);
                match que {
                    // *** LA REGLA 1 MIRA DOS BANDERAS DISTINTAS (2026-08-23).
                    //
                    // La comprobacion no vuelve a calcular nada: pregunta por lo
                    // que la operacion dejo puesto. Pero **no es la misma
                    // bandera**:
                    //
                    //     con signo    `jo`   el resultado no cabe con su signo
                    //     sin signo    `jc`   la suma se dio la vuelta por arriba
                    //
                    // ** Con `jo` para todo, `2^64-1 + 3` sobre `natural64` NO
                    // atrapaba: el resultado daba la vuelta a 2 y nadie decia
                    // nada. Era la cuarta familia del fallo del signo de esta
                    // manana, y la unica que se quedo sin arreglar -- el commit
                    // de aquel arreglo la daba por hecha y no lo estaba.
                    //
                    // Lo destapo quitar una guardia REDUNDANTE en `junta`: el
                    // programa dejo de atrapar y se puso a copiar 2^64 bytes.
                    Comprobacion::Desborde => {
                        let cc = if *sin_signo { 0x82 } else { 0x80 };
                        out.extend_from_slice(&[0x0F, cc]);
                        huecos_de_atrapa.push((out.len(), codigo));
                        out.extend_from_slice(&[0, 0, 0, 0]);
                    }

                    // ** REGLA 3, y son cuatro instrucciones.
                    //
                    // La IR ya la coloco ANTES de la division y le paso el
                    // DIVISOR, que era la pieza que faltaba: mirar el resultado
                    // no sirve de nada porque dividir entre cero no deja
                    // resultado, deja una excepcion del procesador.
                    Comprobacion::EntreCero => {
                        carga(out, IZQ, sobre, &marco);
                        x86::test_r64_r64(out, IZQ, IZQ);
                        out.extend_from_slice(&[0x0F, 0x84]); // jz
                        huecos_de_atrapa.push((out.len(), codigo));
                        out.extend_from_slice(&[0, 0, 0, 0]);
                    }

                    // *** LA REGLA 1 ESCONDIDA DENTRO DE UNA DIVISION.
                    //
                    // `-2^63 entre -1` no cabe en 64 bits. Es la unica de las
                    // cinco que mira DOS valores, y por eso `Comprueba` lleva un
                    // `contra`: el cociente solo se sale cuando el dividendo es
                    // el minimo Y el divisor es -1.
                    //
                    // El camino que no atrapa paga una comparacion y un salto
                    // que no salta: al segundo `cmp` solo se entra si el divisor
                    // es exactamente -1, que casi nunca.
                    Comprobacion::Cociente => {
                        let Some(divisor) = contra else {
                            // Sin el segundo valor no hay nada que comprobar, y
                            // callar aqui seria emitir una regla que aprueba
                            // todo. Se dice y no se emite.
                            sin_emitir.push(
                                "la regla del cociente llego sin su segundo valor".to_string(),
                            );
                            continue;
                        };
                        carga(out, DER, divisor, &marco);
                        x86::cmp_r64_imm32(out, DER, -1);
                        let al_final = x86::salto_corto(out, 0x75); // jne
                        carga(out, IZQ, sobre, &marco);
                        x86::mov_r64_imm64(out, 2, i64::MIN as u64);
                        x86::cmp_r64_r64(out, IZQ, 2);
                        out.extend_from_slice(&[0x0F, 0x84]); // je -> atrapa
                        huecos_de_atrapa.push((out.len(), codigo));
                        out.extend_from_slice(&[0, 0, 0, 0]);
                        x86::cierra_salto_corto(out, al_final);
                    }

                    // ** REGLA 12 -- cabe este numero en tantos bytes?
                    Comprobacion::Conversion(bytes) => {
                        regla_doce(out, sobre, *bytes, &marco, &mut huecos_de_atrapa, codigo);
                    }

                    // ** LA 2 SIGUE SIN EMITIRSE, y ahora esta sola con su
                    // motivo -- que es distinto del que tenian las otras dos.
                    //
                    // Las otras dos no salian por un fallo de sitio: la IR las
                    // ponia detras de la operacion, donde ya no habia nada que
                    // mirar. Esta no sale porque **no hay contra que
                    // comprobar**: un `bufer de T` es una direccion y no lleva
                    // su longitud dentro. Por eso indexarlo pide `crudo`.
                    //
                    // La comprobacion nace con `lista de T` de `pleno`, que SI
                    // lleva la suya. No es deuda de este fichero: es una que
                    // espera a un tipo que todavia no existe.
                    // *** LA REGLA 2, EN BYTES (2026-08-23).
                    //
                    // Aqui ponia `comprobaciones -= 1;` y nada mas: la regla se
                    // pedia en la IR, **no se emitia**, y encima se descontaba
                    // del recuento para que el numero no mintiera. Era la unica
                    // de las cuatro que no llegaba a un byte.
                    //
                    // ** Lo que faltaba no era el emisor: era CONTRA QUE
                    // comparar. `sitio_de` de `runtime/objetos/lista.inti`
                    // compara el indice con `cuantos` --que vive en la cabecera
                    // de la lista, a un `mov`-- y devuelve **0 si se sale**.
                    // Esto convierte ese 0 en la trampa.
                    //
                    // Son las mismas cuatro instrucciones que la Regla 3, y por
                    // la misma razon: lo que no tiene resultado no se mira
                    // DESPUES, se pregunta.
                    Comprobacion::Indice => {
                        carga(out, IZQ, sobre, &marco);
                        x86::test_r64_r64(out, IZQ, IZQ);
                        out.extend_from_slice(&[0x0F, 0x84]); // jz
                        huecos_de_atrapa.push((out.len(), codigo));
                        out.extend_from_slice(&[0, 0, 0, 0]);
                    }
                }
            }

            Instr::Devuelve(v) => {
                if let Some(v) = v {
                    carga(out, IZQ, v, &marco);
                }
                epilogo(out);
            }

            Instr::Salta(e) => {
                out.push(0xE9);
                huecos.push((out.len(), e.0));
                out.extend_from_slice(&[0, 0, 0, 0]);
            }

            Instr::SaltaSi { cond, falso, .. } => {
                carga(out, IZQ, cond, &marco);
                x86::test_r64_r64(out, IZQ, IZQ);
                // Si es cero, al camino falso; si no, sigue.
                out.extend_from_slice(&[0x0F, 0x84]);
                huecos.push((out.len(), falso.0));
                out.extend_from_slice(&[0, 0, 0, 0]);
            }

            Instr::Llama {
                destino,
                que,
                argumentos,
            } => {
                // ** Y antes de nada: es esto una llamada, o es LA PUERTA?
                //
                // La diferencia no la decide una palabra del lenguaje. La
                // decide una fila de `modulos.toml` que el usuario pidio con
                // `usa bmo` -- y por eso `invoca` nunca fue palabra clave, que
                // era la condicion que Eddi puso dos veces.
                //
                // Aqui se ve entera: quitar esa fila de la tabla apaga la
                // puerta sin tocar una linea de este fichero.
                if let Valor::Nombre(n) = que {
                    if taller.abre_la_puerta(n) {
                        let p = &taller.puerta;
                        for (i, a) in argumentos.iter().enumerate().take(p.caben()) {
                            carga(out, p.argumentos[i], a, &marco);
                        }
                        // Solo hay una puerta. Ese es el congelamiento de los
                        // dos syscalls, visto desde el unico sitio donde se
                        // notaria si dejara de ser verdad.
                        x86::mov_r32_imm32(out, p.numero, NR_INVOKE);
                        x86::syscall(out);
                        // ** Y de DONDE se recoge no lo decide la instruccion:
                        // lo decide el nombre. La misma puerta contesta un
                        // codigo y un valor a la vez, por registros distintos.
                        if let Some(d) = destino {
                            let de = p.recogida(taller.recoge.recoge(n));
                            guarda_temporal(out, de, *d, &marco);
                        }
                        continue;
                    }
                }

                // Los argumentos van a los registros que dice la convencion.
                //
                // ** Y aqui se ve para que sirve el freno del asignador: como
                // una funcion con llamadas no reparte registros, ningun
                // argumento puede estar viviendo en `rdi` cuando toca cargar el
                // siguiente. Cargarlos en orden es seguro **porque el reparto
                // se apago**, no por suerte.
                for (i, a) in argumentos.iter().enumerate().take(6) {
                    carga(out, ARGUMENTOS[i], a, &marco);
                }

                match que {
                    Valor::Nombre(n) => {
                        out.push(0xE8); // call rel32
                        salida_huecos.push((out.len(), n.clone()));
                        out.extend_from_slice(&[0, 0, 0, 0]);
                    }
                    otro => {
                        // Una llamada a un valor --una funcion guardada en una
                        // variable-- pide `call reg`. Se deja sin emitir en vez
                        // de emitir algo que salta a donde no debe.
                        let _ = otro;
                    }
                }

                // Lo que devuelve viene en el registro de retorno.
                if let Some(d) = destino {
                    guarda_temporal(out, IZQ, *d, &marco);
                }
            }
            // ** TOCAR MEMORIA. La IR pide "lee 8 bytes de esta direccion" y
            // aqui se elige la instruccion. Ese es el reparto entero: el ancho
            // en bytes es agnostico, el opcode no.
            Instr::Lee {
                destino,
                direccion,
                ancho,
            } => {
                carga(out, IZQ, direccion, &marco);
                match ancho {
                    // Un byte se lee con `movzx` y no con un `mov` de 8 bits:
                    // el `mov` dejaria intactos los 56 bits de arriba, asi que
                    // el resultado traeria basura de lo que hubiera antes en el
                    // registro. Lo peor es que funcionaria casi siempre.
                    1 => x86::movzx_r32_byte_at_reg(out, IZQ, IZQ),
                    2 => x86::movzx_r32_word_at_reg(out, IZQ, IZQ),
                    // ** El de 32 no lleva `movzx` y no es un olvido: escribir
                    // la mitad baja de un registro **pone a cero la mitad
                    // alta** en 64 bits. Por debajo de 32, el silicio conserva
                    // lo que hubiera, y por eso 8 y 16 si lo necesitan.
                    4 => x86::mov_r32_at_reg(out, IZQ, IZQ),
                    8 => x86::mov_r64_at_reg(out, IZQ, IZQ),
                    // Un ancho que no esta en la tabla no puede llegar aqui.
                    // Si llegara, devolver cero es lo unico honesto: dejar el
                    // registro como estaba seria mentir con la direccion
                    // dentro, y esa mentira parece un puntero valido.
                    _ => x86::zero_r32(out, IZQ),
                }
                guarda_temporal(out, IZQ, *destino, &marco);
            }
            // *** LA DIRECCION DE UNA TABLA CONGELADA (2026-08-22).
            //
            // Se emite un `mov reg, imm64` con el inmediato **a cero**, y se
            // apunta donde quedo: la direccion de verdad no se sabe hasta que el
            // cargador coloque `RoData`, y la rellena una reubicacion.
            //
            // ** Y esto es una INSTRUCCION de la IR, no un `Valor`, justamente
            // para que este sea el UNICO sitio del emisor que tiene que apuntar
            // una reubicacion. Con un `Valor::Congelado`, los veintitres sitios
            // que cargan un valor tendrian que acordarse -- y el dia que alguien
            // anadiera el veinticuatro, la tabla se cargaria con un cero.
            Instr::Direccion { destino, congelado } => {
                x86::mov_r64_imm64(out, IZQ, 0);
                // El inmediato son los ocho ultimos bytes de lo que se acaba de
                // emitir. Contarlo desde el principio del `mov` obligaria a
                // saber si el REX esta o no.
                reubicaciones.push((out.len() - 8, *congelado));
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            // *** EL MONTON DE LA TAREA: se DICE, no se baja a un cero.
            //
            // El slot vive en la seccion `Data` --la 1 en la numeracion de las
            // reubicaciones-- y quien lo rellena tiene que ser el arranque,
            // llamando a `monton_nuevo` antes de `principal`. Eso todavia no
            // existe: `arranque.rs` lo lleva escrito en su propia cabecera,
            // *"montar un monton: no, eso es `pleno` y llega despues"*.
            //
            // ** Bajarlo a un cero seria repetir exactamente el fallo que
            // `Const::Texto` tuvo durante meses: una pieza que se calcula bien y
            // no la lee nadie, y un binario firmado que devuelve basura. Aqui se
            // dice, con el numero de cuantas veces hizo falta.
            // *** EL MONTON DE LA TAREA, en dos instrucciones.
            //
            //     mov IZQ, <slot>     inmediato a cero + reubicacion a `Data`
            //     mov IZQ, [IZQ]      y lo que hay dentro es el monton
            //
            // Son dos y no una porque la direccion del slot **no se sabe al
            // emitir**: la elige el cargador. Es exactamente la misma forma que
            // `Instr::Direccion` usa para llegar a `RoData`, y por el mismo
            // motivo.
            //
            // ** Quien lo llena es el arranque, ANTES de `principal`. Si
            // `monton_nuevo` dijo que no, la tarea ya murio alli: aqui no puede
            // haber un cero.
            // *** LA DIRECCION DE UNA LOCAL: un `lea`, y nada mas.
            //
            // `lea IZQ, [rbp + disp]` calcula la direccion sin leer la memoria,
            // que es exactamente lo que hace falta: un `numero` mide 16 bytes y
            // **no se puede cargar**, solo senalar.
            //
            // ** Y `lea` no toca banderas, que importa aqui: entre una operacion
            // y su Regla 1 no puede meterse nada que las pise.
            Instr::DireccionDeLocal { destino, local } => {
                let disp = marco.local(*local);
                out.push(0x48 | (((IZQ >> 3) & 1) << 2));
                out.push(0x8D); // lea
                out.push(0x85 | ((IZQ & 7) << 3)); // [rbp + disp32]
                out.extend_from_slice(&disp.to_le_bytes());
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            Instr::MontonDeLaTarea { destino } => {
                x86::mov_r64_imm64(out, IZQ, 0);
                reubicaciones_del_monton.push(out.len() - 8);
                // `mov IZQ, [IZQ]`
                out.push(0x48 | ((IZQ >> 3) & 1) << 2 | ((IZQ >> 3) & 1));
                out.push(0x8B);
                out.push(((IZQ & 7) << 3) | (IZQ & 7));
                guarda_temporal(out, IZQ, *destino, &marco);
            }

            Instr::Escribe {
                direccion,
                valor,
                ancho,
            } => {
                // *** LOS DOS PUEDEN VIVIR EN EL REGISTRO DEL OTRO, y entonces
                // no hay orden que valga.
                //
                // Aqui habia esto escrito: *"el valor primero: cargar la
                // direccion antes lo perderia si los dos vivieran en el mismo
                // sitio"*. Vio la mitad del problema y arreglo la mitad.
                //
                // La otra mitad: si la DIRECCION vive en `DER`, cargar el valor
                // ahi primero la machaca **antes de leerla**. Y si ademas el
                // valor vive en `IZQ`, cualquiera de los dos ordenes destroza al
                // otro:
                //
                // ```text
                //    valor en IZQ, direccion en DER
                //    mov rcx, rax   -> el valor pisa la direccion
                //    mov rax, rcx   -> y ahora los dos son el valor
                // ```
                //
                // ** Lo encontro un programa de verdad --el escritor de PNG de
                // `ejemplos/`-- y no una prueba: hacen falta DOS operaciones
                // antes de la escritura para que los dos registros esten
                // ocupados a la vez. Con una sola, cualquiera de los dos ordenes
                // funciona, y por eso el banco no lo vio nunca.
                //
                // El caso cruzado se resuelve con un intercambio; los demas, con
                // el orden que no pisa.
                let en_izq = |v: &Valor| {
                    matches!(v, Valor::Temporal(t) if marco.sitio(*t) == Sitio::Registro(IZQ))
                };
                let en_der = |v: &Valor| {
                    matches!(v, Valor::Temporal(t) if marco.sitio(*t) == Sitio::Registro(DER))
                };
                if en_izq(valor) && en_der(direccion) {
                    // Cruzados: un `xchg` y los dos quedan donde toca.
                    x86::xchg_r64_r64(out, IZQ, DER);
                } else if en_der(direccion) {
                    // La direccion ya esta donde estorba: se salva primero.
                    carga(out, IZQ, direccion, &marco);
                    carga(out, DER, valor, &marco);
                } else {
                    // El caso corriente, y el que ya estaba bien.
                    carga(out, DER, valor, &marco);
                    carga(out, IZQ, direccion, &marco);
                }
                match ancho {
                    1 => x86::mov_byte_at_reg_from_low(out, IZQ, DER),
                    2 => x86::mov_word_at_reg_from_r16(out, IZQ, DER),
                    4 => x86::mov_at_reg_from_r32(out, IZQ, DER),
                    8 => x86::mov_at_reg_from_r64(out, IZQ, DER),
                    _ => {}
                }
            }
            // El metal se emite cuando el emisor lea `intrinsics.toml`. Se deja
            // marcado, no escondido.
            Instr::Metal {
                destino,
                nombre,
                argumentos,
            } => {
                metal(
                    out,
                    nombre,
                    argumentos,
                    *destino,
                    &marco,
                    taller,
                    &mut sin_emitir,
                );
            }
        }
    }

    // Toda funcion acaba volviendo, aunque el fuente no lo diga.
    epilogo(out);

    // ** EL SITIO AL QUE VAN LAS COMPROBACIONES QUE FALLAN -- uno POR CODIGO.
    //
    // Antes habia uno solo con `1001` escrito a mano, porque solo se emitia una
    // regla. Con dos, ese destino unico habria contado una mentira concreta:
    // atrapar por dividir entre cero habria devuelto E1001 --desbordamiento-- y
    // el programa habria dicho que le paso otra cosa.
    //
    // Van al final de la funcion y no al lado de cada comprobacion **a
    // proposito**: el camino que se recorre siempre es el que no atrapa, y
    // meterle un bloque de cinco instrucciones en medio lo llena de saltos.
    // Aqui el coste de la regla en el camino normal es UNA instruccion que casi
    // nunca salta -- que es de donde sale el 1% de la seccion 6.3.
    let mut codigos: Vec<u64> = huecos_de_atrapa.iter().map(|(_, c)| *c).collect();
    codigos.sort_unstable();
    codigos.dedup();
    for codigo in codigos {
        let atrapa = out.len();
        // *** P4(c) SE PROBO AQUI EL 2026-08-23, Y SE DEJO SIN APLICAR.
        //
        // ## Lo que hace hoy, y por que es un fallo de verdad
        //
        // El codigo se pone en el registro de retorno y la funcion VUELVE. Para
        // quien llamo, **atrapar y devolver un numero son la misma cosa**:
        //
        //     sube(1e18, 18)   llamada a pelo   ->  atrapa, devuelve 1001
        //     suma(c, a, b)    por dentro       ->  recibe 1001 como
        //                                          coeficiente y sigue
        //
        // Y se hace mas caro cada dia: cuanto mas runtime se escribe en INTI
        // --el monton, el contador, el decimal-- mas trampas viven dentro de una
        // llamada. Hay una prueba que lo fija:
        // `decimal::hoy_una_trampa_en_una_libreria_vuelve_como_un_numero`.
        //
        // ## El arreglo esta escrito y CABE EN DOS LINEAS
        //
        //     mov  <retorno>, codigo
        //     ud2                        <- y NO vuelve
        //
        // `PLAN_EL_SILICIO.md` P4(c) lo nombra: *"un corte que no se puede
        // confundir con un valor"*. Se aplico, compilo, y dejo el arbol en
        // verde menos TRES pruebas.
        //
        // ## [!] Y LA TERCERA ES POR LO QUE NO SE APLICO
        //
        // `sondas/cpu.inti` --la que corrio en el Ryzen el 22-08 y dio
        // `reglas = 0x00`-- tiene su diseno escrito en su propia cabecera:
        //
        //     "Una funcion que atrapa DEVUELVE EL CODIGO: la trampa pone el
        //      numero en el registro de retorno y sale, asi que preguntar
        //      'devolvio 1001?' es preguntar 'atrapo?'"
        //
        // **Esa premisa ES la ambiguedad que P4 existe para matar.** Con `ud2`
        // la primera trampa mata la tarea, asi que las tres reglas ya no se
        // pueden preguntar en una sola pasada: harian falta tres `.bex`, tres
        // pasos de despliegue y tres lineas de informe.
        //
        // *** O sea que P4(c) no es una linea del emisor: es un cambio en la
        // forma de la MEDIDA que produjo el mejor resultado de este proyecto. Y
        // eso lo decide el dueno, no el compilador.
        //
        // El plan ya sabe cual es la salida completa: P4(b) --que el KERNEL
        // aterrice en vez de enterrar-- y el error como dato. Con (b), la sonda
        // vuelve a poder preguntar tres veces.
        x86::mov_r64_imm64(out, IZQ, codigo);
        epilogo(out);
        // ** Y se APUNTA DONDE QUEDO. Es el unico momento en toda la compilacion
        // en que se sabe: dentro de un instante estos bytes son indistinguibles
        // del resto del codigo.
        cuenta.katanas.push((codigo, atrapa, out.len() - atrapa));
        for (h, c) in huecos_de_atrapa.iter().filter(|(_, c)| *c == codigo) {
            let _ = c;
            let rel = (atrapa as i64 - (*h as i64 + 4)) as i32;
            out[*h..*h + 4].copy_from_slice(&rel.to_le_bytes());
        }
    }

    // Y ahora los saltos.
    for (hueco, etiqueta) in huecos {
        let destino = sitios_de_etiqueta
            .iter()
            .find(|(e, _)| *e == etiqueta)
            .map(|(_, off)| *off)
            .unwrap_or(out.len());
        let rel = (destino as i64 - (hueco as i64 + 4)) as i32;
        out[hueco..hueco + 4].copy_from_slice(&rel.to_le_bytes());
    }

    cuenta.comprobaciones = comprobaciones;
    cuenta.reubicaciones_del_monton = reubicaciones_del_monton;
    cuenta.huecos_de_llamada = salida_huecos;
    cuenta.reubicaciones = reubicaciones;
    cuenta.sin_emitir = sin_emitir;
    cuenta
}
