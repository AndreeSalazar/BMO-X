# -*- coding: utf-8 -*-
"""EL BANCO: cada regla contra una tabla hecha para romperla EXACTAMENTE a ella.

** Un guardian que nunca ha dicho que NO no se ha visto funcionar. Esto es lo
que separa una regla de un adorno, y por eso corre en cada build.

[!] Se importa PEREZOSAMENTE desde `contrato.py` --dentro de `main`-- y no
arriba: este modulo necesita las reglas, y las reglas viven alli. Importarlo en
la cabecera seria una vuelta circular.

** Este fichero salio de partir `contrato.py` el 2026-09-02, cuando L6a dijo
que pasaba de las 1.000 lineas. El guardian clasifico el fichero como CAJON y
prescribio el remedio: *"mecanico: mover texto, y demostrable byte a byte"*.

Eso es exactamente lo que se hizo -- **ni una linea de logica cambio**, y se
comprobo contra la salida de `--check` y `--autoprueba` de antes del corte.

[!] Y el corte NO se eligio por gusto: se midieron las masas. `autoprueba` eran
274 lineas, los guardianes de REX 676, y el vocabulario cerrado 114. Tres masas
con nombre, y el resto --el contrato kernel<->ABI y el mando-- se queda en
`contrato.py`.
"""
from contrato import *  # noqa: F401,F403 -- las reglas que se prueban
from contrato_ley import *  # noqa: F401,F403
from contrato_rex import *  # noqa: F401,F403


def autoprueba():
    """Cada regla contra una tabla hecha para romperla EXACTAMENTE a ella."""
    fallos = []
    casos = [0]

    def exige(nombre, quejas, debe_quejarse=True):
        casos[0] += 1
        if debe_quejarse and not quejas:
            fallos.append("la regla %s NO dijo nada y tenia que decir que NO" % nombre)
        if not debe_quejarse and quejas:
            fallos.append("la regla %s se quejo de algo correcto: %s" % (nombre, quejas))

    # R1 -- un kind por encima de la mascara.
    exige("R1", r1_caben_en_su_campo({0x80: "KIND_MALO"}, {}, 0x7F))
    exige("R1(bueno)", r1_caben_en_su_campo({0x7F: "KIND_JUSTO"}, {}, 0x7F), False)

    # R2 -- un numero nuevo en las dos tablas, sin sellar.
    q, _ = r2_las_dos_tablas({0x33: "KIND_X"}, {0x33: "Otro"}, {})
    exige("R2", q)
    q, _ = r2_las_dos_tablas(
        {0x33: "KIND_X"}, {0x33: "Otro"}, {0x33: {"kernel": "KIND_X", "abi": "Otro", "nota": "ok"}}
    )
    exige("R2(sellado)", q, False)
    # Y una pareja que CAMBIA sin pasar por la linea base.
    q, _ = r2_las_dos_tablas(
        {0x33: "KIND_Y"}, {0x33: "Otro"}, {0x33: {"kernel": "KIND_X", "abi": "Otro", "nota": "ok"}}
    )
    exige("R2(pareja cambiada)", q)

    # R3 -- una operacion del kernel que el ABI no tiene, y una con otro numero.
    exige("R3(falta)", r3_operaciones_kernel({"TASK_OP_X": 1}, {}))
    exige("R3(numero)", r3_operaciones_kernel({"TASK_OP_X": 1}, {"TASK_OP_X": 2}))
    exige("R3(bueno)", r3_operaciones_kernel({"TASK_OP_X": 1}, {"TASK_OP_X": 1}), False)

    # R4 -- el userland pierde el prefijo, asi que las dos formas tienen que valer.
    exige("R4(falta)", r4_operaciones_userland({"OP_X": 1}, {}))
    exige("R4(prefijo)", r4_operaciones_userland({"OP_X": 1}, {"TASK_OP_X": 1}), False)
    exige("R4(numero)", r4_operaciones_userland({"OP_X": 1}, {"TASK_OP_X": 9}))

    # R5 -- dos operaciones de la misma familia con el mismo numero.
    exige("R5", r5_sin_numeros_repetidos({"TASK_OP_A": 3, "TASK_OP_B": 3}))
    exige("R5(distintas familias)", r5_sin_numeros_repetidos({"TASK_OP_A": 3, "FB_OP_B": 3}), False)
    # *** Y el falso positivo que la primera version SI daba: una operacion del
    # disco y un codigo de estado del TRIM comparten prefijo y no son la misma
    # enumeracion. Se queda como prueba para que nadie "simplifique" `_familia`.
    exige(
        "R5(prefijo compartido, enumeraciones distintas)",
        r5_sin_numeros_repetidos({"DISCO_OP_TRIM_LIBRE": 1, "DISCO_TRIM_SIN_DISCO": 1}),
        False,
    )
    exige(
        "R5(ES_NODO contra ES_TXT)",
        r5_sin_numeros_repetidos({"ES_NODO_HIJOS": 1, "ES_TXT_RUTA": 1}),
        False,
    )

    # R6 -- L6e: una clase inventada, y un suelo que baja.
    exige("R6(clase inventada)", r6_el_coste_declarado({"x.rs": "CARISIMO"}, 0))
    exige("R6(clase buena)", r6_el_coste_declarado({"x.rs": "MAQUINA"}, 0), False)
    exige("R6(el suelo baja)", r6_el_coste_declarado({"x.rs": "NADA"}, 5))
    exige("R6(el suelo sube)", r6_el_coste_declarado({"x.rs": "NADA", "y.rs": "TAREA"}, 1), False)

    # R7 -- L6f: lo mismo, y ademas que la SEGUNDA clase tambien se juzgue.
    exige("R7(clase inventada)", r7_el_riesgo_declarado({"x.rs": ("RARO",)}, 0))
    exige("R7(clase buena)", r7_el_riesgo_declarado({"x.rs": ("AJENO",)}, 0), False)
    # *** La que de verdad importa: dos clases, la primera buena y la segunda no.
    # Un juez que solo mire la primera palabra deja pasar la mitad de cada linea.
    exige("R7(la segunda tambien se mira)", r7_el_riesgo_declarado({"x.rs": ("AJENO", "RARO")}, 0))
    exige("R7(dos buenas)", r7_el_riesgo_declarado({"x.rs": ("AJENO", "ESPEJO")}, 0), False)
    exige("R7(el suelo baja)", r7_el_riesgo_declarado({"x.rs": ("AJENO",)}, 5))

    # R11/R12 -- L6g en REX. Las cabeceras son C, asi que la marca va dentro
    # del bloque de comentario y no en un `//!`. Se arman por trozos por el
    # mismo motivo que las de R8.
    CAR = " * [carril]  ROJO      x" + chr(10)
    CUE = " * [cuesta]  DATO      x" + chr(10)
    RIE = " * [riesgo]  AJENO ESPEJO" + chr(10)
    entera = CAR + CUE + RIE
    exige("R11(cabecera entera)", r11_el_semaforo_de_rex({"a/roja.h": entera}), False)
    exige("R11(sin carril)", r11_el_semaforo_de_rex({"a/x.h": CUE + RIE}))
    exige("R11(color inventado)",
          r11_el_semaforo_de_rex({"a/x.h": " * [carril]  AZUL x" + chr(10) + CUE + RIE}))
    # *** La regla de corte de L6e, que es la que crea las carpetas.
    exige("R11(dos costes = mal cortado)",
          r11_el_semaforo_de_rex({"a/x.h": CAR + " * [cuesta]  DATO PUERTA" + chr(10) + RIE}))
    # *** El renombrado a medias: se llama verde y dice ROJO.
    exige("R11(nombre contra etiqueta)", r11_el_semaforo_de_rex({"a/verde.h": entera}))
    # *** Y que la SEGUNDA clase de [riesgo] tambien se juzgue.
    exige("R11(la segunda clase tambien)",
          r11_el_semaforo_de_rex({"a/roja.h": CAR + CUE + " * [riesgo]  AJENO RARO" + chr(10)}))

    # R14 -- la operacion sale de REX, no del fichero.
    REXN = {"BMO_ARCH_LEER": (0x01, "archivo/roja.h")}
    D = chr(35) + "define "
    malo = D + "FB_BASE 0x01" + chr(10) + "x = bmo_valor(p, FB_BASE, 0, 0, 0);"
    q, _ = r14_ninguna_app_inventa_un_numero({"a.c": malo}, REXN)
    exige("R14(un numero copiado)", q)
    bueno = "x = bmo_valor(p, BMO_ARCH_LEER, 0, 0, 0);"
    q, _ = r14_ninguna_app_inventa_un_numero({"a.c": bueno}, REXN)
    exige("R14(el nombre de REX)", q, False)
    # *** Un alias que apunta a REX NO es pecado: el numero sigue en un sitio.
    alias = D + "MIO BMO_ARCH_LEER" + chr(10) + "x = bmo_valor(p, MIO, 0, 0, 0);"
    q, _ = r14_ninguna_app_inventa_un_numero({"a.c": alias}, REXN)
    exige("R14(alias de un nombre de REX)", q, False)
    # *** Y un literal desnudo INFORMA, no falla: la sonda de seguridad llama a
    # operaciones que no existen a proposito, y gritarle la apagaria.
    q, n = r14_ninguna_app_inventa_un_numero(
        {"a.c": "x = bmo_codigo(p, 0x7777, 0, 0, 0);"}, REXN)
    exige("R14(un literal no falla)", q, False)
    exige("R14(pero se informa)", n)

    # *** Y una tolerancia que ya no hace falta se dice tambien: una deuda
    # saldada que sigue escrita miente igual que una oculta.
    _guardadas = dict(ABI_CHOQUES_TOLERADOS)
    ABI_CHOQUES_TOLERADOS[("TASK_OP_", 0xEE)] = "una deuda que ya no existe"
    q, _ = r15_el_abi_no_repite_numero({"TASK_OP_A": (0x30, "t.rs")})
    exige("R15(tolerancia que sobra)", q)
    ABI_CHOQUES_TOLERADOS.clear()
    ABI_CHOQUES_TOLERADOS.update(_guardadas)

    # R16 -- la cobertura no baja.
    exige("R16(baja)", r16_la_cobertura_solo_sube(80, 197, 93))
    exige("R16(igual)", r16_la_cobertura_solo_sube(93, 197, 93), False)
    exige("R16(sube)", r16_la_cobertura_solo_sube(94, 197, 93), False)
    # *** Que la SUPERFICIE crezca no es un fallo: el ABI gana operaciones antes
    # de que exista la cabecera. Lo que no puede es perderse una que ya estaba.
    exige("R16(la superficie crece)", r16_la_cobertura_solo_sube(93, 240, 93), False)
    # *** Y la frontera es la que hace honesto el denominador.
    _abi = {"TASK_OP_X": (1, "t.rs"), "CABINA_Y": (2, "c.rs")}
    _c, _s = cobertura_de_rex(_abi, {"TASK_OP_X": {}}, [("CABINA_", "el panel")])
    exige("R16(la frontera no cuenta)",
          [] if (_c, _s) == (1, 1) else ["conto %d de %d" % (_c, _s)], False)

    # R15 -- el ABI no repite dentro de una familia.
    q, _ = r15_el_abi_no_repite_numero(
        {"TASK_OP_A": (0x30, "t.rs"), "TASK_OP_B": (0x30, "t.rs")})
    exige("R15(dos ops con el mismo numero)", q)
    q, _ = r15_el_abi_no_repite_numero(
        {"TASK_OP_A": (0x30, "t.rs"), "TASK_OP_B": (0x31, "t.rs")})
    exige("R15(numeros distintos)", q, False)
    # *** Familias distintas SI pueden compartir: `ARCH_OP_LEER` y
    # `INPUT_OP_PUNTERO` valen las dos 0x01 y no se estorban.
    q, _ = r15_el_abi_no_repite_numero(
        {"ARCH_OP_X": (0x01, "o.rs"), "INPUT_OP_Y": (0x01, "e.rs")})
    exige("R15(familias distintas)", q, False)
    # *** Y el 0x05 de INFO_TSC_HZ contra INFO_TXT_EXT_NOMBRE: NO es choque,
    # porque los TXT entran por otra operacion del kernel.
    q, _ = r15_el_abi_no_repite_numero(
        {"INFO_TSC_HZ": (0x05, "i.rs"), "INFO_TXT_EXT_NOMBRE": (0x05, "i.rs")})
    exige("R15(INFO contra INFO_TXT)", q, False)

    # -- El EXTRACTOR, que es de quien depende R13 entero -------------------
    # *** Los dos truncamientos que tuvo de verdad, cada uno con su nombre.
    exige("valor(desplazamiento)",
          [] if valor_exacto("1 << 1") == 2 else ["1<<1 no da 2"], False)
    exige("valor(guiones bajos)",
          [] if valor_exacto("0xFFFF_FFFF_FFFF_FFFE") == 0xFFFFFFFFFFFFFFFE
          else ["los _ truncan"], False)
    exige("valor(sufijo ULL)",
          [] if valor_exacto("0x8000000000000000ULL") == 0x8000000000000000
          else ["el sufijo estorba"], False)
    exige("valor(decimal)",
          [] if valor_exacto("64") == 64 else ["decimal mal"], False)
    # *** Y lo que NO se sabe evaluar contesta None en vez de inventarse algo.
    exige("valor(lo que no se sabe)",
          [] if valor_exacto("(1024 * 1024)") is None else ["se invento un valor"], False)
    exige("valor(un nombre)",
          [] if valor_exacto("OTRA_CONSTANTE") is None else ["se invento un valor"], False)

    # R13 -- el espejo. Se arman a mano, que es lo que permite probar los casos
    # que el arbol real no tiene (y ojala no tenga nunca).
    ABI = {"TASK_OP_X": (0x09, "tarea.rs")}
    REX = {"BMO_OP_X": (0x09, "bmo/roja.h")}
    PAR = {"TASK_OP_X": "BMO_OP_X"}
    SELLO = {"TASK_OP_X": {"c": "BMO_OP_X", "valor": 0x09, "nota": "ok"}}
    exige("R13(sellado y coincide)", r13_el_espejo_de_rex(ABI, REX, PAR, SELLO), False)
    # Los dos lados discrepan: es el caso obvio.
    exige("R13(discrepan)",
          r13_el_espejo_de_rex(ABI, {"BMO_OP_X": (0x0A, "bmo/roja.h")}, PAR, SELLO))
    # Una pareja nueva que nadie miro: gate de revision.
    exige("R13(pareja sin sellar)", r13_el_espejo_de_rex(ABI, REX, PAR, {}))
    # Un nombre que desaparece de un lado.
    exige("R13(el nombre de C se fue)", r13_el_espejo_de_rex(ABI, {}, {}, SELLO))
    exige("R13(el nombre del ABI se fue)", r13_el_espejo_de_rex({}, REX, {}, SELLO))
    # *** LA QUE JUSTIFICA LA TABLA: cambian los DOS a la vez. Una comparacion
    # en vivo diria que coinciden --y coinciden-- y se le escaparia el unico
    # cambio que rompe los `.bex` que ya estan firmados.
    exige("R13(cambian los dos a la vez)",
          r13_el_espejo_de_rex({"TASK_OP_X": (0x77, "tarea.rs")},
                               {"BMO_OP_X": (0x77, "bmo/roja.h")}, PAR, SELLO))

    FACH = "#include <bmo/x/roja.h>" + chr(10) + "#include <bmo/x/verde.h>" + chr(10)
    exige("R12(carpeta limpia)",
          r12_los_carriles_de_rex({"t/bmo/x": ["roja.h", "verde.h"]}, {"t/bmo/x.h": FACH}),
          False)
    exige("R12(un colado entre carriles)",
          r12_los_carriles_de_rex({"t/bmo/x": ["roja.h", "ayudas.h"]}, {"t/bmo/x.h": FACH}))
    exige("R12(sin fachada)",
          r12_los_carriles_de_rex({"t/bmo/x": ["roja.h"]}, {}))
    # *** La que paga el silencio: la fachada existe y se deja un carril fuera.
    exige("R12(carril fuera de la fachada)",
          r12_los_carriles_de_rex({"t/bmo/x": ["roja.h", "verde.h"]},
                                  {"t/bmo/x.h": "#include <bmo/x/roja.h>" + chr(10)}))

    # R8 -- L6g: las cuatro exigencias de critic/, cada una por separado. Se
    # arman por trozos y no con literales de varias lineas: el fichero que
    # contiene esta prueba se lee y se parchea a menudo, y una cadena con saltos
    # dentro es lo primero que se rompe al hacerlo.
    CU = "//! [cuesta] MAQUINA" + chr(10)
    RI = "//! [riesgo] AJENO" + chr(10)
    BA = "//! [prueba]  bmo-mmio-juicio" + chr(10)
    bueno = (CU + RI + BA, 10)
    # -- R8: el juez nombrado existe ----------------------------------------
    exige("R8(juez que existe)",
          r8_el_juez_nombrado_existe({"mm/phys/amarilla.rs": BA}), False)
    exige("R8(juez que no existe)",
          r8_el_juez_nombrado_existe(
              {"mm/phys/amarilla.rs": "//! [prueba]  bmo-no-existe" + chr(10)}))
    # No declararlo NO es un fallo: la mayoria de Ring 0 no tiene juez que sacar.
    exige("R8(sin declarar juez)",
          r8_el_juez_nombrado_existe({"core/gato/neon.rs": "//! un gato"}), False)

    # -- R9: los carriles POR MODULO ---------------------------------------
    #
    # ** El caso que justifica la regla entera es el tercero: un `.rs` sin
    # nombre de carril viviendo entre carriles. Los otros dos son el letrero;
    # ese es la carpeta.
    sano = {"roja.rs": CU + RI, "verde.rs": CU + RI, "mod.rs": ""}
    exige("R9(carpeta sana)",
          r9_los_carriles_del_modulo({"mm/vmm": sano}), False)
    exige("R9(carril sin cuesta)",
          r9_los_carriles_del_modulo({"mm/vmm": {"roja.rs": RI}}))
    exige("R9(carril sin riesgo)",
          r9_los_carriles_del_modulo({"mm/vmm": {"roja.rs": CU}}))
    exige("R9(colado entre carriles)",
          r9_los_carriles_del_modulo(
              {"mm/vmm": {"roja.rs": CU + RI, "ayudas.rs": CU + RI}}))
    # El verde SI es un carril aqui, y en `critic/` no. No es una incoherencia:
    # son dos jurisdicciones con dos listas, y este caso lo deja fijado.
    exige("R9(el verde es carril de modulo)",
          r9_los_carriles_del_modulo({"obj/fb": {"verde.rs": CU + RI}}), False)
    exige("R9(mod.rs no lleva letrero)",
          r9_los_carriles_del_modulo({"obj/fb": {"mod.rs": ""}}), False)
    exige("R9(sin carpetas)", r9_los_carriles_del_modulo({}), False)

    # -- R10: el semaforo ---------------------------------------------------
    CA = "//! [carril]  ROJO      porque si" + chr(10)
    exige("R10(con color)", r10_el_semaforo({"plat/spin.rs": CA}), False)
    exige("R10(sin color)", r10_el_semaforo({"plat/spin.rs": "//! un fichero"}))
    exige("R10(color inventado)",
          r10_el_semaforo({"plat/spin.rs": "//! [carril]  AZUL x" + chr(10)}))
    # *** El que de verdad guarda algo: un `verde.rs` que dice ROJO. Es un
    # renombrado a medias, y es como una pieza cambia de color sin que nadie lo
    # decida -- justo lo contrario de para lo que existe un semaforo.
    exige("R10(el nombre y la etiqueta se contradicen)",
          r10_el_semaforo({"obj/fb/verde.rs": CA}))
    exige("R10(el nombre y la etiqueta coinciden)",
          r10_el_semaforo({"obj/fb/roja.rs": CA}), False)
    exige("R10(sin ficheros)", r10_el_semaforo({}), False)

    if fallos:
        for f in fallos:
            print("  [X] " + f)
        print("autoprueba: %d regla(s) no saben decir que NO" % len(fallos))
        return 1
    # ** El numero se CUENTA, no se escribe. La version anterior decia "21
    # casos" y habia 19: un guardian con una cifra a mano dentro es un guardian
    # que dice un numero viejo con toda la confianza del mundo.
    print("clean: las DIECISEIS reglas saben decir que NO (%d casos)" % casos[0])
    return 0


# ===========================================================================

