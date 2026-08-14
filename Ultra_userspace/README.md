# Ultra_userspace -- el Ring 3 de BMO-X

Todo lo que corre **fuera del kernel**: el compositor, los servicios y las
aplicaciones. Hermano de `../Ultra_kernel_x86-64/`, no parte de el.

> Este README describio durante meses otro sistema --hablaba de un cargador ELF,
> de syscalls que eran stubs y de un kernel que "solo se para"-- porque venia de
> antes de BMO-X. Nada de aquello es cierto ya. Si algo de lo que sigue deja de
> serlo, corrigelo aqui: un README que miente es peor que no tenerlo.

## Lo unico que hay que entender

Un proceso Ring 3 de BMO **no recibe la estructura de arranque del kernel**. No
sabe cuanta RAM hay, ni donde esta el framebuffer, ni que discos existen.
Recibe *capabilities*: cada una es un permiso concreto sobre un objeto
concreto, y lo que no le hayan dado no existe para el.

Y la superficie son **dos syscalls**, congelados:

```
INVOKE(cap, operacion, a0, a1, a2)   la puerta sincrona
CHANNEL_KICK(cap, secuencia)         avisar al consumidor
WAIT(esperable, visto, timeout_ns)   bloquearse
```

Todo lo demas --abrir un endpoint, escribir en consola, reclamar la pantalla-- es
una *operacion* sobre una capability. La API crece en la pareja `(tipo de
objeto, operacion)`; el ABI no se toca. Anadir "abrir ventana" no es cambiar la
frontera: es un numero mas en una tabla.

## Como llega esto a ejecutarse

Hasta hace poco no llegaba. **No existia forma de convertir un crate de Rust en
algo que BMO pudiera admitir**: todos los `.bex` salian de emisores de bytes
x86 escritos a mano o de los compiladores propios de C y COBOL. Por eso este
directorio estuvo lleno de stubs vacios -- no por dejadez, sino porque no habia
tuberia que los convirtiera en nada.

Ahora la hay:

```
cargo build --target x86_64-unknown-none    ->  ELF
bex-link  <elf>  <salida.bex>               ->  contenedor BEF
kernel: bex::inspect -> espacio propio -> Ring 3
```

`bex-link` vive en `../toolchain/tools/bex-link`. Lo llama `build.ps1` **antes**
de compilar el kernel, porque el kernel embebe el `.bex` con `include_bytes!`.

### El contrato de direcciones

El kernel decide donde va cada seccion: las coloca secuencialmente desde
`USER_IMAGE_BASE` (0x4000_0000), respetando la alineacion declarada y avanzando
por paginas enteras. **No hay reubicacion.** Lo que el enlazador escriba como
direccion absoluta tiene que caer donde el kernel lo mapee.

Por eso existe `userland/link.ld`, que reproduce esa colocacion exacta, y por
eso `bex-link` compara las dos seccion por seccion y **se planta** si no
coinciden. Un desajuste ahi no da un error bonito: da un programa que carga,
salta al vacio y muere con un `#UD` en Ring 3.

## Crates

| Crate | Estado | Que es |
|---|---|---|
| `userland` (`bmo-userland`) | **vivo** | El runtime: los dos syscalls, `Status`, `Pantalla`, consola. Sin dependencias, a proposito. |
| `services/gui` (`compositor`) | **vivo** | Reclama `KIND_FRAMEBUFFER`, pinta y no termina. Es el binario que arranca el kernel. |
| `services/input` | stub | Multiplexor de teclado y raton. Espera a que el compositor sepa atender clientes. |

[!] **Aqui habia dos filas mas, y las dos mentian** (borradas el 2026-08-13):

- `apps/launcher`, *"shell del escritorio"*. Eran **nueve lineas** que hacen
  `hlt` para siempre, y su trabajo ya estaba hecho: el lanzador de verdad son
  440 lineas en `services/gui/src/scene/launcher.rs`, lee `apps\` del disco,
  saca el icono de dentro de cada `.bex` y lanza al pulsar. Un crate vacio con
  el nombre del que si funciona es peor que no tener nada: manda a leer el
  fichero equivocado.
- `apps/terminal`, *"emulador de terminal"*. **Esa carpeta no existia.**

★ Y el que se queda, `services/input`, si es un stub honesto: 50 lineas
esperando a que el compositor sepa atender clientes.

** La leccion es la de siempre en esta casa: una carpeta es una PROMESA. Si se
llama `apps/` dice que ahi viven las aplicaciones de Ring 3, y lo que habia era
un `hlt`. Las aplicaciones reales viven **en el disco** (`A:pps\`), llegan
como `.bex` con su icono dentro, y el escritorio las encuentra sola.

## Construir

```powershell
cd Ultra_userspace
cargo +nightly build -p bmo-service-gui --release --target x86_64-unknown-none
```

O, lo normal, dejar que lo haga la cadena entera:

```powershell
cd Ultra_kernel_x86-64
.\build.ps1 -BuildOnly
```

`Ultra_userspace/` es su **propio workspace** y no es miembro del de la raiz:
sus crates se compilan a `x86_64-unknown-none` con `link.ld`, y arrastrar esos
rustflags a un workspace lleno de herramientas que corren en Windows no tendria
ningun sentido.

## Lo que falta, en orden

1. **El compositor atiende clientes.** Ya existe el mecanismo -- Endpoint RPC
   funciona y se ve en hardware. Falta que el compositor abra su ventanilla y
   defina las operaciones: crear superficie, presentar, mover.
2. **Raton.** El xHCI lo enumera; el puntero es territorio del compositor.
3. **Autoridad sobre la pantalla.** Hoy `KIND_FRAMEBUFFER` la concede al primero
   que la pide. Lo correcto es una bandera en el contenedor BEF verificada por
   el gate al admitir el programa. Esta anotado en `ring0/fb.rs`.
4. **Recuperar memoria al salir.** `proc.rs` v1 mantiene vivos los frames de
   seccion durante toda la vida del sistema. Un escritorio que abre y cierra
   ventanas necesita que eso deje de ser asi.
