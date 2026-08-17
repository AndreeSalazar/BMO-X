# AUTOCURACION MAESTRO -- perfilar el FALLO como se perfila el CPU

> Escrito el **2026-08-08**, el dia que el kernel empezo a redactar la autopsia
> de cada tarea que mata.
>
> Es el documento conceptual, no la lista de tareas: eso esta en
> [`PLAN_AUTOCURACION.md`](../plan/PLAN_AUTOCURACION.md). Este dice **que se copia de lo
> que ya existe en el mundo, que seria un ERROR copiar, y por que la
> arquitectura de BMO-X cambia la respuesta.**
>
> Mismo metodo que [`SMP_MAESTRO.md`](SMP_MAESTRO.md): antes de escribir codigo,
> separar la mitad que hay que copiar de la mitad que seria un error.

---

# ★ 1. QUE ES "AUTO-CURACION" DE VERDAD, Y QUE NO

La palabra se usa para tres cosas distintas y solo una vale.

| Lo que se llama auto-curacion | Que hace de verdad | Sirve? |
|---|---|---|
| **Watchdog que reinicia** | vuelve a arrancar la maquina cuando se cuelga | ⛔ **tapa el fallo**: el bug sigue, y ahora sin rastro |
| **Reintentar en bucle** | repite la operacion que fallo | ⛔ si el fallo es determinista, es un bucle; si no, esconde la causa |
| **Contener + restaurar + INFORMAR** | aisla el dano, devuelve el servicio, y deja escrito que paso | ✅ es lo unico que deja el sistema mejor que antes |

★ **La linea que las separa**: una cura que no deja constancia no es una cura, es
un encubrimiento. Un sistema que se reinicia solo y no dice por que **parece mas
sano que uno que se queja, y esta peor** -- porque el que se queja se puede
arreglar.

Es la misma ley que el resto del proyecto: *lo que no se dice, no ocurrio.*

---

# ★★ 2. LO QUE SE COPIA: EL MODELO DE ERLANG

Erlang/OTP es de lo poco que funciono de verdad en esto, y lleva treinta anos
corriendo centrales telefonicas con nueve nueves. Su idea es **let it crash**:

1. Un proceso que encuentra algo que no esperaba **no intenta arreglarlo**: muere.
2. Un **supervisor** lo vigila, y cuando muere aplica una politica: relanzar,
   relanzar a los hermanos tambien, o rendirse hacia arriba.
3. El estado que necesita sobrevivir vive **fuera** del proceso que puede morir.

Las tres encajan con lo que BMO-X ya es. La primera es literalmente lo que hace
`fault_dispatch`: la tarea muere y la maquina sigue.

## ★ Y aqui BMO-X tiene una ventaja sobre Erlang, y no es marketing

En Erlang un proceso reiniciado **puede heredar estado**: el supervisor le pasa
argumentos, hay tablas ETS globales, hay un sistema de ficheros debajo. El
aislamiento es por convencion del runtime.

En BMO-X **las capabilities mueren con la tarea**. `revoke_all` corre en las dos
salidas --`EXIT` voluntario y muerte por fault-- y despues de eso el muerto no
tiene handles: no los tiene mal, **no los tiene**.

> Un reinicio en BMO-X empieza de un estado limpio **por construccion**, no
> porque alguien se acordo de limpiar.

Y eso es exactamente lo que el dueno llama *"no hereda el pasado"*.

---

# ⛔ 3. LO QUE SERIA UN ERROR COPIAR

## 3.1 El watchdog de hardware

Es lo primero que se propone y es lo peor: reiniciar la maquina cuando algo se
cuelga. Convierte un fallo con causa en un arranque sin historia. **BMO-X ya
tiene lo contrario** -- la autopsia-- y anadir un watchdog encima seria borrar
justo lo que se acaba de construir.

Si algun dia hace falta un watchdog, la regla es: **escribe el informe primero y
reinicia despues**, nunca al reves.

## 3.2 El reintento sin presupuesto

"Si falla, vuelve a intentarlo" sin un tope es como se hace un bucle de caidas.
Por eso en el plan **la cuarentena (escalon 2) va ANTES de la supervision
(escalon 4)**: el freno se instala antes que el acelerador.

## 3.3 La "resiliencia" que se traga el error

Un `catch` que sigue adelante con un valor por defecto. Este proyecto ya tiene
una ley entera sobre eso --las AGUJAS, 57 sitios revisados y 7 que mentian-- y
la regla quedo escrita: **un fallo o se maneja o se GRITA con su numero, nunca
se descarta callando.** La auto-curacion no puede ser la puerta de atras por la
que eso vuelva a entrar.

---

# ★ 4. EL MARCO, Y DONDE ESTA BMO-X HOY

La literatura de *autonomic computing* llama a esto **MAPE-K**: Monitor,
Analyze, Plan, Execute, sobre una Knowledge. Sirve para ver **que mitad falta**:

| | Que es | En BMO-X hoy |
|---|---|---|
| **Monitor** | ver lo que pasa | ✅ CABINA, klog, `info`, choques de cerrojo |
| **Knowledge** | recordar lo que paso | ✅ **la autopsia** (08-08): 8 renglones por muerte |
| **Analyze** | decidir si es grave y de que tipo | ⛔ **no existe** |
| **Plan** | elegir que hacer | ⛔ no existe |
| **Execute** | hacerlo | ~ a medias: `revoke_all` restaura, el raton se degrada |

★ **El agujero esta en Analyze**, y por eso el escalon 1 del plan es el que es.
No se puede decidir si un fallo es grave sin saber **que dejo sin devolver**, y
hoy nadie lo comprueba: `revoke_all` hace su trabajo y **nadie mira si funciono.**

> No se puede curar lo que no se sabe describir. La autopsia describe la MUERTE;
> falta describir lo que la muerte dejo detras.

---

# ★★ 5. EL MODELO CONCRETO PARA BMO-X

```
   FALLO
     |
     +-- CONTENER      revoke_all           ya existe
     +-- DESCRIBIR     autopsia             ya existe (08-08)
     +-- COMPROBAR     que volvio todo?     <- ESCALON 1, el que falta
     +-- DECIDIR       roto? repetido?      <- ESCALON 2 (cuarentena)
     +-- DEGRADAR      hay modo de respaldo?<- ESCALON 3
     +-- RELANZAR      con politica         <- ESCALON 4
```

## La regla de oro, y es la del SMP otra vez

**Un obrero que solo computa no toca ninguno de los 209 `static mut`.** Aqui la
version es:

> **Cada escalon solo puede leer lo que el anterior dejo escrito.** La cuarentena
> lee la autopsia. El supervisor lee la cuarentena. Ninguno mira el estado vivo
> del kernel.

Sin eso, la auto-curacion se convierte en un modulo que toca todo -- y un modulo
que toca todo es el sitio donde nace el siguiente fallo.

---

# ⚠ 6. LOS MODOS DE FALLO DE LA PROPIA CURA

Se escriben antes de construirla, porque despues se defienden.

| Modo | Como se ve | Que lo impide |
|---|---|---|
| **Bucle de caidas** | el sistema relanza y relanza | la cuarentena, **antes** que el supervisor |
| **Cura silenciosa** | parece sano, va mal | toda degradacion sale por CABINA, sin excepcion |
| **Herencia por la puerta de atras** | el supervisor guarda handles "para ahorrar" | el supervisor **concede desde cero**, nunca guarda |
| **Estado curado y equivocado** | el sistema se recupera a un estado que no es valido | solo se restaura lo que tiene dueno claro; lo demas se rinde y lo dice |
| **El informe que causa el fallo** | escribir la autopsia toca el disco que acaba de caerse | el kernel **captura en RAM**; Ring 3 persiste |

★ La ultima ya esta resuelta y esta escrita en `ring0/core/autopsia.rs`. Las
otras cuatro son las que hay que respetar al construir los escalones.

---

# ★★ 7. Y LA PREGUNTA DEL "DOJO DE HACKING", CONTESTADA CON LO QUE HAY

> *"otra IA dijo que BMO-X podria convertirse en DOJO de hacking porque no usa
> root ni nada de eso, porque NO HEREDA EL PASADO -- en serio?"*

Medio en serio. Lo que es cierto, lo que no lo es todavia, y por que importa
aqui: **la revocacion y la contencion son el MISMO mecanismo**, asi que cada
escalon de auto-curacion es tambien un escalon de seguridad.

## Lo que SI es cierto, y es fuerte

| | |
|---|---|
| **No hay `root`** | no es que este restringido: **no existe el concepto** |
| **No hay autoridad ambiental** | ni SUID, ni tokens, ni `/proc`, ni `ptrace`, ni variables de entorno con poder |
| **Un proceso solo tiene handles** | lo que no le concedieron **no existe para el** |
| **La superficie cabe en la cabeza** | 2 syscalls y 40 operaciones. Se audita en una tarde |
| **Las capabilities mueren con la tarea** | no hay estado que sobreviva para reusarlo |

★ La mayoria de las tecnicas de escalada de privilegios de un Unix atacan la
**autoridad ambiental**: un binario SUID, un descriptor heredado por `fork`, un
token robado, una capability de Linux mal puesta. **Ninguna de esas cosas existe
aqui**, y no porque se hayan tapado: porque nunca se anadieron.

## ⛔ Lo que NO es cierto todavia, y hay que decirlo

Hoy BMO-X **tiene tres autoridades ambientales**, y las tres estan confesadas en
sus propios comentarios:

1. **`TASK_OP_EJECUTAR`** -- cualquier tarea de Ring 3 puede lanzar un programa.
   No pide capability.
2. **`TASK_OP_REINICIAR`** -- igual. El comentario lo dice con todas las letras:
   *"hoy no esta atada a una capability, igual que EJECUTAR"*.
3. **`TASK_OP_ENDPOINT_CONNECT`** -- *"hoy cualquier proceso puede pedir
   cualquier endpoint por su indice, y eso NO es disciplina de capabilities"*.

Son **tres filas**, no un rediseno. Pero mientras esten, la frase "aqui no hay
autoridad ambiental" es una aspiracion y no una propiedad -- y en seguridad esa
diferencia es toda la diferencia.

⚠ Y una cuarta, distinta de las otras: **el kernel tiene 209 `static mut` y un
bufer de imagen UNICO**. Eso no es escalada de privilegios, es superficie: es
donde vive la clase de fallo que un pentest de verdad busca primero.

## Por que si seria un buen sitio para ENSENAR

No porque sea inexpugnable -- no lo es, y acaba de listar por que. Sino porque:

- **cabe entero en la cabeza**: 2 syscalls, 40 operaciones, un modelo de objetos
  con generaciones. Un alumno puede leerlo todo;
- **cada fallo deja una autopsia**: se le puede dar un bug a alguien y de verdad
  lo encuentra, en vez de rebotar contra un kernel de 30 millones de lineas;
- y **las tres autoridades ambientales de arriba son ejercicios perfectos**: son
  reales, estan documentadas, y cerrarlas es trabajo de verdad con un resultado
  comprobable.

> Un sistema es buen material de ensenanza cuando sus agujeros son **legibles**,
> no cuando no los tiene.

---

Ver [`PLAN_AUTOCURACION.md`](../plan/PLAN_AUTOCURACION.md) para las casillas,
`ring0/core/autopsia.rs` para lo que ya se captura, y `BITACORA.md` para los
fallos que ensenaron cada regla de aqui.
