# BMO Simple — sintaxis y referencia rápida

## Keywords (sesión 15, base mínima)

| Categoría | Keywords |
|---|---|
| Funciones / variables | `def`, `let`, `retorna` |
| Control de flujo | `si`, `sino`, `mientras`, `rompe`, `continua`, `match` |
| Bajo nivel | `reg`, `emit` |
| Tipos básicos | `byte`, `num`, `ptr` |
| Memoria | `aloc`, `libre`, `arr`, `ref`, `nulo` |
| Aritmética | `suma`, `resta`, `mult`, `div` |
| Lógica | `y`, `o`, `no` |
| Comparación | `igual`, `mayor`, `menor` |
| OOP (futuro) | `tipo`, `impl`, `nuevo` |
| UI/apps (futuro) | `ventana`, `evento`, `dibuja` |

## Ejemplo mínimo

```bmo
def doble(x: num) -> num {
    retorna x mult 2
}
```

## Pipeline interno

```
fuente .bmo  ──▶ lexer ──▶ [Token]
                            │
                            ▼
                          parser ──▶ Ast { Stmt, Expr }
                                       │
                                       ▼
                                     sema (scopes + tipos)
                                       │
                                       ▼
                                     emit ──▶ bytes x86-64
```

## Mapeo a BMO ABI

- `def f()` usa la calling convention BMO: 7 GPR de args, 64 B stack align.
- `retorna x` deposita en `BmoStatus` (RAX:RDX), no en RAX solo.
- `reg rax = 42` accede registros directos con nombres x86-64 estándar.
- `emit 0x0F 0x05` escribe bytes literales (`syscall` en este caso).
- `aloc N` llama a `barex::abi::memory` (no `malloc`).
