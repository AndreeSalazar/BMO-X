# MAQUETA

El compilador de composicion del escritorio. Lee un arbol de cajas y unas
reglas; emite las coordenadas ya calculadas.

- **Como se construye** -> `docs/plan/PLAN_MAQUETA.md`
- **Que acepta y que rechaza** -> `docs/componente/LA_MAQUETA_EXIGE.md` (el contrato)

---

## El mapa, y por que esta cortado asi

```
   maqueta/
      lex/        abuelo     que trozos hay en el fichero
      node/       padre      que es cada pieza, con su nombre
      cascade/    hijo       que regla le toca a que nodo
      layout/     nieto      donde cae cada caja
      verdict/    bisnieto   si esto esta bien o no

      emit/                  LOS CONSUMIDORES -- no son generaciones
      vista/                 el reflejo: PPM con el rasterizador de verdad
      cli/                   el binario `maqueta`

      tema/                  tema.maqueta, la paleta compartida
      pruebas/               los ficheros dorados
```

### ★★ Las cinco de arriba son la CADENA. Las tres de abajo NO.

L7 dice *el conocimiento solo baja: ninguna generacion sabe quien la consume*.
Por eso `emit/`, `vista/` y `cli/` estan **al lado** y no debajo: son
consumidores, y **nadie de la cadena sabe que existen**.

Eso se puede comprobar sin leer una linea de logica -- basta mirar los
`Cargo.toml`:

```
   lex        <- sin dependencias
   node       <- lex
   cascade    <- node
   layout     <- cascade
   verdict    <- layout
   emit       <- verdict        (y verdict no menciona a emit)
   vista      <- verdict + bmo-dibujo
```

**Si algun dia una flecha apunta hacia arriba, el corte esta mal** (L7a). Es la
misma prueba que se le hace al kernel: `bmo-xhci` no nombra al teclado.

### Y lo que eso compra, en concreto

La eleccion entre **emitir Rust** o **emitir un recurso BEF 0x0B** no hay que
tomarla hoy. Se anade un segundo modulo en `emit/` y **no se toca ninguna de las
cinco**. La ley convirtio una decision irreversible en reversible.

---

## `vista/` -- el reflejo, y su precedente

Mismo argumento que `toolchain/tools/vista-ciudad`, escrito ahi el 15-08:

> *"Un arranque animado que solo se puede juzgar reiniciando la maquina es un
> arranque que nadie va a ajustar nunca."*

`vista-maqueta` pinta los rects con **`platform/shared/bmo-dibujo`, el mismo
rasterizador que corre en Ring 0 y en Ring 3**, y saca un PPM. No es una
imitacion: si aqui una caja se sale, en el Ryzen tambien.

★ Eso **degrada al navegador a boceto**. Un `.maqueta` se abre en Firefox y
orienta -- util, gratis, y con la fuente equivocada. La verdad es `vista/` y los
ficheros dorados de `pruebas/`.

⏳ **Tiempo real de verdad** (guardar el fichero y verlo cambiar en el Ryzen sin
reiniciar) es otra cosa y esta lejos: pide leer recursos en ejecucion (BEF 0x0B,
pendiente) mas un vigia de fichero. Escalon 8 de la escalera, no ahora.

---

## Nombres

**Identificadores y comentarios en INGLES**, desde la primera linea -- regla del
2026-08-08, incumplida tres veces, y su disparador es exactamente este: crear
ficheros nuevos en un arbol cuyos vecinos estan en espanol.

`maqueta` sobrevive como **nombre de producto** (como CABINA, DOOM o ESTRATOS),
no como identificador. Los crates son `bmo-maqueta-lex`, `-node`, `-cascade`,
`-layout`, `-verdict`; el reflejo es `bmo-vista-maqueta`, hermano de nombre de
`bmo-vista-ciudad`.
