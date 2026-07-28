      * El MOTOR de la calculadora con botones. Sin adornos y sin preguntas.
      *
      * Lee tres numeros y escribe UNO. Nada mas. Ni un "introduzca el primer
      * operando", ni una cabecera: quien lo llama es un programa, no una
      * persona, y todo lo que salga por aqui es la respuesta.
      *
      *     entra:  operando1 \n  codigo \n  operando2 \n
      *     sale:   resultado \n
      *
      *     codigo: 1 sumar   2 restar   3 multiplicar   4 dividir
      *
      * ★ La cara la dibuja el compositor, en Rust. El CALCULO es esto, en
      * COBOL, con decimal exacto en centavos. Es la separacion que Windows no
      * hace —su calculadora lleva el motor dentro de la app— y es la que
      * permite cambiar la una sin tocar la otra.
      *
      * Compilar:
      *     cargo run -p bmo-cobol-front -- toolchain/lang/cobol/examples/calcgui.cob
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALCGUI.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 UNO    PIC S9(9)V99.
       01 DOS    PIC S9(9)V99.
       01 COD    PIC 9.
       01 RES    PIC S9(9)V99.
       PROCEDURE DIVISION.
           ACCEPT UNO.
           ACCEPT COD.
           ACCEPT DOS.

           MOVE 0 TO RES.

           IF COD = 1
               COMPUTE RES = UNO + DOS
           END-IF.
           IF COD = 2
               COMPUTE RES = UNO - DOS
           END-IF.
           IF COD = 3
               COMPUTE RES = UNO * DOS
           END-IF.
           IF COD = 4
               COMPUTE RES = UNO / DOS
           END-IF.

           DISPLAY RES.
           STOP RUN.
