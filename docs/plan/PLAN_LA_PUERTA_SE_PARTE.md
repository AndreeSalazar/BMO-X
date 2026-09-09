# PLAN LA PUERTA SE PARTE -- dividir lo que no se puede abaratar

> Propuesta del dueno, **2026-09-09**:
>
> > *"me gustaria saber si es posible hacer que aunque syscall esta definido
> > como es, que sea division que divida los syscall como son pero que no se
> > sumen, para intentar fragmentar todo y llegue el objetivo. Es como que si
> > ese ciclo es 969 ciclos pero me gustaria que se divida en 10, lo que MAS
> > PUEDA para que llegue el destino."*
>
> [!] **La idea funciona. Pero no sobre la pieza que senala**, y esa distincion
> es todo este documento.

---

# 1. [!] LO QUE NO SE PUEDE DIVIDIR, Y HAY QUE DECIRLO PRIMERO

`syscall` y `sysretq` son **un par de instrucciones**. Cada una es microcodigo
del CPU que hace, de una vez y sin puntos intermedios:

```text
   syscall     lee IA32_LSTAR y IA32_STAR      cambia CS/SS y el CPL
               enmascara RFLAGS con SFMASK     serializa el cauce
   sysretq     lo mismo al reves
```

No hay forma de ejecutar "un tercio de un `syscall`". No es una limitacion de
BMO-X ni una decision de diseno que se pueda revisar: **es una instruccion**. El
perfil de esta placa la estima en **~150 ticks** y lo declara como estimacion
(`suelo: Suelo { ticks: 150, medido: false }`).

> Ese numero es el unico de toda la cuenta que **no ha bajado en treinta anos**:
> Liedtke consiguio ~250 ciclos en L4 sobre un 486 en los 90.

** Asi que la pregunta *"como divido los 150"* no tiene respuesta. La que si la
tiene es otra, y es mejor:

```text
   NO   como hago que cruzar cueste menos de 150
   SI   como hago que 150 sirvan para DIEZ operaciones en vez de una
```

Eso no es un juego de palabras: es exactamente lo que el dueno describio --*"que
se divida en 10"*-- aplicado a la parte que si se puede repartir.

---

# 2. *** DE QUE ESTA HECHA UNA PUERTA, Y POR QUE ESO DECIDE TODO

Una puerta cuesta dos cosas que se suman y **no se parecen en nada**:

```text
   FIJO      cruzar, guardar 15 registros, entrar en `dispatch`, volver
             y `sysretq`. Lo paga TODA puerta, haga lo que haga
   TRABAJO   resolver el handle, mirar la tabla, hacer la operacion.
             Depende de lo que se pidio
```

Y aqui esta la palanca entera:

```text
   el FIJO se paga UNA vez por CRUCE
   el TRABAJO se paga UNA vez por OPERACION
```

Si una sola puerta llevara N operaciones, el coste por operacion seria:

```text
   FIJO/N + TRABAJO
```

** El fijo **se divide**. El trabajo no. Cuanto se gana depende enteramente de
cual de los dos es la mayoria -- y hasta hoy **eso no se habia medido nunca**.

---

# 3. ★★ COMO SE MIDE EL FIJO SIN INSTRUMENTAR EL STUB

Con una puerta que **el kernel rechaza**. `INVOKE` sobre `BMO_TAREA_ACTUAL` con
una operacion que no existe cae en `_ => unsupported()`: un `BmoStatus::err` y
se vuelve. Recorre la maquina entera y no hace ningun trabajo.

```text
   cruzar `syscall`      si
   guardar los 15 GPR    si
   entrar en `dispatch`  si
   el trabajo pedido     NO, no existe
   devolver y `sysretq`  si
```

*** **Un rechazo ES el coste fijo.** Y la casa ya lo habia visto sin buscarlo:
`medida/coste` lleva escrito que una sonda paso el campo `0` a `INFO` por error,
cayo en el brazo por defecto, y **salio 784 contra los 870 de una operacion de
verdad**. Se leyo como *"la sonda estaba mal"*, se arreglo, y el 784 se tiro.

> Era el numero mas valioso de los dos. Un rechazo no es una medida estropeada:
> es la unica forma de medir el cruce sin poner un `rdtsc` dentro del stub.

# 3b. ★★★ EL METAL CONTESTO -- 2026-09-09, `c/ciclos.bex` en el Ryzen

```text
   0. bucle vacio         min   11   media   124
   1. llamada normal      min   32   media    38
   2. rdtsc suelto        min  111   media   112
   3. RECHAZO (op)        min  633   media   921
   4. RECHAZO (campo)     min  745   media  1150
   5. PID (la barata)     min  780   media  1061
   6. INFO ticks          min  874   media  1320
```

```text
   FIJO      633     el 81 %
   TRABAJO   147     el 19 %   (PID)
```

Y la tabla, con los numeros de verdad:

```text
     N    ticks/op
     1        780
     2        463
     4        305
     8        226     <== bajo la meta de 300
    32        166
```

** **Ocho operaciones por puerta cumplen la meta**, no cuatro. La proyeccion de
antes se hizo con el 784/86 viejo y salio optimista: el trabajo es mas gordo de
lo que se creia y el fijo mas pequeno.

## ★★★ Y AQUI ESTA LA NOTICIA QUE NADIE BUSCABA

```text
   el FIJO medido                   633 ticks
   el cruce del silicio (estimado)  150 ticks
   ------------------------------------------
   lo que NO es el silicio          483 ticks   <- el 76 % del fijo
```

*** **Tres cuartas partes del coste fijo son codigo NUESTRO, no fisica.** Y el
prologo mas el epilogo estan medidos en 30 + 30. O sea que ~420 ticks estan
entre el `call {dispatch}` y la primera linea util -- y ahi solo hay tres cosas.

### Los tres sospechosos, y dos son INSTRUMENTOS

```rust
let __metro = meter::start();                       // gated: vale 0 sin metro
registrar_publicacion(trap_rsp(), current_tid());   // SIEMPRE
let clase = match frame.rax { ... };                // SIEMPRE, ~3 comparaciones
meter::count_class(clase);                          // SIEMPRE
```

** Y `current_tid()` es esto (`scheduler/verde.rs:352`):

```rust
pub fn current_tid() -> u32 {
    let _g = SCHED_LOCK.lock();     // <-- UN CERROJO
    let s = sched();
    s.tasks[s.current].tid
}
```

*** **Toda puerta de BMO-X cierra el planificador para leer un `u32`.** Y su
gemelo `current_pid()` es identico, lo que explica el otro numero: **el
`trabajo de PID` son 147 ticks para leer un entero**, porque lo que se paga no
es la lectura, es el cerrojo.

Y `registrar_publicacion` hace, ademas:

```text
   2 escrituras volatiles a dos arrays de 4
   1 lectura volatil de `base + XSAVE_BV`   <- la pila que la VIA RAPIDA ya no
                                               escribe: linea probablemente FRIA
```

[!] Y a esa contabilidad **la lee un solo sitio**: `plat/faults/roja.rs:598`, el
informe de fallos. O sea que corre en TODA puerta y se lee **solo cuando algo se
cae**.

> Es la misma frase que esta casa se escribio a si misma al retirar los cuatro
> sellos `rdtsc` del stub: **un instrumento que ya dio su numero y sigue
> cobrando es un peaje, no una medida.** Van dos.

### La otra medida que salio de paso

```text
   los dos rechazos difieren en 112 ticks
```

El rechazo por OPERACION cuesta 633 y el rechazo por CAMPO 745. El segundo
entra en `OP_INFO` y recorre su `match` de **100 campos** antes del `_ => 0`.
Esos 112 ticks son lo que cuesta ese `match`, y los paga toda lectura de `INFO`.

### Y la expropiacion, que es el tercer numero

```text
   PID    min 780   media 1061    +36 %
   INFO   min 874   media 1320    +51 %
```

El minimo es la puerta; **la media es la puerta mas la probabilidad de que te
la quiten**. Para el compositor --que hace puertas todo el rato-- la que manda
es la media, y nadie la habia mirado.

[!] Y si al medir sale al contrario --que el trabajo es la mayoria-- **este plan
se archiva y el trabajo es otro**: abaratar el trabajo. `c/ciclos.bex` existe
para decidir eso, y lo dice en pantalla con esas palabras.

---

# 4. ** Y LOS DOS SYSCALLS CONGELADOS NO SE TOCAN

Esta es la parte que hace el plan posible en BMO-X y no en otro sitio.

Un lote **no es un syscall nuevo**. Es una operacion:

```text
   INVOKE(CURRENT_TASK, OP_LOTE, puntero_al_array, n)
```

`INVOKE` ya recibe (handle, operacion, argumentos). Un array de operaciones es
un argumento como cualquier otro. O sea que:

```text
   [x] los DOS syscalls siguen siendo dos
   [x] el ABI no crece un opcode de puerta: crece UNA operacion
   [x] un binario viejo no se entera de nada
   [x] R14 y R19 lo juzgan como a cualquier otra operacion
```

*** La superficie congelada aguanta porque **la congelacion era de los
syscalls, no de las operaciones**. Eso estaba pensado desde el principio y esta
es la primera vez que cobra.

---

# 5. [!] LO QUE ESTE PLAN SACRIFICA (L3)

Toda regla trae su sacrificio, y estos son cuatro y ninguno es pequeno.

```text
   1. NO SE PUEDE RAMIFICAR DENTRO DE UN LOTE
      Si la operacion 3 decide si se hace la 4, el lote no sirve: hay que
      cruzar, mirar, y volver a cruzar. Un lote es para N cosas que YA se
      sabe que hay que hacer.

   2. LOS ERRORES DEJAN DE SER UNO
      Hoy una puerta devuelve UN `BmoStatus`. Un lote de 32 devuelve 32, y
      eso son 32 escrituras en memoria del usuario que HAY QUE PAGAR -- son
      parte del TRABAJO, no del fijo, asi que no se dividen.

   3. EL KERNEL TIENE QUE VALIDAR N ARGUMENTOS
      Un puntero del usuario con N entradas es N veces la superficie de
      ataque de uno. Y no se valida "el array": se valida CADA entrada,
      antes de ejecutar la primera. Eso es trabajo nuevo en Ring 0 y en el
      carril ROJO.

   4. LA PRIMERA OPERACION SE VUELVE MAS LENTA
      Si el que pide espera a juntar 32, la primera espera a las otras 31.
      Un lote baja el coste MEDIO y sube la latencia del PRIMERO. Para el
      camino de la mano al pixel --que es el que manda-- eso puede ser un
      empeoramiento, y por eso el lote es para el trabajo A GRANEL, no para
      la entrada. Ver `PLAN_EL_PIXEL.md`.
```

** El 4 es el que hay que llevar puesto: **`WAIT` existe justo para no esperar,
y un lote es esperar a proposito.** Los dos syscalls tiran en direcciones
opuestas y las dos direcciones son correctas -- cada trabajo elige.

---

# 6. LA TERCERA VIA, QUE ES MEJOR QUE LAS DOS

Hay operaciones que no necesitan cruzar **nada**: las que solo LEEN un numero
que el kernel ya tiene escrito.

```text
   INFO_TICKS        un contador
   INFO_TAREAS       un contador
   INFO_USB_RITMO    dos numeros y un indice
```

Nada de eso decide nada ni cambia nada: se puede publicar en una pagina de
**solo lectura** mapeada en el espacio de la app. Y entonces leerlo cuesta:

```text
   por la puerta         ~870 ticks
   de una pagina suya      ~4 ticks (un acierto de L1)
```

*** Eso no es dividir por 10: es dividir por 200. Y **BMO-X ya lo hace en un
sitio**: `DIRECTOR` le da a una app en ventana sus teclas y su raton por un buzon
en su propia memoria, *cero syscalls* (ver `docs/` de superficies, 23-08).

[!] Lo que cuesta: una pagina publicada es un contrato de FORMATO --si el kernel
mueve un campo, toda app compilada contra el formato viejo lee basura sin
enterarse--. Una puerta puede cambiar de version; una pagina compartida, no. Por
eso esta via es solo para lo que **nunca va a cambiar de forma**, y decidir eso
es el paso M1.

---

# 6b. [!] EL PRECIO DE NO CRUZAR ES EL DESMAPEO EN CALIENTE

> Pregunta del dueno, **2026-09-09**: *"el hot-unmapping, podemos agregar? Eso
> podria ser habilidad de mi BMO-X porque si es como ya sabemos tipico por
> ciclos, puede ayudar?"*

Dos respuestas, y la segunda es la que importa.

## 1. Ya existe, y por ciclos va en CONTRA

`vmm::unmap_page` desmapea una pagina viva y hace su `invlpg`
(`mm/vmm/amarilla.rs:251`). Lo usan SIETE sitios: `obj/fb`, `obj/loan`,
`obj/memory`, `obj/mmio` y `obj/cap`. Y `fb::release` es hot-unmapping de libro:

```rust
let bytes = mapped_bytes();
let mut off = 0u64;
while off < bytes {
    vmm::unmap_page(aspace, vmm::FRAMEBUFFER_VA_BASE + off);
    off += mm::PAGE;
}
```

*** No es una habilidad que falte: **es una que ya se paga**. Y no ahorra
ciclos, los cuesta -- un `invlpg` es del orden de cientos de ciclos, y un
framebuffer de 1920x1080x4 son **2.025 paginas**. Ese numero no esta medido
aqui, y por LEY 24 no se estima: se mide (ver el paso M1b).

## 2. ★★★ Pero es el precio que la SECCION 6 no habia dicho

La seccion 6 dice que una pagina de solo lectura cuesta *"un contrato de
formato"*. **Eso es la mitad.** La otra mitad es esta, y es mas grave:

```text
   por la PUERTA     780 ticks por lectura   revocar es GRATIS
                                             (se sube la generacion y el
                                              siguiente handle rebota EN la
                                              puerta, sin tocar nada mas)

   por la PAGINA       4 ticks por lectura   revocar cuesta DESMAPEAR
                                             (no hay puerta donde comprobar
                                              nada: la app escribe y ya)
```

*** **Una capability se revoca con un numero. Un mapeo, solo con un `invlpg`.**
Esa asimetria es toda la diferencia entre las dos vias, y no es un detalle de
implementacion: es lo que hace que la via de 4 ticks no sea gratis.

> El que no cruza la puerta no puede ser detenido en la puerta.

## 3. [!] Y DOS LIMITES QUE HAY QUE LLEVAR PUESTOS

```text
   1. EL DESMAPEO ES CIEGO CON EL DMA
      Quitarle a la CPU su vista de una pagina no le quita la pagina a un
      aparato que ya tiene la direccion FISICA en su anillo de descriptores.
      Es exactamente el agujero que `docs/` de EL NEUTRO ya declara -- el
      celo del orquestador tampoco llega ahi. Un desmapeo protege del
      programa, no del aparato al que ese programa le pidio algo.

   2. HOY ES BARATO POR ACCIDENTE, Y DEJARA DE SERLO
      Con un solo nucleo corriendo, desmapear es `invlpg` local y ya. Con
      varios, cada desmapeo necesita un IPI a todo nucleo que pueda tener la
      traduccion en su TLB -- un TLB shootdown, que es coordinacion entre
      nucleos y no una instruccion.

      ** AXION ya apaga nucleos y todavia no los enciende (falta MWAIT). O
      sea que **esta deuda crece el dia que SMP funcione**, que es una
      funcion planificada. Cada `unmap_page` de hoy es un shootdown de
      manana, y hay siete sitios.
```

## 4. Que hacer con esto

Nada, todavia. Pero cambia el ORDEN de los pasos: la pagina de solo lectura
(M1) deja de ser *"la ganancia mas grande y la mas barata"* y pasa a ser **la
ganancia mas grande con un coste que hay que medir antes**. De ahi el M1b.

---

# 7. LOS PASOS

- [x] **M0 -- MEDIR, y no hacer nada mas.** HECHO el 2026-09-09 en el Ryzen:
  `FIJO 633 / TRABAJO 147`, o sea que el fijo es el 81% y **el lote es el
  proyecto correcto**. Ver la seccion 3b. Se verifica: `run c/ciclos.bex`.

- [ ] **M0b -- ★★★ EL PEAJE DEL PAPELEO, y va ANTES del lote porque es mas
  barato y mas grande.** El fijo son 633 y el cruce del silicio ~150: **483
  ticks son codigo nuestro**. `registrar_publicacion` corre en toda puerta,
  llama a `current_tid()` --que **cierra el planificador**-- y hace una lectura
  volatil de una linea de pila que la via rapida ya no escribe. Lo lee UN sitio:
  el informe de fallos.

  Lo que hay que hacer, en este orden:

  ```text
  1. `current_tid` y `current_pid` sin cerrojo. El `tid` de la tarea EN CURSO
     no lo cambia nadie mas: el que pregunta ES la tarea. Un cerrojo protege de
     una carrera que no existe -- pero eso hay que DEMOSTRARLO leyendo quien
     escribe `s.current`, no suponerlo
  2. `registrar_publicacion` detras de una bandera, como `metro_puerta`. Un
     diagnostico que se lee al caerse no tiene que cobrar en cada puerta
  3. medir otra vez con `ciclos.bex`. Si el fijo baja de 633 a ~200, la meta de
     300 se cumple SIN LOTE, con una sola operacion
  ```

  ** Y si sale que el cerrojo SI hace falta, se dice y se queda: la mitad de
  este paso es la pregunta, no el recorte. Se verifica: `ciclos.bex` imprime un
  fijo menor y el banco sigue en verde.

- [ ] **M0c -- los 112 ticks del `match` de `INFO`.** El rechazo por campo
  cuesta 112 mas que el rechazo por operacion, y la diferencia es recorrer un
  `match` de 100 campos. Lo paga TODA lectura de `INFO`, que es lo que hace la
  barra del escritorio sesenta veces por segundo. Se verifica: la fila 4 de
  `ciclos.bex` se acerca a la 3.

- [x] **M0-original -- el enunciado, conservado.** `c/ciclos.bex` ya esta escrito y
  compila: parte una puerta en FIJO y TRABAJO usando los dos rechazos, y
  proyecta la tabla del lote. **Hasta que ese numero salga del Ryzen, los pasos
  de abajo no se empiezan** -- si el trabajo resulta ser la mayoria, este plan
  entero es el proyecto equivocado. Se verifica: el programa imprime `fijo` y
  `trabajo de PID`, y la tabla marca donde cruza los 300.

- [ ] **M1b -- CUANTO CUESTA REVOCAR UNA PAGINA, y va ANTES de M1.** La seccion
  6b dice que la via de 4 ticks se paga en el desmapeo, y ese numero no existe.
  **Y se puede medir hoy, sin escribir kernel**: `bmo_pantalla_soltar()` seguido
  de `bmo_pantalla_abrir()` desmapea y vuelve a mapear el framebuffer entero --
  2.025 paginas a 1920x1080-- y DOOM ya tiene ese camino en su `F12`. Un
  `rdtsc` a cada lado y sale el coste por pagina.

  ** Con ese numero se sabe si una pagina publicada es revocable de verdad o si
  su revocacion es tan caro que en la practica no se revoca -- que seria peor
  que no publicarla. Se verifica: `ciclos.bex` gana una fila `soltar+abrir` con
  su coste por pagina.

- [ ] **M1 -- LA PAGINA DE SOLO LECTURA, la ganancia mas grande, con un coste
  que M1b tiene que decir primero.** Elegir los campos de `INFO` que son contadores puros y publicarlos.
  No todos: los que no pueden cambiar de forma. Se verifica: `ciclos.bex` gana
  una fila que lee el mismo dato de la pagina y de la puerta, y las dos dan el
  mismo valor con dos ordenes de magnitud de diferencia en coste.

- [ ] **M2 -- EL CONTRATO DEL LOTE, en papel y antes del codigo.** Que entra
  (un array de que estructura, con que alineacion, cuantas entradas como
  maximo), que sale (N status, donde), y que pasa si la entrada 7 falla: se
  para o se sigue. **Esa ultima pregunta es la que decide si el lote sirve para
  algo**, y contestarla mal despues de escribir el codigo es reescribirlo.

- [ ] **M3 -- `OP_LOTE`, y solo para operaciones SIN handle.** Las de
  `CURRENT_TASK`, que no caminan la tabla de capabilities. Es el subconjunto
  donde el trabajo es minimo y el fijo es todo, o sea donde el lote gana mas --
  y donde la validacion de Ring 0 es mas simple, porque no hay handles ajenos
  que resolver. Se verifica: `ciclos.bex` gana una fila con un lote de verdad y
  se compara con su propia proyeccion.

- [ ] **M4 -- el lote con handles, si M3 lo justifica.** Aqui la validacion es
  el trabajo de verdad: N handles, N generaciones, N derechos, todo antes de
  ejecutar el primero. No se empieza sin que M3 haya dado un numero.

---

# 8. LO QUE ESTE PLAN NO ES

```text
   [ ] no es "hacer los syscalls mas rapidos". Ni un ciclo del stub se toca
   [ ] no es un syscall nuevo. Los dos congelados siguen siendo dos
   [ ] no es para la entrada ni para el pixel: esos quieren lo contrario
       (ver PLAN_EL_PIXEL.md y PLAN_EL_COMPAS.md)
   [ ] y no empieza hasta que M0 diga que el fijo es la mayoria
```

> La puerta no se abarata. **Se reparte.** Y lo que decide si eso vale la pena
> es un numero que se mide en un arranque.
