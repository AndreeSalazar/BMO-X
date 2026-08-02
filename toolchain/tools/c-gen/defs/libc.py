"""La superficie de libc, por cabecera, **con destinatario**.

★ La columna que decide no es "existe en el estandar": es PARA QUE. Un
compilador acotado se define por lo que deja fuera, y una lista de libc sin
motivo al lado es una invitacion a implementarla entera — que es justo el fallo
que la hoja de ruta descarta con nombre propio ("prometer compatibilidad que no
existe").

Destinatarios:
    ESENCIA   lo que BMO necesita para lo suyo (banca, ficheros, consola).
    DOOM      lo que pide el objetivo de prueba. Concreto y comprobable.
    FUERA     existe en C y NO entra: se dice por que.
"""

# (nombre, cabecera, destinatario, motivo, sonda_o_None)
FUNCIONES = [
    # ── Salida y entrada de consola ──
    ("printf", "stdio.h", "ESENCIA", "ya esta: es lo primero que hizo BMO C",
     'int main(){printf("x");return 0;}'),
    ("getchar", "stdio.h", "ESENCIA", "ya esta: verificado en el Ryzen",
     "int main(){return getchar();}"),
    ("scanf", "stdio.h", "ESENCIA", "ya esta: pregc.bex pregunta la edad",
     'int main(){int n;scanf("%d",&n);return n;}'),
    ("puts", "stdio.h", "DOOM", "una linea y salto; trivial encima de printf",
     'int main(){puts("x");return 0;}'),
    ("sprintf", "stdio.h", "DOOM", "DOOM formatea en buffers, no solo en pantalla",
     'int main(){char b[8];sprintf(b,"%d",1);return b[0];}'),

    # ── Memoria ──
    ("malloc", "stdlib.h", "DOOM", "★ DOOM pide UN bloque grande (Z_Zone) y se lo administra el",
     "int main(){char*p=malloc(16);return p==0;}"),
    ("free", "stdlib.h", "DOOM", "pareja de malloc; con Z_Zone se llama poquisimo",
     "int main(){char*p=malloc(4);free(p);return 0;}"),
    ("memset", "string.h", "DOOM", "limpiar el framebuffer y las estructuras",
     "int main(){char b[4];memset(b,0,4);return b[0];}"),
    ("memcpy", "string.h", "DOOM", "★ el blit de cada fotograma pasa por aqui",
     "int main(){char a[4],b[4];memcpy(a,b,4);return a[0];}"),

    # ── Cadenas ──
    ("strlen", "string.h", "DOOM", "esencial en cuanto hay texto",
     'int main(){return strlen("abc");}'),
    ("strcmp", "string.h", "DOOM", "DOOM busca lumps del WAD por nombre",
     'int main(){return strcmp("a","a");}'),
    ("strcpy", "string.h", "DOOM", "pareja obligada de strcmp",
     'int main(){char b[8];strcpy(b,"a");return b[0];}'),

    # ── Numeros ──
    ("abs", "stdlib.h", "DOOM", "el render lo usa a manos llenas",
     "int main(){return abs(-3);}"),
    ("atoi", "stdlib.h", "DOOM", "parametros de linea de ordenes",
     'int main(){return atoi("12");}'),

    # ── Ficheros ──
    ("fopen/fread", "stdio.h", "DOOM", "★ el WAD son 4 MB. BMO ya tiene KIND_ARCHIVO",
     None),
    ("exit", "stdlib.h", "DOOM", "I_Quit. BMO ya sale por la puerta normal",
     "int main(){exit(0);}"),

    # ── Lo que NO entra, y por que ──
    ("pow/sin/cos", "math.h", "FUERA",
     "DOOM NO usa coma flotante en el render: son tablas de punto fijo", None),
    ("pthread_*", "pthread.h", "FUERA",
     "no hay hilos de usuario y no los pide el objetivo", None),
    ("setlocale", "locale.h", "FUERA",
     "una libc de verdad empieza aqui y no acaba nunca", None),
    ("wchar_t / wcs*", "wchar.h", "FUERA",
     "la consola de BMO es de un byte por caracter a proposito", None),
    ("signal", "signal.h", "FUERA",
     "no hay senales que mandar: aqui un fallo mata la tarea y lo dice", None),
    ("setjmp/longjmp", "setjmp.h", "FUERA",
     "DOOM no lo necesita y emitirlo pide guardar el marco entero", None),
]


def por_destinatario(dest):
    return [f for f in FUNCIONES if f[2] == dest]
