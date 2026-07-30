      * El primer "hola mundo" en COBOL que BMO-X ejecuta de verdad.
      *
      * DISPLAY no llama a un runtime COBOL ni al sistema anfitrion: baja a
      * la puerta generica (bmo-lower) y de ahi al unico syscall que existe,
      * INVOKE(CURRENT_TASK, CONSOLE_WRITE). Cada DISPLAY termina en salto de
      * linea, asi que ocupa su propia fila en el log del kernel.
      *
      * Compilar:
      *     cargo run -p bmo-cobol-front -- toolchain/lang/cobol/examples/hola.cob
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HOLA.
       PROCEDURE DIVISION.
           DISPLAY "BMO-X: hola mundo desde COBOL".
           DISPLAY "COBOL -> puerta L1 -> INVOKE -> Ring 0".
           STOP RUN.
