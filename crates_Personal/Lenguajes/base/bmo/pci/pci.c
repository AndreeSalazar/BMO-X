struct PciDevice {
    unsigned long long vendor_id;
    unsigned long long device_id;
    unsigned long long bar0;
    unsigned long long bar1;
    unsigned long long irq;
};

// PCI config space access via ports 0xCF8/0xCFC
// stub: real implementation needs in/out instructions
int pci_scan(struct PciDevice* devices, int max) {
    int count = 0;
    int bus;
    int slot;
    int func;
    for (bus = 0; bus < 256; bus = bus + 1) {
        for (slot = 0; slot < 32; slot = slot + 1) {
            for (func = 0; func < 8; func = func + 1) {
                if (count >= max) return count;
                unsigned int vendor = pci_read_config(bus, slot, func, 0);
                if (vendor != 0xFFFFFFFF) {
                    devices[count].vendor_id = vendor & 0xFFFF;
                    devices[count].device_id = (vendor >> 16) & 0xFFFF;
                    devices[count].bar0 = pci_read_config(bus, slot, func, 0x10);
                    devices[count].bar1 = pci_read_config(bus, slot, func, 0x14);
                    devices[count].irq = pci_read_config(bus, slot, func, 0x3C) & 0xFF;
                    count = count + 1;
                }
            }
        }
    }
    return count;
}

unsigned int pci_read_config(int bus, int slot, int func, int offset) {
    // Write address to 0xCF8, read data from 0xCFC
    // Stub: use MMIO-based approach
    return 0xFFFFFFFF;
}

void pci_write_config(int bus, int slot, int func, int offset, unsigned int val) {
    // Stub
}
