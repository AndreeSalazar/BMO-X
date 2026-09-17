# PLAN LA DEUDA -- lo que el arbol debe, medido el 2026-09-17

> Peticion del dueno, **2026-09-17**: *"busca todos los elementos de
> vulnerabilidad, deudas [...] y organizar"*.
>
> Las vulnerabilidades tienen su plan ([`PLAN_SEGURIDAD.md`](PLAN_SEGURIDAD.md),
> seccion 5). Los lenguajes tienen el suyo ([`PLAN_EL_ENLAZADOR.md`](PLAN_EL_ENLAZADOR.md)
> y los de cada frontend). Este contesta la tercera pregunta: **que DEBE el
> arbol que no es un agujero ni una caracteristica que falta** -- lo que hace
> que tocarlo manana cueste mas que hoy.
>
> **Como se cae**: una casilla de aqui que diga un numero que el arbol ya no da.
> Cada una dice con que se midio, para que recontarla sea un rato.

---

## 0. La base, para no asustar de mas

```text
   bmo.ps1               verde, 145 s, sin tocar ningun disco
   el banco              2.871 filas, CERO rojas, 75 crates
   las reglas            23, todas saben decir que no
   R22 (L6i)             4 puertas contestan exito al negar -- 0 DEUDA
   opcodes del ABI       51, ninguno repetido (el 0x1C del 16-08 esta cerrado)
   NX, SMEP, SMAP, UMIP  encendidos
```

La deuda de abajo es real, pero es la de un arbol **que se mide a si mismo**:
casi todo lo que sigue lo dice ya algun guardian en cada build. Lo que faltaba
era juntarlo en un sitio y ordenarlo.

---

## 1. El criterio de orden

El de [`EL_ORDEN.md`](EL_ORDEN.md), sin cambiar una palabra: **lo que MIENTE va
antes que lo que FALTA**, porque lo que falta se nota y lo que miente no.

---

## D1 -- LO QUE MENTIA (y ya no)

- [x] **D1a -- `toolchain/lang/c/BRECHA.md` decia que `sprintf`, `puts`, `atoi` y
      `exit` NO compilaban.** Arreglado el 2026-09-17, y eran DOS fallos en
      `toolchain/tools/c-gen`, no uno:

  1. **Las sondas no llevaban su `#include`**: `int main(){exit(0);}` a secas. C99
     quito las declaraciones implicitas, asi que BMO C lo rechazaba **por el
     motivo correcto** y el informe decia "no hay `exit`" con `exit` en
     `stdlib.h`. Una sonda tiene que hacer la pregunta que hace un programa de
     verdad.
  2. **El generador escribia no-ASCII** (`--` como raya larga, `?` con apertura).
     El barrido del 08-08 limpio el FICHERO y no el GENERADOR, asi que
     regenerarlo rompia el guardian ASCII -- y nadie lo regenero en seis
     semanas. Ahora la salida pasa por la misma tabla que `ascii-sweep`, y un
     caracter que la tabla no conoce PARA el generador.

  El diff de la regeneracion son exactamente **cinco lineas**: las cuatro filas
  y la fecha.

- [x] **D1b -- `toolchain/tools/README.md` llamaba a `bmo-linker` "el enlazado
      propio".** Es un REGISTRO de simbolos ELF. Corregido el 2026-09-17, y la
      fila apunta al plan del enlazador que de verdad falta.

- [x] **D1c -- `bmo-linker` hacia `SKIP` y salia con 0.** Un modulo que no se
      podia leer se saltaba, y el registro salia sin sus simbolos con pinta de
      completo -- la regla 1b de la casa al reves. Ahora nombra cada modulo roto
      y no escribe nada. `toolchain/tools/bmo-linker/src/main.rs`, 2026-09-17.

- [x] **D1d -- `fat32::lba_de_cluster` imprimia un LBA inventado** justo el dia
      que el diagnostico hacia falta. Es el hallazgo 6 de `PLAN_SEGURIDAD`,
      seccion 5; aqui solo se cuenta como mentira cerrada.

- [ ] **D1e -- `platform/abi/bmo-abi/src/bef/linker/`: 308 lineas que dicen
      *"Debe llamarse una vez al boot"* y no las llama NADIE.** Es un enlazador
      DINAMICO (un registro global de exports que resuelve imports al cargar),
      con `#![allow(dead_code)]` en la cabecera, un `static mut SYMBOLS: Vec` y
      un cerrojo que es un `static mut LOCK: bool`. Y contradice una decision
      escrita: *"no hay enlazado dinamico y es una decision, no una carencia"*
      (`docs/maestro/PYTHON_MAESTRO.md`, seccion 6).

  **La decision es del dueno**, y son dos salidas: borrarlo con epitafio en el
  `Cargo.toml`, como los seis crates del 02-08, o que `PLAN_EL_ENLAZADOR` lo
  reclame. Lo que no puede es seguir afirmando que se llama al arrancar.

---

## D2 -- `static mut`: el bloqueante de SMP que crece mientras SMP espera

- [ ] **D2a -- un TRINQUETE.** `static mut` declarados en
      `Ultra_kernel_x86-64/kernel/src`, contados como declaracion (no como
      palabra en un comentario):

  ```text
     2026-08-06     209
     2026-08-11     236
     2026-08-12     253
     2026-09-17     385     +52 % en cinco semanas
  ```

  ```text
     dev/usb/mod.rs      30      red/mod.rs           11
     obj/file.rs         18      mm/titular/mod.rs    11
     dev/disk/mod.rs     17      uconsole.rs          10
     dev/usb/bus.rs      12      dev/keyboard.rs      10
  ```

  **No se propone quitarlos hoy**: se propone que dejen de CRECER sin que nadie
  lo vea, igual que los 37 avisos del compilador (`toolchain/tools/avisos`).
  Un guardian con `LINEA_BASE.txt` que solo deja bajar. El sitio natural es la
  misma puerta de `avisos`, para no engordar `build.ps1` (su propia linea base
  dice que *"el siguiente guardian NO se anade: primero se parte este
  fichero"*).

  **Como se mide**: `grep -rn '^\s*\(pub\(([a-z]*)\)\? \)\?static mut' --include=*.rs Ultra_kernel_x86-64/kernel/src | wc -l`.

- [ ] **D2b -- el reparto, por fichero, cuando SMP se retome.** No antes: sin
      SMP encendido, un `static mut` bien guardado por un solo nucleo no es un
      fallo, es un contrato que nadie ha escrito. Ver `docs/maestro/SMP_MAESTRO.md`.

---

## D3 -- Los monolitos que quedan

L6a ya tiene metro y trinquete (`toolchain/tools/censo-modular`). Lo que dice
hoy, sin tocar:

```text
   1869  toolchain/lang/cobol/src/codegen.rs
   1565  toolchain/lang/cobol/src/parser.rs
   1179  platform/abi/bmo-abi/src/bef/validator.rs
   1177  toolchain/lang/cpp/src/parser.rs         (aparcado: no se toca)
```

- [ ] **D3a -- el codegen de COBOL.** Es el mayor, y es el que va a crecer con
      `PLAN_BANCA`. Partirlo ANTES de las fases 3-6 es mas barato que despues. El
      metodo esta escrito y probado en el reparto de BMO C: cortar **por nombre
      de metodo**, y la prueba de que no cambio nada son **los mismos bytes** en
      todos los `.bex`, no que los tests pasen.
- [ ] **D3b -- el validador BEF.** 1.179 lineas de codigo en el gate por el que
      pasa todo `.bex`. Ahora tiene pasada hostil (`bmo-abi/tests/hostile.rs`),
      que es la red que hacia falta para partirlo sin miedo.

---

## D4 -- Crates que compilan y no tienen ni una prueba (14)

El banco de `bmo.ps1` los nombra en cada corrida. Ordenados por lo que cuesta
que fallen:

- [ ] **D4a -- `bmo-ahci`.** El driver del disco que SE ESCRIBE. La puerta que
      protege el NVMe de Eddi vive en `bmo-block` (con pruebas); lo que falta
      probar es la aritmetica de comandos y PRDT del propio AHCI -- el patron 4
      de los bugs, que ya costo una vez.
- [ ] **D4b -- `bmo-firmar`.** La unica herramienta que puede firmar. Sus cuatro
      "no" (privada dentro de un repo, pisar una clave, firmar digests que no
      cuadran, dejar el fichero tocado) estan escritos y ninguno tiene prueba.
- [ ] **D4c -- `bmo-pack` y `bmo-bex-link`.** Fabrican lo que el kernel carga.
- [ ] **D4d -- los binarios de demostracion** (`bmo-hello-bex`, `bmo-rpc-demo`,
      `bmo-vista-ciudad`, `bmo-fontgen`, `bmo-bef-bootstrap`, `bmo-estratos-fmt`)
      y `bmo-input`, `bmo-audio`, `bmo-maqueta`, `boot-context`: decidir UNO A UNO
      si llevan prueba o se declaran "herramienta sin banco" con su motivo.

---

## D5 -- Lo que ya tiene plan propio, y aqui solo se cita

No se copia: se apunta, para que la deuda entera se lea desde un sitio sin
duplicar casillas.

| deuda | donde vive | lo que dice hoy |
|---|---|---|
| el SEMAFORO: 70 ficheros ROJOS de 186 en Ring 0, drivers al 28 % | [`PLAN_EL_SEMAFORO_COMPLETO.md`](PLAN_EL_SEMAFORO_COMPLETO.md) | S4-S6 abiertas |
| el codegen de BMO C: 35 instrucciones para 8 bytes | [`PLAN_EL_CODEGEN.md`](PLAN_EL_CODEGEN.md) | C1-C9, cero hechas |
| lo que el compilador adivina | [`PLAN_NUNCA_ADIVINA.md`](PLAN_NUNCA_ADIVINA.md) | A4-A6 abiertas |
| el planificador sin envejecimiento | [`PLAN_EL_PLAZO.md`](PLAN_EL_PLAZO.md), [`PLAN_EL_COMPAS.md`](PLAN_EL_COMPAS.md) | P2, E0-E2 |
| el DIRECTOR y sus mezclas | [`PLAN_DIRECTOR_CENSO.md`](PLAN_DIRECTOR_CENSO.md) | 4 abiertas |

---

## D6 -- Pequenas, y se dicen para que no se pierdan

- [ ] **D6a -- la linea base de tamanos esta vieja.** `bmo.ps1` avisa en cada
      corrida: 14 de 40 ejecutables cambiaron y `sys/d.bex` crecio un 39,8 %.
      No es un fallo -- son 7 ejecutables nuevos y el DIRECTOR con ventanas --
      pero un aviso que sale siempre entrena a no leerlo. Fijarla es
      `py toolchain/tools/tamano/tamano.py --fijar`, **despues** de mirar que el
      +39,8 % del DIRECTOR es lo que se espera.
- [ ] **D6b -- `toolchain/tools/c-gen` no corre en el build.** Por eso
      `BRECHA.md` pudo mentir seis semanas. Candidato a la misma puerta que
      `avisos`: comprobar que la fecha de `Medido el` no tiene mas de N dias, sin
      recompilar nada en cada build.
- [x] **D6c -- `un_bLength_de_cero_no_cuelga`** en
      `platform/drivers/usb/uaudio/src/lib.rs` era un aviso de `non_snake_case`
      en cada `cargo test`. Renombrada el 2026-09-17.

---

## D7 -- Decisiones del dueno que se ejecutaron hoy

- [x] **D7a -- Python AOT, QUITADO** (2026-09-17). *"Es lo mismo como INTI"*.
      `docs/maestro/PYTHON_MAESTRO.md` seccion 4b, *UN SOLO MODO*: sin tipos, el
      AOT de Python solo quita el bucle de despacho (~2-4x); con tipos es otro
      lenguaje, y ese lenguaje ya existe.

---

Ver [`PLAN_SEGURIDAD.md`](PLAN_SEGURIDAD.md) seccion 5 (las vulnerabilidades de
esta misma pasada), [`PLAN_EL_ENLAZADOR.md`](PLAN_EL_ENLAZADOR.md) (lo que madura
a los lenguajes) y [`EL_ORDEN.md`](EL_ORDEN.md) (que va primero entre todos).
