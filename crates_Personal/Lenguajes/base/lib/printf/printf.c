void printf(char* fmt) {
    char c;
    char* p;
    unsigned long long tmp;
    char buf[20];
    int i;
    int state;
    state = 0;
    for (;;) {
        c = *fmt;
        if (c == 0) break;
        fmt = fmt + 1;
        if (state == 1) {
            if (c == 'd') {
                state = 0;
            } else if (c == 'x') {
                state = 0;
            } else if (c == 's') {
                state = 0;
            } else if (c == '%') {
                state = 0;
            } else {
                state = 0;
            }
        } else if (c == '%') {
            state = 1;
        }
    }
}
