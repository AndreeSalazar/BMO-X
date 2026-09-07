# QUE TECLEAR EN EL RYZEN -- tanda del 2026-09-07

> La regla de esta carpeta, de la primera hoja y sin cambios:
>
> *"Cada prueba dice **que afirma** y **como se cae**. Una prueba que solo puede
> salir bien no prueba nada -- si no se sabe de antemano que aspecto tiene el
> fallo, cualquier cosa que aparezca en pantalla se lee como exito."*
>
> Y la de orden: **lo que no toca nada va primero, lo que no se deshace va al
> final.**

---

# 0. LO QUE HAY QUE SABER ANTES DE ARRANCAR

**Esta tanda no es de un commit: son 130 que nadie ha ejecutado**, desde el
2026-08-26. La hoja anterior es la del 25-08.

Y a diferencia de las otras, **esta empieza con un fallo YA REPRODUCIDO**. No es
una lista de estrenos: es una caceria con un sospechoso nombrado por el propio
instrumento del kernel.

---

# 1. ★★★ LA PURGA -- el fallo que ya tiene receta y veredicto

## 1.1 La receta, dicha por el dueno

```text
   1.  entrar al escritorio
   2.  jugar a DOOM
   3.  cerrarlo con Ctrl+Alt+Esc            <- LA PURGA
   4.  volver a lanzar DOOM (o `ray`)
   5.  PANTALLA AZUL
```

**Reproducible.** Y el paso 4 importa: la azul **no sale al purgar**, sale al
volver a lanzar algo despues. Lo que la purga rompe no se nota hasta que
alguien vuelve a pedir memoria.

## 1.2 Lo que dijo la azul, campo por campo

```text
   #PF  vec=0x0E  err=0x00000002
   no-presente  ESCRIBIENDO  desde el KERNEL
   rip=0x0000000000000000    <- CERO NO ES UNA DIRECCION: no se pudo leer
   cr2=0x00000008FFFFFFFF
   rsp=0xFFFF800000B8FC50  pila de HILO DEL KERNEL -- de NADIE VIVO
                           marco OCUPADO  (morgue: 02...)
     y NINGUNA tabla reclama ese marco: contabilidad rota
   corria tid=05 (Ring 3)
   ticks=0000C3AB
   iq: en rsp no hay marco de iretq (cs=0x0000). El fallo no es de un
       cambio de contexto
```

## 1.3 ** EL VEREDICTO, Y NO LO PONE NADIE: LO PONE EL INSTRUMENTO

`plat/faults/amarilla.rs` tiene escrita la tabla que interpreta ese bit, y la
azul cayo en la rama que la usa:

```text
   marco LIBRE     no lo tiene nadie      ->  uso despues de liberar
   marco OCUPADO   alguien lo tiene AHORA ->  SE ENTREGO DOS VECES
```

**La foto dice OCUPADO.** Asi que el veredicto del kernel, con sus palabras, es
**doble entrega del asignador** -- y NO es un uso-despues-de-liberar. Son dos
fallos opuestos y este bit existe justo para separarlos.

Y las otras dos lineas lo estrechan mas:

| lo que dice | lo que descarta |
|---|---|
| `de NADIE VIVO` | ninguna tarea viva reclama esa pila |
| la morgue **NO la reconoce** (cayo en la rama `else`) | no la solto `reap`, **o** se fue por el anillo de ocho fichas |
| `NINGUNA tabla reclama ese marco` | no es un dueno que falta: es **contabilidad rota** |
| `iq: no hay marco de iretq` | **no** es un cambio de contexto |
| `rip=0x0` | se salto a CERO -- o sea, un `ret` sobre una pila con basura |

## 1.3b ⭐ Y DESDE EL 07-09 LA AZUL CONTESTA UNA COSA MAS

El `else` de `phys::free_frame` cazaba un marco devuelto dos veces desde el
01-09 **y solo lo decia en CABINA**, que muere con la azul. Ahora lo apunta en
un libro de ocho fichas que la azul consulta, asi que si vuelve a pasar el
renglon dira:

```text
   marco OCUPADO -- Y SE DEVOLVIO DOS VECES en tick XXXXXXXX
```

** Si sale ese trozo, **el caso esta cerrado**: hay un doble `free`, y el tick
dice cuando. Si NO sale, tambien es una respuesta y de las caras -- significa
que el marco se entrego dos veces **sin que nadie lo devolviera dos veces**, y
entonces el fallo esta en el camino de ENTREGA, no en el de devolucion.

Es la misma jugada que la morgue: dos respuestas, las dos utiles.

---

★ **La cadena entera, en una frase**: la purga libera marcos; algo se libera dos
veces; el asignador entrega el mismo marco al siguiente que pide (DOOM); ese
marco lleva dentro la pila de un hilo que todavia esta sentado en ella; el
hilo hace `ret` sobre basura, salta a cero, y el `#PF` mata la maquina.

## 1.4 QUE TECLEAR, y en este orden

```text
   [ ] 1.4a  entrar, NO lanzar nada, `mem`         apuntar marcos libres
   [ ] 1.4b  lanzar DOOM, salir con Ctrl+Alt+Esc   leer EL PARTE de la purga
   [ ] 1.4c  `mem` otra vez                        comparar con 1.4a
   [ ] 1.4d  lanzar `ray` (no DOOM: es mas chico)  aqui es donde revienta
   [ ] 1.4e  si NO revienta, repetir 1.4b-1.4d     hasta tres veces
```

### El parte de la purga: los seis numeros que hay que copiar

`core/purga.rs` los imprime en cuatro renglones. **Estos son los que deciden:**

```text
   tareas          cuantas se cerraron
   marcos antes / despues
   vueltos         marcos que volvieron   <-- si es MENOR que lo que se fue, ahi esta la fuga
   ranuras antes / despues
   vueltas         cuantas cesiones hizo falta   <-- si sale 8, se agoto el plazo
   completa        si / no                        <-- si dice NO, quedo algo vivo
```

** Y la pregunta que contestan: **`vueltos` mayor que cero y `completa=si` con
la maquina rota despues** significa que el fallo no es que falte limpiar, sino
que se limpio **de mas** -- que es exactamente lo que dice `marco OCUPADO`.

---

# 2. ⚠ LOS DOS DEFECTOS QUE YA SE ENCONTRARON LEYENDO, SIN ARRANCAR

No estan arreglados a proposito: tocar la purga o `reap` es ROJO, y el dueno
pidio **mecanismos, no parches**. Se apuntan aqui para que la tanda los mire
con los ojos puestos.

## 2.1 `queda_alguna_de_ring3` lee la tabla SIN CERROJO

`task/scheduler/verde.rs`:

```rust
   pub fn queda_alguna_de_ring3() -> bool {
       let s = unsafe { &*core::ptr::addr_of!(SCHEDULER) };   // <-- sin lock
       s.tasks.iter().any(|t| t.is_user && t.state != TaskState::Empty)
   }
```

Sus DOS vecinas inmediatas, en el mismo fichero, si lo toman:

```rust
   pub fn hay_hueco()      { let _g = SCHED_LOCK.lock(); ... }
   pub fn huecos_libres()  { let _g = SCHED_LOCK.lock(); ... }
```

** Y la llama el bucle de la purga **entre cesiones**, que es exactamente cuando
`reap` esta reescribiendo esa tabla. Lee mientras otro escribe.

**Como se ve si es esto**: `vueltas` alto o `completa=no` sin que quede nada
vivo de verdad -- o sea, el bucle vio un estado a medias.

## 2.2 El tope de OCHO vueltas puede tapar el sintoma

`VUELTAS_MAX = 8` en `core/purga.rs`, y con motivo escrito: quedarse dando
vueltas ahi deja la maquina sin la unica tecla que la salva.

Pero si se agota, `completa=false` y **la purga se va dejando algo vivo sin que
nadie lo persiga**.

**Como se ve si es esto**: `vueltas = 8` en el parte.

## 2.3 Lo que se DESCARTO leyendo, para no perseguirlo

| descartado | por que |
|---|---|
| que `reap` recoja una tarea por vuelta | recorre la tabla ENTERA (`for i in 0..len`) |
| que `reap` no libere la ranura | pone `self.tasks[i] = Task::EMPTY` |
| que el supervisor relance en medio | corre en `run_shell`, UNA vez, no en bucle |

---

# 3. LO QUE SE ACUMULO DESDE EL 26-08, POR GRUPOS

## 3.1 Ring 0 -- lo que puede dejarte sin maquina

```text
   [ ] LA PURGA + LA REGLA 7    seccion 1. Va primero por eso
   [ ] LA MORGUE                `de NADIE VIVO` -> dice de quien FUE
   [ ] las 17 estaciones         la azul dice el MOMENTO, no solo el marco
   [ ] el asignador con dueno    "ese marco NO es tuyo"
   [ ] el marco 4D2000           tenia codigo dentro y seguia enlazado
```

**Como se ve si va bien**: arranca y el escritorio aparece.
**Como falla**: pantalla negra, o azul en el primer lanzamiento.

## 3.2 La orquesta -- 12 nucleos desde Ring 3

```text
   [ ] `smp orquesta`      NUNCA se ha ejecutado. El lado del kernel
                           midio 11,52x sobre un nucleo
```

**Bien**: reparte y dice el factor. **Mal**: se cuelga, o dice 1 de 12.

## 3.3 El escritorio

```text
   [ ] Alt+F4              cierra lo de delante SEA LO QUE SEA, y lo dice
                           cuando no hay nada que cerrar
   [ ] el puntero          nace en el CENTRO, no en la esquina
   [ ] F1 -> ESTRUCTURA    ★ YA VERIFICADO el 06-09, con foto
```

## 3.4 La red

```text
   [ ] `red rx`            las tres causas del 28-08, arregladas y sin ejecutar
                           (el sondeo, la foto cacheada, y `MAR` a ceros)
```

**Bien**: `cogidas` sube. **Mal, y distinto**: `cogidas 0 / perdidas 0` = la MAC
no acepto ninguna; `cogidas 0 / perdidas subiendo` = el anillo no las recoge.

## 3.5 El compilador de C -- lo que arreglo la semana del 01 al 04

```text
   [ ] `ray.bex` ensena el 3D    la regresion de `89dfb77f`
   [ ] DOOM pasa del primer fotograma
```

Los cinco fallos del codegen (`*p = x` de ocho bytes, el 32 bits que no
envolvia) se probaron en el anfitrion. **En metal, no.**

---

# 4. EL ORDEN DE LA TANDA

**No es el orden de esta hoja.** Aqui estan por lo que son; abajo por lo que
cuestan si salen mal:

```text
   1.  arrancar y mirar el escritorio        si esto falla, nada mas importa
   2.  `mem`, `cpu`, `info`                  no tocan nada
   3.  F1                                    ya verificado, confirma que sigue
   4.  Alt+F4 sobre una ventana
   5.  `red rx`                              no escribe en ningun sitio
   6.  `smp orquesta`                        primera ejecucion de verdad
   7.  ★ LA PURGA, seccion 1.4               reproduce, y APUNTA LOS NUMEROS
   8.  DOOM y `ray`                          lo que revienta despues de 7
```

★ **La 7 va casi al final a proposito**: se sabe que rompe la maquina, asi que
todo lo que se pueda mirar antes hay que mirarlo antes. Una azul en el paso 3
se lleva por delante los datos de los pasos 4, 5 y 6.

---

# 5. LO QUE ESTA HOJA NO CUBRE

* **El audio** (A1, el tubo). Sigue de la hoja del 25-08 y nadie lo ha tocado.
* **La calculadora con botones**. Igual.
* **El contador de puertas** (`INFO_SYSCALL_CUENTA`) para confirmar la cuenta de
  `PLAN_ESTRUCTURA.md` seccion 4b. Es una fila mas de `info`, y no existe
  todavia -- el numero de esa seccion sigue siendo una cuenta del codigo.

---

Ver [`PLAN_LA_PILA_HUERFANA.md`](../plan/PLAN_LA_PILA_HUERFANA.md) (los cinco
escalones que predijeron esto),
[`EL_AISLAMIENTO.md`](../identidad/EL_AISLAMIENTO.md) seccion 4.2 (lo que
ninguno de los ocho muros para, y la fila del bug del kernel) y
[`BITACORA.md`](../../BITACORA.md) (los episodios, para comparar la azul con
las anteriores).
