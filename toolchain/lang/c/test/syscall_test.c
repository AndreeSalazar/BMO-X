int main() {
    // Invoke debug_print (0xF0) directly
    __asm_syscall(0xF0, "BMO devours WINE & Linux!\n", 26);
    return 0;
}
