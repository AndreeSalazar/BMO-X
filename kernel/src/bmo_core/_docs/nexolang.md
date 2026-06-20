# Nexolang — Language Spec v0.1

> Nexolang es el lenguaje de scripting de FastOS. Se compila a
> BMO bytecode (que luego se JIT-ea).

## Hello world

```nexo
fn main() {
    print("Hello, FastOS!");
}
```

## Tipos

Primitivos:

- `int`     — 64-bit signed
- `uint`    — 64-bit unsigned
- `bool`    — true / false
- `str`     — string inmutable (UTF-8)
- `void`    — sólo para returns

Compuestos (v0.2, no en v1.7.4):

- `array<T>`
- `map<K, V>`
- `struct`

## Variables

```nexo
let x = 42;          // immutable
let mut y = 0;       // mutable
y = y + 1;
```

## Funciones

```nexo
fn add(a: int, b: int) -> int {
    return a + b;
}

fn main() {
    let r = add(1, 2);
    print("r =", r);
}
```

Closures (no en v1.7.4).

## Control de flujo

```nexo
if x > 0 {
    print("positive");
} else if x < 0 {
    print("negative");
} else {
    print("zero");
}

while x < 10 {
    x = x + 1;
}

for i in 0..10 {
    print(i);
}

loop {
    if done { break; }
}
```

## Operadores

Aritméticos: `+`, `-`, `*`, `/`, `%`.
Comparación: `==`, `!=`, `<`, `>`, `<=`, `>=`.
Lógicos: `&&`, `||`, `!`.
Bitwise: `&`, `|`, `^`, `<<`, `>>`.

Precedencia (de mayor a menor):
1. `()` llamada, `[]` index
2. Unarios: `-`, `!`, `~`
3. `*`, `/`, `%`
4. `+`, `-`
5. `<<`, `>>`
6. `<`, `<=`, `>`, `>=`
7. `==`, `!=`
8. `&`
9. `^`
10. `|`
11. `&&`
12. `||`
13. `=`, `+=`, `-=`, ...

## Built-ins

- `print(args...)` — print to stdout.
- `len(str) -> int` — string length.
- `int(val) -> int` — convert to int.
- `str(val) -> str` — convert to string.
- `read_file(path) -> str` — read entire file.
- `write_file(path, data)` — write file.
- `time_ms() -> int` — milliseconds since boot.
- `sleep_ms(ms)` — sleep.
- `syscall(nr, args...)` — direct syscall.

## Compilación

Pipeline:

```
lex  → tokens
parse → AST
typecheck (v0.2)
emit  → BMO bytecode
```

AST nodes:

- `Program { stmts: Vec<Stmt> }`
- `Stmt::Let { name, value }`
- `Stmt::Assign { target, value }`
- `Stmt::If { cond, then, else }`
- `Stmt::While { cond, body }`
- `Stmt::For { var, iter, body }`
- `Stmt::Return { value }`
- `Stmt::Expr(Expr)`
- `Expr::IntLit(i64)`
- `Expr::StrLit(String)`
- `Expr::BoolLit(bool)`
- `Expr::Ident(String)`
- `Expr::Binary { op, lhs, rhs }`
- `Expr::Unary { op, expr }`
- `Expr::Call { name, args }`
- `Expr::Index { expr, idx }` (v0.2)
- `Expr::Field { expr, name }` (v0.2)

## Ejemplo: factorial

```nexo
fn fact(n: int) -> int {
    if n <= 1 {
        return 1;
    }
    return n * fact(n - 1);
}

fn main() {
    print("10! =", fact(10));
}
```

Compila a:

```bmoasm
fn fact:
    movi r0, 1
    cmp rdi, r0       ; rdi = n
    jle .L1
    ; recurse
    mov r0, rdi
    subi r0, 1
    push rdi
    call fact
    pop rdi
    mul r0, rdi
    ret
.L1:
    movi r0, 1
    ret

fn main:
    movi rdi, 10
    call fact
    ; print result
    mov r8, 0x01
    mov rdi, r0
    syscall
    halt
```

## Limitaciones v1.7.4

- Sin type checking (todo es int).
- Sin generics.
- Sin módulos (todo en un archivo).
- Sin exceptions.
- Sin pattern matching.
- Sin async/await.
- Sin operador `?` (no hay Result en v0.1).
