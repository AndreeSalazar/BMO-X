# Anatomía Física del Kernel (Data Structures)

Este documento contiene el 'Molde de Memoria RAM' de los 5 objetos principales del Kernel NT. Estas estructuras son obligatorias para que un SO BMO pueda procesar Syscalls y hablar con los drivers (GSP/NVIDIA, NVMe, USB).

## 1. IRP (I/O Request Packet) e IO_STACK_LOCATION
El 'Paquete de FedEx' del Kernel. Todo envío o recepción de datos viaja dentro de esta estructura.
```c
typedef struct _IO_STACK_LOCATION {
    UCHAR MajorFunction;
    UCHAR MinorFunction;
    UCHAR Flags;
    UCHAR Control;
    union {
        struct {
            PIO_SECURITY_CONTEXT SecurityContext;
            ULONG Options;
            USHORT FileAttributes;
            USHORT ShareAccess;
            ULONG EaLength;
        } Create;
        struct {
            ULONG Length;
            ULONG POINTER_ALIGNMENT Key;
            LARGE_INTEGER ByteOffset;
        } Read;
        struct {
            ULONG Length;
            ULONG POINTER_ALIGNMENT Key;
            LARGE_INTEGER ByteOffset;
        } Write;
        struct {
            ULONG OutputBufferLength;
            ULONG POINTER_ALIGNMENT InputBufferLength;
            ULONG POINTER_ALIGNMENT IoControlCode;
            PVOID Type3InputBuffer;
        } DeviceIoControl;
        struct {
            PVOID Argument1;
            PVOID Argument2;
            PVOID Argument3;
            PVOID Argument4;
        } Others;
    } Parameters;
    PDEVICE_OBJECT DeviceObject;
    PFILE_OBJECT FileObject;
    PIO_COMPLETION_ROUTINE CompletionRoutine;
    PVOID Context;
} IO_STACK_LOCATION, *PIO_STACK_LOCATION;

typedef struct _IRP {
    CSHORT Type;
    USHORT Size;
    PMDL MdlAddress;
    ULONG Flags;
    union {
        struct _IRP *MasterIrp;
        __volatile LONG IrpCount;
        PVOID SystemBuffer;
    } AssociatedIrp;
    LIST_ENTRY ThreadListEntry;
    IO_STATUS_BLOCK IoStatus;
    KPROCESSOR_MODE RequestorMode;
    BOOLEAN PendingReturned;
    CHAR StackCount;
    CHAR CurrentLocation;
    BOOLEAN Cancel;
    KIRQL CancelIrql;
    CCHAR ApcEnvironment;
    UCHAR AllocationFlags;
    PIO_STATUS_BLOCK UserIosb;
    PKEVENT UserEvent;
    union {
        struct {
            union {
                PIO_APC_ROUTINE UserApcRoutine;
                PVOID IssuingProcess;
            };
            PVOID UserApcContext;
        } AsynchronousParameters;
        LARGE_INTEGER AllocationSize;
    } Overlay;
    __volatile PDRIVER_CANCEL CancelRoutine;
    PVOID UserBuffer;
    union {
        struct {
            union {
                KDEVICE_QUEUE_ENTRY DeviceQueueEntry;
                struct {
                    PVOID DriverContext[4];
                };
            };
            PETHREAD Thread;
            PCHAR AuxiliaryBuffer;
            struct {
                LIST_ENTRY ListEntry;
                union {
                    struct _IO_STACK_LOCATION *CurrentStackLocation;
                    ULONG PacketType;
                };
            };
            PFILE_OBJECT OriginalFileObject;
        } Overlay;
        KAPC Apc;
        PVOID CompletionKey;
    } Tail;
} IRP, *PIRP;
```

## 2. MDL (Memory Descriptor List)
Describe cómo la memoria RAM fragmentada se presenta contigua al hardware.
```c
typedef struct _MDL {
  struct _MDL  *Next;
  CSHORT  Size;
  CSHORT  MdlFlags;
  struct _EPROCESS  *Process;
  PVOID  MappedSystemVa;
  PVOID  StartVa;
  ULONG  ByteCount;
  ULONG  ByteOffset;
} MDL, *PMDL;
```

## 3. DRIVER_OBJECT
La tarjeta de identidad de un driver `.sys`.
```c
typedef struct _DRIVER_OBJECT {
    CSHORT Type;
    CSHORT Size;
    PDEVICE_OBJECT DeviceObject;
    ULONG Flags;
    PVOID DriverStart;
    ULONG DriverSize;
    PVOID DriverSection;
    PDRIVER_EXTENSION DriverExtension;
    UNICODE_STRING DriverName;
    PUNICODE_STRING HardwareDatabase;
    PFAST_IO_DISPATCH FastIoDispatch;
    PDRIVER_INITIALIZE DriverInit;
    PDRIVER_STARTIO DriverStartIo;
    PDRIVER_UNLOAD DriverUnload;
    PDRIVER_DISPATCH MajorFunction[0x1B + 1]; // IRP_MJ_MAXIMUM_FUNCTION
} DRIVER_OBJECT, *PDRIVER_OBJECT;
```

## 4. DEVICE_OBJECT
El objeto físico que el driver crea (ej. tu tarjeta NVIDIA o un teclado).
```c
typedef struct _DEVICE_OBJECT {
    CSHORT Type;
    USHORT Size;
    LONG ReferenceCount;
    struct _DRIVER_OBJECT *DriverObject;
    struct _DEVICE_OBJECT *NextDevice;
    struct _DEVICE_OBJECT *AttachedDevice;
    struct _IRP *CurrentIrp;
    PIO_TIMER Timer;
    ULONG Flags;
    ULONG Characteristics;
    __volatile PVPB Vpb;
    PVOID DeviceExtension;
    DEVICE_TYPE DeviceType;
    CCHAR StackSize;
    union {
        LIST_ENTRY ListEntry;
        WAIT_CONTEXT_BLOCK Wcb;
    } Queue;
    ULONG AlignmentRequirement;
    KDEVICE_QUEUE DeviceQueue;
    KDPC Dpc;
    ULONG ActiveThreadCount;
    PSECURITY_DESCRIPTOR SecurityDescriptor;
    KEVENT DeviceLock;
    USHORT SectorSize;
    USHORT Spare1;
    struct _DEVOBJ_EXTENSION  *DeviceObjectExtension;
    PVOID  Reserved;
} DEVICE_OBJECT, *PDEVICE_OBJECT;
```

## 5. KPCR y EPROCESS (Núcleo y Procesos)
`KPCR` maneja el núcleo actual de la CPU. `EPROCESS` es el molde de `obs64.exe` en el kernel.
```c
typedef struct _KPCR {
    union {
        NT_TIB NtTib;
        struct {
            union _KGDTENTRY64 *GdtBase;
            struct _KTSS64 *TssBase;
            ULONG64 UserRsp;
            struct _KPCR *Self;
            struct _KPRCB *CurrentPrcb;
            PKSPIN_LOCK_QUEUE LockArray;
            PVOID Used_Self;
        };
    };
    union _KIDTENTRY64 *IdtBase;
    ULONG64 Unused[2];
    KIRQL Irql;
    UCHAR SecondLevelCacheAssociativity;
    UCHAR ObsoleteNumber;
    UCHAR Fill0;
    ULONG Unused0[3];
    USHORT MajorVersion;
    USHORT MinorVersion;
    ULONG StallScaleFactor;
    PVOID Unused1[3];
    ULONG KernelReserved[15];
    ULONG SecondLevelCacheSize;
    ULONG HalReserved[16];
    ULONG Unused2;
    PVOID KdVersionBlock;
    PVOID Unused3;
    ULONG PcrAlign1[24];
} KPCR, *PKPCR;

typedef struct _EPROCESS {
    KPROCESS Pcb;                     
    EX_PUSH_LOCK ProcessLock;
    LARGE_INTEGER CreateTime;
    LARGE_INTEGER ExitTime;
    EX_RUNDOWN_REF RundownProtect;
    HANDLE UniqueProcessId;           
    LIST_ENTRY ActiveProcessLinks;    
    SIZE_T QuotaUsage[2];
    SIZE_T QuotaPeak[2];
    SIZE_T CommitCharge;
    SIZE_T PeakVirtualSize;
    SIZE_T VirtualSize;
    LIST_ENTRY SessionProcessLinks;
    PVOID DebugPort;
    PVOID ExceptionPort;
    PVOID ObjectTable;                
    EX_FAST_REF Token;                
    PFN_NUMBER WorkingSetPage;
    KGUARDED_MUTEX AddressCreationLock;
    PETHREAD RotateInProgress;
    PETHREAD ForkInProgress;
    ULONG_PTR HardwareTrigger;
    PMM_AVL_TABLE PhysicalVadRoot;
    PVOID CloneRoot;
    PFN_NUMBER NumberOfPrivatePages;
    PFN_NUMBER NumberOfLockedPages;
    PVOID Win32Process;               
    struct _EJOB *Job;                
    PVOID SectionObject;
    PVOID SectionBaseAddress;         
    PVOID QuotaBlock;
    PVOID WorkingSetWatch;
    HANDLE Win32WindowStation;
    HANDLE InheritedFromUniqueProcessId;
    PVOID LdtInformation;
    PVOID VadFreeHint;
    PVOID VdmObjects;
    PVOID DeviceMap;
    UCHAR ImageFileName[15];          
} EPROCESS, *PEPROCESS;
```
