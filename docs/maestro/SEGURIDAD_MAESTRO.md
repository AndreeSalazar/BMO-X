# SEGURIDAD MAESTRO -- de que copiar la confianza, y de que seria un error

> Escrito el **2026-08-18**. Pregunta del dueno: *"puedo estudiar que es
> backdoors para que me des ejemplo para que existen? a parte mi BMO-X
> ironicamente no tiene, no? ... podria INSPIRARME en BLAKE3 y mas cosas"*.
>
> Este documento contesta **de que copiar**. Lo que falta, que lo bloquea y como
> se sabe que quedo hecho vive en [`PLAN_SEGURIDAD.md`](../plan/PLAN_SEGURIDAD.md).
> Son dos ficheros por la tabla de `docs/README.md`, no por longitud.

---

# ★★ 0. LA FRASE QUE ORDENA EL DOCUMENTO ENTERO

```
   INTEGRIDAD    estos bytes son los que se escribieron    <- BLAKE3 ya lo hace
   AUTENTICIDAD  los escribio alguien en quien confias     <- no hay nada
```

BMO-X tiene la primera **entera y bien puesta**. No tiene ni un gramo de la
segunda. Todo lo que sigue es esa distincion mirada desde seis sitios distintos.

Y el motivo de que la primera no baste se dice en una linea: **el volumen lo
escribe `estratos-fmt`, o sea que el mismo programa que pone el binario pone su
`:firma`.** Quien pueda escribir uno puede escribir la otra. El hash prueba el
TRANSPORTE, nunca el AUTOR.

---

# 1. QUE ES UN BACKDOOR, con las dos mitades que hacen falta

> Un camino de acceso que **salta la autorizacion del propio sistema**, conocido
> por quien lo puso y **no** por quien lo usa.

Las dos mitades importan, y separan tres cosas que se confunden a diario:

| | salta la autorizacion | oculto | como se llama |
|---|---|---|---|
| consola de mantenimiento documentada | si | no | una **funcion** |
| desbordamiento de bufer | no | si | un **fallo** |
| clave de servicio no documentada | si | si | un **backdoor** |

Un fallo se arregla. Un backdoor **se retira**, y despues hay que preguntarse
quien mas lo sabia.

---

# 2. LAS CUATRO FAMILIAS, y de cual esta BMO-X a salvo por construccion

## 2.1 Depuracion que se quedo dentro

La mas comun y la menos dramatica: un usuario de servicio con clave fija para
poder entrar cuando el cliente no coge el telefono, y nadie lo quito antes de
vender. No hay malicia; hay una fecha de entrega.

★ **BMO-X esta a salvo de esta por una CARENCIA, no por virtud**: no hay
usuarios, no hay claves y no hay sesion, asi que no hay donde poner una. El dia
que haya red y algo parecido a una cuenta, esta familia entra de golpe.

## 2.2 El fabricante, o un estado

`Dual_EC_DRBG`: un generador de numeros aleatorios estandarizado por el NIST
cuyas constantes permitian, a quien conociera un valor asociado, predecir toda
la salida. Se retiro en 2014.

★★ **La leccion que aplica aqui no es la del espionaje: es que el backdoor
perfecto NO ES CODIGO, son constantes.** Nadie lo encuentra leyendo un `if`. Y
BMO-X va a copiar aritmetica con constantes en cuanto toque Ed25519 --el primo
`2^255 - 19`, el punto base, los vectores-- asi que la regla se escribe hoy:

> **Ninguna constante criptografica entra sin su vector oficial al lado y una
> prueba que lo compruebe.** Copiar los numeros de un blog es exactamente el
> camino por el que entro Dual_EC.

## 2.3 La cadena de suministro

`xz-utils`, marzo de 2024: alguien se gano el mantenimiento de una libreria de
compresion a lo largo de dos anos y metio, **en los ficheros de construccion y
no en el fuente**, algo que enganchaba la autenticacion de `sshd`. Lo destapo un
ingeniero que estaba midiendo latencias y noto medio segundo de mas en un login.

★★ **Lo que hay que copiar de este caso es DONDE miro el que lo encontro**: no
leyo el codigo, **midio**. Y lo que hay que mirar aqui es que la superficie
equivalente en BMO-X es `build.ps1` y las tablas de `sem-asm` -- ficheros que
deciden lo que entra en el binario y que **no pasan por ninguna comprobacion de
formato**, al contrario que el `.bex`.

## 2.4 ★★ La que te aplica de verdad: el compilador

Ken Thompson, *Reflections on Trusting Trust* (1984). Un compilador puede meter
un backdoor en lo que compila **y meterse a si mismo cuando compila un
compilador**. Entonces no esta en ningun fuente: borras todo el codigo malicioso,
recompilas, y vuelve.

Aqui no es una anecdota historica. **BMO C, COBOL, `sem-asm` y `bex-link` son
tuyos**, y eso es enorme a tu favor --no dependes del compilador de nadie-- pero
es a la vez la unica pieza que nadie puede verificar por ti, porque el primer
eslabon de la cadena lo compilo `rustc`, que no escribiste.

La contramedida tiene nombre y no es teorica: **diverse double-compiling**
(David A. Wheeler, 2009). Se compila el compilador con un compilador
independiente y se comparan los binarios resultantes; si el backdoor esta en uno
solo de los dos caminos, los bytes no cuadran.

---

# ★ 3. DE QUE COPIAR, ordenado por lo que cuesta

| idea | de quien | que compra | que cuesta | veredicto |
|---|---|---|---|---|
| **NX, SMEP, SMAP, UMIP** | el propio CPU | que una pagina de datos no se ejecute, y que Ring 0 no toque Ring 3 sin querer | **cuatro bits** | ★★ SI, ya |
| **Ed25519** | la cripto moderna | autenticidad: solo la clave PUBLICA vive en la maquina | dias de aritmetica de curva, mas vectores | ★★ SI, es LA pieza |
| **Compilacion reproducible / DDC** | Debian, Wheeler | que el binario no dependa de quien lo genera | disciplina en `build.ps1` | ★ SI, y ya empezo |
| `pledge` / `unveil` | OpenBSD | que un proceso declare lo que va a usar y pierda el resto | -- | **YA LO TIENES**: se llaman capabilities |
| la prueba de aislamiento | seL4 | saber que no queda canal entre dos procesos | una tesis doctoral | ★ copiar la PREGUNTA, no el metodo |
| registro publico de firmas | Certificate Transparency, Sigstore | que una firma no se pueda emitir en secreto | red y un tercero | NO todavia |
| arranque medido / TPM | la industria | que el arranque conste | encadenarte a la llave de otro | ★★ NO, y ver 4.3 |
| ASLR | todos | que una direccion no se adivine | reubicacion en cada carga | matizado, ver 4.4 |

## 3.1 ★★ Lo mas barato es justo lo que falta: cuatro bits

No lo dice este documento: lo dice **la propia tabla del kernel**, en
`kernel/src/ring0/cpu_vendor/features/usage.rs:139`.

> *"Las tres son GRATIS --bits de CR4 y de EFER-- y ninguna esta puesta, en un
> microkernel cuyo lema declarado es cero confianza en el codigo. Es la seccion
> mas incomoda de esta tabla y por eso va entera."*

Y debajo, las cuatro filas con su motivo escrito:

```
   Nx     nadie toca EFER.NXE: TODA pagina que BMO mapea es ejecutable
   Smep   impide que Ring 0 EJECUTE una pagina de Ring 3. Un bit de CR4
   Smap   impide que Ring 0 LEA una de Ring 3 sin querer. Otro bit
   Umip   esconde SGDT/SIDT/SLDT a Ring 3; fuga de direcciones del kernel
```

★★ **El sistema ya sabe lo que le falta y lo dice en voz alta.** Esa es la razon
de que este documento se pueda escribir con numeros en vez de con opiniones, y
es merito del censo de caracteristicas, no de quien audita.

## 3.2 De BLAKE3, lo que ya se copio BIEN -- y es el modelo de como se copia aqui

- **Una sola implementacion.** Salio de `bmo-abi` a `platform/shared/bmo-hash` el
  dia que iba a haber una segunda. Firmas BEF, `bmo-verify` y las sumas de
  ESTRATOS usan la misma. Dos implementaciones de un hash son dos
  comportamientos con un solo nombre.
- **La suma vive en QUIEN APUNTA, no en el bloque.** El `BlockPtr` de ESTRATOS
  lleva direccion + suma, copiado de ZFS: el Merkle sale gratis y el que lee no
  necesita indice.
- **Se comprueba al ATERRIZAR, no al leer** (`task/landing.rs`). Comprobar el
  bufer contesta *"lo que lei cuadra con lo que se escribio"*; comprobar al
  aterrizar contesta *"lo que este proceso va a EJECUTAR cuadra"* -- y entre las
  dos hay una copia, que es un sitio donde las cosas se rompen.

★ **Y lo que NO se copio, tambien a proposito**: BLAKE3 trae modo con clave
(`keyed_hash`). Se propuso el 2026-08-10 y se rechazo con motivo -- ver 4.2.

---

# 4. LO QUE SERIA UN ERROR COPIAR

## 4.1 `root`, `setuid`, `chmod`

Ya esta rechazado, y la mitad de los backdoors clasicos son "conseguir root". Un
privilegio universal es un unico punto que robar; aqui la palabra no existe.
**Esto no hay que decidirlo otra vez: hay que no deshacerlo.**

## 4.2 Un MAC con la clave dentro del kernel

Tentador porque BLAKE3 ya lo trae y saldria casi gratis. **No resuelve la
amenaza**: la clave viviria en el mismo disco que el `.bex`, y quien pueda
reescribir uno puede leer la otra. Protege del ERROR --fichero equivocado,
escritura a medias--, que ya es algo, pero llamarlo firma seria la clase de
mentira que este arbol persigue.

★★ Lo que si resuelve la amenaza es **asimetrico**. Por eso el destino es Ed25519
y no un atajo que se parezca.

## 4.3 Secure Boot o arranque medido con la cadena de otro

Contradice la tesis del proyecto. Encadenar el arranque de BMO-X a una llave de
Microsoft o de un fabricante de placas es el error de Tiny Core con X11 dicho en
la capa de arranque: **el nucleo pequeno no te sirve si dependes de la
infraestructura de confianza de otro.** El dia que haga falta arranque medido, la
cadena tiene que ser tuya o no vale para lo que se compro.

## 4.4 ASLR, con matiz -- y la trampa de analisis que lleva al lado

En un sistema con red, varios usuarios y atacantes remotos, ASLR compra mucho.
Aqui hoy no hay red, no hay usuarios y quien lanza el `.bex` es el dueno de la
maquina: compra **poco**, y cuesta reubicacion en cada carga.

★ **Pero W^X no comparte ese matiz y meterlos en el mismo saco seria el error.**
NX es un bit, no un mecanismo. Su ausencia no significa "una defensa menos":
significa que hoy **cualquier pagina de datos de BMO-X es ejecutable**.

## 4.5 Antivirus, listas negras, heuristicas

Enumerar lo malo es la estrategia perdedora y ademas es incompatible con el
alcance acotado. BMO-X enumera lo BUENO --capabilities, lista cerrada de
etiquetas, dos syscalls-- y eso no es una version pequena de un antivirus: es lo
contrario.

---

# ★★ 5. LA IRONIA, DICHA CON SU NUMERO

El dueno lo dijo asi: *"es ironico que mi BMO-X ya esta duro pero le falta
algunas piezas"*. La ironia es real y se mide:

```
   la superficie de ataque del sistema      2 syscalls
   las protecciones del CPU que faltan      4 bits
   la pieza que falta de verdad             1 firma asimetrica
```

**Lo duro es la FORMA; lo que falta es BARATO.** Dos syscalls, sin `root`, sin
`..` que escape del arbol concedido, capabilities en vez de permisos
ambientales: eso no se anade despues, y BMO-X ya lo tiene. Lo que falta son
cuatro bits y una curva.

★★ **Y por eso no hay que subestimarlo, que es exactamente lo que dijo el
dueno**: un sistema con una forma dura y una pieza barata suelta **invita a creer
que esta entero**. La forma dura hace de aval de lo que todavia no se ha hecho.
Esa confusion es mas peligrosa aqui que en un sistema flojo, porque del flojo
nadie se fia.

> **Lo que BMO-X puede afirmar hoy no es "no tiene backdoor": es que NO HAY DONDE
> ESCONDER UNO.** Dos syscalls se leen en una tarde. Eso no lo puede decir de si
> mismo ningun sistema que se use a diario -- y no porque sean deshonestos, sino
> porque son demasiado grandes para que nadie pueda afirmarlo.

---

# 6. LA TRAMPA QUE YA ESTABA ANOTADA, para no perderla

`c/sonda.bex` ataca a su propio kernel con siete familias, y en metal salieron
**13 defensas correctas, 0 agujeros**. Es una buena prueba, y tiene un limite que
ya estaba escrito:

> **La sonda la escribio el mismo lado que escribio las defensas.** Prueba lo que
> se nos ocurrio atacar; el atacante imaginado se parece al defensor.

★ **La primera prueba que no se escribe uno mismo llega con la RED.** Un `.bex`
malo lo trae quien ya tiene la maquina; una trama Ethernet la manda cualquiera
que comparta el cable. Por eso el parser de tramas va en Ring 3: morirse ahi es
barato.

---

Ver `docs/plan/PLAN_SEGURIDAD.md` (las casillas y que las bloquea),
`docs/componente/LA_PUERTA_POR_DENTRO.md` (la superficie de los syscalls) y
`docs/identidad/EL_CONTRATO_DE_CARGA.md` (que promete la carga y que no).
