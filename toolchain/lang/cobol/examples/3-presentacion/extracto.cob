      * EXTRACTO -- la linea de un banco, impresa por BMO COBOL.
      *
      * Los otros ejemplos demuestran que la CUENTA sale exacta. Este
      * demuestra la otra mitad, la que faltaba: que se puede IMPRIMIR.
      *
      * Un informe bancario no es mas que campos editados. El importe vive
      * como un entero de centavos --esa es la aritmetica, el alma de Grace
      * Hopper-- y la PICTURE de edicion lo convierte en la linea que sale por
      * la impresora: con su moneda, sus millares, sus ceros suprimidos y su
      * CR cuando el saldo esta en rojo.
      *
      *   PIC $$$,$$9.99   12345.67  ->  "$12,345.67"
      *   PIC **,**9.99         0.45  ->  "*****0.45"   (proteccion de cheque)
      *   PIC Z,ZZ9.99CR     -120.00  ->  "  120.00CR"
      *
      * Lo que hace esto BMO y no otra cosa: la plantilla se gasta AL
      * COMPILAR. En el .bex no hay ni una copia de la mascara ni un
      * interprete que la lea -- hay las instrucciones que hacen exactamente
      * lo que la mascara decia. GnuCOBOL para esto genera C y llama a gcc;
      * aqui va COBOL -> BEF, directo, y lo que corre en el Ryzen se puede
      * leer entero.
      *
      * Compilar:
      *   cargo run -p bmo-cobol-front -- \
      *     toolchain/lang/cobol/examples/extracto.cob \
      *     -o toolchain/lang/cobol/examples/extracto.bex
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EXTRACTO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
      *    Los datos de CALCULO: enteros escalados, sin coma flotante.
       01 SALDO     PIC S9(7)V99.
       01 CARGO     PIC S9(7)V99.
      *    Los campos de PRESENTACION. Guardan lo mismo; se ensenan distinto.
       01 L-SALDO   PIC $$$,$$9.99.
       01 L-CHEQUE  PIC **,**9.99.
       01 L-BALANCE PIC Z,ZZ9.99CR.
       PROCEDURE DIVISION.
           DISPLAY "BANCO BMO - EXTRACTO DE CUENTA".
           DISPLAY "-----------------------------".

      *    Movimientos del mes, en centavos exactos.
           MOVE 0 TO SALDO.
           MOVE 12000.00 TO CARGO.
           ADD CARGO TO SALDO.
           MOVE 345.67 TO CARGO.
           ADD CARGO TO SALDO.

           DISPLAY "saldo disponible:".
           MOVE SALDO TO L-SALDO.
           DISPLAY L-SALDO.

      *    Un talon pequeno: los huecos van con asterisco para que nadie
      *    pueda escribir una cifra encima. Esa es la razon de que `*` exista.
           DISPLAY "talon a cobrar:".
           MOVE 0.45 TO L-CHEQUE.
           DISPLAY L-CHEQUE.

      *    Y el descubierto. El CR solo aparece si el numero es negativo; en
      *    positivo son dos espacios, para que la columna no se descuadre.
           COMPUTE SALDO = SALDO - 12345.67.
           SUBTRACT 120.00 FROM SALDO.
           DISPLAY "balance final:".
           MOVE SALDO TO L-BALANCE.
           DISPLAY L-BALANCE.

           IF SALDO IS LESS THAN 0
               DISPLAY "cuenta en descubierto"
           ELSE
               DISPLAY "cuenta al corriente"
           END-IF.

           STOP RUN.
