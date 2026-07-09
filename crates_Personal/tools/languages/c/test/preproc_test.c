#define MAX 100
#define GREETING "HOLA desde preprocessor"

int add(int a, int b) {
    return a + b;
}

int main() {
    int x = MAX;
    printf(GREETING);
    return 0;
}