# PLAN EL PLAZO -- V-Sync, VBlank y la deuda de planificacion

> Escrito el **2026-09-08**, al final de la caza del fotograma que dependia del
> teclado. Lo pidio el dueno con estas palabras:
>
> > *"el V-Sync y VBlank y la deuda es para eso, para planificacion, por eso el
> > orquestador por algo orquesta. Pon ese plan para aplicar luego."*
>
> Y la frase de arriba es el plan entero en una linea: **un orquestador sin
> plazo no orquesta, reparte.** Lo que falta no son fotogramas por segundo: es
> un numero contra el que decidir.

---

# 0. LA FRASE QUE ORDENA TODO

```text
   un motor grafico no pregunta "cuanto puedo dibujar".
   pregunta "cuanto me CABE antes del siguiente barrido".
```

Hoy BMO-X no puede hacer la segunda pregunta, y por eso el compositor va montado
en un ritmo **elegido a mano** --el latido de 1 kHz, ver
[`bmo-wait-latido`](../../Ultra_userspace/services/director/src/desktop/tick.rs)--
en vez de en un plazo medido. Eso funciona y no es lo mismo.

## ⚠ Y LA PRIMERA LINEA DEL PLAN ES UN NO

**BMO-X no puede tener un VBlank de verdad hoy, y ninguna cantidad de trabajo en
Ring 0 lo cambia.** Hay que decirlo antes de disenar encima:

```text
   el framebuffer viene del GOP de UEFI, y tras ExitBootServices el GOP NO EXISTE
   -> queda un bufer lineal, y un bufer lineal no dice por donde va el escaner
   la posicion del haz vive en los registros del CONTROLADOR DE PANTALLA
   -> o sea en la GPU, o sea detras de un driver de pantalla
```

*** Asi que **V-Sync real es una consecuencia del driver de pantalla, no una
tarea aparte**, y ese driver vive en [`NEUTRO/`](../../NEUTRO/README.md) por la
misma razon que la GPU: ejecuta codigo que esta casa no escribio. El escalon 8 de
[`LA_RAM.md`](../identidad/LA_RAM.md) ya lo dice del *page flip*: mismo
bloqueante, misma puerta.

> Lo que este plan puede dar ANTES de esa puerta es un **plazo declarado con
> juez**, que es lo que convierte el latido de heuristica en contrato.

---

# 1. LOS DOS BLOQUES, Y SOLO UNO DEPENDE DE LA GPU

| # | bloque | depende de la GPU? | que desbloquea |
|---|---|---|---|
| **P1** | el plazo DECLARADO y su juez | no | que "va lento" sea un numero |
| **P2** | la deuda de planificacion | no | que el compositor RECIBA el turno |
| **P3** | el VBlank de verdad | ⚠ si | V-Sync, page flip, cero desgarro |

** El orden no es negociable y no es de gusto: **P2 antes que P1.** Un plazo que
se mide mientras al medidor no le dan turnos mide el planificador, no el
compositor. La sesion del 08-09 lo demostro con dos numeros --`cuerpo 1` contra
`puerta 33750`-- en `bmo-planificador-suelo`.

---

# 2. P2 -- LA DEUDA DE PLANIFICACION (se puede hacer YA)

El fantasma del 08-09, en el idioma del tiempo real: no era un problema de
prioridad --la asignacion cumple **Rate Monotonic**, el bus tiene el periodo mas
corto y va arriba-- era un problema de **utilizacion**.

```text
   T = 4 ms         el periodo del hilo del bus (250 latidos/s)
   C = hasta 4 ms   porque `park_until` se quedaba el CPU HALTADO hasta que
                    expiraba su quantum
   U = C/T = 100 %  y con eso todo lo de abajo es INPLANIFICABLE. No lento:
                    imposible, y es un teorema
```

## Escalones

- [X] **P2.1 -- un quantum es de quien CORRE.** `on_timer` daba el resto del
      quantum a `s.current` sin mirar si seguia `Running`. HECHO el **2026-09-08**
      en `scheduler/roja.rs` (commit `cd9f5bf3`), **sin medir en metal**. Baja `C`
      de 4 ms a 1 ms.

- [ ] **P2.2 -- RESCHEDULE FORZADO: una tarea que se duerme suelta el CPU en el
      acto.** Con P2.1 aun se retiene hasta un tic: `park_until` hace `hlt` y
      espera al reloj. Dos parkers a 250 Hz --el hilo del bus y `descansar()` de
      `core/shell/ui.rs`-- son hasta **500 ms de cada segundo quemados
      HALTANDO**. La tecnica tiene nombre: **interrupcion software para forzar el
      cambio**, porque el cambio de contexto solo se consuma en el epilogo del
      trap (ver la nota de `park_until` en `scheduler/roja.rs`).
      ⚠ Cuidado: el vector 48 del LAPIC hace `TICKS.fetch_add` y un EOI. Un
      trap propio para ceder no puede reusarlo sin mentir en el contador.

- [ ] **P2.3 -- el kernel publica el TIEMPO DE CPU de una tarea.** Hoy
      `Tick::cuerpo_ms` mide **reloj de pared**, y su propia cabecera en
      `desktop/tick.rs` lo avisa: 82 us por vuelta de los que solo 2,36 eran
      trabajo. Sin esto, "el compositor trabaja mucho" y "al compositor no le dan
      turno" **se leen igual**. Es un campo de `OP_INFO` y un contador en el
      cambio de contexto.

- [ ] **P2.4 -- envejecimiento en `choose_next`, y SOLO si P2.1+P2.2 no bastan.**
      La cura de libro de la inanicion, y no cambia la intencion de nadie.
      ⚠ La alternativa --subir el compositor a prioridad 1-- **empeora**: mataria
      de hambre a las apps de Ring 3, que nacen en 0 (`admitir.rs:963`), DOOM
      incluido. El razonamiento entero esta en `scheduler/verde.rs`, junto a
      `QUANTUM_DELANTE`.

---

# 3. P1 -- EL PLAZO DECLARADO, Y SU JUEZ

Sin VBlank no hay plazo medido. Pero **si** hay plazo declarado, y esta casa ya
sabe que hacer con un dato declarado: se escribe, se expone y un guardian no deja
que se separe del codigo. Es lo que hace [`PERFIL/`](../../PERFIL/README.md).

- [ ] **P1.1 -- `PERFIL/PANTALLA.txt`.** Resolucion, formato, stride y
      `hz_declarado`, con su seccion `SI ESTE PERFIL ESTA MAL, ASI SE NOTA` y su
      `expone:`. Hoy la frecuencia **no se puede preguntar**: el GOP no la trae.
      Por eso se PERFILA -- es el criterio literal de `PERFIL/README.md` seccion
      0b: *"lo que no se puede preguntar se supone, y entonces se escribe"*.

- [ ] **P1.2 -- el PRESUPUESTO del fotograma, en el compositor.** `16,7 ms` a 60
      Hz sale de `hz_declarado`, no de una constante suelta. Y el numero contra
      el que compararlo ya existe medido: **volcar la pantalla entera son 27,6 ms**
      (`bmo-compositor-escaner`), o sea que a pantalla completa el plazo ya se
      incumple **por 11 ms** y eso hoy no lo dice nadie.

- [ ] **P1.3 -- el juez: `pinta` pasa a decir CUANTAS LLEGARON A TIEMPO.**
      `scene/pulso/amarilla.rs` ya cuenta las vueltas que pintaron (decision 5);
      falta partir esa cuenta en dos contra el presupuesto de P1.2. Un fotograma
      que tarda 30 ms no es "un fotograma": es un plazo incumplido, y el
      instrumento tiene que decirlo -- misma regla que hizo falta para la aguja.

- [ ] **P1.4 -- el orquestador USA el plazo.** Aqui es donde deja de repartir y
      empieza a orquestar: si lo que queda del presupuesto no da para componer
      una superficie mas, **no se compone y se dice**. La doctrina ya esta escrita
      en `bmo-orquestal` --*"el foco decide CUANTO, no QUIEN"*-- y `QUANTUM_DELANTE`
      es su primera mitad. Esta es la segunda.

---

# 4. P3 -- EL VBLANK DE VERDAD (detras de la puerta de NEUTRO)

- [ ] **P3.1 -- leer la posicion del haz.** Registro del controlador de pantalla.
      Exige el driver, o sea `platform/drivers/gpu/` y su bring-up por
      `psp.rs`. Sin eso no hay conversacion.
- [ ] **P3.2 -- la interrupcion de VBlank a Ring 3.** Y esto **ya tiene el suelo
      puesto**: es `KIND_LATIDO` con otra fuente. Lo dice su propia cabecera en
      `obj/latido.rs`: *"el dia que la fuente sea la IRQ de una tarjeta, este
      brazo no cambia: cambia quien llama a `tic()`"*. Escrito antes de que
      hiciera falta, y por eso este escalon es pequeno.
- [ ] **P3.3 -- page flip en vez de copiar.** Escalon 8 de `LA_RAM.md`. Mata los
      27,6 ms de `Pantalla::volcar`, que hoy es *"trabajo de la GPU hecho por
      quien no toca"* -- lo dice el propio fichero.

---

# 5. LO QUE ESTE PLAN NO PROMETE

```text
   [ ] no da 60 fps: da un PLAZO y un juez que diga si se cumplio
   [ ] no quita el desgarro sin P3 -- eso es page flip, no ritmo
   [ ] no hace tiempo real DURO: sin plazo medido no hay garantia, solo
       presupuesto. La palabra honesta es SOFT REAL TIME hasta P3
   [ ] y no toca `choose_next` mientras P2.1 y P2.2 no esten medidos: el
       fantasma del 08-09 se cazo por medir, no por cambiar prioridades
```

> Un plazo que nadie juzga es un comentario. Un ritmo sin plazo es una
> costumbre. **Orquestar es tener las dos cosas y decir NO con la segunda.**
