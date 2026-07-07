// Semantic_ASM stdlib/heap implementation
// Dynamic memory allocation via bmo_mem_alloc / bmo_mem_free syscalls.
// Each allocation stores an 8-byte size header before the returned pointer.

void *malloc(unsigned long size) {
    if (size == 0) return 0;
    unsigned long total = size + 8;
    if (total < 24) total = 24;
    total = (total + 7) & ~7;
    unsigned long raw = bmo_mem_alloc(total);
    if (raw == 0) return 0;
    *(unsigned long *)raw = total;
    return (void *)(raw + 8);
}

void free(void *ptr) {
    if (ptr == 0) return;
    unsigned long raw = (unsigned long)ptr - 8;
    unsigned long sz = *(unsigned long *)raw;
    bmo_mem_free(raw, sz);
}

void *calloc(unsigned long nmemb, unsigned long size) {
    unsigned long total = nmemb * size;
    void *ptr = malloc(total);
    if (ptr == 0) return 0;
    unsigned long i = 0;
    while (i < total) {
        *(unsigned char *)((unsigned long)ptr + i) = 0;
        i = i + 1;
    }
    return ptr;
}

void *realloc(void *ptr, unsigned long new_size) {
    if (ptr == 0) return malloc(new_size);
    if (new_size == 0) { free(ptr); return 0; }
    unsigned long raw = (unsigned long)ptr - 8;
    unsigned long old_size = *(unsigned long *)raw - 8;
    void *np = malloc(new_size);
    if (np == 0) return 0;
    unsigned long n = old_size;
    if (new_size < n) n = new_size;
    unsigned long i = 0;
    while (i < n) {
        *(unsigned char *)((unsigned long)np + i) = *(unsigned char *)((unsigned long)ptr + i);
        i = i + 1;
    }
    free(ptr);
    return np;
}
