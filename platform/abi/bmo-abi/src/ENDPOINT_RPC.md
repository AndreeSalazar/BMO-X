# Endpoint RPC -- diseno (pre-implementacion)

**Estado**: diseno aprobado, implementacion DIFERIDA hasta que el round-trip
de contexto Ring 3 este verde en hardware (el #GP del iretq del timer debe
estar resuelto antes de construir RPC bloqueante encima).

## Motivacion

Hoy `INVOKE` solo alcanza operaciones implementadas **en el kernel**
(TASK_OP_*, CHANNEL_OP_*). Para F4/F5 -- drivers y GUI **en Ring 3** -- falta
que un proceso pueda ser **servidor**: que otro proceso haga `INVOKE` sobre
un handle y el kernel entregue esa llamada al servidor Ring 3, bloqueando al
caller hasta la respuesta. Es el patron seL4 Call/ReplyRecv y Zircon
channel-call, adaptado a la superficie congelada de BMO.

## Regla de oro

**La superficie de 2 syscalls NO cambia.** `INVOKE` /
`WAIT` quedan como estan. Todo lo nuevo es:

- 1 kind nuevo de capability: `KIND_ENDPOINT` (servidor) y su par
  `KIND_REPLY` (efimero, one-shot).
- Semantica nueva de `INVOKE` cuando el handle resuelve a un endpoint.
- Semantica nueva de `WAIT` cuando el waitable es el endpoint del servidor.

## Objetos

```
Endpoint  = cola de llamadas pendientes + (opcional) servidor bloqueado
ReplyCap  = derecho one-shot a responder UNA llamada concreta
```

### Flujo (caso feliz)

```
CLIENTE                     KERNEL                        SERVIDOR
INVOKE(ep, op, a0..a3) --►  encola {caller_tid, op, args}
        (caller BLOCKED)    despierta al servidor          WAIT(ep, ...) retorna
                                                           {op, args, reply_h}
                                                           ...procesa...
                            copia status al frame          INVOKE(reply_h,
        caller RUNNABLE ◄-- del caller, libera ReplyCap      status, value)
BmoStatus{code,value}
```

- El **mensaje viaja por registros** (op + 4 args, como hoy) -- by-value,
  jamas punteros. Payloads grandes van por el BMO Channel que cliente y
  servidor ya compartan (el endpoint lleva control, el estuario lleva datos:
  mismo reparto que hoy hacen INVOKE/KICK).
- `ReplyCap` es **one-shot y no transferible**: responder lo consume;
  la muerte del servidor lo revoca y el caller despierta con
  `ERROR_ENDPOINT_DEAD`.

## Estados y aristas duras

| Situacion | Comportamiento |
|---|---|
| INVOKE sin servidor esperando | encola; caller BLOCKED (cola FIFO corta, p.ej. 16; llena => `ERROR_BUSY`) |
| WAIT sin llamadas pendientes | servidor BLOCKED hasta el proximo INVOKE |
| Muere el caller bloqueado | su entrada se retira; la ReplyCap en vuelo queda huerfana: responder es no-op OK |
| Muere el servidor | todos los callers encolados/bloqueados despiertan con `ERROR_ENDPOINT_DEAD`; el endpoint queda tombstone hasta revocacion |
| Timeout | `WAIT` ya tiene `timeout_ns`; INVOKE-sobre-endpoint adopta el mismo campo (0 = infinito) |

## Por que encaja con lo existente

- **cap.rs**: `KIND_ENDPOINT`/`KIND_REPLY` son kinds nuevos en la tabla
  per-PID que ya existe (generacion anti-UAF ya resuelta).
- **scheduler**: `wait_current_checked` + `wake_by_key` ya dan el bloqueo
  sin lost-wakeup; la clave de espera del endpoint es su direccion de
  objeto, como hoy `channel::wait_key`.
- **syscall.rs**: `invoke()` gana una rama `KIND_ENDPOINT`; el dispatcher
  ya puede responder con *otro* contexto (doc del entry: "the dispatcher
  may answer with a different context") -- exactamente lo que necesita el
  switch caller->servidor en la frontera del syscall.

## Prerrequisitos (en orden)

1. #GP del iretq resuelto (contexto Ring 3 restaurable de forma probada).
2. Fault-isolation: fault de CPL3 mata la tarea, no el kernel (faults.rs).
3. EXIT-reclaim (per-task allocation lists) -- un servidor que muere debe
   liberar; sin esto los tombstones acumulan.
4. >=2 procesos Ring 3 estables round-robin (hoy solo init).

Con 1-4 verdes, Endpoint RPC es la puerta a F4 (drivers Ring 3) y F5
(compositor/desktop): el GUI server es simplemente el primer servidor real.
