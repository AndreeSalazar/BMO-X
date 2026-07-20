unsigned int mmio_read32(unsigned long long addr) {
    return *(volatile unsigned int*)addr;
}

void mmio_write32(unsigned long long addr, unsigned int val) {
    *(volatile unsigned int*)addr = val;
}

unsigned char mmio_read8(unsigned long long addr) {
    return *(volatile unsigned char*)addr;
}

void mmio_write8(unsigned long long addr, unsigned char val) {
    *(volatile unsigned char*)addr = val;
}
