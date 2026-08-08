      * CUENTAS -- COMP-3, el formato en el que estan los datos de un banco.
      *
      * Los seis ejemplos anteriores calculan bien y presentan bien, y todos
      * guardan el dinero igual: un entero de 64 bits con la escala de su PIC.
      * Eso vale para el COBOL que se escribe HOY. No vale para el que ya
      * existe, porque un banco no guarda importes asi: los guarda
      * EMPAQUETADOS, dos digitos por byte y el signo en el ultimo nibble, y
      * lleva cuarenta anos haciendolo.
      *
      *   SALDO = -1234,56 en PIC S9(5)V99 COMP-3  ->  4 bytes
      *
      *     byte 0    byte 1    byte 2    byte 3
      *    +----+----+----+----+----+----+----+----+
      *    | 0  | 0  | 1  | 2  | 3  | 4  | 5  | D  |   D = negativo
      *    +----+----+----+----+----+----+----+----+
      *      ^                                  ^
      *      relleno                        el SIGNO, no un digito
      *
      * Lo que este programa demuestra, y que no se puede fingir:
      *
      *   1. El campo mide LO QUE DICE SU PICTURE. Un PIC 9(3) COMP-3 son dos
      *      bytes y guarda tres digitos: lo que no cabe se pierde por arriba,
      *      que es lo que manda el estandar. Un DISPLAY, hoy, se los queda
      *      todos - y por eso las dos lineas de abajo salen distintas.
      *
      *   2. El signo VUELVE. Un campo con S que perdiera el nibble
      *      convertiria un cargo en un abono.
      *
      *   3. Un campo SIN S guarda el VALOR ABSOLUTO y marca F. No es un
      *      detalle de formato: es la diferencia entre una cuenta que puede
      *      estar en rojo y una que no.
      *
      *   4. Empaquetado y DISPLAY se mezclan en la misma cuenta, porque la
      *      aritmetica sigue viendo el entero escalado. El decimal exacto no
      *      se entera de la representacion, y ese es justo el reparto.
      *
      * Lo que TODAVIA no es: el fichero de aqui abajo sigue siendo texto, un
      * numero por linea. Los registros BINARIOS -leer los bytes empaquetados
      * tal cual vienen del mainframe- son el paso siguiente, y piden un
      * registro con varios campos. Ver BANCA_REAL.md.
      *
      * Compilar:
      *   cargo run -p bmo-cobol-front -- \
      *     toolchain/lang/cobol/examples/7-empaquetado/cuentas.cob \
      *     -o apps/cuentas.bex
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CUENTAS.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT MOVIM ASSIGN TO "datos/movim.txt".
       DATA DIVISION.
       FILE SECTION.
       FD MOVIM.
       01 IMPORTE   PIC S9(7)V99 COMP-3.
       WORKING-STORAGE SECTION.
       01 SALDO     PIC S9(7)V99 COMP-3.
       01 CORTO     PIC 9(3) COMP-3.
       01 ANCHO     PIC 9(3).
       01 EN-ROJO   PIC S9(5)V99 COMP-3.
       01 SIN-ROJO  PIC 9(5)V99 COMP-3.
       01 COMISION  PIC 9(3)V99.
       01 FIN       PIC 9.
       01 LINEA     PIC $$$,$$9.99.
       PROCEDURE DIVISION.
           DISPLAY "CUENTAS - DECIMAL EMPAQUETADO".

      *    -- 1. El ancho del campo es el de su PICTURE --
      *    Los dos reciben 12345. El empaquetado tiene sitio para TRES
      *    digitos y se queda con 345; el DISPLAY, hoy, se los queda todos.
      *    Si algun dia estas dos lineas salen iguales, el COMP-3 dejo de
      *    guardar nibbles y volvio a ser un entero con otro nombre.
           MOVE 12345 TO CORTO.
           MOVE 12345 TO ANCHO.
           DISPLAY "empaquetado de 3 digitos:".
           DISPLAY CORTO.
           DISPLAY "el mismo dato sin empaquetar:".
           DISPLAY ANCHO.

      *    -- 2. El signo vuelve --
           MOVE 0 TO EN-ROJO.
           SUBTRACT 1234.56 FROM EN-ROJO.
           DISPLAY "una cuenta en rojo:".
           DISPLAY EN-ROJO.

      *    -- 3. Sin S no hay rojo: el campo guarda el valor absoluto --
           MOVE 0 TO SIN-ROJO.
           SUBTRACT 1234.56 FROM SIN-ROJO.
           DISPLAY "el mismo importe en un campo sin signo:".
           DISPLAY SIN-ROJO.

      *    -- 4. El batch: se totaliza empaquetado y cuadra --
      *    El registro del fichero tambien es COMP-3, asi que cada importe
      *    leido pasa por los nibbles antes de sumarse. Si el empaquetado
      *    perdiera un centimo, el total lo diria.
           MOVE 0 TO SALDO.
           MOVE 0 TO FIN.
           OPEN INPUT MOVIM.
           PERFORM UNTIL FIN = 1
               READ MOVIM
                   AT END MOVE 1 TO FIN
                   NOT AT END ADD IMPORTE TO SALDO
               END-READ
           END-PERFORM.
           CLOSE MOVIM.

      *    Y la comision, que es un DISPLAY: se mezcla con el empaquetado en
      *    la misma cuenta porque la aritmetica ve enteros escalados y no
      *    sabe -ni le importa- como se guarda cada uno.
           MOVE 1.50 TO COMISION.
           COMPUTE SALDO = SALDO - COMISION.

           DISPLAY "saldo tras el cierre, menos comision:".
           MOVE SALDO TO LINEA.
           DISPLAY LINEA.
           STOP RUN.
