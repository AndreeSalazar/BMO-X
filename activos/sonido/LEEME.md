# El sonido de BMO-X

## `neko.wav` -- lo primero que esta maquina va a tocar

```text
   48000 Hz / 16 bits / 2 canales
   192 bytes por milisegundo
   5,12 s
```

★★ **Esos 192 no son una casualidad: son el numero que `AUDIO_MAESTRO` predijo
para este audifono antes de mirarlo**, y el mismo que sale de
`bmo_sonido::Pcm::bytes_por_ms`. El fichero esta hecho en el formato del aparato
**a proposito**, para que el camino entero sea *leer y dar* -- cero conversion,
que es justo lo que la parte 8 del maestro se niega a hacer:

> *"Si el aparato pide 48 kHz y el fichero viene a 44,1, aqui no se convierte: se
> dice y se convierte fuera. Un resampler malo suena peor que no sonar."*

## Como esta hecho, y por que cada decision

| decision | por que |
|---|---|
| **escala pentatonica** | no tiene semitonos, asi que **ninguna combinacion de dos notas puede sonar mal**. Una melodia sencilla no puede sonar a error |
| **onda cuadrada al 34%** | el sonido de un chip de los ochenta. Es lo que suena a BMO |
| **triangulo en el bajo** | mas suave que la cuadrada: sostiene sin tapar la melodia |
| **sobre ADSR con release largo** | ⚠ una onda que empieza o acaba en un valor distinto de cero es un **escalon**, y un escalon **se oye** como un clic |
| **vibrato del 0,6%** | cuatro operaciones por muestra, y es lo que separa un tono de prueba de algo vivo |
| **limitador `tanh`** | dos voces que coinciden pasan de 1.0. Recortar suena a distorsion sucia; con `tanh` el pico se **dobla** en vez de cortarse, que es lo que hacen los cacharros analogicos |
| **pico al 82%** | margen. Un fichero al 100% no deja sitio para el error de nadie |

## Y sirve de DOS cosas

1. **Es musica.** Cuatro compases que acaban donde empiezan, para que se pueda
   repetir sin costura.
2. ★ **Es el activo de prueba de A3.** Lo abre `bmo-sonido` y contesta *"CABE en
   el audifono tal cual"*. El dia que `audio silencio` de `tarde = 0`, esto es lo
   siguiente que se manda por el mismo tubo -- y **si suena, el camino entero
   esta probado de punta a punta**.

[!] No esta en `staging/`, que se regenera. Vive aqui, en el repositorio, porque
un activo que desaparece al reconstruir no es un activo: es un fichero temporal
con suerte.
