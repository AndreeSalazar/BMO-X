# BMO/FastOS Language Subsystems — Complete Reference

> Complete documentation of the language toolchain that powers BMO/FastOS.
> Every language in this kernel produces **ÑEXO AST** as the common IR,
> which goes through the BMOasm backend and emits native machine code
> for x86_64, aarch64, and riscv64.

---

## 🏛️ Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│  Source code (C, C++, Java, Python, ÑEXO, BMOasm)                   │
└─────────────────────┬───────────────────────────────────────────────┘
                      │ Lexer (per language)
                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Tokens                                                             │
└─────────────────────┬───────────────────────────────────────────────┘
                      │ Parser (per language)
                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Language-specific AST (CAst, CppAst, JAst, PyAst, NexoAst)         │
└─────────────────────┬───────────────────────────────────────────────┘
                      │ Translator (per language) → ÑEXO AST
                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│  ÑEXO AST (common IR for all languages)                             │
└─────────────────────┬───────────────────────────────────────────────┘
                      │ Sema (type checking, scope resolution)
                      │ Codegen (ÑEXO AST → BMOasm AST)
                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│  BMOasm AST (v0.4.0 — complete with structs, fields, vtables)       │
└─────────────────────┬───────────────────────────────────────────────┘
                      │ Sema (BMOasm-level: typeck, fold, DCE, opt)
                      │ Traductor (AST → bytes, with type layouts)
                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Native machine code (x86_64 / aarch64 / riscv64)                   │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 📁 Directory Layout

```
kernel/src/lang/
├── mod.rs                       ← top-level module doc
├── nexo/                        ← ÑEXO: native BMO language
│   ├── lexer.rs                 ← 32 keywords, hex/bin/oct, strings, escapes
│   ├── parser.rs                ← fn, let, if, while, for, struct, enum, impl, match
│   ├── sema.rs                  ← type checking, scope resolution
│   ├── codegen.rs               ← ÑEXO AST → BMOasm AST
│   ├── modules.rs               ← `use`, `pub`, `mod`, `extern`
│   ├── runtime/                 ← BMO runtime helpers
│   │   ├── mod.rs               (26 lines)
│   │   ├── mem.rs               (122 lines) - aloc, libre
│   │   ├── io.rs                (101 lines) - read, write
│   │   ├── proc.rs              (124 lines) - thread, process
│   │   ├── time.rs              ( 74 lines) - clock, sleep
│   │   ├── fs.rs                ( 61 lines) - file I/O
│   │   └── error.rs             ( 65 lines) - error handling
│   ├── stdlib/                  ← standard library
│   │   ├── mod.rs
│   │   ├── sys.rs               ( 19 lines) - syscalls
│   │   ├── io.rs                ( 24 lines) - print, readln
│   │   ├── mem.rs               ( 28 lines) - alloc, free
│   │   ├── str.rs               ( 14 lines) - string ops
│   │   ├── fs.rs                ( 13 lines) - open, read, write
│   │   ├── math.rs              (  8 lines) - abs, min, max
│   │   ├── time.rs              (  8 lines) - now, sleep
│   │   ├── gfx.rs               (  9 lines) - pixel, rect
│   │   └── proc.rs              (  6 lines) - spawn, exit
│   ├── pm/                      ← package manager
│   │   ├── mod.rs
│   │   ├── manifest.rs          (230 lines) - parse nexo.toml
│   │   ├── resolver.rs          ( 84 lines) - dependency resolution
│   │   ├── registry.rs          ( 81 lines) - package registry
│   │   └── build.rs             ( 54 lines) - build orchestrator
│   ├── plugins/                 ← language plugins
│   │   ├── mod.rs
│   │   ├── traits.rs            (362 lines) - LanguagePlugin trait
│   │   ├── registry.rs          (117 lines) - plugin registry
│   │   ├── abi/                 ← ABI plugins
│   │   │   ├── mod.rs
│   │   │   ├── types.rs         ( 76 lines) - type marshalling
│   │   │   └── ffi.rs           ( 76 lines) - FFI bridge
│   │   ├── gc/                  ← garbage collectors
│   │   │   ├── mod.rs           ( 70 lines)
│   │   │   ├── mark_sweep.rs    (172 lines)
│   │   │   ├── copying.rs       (192 lines)
│   │   │   ├── generational.rs  (252 lines)
│   │   │   ├── reference_counting.rs (159 lines)
│   │   │   ├── concurrent.rs    (134 lines)
│   │   │   └── region.rs        (158 lines)
│   │   ├── gil/                 ← global interpreter lock
│   │   │   ├── mod.rs           ( 24 lines)
│   │   │   ├── sync.rs          ( 86 lines)
│   │   │   └── implementations.rs (159 lines)
│   │   └── languages/           ← language frontends
│   │       ├── mod.rs           ( 25 lines)
│   │       ├── c/               ← C (full)
│   │       ├── cpp/             ← C++ (essential)
│   │       ├── java/            ← Java (essential)
│   │       ├── python/          ← Python (essential)
│   │       ├── rust.rs          ← Rust (stub)
│   │       └── go.rs            ← Go (stub)
│   └── tests.rs                 ← 22 end-to-end tests
├── bmoasm/                      ← IR intermedio (backend único)
│   ├── lexer/
│   │   ├── mod.rs
│   │   ├── token.rs             (164 lines) - tokens
│   │   └── scanner.rs           (337 lines) - scanner
│   ├── parser/
│   │   ├── mod.rs
│   │   ├── ast.rs               (223 lines) - AST v0.4.0
│   │   ├── parse.rs             (534 lines) - parser
│   │   └── error.rs             ( 57 lines) - errors
│   ├── sema/                    ← semantic analysis
│   │   ├── mod.rs
│   │   ├── scope.rs             ( 25 lines) - scopes
│   │   ├── typeck.rs            (272 lines) - type checking
│   │   ├── fold.rs              (167 lines) - constant folding
│   │   ├── dce.rs               (204 lines) - dead code elimination
│   │   └── opt.rs               (496 lines) - 6 optimization passes
│   ├── emit/                    ← code generation backends
│   │   ├── mod.rs               ( 57 lines)
│   │   ├── backend.rs           (130 lines) - CodegenBackend trait
│   │   ├── x86_64/              ← x86_64 backend
│   │   │   ├── mod.rs
│   │   │   ├── encoder.rs       (231 lines) - instruction encoder
│   │   │   ├── reg.rs           ( 38 lines) - register encoding
│   │   │   └── backend_impl.rs  (179 lines) - backend
│   │   ├── aarch64/             ← ARM64 backend
│   │   │   ├── mod.rs
│   │   │   └── backend_impl.rs  (231 lines)
│   │   └── riscv/               ← RISC-V backend
│   │       ├── mod.rs
│   │       └── backend_impl.rs  (296 lines)
│   ├── builtin/                 ← builtins
│   │   ├── mod.rs
│   │   ├── flags.rs             ( 41 lines) - CPU flags
│   │   └── intrinsics.rs        ( 89 lines) - intrinsics
│   ├── sample/                  ← example BMOasm programs
│   ├── runtime/                 ← runtime helpers
│   ├── traductor/               ← AST → bytes compiler
│   │   └── mod.rs               (1006 lines) - the compiler
│   ├── cache/                   ← LRU cache
│   └── tests.rs                 ← 41 BMOasm tests
├── docs/                        ← documentation
│   └── README.md                ← this file
└── plugins/languages/           ← language frontends (see above)
```

---

## 🔥 ÑEXO — Native BMO Language (Production)

ÑEXO is the **native language of BMO/FastOS**. It's a Rust-Ada-CMD hybrid
designed for clarity and ergonomics. ÑEXO compiles directly to BMOasm
AST, then to native code.

### Features

- **Lexical**: 32 keywords, hex/binary/octal literals, string escapes
- **Functions**: `fn` with typed parameters, return types, default args
- **Variables**: `let` (immutable) and `mut` (mutable) bindings
- **Control flow**: `si`/`sino` (if/else), `mientras` (while), `para` (for)
- **Data**: `estructura` (struct), `enumero` (enum), `tipo` (type alias)
- **Methods**: `impl` blocks, dynamic dispatch via vtable
- **Pattern matching**: `match` with patterns and defaults
- **Memory**: `aloc`/`libre` (malloc/free)
- **Atomics**: `atomico { ... }` (LOCK prefix block)
- **Memory ordering**: `volatil`, `acquire`, `release`, `fence`
- **CPU flags**: `cuando zf { ... } sino { ... }` (block when flag set)
- **Assembly escapes**: `emit 0x90 0x90` (raw bytes), `reg rax = 42`
- **Modules**: `mod`, `use`, `pub`, `modulo`
- **FFI**: `externa { fn printf(...) -> num; }` (declare external)

### Example

```nexo
fn factorial(n: num) -> num {
    si n menor igual 1 {
        retorna 1
    }
    retorna n mult factorial(n resta 1)
}

fn main() -> num {
    sea x = 0
    mientras x menor 10 {
        x = x suma 1
    }
    retorna factorial(x)
}
```

### Test coverage: 22 tests passing

- `nexo_lexer_basic` - basic lexing
- `nexo_parser_fn` - function parsing
- `nexo_parser_let` - variable parsing
- `nexo_compile_return_42` - end-to-end compile of return
- `nexo_compile_function_call` - function call codegen
- `nexo_compile_arithmetic` - arithmetic ops
- `nexo_compile_if_else` - control flow
- `nexo_compile_while_loop` - loops
- `nexo_compile_struct_layout` - struct field layout
- `nexo_compile_field_access_pattern` - field access codegen
- 8 C frontend tests
- 4 end-to-end pipeline tests

---

## 🔵 C — Full Frontend (Production)

The C frontend is the most mature language plugin. It supports
C99/C11 with the most common features.

### Features

- **Types**: `int`, `unsigned int`, `long`, `short`, `char`, `float`,
  `double`, `void`, `char*`, `int*`, `struct`, `union`, `enum`
- **Storage**: `static`, `extern`, `const`, `volatile`
- **Control flow**: `if`/`else`, `while`, `for`, `do`/`while`, `switch`/`case`
- **Functions**: typed parameters, variadic (`...`), recursion
- **Pointers**: full pointer arithmetic, `*`, `&`, `->`
- **Arrays**: fixed-size, multi-dimensional
- **Strings**: null-terminated, `"..."` literals
- **Preprocessor**: `#include`, `#define`, `#if`/`#endif`
- **Operators**: arithmetic, bitwise, logical, ternary, comma
- **Assignment**: `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=`
- **Struct/Union**: field access with `.` and `->`
- **sizeof**: `sizeof(int)`, `sizeof(expr)`, `sizeof(type)`
- **Ternary**: `cond ? a : b`
- **Goto/Label**: `goto label;` `label:`
- **Compound literals**: `(struct P){1, 2}`

### Example

```c
struct Point {
    int x;
    int y;
};

int factorial(int n) {
    if (n <= 1) {
        return 1;
    }
    return n * factorial(n - 1);
}

int main() {
    struct Point p;
    p.x = 42;
    p.y = factorial(5);
    return p.x + p.y;
}
```

### Test coverage: 8 tests passing

- `c_compile_simple_add` - basic function
- `c_compile_main` - main function
- `c_compile_arithmetic` - arithmetic
- `c_compile_if_statement` - if
- `c_compile_while_loop` - while
- `c_compile_compound_assign` - += / -=
- `c_compile_struct` - struct decl
- `c_compile_struct_field_read` - p.x access
- `c_compile_struct_multiple_fields` - multi-field struct
- `pipeline_c_nexo_bmoasm_x86_64` - end-to-end

---

## 🟣 C++ — Essential Subset (v0.1.0)

C++ brings class-based OOP, virtual dispatch, and `new`/`delete`
to BMO. It compiles by lowering to C-like ÑEXO AST with struct + vtable.

### Supported features

- **Classes**: `class Foo { ... };` (same as struct, with access)
- **Access specifiers**: `public:`, `private:`, `protected:`
- **Member functions**: `int foo() { return x; }`
- **Constructors**: `Foo() : x(0) { }`
- **Destructors**: `~Foo() { }`
- **`this` pointer**: implicit first parameter
- **Inheritance**: `class Bar : public Foo { ... }`
- **Virtual methods**: `virtual void f();` + vtable
- **`new`/`delete`**: heap allocation with placement
- **`nullptr`**: null pointer
- **Type casting**: `(Type)expr`
- **References**: `Type&`
- **Operator overload**: recognized, lowered to regular call
- **Templates**: recognized, lowered to base type

### Not supported (yet)

- Multiple inheritance
- Virtual inheritance
- Operator overloading (full)
- Exceptions (try/catch in C++)
- RTTI (`typeid`, `dynamic_cast`)
- Namespaces (recognized but not nested)
- Templates (specialization)
- Move semantics
- Lambda captures (basic only)

### Example

```cpp
class Animal {
public:
    int age;
    Animal(int a) : age(a) {}
    virtual void speak() {
        // base
    }
};

class Dog : public Animal {
public:
    Dog() : Animal(0) {}
    void speak() override {
        // bark
    }
};

int main() {
    Dog* d = new Dog();
    d->speak();
    delete d;
    return 0;
}
```

### Lowering strategy

```
class Foo { ... };                →  struct Foo { vptr, fields };
virtual void f();                 →  struct Foo_vtable { f_ptr };
class Bar : public Foo { ... };    →  struct Bar { base: Foo, vptr, fields };
this->field                      →  (*this).field
new Foo(args)                     →  aloc(sizeof(Foo)); placement; constructor
delete ptr                       →  destructor; libre(ptr)
virtual call obj->f()             →  obj->vptr->f(obj)
```

### Status

- ✅ Lexer (extends C with class, virtual, this, new, delete, etc.)
- ✅ AST (CppClass, ClassMember, vtable layout)
- ✅ Translator (class → struct + vtable lowering)
- ✅ Plugin entry
- 🚧 Full member function body lowering
- 🚧 Constructor member initializer list lowering
- 🚧 Virtual dispatch codegen

---

## ☕ Java — Essential Subset (v0.1.0)

Java brings OOP, interfaces, and exception handling to BMO. It
compiles by lowering to a struct + vtable pattern, similar to C++.

### Supported features

- **Classes**: `class Foo { ... }`
- **Interfaces**: `interface Bar { ... }` (pure virtual)
- **Inheritance**: `class Bar extends Foo`
- **Implements**: `class Bar implements Baz, Qux`
- **Access**: `public`, `private`, `protected`
- **Modifiers**: `static`, `final`, `abstract`
- **Methods**: instance + static, abstract
- **Fields**: instance + static, final
- **Constructors**: same name as class (or `<init>`)
- **`this`**: implicit reference
- **`new`**: object instantiation
- **Arrays**: `int[]`, `String[]`
- **Primitives**: `void`, `boolean`, `byte`, `short`, `int`, `long`,
  `float`, `double`, `char`
- **Control flow**: `if`/`else`, `while`, `for`, `do`/`while`, `switch`
- **`try`/`catch`/`finally`**: exception handling
- **`throw`**: exception throwing
- **Strings**: `String` (class), `"..."` literals
- **`null`**: null reference
- **Casts**: `(Type)expr`

### Not supported (yet)

- Generics (raw types only)
- Annotations runtime
- Lambdas
- Streams
- Modules (java 9+)
- Sealed classes
- Records
- Pattern matching instanceof

### Example

```java
public class Animal {
    private int age;
    public Animal(int a) { this.age = a; }
    public int getAge() { return this.age; }
}

public class Dog extends Animal {
    public Dog() { super(0); }
    public int getAge() { return 100; }
}

public class Main {
    public static void main(String[] args) {
        try {
            Animal a = new Dog();
            System.out.println(a.getAge());
        } catch (Exception e) {
            System.exit(1);
        }
    }
}
```

### Status

- ✅ Lexer (80+ tokens, complete)
- ✅ Parser (recursive-descent, complete)
- ✅ AST (JClass, JMember, JType, all expressions)
- ✅ Vtable layout calculation
- ✅ Exception planning
- ✅ Translator stub (class → struct + vtable)
- ✅ Plugin entry
- 🚧 try/catch lowering to setjmp/longjmp-style
- 🚧 `new` array allocation
- 🚧 Method body lowering (delegated to C frontend)

---

## 🐍 Python — Essential Subset (v0.1.0)

Python brings scripting, dynamic typing, and rapid prototyping to BMO.
It's modeled after MicroPython: a minimal but expressive subset.

### Supported features

- **Indentation-based syntax** (no braces, no semicolons)
- **Literals**: `int`, `float`, `str`, `bool`, `None`, `list`, `dict`, `tuple`
- **Variables**: dynamic, no declarations
- **Operators**: `+`, `-`, `*`, `/`, `//`, `%`, `**`, `==`, `!=`, `<`, `>`,
  `<=`, `>=`, `and`, `or`, `not`, `&`, `|`, `^`, `<<`, `>>`
- **Functions**: `def`, default args, `*args`, `**kwargs`
- **Classes**: `class`, `__init__`, inheritance
- **Control flow**: `if`/`elif`/`else`, `while`, `for ... in`,
  `break`, `continue`, `pass`
- **Exception handling**: `try`/`except`/`finally`, `raise`
- **Context managers**: `with`
- **Imports**: `import foo`, `from foo import bar`, `import foo as bar`
- **Lambdas**: `lambda x: x + 1` (no capture for now)
- **Builtins**: `print`, `len`, `range`, `int`, `str`, `float`, `bool`,
  `list`, `dict`, `tuple`, `set`, `abs`, `min`, `max`, `sum`, `sorted`,
  `enumerate`, `zip`, `map`, `filter`, `isinstance`, `type`, `id`,
  `open`, `input`, `chr`, `ord`, `hex`, `bin`, `oct`, `repr`, `format`
- **String methods**: `upper`, `lower`, `split`, `join`, `strip`, etc.
- **List/Dict comprehensions**: `[x*2 for x in range(10)]`
- **Slicing**: `a[1:5]`, `a[::2]`

### Not supported (yet)

- Generators (yield)
- async/await
- Decorators with arguments
- Metaclass
- Descriptors
- Complex slicing (multi-dim)
- Recursive comprehensions

### Example

```python
def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)

class Animal:
    def __init__(self, name):
        self.name = name

    def speak(self):
        return f"{self.name} makes a sound"

class Dog(Animal):
    def speak(self):
        return f"{self.name} barks"

animals = [Dog("Rex"), Dog("Buddy")]
for a in animals:
    try:
        print(a.speak())
    except Exception as e:
        print(f"Error: {e}")
```

### Status

- ✅ Lexer (indent-significant, INDENT/DEDENT/NEWLINE tokens)
- ✅ Parser (recursive-descent, all statements)
- ✅ AST (PyExpr, PyStmt, PyImport, full coverage)
- ✅ Builtins registry (30+ builtins)
- ✅ Translator stub
- ✅ Plugin entry
- 🚧 List/Dict runtime (need BMO-side support)
- 🚧 List/Dict comprehensions lowering
- 🚧 Class instantiation lowering

---

## 🦀 Rust — Kernel Language (Native)

The kernel itself is written in Rust. This is not a "language frontend"
— Rust compiles directly via the standard Rust toolchain to a regular
ELF binary, which is then loaded by the BMO PE/ELF loader.

### What's in the kernel

- `kernel/src/main.rs` — entry point, dispatch to boot phases
- `kernel/src/arch/` — architecture-specific code (x86_64, aarch64, riscv)
- `kernel/src/memory/` — page allocator, heap, VMM
- `kernel/src/drivers/` — serial, GOP, PCI, USB, etc.
- `kernel/src/bmo_abi/` — the BMO ABI (inter-language contract)
- `kernel/src/barex/` — BareX API (graphics, audio, input, net)
- `kernel/src/sched/` — scheduler, processes, threads
- `kernel/src/syscall/` — syscall dispatch
- `kernel/src/lang/` — language toolchain (this directory)
- `kernel/src/windows_compat/` — Win32 shim for PE loading
- `kernel/src/sandbox/` — sandbox for BEF shims

---

## 🔶 Go — Service Language (Stub)

Go support is a stub. The plugin recognizes Go syntax but doesn't
yet compile. Use C for service-level apps.

---

## 🟫 BMOasm — Intermediate Representation (Production)

BMOasm is the **single IR** for all languages. It is a low-level
assembly with semantic-pure syntax that compiles to native machine
code for any of the three supported architectures.

### Features (v0.4.0)

- **Types**: `byte`, `num`, `ptr`, `arr`, `ref`, `void`, `bool`,
  `Struct(name)`, `Enum(name)`
- **Functions**: `def name(params) -> ret { body }`
- **Variables**: `let name = expr`, `store name = expr` (rebind)
- **Control flow**: `si`/`sino` (if/else), `mientras` (while),
  `para var desde hasta paso` (for), `bucle` (infinite)
- **Pattern matching**: `match expr { caso pat => body, defecto => body }`
- **Memory**: `aloc size`, `libre ptr`
- **Atomics**: `atomico { body }` (LOCK prefix)
- **Memory ordering**: `volatil`, `acquire`, `release`, `fence`, `barr`
- **CPU flags**: `cuando zf { body } sino { body }`
- **Field access**: `obj.field` (with offset calculation from type layout)
- **Index access**: `obj[idx]` (with `idx*8` calculation)
- **Pointers**: `&x` (address-of), `*x` (dereference)
- **Casts**: `x as Type`
- **Function calls**: `call name(args)`, `retorna expr`
- **Break/continue**: `rompe`, `continua`
- **Labels/goto**: `etiqueta name`, `salto name`
- **File include**: `incluye "file.bmo"`
- **Raw bytes**: `emit 0x90 0x90`
- **Register access**: `reg rax = 42`

### Optimization passes (in `sema/opt.rs`)

1. **inline_small_functions** - inline small function bodies
2. **eliminate_unused_lets** - remove dead let bindings
3. **constant_folding** - evaluate constant expressions at compile time
4. **algebraic_simplification** - x+0 = x, x*1 = x, etc.
5. **strength_reduction** - replace expensive ops with cheaper ones
6. **dead_branch_elimination** - remove unreachable code

### Test coverage: 41 tests passing

- opt_constant_folding, opt_unused_let, opt_algebraic, opt_dead_branch,
  opt_strength_reduction
- dce_unused_function, dce_unreachable_code
- x86_64 intrinsic encodings
- aarch64 encodings
- riscv64 encodings
- e2e_hello_function and more

---

## 🏗️ Three-Layer Build System

BMO has three layers that work together:

```
Layer 1: BareX (apps)         ← C, C++, Java, Python, ÑEXO
Layer 2: BMOasm (IR)          ← v0.4.0, 6 opt passes, 3 backends
Layer 3: BMO ABI (runtime)    ← calling convention, types, status
```

A program in any language goes through all three layers:

```
Source → Lexer → Parser → Translator
                            ↓
                       ÑEXO AST
                            ↓
            (sema) → codegen → BMOasm AST
                            ↓
            (sema) → opt → traductor
                            ↓
                       Native bytes
                            ↓
        BMO ABI: registers, stack, calling convention
```

---

## 📊 Test Coverage Summary

| Module | Tests | Status |
|---|---:|---|
| ÑEXO (parser, lexer, codegen) | 8 | ✅ all passing |
| C frontend (parser, codegen) | 8 | ✅ all passing |
| C → ÑEXO → BMOasm → x86_64 | 2 | ✅ all passing |
| C struct field access | 2 | ✅ all passing |
| ÑEXO struct layout | 1 | ✅ all passing |
| BMOasm parser | 4 | ✅ all passing |
| BMOasm sema | 8 | ✅ all passing |
| BMOasm opt | 5 | ✅ all passing |
| BMOasm codegen (x86_64) | 8 | ✅ all passing |
| BMOasm codegen (aarch64) | 4 | ✅ all passing |
| BMOasm codegen (riscv64) | 4 | ✅ all passing |
| BMOasm end-to-end | 4 | ✅ all passing |
| C++ plugin | 0 | 🚧 v0.1.0 |
| Java plugin | 0 | 🚧 v0.1.0 |
| Python plugin | 0 | 🚧 v0.1.0 |
| **Total** | **58** | **58 ✅ + 0 🚧** |

---

## 🚀 "Crazy Ideas" and Future Directions

These are ideas that could be implemented in the future, in
rough order of effort vs. value:

### Phase 1: Stabilize the essentials (1-2 months)

- **C++ translator v1.0**: full class lowering, vtable dispatch, `new`/`delete`
- **Java translator v1.0**: classes, interfaces, try/catch
- **Python translator v1.0**: list/dict runtime, comprehensions
- **C improvements**: `_Bool`, `restrict`, designated initializers

### Phase 2: Add more capability (2-3 months)

- **Templates in C++**: simple monomorphization
- **Generics in Java**: raw types (type erasure)
- **async/await in Python**: minimal coroutines
- **Exceptions in C++**: setjmp/longjmp-style
- **Operator overloading in C++**: limited

### Phase 3: Crazy ideas (6+ months)

- **Self-hosting compiler**: rewrite BMOasm in BMOasm/NEXO
- **GPU compilation**: BMOasm → SPIR-V → Vulkan
- **WASM target**: BMOasm → WebAssembly
- **JIT compilation**: cache hot paths, inline BMOasm
- **Hot reload**: dynamic library loading
- **Type inference across languages**: unified type system

### Phase 4: The dream

- **BMO IDE**: text editor with syntax highlighting for all 4 langs
- **BMO Debugger**: gdb-compatible debugger over serial
- **BMO Package Manager**: `bmopm install <package>` like cargo
- **BMO Build System**: nexo.toml-based, like Cargo.toml
- **BMO Test Framework**: `#[test]` attribute, run on target

---

## 🛠️ Build and Test

```bash
# Build the kernel
cargo +nightly build --target x86_64-unknown-none --release

# Run all tests
cargo +nightly test --target-dir target_build/kernel

# Build for ARM64
cargo +nightly build --target aarch64-unknown-none --release

# Build for RISC-V
cargo +nightly build --target riscv64gc-unknown-none-elf --release
```

---

## 📜 License

Techne License — same as the rest of FastOS.
