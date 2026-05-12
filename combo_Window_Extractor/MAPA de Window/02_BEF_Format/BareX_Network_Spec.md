# BareX Network (`bx_net`) Specification v1.0

**Capa:** L3 (subsistema de red de FastOS)
**Hardware Target:** Realtek RTL8125B 2.5 GbE / Intel I225-V (chipsets B550/X570 típicos), Wi-Fi 6/6E (Intel AX200/AX210), USB Ethernet
**Filosofía:** Stack TCP/IP/QUIC propio en Rust, async-first con io_uring-style submission queues, kernel bypass opcional para juegos. Cero Winsock, cero BSD sockets warts.

> **Objetivo:** Latencia syscall + stack < **15 µs** (vs ~50 µs Windows Winsock, ~30 µs Linux io_uring). Throughput **line-rate 2.5 GbE** con un solo core del 5600X.

---

## 1. Qué heredamos y qué descartamos

| Fuente | Heredamos | Descartamos |
|---|---|---|
| **Linux io_uring** | Modelo SQ/CQ asíncrono zero-copy | Sintaxis críptica, op codes escondidos |
| **DPDK / netmap** | Kernel bypass opt-in para juegos | Necesidad de hugepages obligatorias, drivers vendor |
| **Quiche / quinn** (QUIC) | Implementación QUIC + HTTP/3 | — |
| **Rustls** | TLS 1.3 puro Rust, sin OpenSSL | — |
| **smoltcp** | Stack TCP/IP embedded en Rust | — (lo usamos como base inicial) |
| **WinHTTP / WinINet** | Modelo de cliente HTTP de alto nivel | COM, callbacks Win32 |
| **Windows IOCP** | Completion-based async | API verbosa (CreateIoCompletionPort) |
| **WebTransport** | Transporte moderno bidireccional sobre HTTP/3 | — |

**Eliminado por basura legacy de Windows:**
- ❌ Winsock 1.1 (`wsock32.dll` — Windows 95-era)
- ❌ Named Pipes vía SMB (CIFS) — usamos sockets UNIX-domain en VFS
- ❌ NetBIOS / NBT (puerto 137-139)
- ❌ RPC sobre named pipes
- ❌ `\\?\UNC\` paths para network shares
- ❌ "WPAD" auto-discovery proxy (vector de seguridad)
- ❌ SChannel TLS (reemplazado por Rustls)
- ❌ BITS (Background Intelligent Transfer Service)
- ❌ WinHTTP cookies/cache opaco

---

## 2. Capas y arquitectura

```diagram
╭───────────────────────────────────────────────────╮
│  App BEF                                          │
│   bx_http3 / bx_ws / bx_quic / bx_tcp / bx_udp   │
╰────────────────┬──────────────────────────────────╯
                 ▼
╭───────────────────────────────────────────────────╮
│  bx_net runtime (Ring 3)                          │
│   - HTTP/3, HTTP/2, HTTP/1.1                      │
│   - WebSocket, WebTransport                       │
│   - QUIC (RFC 9000)                               │
│   - TLS 1.3 (Rustls)                              │
│   - DNS (DoH/DoT por defecto)                     │
╰────────────────┬──────────────────────────────────╯
                 ▼  io_uring-style SQ/CQ (mmap)
╭───────────────────────────────────────────────────╮
│  Net Service FastOS (Ring 3 privilegiado)         │
│   - TCP / UDP socket multiplexor                  │
│   - Routing / firewall                            │
│   - Stateful NAT (si modo router)                 │
│   - eBPF-like hooks (programs Rust verificados)   │
╰────────────────┬──────────────────────────────────╯
                 ▼
╭───────────────────────────────────────────────────╮
│  Net Stack FastOS (Ring 0 mínimo)                 │
│   - IPv4/IPv6 (dual stack)                        │
│   - TCP, UDP, ICMP                                │
│   - ARP, NDP                                      │
│   - VLAN 802.1Q opcional                          │
╰────────────────┬──────────────────────────────────╯
                 ▼
╭───────────────────────────────────────────────────╮
│  NIC Drivers (Ring 0)                             │
│   - Realtek RTL8125B (2.5 GbE)                    │
│   - Intel I225-V                                  │
│   - Intel Wi-Fi AX200/AX210 (Wi-Fi 6E)            │
│   - USB CDC-NCM (USB Ethernet)                    │
│   - DMA descriptors, MSI-X interrupts             │
╰───────────────────────────────────────────────────╯
                 ▼ (kernel bypass opt-in)
╭───────────────────────────────────────────────────╮
│  bx_net::raw (apps con privilegio "net.raw")      │
│   - Acceso DMA directo a la NIC                   │
│   - Para juegos competitivos / servidores         │
╰───────────────────────────────────────────────────╯
```

---

## 3. API: Sockets de bajo nivel

```rust
use barex::net::*;

// TCP cliente
let sock = bx_tcp::connect("api.example.com:443").await?;
sock.write_all(b"GET / HTTP/1.1\r\n...").await?;
let mut buf = vec![0u8; 4096];
let n = sock.read(&mut buf).await?;

// UDP socket (juegos)
let udp = bx_udp::bind("0.0.0.0:7777")?;
udp.send_to(&packet, server_addr).await?;
let (n, from) = udp.recv_from(&mut buf).await?;

// Cero callbacks. Cero IOCP boilerplate. Solo async/await Rust.
```

---

## 4. API: HTTP cliente

```rust
let client = bx_http::Client::builder()
    .http3()                    // HTTP/3 si el server lo anuncia (Alt-Svc)
    .timeout_secs(30)
    .build();

let resp = client.get("https://api.example.com/data")
    .header("Authorization", token)
    .send()
    .await?;

let json: MyData = resp.json().await?;
```

Default sensato: **HTTP/3 → HTTP/2 → HTTP/1.1** negociado automáticamente. **TLS 1.3 only** (1.2 opt-in para legacy). DoH para resolución DNS.

---

## 5. API: WebSocket / WebTransport

```rust
let ws = bx_ws::connect("wss://game.example.com/lobby").await?;
ws.send(Message::Binary(packet)).await?;
while let Some(msg) = ws.next().await {
    handle(msg);
}

// WebTransport sobre HTTP/3 (juegos web modernos)
let wt = bx_webtransport::connect("https://wt.example.com/").await?;
let stream = wt.open_bidirectional().await?;
```

---

## 6. API: QUIC nativo (juegos modernos)

```rust
let endpoint = bx_quic::Endpoint::client("0.0.0.0:0")?;
let conn = endpoint.connect("game.example.com:443", "game.example.com").await?;

// Múltiples streams sobre una conexión
let s1 = conn.open_uni().await?;       // stream unidireccional para snapshots
let s2 = conn.open_bi().await?;        // stream bidireccional para chat

// Datagrama no fiable (mejor que UDP raw: cifrado y multiplexed)
conn.send_datagram(&player_pos)?;
```

QUIC es la base ideal para netcode de juegos: 0-RTT, congestion control moderno, multipath opcional, sin head-of-line blocking.

---

## 7. Kernel bypass para juegos competitivos

Para latencia extrema (CS2/Valorant-tier):

```rust
let raw = bx_net::raw::open(InterfaceId::primary())?;  // requiere capability
raw.bind_udp_port(7777)?;

// Polling directo del ring DMA, sin pasar por el stack TCP/IP
loop {
    if let Some(pkt) = raw.try_recv() {
        process_packet(&pkt);
    }
}
```

- Latencia syscall: **0** (mmap a registros NIC).
- Latencia hardware → app: **3–5 µs**.
- Trade-off: la app es responsable de parsear UDP/IP. Helper crate `bx_net::raw::quic` lo hace.

---

## 8. DNS

- **DoH (DNS over HTTPS)** por defecto, server configurable (Cloudflare 1.1.1.1, Quad9, custom).
- **DoT (DNS over TLS)** alternativa.
- Resolver cache local en el net service.
- Sin LLMNR, sin mDNS forzado (mDNS opt-in para devices locales tipo Chromecast).
- Sin NetBIOS name resolution.
- IPv6 first con happy eyeballs.

---

## 9. TLS

- **TLS 1.3 obligatorio** para HTTPS por defecto.
- TLS 1.2 opt-in para servidores legacy (con warning).
- Certificate store basado en Mozilla CA bundle (actualizable vía `bx_pkg`).
- Soporte ECH (Encrypted Client Hello).
- Sin SChannel, sin ASN.1 parsing en C, sin CryptoAPI.

---

## 10. Wi-Fi

- WPA2/WPA3 personal y enterprise.
- WPS deshabilitado por diseño (vector de ataque).
- Roaming 802.11k/v/r soportado.
- Configuración por TOML en `/system/network.toml` o vía `bx_settings`.
- Sin "Microsoft Wi-Fi Direct Virtual Adapter" ni mockfest similar.

---

## 11. Firewall y sandbox de red

Cada proceso BEF tiene capabilities de red explícitas:

```toml
# manifest.bef.toml
[capabilities.network]
outbound = ["api.example.com:443", "*.cdn.example.com:443"]
inbound = []                  # no listen sockets
raw = false                   # no kernel bypass
```

El net service rechaza conexiones que no estén en la lista. Esto elimina toda una clase de malware/spyware silencioso.

---

## 12. NetCode helpers para juegos

`bx_netcode` (crate alto nivel sobre QUIC):
- **Snapshot interpolation** + **lag compensation** built-in.
- **Reliable / Unreliable / Unreliable-Sequenced** channels.
- **Delta encoding** para snapshots.
- **Bandwidth profiling** automático.
- Compatible con el modelo Quake 3 / Source / Overwatch.

---

## 13. Servidor (opcional)

`bx_net` también sirve para servers (FastOS puede correr como dedicated server):
- HTTP/3 + HTTP/2 server (router style).
- WebSocket server.
- Game server con UDP/QUIC.
- Acceptor IOCP-style con io_uring.

Throughput objetivo: **> 500k req/s en un core 5600X** sirviendo HTTP/2 simple.

---

## 14. Compatibilidad con el shim L4

Fake DLLs para juegos Windows:
- `ws2_32.dll` (Winsock 2) → wrapper a `bx_tcp`/`bx_udp` con BSD-like socket API.
- `wininet.dll` / `winhttp.dll` → wrapper a `bx_http`.
- `iphlpapi.dll` (network info) → stubs.
- `dnsapi.dll` → wrapper a resolver DoH.
- `secur32.dll` (SSPI/SChannel) → wrapper limitado a TLS 1.2/1.3.

Funciones limitadas: NetBIOS, RPC sobre TCP, Active Directory client, MSMQ → **no implementadas**.

---

## 15. Métricas de éxito

| Métrica | Objetivo |
|---|---|
| Latencia syscall socket I/O | < 15 µs |
| Latencia kernel-bypass UDP recv | < 5 µs |
| Throughput TCP single-stream 2.5 GbE | line-rate (~2.35 Gbps) |
| Throughput HTTP/2 server (small responses) | > 500k req/s en 1 core |
| Conexiones QUIC concurrentes | > 100k en 8 GB RAM |
| Handshake TLS 1.3 1-RTT | < 30 ms (red + cómputo) |
| Handshake QUIC 0-RTT (resumido) | < 5 ms (1 viaje + cómputo) |

---

## 16. Archivos relacionados

- `BareX_API_Spec.md`
- `BareX_Audio_Spec.md`
- `BareX_Input_Spec.md`
- `FastOS_Syscall_Table_Spec.md` (syscalls de I/O async)
- `FastOS_App_Sandbox.md` (capabilities de red)
- `FastOS_Security_Model.md` (firewall, TLS, DoH)
