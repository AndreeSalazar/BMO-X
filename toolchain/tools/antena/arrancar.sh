#!/data/data/com.termux/files/usr/bin/bash
# arrancar.sh -- la ANTENA entera con UNA orden, en Termux (2026-09-16).
#
#     bash ~/storage/shared/bmo-antena/arrancar.sh <IP de BMO-X o del PC>
#
# Eddi: *"la idea en Termux necesitaria automatizar eso, para que guie"*.
# Esto hace, en orden, lo que antes eran tres ventanas:
#
#   1. pide a Android que no duerma Termux (termux-wake-lock)
#   2. enciende un Chromium SIN CABEZA con el puerto de depuracion 9222:
#        - el de Termux si esta instalado (`pkg install tur-repo chromium`)
#        - si no, el de un Debian de proot-distro (`proot-distro login debian`)
#        - si no hay ninguno, la antena arranca SIN navegador y PAGINA
#          contesta NO: no se finge
#   3. espera hasta 40 s a que el 9222 conteste
#   4. arranca antena.py con la carpeta de al lado y --navegador 9222
#
# Al salir de la antena (Ctrl+C) apaga el Chromium que encendio. Un Chromium
# que ya estuviera encendido antes se respeta y no se toca.
#
# Se lanza con `bash` y no como ejecutable a proposito: el almacenamiento
# compartido de Android no permite el bit de ejecucion.

set -u
PERMITIR="${1:-}"
if [ -z "$PERMITIR" ]; then
    echo "uso: bash arrancar.sh <IP permitida (BMO-X o el PC)>"
    exit 1
fi
AQUI="$(cd "$(dirname "$0")" && pwd)"
PUERTO=9222
FLAGS="--headless=new --no-sandbox --disable-gpu --disable-dev-shm-usage --remote-debugging-port=$PUERTO about:blank"

command -v termux-wake-lock >/dev/null 2>&1 && termux-wake-lock

escucha() {
    python - <<'PY'
import socket, sys
s = socket.socket(); s.settimeout(0.5)
try:
    sys.exit(0 if s.connect_ex(("127.0.0.1", 9222)) == 0 else 1)
finally:
    s.close()
PY
}

CHROMIUM_PID=""
if escucha; then
    echo "antena: ya hay un navegador en el $PUERTO, lo uso"
elif command -v chromium >/dev/null 2>&1; then
    echo "antena: enciendo el chromium de Termux"
    chromium $FLAGS > "$HOME/chromium.log" 2>&1 &
    CHROMIUM_PID=$!
elif command -v chromium-browser >/dev/null 2>&1; then
    echo "antena: enciendo chromium-browser"
    chromium-browser $FLAGS > "$HOME/chromium.log" 2>&1 &
    CHROMIUM_PID=$!
elif command -v proot-distro >/dev/null 2>&1 && proot-distro list 2>/dev/null | grep -qi "debian.*installed"; then
    echo "antena: enciendo el chromium del Debian de proot"
    proot-distro login debian -- chromium $FLAGS > "$HOME/chromium.log" 2>&1 &
    CHROMIUM_PID=$!
else
    echo "antena: no hay chromium (ni en Termux ni en proot): PAGINA contestara NO"
fi

NAVEGADOR=""
if [ -n "$CHROMIUM_PID" ] || escucha; then
    for i in $(seq 1 40); do
        if escucha; then
            NAVEGADOR="--navegador $PUERTO"
            echo "antena: navegador listo en el $PUERTO ($i s)"
            break
        fi
        sleep 1
    done
    if [ -z "$NAVEGADOR" ]; then
        echo "antena: el navegador no contesto en 40 s (mira ~/chromium.log); sigo sin el"
    fi
fi

apagar() {
    if [ -n "$CHROMIUM_PID" ]; then
        echo; echo "antena: apago el navegador que encendi"
        kill "$CHROMIUM_PID" 2>/dev/null
        pkill -f "remote-debugging-port=$PUERTO" 2>/dev/null
    fi
}
trap apagar EXIT

# shellcheck disable=SC2086
python "$AQUI/antena.py" --carpeta "$AQUI" --permitir "$PERMITIR" $NAVEGADOR
