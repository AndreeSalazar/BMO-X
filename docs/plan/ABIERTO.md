<!-- GENERADO por toolchain/tools/planes. No se edita a mano. -->

# LO QUE FALTA -- las casillas abiertas de los 26 planes

> Generado por `toolchain/tools/planes`. **El build comprueba que
> este fichero y los planes dicen lo mismo**, asi que no puede
> envejecer sin que algo se ponga rojo.

```text
   160 casillas ABIERTAS en 21 planes
   114 hechas
     5 planes CUMPLIDOS (ni una casilla pendiente)
     0 en plan/ SIN NI UNA CASILLA -- ver el final
```

---

# Los planes VIVOS, el que mas debe primero

## [`PLAN_AUTOCURACION.md`](PLAN_AUTOCURACION.md) -- 17 abiertas, 0 hechas

*El plan de la AUTO-CURACION: de informar a actuar*

- [ ] 1.0 (S) ★ Contar lo que queda del muerto DESPUES de revocar y
- [ ] 1.1 (S) Si algo no volvio, la linea sale en ROJO y dice que no
- [ ] 1.2 (S) La misma comprobacion en EXIT: una salida limpia tambien
- ... y 14 mas

## [`PLAN_EL_GUARDIAN.md`](PLAN_EL_GUARDIAN.md) -- 15 abiertas, 0 hechas

*PLAN EL GUARDIAN -- BMO-X como aparato, no como invitado*

- [ ] G1.1 -- una placa. VisionFive 2 / Milk-V, ~60-100 EUR. Sin PCIe
- [ ] G1.2 -- backend RISC-V en el toolchain. El emisor de x86-64 vive en
- [ ] G1.3 -- el arranque. No hay UEFI GOP: en RISC-V es SBI + device tree.
- ... y 12 mas

## [`PLAN_EL_PLAZO.md`](PLAN_EL_PLAZO.md) -- 14 abiertas, 1 hechas

*PLAN EL PLAZO -- V-Sync, VBlank y la deuda de planificacion*

- [ ] **P2.2 -- RESCHEDULE FORZADO: una tarea que se duerme suelta el CPU en el
- [ ] P2.3 -- el kernel publica el TIEMPO DE CPU de una tarea. Hoy
- [ ] P2.4 -- envejecimiento en choose_next, y SOLO si P2.1+P2.2 no bastan.
- ... y 11 mas

## [`PLAN_EL_ASISTENTE.md`](PLAN_EL_ASISTENTE.md) -- 13 abiertas, 2 hechas

*PLAN EL ASISTENTE -- un ayudante que corre DENTRO de BMO-X*

- [ ] 1a -- exp en INTI (dias). Lo unico que falta de matematicas --
- [ ] 1b -- el reparto de nucleos en el ABI (semanas). plat/smp/crew.rs
- [ ] 1c -- el motor de inferencia en INTI (semanas). El cargador de GGUF,
- ... y 10 mas

## [`PLAN_EL_COMPAS.md`](PLAN_EL_COMPAS.md) -- 12 abiertas, 0 hechas

*PLAN EL COMPAS -- el quantum se retira, y el turno se CONCEDE*

- [ ] E0 -- LA TAREA IDLE. Prioridad minima, siempre lista, cuerpo
- [ ] E1 -- EL TIEMPO DE CPU POR TAREA. Un contador en el cambio de contexto
- [ ] E2 -- (C,T) DECLARADOS Y EL AFORO. Cada tarea trae su compas; el kernel
- ... y 9 mas

## [`PLAN_LA_PUERTA_SE_PARTE.md`](PLAN_LA_PUERTA_SE_PARTE.md) -- 11 abiertas, 8 hechas

*PLAN LA PUERTA SE PARTE -- dividir lo que no se puede abaratar*

- [ ] M0b-2 -- lo que queda del papeleo, SI la medida lo pide. Quedan dos
- [ ] M0c -- los 112 ticks del match de INFO. El rechazo por campo
- [ ] M1b -- CUANTO CUESTA REVOCAR UNA PAGINA, y va ANTES de M1. La seccion
- ... y 8 mas

## [`PLAN_EL_CODEGEN.md`](PLAN_EL_CODEGEN.md) -- 9 abiertas, 0 hechas

*PLAN EL CODEGEN -- 35 instrucciones para escribir 8 bytes*

- [ ] C1 -- PLEGAR CONSTANTES. 1 * 8 es 8. Un operador binario con los
- [ ] C2 -- LITERALES PEQUENOS SIN movabsq. movabsq $0x1,%rax son diez
- [ ] C3 -- NO PASAR POR LA PILA CUANDO EL OTRO OPERANDO ES CONSTANTE.
- ... y 6 mas

## [`PLAN_EL_TROQUEL.md`](PLAN_EL_TROQUEL.md) -- 9 abiertas, 0 hechas

*PLAN EL TROQUEL -- la geometria de los registros, estampada de un golpe*

- [ ] S1 -- EL CONTRATO, EN PAPEL Y ANTES QUE EL CODIGO. Que entra y que
- [ ] S2 -- LA TABLA DE REGISTROS COMO DATO, no como codigo. x86-64 nombra
- [ ] S3 -- EL PRIMER CLIENTE: INTI. Porque puede declarar el alias. Y
- ... y 6 mas

## [`PLAN_AUTOHOSPEDAJE.md`](PLAN_AUTOHOSPEDAJE.md) -- 7 abiertas, 1 hechas

*PLAN DEL AUTOHOSPEDAJE -- que BMO-X compile SOBRE SI MISMO*

- [ ] 1 la sonda del hueco quitar std de toolchain/lang/ada y CONTAR
- [ ] 2 BTreeMap en Ada los 7 HashMap de toolchain/lang/ada/src.
- [ ] 4 ada como lib no_std toolchain/lang/ada/src/lib.rs con
- ... y 4 mas

## [`PLAN_ESTRUCTURA.md`](PLAN_ESTRUCTURA.md) -- 7 abiertas, 1 hechas

*PLAN DE ESTRUCTURA -- el taller de BMO-X, en F1*

- [ ] 1 F1 abre una ventana VACIA en `Ultra_userspace/services/director/
- [ ] 2b la ventana con REJILLA scroll como modulo reutilizable, de la
- [ ] 3 estructura.bex DIBUJA una ventana con su rejilla y su cursor,
- ... y 4 mas

## [`PLAN_MEDIOS.md`](PLAN_MEDIOS.md) -- 7 abiertas, 0 hechas

*PLAN MEDIOS -- VLC como objetivo, medido contra lo que hay*

- [ ] el tubo abre (A1) <- lo unico que bloquea M1, y es un ARRANQUE
- [ ] M1 WAV dias despues del tubo
- [ ] M2 MP3 (= A5) media tarde de comprobar la coma flotante antes
- ... y 4 mas

## [`PLAN_EL_NEUTRO_VIGILADO.md`](PLAN_EL_NEUTRO_VIGILADO.md) -- 6 abiertas, 11 hechas

*PLAN EL NEUTRO VIGILADO -- que algo procese el DMA aunque la CPU no mire*

- [ ] N5b -- EL NUMERO, y no se elige (LEY 24). peor_silencio() guarda lo
- [ ] varios arranques con mudo= anotado, incluido uno con DOOM leyendo el WAD
- [ ] elegir el margen y escribirlo en PLAZO_SIN_MEDIR con su porque
- ... y 3 mas

## [`PLAN_EL_PIXEL.md`](PLAN_EL_PIXEL.md) -- 6 abiertas, 0 hechas

*PLAN EL PIXEL -- las reglas de lo que se pinta, y donde estan los milisegundos*

- [ ] Y1.1 subir el bInterval del raton a Ring 0 y a Ring 3. Sin ese
- [ ] Y1.2 que BUS_PERIOD_MS salga del minimo de los aparatos vivos y no
- [ ] no promete 0 ms, y llamarlo asi seria vender humo: un pixel viaja por
- ... y 3 mas

## [`PLAN_DOOM.md`](PLAN_DOOM.md) -- 5 abiertas, 3 hechas

*El plan largo: de "BMO C compila 69 de 81" a "DOOM se juega en el Ryzen"*

- [ ] pendiente [~] a medias, y se dice cuanto [x] hecho, con fecha
- [ ] 2 A: tipar la binaria las 3 casillas nuevas en verde, y 449 sin
- [ ] 3 recompilar doom.bex y comparar el tamano contra 880.250
- ... y 2 mas

## [`PLAN_LA_CARA_VIAJA.md`](PLAN_LA_CARA_VIAJA.md) -- 5 abiertas, 2 hechas

*PLAN: LA CARA VIAJA*

- [ ] 3 el LECTOR, las cinco comprobaciones -> Ultra_userspace/services/director/src/scene/cara.rs
- [ ] 4 desde un FICHERO suelto, y que se pinte -> Ultra_userspace/services/director/src/scene/cara_ca
- [ ] 5 en la seccion 0x0B del .bex -> toolchain/tools/maqueta/pruebas/calc.dorado
- ... y 2 mas

## [`PLAN_LA_PILA_HUERFANA.md`](PLAN_LA_PILA_HUERFANA.md) -- 5 abiertas, 3 hechas

*PLAN: LA PILA HUERFANA*

- [ ] 1. ARRANCAR Y LEER. Reproducir --matar Ring 3, volver a entrar-- y
- [ ] 2. EL JUEZ, en su crate. platform/shared/bmo-pila-juicio: *"se
- [ ] 3. reap PREGUNTA AL JUEZ en vez de mirar solo su rsp. El cambio
- ... y 2 mas

## [`PLAN_LA_RAM_SALE_DEL_KERNEL.md`](PLAN_LA_RAM_SALE_DEL_KERNEL.md) -- 4 abiertas, 3 hechas

*LA RAM SALE DEL KERNEL -- que parte es agnostica, medido*

- [ ] un arranque verde con lo que ya hay (vuelo, mudo, ajenos, centinela)
- [ ] sacar titular/ a platform/shared/bmo-marcos, con la tabla como
- [ ] sus filas de banco -- las ocho reglas del DMA, en el anfitrion
- ... y 1 mas

## [`PLAN_EL_SILICIO.md`](PLAN_EL_SILICIO.md) -- 3 abiertas, 5 hechas

*PLAN EL SILICIO*

- [ ] S6 -- el techo de crudo (roca 3): hoy no lo acota nadie
- [ ] S7 -- las katanas del silicio, P4 y P5 de la seccion 5 de este mismo
- [ ] S8 -- que el barrido NIEGUE en vez de callar cuando no puede leer una

## [`PLAN_AUDIO.md`](PLAN_AUDIO.md) -- 2 abiertas, 10 hechas

*PLAN AUDIO -- las casillas de su MAESTRO, medidas contra el codigo*

- [ ] A1 -- SET_INTERFACE -- ⛔ EL RYZEN LO NEGO. Corregido el 26-08, sin ejecutar
- [ ] A1 SET_INTERFACE EL METAL LO NEGO; corregido 26-08

## [`PLAN_REX.md`](PLAN_REX.md) -- 2 abiertas, 15 hechas

*REX -- la puerta de los terceros, ORDENADA*

- [ ] 5b <bmo/latido.h> LATIDO + WAIT el tiempo, y la 2a puerta
- [ ] 5c <bmo/corriente.h> ARCHIVO_ASINC + LISTO leer a ritmo de quien lee

## [`PLAN_MAQUETA.md`](PLAN_MAQUETA.md) -- 1 abiertas, 8 hechas

*PLAN MAQUETA*

- [ ] 7 ficheros dorados como oraculo -> toolchain/tools/maqueta/pruebas/calc.dorado

---

# CUMPLIDOS -- todas sus casillas marcadas

** No se archivan ni se mueven: siguen siendo la razon por la
que algo se hizo asi, y eso se consulta mas que la casilla.

- [`PLAN_ALMACENAMIENTO.md`](PLAN_ALMACENAMIENTO.md) -- 5 hechas, 205 lineas
- [`PLAN_DIRECTOR.md`](PLAN_DIRECTOR.md) -- 7 hechas, 1005 lineas
- [`PLAN_EL_PERFIL_TOTAL.md`](PLAN_EL_PERFIL_TOTAL.md) -- 8 hechas, 611 lineas
- [`PLAN_SEGURIDAD.md`](PLAN_SEGURIDAD.md) -- 17 hechas, 785 lineas
- [`PLAN_SUELO_RING3.md`](PLAN_SUELO_RING3.md) -- 4 hechas, 316 lineas

