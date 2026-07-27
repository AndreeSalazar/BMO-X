# Ultra_userspace — el Ring 3 de BMO-X

Todo lo que corre **fuera del kernel**: el compositor, los servicios y las
aplicaciones. Hermano de `../Ultra_kernel_x86-64/`, no parte de él.

> Este README describió durante meses otro sistema —hablaba de un cargador ELF,
> de syscalls que eran stubs y de un kernel que "sólo se para"— porque venía de
> antes de BMO-X. Nada de aquello es cierto ya. Si algo de lo que sigue deja de
> serlo, corrígelo aquí: un README que miente es peor que no tenerlo.

## Lo único que hay que entender

Un proceso Ring 3 de BMO **no recibe la estructura de arranque del kernel**. No
sabe cuánta RAM hay, ni dónde está el framebuffer, ni qué discos existen.
Recibe *capabilities*: cada una es un permiso concreto sobre un objeto
concreto, y lo que no le hayan dado no existe para él.

Y la superficie son **tres syscalls**, congelados:

```
INVOKE(cap, operacion, a0, a1, a2)   la puerta síncrona
CHANNEL_KICK(cap, secuencia)         avisar al consumidor
WAIT(esperable, visto, timeout_ns)   bloquearse
```

Todo lo demás —abrir un endpoint, escribir en consola, reclamar la pantalla— es
una *operación* sobre una capability. La API crece en la pareja `(tipo de
objeto, operación)`; el ABI no se toca. Añadir "abrir ventana" no es cambiar la
frontera: es un número más en una tabla.

## Cómo llega esto a ejecutarse

Hasta hace poco no llegaba. **No existía forma de convertir un crate de Rust en
algo que BMO pudiera admitir**: todos los `.bex` salían de emisores de bytes
x86 escritos a mano o de los compiladores propios de C y COBOL. Por eso este
directorio estuvo lleno de stubs vacíos — no por dejadez, sino porque no había
tubería que los convirtiera en nada.

Ahora la hay:

```
cargo build --target x86_64-unknown-none    →  ELF
bex-link  <elf>  <salida.bex>               →  contenedor BEF
kernel: bex::inspect → espacio propio → Ring 3
```

`bex-link` vive en `../toolchain/tools/bex-link`. Lo llama `build.ps1` **antes**
de compilar el kernel, porque el kernel embebe el `.bex` con `include_bytes!`.

### El contrato de direcciones

El kernel decide dónde va cada sección: las coloca secuencialmente desde
`USER_IMAGE_BASE` (0x4000_0000), respetando la alineación declarada y avanzando
por páginas enteras. **No hay reubicación.** Lo que el enlazador escriba como
dirección absoluta tiene que caer donde el kernel lo mapee.

Por eso existe `userland/link.ld`, que reproduce esa colocación exacta, y por
eso `bex-link` compara las dos sección por sección y **se planta** si no
coinciden. Un desajuste ahí no da un error bonito: da un programa que carga,
salta al vacío y muere con un `#UD` en Ring 3.

## Crates

| Crate | Estado | Qué es |
|---|---|---|
| `userland` (`bmo-userland`) | **vivo** | El runtime: los tres syscalls, `Status`, `Pantalla`, consola. Sin dependencias, a propósito. |
| `services/gui` (`compositor`) | **vivo** | Reclama `KIND_FRAMEBUFFER`, pinta y no termina. Es el binario que arranca el kernel. |
| `services/input` | stub | Multiplexor de teclado y ratón. Espera a que el compositor sepa atender clientes. |
| `apps/launcher` | stub | Shell del escritorio. |
| `apps/terminal` | stub | Emulador de terminal. |

Los tres stubs siguen siendo stubs, pero ya **pueden** dejar de serlo: el
camino existe. Antes no.

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

`Ultra_userspace/` es su **propio workspace** y no es miembro del de la raíz:
sus crates se compilan a `x86_64-unknown-none` con `link.ld`, y arrastrar esos
rustflags a un workspace lleno de herramientas que corren en Windows no tendría
ningún sentido.

## Lo que falta, en orden

1. **El compositor atiende clientes.** Ya existe el mecanismo — Endpoint RPC
   funciona y se ve en hardware. Falta que el compositor abra su ventanilla y
   defina las operaciones: crear superficie, presentar, mover.
2. **Ratón.** El xHCI lo enumera; el puntero es territorio del compositor.
3. **Autoridad sobre la pantalla.** Hoy `KIND_FRAMEBUFFER` la concede al primero
   que la pide. Lo correcto es una bandera en el contenedor BEF verificada por
   el gate al admitir el programa. Está anotado en `ring0/fb.rs`.
4. **Recuperar memoria al salir.** `proc.rs` v1 mantiene vivos los frames de
   sección durante toda la vida del sistema. Un escritorio que abre y cierra
   ventanas necesita que eso deje de ser así.
