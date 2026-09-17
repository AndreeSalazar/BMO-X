# PLAN ADA -- de "primer incremento" a "el fallo se caza antes de correr"

> Vive junto al frontend y no en `docs/plan/` por la regla de `docs/README.md`:
> *un documento sobre UNA PIEZA DE CODIGO vive junto a esa pieza* -- como
> `PLAN_BANCA.md` en `cobol/`.
>
> Escrito el **2026-09-17**, cuando el dueno pidio madurar *"por completo"* los
> lenguajes AOT. Medido contra `toolchain/lang/ada/src` ese dia.

---

## 0. El numero incomodo primero

```text
   toolchain/lang/ada     1.608 lineas, 6 ficheros, 20 pruebas
   ultimo cambio          2026-08-13 -- CINCO SEMANAS parado
   en el Ryzen            SI: docs/evidencia/09-ada-cierre-decimal-exacto.jpg
                          (19,99 x 3 = 59,97 exacto)
```

Y la frase que ordena todo el plan, porque es la prueba de fuego de Ada en
`toolchain/lang/PROPOSITO.md`:

> *esto convierte un fallo de ejecucion en uno de compilacion?*

**Hoy no.** `type Saldo is delta 0.01 digits 12` se declara y **nadie lo
verifica**: si un total se pasa de doce digitos, desborda callado, exactamente
como en C. El codegen no emite ni un `jo` ni una comparacion contra el tope.
**Hoy BMO Ada es sintaxis de Ada con la seguridad de C**, que es lo peor de los
dos -- y por eso la primera casilla no es una caracteristica nueva: es la
esencia que falta.

---

## 1. Los escalones

- [x] **A0 -- el primer incremento, en metal.** `examples/1-basico/cierre.adb`:
      ZFP secuencial + Annex F, `delta ... digits`, `Put_Line`, `if`, `while`.
      HECHO el 2026-07-30 en el emulador; la foto del Ryzen esta en
      `docs/evidencia/09-ada-cierre-decimal-exacto.jpg`.

- [ ] **A1 -- LAS COMPROBACIONES DE RANGO. La esencia.** Tres formas, y las tres
      son un `cmp` y un salto a un bloque que dice QUE tipo y QUE valor y termina
      -- la misma forma que la guarda de `OCCURS` de COBOL:

  ```text
     delta/digits    |valor| < 10^digits despues de cada operacion
     Integer         `jo` despues de + - *; division por cero antes de dividir
     subtype range   A .. B al asignar y al pasar un argumento
  ```

  **Y lo que se pueda decidir AL COMPILAR, se decide al compilar**: una
  constante fuera de rango no genera la comprobacion, genera un ERROR. Esa es la
  mitad de la prueba de fuego que no cuesta ni un ciclo en ejecucion.

  **Como se sabe**: filas de matriz que EJECUTAN un desbordamiento y exigen el
  mensaje exacto, y una que exige que `X : Saldo := 10.0 ** 13;` no compile.
  **Como se comprueba que no es un no-op**: mutar la guarda a nada y contar
  cuantas filas caen (el metodo que valido COMP-3 en COBOL).

- [ ] **A2 -- `elsif` y `for ... loop`.** Hoy se rechazan con motivo. Ergonomia
      barata, y `for I in 1 .. N` es ademas el primer sitio donde el RANGO de A1
      se usa sin que el programador lo escriba.

- [ ] **A3 -- subprogramas con parametros.** `procedure` y `function` locales con
      modos `in`, `out`, `in out`. Sin ellos no hay programa de banca que se
      pueda leer.

- [ ] **A4 -- `Ada.Sequential_IO`.** Sobre la misma puerta que abrio el File I/O
      de COBOL (`bmo_lower::archivo`). Su fila de matriz **con disco sembrado**,
      como la de COBOL.

- [ ] **A5 -- `Ada.Text_IO.Editing`, el PICTURE.** Es literalmente el de COBOL
      (Annex F copio ANSI X3.23-1985). **No se importa `lang/cobol`**: el motor
      de edicion se promueve a `toolchain/forge/` como libreria OPCIONAL que los
      dos eligen enlazar (regla 2). Decidido el 2026-07-30.

- [ ] **A6 -- `package` en UN fichero.** Especificacion y cuerpo en el mismo
      fuente, elaboracion en orden textual -- el estandar lo permite y GNAT solo
      lo desaconseja. **En dos ficheros** es `docs/plan/PLAN_EL_ENLAZADOR.md`, E7.

- [ ] **A7 -- ACATS como matriz.** La suite de conformidad son 1.821 pruebas; el
      subconjunto que toca es el de los capitulos 2 a 5 **sin tareas** (el perfil
      es ZFP). Se elige por lista escrita, no se importa entera, y cada prueba que
      entra EJECUTA.

---

## 2. Lo que NO entra, con motivo

| fuera | motivo |
|---|---|
| **tareas, `protected`, Ravenscar** | piden planificador DENTRO del runtime del lenguaje, y aqui el planificador es del kernel. ZFP es el perfil elegido |
| **genericos** | hasta que un programa de banca real los pida; hoy se rechazan con motivo |
| **excepciones propagadas y `exception when`** | piden tablas de desenrollado -- la misma razon por la que C++ las descarto. A1 TERMINA con mensaje, que es lo que ZFP permite (`pragma Restrictions (No_Exception_Propagation)`) |
| **`with` de paquetes que no son del compilador** | hasta E7 del enlazador |

---

## 3. Lo que este plan comparte con otros, y no duplica

- **Autohospedaje** (que Ada compile DENTRO de BMO-X): escalones 1, 2 y 4 de
  `docs/plan/PLAN_AUTOHOSPEDAJE.md` -- quitar `std`, `BTreeMap` en vez de
  `HashMap`, Ada como biblioteca `no_std`. Van por su lado: no bloquean A1.
- **El enlazador**: A6 en dos ficheros es E7 de `PLAN_EL_ENLAZADOR`.

---

Ver `toolchain/lang/PROPOSITO.md` (la prueba de fuego), `toolchain/lang/cobol/PLAN_BANCA.md`
(el hermano que ya recorrio este camino) y `toolchain/lang/c/VERDAD.md` (la
autoridad: el Ryzen, el documento, el emulador).
