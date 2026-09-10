# EL ORDEN -- que va primero entre veintidos planes, y por que

> Peticion del dueno, **2026-09-10**: *"puedes preparar el plan critico
> importante? hacer listas, dividir [...] para aplicar inteligente y poco a poco
> mejorar"*.
>
> [`ABIERTO.md`](ABIERTO.md) dice **QUE** falta -- 162 casillas en 22 planes. Lo
> que no existia es **QUE VA PRIMERO**, y eso no es una lista mas larga: es un
> criterio.

---

# 1. EL CRITERIO, antes que la lista

Cinco preguntas, en este orden. La primera que conteste que si decide.

```text
   1. DESBLOQUEA        una casilla que abre cinco vale mas que cinco sueltas
   2. CORRIGE UNA       un hueco te deja sin hacer algo; un numero que MIENTE
      MENTIRA           te hace decidir mal. Lo segundo es peor
   3. YA ESTA MEDIDO    si el numero existe y solo falta aplicarlo, es barato
   4. PIDE EL METAL     no se puede hacer sin arrancar: se agrupa en UNA tanda
   5. ES GRANDE Y NO    al final, sin culpa
      BLOQUEA NADA
```

*** **La 2 es la que mas cambia el orden**, y es propia de esta casa. Un plan
al que le falta una pieza avisa solo: no compila, o no arranca. Un contador que
da un numero plausible y falso **no avisa nunca**, y encima se usa para decidir.
Paso dos veces en septiembre y las dos las cazo mirar el metal, no una prueba.

> Lo que falta se nota. Lo que miente, no. Por eso lo que miente va antes.

[!] Y una decision del dueno que reordena la lista entera: **el asistente de IA
NO es prioridad.** Es el ultimo. Lo que arrastraba consigo --medir el ancho de
memoria, `exp` en INTI-- baja con el, salvo lo que sirva a otra cosa.

---

# 2. 🔴 CRITICO -- lo que hace falsa una promesa que el sistema ya hace

## [ ] C1 -- el juez del DMA solo esta cableado en UN aparato de tres

```text
   AHCI    4 sitios   comprueba la paridad, y nada mas
   xHCI    9 sitios   NO COMPRUEBA NADA
   NIC     1 sitio    la unica que ya esta bien: sale de una arena
```

### [!] Y el matiz importa, porque N0 **si esta bien marcado**

Al escribir esta lista se sospecho que `N0` estaba mal cerrado. No lo esta: el
embudo no se hizo funcion, se hizo **TIPO**. `bmo-dma-juicio::Prenda` envuelve
un `u64` con el campo privado, asi que la unica forma de tener una es `juzgar`,
y los drivers son crates distintos. **El embudo lo cuenta el compilador.**

*** Lo que falta no es el mecanismo: es la COBERTURA. El tipo es incorruptible
para quien lo usa, y **el xHCI no lo usa**. N2 cablo el juez en el disco
(`dev/disk/transfer.rs`) y ahi se quedo.

```text
   el juez        HECHO, y no se puede rodear      N0/N1, 09-09
   en el AHCI     CABLEADO                        N2, 09-09
   en el xHCI     nueve sitios, ninguno pasa      <-- ESTO
```

** Y por eso es el critico numero uno: no porque el juez sea malo, sino porque
**la casa ya cree que el problema esta resuelto**. Un mecanismo bueno aplicado
a un tercio del arbol se lee, desde fuera, exactamente igual que uno completo.

Ver [`NEUTRO/DMA/EMBUDO.txt`](../../NEUTRO/DMA/EMBUDO.txt), que sigue contando
14 y diciendo que tiene que llegar a 1.

**Desbloquea**: que las ocho reglas del DMA valgan para los tres aparatos y no
para uno. **Lo bloquea**: nada.

## [ ] C2 -- la firma: `sig_algo = 0` y el ancla de confianza vacia

Todo `.bex` sale sin firmar, y el validador lo dice por escrito: *"eso es
integridad, no autoria"*. El README promete que un binario corrupto se rechaza
**antes** de ejecutar, y eso hoy es media verdad.

** Y ya no es un problema de criptografia: `bmo-cripto` tiene Ed25519 completo
--637 lineas-- con SHA-512 debajo. **Falta cablearlo y decidir el ancla**, no
inventarlo.

*** Esto es exactamente lo que el README llama *"estricto consigo mismo"*. Un
sistema que promete verificar y no verifica no se estorba a si mismo: se deja
una puerta abierta y no lo dice.

**Desbloquea**: `bmo-verify`, los mods de codigo, y que la Base inmutable sea un
argumento y no una intencion.

## [ ] C3 -- las bandas verticales de DOOM

El unico fallo VISIBLE que lleva semanas sin explicacion. Dos hipotesis
falsadas y escritas (`sonda_columnas_de_doom.rs`, 22 celdas verdes): no era el
`-1` de `R_GetColumn` ni la division sin signo.

** Va en critico porque es lo unico de esta lista que **se ve en una foto**. Y
porque el patron de esta casa dice que un fallo de dibujo casi siempre acaba
siendo del compilador -- y eso lo paga todo el mundo, no solo DOOM.

---

# 3. 🟡 EL SIGUIENTE ARRANQUE -- gratis, y contesta cuatro preguntas

Todo esto es **una tanda de metal**, no cuatro. Se agrupa a proposito: arrancar
cuesta lo mismo para una pregunta que para cuatro.

```text
   [ ] M1  y callo         ya da microsegundos, o mi arreglo era falso?
   [ ] M2  cortadas        bajo? entonces el falso compartimiento era el culpable
   [ ] M3  ajenos          QUIENES son los 3 maestros que este kernel no encendio
   [ ] M4  N5b             el plazo de R-DMA-8 sale de `y callo` + margen
```

[!] **M3 es el que puede sorprender.** Tres aparatos alcanzan la RAM y BMO-X no
los adopto. Hasta saber cuales son, el cerrojo del portero duro **no se toca** --
y por eso salio de fabrica en `Mirar`.

---

# 4. 🟢 BARATO Y YA MEDIDO -- aplicar, no investigar

## [ ] B1 -- el lote de INVOKE: la puerta de 656 a 222 ticks

`ciclos.bex` ya midio la proyeccion entera en el Ryzen:

```text
   N=1   656 ticks/op        N=4   222   <-- bajo la meta de 300
   N=2   367                 N=8   150
```

** El fijo (578) se DIVIDE y el trabajo (78) no, asi que **el suelo del lote son
78 ticks, no cero**. La meta de 300 se cruza en N=4, y eso ya no es una
esperanza: es aritmetica sobre una medida.

Es el trabajo de `INVOKE` que el dueno pide, y vive en
[`PLAN_LA_PUERTA_SE_PARTE.md`](PLAN_LA_PUERTA_SE_PARTE.md) -- 8 casillas ya
hechas de 19.

## [ ] B2 -- `titular/` sale del kernel

Medido: **1.716 de las 2.928 lineas de `mm/` no tienen ni un `asm!`**, y el
unico rastro de x86-64 en `titular/` es una palabra en un comentario. El plan
esta escrito con su coste y sus casillas.

[!] **Despues de un arranque verde**, no antes. Ver
[`PLAN_LA_RAM_SALE_DEL_KERNEL.md`](PLAN_LA_RAM_SALE_DEL_KERNEL.md).

## [ ] B3 -- los tres numeros que le faltan al DMA inteligente

`INTELIGENTE.txt` dice que elegir entre corral, prestamo y rebote es una
costumbre que funciona, y que le faltan cuatro numeros. **Uno ya esta**: los
motivos de cada rebote se cuentan desde el 10-09. Quedan tres, y los tres se
miden con lo que hay:

```text
   [ ] cuanto cuesta un rebote en ESTA placa       `c/blit.bex` lo sabe medir
   [ ] cuanto cuesta revocar una pagina            M1b de LA_PUERTA_SE_PARTE
   [ ] el umbral del desmapeo: N `invlpg` vs `mov cr3`
```

*** Con esos tres, *"inteligente"* deja de ser una palabra y pasa a ser una
desigualdad. Sin ellos es una costumbre que acierta.

---

# 5. 🔵 GRANDE, Y NO BLOQUEA A NADIE

Van juntos porque comparten forma: mucho trabajo, ninguna urgencia, y ninguno
hace falsa ninguna promesa.

| plan | abiertas | de que |
|---|---|---|
| [`PLAN_AUTOCURACION.md`](PLAN_AUTOCURACION.md) | 17 | de informar a ACTUAR: cuarentena y supervision |
| [`PLAN_EL_GUARDIAN.md`](PLAN_EL_GUARDIAN.md) | 15 | BMO-X en RISC-V. **Compra una placa**, o sea que empieza fuera |
| [`PLAN_EL_PLAZO.md`](PLAN_EL_PLAZO.md) | 14 | V-Sync y la deuda del planificador |
| [`PLAN_EL_COMPAS.md`](PLAN_EL_COMPAS.md) | 12 | el turno se CONCEDE en vez de gastarse |
| [`PLAN_EL_CODEGEN.md`](PLAN_EL_CODEGEN.md) | 9 | 35 instrucciones para escribir 8 bytes |
| [`PLAN_EL_TROQUEL.md`](PLAN_EL_TROQUEL.md) | 9 | la geometria de los registros, de un golpe |

[!] `EL_PLAZO` y `EL_COMPAS` son **el mismo eje** --quien recibe turno y
cuando-- y `EL_FANTASMA` ya nombro su fallo: prioridad estricta sin
envejecimiento, con el compositor en 0. El dia que uno de los dos se toque, se
miran juntos.

---

# 6. ⚫ EL ULTIMO, por decision del dueno

**[`PLAN_EL_ASISTENTE.md`](PLAN_EL_ASISTENTE.md)** -- 13 abiertas. *"El
asistente IA no lo necesitamos, eso es el ultimo."*

Se queda escrito entero y no se archiva, porque **dos de sus casillas sirven a
otras cosas**:

```text
   A0  el ancho de memoria    lo pide tambien cualquier decision de rendimiento
   1b  el reparto en el ABI   `crew` ya da 11,27x y no hay puerta desde Ring 3
```

*** Y el hallazgo que dejo al ponerle casillas se queda dicho aunque el plan
duerma: **la criptografia ya no es el muro.** `bmo-cripto` tiene el juego
completo de TLS 1.3 en primitivas; lo que falta es el protocolo. Eso mueve C2
--la firma-- de *"meses"* a *"cablear lo que ya existe"*.

---

# 7. [!] LO QUE ESTE DOCUMENTO NO ES

**No es una fecha.** No hay semanas ni meses aqui, y es a proposito: esta casa
ya tiene un documento que se quemo estimando (`PLAN_EL_ASISTENTE`, seccion 7,
*"las cuatro cosas que MESES escondia"*).

**No es una lista de casillas.** Las casillas viven en sus planes y las cuenta
[`ABIERTO.md`](ABIERTO.md), que el build comprueba. Aqui solo esta el ORDEN, y
el orden se revisa cuando cambia el criterio -- no cuando cambia el humor.

**Y no se genera solo.** A diferencia de `ABIERTO.md`, esto es un juicio: que
DESBLOQUEA a que, y que MIENTE hoy. Ninguna maquina lo sabe todavia, asi que
envejece -- y por eso lleva la fecha arriba.
