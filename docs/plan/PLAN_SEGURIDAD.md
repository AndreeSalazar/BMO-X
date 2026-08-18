# PLAN SEGURIDAD -- las casillas que faltan, medidas contra el codigo

> Escrito el **2026-08-18**, auditando el arbol tal como esta hoy. El **por que**
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
| **`.bex` en FAT32** | nada: `veredicto` es `None` y el gate no corre | -- (limitacion del formato, ver C5) |
| **Syscalls** | capabilities con derechos, `sonda.bex` los empuja | `EJECUTAR` y `REINICIAR` sin atar |
| **Memoria del proceso** | separacion de anillos | **NX, SMEP, SMAP, UMIP: los cuatro apagados** |
| **Dispositivos** (HID, GPT, MADT) | se parsean sin desconfiar | ninguna sonda |
| **Red** | no existe | superficie CERO, y es la unica linea buena de la tabla |

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

## ★★ C1 -- `verify_ed25519` devuelve `true` a una firma de ceros

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

## ★★ C2 -- los cuatro bits que el CPU regala y nadie enciende

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

## C3 -- Ed25519 de verdad, que es LA pieza

**Que falta**: la aritmetica de curva (campo sobre `2^255-19` mas SHA-512) y el
`verify` encima. El formato **ya la espera**: `SigAlgorithm::Ed25519 = 1` y
`Ed25519Signature` (96 B) existen en `bef/signing.rs`, y `chain_hash` ya encadena
los hashes de seccion, que es exactamente lo que hay que firmar.

**Que la bloquea**: nada tecnico -- son dias de trabajo. Lo que la condiciona es
**que se pruebe contra los vectores oficiales (RFC 8032) o no vale nada**, y la
regla del maestro: ninguna constante entra sin su vector al lado.

⚠ Y una decision que hay que tomar antes de escribir una linea: **donde vive la
clave privada**. En la maquina no, o vuelve el problema de 4.2 del maestro. Vive
donde se firma, que es el anfitrion, y a BMO-X solo baja la publica.

**Como se sabe que quedo hecha**: los vectores del RFC pasan; un `.bex` firmado
arranca; el mismo con un byte cambiado **en la seccion de codigo** es rechazado
por firma y no por hash --que son dos rechazos distintos y hay que distinguirlos--;
y C1 ya no puede devolver `true` por omision.

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

## C5 -- `bmo-verify` existe y NO esta cableado a la construccion

**Donde**: la herramienta esta en `toolchain/forge/bmo-verify`. `grep` sobre
`build.ps1` y `bmo.ps1`: **cero menciones**.

**Que significa**: hay un verificador de `.bex` escrito y el build no lo llama
nunca. Un binario mal formado se descubre en el Ryzen y no en la maquina de
construir, que es donde sale barato.

**Que la bloquea**: nada. Es una linea en `build.ps1` sobre cada `.bex`
producido, igual que ya se hace con el ASCII y con la cara de MAQUETA.

**Como se sabe que quedo hecha**: se corrompe un `.bex` a proposito y **el build
falla**, no el arranque.

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
   C1  el stub que dice que si          cinco minutos, y tapa una trampa futura
   C5  cablear bmo-verify               una linea, y mueve fallos al build
   C2  UMIP y SMEP                      un bit cada uno
   C2  NX y SMAP                        piden repasar mapeos y copias
   C3  Ed25519                          LA pieza; dias, y con vectores
   C4  la capability de EJECUTAR        pide resolver delegacion
   C6  la sonda de dispositivos         la mas cara; despues de la red
```

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
- **Cualquier cosa de red** -- la superficie es cero hoy. Cuando deje de serlo,
  este plan se relee entero: es el unico cambio que mueve todas las casillas a la
  vez.
