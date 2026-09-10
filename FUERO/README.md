# FUERO -- lo que BMO-X CONCEDE y lo que EXIGE a cambio

> Creada el **2026-09-10**. La raiz tenia dieciocho ficheros sueltos y cuatro de
> ellos eran **una sola cosa contada en cuatro documentos**: la carta y sus tres
> leyes.

---

# 1. LA CUARTA FRONTERA

La raiz de BMO-X ya tenia tres carpetas en mayusculas, y **ninguna de las tres
es codigo**: son fronteras. Esta es la cuarta, y cierra la lista.

```text
   VALKYRIE-ABI/   la frontera de ARRIBA     la superficie que se promete
   PERFIL/         la frontera de ABAJO      lo que ESTA maquina da
   NEUTRO/         la frontera de AL LADO    quien alcanza la RAM sin obedecer
   FUERO/          la frontera con QUIEN     lo que se concede a quien
                   CONSTRUYE                 construye encima, y lo que se
                                             le pide a cambio
```

*** Las tres primeras son fronteras del sistema con **algo**. Esta es la
frontera del sistema con **alguien**, y por eso hacia falta: un desarrollador no
choca con el hardware ni con el DMA. Choca con lo que se le deja hacer.

---

# 2. LOS CUATRO, Y EN QUE ORDEN SE LEEN

| documento | que contesta |
|---|---|
| ★ [`EL_FUERO.md`](EL_FUERO.md) | **empieza por aqui.** Que te concede el sistema, que te exige de vuelta, y **que no te concede a proposito**. No es un SDK: es una carta |
| [`META-KERNEL_HARD.md`](META-KERNEL_HARD.md) | **la ley de la maquina.** Una regla existe solo si trae el componente que la exige y el numero que pide |
| [`META-APP_HARD.md`](META-APP_HARD.md) | **la ley de una app.** Que exige BMO-X de cualquier cosa que quiera serlo, y que le devuelve |
| [`META-SDK_HARD.md`](META-SDK_HARD.md) | **la ley de REX**: las cabeceras `<bmo/...>` con las que se escribe una app, y las dos pruebas que impiden que una libreria se vuelva un marco de trabajo |

** El orden no es de tamano: es de DEPENDENCIA. La carta dice que hay un trato;
las tres leyes dicen en que consiste, cada una para un lado de la puerta.

---

# 3. POR QUE SE MOVIERON, y por que estos cuatro y no siete

La raiz tenia siete documentos sueltos. Se midio cuantas citas de cada uno
llevan RUTA --las unicas que hay que reescribir al mover-- y cuantas van solo
por nombre:

```text
                        con ruta   solo nombre
   META-KERNEL_HARD.md        13            88
   META-SDK_HARD.md           15            17
   META-APP_HARD.md           12            21
   EL_FUERO.md                 7             8
   ---------------------------------------------
   BITACORA.md                 6            30     <- se QUEDA
   ARQUITECTURA.md             6            12     <- se QUEDA
   AVANCES.md                  3            14     <- se QUEDA
```

*** De 252 citas, solo **62 llevaban ruta**: el resto va por nombre y el
guardian de `enlaces` las resuelve esten donde esten. O sea que mover era barato
-- lo que no estaba claro era **que merecia moverse**.

## Los tres que se quedan arriba, y su motivo

```text
   ARQUITECTURA.md   que ES esto
   AVANCES.md        que hay HECHO
   BITACORA.md       que PASO, con cada fallo que costo un dia
```

** Esos tres son lo que un visitante lee **justo despues del README**, y no son
ley: son estado e historia. Bajarlos seria esconder el diario.

> Se agrupa lo que se lee JUNTO. No lo que empieza por la misma letra.

---

# 4. [!] LO QUE ESTA CARPETA SACRIFICA (L3)

```text
   1. `EL_FUERO.md` YA NO SE VE AL ABRIR EL REPO
      Era el fichero marcado como "empieza aqui si quieres construir sobre
      BMO-X" y estaba a la vista. Se acepta porque el README de la raiz sigue
      apuntandolo con estrella, y porque a cambio los cuatro se encuentran
      juntos -- que es como se leen

   2. 47 CITAS CON RUTA HUBO QUE RECALCULARLAS
      Una a una, con la relativa nueva. Las cazo el guardian de enlaces: doce
      quedaron rotas al primer intento --las que salen DESDE estos ficheros
      hacia el resto del arbol, que bajaron un nivel-- y dos mas despues

   3. Y SI ALGUN DIA HAY UNA QUINTA LEY, ESTA CARPETA CRECE SIN AVISAR
      Hoy son tres leyes porque hay tres lados: la maquina, la app y REX. Una
      cuarta ley que no encaje en esos tres lados es una senal de que la
      division esta mal, no de que falte un fichero
```
