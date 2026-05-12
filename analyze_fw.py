import struct, os

def analyze(path, name):
    with open(path, 'rb') as f:
        data = f.read()
    print(f"=== {name}: {len(data):,} bytes ===")
    
    # NVIDIA HS ucode format from nvidia-open rmflcnbl.h:
    # The bootloader BL goes at END of IMEM
    # Data section (with descriptor + signed payload) goes to DMEM
    # RM_FLCN_BL_DESC tells us code/data layout
    
    # Parse wrapper header
    sig = struct.unpack_from('<I', data, 0)[0]
    ver = struct.unpack_from('<I', data, 4)[0]
    print(f"  Signature: 0x{sig:08X} ({'NVIDIA' if sig==0x10DE else '?'})")
    print(f"  Version: {ver}")
    
    # Fields from file header
    fields = {}
    for i in range(0, min(108, len(data)), 4):
        fields[i] = struct.unpack_from('<I', data, i)[0]
    
    # Based on nvidia-open: the BL binary has code+data
    # Code goes to end of IMEM, data goes to DMEM offset 0
    # Field at 0x08 might be: code load offset in IMEM (where BL code starts)
    # Field at 0x0C: header/descriptor size
    # Field at 0x10: data offset in file
    # Field at 0x14: data size
    
    code_imem_offset = fields.get(0x08, 0)
    hdr_size = fields.get(0x0C, 0)
    data_file_offset = fields.get(0x10, 0)
    data_size = fields.get(0x14, 0)
    entry_val = fields.get(0x18, 0)
    imem_size = fields.get(0x1C, 0)
    
    print(f"  [0x08] code_imem_offset? = 0x{code_imem_offset:X} ({code_imem_offset})")
    print(f"  [0x0C] hdr/desc_size?   = 0x{hdr_size:X} ({hdr_size})")
    print(f"  [0x10] data_file_offset? = 0x{data_file_offset:X} ({data_file_offset})")
    print(f"  [0x14] data_size?        = 0x{data_size:X} ({data_size})")
    print(f"  [0x18] entry/num_apps?   = 0x{entry_val:X} ({entry_val})")
    print(f"  [0x1C] imem_size?        = 0x{imem_size:X} ({imem_size})")
    
    # Verify: data_offset + data_size should = file size
    if data_file_offset + data_size == len(data):
        print(f"  ** data_offset(0x{data_file_offset:X}) + data_size(0x{data_size:X}) = {data_file_offset+data_size} = FILE SIZE! **")
        print(f"  -> Header: bytes 0..0x{data_file_offset-1:X}")
        print(f"  -> Data: bytes 0x{data_file_offset:X}..0x{len(data)-1:X}")
    
    # Code section: header bytes from hdr_size to data_file_offset?
    code_in_file = data_file_offset - hdr_size
    if code_in_file > 0:
        print(f"  -> Possible code in file: bytes 0x{hdr_size:X}..0x{data_file_offset-1:X} ({code_in_file} bytes)")
    
    # App entries (after main header)
    print(f"\n  App/descriptor entries at 0x{hdr_size:X}:")
    for i in range(hdr_size, min(data_file_offset, hdr_size+64), 4):
        v = struct.unpack_from('<I', data, i)[0]
        print(f"    [0x{i:03X}] 0x{v:08X}")
    
    print()

fw_dir = os.path.join(os.path.dirname(__file__), "USB_boot", "firmware")
analyze(os.path.join(fw_dir, "bootloader-535.113.01.bin"), "bootloader-535")
analyze(os.path.join(fw_dir, "booter_load-535.113.01.bin"), "booter_load-535")
