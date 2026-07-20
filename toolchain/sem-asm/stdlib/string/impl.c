// Semantic_ASM stdlib/string implementation
// Compiled by the C frontend when functions are referenced.

void *memcpy(void *dest, const void *src, unsigned long n) {
    unsigned char *d = dest;
    const unsigned char *s = src;
    for (unsigned long i = 0; i < n; i = i + 1) {
        d[i] = s[i];
    }
    return dest;
}

void *memmove(void *dest, const void *src, unsigned long n) {
    unsigned char *d = dest;
    const unsigned char *s = src;
    if (d < s) {
        for (unsigned long i = 0; i < n; i = i + 1) {
            d[i] = s[i];
        }
    } else {
        long i = n;
        while (i > 0) {
            i = i - 1;
            d[i] = s[i];
        }
    }
    return dest;
}

void *memset(void *s, int c, unsigned long n) {
    unsigned char *p = s;
    for (unsigned long i = 0; i < n; i = i + 1) {
        p[i] = c;
    }
    return s;
}

int memcmp(const void *s1, const void *s2, unsigned long n) {
    const unsigned char *p1 = s1;
    const unsigned char *p2 = s2;
    for (unsigned long i = 0; i < n; i = i + 1) {
        if (p1[i] != p2[i]) {
            return (int)(p1[i] - p2[i]);
        }
    }
    return 0;
}

unsigned long strlen(const char *s) {
    unsigned long n = 0;
    while (s[n] != 0) { n = n + 1; }
    return n;
}

int strcmp(const char *s1, const char *s2) {
    while (1) {
        if (*s1 != *s2) { return (int)(*s1 - *s2); }
        if (*s1 == 0) { return 0; }
        s1 = s1 + 1;
        s2 = s2 + 1;
    }
}

char *strcpy(char *dest, const char *src) {
    char *d = dest;
    while (1) {
        *d = *src;
        if (*src == 0) { return dest; }
        d = d + 1;
        src = src + 1;
    }
}
