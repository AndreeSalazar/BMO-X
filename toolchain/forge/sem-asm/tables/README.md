# tables/ -- DOS DUENOS EN UNA CARPETA, y cual abre cada cosa

> `tables/` es **la puerta de los terceros**: quien escribe C para BMO-X abre
> esta carpeta, y `$BMO_MODS` la tapa sin bifurcar el repo. La ley esta en
> [`META-SDK_HARD.md`](../../../../FUERO/META-SDK_HARD.md). Este fichero solo
> dice **que hay dentro y quien lo abre**, porque hasta el 2026-09-11 la raiz
> tenia trece cabeceras sueltas entre seis carpetas y no se veia.

```text
   LO QUE ABRE UN TERCERO -- la fabrica, vista desde fuera
   -------------------------------------------------------------------------
   bmo/            REX: las cabeceras con las que se escribe una app de BMO-X
   standards/C/    <stdio.h> <stdlib.h> <string.h> <stdint.h>... lo que C
                   PROMETE y BMO C cumple. Trece cabeceras, y al lado los
                   cXX.toml que dicen que admite cada version del estandar
   semantic/       el metal como libreria: <semantic/cpu.h>, atomico.h...
   stdlib/         los CUERPOS: heap, string, math. Modulos con BMO.toml

   LO QUE LEE EL COMPILADOR -- la fabrica, por dentro
   -------------------------------------------------------------------------
   arch/x86_64/    instructions.toml, intrinsics.toml, abi.toml: como se
                   codifica cada instruccion. Nadie los incluye: sem-asm los lee
   lang/inti/      las tablas propias de INTI
   standards/*.toml   (los cXX.toml de arriba, y los de COBOL y C++)
```

** El resolutor de C busca **primero** en `standards/C/` y despues en la raiz
(`lang/c/src/module.rs`, `discover_include_paths`). Ese orden llevaba escrito
desde el principio; lo que faltaba era poner las cabeceras donde el ya miraba.
El port de DOOM copio esa forma --`include/standards/C/`-- y por eso sus stubs
la tapan sin tocar nada de aqui.

[!] **Regla de la raiz: aqui no vive ningun fichero suelto.** Una cabecera en la
raiz no dice de quien es; en `standards/C/` dice *"esto lo promete C"*, en
`bmo/` dice *"esto lo da BMO-X"*, en `semantic/` dice *"esto es el silicio"*.

  > Una carpeta con dos duenos se ordena diciendo cual es cual, no juntando
  > mas cosas.
