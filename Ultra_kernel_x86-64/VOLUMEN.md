# VOLUMEN.md -- el volumen de datos de BMO-X

Esto es el **espejo** de lo que hay que copiar al Kingston (`BMO-DATA`). Lo
genera `build.ps1` entero, incluidos los datos de los ejemplos.

```
BMO-DATA/
  sys/      gui.bex                     el sistema: lo que arranca solo
  cobol/    banco calc calcgui extracto batch concep carter
  c/        holac scrollc pregc
  ada/      cierre
  datos/    movim.txt concs.txt imps.txt grande.txt
            salida.txt   <- lo escribe LA MAQUINA, no el deploy
            ctas.bin     <- idem: lo crea el nivel 10 de COBOL
```

## ★ Lo que aparece aqui SOLO, y por que importa

`datos/` es de doble sentido: el deploy pone las entradas de los ejemplos, y
**BMO-X deja ahi lo que produce**.

- **`salida.txt`** -- el historial del escritorio. Se escribe solo cada vez que
  termina un programa lanzado desde `Ejecutar`, y a mano con `guarda [ruta]`.
- **`ctas.bin`** -- el fichero maestro que escribe `cobol/10/maestro.bex`.

Eso convierte el bucle de depuracion. Antes era *flashear y hacerle una foto a
la pantalla*: no se compara con la de ayer, no se busca dentro, y no se le puede
ensenar a nadie que no este delante de la maquina. Ahora se arranca BMO-X, se
corre lo que sea, se apaga, se enchufa el disco a un Windows y **se abre el
`.txt`**.

★ Y por eso vive en esta particion y no en ESTRATOS, aunque ESTRATOS sea el
sistema de ficheros bueno: **ningun otro sistema operativo sabe leer ESTRATOS**.
Un volcado que solo BMO puede abrir no resuelve el problema por el que se
escribio. FAT32 aqui no es deuda: es el idioma comun.

⚠ El deploy **no borra lo que no puso el**, asi que estos archivos sobreviven a
un `-Data`. Si un `salida.txt` parece viejo, es que la corrida no llego a
guardarlo -- y entonces la maquina lo dijo en pantalla y el motivo esta en `F11`.

## Por que esta partido asi

Antes era **una sola carpeta `apps/`** con los diecisiete ficheros revueltos:
los siete `.bex` de COBOL, los de C, el de Ada, el compositor y los `.txt` de
entrada. Un `ls` daba diecisiete lineas sin orden, y para lanzar algo habia que
acordarse del nombre exacto.

La primera division es la que importa: **programa o dato**. Dentro de los
programas, por quien los compila -- que es como se busca cuando estas trabajando
en un lenguaje concreto.

★ Y se teclea **menos** que antes: `cobol/banco.bex` es mas corto que
`apps/banco.bex`. Ordenar no ha costado tecleo, lo ha ahorrado.

## Reglas que no se pueden romper

- **Todo nombre es 8.3**, carpetas incluidas. El driver FAT32 del kernel se
  NIEGA a recortar, y una carpeta recortada manda a otro sitio igual que un
  fichero recortado abre otro archivo.
- **`sys/gui.bex` es un contrato.** `RUTA_COMPOSITOR` de `phase.rs` dice esa
  ruta exacta; si una de las dos cambia sin la otra, el escritorio no arranca y
  la maquina se queda en el panel del kernel.
- **Los `ASSIGN TO` de COBOL llevan la ruta dentro del `.bex`.** Mover un `.txt`
  obliga a recompilar el programa que lo lee. Por eso `datos/` es una decision y
  no una preferencia: cambiarla otra vez cuesta recompilar.

## Al actualizar el disco

`build.ps1 -Data <letra>` copia recursivo y crea las carpetas que falten, pero
**no borra nada**: un deploy no tiene derecho a decidir sobre lo que no puso el.

La primera vez tras este cambio hay que **borrar la vieja `apps\` del Kingston a
mano**, o quedan dos copias de cada programa y la de `apps\` es la vieja.
