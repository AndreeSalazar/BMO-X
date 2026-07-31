# VOLUMEN.md — el volumen de datos de BMO-X

Esto es el **espejo** de lo que hay que copiar al Kingston (`BMO-DATA`). Lo
genera `build.ps1` entero, incluidos los datos de los ejemplos.

```
BMO-DATA/
  sys/      gui.bex                     el sistema: lo que arranca solo
  cobol/    banco calc calcgui extracto batch concep carter
  c/        holac scrollc pregc
  ada/      cierre
  datos/    movim.txt concs.txt imps.txt grande.txt
```

## Por qué está partido así

Antes era **una sola carpeta `apps/`** con los diecisiete ficheros revueltos:
los siete `.bex` de COBOL, los de C, el de Ada, el compositor y los `.txt` de
entrada. Un `ls` daba diecisiete líneas sin orden, y para lanzar algo había que
acordarse del nombre exacto.

La primera división es la que importa: **programa o dato**. Dentro de los
programas, por quién los compila — que es como se busca cuando estás trabajando
en un lenguaje concreto.

★ Y se teclea **menos** que antes: `cobol/banco.bex` es más corto que
`apps/banco.bex`. Ordenar no ha costado tecleo, lo ha ahorrado.

## Reglas que no se pueden romper

- **Todo nombre es 8.3**, carpetas incluidas. El driver FAT32 del kernel se
  NIEGA a recortar, y una carpeta recortada manda a otro sitio igual que un
  fichero recortado abre otro archivo.
- **`sys/gui.bex` es un contrato.** `RUTA_COMPOSITOR` de `phase.rs` dice esa
  ruta exacta; si una de las dos cambia sin la otra, el escritorio no arranca y
  la máquina se queda en el panel del kernel.
- **Los `ASSIGN TO` de COBOL llevan la ruta dentro del `.bex`.** Mover un `.txt`
  obliga a recompilar el programa que lo lee. Por eso `datos/` es una decisión y
  no una preferencia: cambiarla otra vez cuesta recompilar.

## Al actualizar el disco

`build.ps1 -Data <letra>` copia recursivo y crea las carpetas que falten, pero
**no borra nada**: un deploy no tiene derecho a decidir sobre lo que no puso él.

La primera vez tras este cambio hay que **borrar la vieja `apps\` del Kingston a
mano**, o quedan dos copias de cada programa y la de `apps\` es la vieja.
