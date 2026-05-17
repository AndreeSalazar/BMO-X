"""
FastOS v5.6 — REAL Descriptor from nvidia-open bindata
========================================================
Extracted GA102 IMAGE_PROD (60416B) + HEADER_PROD descriptor:
  appCodeOffset = 0x100  → CODE start in image
  appCodeSize   = 0x8900 → CODE = image[0x100..0x8A00] (35072B) → IMEM
  osDataOffset  = 0x8A00 → DATA start in image  
  osDataSize    = 0x6200 → DATA = image[0x8A00..0xEC00] (25088B) → DMEM
  BOOTVEC       = 0x100  (= appCodeOffset = imemVa)
"""
import struct, os, sys

OP_WRITE32=0x01; OP_POLL32=0x02; OP_WRITE_BLOCK=0x03; OP_READ32=0x05
FIRMWARE_DIR = os.path.join(os.path.dirname(__file__), "USB_boot", "firmware")
OUTPUT_FILE = os.path.join(os.path.dirname(__file__), "fastos_boot.bin")

# GA102 HEADER_PROD descriptor values (decompressed from nvidia-open bindata)
APP_CODE_OFFSET = 0x100
APP_CODE_SIZE   = 0x8900  # 35072 bytes
OS_DATA_OFFSET  = 0x8A00
OS_DATA_SIZE    = 0x6200  # 25088 bytes
BOOTVEC         = APP_CODE_OFFSET  # 0x100

class PB:
    def __init__(self): self.e=[]; self.l=[]
    def w32(s,r,v,d=""):
        s.e.append(struct.pack("<BII I",OP_WRITE32,r,4,v))
        s.l.append(f"[{len(s.e):02d}] W32 0x{r:06X}<-0x{v:08X} // {d}")
    def r32(s,r,d=""):
        s.e.append(struct.pack("<BII",OP_READ32,r,0))
        s.l.append(f"[{len(s.e):02d}] R32 0x{r:06X} // {d}")
    def p32(s,r,m,x,d=""):
        s.e.append(struct.pack("<BII",OP_POLL32,r,8)+struct.pack("<II",m,x))
        s.l.append(f"[{len(s.e):02d}] P32 0x{r:06X}&0x{m:X}==0x{x:X} // {d}")
    def wb(s,r,data,d=""):
        p=data+b'\x00'*((4-len(data)%4)%4)
        s.e.append(struct.pack("<BII",OP_WRITE_BLOCK,r,len(p))+p)
        s.l.append(f"[{len(s.e):02d}] BLK 0x{r:06X}<-{len(data):,}B // {d}")
    def build(s):
        return struct.pack("<4sII",b'FOSB',5,len(s.e))+b''.join(s.e)

def main():
    print("="*60)
    print(" FastOS GA106 v5.6 — REAL nvidia-open descriptor")
    print("="*60)

    img_path = os.path.join(FIRMWARE_DIR, "booter_load_ga102_embedded.bin")
    if not os.path.exists(img_path):
        print("ERROR: embedded image not found"); sys.exit(1)
    with open(img_path,'rb') as f: img = f.read()
    
    assert len(img) == 60416, f"Bad size: {len(img)}"
    
    code = img[APP_CODE_OFFSET : APP_CODE_OFFSET + APP_CODE_SIZE]
    data = img[OS_DATA_OFFSET  : OS_DATA_OFFSET  + OS_DATA_SIZE]
    
    print(f"[*] Image: {len(img):,}B (embedded GA102 IMAGE_PROD)")
    print(f"    CODE: [{APP_CODE_OFFSET:#x}..{APP_CODE_OFFSET+APP_CODE_SIZE:#x}] = {len(code):,}B")
    print(f"    DATA: [{OS_DATA_OFFSET:#x}..{OS_DATA_OFFSET+OS_DATA_SIZE:#x}] = {len(data):,}B")
    print(f"    BOOTVEC: {BOOTVEC:#x}")

    S=0x840000; R=0x841000
    b=PB()

    # ═══ PRIV + PMC ═══
    b.w32(0x12004C,1,"PRIV"); b.w32(0x122204,1,"RING")
    b.p32(0x122100,1,1,"RING_OK")
    b.w32(0x200,0xFFFFFFFF,"PMC"); b.w32(0x600,0xFFFFFFFF,"DEV")

    # ═══ BCR + SRESET ═══
    b.w32(R+0x668,0,"BCR_CTRL")
    b.w32(S+0x100,0x40,"SRESET")
    b.r32(S+0x100,"CPUCTL post-SRESET")
    b.w32(S+0x10C,0,"DMACTL=0")
    b.w32(0x840600,0x05,"FBIF_TRANSCFG")

    # ═══ CODE → IMEM (SECURE, offset 0) ═══
    b.w32(S+0x180, 0x11000000, "IMEMC SECURE auto-inc off=0")
    b.wb(S+0x184, code, f"IMEM<-code({len(code):,}B)")

    # ═══ DATA → DMEM (offset 0) ═══
    b.w32(S+0x1C0, 0x01000000, "DMEMC auto-inc off=0")
    b.wb(S+0x1C4, data, f"DMEM<-data({len(data):,}B)")

    # ═══ PKC BROM ═══
    b.w32(R+0x180,1,"MOD_SEL=RSA3K")
    b.w32(R+0x198,0,"UCODE_ID")
    b.w32(R+0x19C,0,"ENGIDMASK")
    b.w32(R+0x210,0,"PARAADDR(0)")

    # ═══ BOOTVEC=0x100 + Start ═══
    b.w32(S+0x104, BOOTVEC, f"BOOTVEC={BOOTVEC:#x}")
    b.w32(S+0x040,0,"MBOX0=0"); b.w32(S+0x044,0,"MBOX1=0")
    b.w32(S+0x100,0x02,"START")

    # ═══ Diagnostics ═══
    b.r32(S+0x100,"CPUCTL"); b.r32(S+0x008,"IRQSTAT")
    b.r32(S+0x040,"MBOX0"); b.r32(S+0x044,"MBOX1")
    b.p32(S+0x008,0x10,0x10,"HALT")
    b.r32(S+0x040,"MBOX0f"); b.r32(S+0x044,"MBOX1f")
    b.r32(S+0x100,"CPUCTLf"); b.r32(S+0x01C,"EXCI")

    d=b.build()
    with open(OUTPUT_FILE,'wb') as f: f.write(d)
    print()
    for l in b.l: print("  "+l)
    print(f"\n[+] {len(d):,}B, {len(b.e)} entries")
    print(f"\nv5.6: REAL embedded image + descriptor")
    print(f"  BOOTVEC=0x100, CODE=35072B, DATA=25088B")

if __name__=="__main__": main()
