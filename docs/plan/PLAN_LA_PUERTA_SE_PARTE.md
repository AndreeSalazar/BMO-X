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

Si esos numeros aguantan --y hay que volver a medirlos, porque el 870 era el
`CONSOLE_READ` de la fila equivocada y el 784 es de antes de que se fueran el
XSAVE y el `iretq`-- la cuenta sale asi:

```text
   FIJO      ~784        el 90 %
   TRABAJO   ~86         el 10 %
```

Y entonces:

```text
     N    ticks/op    (FIJO/N + TRABAJO)
     1        870
     2        478
     4        282     <== bajo la meta de 300
     8        184
    16        135
    32        110
```

** Con **cuatro operaciones por puerta se cumple la meta de 300**, y con 32 se
llega a 110 -- un octavo de lo que cuesta hoy. Sin tocar un solo ciclo del
ensamblador del stub.

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

# 7. LOS PASOS

- [ ] **M0 -- MEDIR, y no hacer nada mas.** `c/ciclos.bex` ya esta escrito y
  compila: parte una puerta en FIJO y TRABAJO usando los dos rechazos, y
  proyecta la tabla del lote. **Hasta que ese numero salga del Ryzen, los pasos
  de abajo no se empiezan** -- si el trabajo resulta ser la mayoria, este plan
  entero es el proyecto equivocado. Se verifica: el programa imprime `fijo` y
  `trabajo de PID`, y la tabla marca donde cruza los 300.

- [ ] **M1 -- LA PAGINA DE SOLO LECTURA, que es la ganancia mas grande y la mas
  barata.** Elegir los campos de `INFO` que son contadores puros y publicarlos.
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
