      * BATCH — el programa que justifica todo lo demas.
      *
      * Lee un fichero de movimientos, los totaliza en decimal EXACTO y
      * escribe el total en otro fichero. Eso es un proceso por lotes, y es
      * literalmente lo que un banco lleva sesenta anos haciendo de noche.
      *
      * Hasta ahora BMO COBOL sabia calcular y sabia presentar, y no tenia de
      * donde sacar los datos: OPEN/READ/WRITE/CLOSE se RECHAZABAN con un
      * error honesto porque no existia la capability. Ya existe.
      *
      * La cadena entera, de una punta a la otra:
      *
      *   el disco SATA (AHCI, Ring 0)
      *     -> FAT32, y el gate de identidad que deja escribir SOLO en datos
      *       -> KIND_ARCHIVO, una capability que a este programa le
      *          CONCEDIERON (no un nombre que adivino)
      *         -> OPEN INPUT / READ ... AT END
      *           -> aritmetica decimal EXACTA en centavos, sin coma flotante
      *             -> WRITE, y el CLOSE que lo lleva al disco
      *
      * El AT END no es adorno: es lo unico que puede parar el PERFORM. Un
      * READ sin el compilaria a un bucle que no termina, asi que el parser
      * lo exige.
      *
      * Compilar:
      *   cargo run -p bmo-cobol-front -- \
      *     toolchain/lang/cobol/examples/batch.cob -o apps/batch.bex
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BATCH.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT MOVIM ASSIGN TO "datos/movim.txt".
           SELECT CIERRE ASSIGN TO "datos/cierre.txt".
       DATA DIVISION.
       FILE SECTION.
       FD MOVIM.
       01 IMPORTE  PIC S9(7)V99.
       FD CIERRE.
       01 TOTAL-F  PIC S9(7)V99.
       WORKING-STORAGE SECTION.
       01 TOTAL    PIC S9(7)V99.
       01 CUANTOS  PIC 9(5).
       01 FIN      PIC 9.
       01 LINEA    PIC $$$,$$9.99.
       PROCEDURE DIVISION.
           DISPLAY "BATCH DE CIERRE - BANCO BMO".

           MOVE 0 TO TOTAL.
           MOVE 0 TO CUANTOS.
           MOVE 0 TO FIN.

           OPEN INPUT MOVIM.
           PERFORM UNTIL FIN = 1
               READ MOVIM
                   AT END MOVE 1 TO FIN
                   NOT AT END ADD IMPORTE TO TOTAL
               END-READ
           END-PERFORM.
           CLOSE MOVIM.

      *    El total, con su mascara: esto es la linea del extracto.
           DISPLAY "total del dia:".
           MOVE TOTAL TO LINEA.
           DISPLAY LINEA.

      *    Y el cierre al disco. Sin el CLOSE no se guarda NADA: un proceso
      *    que muere a medias no deja medio fichero, deja ninguno.
           OPEN OUTPUT CIERRE.
           MOVE TOTAL TO TOTAL-F.
           WRITE TOTAL-F.
           CLOSE CIERRE.

           DISPLAY "cierre escrito en datos/cierre.txt".
           STOP RUN.
