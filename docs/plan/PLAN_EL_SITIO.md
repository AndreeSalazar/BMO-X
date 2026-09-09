# PLAN EL SITIO -- donde vive un valor, decidido UNA vez para todos

> Propuesta del dueno, **2026-09-09**, y la trajo mirando la jerarquia de caches:
>
> > *"pensaba que INTI mas cerca del registro, esa jerarquia, para que INTI
> > salte y asi facilitar. Podemos preparar la propuesta, pero ya no seria
> > compilador sino que ese es LIBRERIA para x86-64, basicamente como que es
> > intermedio para procesar en vez de demorar en RAM."*
>
> [!] **Esto es una propuesta y no toca nada todavia.** Lo que hay aqui es el
> mapa medido, el hueco que se ve al dibujarlo, y lo que costaria.

---

# 1. ★★★ EL NUMERO QUE ABRE LA PROPUESTA

Se conto cuantos sitios del arbol escriben bytes de x86-64 a mano, y cuantos lo
piden a la libreria que existe para eso (`sem-asm`):

```text
   emisor                     a mano    por sem-asm
   ------------------------------------------------
   lang/c                        321          18
   lang/cobol                    105          33
   lang/ada                       16           0
   inti/emisor-x86_64             51           2
   forge/bmo-lower               176           0
   ------------------------------------------------
   TOTAL                         669          53      -> el 7,3 %
```

★★ Y el ultimo de la lista es el que duele: **`bmo-lower` es la capa GENERICA
del pipeline** --su propia cabecera se llama a si misma *"L1: el descenso
generico"*-- y escribe 176 secuencias a mano sin pedirle ni una a `sem-asm`.

## ⚠ Y `sem-asm` lleva escrito para que existe desde el dia que nacio

Su primera linea, sin tocar:

> *"Reemplaza el hardcodeo de bytes DUPLICADO en `lang/c/src/codegen.rs` y
> `lang/cobol/src/codegen.rs` (ambos escriben 0x48/0xB8... a mano)."*

**Van ocho.** `constante_de` estaba completo y el emisor no preguntaba;
`ritmo()` se media y solo lo leia Ring 0; `FB_OP_BYTES` decia *"para un
`rep stosd`"* y nadie lo hacia. Esta es la misma frase otra vez:

> La pieza existe, dice para que existe, y quien la necesita no la usa.

---

# 2. PERO LO QUE PROPONE EL DUENO NO ES `sem-asm`

Y esa distincion es toda la propuesta. `sem-asm` traduce **un mnemonico a
bytes**: le dices `mov rax, 8` y te da `48 C7 C0 08 00 00 00`. Es una tabla.

Lo que falta es la pregunta de **antes**: *"este valor, donde vive?"*.

```text
   L2  ESPECIALIZADA    la semantica de cada lenguaje       existe, una por uno
   ??  EL HUECO         donde vive un VALOR: registro o     NO EXISTE
                        pila; cuando muere; con quien
                        comparte sitio
   L1  GENERICA         `bmo-lower`: el descenso al ABI     existe
   L0  SUPERFICIE       INVOKE / WAIT                       congelada
```

★ El diagrama de arriba es **el del propio `bmo-lower`**, con una fila anadida.
El hueco estaba dibujado y sin nombre.

---

# 3. POR QUE ESTO ES "MAS CERCA DEL REGISTRO"

La jerarquia que el dueno nombro, con el nivel que se le olvida a todo el mundo
puesto arriba:

```text
   registros    0 ciclos      son el operando. No hay viaje
   L1d         ~4-5 ciclos    32 KiB por nucleo    (cache.rs lo sabe)
   L2         ~13 ciclos      512 KiB por nucleo
   L3         ~46 ciclos      32 MiB, un solo CCX
   RAM       ~300+ ciclos
```

[!] Esas latencias son las **publicadas** de Zen 3, no medidas aqui. LEY 24 dice
que una cifra generica es de otro proyecto, y `bmo::ciclos()` mas cinco bucles
darian las de esta maquina.

*** **BMO C nunca usa el nivel 0.** Es una maquina de pila: cada variable vive
en su hueco de `%rbp` y cada operacion viaja a memoria, aunque sea a L1. Los
`push`/`pop` que se quitaron el 09-09 eran exactamente eso.

Y no se puede "elegir L1 en vez de L2" --eso lo decide el silicio--. Lo unico
que se decide es **si el valor baja a la escalera o no**. Eso es esta libreria.

---

# 4. ★★★ LA CLAVE NO ES EL REGISTRO: ES EL ALIAS

Por que un compilador manda a memoria una variable que cabria en un registro?
Porque **no puede probar que un puntero no la pisa**. Con un `*p = x` en medio
tiene que asumir lo peor y volver a leerla.

```text
   adivinar el TIPO    -> emite `div` donde iba `idiv`      C5 de PLAN_EL_CODEGEN
   adivinar el ALIAS   -> tiene que escribir a memoria      ESTE plan
```

★★ **Las dos son la misma palabra del dueno: adivinar.** Y la segunda es
justamente la que aleja del registro.

Una libreria del sitio no adivina: **recibe lo que el lenguaje declara**.

```text
   entra   los valores, sus vidas, y que punteros NO se solapan
   sale    este en `rax`, este en `rdx`, este a la pila y por que
```

Y por eso el cliente natural es **INTI**: tiene cero UB comprobado en el Ryzen y
puede declarar en la gramatica lo que C solo puede deducir. Un lenguaje nuevo
puede exigir lo que un lenguaje de 1972 tiene que suponer.

---

# 5. ⚠ LA CORRECCION AL PLANTEAMIENTO, Y ES IMPORTANTE

El dueno lo dijo como *"INTI ya no seria compilador sino libreria"*. No:

```text
   INTI el LENGUAJE          se queda. Es "el C de BMO-X" y tiene su gramatica
   inti/emisor-x86_64        se queda. Depende de `bmo-inti-front`, o sea que
                             esta acoplado al arbol de INTI: no es generico
   la libreria del SITIO     es NUEVA, vive en `forge/`, y no sale de INTI --
                             INTI es su primer CLIENTE, que no es lo mismo
```

Sacar el emisor de INTI y llamarlo libreria seria heredar su acoplamiento al
AST de INTI, y entonces C y COBOL no podrian usarla. El sitio de un valor no
depende del lenguaje: **por eso puede ser una libreria, y por eso tiene que
nacer sin dueno.**

---

# 6. ★★ Y ESTO RESUELVE LA OBJECION QUE YO MISMO ESCRIBI CONTRA `C4`

En [`PLAN_EL_CODEGEN`](PLAN_EL_CODEGEN.md), el escalon `C4` --mantener en
registro la variable de un bucle-- lleva escrito este sacrificio:

> ⚠ *"es donde un compilador deja de poder leerse de una sentada, que es la
> propiedad por la que existe este."*

*** Con la libreria **esa objecion desaparece**: la complejidad del analisis de
vivos vive FUERA, en una crate con su propio banco, y BMO C sigue cabiendo en
una tarde de lectura. El dueno resolvio sin saberlo la pega que bloqueaba `C4`.

Y hay un segundo premio: **una asignacion de registros escrita SEIS veces no se
escribe nunca.** Hoy hay seis sitios que emiten x86-64. Escrita una vez, la
heredan los cinco restantes el dia que la enlacen.

---

# 7. LA ESCALERA

- [ ] **S1 -- EL CONTRATO, EN PAPEL Y ANTES QUE EL CODIGO.** Que entra y que
      sale. Sin un lenguaje dentro: valores, vidas, alias, y la tabla de
      registros de la arquitectura.
      ★ Si el contrato no se puede escribir sin nombrar C ni INTI, es que no
      era una libreria.

- [ ] **S2 -- LA TABLA DE REGISTROS COMO DATO, no como codigo.** x86-64 nombra
      16; RISC-V nombra 32. Cual esta reservado, cual lo pisa una llamada.
      ★★ Y aqui esta el regalo para [`PLAN_EL_GUARDIAN`](PLAN_EL_GUARDIAN.md):
      el backend de RISC-V (G1.2) deja de ser "otro emisor" y pasa a ser **otra
      tabla**. Es la apuesta de esta casa --*tablas y no cerebros*-- aplicada al
      sitio donde mas se nota.

- [ ] **S3 -- EL PRIMER CLIENTE: INTI.** Porque puede declarar el alias. Y
      porque si se estrena en C hay que arrastrar 321 sitios a mano el primer
      dia.

- [ ] **S4 -- BMO C, y solo el bucle caliente.** No los 321 sitios: los de
      `emitir/valor.rs`, que es donde estan los 157 ciclos por escritura.

- [ ] **S5 -- `bmo-lower` deja de escribir bytes a mano.** Los 176. Es la capa
      generica: que la generica no use las librerias genericas es la deuda mas
      barata de pagar y la mas fea de tener.

---

# 8. EL NOMBRE

`bmo-sitio` -- *"el sitio de cada valor"*. Corto, literal, y dice lo que hace
sin prometer lo que no.

[!] Pero el nombre lo pone el dueno. `CUPO` se rechazo por lo que recordaba en
Peru, y esa clase de cosa no la puede saber quien escribe el plan.

---

# 9. LO QUE ESTE PLAN NO PROMETE

```text
   [ ] no promete "todo en registros". x86-64 NOMBRA 16, y el fichero fisico
       de ~180 no se puede direccionar. Lo alcanzable es el bucle caliente
   [ ] no promete un numero. Del 09-09 quedan 28 instrucciones en el bucle de
       la expansion y DOS accesos a la pila: puede que ya no den los 157
       ciclos. **Eso lo dice el metal y todavia no ha hablado**
   [ ] no es un compilador optimizador. Es UNA decision --donde vive un valor--
       sacada a su sitio. El resto de `PLAN_EL_CODEGEN` sigue donde estaba
   [ ] y no empieza sin S1. Una libreria que nace pegada a un lenguaje se
       queda pegada, y entonces son seis emisores y una excusa
```

> Hoy hay seis sitios que escriben x86-64 y una libreria de codificacion que
> usa el 7% de ellos. **El problema no es que falte una capa: es que las que hay
> no se usan.** Este plan solo vale si la nueva nace con clientes, y por eso el
> primer escalon es un contrato y no un `cargo new`.
