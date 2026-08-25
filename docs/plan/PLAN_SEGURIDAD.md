# PLAN SEGURIDAD -- las casillas que faltan, medidas contra el codigo

> Escrito el **2026-08-18** y **RELEIDO CONTRA EL CODIGO el 2026-08-25**: de las
> siete casillas, **dos estaban hechas y una estaba mal medida**. Lo que cambio
> va marcado con su fecha; lo que sigue abierto se dejo como estaba.
>
> Escrito auditando el arbol tal como estaba entonces. El **por que**
> de cada eleccion --de que copiar y de que seria un error copiar-- vive en
> [`SEGURIDAD_MAESTRO.md`](../maestro/SEGURIDAD_MAESTRO.md). Aqui solo hay
> casillas: **que falta, que la bloquea, y como se sabe que quedo hecha.**
>
> Todo lo que se afirma abajo se comprobo leyendo el fichero, no la memoria. Las
> rutas y los numeros de linea son de esta fecha.

---

# 0. EL ESTADO, EN UNA TABLA

| superficie | que la protege hoy | que le falta |
|---|---|---|
| **`.bex` al ejecutar** | BLAKE3 por seccion al aterrizar, y el gate rechaza sin firma en ESTRATOS | **autoria**: no hay clave |
| **`.bex` al EMITIR** | `bmo-verify`, que llaman los **cinco** frontends antes de escribir | que lo use el **kernel** (ver C5) |
| **`.bex` en FAT32** | nada: `veredicto` es `None` y el gate no corre | -- (limitacion del formato, ver C5) |
| **Syscalls** | capabilities con derechos, `sonda.bex` los empuja | `EJECUTAR` y `REINICIAR` sin atar |
| **Memoria del proceso** | separacion de anillos, y ★ **los CUATRO bits encendidos** (25-08): NX/W^X, SMEP, SMAP, UMIP | ASLR, y esta fuera a proposito (ver 4) |
| **Dispositivos** (HID, GPT, MADT) | se parsean sin desconfiar | ninguna sonda |
| **Red** | ⚠ **ya NO es cero** (25-08): el anillo RX recibio 16 tramas / 7.967 bytes en metal. Lo que la acota hoy: **no se transmite**, y el DMA va a un corral | la que deja de ser la linea buena de la tabla. Ver 4 |

---

# ★ 1. LO QUE YA ESTA PUESTO -- y no hay que rehacerlo ni deshacerlo

Se dice primero porque una auditoria que solo enumera agujeros miente por
omision, y porque tres de estas cuatro **no se anaden despues**:

- **Dos syscalls congelados.** La superficie entera del sistema se lee en una
  tarde. Es la propiedad que hace posible este documento.
- **Capabilities en vez de permisos ambientales.** Sin `root`, sin `chmod`, sin
  `..` que escape del arbol concedido. No hay privilegio universal que robar.
- **El hash se cierra al ATERRIZAR** (`kernel/src/ring0/task/landing.rs`), no al
  leer el bufer. Cubre la copia que hay entre las dos preguntas.
- **El gate de admision funciona** (`kernel/src/ring0/task/launch.rs:681`): en
  ESTRATOS, `sin firma no hay ejecucion`, literal. Es mas duro de lo que la
  memoria del proyecto decia.

---

# 2. LAS CASILLAS

## [X] C1 -- `verify_ed25519` devuelve `true` a una firma de ceros -- **HECHA el 24-08**

> **Como quedo**: la funcion ya no existe con ese nombre ni con ese tipo. Hoy es
> `examinar_ed25519` y devuelve un `enum Firma` con **dos** variantes --
> `SinFirmar` y `NoSePuedeComprobar`-- y **ninguna de las dos se puede confundir
> con una firma valida**. Es mas fuerte que lo que esta casilla pedia: se pidio
> que devolviera `false`, y lo que se hizo fue **quitar el `bool`**, porque un
> `bool` obliga a elegir entre dos mentiras cuando la respuesta honesta es *"no
> lo se"*. Sigue sin llamarla nadie.
>
> Lo de abajo se conserva porque **describe la trampa**, que es lo que hay que
> recordar el dia que C3 la cablee.

**Donde**: `platform/abi/bmo-abi/src/bef/signing.rs:228`.

```rust
let is_unsigned = _sig.sig.iter().all(|&b| b == 0) && _sig.pubkey.iter().all(|&b| b == 0);
if is_unsigned {
    return true;
}
```

**Que pasa**: una firma con todo a cero **verifica como valida**. El comentario
lo dice y era razonable mientras se construia (*"unsigned binaries are allowed in
dev"*), pero la forma es la de un **fallo abierto**: para pasar el control no hay
que falsificar una firma, hay que **borrarla**.

**Por que no es un agujero hoy**: no lo llama nadie. Se comprobo con `grep` sobre
el arbol entero -- cero llamadas fuera de su propia definicion.

⚠ **Por que es la casilla mas urgente igualmente**: es `pub` en `bmo-abi`, o sea
superficie publica del ABI. El dia que alguien la cablee --que es justo lo que
pide C3-- hereda el `true` sin enterarse, y entonces el gate dira que comprobo
algo que no miro. **Esa es la definicion de mentira que envejece sin avisar.**

**Que la bloquea**: nada. Son cinco minutos.

**Como se sabe que quedo hecha**: la funcion devuelve `false` mientras no haya
verificacion de verdad, y quien quiera permitir binarios sin firmar lo decide
**arriba, en la politica**, donde se ve -- no dentro del verificador. Una
excepcion repartida tapa el sintoma que se vio, no el que viene; es la misma
leccion que `verdict::es_fragmento` de MAQUETA.

## [X] C2 -- los cuatro bits que el CPU regala y nadie enciende -- **HECHA el 25-08**

> ★★ **Y esta casilla estaba MAL MEDIDA: tres de los cuatro ya estaban puestos.**
> `SMEP` (`cr4 |= 1 << 20`) y `UMIP` (`cr4 |= 1 << 11`) los pone `s1_cpu/cpu/mod.rs`
> desde hace meses, y `EFER.NXE` lo pone `s1_cpu/cpu/zen3.rs`. La fila de la
> tabla se escribio mirando **el kernel**, y los bits los enciende **otra etapa**.
>
> ```text
>    NX / W^X   [X] 25-08   EFER.NXE ya estaba; faltaba el PTE_NX en `vmm.rs`
>    SMEP       [X] ya estaba   Ring 0 no EJECUTA una pagina de Ring 3
>    SMAP       [X] 25-08   y no fue un bit: habia DOS sitios que tocaban Ring 3
>    UMIP       [X] ya estaba   SGDT/SIDT/SLDT/STR ya no fugan del kernel
> ```
>
> ⚠ **La leccion es de este documento, no del codigo**: una tabla equivocada no
> deja un hueco, **CIERRA LA PREGUNTA** -- nadie vuelve a mirar lo que ya esta
> contestado. Y por eso esta casilla, hecha, se conserva entera en vez de
> borrarse.
>
> ★ Lo que W^X compro, dicho sin adornos: **la mitad de una explotacion.** Quien
> logre escribir en cualquier sitio ya no puede escribir instrucciones y saltar a
> ellas; tiene que armar la cadena con codigo que YA existe (ROP). Y con SMEP, el
> destino habitual de esa cadena --saltar a codigo de Ring 3-- tambien esta
> cerrado. Y SMAP fue el unico que costo dias: no es un bit, es **quitarle al
> kernel dos caminos que ya usaba**, y el permiso que queda vive en UN sitio con
> nombre (`autopsy::con_permiso`). Un permiso repartido por seis sitios no es un
> permiso: es la regla apagada.

**Donde**: `kernel/src/ring0/cpu_vendor/features/usage.rs:142-146`. La tabla del
propio kernel ya los declara sin uso:

```
   Nx     nadie toca EFER.NXE: TODA pagina que BMO mapea es ejecutable
   Smep   impide que Ring 0 EJECUTE una pagina de Ring 3. Un bit de CR4
   Smap   impide que Ring 0 LEA una de Ring 3 sin querer. Otro bit
   Umip   esconde SGDT/SIDT/SLDT a Ring 3; fuga de direcciones del kernel
```

**Que la bloquea**: NX no es solo el bit de `EFER`. Encenderlo sin poner el bit
`NX` en las paginas de datos no cambia nada, y ponerlo en las equivocadas deja de
arrancar. **SMAP es el que muerde**: el kernel lee bufers de Ring 3 a proposito
en varios sitios, y cada uno de ellos necesita `STAC`/`CLAC` alrededor o el
sistema se cae en el primer syscall que copie algo.

**Orden barato, medido por lo que puede romper**: `UMIP` -> `SMEP` -> `NX` ->
`SMAP`. Los dos primeros son un bit y ya esta; los dos ultimos piden repasar
mapeos y copias.

**Como se sabe que quedo hecha**: `ext` imprime el censo y esas cuatro filas
pasan de `No` a `Yes` con su motivo; y **la sonda gana un empujon nuevo**: un
`.bex` que salte a una pagina de datos suya tiene que morir, no ejecutarla.

## [X] C3 -- Ed25519 -- **HECHA el 2026-08-25**, y era media pieza

**Como quedo**: `platform/shared/bmo-cripto/src/ed25519.rs`, con los **cuatro
vectores de RFC 8032 7.1** verificando y seis pruebas negativas. Y `sha512.rs`
debajo, con los vectores de FIPS 180-4 -- entro por esto y solo por esto.

### ★ La casilla decia "LA pieza" y ya estaba medio pagada

Se escribio cuando no habia ni un hash en el arbol. Para cuando le llego el
turno:

```text
   el campo 2^255-19    YA ESTABA, escrito y probado para X25519
   SHA-512              se escribio el mismo dia. Vectores de NIST
   la curva de Edwards  lo unico de verdad nuevo
```

**La aritmetica modular --la parte que asusta-- llevaba semanas hecha.** Lo que
faltaba era otra curva encima del mismo campo.

### ⚠ Y FIRMAR NO ESTA, QUE ES LA DECISION Y NO UNA OBRA A MEDIAS

Esta casilla ya lo tenia escrito, y se cumplio al pie de la letra:

> *"En la maquina no, o vuelve el problema de 4.2 del maestro. Vive donde se
> firma, que es el anfitrion, y a BMO-X solo baja la publica."*

Una maquina que puede firmar **tiene dentro con que falsificar lo que ejecuta**.
BMO-X solo necesita saber decir que no.

### *** LO QUE UNA PRUEBA DESTAPO, Y ES C1 OTRA VEZ POR OTRA PUERTA

Se escribio una prueba con la firma de ceros --la de C1-- y **fallo en la primera
pasada**: `verificar` decia que SI. Y no era un fallo de la curva, era la curva
funcionando:

```text
   32 bytes a cero  ->  y = 0  ->  x2 = (0-1)/(0+1) = -1
                    ->  y -1 SI tiene raiz en este campo
```

Una clave de ceros **es un punto de verdad**: uno de orden 4. Con `S = 0` la
ecuacion se queda en `[-k]T == T`, que se cumple **una de cada cuatro veces**
segun lo que salga del hash. Con el mensaje del vector 1 salio.

> C1 decia: *"para pasar el control no hay que falsificar una firma, hay que
> BORRARLA."* Se quito el `if is_unsigned { return true; }`, y la misma entrada
> volvia a pasar -- ahora **por matematicas en vez de por un atajo**.
>
> ★★ **Un agujero tapado por arriba y abierto por abajo.**

Se cierra rechazando los puntos de orden pequeno --`[8]P == O`, tres doblados--
en la clave publica **y** en la `R`. Y la leccion es de metodo: **esa prueba se
escribio por historia, no por sospecha.** Sin la memoria de C1 no se habria
escrito, y el agujero habria entrado con los cuatro vectores del RFC en verde.

### Lo que esta pieza NO promete

- **No es de tiempo constante, y no tiene por que serlo**: verificar no toca
  ningun secreto. Los tres datos --clave publica, mensaje y firma-- son publicos.
- **Se usa `[S]B = R + [k]A` y no la de los ochos.** RFC 8032 5.1.7 permite las
  dos; esta es **mas estricta**. Si algun dia una firma valida en otro sitio se
  rechaza aqui, esa es la primera linea que releer.

### [X] Y CABLEADO AL CARGADOR EL MISMO DIA -- pero no como se penso

La casilla decia *"no lo llama nadie todavia"*. Se fue a cablear, y al mirar el
formato aparecio lo que convertia el cableado obvio en un control inutil:

```text
   Ed25519Signature = sig[64] || pubkey[32]
```

⚠⚠ **La clave publica viaja DENTRO de la firma.** Comprobarla contra esa clave
siempre da que si, porque **el firmante eligio las dos cosas**. Cualquiera se
genera un par, firma el binario y mete su clave al lado.

> Una firma que trae su propia clave demuestra que **nadie la ha tocado desde que
> se firmo**. No demuestra **quien la firmo**.

★★ **Y es la MISMA forma, por tercera vez en dos dias:**

```text
   C1 (24-08)   `verify_ed25519` decia SI a una firma de ceros
   C3 (25-08)   la firma de ceros PASABA otra vez, por matematicas
   el cableado  la firma cuadraria... con la clave que trajo el firmante
```

### Lo que se hizo en su lugar: el ANCLA

```text
   bmo-firma                        la ARITMETICA. Sin opinion, sin alloc
   task/confianza.rs                LA OPINION: en quien confia esta maquina
```

Y el reparto es el que C1 dejo escrito el dia que se arreglo:

> *"quien quiera permitir binarios sin firmar lo decide **arriba, en la
> politica, donde se ve** -- no dentro del verificador."*

### Los cuatro noes, y cada uno manda a un sitio distinto

| veredicto | que significa | donde mirar |
|---|---|---|
| `SoloIntegridad` | `sig_algo = 0`: hashes y nada mas. **Lo de hoy** | -- |
| `Firmado{clave}` | cuadra Y esta en el ancla. **Dice CUAL** | -- |
| `NoCuadra` | la firma no es de estos bytes | el fichero |
| ★ `AutorDesconocido` | **firma impecable, clave que no conozco** | el ancla |
| `AlgoritmoDesconocido` | firmado con algo que no implemento | el emisor |
| `SeccionRota` | la seccion no mide lo que promete | el escritor |

** `AlgoritmoDesconocido` **se rechaza en vez de ignorarse**: un `.bex` que
declara un algoritmo que este sistema no entiende puede estar firmado
perfectamente por otro, y tratarlo como *"sin firma"* seria degradarlo en
silencio a un control mas flojo.

### ⚠ Y HOY NO CAMBIA LA CONDUCTA DE NADA, a proposito

Se midieron los 24 `.bex` del arbol: **19 traen seccion `Signature` y los 19
tienen `sig_algo = 0`**. Todos dan `SoloIntegridad`, y con `exige_firma() =
false` todos siguen arrancando igual.

★ **Encender la firma es una decision con nombre y tiene orden**, escrito en
`confianza.rs`: primero una clave en el ancla, despues un `.bex` firmado con ella
que arranque, y **al final** `exige_firma()`. Al reves, la maquina deja de
arrancar y el motivo parece del cargador.

[!] Y una deuda que nace con esto y hay que decirla: **anadir una clave al ancla
concede ejecucion a todo lo que esa clave firme, para siempre. No hay
revocacion.** Escribirla antes de la primera clave seria construir la puerta
antes de la casa; despues de la segunda seria tarde.

### El firmador existe, y el kernel no puede llamarlo

Hizo falta para poder probar el gate --un gate no se comprueba contra firmas que
no existen-- y vive detras de la bandera `firmar` de `bmo-cripto`, que el kernel
no enciende. Se comprueba **al reves**: firma los mensajes de los vectores del
RFC con sus claves secretas y **salen las firmas del RFC byte a byte**.

> Un firmador comprobado solo con su propio verificador es un par de funciones
> que se creen la una a la otra.

## C4 -- `EJECUTAR` y `REINICIAR` no piden ninguna capability

**Donde**: declarado por el propio kernel, `syscall/ops.rs:200`:

> *"**Limitacion declarada**: hoy no esta atada a una capability, igual que
> `EJECUTAR`. Cualquier tarea de Ring 3 puede llamarla. [...] las dos
> operaciones quieren la misma capability el dia que exista."*

**Que significa**: cualquier `.bex` que corra puede lanzar otro `.bex`, o
reiniciar la maquina. Con el gate de firma puesto, lo primero esta acotado
--solo puede lanzar lo que pase el gate-- pero lo segundo no: reiniciar es
gratis para cualquiera.

**Que la bloquea**: decidir **quien la concede**. Hoy el escritorio lanza hijos
todo el rato, asi que la capability tiene que llegarle a el sin que se la pueda
pasar a lo que lanza. Es un problema de delegacion, no de comprobacion.

**Como se sabe que quedo hecha**: `sonda.bex` gana un empujon octavo --pedir
reiniciar sin tenerla-- y el kernel lo niega y sigue en pie.

## ⚠ C5 -- `bmo-verify` -- **LA CASILLA MIDIO EL SITIO EQUIVOCADO** (25-08)

**Lo que decia**: *"`grep` sobre `build.ps1` y `bmo.ps1`: cero menciones (...) el
build no lo llama nunca. Un binario mal formado se descubre en el Ryzen."*

**El grep era cierto y la conclusion falsa.** `bmo-verify` no se llama desde el
script de construccion porque **se llama desde dentro de los compiladores**, que
es antes y es mejor:

```text
   toolchain/lang/ada/Cargo.toml       bmo-verify = { path = ... }
   toolchain/lang/c/Cargo.toml         idem
   toolchain/lang/cobol/Cargo.toml     idem
   toolchain/lang/cpp/Cargo.toml       idem
   toolchain/lang/inti/emisor-x86_64/  idem
```

Los **cinco** frontends lo llaman **antes de escribir el `.bex`**, y
`CONTRIBUTING.md` lo declara desde el 2 de agosto. Un `.bex` mal formado no llega
al Ryzen: no llega ni a existir.

★★ **Es EXACTAMENTE el mismo fallo que C2**, cometido por el mismo documento el
mismo dia: se busco la comprobacion **donde a uno le parecia que tenia que
estar**, no se encontro, y se escribio que no existia. Dos veces en siete
casillas. Y las dos veces la comprobacion estaba una etapa mas abajo.

> **Un `grep` que no encuentra prueba que no esta AHI. No prueba que no este.**

**Lo que SI queda abierto, y es la mitad de verdad de esta casilla**: el que no
lo usa es **el kernel**.

---

### [X] C5.1 -- las relocations, juzgadas al CARGAR -- **HECHA el 25-08**

Al abrirlo, la casilla resulto estar mal planteada **otra vez**, y en la
direccion contraria a la primera.

**Lo que se creia**: *"el kernel no llama a `bmo-verify`; hay que cablearselo."*

**Lo que hay**: el kernel **no puede** llamarlo --`bmo-verify` delega en
`bmo_abi::bef::validator`, que usa `alloc`, y en Ring 0 no hay a quien pedirle
memoria-- y eso **ya estaba resuelto el 2026-08-10**: existe `bmo-bex-gate`, sin
`alloc` y sin dependencias, que es el juez comun. El kernel lo llama. Los dos
gates comparten juez.

★ Asi que el hueco no era *"el kernel no verifica"*. Era mas fino y mas concreto:

```text
   al EMITIR    bmo_bex_gate::revisar()  +  validator::validate()   DOS capas
   al CARGAR    bmo_bex_gate::revisar()                             UNA
```

### *** Y en la capa que falta habia UNA comprobacion que importa

De las catorce familias que `validator` anade, trece producen **un programa
roto**, no un kernel comprometido. La que no:

> **`validate_reloc_section`: que `offset + 8` quepa dentro de la seccion que la
> relocation dice parchear.**

[!] Y lo que la hace grave es que **no se sale de la imagen**: las secciones se
colocan seguidas desde `USER_IMAGE_BASE`, o sea que un offset pasado de rosca
**cae dentro de la SIGUIENTE**. La unica comprobacion que el cargador tenia
--*"cae en la pagina que estoy parcheando"*-- se cumplia, y escribia.

```text
   una reloc que dice `.data + 0x9000` en una `.data` de 0x400
      no falla: ACIERTA EN OTRA SECCION
```

Y el hash tampoco lo caza: el de cada seccion **se cierra antes de parchear**.

** No es una fuga fuera del proceso --el marco es suyo, y el write esta acotado
a la pagina-- pero si es un programa que se corrompe a si mismo en silencio, que
es la clase de fallo que tarda semanas en atribuirse a nada.

### La regla se escribio en el GATE, y no en el cargador

Anadirle la comprobacion al kernel era lo obvio y era lo equivocado: serian
**dos copias de la misma decision**, que es el problema que `bmo-bex-gate` se
creo para terminar.

```text
   la REGLA    `gate::reloc_cabe`, una vez, sin alloc y sin dependencias
   los DATOS   los pone cada llamante, porque cada uno tiene otros
```

[!] Y hacen falta los dos, porque `revisar()` **no puede** hacerlo: en el kernel
recibe solo el **prologo** del fichero, y la tabla de relocations vive mucho mas
alla --la de DOOM son 30.840 bytes--. No es que no se quisiera: ahi todavia no
estan esos bytes.

### ⚠ Y una tercera copia que NO se junto, a proposito

`validator` sigue con su version de la regla. Delegar habria hecho que
`bmo-abi` dependiera de `bmo-bex-gate`, y su `Cargo.toml` lo prohibe por escrito:

> *"No es dependencia de la libreria -- **el contrato no depende de la puerta**."*

Delegar habria invertido esa flecha en silencio. Asi que las dos copias siguen,
**atadas por una prueba** (`tests/gate_y_validador_no_se_separan.rs`) que le hace
la misma pregunta a las dos y exige la misma respuesta.

> Cuando la arquitectura no deja juntar dos copias de una decision, lo que queda
> no es confiar: es **atarlas por fuera**.

### *** LA VERIFICACION QUE MAS VALIO: no rechaza nada, y por UN BYTE

Antes de cablear la regla se midieron **todos los `.bex` del arbol** contra ella
--los 5 que el kernel EMBEBE (que no pasan por el escritor) y los 19 de
staging--. Ninguno se rechaza. Pero la mas ajustada sale asi:

```text
   .data de doom.bex     151.560 bytes = 0x25008
   la reloc #706         offset 0x25000, ocho bytes, acaba en 0x25008
   holgura               CERO
```

★★ **Un `<` en vez de un `<=` no habria fallado ninguna prueba: habria dejado de
cargar DOOM.** Y el sintoma no seria *"relocation invalida"*, seria que el
programa mas grande del arbol deja de arrancar por un byte. Esos numeros estan
hoy dentro de la prueba del gate, para que si alguien afila la regla se entere
antes de llegar al Ryzen.

**Como se sabe que quedo hecha**: un `.bex` con una reloc fuera de su seccion es
rechazado **por el cargador** nombrando el offset, y no por el compilador --que
ese caso ya lo cubria--.

## C6 -- los parsers de dispositivo no tienen sonda

**Que falta**: informes HID del USB, tablas del firmware (MADT), y la GPT se
parsean **sin desconfiar del que los emite**. `sonda.bex` empuja los syscalls y
nada empuja esto.

**Que la bloquea**: que un informe HID malo hay que **inyectarlo**, y eso pide
o un aparato programable o un camino de pruebas dentro del kernel. Es la casilla
mas cara de todas las de este plan y por eso va la ultima.

★ Y el limite que ya estaba escrito: **la sonda la escribio el mismo lado que
escribio las defensas**, asi que prueba lo que se nos ocurrio atacar. La primera
prueba que no se escribe uno mismo llega con la RED.

## C7 -- compilacion reproducible, y el precedente que ya la empezo

**Que falta**: que el mismo fuente de al mismo binario en dos maquinas
distintas, para que un artefacto se pueda comparar y, con el tiempo, para poder
hacer *diverse double-compiling* al toolchain.

★★ **Y esto no arranca de cero: MAQUETA ya se llevo la leccion.** El emisor
recibia la ruta *tal como se tecleo*, asi que generar la misma cara con ruta
relativa y con ruta absoluta daba **dos artefactos distintos**. La frase que
salio de ahi vale para todo el arbol:

> **un artefacto que depende de quien lo genera NO SE PUEDE COMPARAR**

**Como se sabe que quedo hecha**: un `.bex` construido dos veces da el mismo
BLAKE3.

---

# 3. EL ORDEN, y por que es ese

```
   [X] C1  el stub que dice que si      HECHA 24-08. Y sin `bool`, que es mas
   [X] C2  UMIP y SMEP                  ya estaban puestos. La tabla mentia
   [X] C2  NX / W^X y SMAP              HECHAS 25-08. SMAP costo dos caminos
   --------------------------------------------------------------------------
   C5  el gate en el CARGADOR           el compilador ya lo hace; el kernel no
   C3  Ed25519                          LA pieza; dias, y con vectores
   C4  la capability de EJECUTAR        pide resolver delegacion
   C6  la sonda de dispositivos         la mas cara; despues de la red
```

★★ **Y el orden cambio de dueno: hoy la que manda es C3.** No por gravedad, sino
porque **es la unica que ya no esta sola**: `platform/shared/bmo-cripto` existe
desde el 24-08 con SHA-256, HMAC, HKDF, X25519 y AES-GCM, todos contra sus
vectores oficiales. Ed25519 pide SHA-512 y aritmetica de Edwards -- y `campo25519.rs`,
que es la parte que asusta, **ya esta escrita y probada** para X25519.

> El dia que se escribio esta lista, C3 era *"LA pieza"* y no habia ni un hash en
> el arbol. Hoy le falta **la mitad de una pieza**.

★ **El criterio no es la gravedad: es lo que cuesta descubrir el fallo mas
tarde.** C1 y C5 son casi gratis y las dos evitan que algo pase inadvertido; C6
es la mas grave en abstracto y la ultima, porque hoy no hay forma barata de
probarla.

---

# 4. LO QUE NO ESTA EN ESTE PLAN, dicho para que no parezca un olvido

- **ASLR** -- ver 4.4 del maestro. Compra poco sin red y sin usuarios, y no es lo
  mismo que W^X aunque se citen juntos.
- **Arranque medido / Secure Boot** -- rechazado con motivo (4.3 del maestro):
  encadena BMO-X a la llave de otro.
- **Cifrado del volumen** -- fuera del alcance acotado de ESTRATOS, que ya lo
  declara junto a RAID, cuotas y ACLs.
- **Cualquier cosa de red** -- ⚠ **y esto es lo que acaba de caducar.** Este plan
  decia *"la superficie es cero hoy. Cuando deje de serlo, este plan se relee
  entero: es el unico cambio que mueve todas las casillas a la vez."*

  **Dejo de serlo el 2026-08-25.** BMO-X leyo 16 tramas y 7.967 bytes que puso
  otra maquina en el cable. La condicion que este documento escribio para
  releerse entero **se cumplio**, y el aviso queda aqui puesto por su propia
  regla.

  ★ Lo que todavia la acota, y hay que decirlo para no asustar de mas: **no se
  transmite** --un fallo no puede molestar a nadie mas de la red-- y el DMA de
  la tarjeta va contra un corral acotado. O sea que hay superficie de
  **entrada**, que es la que importa: bytes de un tercero parseados por codigo
  propio, que es la definicion de C6 y ya no es hipotetica.

  [!] **La relectura entera NO se hizo en esta pasada, a peticion del dueno**
  (*"no toques en RED"*). Queda como la primera casilla de la siguiente.
