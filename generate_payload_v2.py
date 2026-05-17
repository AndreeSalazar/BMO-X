"""
FastOS v6.2 — Compact output + WRITE_CONTEXT for MAILBOX
=========================================================
Changes from v6.1:
- Removed verbose GPU diagnostics (already confirmed in v6.0)
- Compact SEC2 boot sequence
- Key results shown at end
- MAILBOX via OP_WRITE_CONTEXT (kernel injects WPR meta addr)
"""
import struct, os, sys

OP_WRITE32=0x01; OP_POLL32=0x02; OP_WRITE_BLOCK=0x03; OP_READ32=0x05
OP_WRITE_CONTEXT=0x09
FIRMWARE_DIR = os.path.join(os.path.dirname(__file__), "USB_boot", "firmware")
OUTPUT_FILE = os.path.join(os.path.dirname(__file__), "fastos_boot.bin")

APP_CODE_OFFSET = 0x100; APP_CODE_SIZE = 0x8900
OS_DATA_OFFSET  = 0x8A00; OS_DATA_SIZE  = 0x6200
BOOTVEC = 0x100

class PB:
    def __init__(self): self.e=[]; self.l=[]
    def w32(s,r,v,d=""):
        s.e.append(struct.pack("<BII I",OP_WRITE32,r,4,v))
        s.l.append(f"W32 0x{r:06X}<-0x{v:08X} {d}")
    def r32(s,r,d=""):
        s.e.append(struct.pack("<BII",OP_READ32,r,0))
        s.l.append(f"R32 0x{r:06X} {d}")
    def p32(s,r,m,x,d=""):
        s.e.append(struct.pack("<BII",OP_POLL32,r,8)+struct.pack("<II",m,x))
        s.l.append(f"P32 0x{r:06X}&{m:X}=={x:X} {d}")
    def wb(s,r,data,d=""):
        p=data+b'\x00'*((4-len(data)%4)%4)
        s.e.append(struct.pack("<BII",OP_WRITE_BLOCK,r,len(p))+p)
        s.l.append(f"BLK 0x{r:06X}<-{len(data):,}B {d}")
    def wctx(s,r,slot,d=""):
        s.e.append(struct.pack("<BII I",OP_WRITE_CONTEXT,r,4,slot))
        s.l.append(f"CTX [{slot}]->0x{r:06X} {d}")
    def build(s):
        return struct.pack("<4sII",b'FOSB',6,len(s.e))+b''.join(s.e)

def main():
    img_path = os.path.join(FIRMWARE_DIR, "booter_load_ga102_embedded.bin")
    with open(img_path,'rb') as f: img = f.read()
    assert len(img) == 60416
    code = img[APP_CODE_OFFSET : APP_CODE_OFFSET + APP_CODE_SIZE]
    data = img[OS_DATA_OFFSET  : OS_DATA_OFFSET  + OS_DATA_SIZE]
    
    S=0x840000; R=0x841000
    b=PB()

    # ═══ 1. PRIV + PMC (3 entries) ═══
    b.w32(0x12004C,1,"PRIV"); b.w32(0x122204,1,"RING")
    b.p32(0x122100,1,1,"RING")

    # ═══ 2. PMC Enable ═══
    b.w32(0x200,0xFFFFFFFF,"PMC"); b.w32(0x600,0xFFFFFFFF,"DEV")

    # ═══ 3. SEC2 Reset + Config (5 entries) ═══
    b.w32(R+0x668,0,"BCR")
    b.w32(S+0x100,0x40,"RST")
    b.w32(S+0x10C,0,"DMA=0")
    b.w32(0x840600,0x05,"FBIF")

    # ═══ 4. IMEM + DMEM load (4 entries) ═══
    b.w32(S+0x180, 0x11000000, "IMEMC")
    b.wb(S+0x184, code, f"IMEM {len(code):,}B")
    b.w32(S+0x1C0, 0x01000000, "DMEMC")
    b.wb(S+0x1C4, data, f"DMEM {len(data):,}B")

    # ═══ 5. PKC (4 entries) ═══
    b.w32(R+0x180,1,"PKC"); b.w32(R+0x198,0,"UID")
    b.w32(R+0x19C,0,"ENG"); b.w32(R+0x210,0,"PARA")

    # ═══ 6. BOOTVEC + MAILBOX (context) + START ═══
    b.w32(S+0x104, BOOTVEC, "BVEC=0x100")
    b.wctx(S+0x040, 0, "MBOX0=wpr_lo")  # Kernel injects WPR meta addr
    b.wctx(S+0x044, 1, "MBOX1=wpr_hi")  # High 32 bits
    b.w32(S+0x100,0x02,"START!")

    # ═══ 7. Wait + Results ═══
    b.p32(S+0x008,0x10,0x10,"HALT")
    b.r32(S+0x100,"CPUCTL")
    b.r32(S+0x040,"MBOX0")
    b.r32(S+0x044,"MBOX1")
    b.r32(S+0x01C,"EXCI")
    
    # ═══ 8. GSP Falcon post-boot check ═══
    b.r32(0x110100, "GSP_CPU")
    b.r32(0x110040, "GSP_MB0")
    b.r32(0x110008, "GSP_IRQ")

    d=b.build()
    with open(OUTPUT_FILE,'wb') as f: f.write(d)
    
    print(f"v6.2 — {len(b.e)} entries, {len(d):,}B")
    print(f"  CODE={len(code):,}B DATA={len(data):,}B BOOTVEC={BOOTVEC:#x}")
    print(f"  MBOX via WRITE_CONTEXT (kernel injects WPR meta addr)")
    for i,l in enumerate(b.l,1): print(f"  [{i:02d}] {l}")

if __name__=="__main__": main()
