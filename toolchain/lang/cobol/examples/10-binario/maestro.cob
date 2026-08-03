      * MAESTRO — el FICHERO DE UN BANCO, en su formato de verdad.
      *
      * Los nueve niveles anteriores leen y escriben TEXTO: una linea, un
      * numero. Un banco no da eso. Da registros de LARGO FIJO con los campos en
      * su byte y los importes empaquetados, y ese fichero NO SE PUEDE MIRAR con
      * un `cat`: los nibbles de un COMP-3 no son texto.
      *
      * Este es el escalon que separa "COBOL que compila" de "COBOL que abre los
      * datos que ya tienes".
      *
      * ── El registro, byte a byte ──
      *
      *   01 REG-CUENTA.                        desde  bytes  como
      *       05 CTA-NUMERO PIC 9(10).              0     10  zonado
      *       05 CTA-SALDO  PIC S9(7)V99 COMP-3.   10      5  nibbles
      *       05 CTA-ESTADO PIC 9.                 15      1  zonado
      *                                          ─────────────
      *                                   REG-CUENTA:    16
      *
      * Diecisiete bytes NO: dieciseis. Un registro va PEGADO, sin relleno,
      * porque esto es el formato del fichero y un byte de padding es un byte
      * que aparece en el disco.
      *
      * Y sin salto de linea al final: un registro de largo fijo no lleva
      * separador. El que lo lea ya sabe cuanto mide, y un `\n` correria todo lo
      * de detras un byte.
      *
      * ── Las dos herramientas que van con esto ──
      *
      *   bmo-cobol --copybook maestro.cob
      *       El byte exacto de cada campo. En banca ese documento es lo que se
      *       intercambia para que dos sistemas lean el mismo fichero, y el que
      *       se mantiene a mano SIEMPRE acaba mintiendo. Este no puede: sale de
      *       la misma tabla que emite el READ y el WRITE.
      *
      *   bmo-cobol --ver datos/ctas.bin maestro.cob
      *       El fichero DECODIFICADO, con el valor y los bytes crudos al lado.
      *       Y lee con la misma regla con la que este programa escribio.
      *
      * Compilar:
      *   cargo run -p bmo-cobol-front -- \
      *     toolchain/lang/cobol/examples/10-binario/maestro.cob -o apps/maestro.bex
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAESTRO.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT SALIDA  ASSIGN TO "datos/ctas.bin".
           SELECT ENTRADA ASSIGN TO "datos/ctas.bin".
       DATA DIVISION.
       FILE SECTION.
       FD SALIDA.
       01 REG-OUT.
           05 O-NUMERO PIC 9(10).
           05 O-SALDO  PIC S9(7)V99 COMP-3.
           05 O-ESTADO PIC 9.
       FD ENTRADA.
       01 REG-IN.
           05 I-NUMERO PIC 9(10).
           05 I-SALDO  PIC S9(7)V99 COMP-3.
           05 I-ESTADO PIC 9.
       WORKING-STORAGE SECTION.
       01 TOTAL    PIC S9(9)V99 COMP-3 VALUE ZERO.
       01 CUANTAS  PIC 9(5)            VALUE ZERO.
       01 EN-ROJO  PIC 9(5)            VALUE ZERO.
       01 FIN      PIC 9               VALUE ZERO.
           88 SE-ACABO                 VALUE 1.
      * ★ La mascara lleva CR, y eso NO es decoracion. Con `$$$,$$9.99` a
      * secas, un saldo de -890,10 sale impreso como `$890.10`: el numero es
      * negativo por dentro y el extracto dice que no. Un campo editado sin
      * simbolo de signo NO ENSENA EL SIGNO, y ese es el fallo que convierte un
      * descubierto en un abono a ojos de quien lee el papel.
       01 LINEA    PIC ZZ,ZZ9.99CR.
       PROCEDURE DIVISION.

           PERFORM 1000-ESCRIBIR-MAESTRO.
           PERFORM 2000-LEER-Y-CUADRAR.
           PERFORM 3000-INFORME.
           STOP RUN.

       1000-ESCRIBIR-MAESTRO.
           DISPLAY "BANCO BMO - MAESTRO DE CUENTAS".
           DISPLAY "------------------------------".
           OPEN OUTPUT SALIDA.

           MOVE 4471998200 TO O-NUMERO.
           MOVE 15234.75   TO O-SALDO.
           MOVE 1          TO O-ESTADO.
           WRITE REG-OUT.

           MOVE 4471998201 TO O-NUMERO.
           MOVE -890.10    TO O-SALDO.
           MOVE 2          TO O-ESTADO.
           WRITE REG-OUT.

           MOVE 4471998202 TO O-NUMERO.
           MOVE 3105.40    TO O-SALDO.
           MOVE 1          TO O-ESTADO.
           WRITE REG-OUT.

           CLOSE SALIDA.
           DISPLAY "escritas 3 cuentas de 16 bytes".

       2000-LEER-Y-CUADRAR.
      *    Y ahora se vuelve a leer el MISMO fichero. Que los importes salgan
      *    iguales es lo que prueba que empaquetar y desempaquetar dicen lo
      *    mismo — un ida y vuelta que no cuadra es un descuadre en el disco.
           OPEN INPUT ENTRADA.
           PERFORM UNTIL SE-ACABO
               READ ENTRADA
                   AT END MOVE 1 TO FIN
                   NOT AT END PERFORM 2100-UNA-CUENTA
               END-READ
           END-PERFORM.
           CLOSE ENTRADA.

       2100-UNA-CUENTA.
           ADD I-SALDO TO TOTAL.
           ADD 1 TO CUANTAS.
           IF I-SALDO < 0
               ADD 1 TO EN-ROJO
           END-IF.
           DISPLAY "cuenta:".
           DISPLAY I-NUMERO.
           DISPLAY "  saldo:".
           MOVE I-SALDO TO LINEA.
           DISPLAY LINEA.

       3000-INFORME.
           DISPLAY "------------------------------".
           DISPLAY "cuentas leidas:".
           DISPLAY CUANTAS.
           DISPLAY "en descubierto:".
           DISPLAY EN-ROJO.
           DISPLAY "saldo total:".
           MOVE TOTAL TO LINEA.
           DISPLAY LINEA.
