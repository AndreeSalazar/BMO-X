// Semantic_ASM stdlib/stdlib implementation

int atoi(const char *s) {
    int sign = 1;
    int n = 0;
    while (*s == ' ') { s = s + 1; }
    if (*s == '-') { sign = -1; s = s + 1; }
    else if (*s == '+') { s = s + 1; }
    while (*s >= '0' && *s <= '9') {
        n = n * 10 + (*s - '0');
        s = s + 1;
    }
    return sign * n;
}

long atol(const char *s) {
    long sign = 1;
    long n = 0;
    while (*s == ' ') { s = s + 1; }
    if (*s == '-') { sign = -1; s = s + 1; }
    else if (*s == '+') { s = s + 1; }
    while (*s >= '0' && *s <= '9') {
        n = n * 10 + (*s - '0');
        s = s + 1;
    }
    return sign * n;
}

int abs(int x) {
    if (x < 0) { return -x; }
    return x;
}

long labs(long x) {
    if (x < 0) { return -x; }
    return x;
}
