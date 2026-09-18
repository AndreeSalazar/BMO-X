#!/usr/bin/env python3
"""escritores -- cada `static mut` del USB dice QUIEN lo escribe, y "los dos" solo puede bajar.

Por que existe
==============

El 2026-09-18 se escribio `docs/plan/PLAN_EL_BUS_APARTE.md`: el bus USB quiere
vivir en su propio nucleo. Y la primera pregunta de ese plan no tiene
respuesta sin leer el codigo entero: **de los 57 `static mut` de `dev/usb/`,
cuales toca el hilo del bus, cuales el escritorio desde su syscall, y cuales
LOS DOS**. Los dos son las carreras del dia en que haya dos nucleos.

Se leyeron los 57 a mano y se etiquetaron. Este guardian hace que la etiqueta
no caduque: un `static mut` nuevo sin etiqueta para el build, y la cuenta de
`ambos` es un trinquete -- solo puede bajar.

    [escribe] bus         solo el hilo del bus (`bus_thread`)
    [escribe] bombeo      quien bombee: el hilo, o un syscall en RESCATE
                          (si el hilo lleva un segundo sin latir)
    [escribe] escritorio  solo el lado del escritorio (un syscall)
    [escribe] arranque    una vez, antes de que exista el hilo
    [escribe] ambos       los dos lados. CARRERA con dos nucleos: es la deuda

La etiqueta va en la misma linea del `static mut`. Lo que dice es un hecho
sobre el codigo de hoy, no una intencion: si un cambio hace que el escritorio
escriba algo que decia `bus`, la etiqueta miente y hay que cambiarla -- y el
diff lo ensena, que es lo que se quiere.

    --check   lo que corre el build
"""
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..'))
CARPETA = os.path.join(RAIZ, 'Ultra_kernel_x86-64', 'kernel', 'src', 'ring0', 'dev', 'usb')
BASE = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'LINEA_BASE.txt')
CLASES = ('bus', 'bombeo', 'escritorio', 'arranque', 'ambos')
RX_STATIC = re.compile(r'^\s*static mut ([A-Z_0-9]+)\s*:')
RX_TAG = re.compile(r'\[escribe\]\s+([a-z]+)')


def censar():
    cuenta = {c: 0 for c in CLASES}
    sin = []
    mal = []
    for nombre in sorted(os.listdir(CARPETA)):
        if not nombre.endswith('.rs'):
            continue
        ruta = os.path.join(CARPETA, nombre)
        with open(ruta, 'rb') as f:
            lineas = f.read().decode('utf-8', 'replace').replace('\r\n', '\n').split('\n')
        for n, l in enumerate(lineas, 1):
            m = RX_STATIC.match(l)
            if not m:
                continue
            t = RX_TAG.search(l)
            sitio = '%s:%d %s' % (nombre, n, m.group(1))
            if not t:
                sin.append(sitio)
            elif t.group(1) not in CLASES:
                mal.append(sitio + ' -> ' + t.group(1))
            else:
                cuenta[t.group(1)] += 1
    return cuenta, sin, mal


def main():
    if '--check' not in sys.argv:
        print(__doc__)
        return 2
    cuenta, sin, mal = censar()
    if sin:
        print('static mut en dev/usb SIN decir quien lo escribe (// [escribe] %s):' % '|'.join(CLASES))
        for s in sin:
            print('    ' + s)
        return 1
    if mal:
        print('static mut con una clase que no existe:')
        for s in mal:
            print('    ' + s)
        return 1
    if not os.path.exists(BASE):
        print('no hay LINEA_BASE.txt: este guardian no sabe contra que comparar')
        return 1
    with open(BASE) as f:
        base = int(f.read().strip() or '0')
    total = sum(cuenta.values())
    if cuenta['ambos'] > base:
        print('`ambos` SUBIO: %d static mut del USB los escriben los dos lados, y la linea base es %d.'
              % (cuenta['ambos'], base))
        print('  Cada uno es una carrera el dia que el bus viva en otro nucleo (PLAN_EL_BUS_APARTE).')
        print('  O se le da un solo escritor, o se BAJA... no: se SUBE la cifra a mano en')
        print('  toolchain/tools/escritores/LINEA_BASE.txt diciendo por que en el commit.')
        return 1
    print('clean: %d static mut en dev/usb y todos dicen quien escribe -- bombeo %d  bus %d  '
          'escritorio %d  arranque %d  ambos %d (base %d: solo puede bajar)'
          % (total, cuenta['bombeo'], cuenta['bus'], cuenta['escritorio'], cuenta['arranque'],
             cuenta['ambos'], base))
    if cuenta['ambos'] < base:
        print('clean: `ambos` BAJO de %d a %d -- baja la cifra en LINEA_BASE.txt' % (base, cuenta['ambos']))
    return 0


if __name__ == '__main__':
    sys.exit(main())
