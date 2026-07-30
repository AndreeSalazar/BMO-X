      * CALCULADORA — COBOL sobre BMO-X, en metal desnudo.
      *
      * Lee dos importes por teclado, los suma, y ensena el total. Suena a
      * poco y es la cadena entera de este sistema funcionando de una punta a
      * la otra:
      *
      *   el teclado USB (xHCI+HID, Ring 0)
      *     -> KIND_INPUT, cedido al compositor
      *       -> el terminal de Ring 3, que teclea el usuario
      *         -> KIND_CONSOLE, anillo de ENTRADA
      *           -> ACCEPT, en un programa que NO tiene la capability
      *              del teclado y no le hace falta
      *             -> aritmetica decimal EXACTA en centavos
      *               -> DISPLAY, formateado en ejecucion
      *                 -> KIND_CONSOLE, anillo de SALIDA
      *                   -> el terminal otra vez, a la pantalla
      *
      * Y el decimal es exacto. `0.10 + 0.20` da `0.30`, no `0.30000000000004`
      * — porque aqui no hay coma flotante en ningun sitio: los importes viven
      * como enteros de centavos y sumar centavos es sumar enteros. Eso es lo
      * que Grace Hopper puso en COBOL y por lo que los bancos siguen usandolo.
      *
      * Compilar:
      *     cargo run -p bmo-cobol-front -- toolchain/lang/cobol/examples/calc.cob
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 UNO    PIC S9(7)V99.
       01 DOS    PIC S9(7)V99.
       01 TOTAL  PIC S9(7)V99.
       PROCEDURE DIVISION.
           DISPLAY "calculadora COBOL - importes con dos decimales".
           DISPLAY "primer importe:".
           ACCEPT UNO.
           DISPLAY "segundo importe:".
           ACCEPT DOS.

           MOVE UNO TO TOTAL.
           ADD DOS TO TOTAL.

           DISPLAY "suma:".
           DISPLAY TOTAL.

           COMPUTE TOTAL = UNO - DOS.
           DISPLAY "resta:".
           DISPLAY TOTAL.

           STOP RUN.
