// cuentas.cpp -- the first C++ program of BMO-X that reaches the disk.
//
// It uses, on purpose, the four things C++ exists for in PROPOSITO.md --
// abstract without paying: classes, methods, destructors that run in reverse
// order on every exit, and virtual dispatch through a base pointer.
// Money is kept in CENTS, as integers: the exact decimal belongs to COBOL and
// Ada, and C++ does not pretend otherwise.
//
// Expected output, byte for byte (the test `el_ejemplo_cuentas_dice_lo_que_promete`
// in src/tests/mod.rs compiles and runs this very file):
//
//   abre corriente
//   abre ahorro
//   corriente 12000
//   ahorro 10300
//   total 22300
//   cierra ahorro
//   cierra corriente

class Cuenta {
public:
    int centimos;
    int tipo;
    // Opening is a method and not a constructor with arguments on purpose:
    // the member initializer list (`: Cuenta(inicial, 2)`) arrives in step 4
    // of BRECHA.md, and a derived class cannot hand arguments to its base
    // without it. The compiler says exactly that, with the line.
    void abrir(int inicial, int t) {
        centimos = inicial;
        tipo = t;
        if (tipo == 1) { printf("abre corriente\n"); } else { printf("abre ahorro\n"); }
    }
    ~Cuenta() {
        if (tipo == 1) { printf("cierra corriente\n"); } else { printf("cierra ahorro\n"); }
    }
    void ingresar(int c) { centimos = centimos + c; }
    virtual int cierre_de_mes() { return centimos; }
};

class Ahorro : public Cuenta {
public:
    // 3 % a fin de mes, en centimos enteros.
    virtual int cierre_de_mes() { return centimos + centimos * 3 / 100; }
};

int saldo(Cuenta *c) { return c->cierre_de_mes(); }

int main() {
    Cuenta corriente;
    corriente.abrir(10000, 1);
    Ahorro ahorro;
    ahorro.abrir(10000, 2);
    corriente.ingresar(2000);
    int a = saldo(&corriente);
    int b = saldo(&ahorro);
    printf("corriente %d\n", a);
    printf("ahorro %d\n", b);
    printf("total %d\n", a + b);
    return 0;
}
