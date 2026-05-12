# ADN de WINE / ReactOS (Estructuras C Reales)

Este documento contiene las estructuras de datos reales extraídas del código fuente de WINE.
Cruzar estos datos con las funciones exportadas en los Markdowns de Ring 0 y Ring 3 te dará el ADN completo para FastOS.

## `d3d12.h`
Total de estructuras extraídas: 227

### Muestra de Estructuras Clave:
```c
typedef struct D3D12_FEATURE_DATA_DISPLAYABLE
{
    BOOL DisplayableTexture;
    D3D12_SHARED_RESOURCE_COMPATIBILITY_TIER SharedResourceCompatibilityTier;
} D3D12_FEATURE_DATA_DISPLAYABLE;

typedef struct D3D12_BOX
{
    UINT left;
    UINT top;
    UINT front;
    UINT right;
    UINT bottom;
    UINT back;
} D3D12_BOX;

typedef struct D3D12_VIEWPORT
{
    FLOAT TopLeftX;
    FLOAT TopLeftY;
    FLOAT Width;
    FLOAT Height;
    FLOAT MinDepth;
    FLOAT MaxDepth;
} D3D12_VIEWPORT;

typedef struct D3D12_RANGE
{
    SIZE_T Begin;
    SIZE_T End;
} D3D12_RANGE;

typedef struct D3D12_RANGE_UINT64
{
    UINT64 Begin;
    UINT64 End;
} D3D12_RANGE_UINT64;

typedef struct D3D12_SUBRESOURCE_RANGE_UINT64
{
    UINT Subresource;
    D3D12_RANGE_UINT64 Range;
} D3D12_SUBRESOURCE_RANGE_UINT64;

typedef struct D3D12_SUBRESOURCE_INFO
{
    UINT64 Offset;
    UINT RowPitch;
    UINT DepthPitch;
} D3D12_SUBRESOURCE_INFO;

typedef struct D3D12_RESOURCE_ALLOCATION_INFO
{
    UINT64 SizeInBytes;
    UINT64 Alignment;
} D3D12_RESOURCE_ALLOCATION_INFO;

typedef struct D3D12_RESOURCE_ALLOCATION_INFO1
{
    UINT64 Offset;
    UINT64 Alignment;
    UINT64 SizeInBytes;
} D3D12_RESOURCE_ALLOCATION_INFO1;

typedef struct D3D12_DRAW_ARGUMENTS
{
    UINT VertexCountPerInstance;
    UINT InstanceCount;
    UINT StartVertexLocation;
    UINT StartInstanceLocation;
} D3D12_DRAW_ARGUMENTS;

typedef struct D3D12_DRAW_INDEXED_ARGUMENTS
{
    UINT IndexCountPerInstance;
    UINT InstanceCount;
    UINT StartIndexLocation;
    INT BaseVertexLocation;
    UINT StartInstanceLocation;
} D3D12_DRAW_INDEXED_ARGUMENTS;

typedef struct D3D12_DISPATCH_ARGUMENTS
{
    UINT ThreadGroupCountX;
    UINT ThreadGroupCountY;
    UINT ThreadGroupCountZ;
} D3D12_DISPATCH_ARGUMENTS;

typedef struct D3D12_FEATURE_DATA_D3D12_OPTIONS
{
    BOOL DoublePrecisionFloatShaderOps;
    BOOL OutputMergerLogicOp;
    D3D12_SHADER_MIN_PRECISION_SUPPORT MinPrecisionSupport;
    D3D12_TILED_RESOURCES_TIER TiledResourcesTier;
    D3D12_RESOURCE_BINDING_TIER ResourceBindingTier;
    BOOL PSSpecifiedStencilRefSupported;
    BOOL TypedUAVLoadAdditionalFormats;
    BOOL ROVsSupported;
    D3D12_CONSERVATIVE_RASTERIZATION_TIER ConservativeRasterizationTier;
    UINT MaxGPUVirtualAddressBitsPerResource;
    BOOL StandardSwizzle64KBSupported;
    D3D12_CROSS_NODE_SHARING_TIER CrossNodeSharingTier;
    BOOL CrossAdapterRowMajorTextureSupported;
    BOOL VPAndRTArrayIndexFromAnyShaderFeedingRasterizerSupportedWithoutGSEmulation;
    D3D12_RESOURCE_HEAP_TIER ResourceHeapTier;
} D3D12_FEATURE_DATA_D3D12_OPTIONS;

typedef struct D3D12_FEATURE_DATA_FORMAT_SUPPORT
{
    DXGI_FORMAT Format;
    D3D12_FORMAT_SUPPORT1 Support1;
    D3D12_FORMAT_SUPPORT2 Support2;
} D3D12_FEATURE_DATA_FORMAT_SUPPORT;

typedef struct D3D12_FEATURE_DATA_MULTISAMPLE_QUALITY_LEVELS
{
    DXGI_FORMAT Format;
    UINT SampleCount;
    D3D12_MULTISAMPLE_QUALITY_LEVEL_FLAGS Flags;
    UINT NumQualityLevels;
} D3D12_FEATURE_DATA_MULTISAMPLE_QUALITY_LEVELS;

```

*(Archivo completo guardado localmente en `C:\Users\andre\OneDrive\Documentos\SigDead\combo_Window_Extractor\MAPA de Window\WINE_Reference_DNA\d3d12.h`)*

---

## `dxgi.h`
Total de estructuras extraídas: 10

### Muestra de Estructuras Clave:
```c
typedef struct _LUID {
    DWORD LowPart;
    LONG HighPart;
} LUID, *PLUID;

typedef struct DXGI_SURFACE_DESC {
    UINT Width;
    UINT Height;
    DXGI_FORMAT Format;
    DXGI_SAMPLE_DESC SampleDesc;
} DXGI_SURFACE_DESC;

typedef struct DXGI_MAPPED_RECT {
    INT Pitch;
    BYTE *pBits;
} DXGI_MAPPED_RECT;

typedef struct DXGI_OUTPUT_DESC {
    WCHAR DeviceName[32];
    RECT DesktopCoordinates;
    BOOL AttachedToDesktop;
    DXGI_MODE_ROTATION Rotation;
    HMONITOR Monitor;
} DXGI_OUTPUT_DESC;

typedef struct DXGI_FRAME_STATISTICS {
    UINT PresentCount;
    UINT PresentRefreshCount;
    UINT SyncRefreshCount;
    LARGE_INTEGER SyncQPCTime;
    LARGE_INTEGER SyncGPUTime;
} DXGI_FRAME_STATISTICS;

typedef struct DXGI_ADAPTER_DESC {
    WCHAR Description[128];
    UINT VendorId;
    UINT DeviceId;
    UINT SubSysId;
    UINT Revision;
    SIZE_T DedicatedVideoMemory;
    SIZE_T DedicatedSystemMemory;
    SIZE_T SharedSystemMemory;
    LUID AdapterLuid;
} DXGI_ADAPTER_DESC;

typedef struct DXGI_SWAP_CHAIN_DESC {
    DXGI_MODE_DESC BufferDesc;
    DXGI_SAMPLE_DESC SampleDesc;
    DXGI_USAGE BufferUsage;
    UINT BufferCount;
    HWND OutputWindow;
    BOOL Windowed;
    DXGI_SWAP_EFFECT SwapEffect;
    UINT Flags;
} DXGI_SWAP_CHAIN_DESC;

typedef struct DXGI_SHARED_RESOURCE {
    HANDLE Handle;
} DXGI_SHARED_RESOURCE;

typedef struct DXGI_ADAPTER_DESC1 {
    WCHAR  Description[128];
    UINT   VendorId;
    UINT   DeviceId;
    UINT   SubSysId;
    UINT   Revision;
    SIZE_T DedicatedVideoMemory;
    SIZE_T DedicatedSystemMemory;
    SIZE_T SharedSystemMemory;
    LUID   AdapterLuid;
    UINT   Flags;
} DXGI_ADAPTER_DESC1;

typedef struct DXGI_DISPLAY_COLOR_SPACE {
    FLOAT PrimaryCoordinates[8][2];
    FLOAT WhitePoints[16][2];
} DXGI_DISPLAY_COLOR_SPACE;

```

*(Archivo completo guardado localmente en `C:\Users\andre\OneDrive\Documentos\SigDead\combo_Window_Extractor\MAPA de Window\WINE_Reference_DNA\dxgi.h`)*

---

## `ntddk.h`
Total de estructuras extraídas: 10

### Muestra de Estructuras Clave:
```c
typedef struct _CONFIGURATION_INFORMATION
{
    ULONG DiskCount;
    ULONG FloppyCount;
    ULONG CdRomCount;
    ULONG TapeCount;
    ULONG ScsiPortCount;
    ULONG SerialCount;
    ULONG ParallelCount;
    BOOLEAN AtDiskPrimaryAddressClaimed;
    BOOLEAN AtDiskSecondaryAddressClaimed;
    ULONG Version;
    ULONG MediumChangerCount;
} CONFIGURATION_INFORMATION, *PCONFIGURATION_INFORMATION;

        struct
        {
            ULONG ImageAddressingMode  : 8;
            ULONG SystemModeImage      : 1;
            ULONG ImageMappedToAllPids : 1;
            ULONG ExtendedInfoPresent  : 1;
            ULONG MachineTypeMismatch : 1;
            ULONG ImageSignatureLevel : 4;
            ULONG ImageSignatureType : 3;
            ULONG ImagePartialMap : 1;
            ULONG Reserved : 12;
        } DUMMYSTRUCTNAME;

typedef struct _FILE_VALID_DATA_LENGTH_INFORMATION
{
  LARGE_INTEGER ValidDataLength;
} FILE_VALID_DATA_LENGTH_INFORMATION, *PFILE_VALID_DATA_LENGTH_INFORMATION;

typedef struct _PROCESS_ACCESS_TOKEN
{
    HANDLE Token;
    HANDLE Thread;
} PROCESS_ACCESS_TOKEN, *PPROCESS_ACCESS_TOKEN;

    struct _RTL_SPLAY_LINKS *RightChild;
} RTL_SPLAY_LINKS, *PRTL_SPLAY_LINKS;

typedef struct _RTL_GENERIC_TABLE
{
    PRTL_SPLAY_LINKS TableRoot;
    LIST_ENTRY InsertOrderList;
    LIST_ENTRY *OrderedPointer;
    ULONG WhichOrderedElement;
    ULONG NumberGenericTableElements;
    PRTL_GENERIC_COMPARE_ROUTINE CompareRoutine;
    PRTL_GENERIC_ALLOCATE_ROUTINE AllocateRoutine;
    PRTL_GENERIC_FREE_ROUTINE FreeRoutine;
    void *TableContext;
} RTL_GENERIC_TABLE;

    struct _RTL_BALANCED_LINKS *RightChild;
    CHAR Balance;
    UCHAR Reserved[3];
} RTL_BALANCED_LINKS;

typedef struct _RTL_AVL_TABLE {
    RTL_BALANCED_LINKS BalancedRoot;
    void *OrderedPointer;
    ULONG WhichOrderedElement;
    ULONG NumberGenericTableElements;
    ULONG DepthOfTree;
    PRTL_BALANCED_LINKS RestartKey;
    ULONG DeleteCount;
    PRTL_AVL_COMPARE_ROUTINE CompareRoutine;
    PRTL_AVL_ALLOCATE_ROUTINE AllocateRoutine;
    PRTL_AVL_FREE_ROUTINE FreeRoutine;
    void *TableContext;
} RTL_AVL_TABLE, *PRTL_AVL_TABLE;

        struct {
            ULONG FileOpenNameAvailable :1;
            ULONG IsSubsystemProcess :1;
            ULONG Reserved :30;
        } DUMMYSTRUCTNAME;

    struct _FILE_OBJECT *FileObject;
    PCUNICODE_STRING     ImageFileName;
    PCUNICODE_STRING     CommandLine;
    NTSTATUS             CreationStatus;
} PS_CREATE_NOTIFY_INFO, *PPS_CREATE_NOTIFY_INFO;

```

*(Archivo completo guardado localmente en `C:\Users\andre\OneDrive\Documentos\SigDead\combo_Window_Extractor\MAPA de Window\WINE_Reference_DNA\ntddk.h`)*

---

## `wdm.h`
Total de estructuras extraídas: 113

### Muestra de Estructuras Clave:
```c
typedef struct _DISPATCHER_HEADER {
  UCHAR  Type;
  UCHAR  Absolute;
  UCHAR  Size;
  UCHAR  Inserted;
  LONG  SignalState;
  LIST_ENTRY  WaitListHead;
} DISPATCHER_HEADER, *PDISPATCHER_HEADER;

typedef struct _KEVENT {
  DISPATCHER_HEADER  Header;
} KEVENT, *PKEVENT, *RESTRICTED_POINTER PRKEVENT;

typedef struct _KSEMAPHORE {
  DISPATCHER_HEADER  Header;
  LONG Limit;
} KSEMAPHORE, *PKSEMAPHORE, *PRKSEMAPHORE;

    struct {
      UCHAR  Type;
      UCHAR  Importance;
      volatile USHORT  Number;
    } DUMMYSTRUCTNAME;

typedef struct _KDEVICE_QUEUE_ENTRY {
  LIST_ENTRY  DeviceListEntry;
  ULONG  SortKey;
  BOOLEAN  Inserted;
} KDEVICE_QUEUE_ENTRY, *PKDEVICE_QUEUE_ENTRY,

typedef struct _KDEVICE_QUEUE {
  CSHORT  Type;
  CSHORT  Size;
  LIST_ENTRY  DeviceListHead;
  KSPIN_LOCK  Lock;
  BOOLEAN  Busy;
} KDEVICE_QUEUE, *PKDEVICE_QUEUE, *RESTRICTED_POINTER PRKDEVICE_QUEUE;

    struct _KTHREAD *RESTRICTED_POINTER OwnerThread;
    BOOLEAN Abandoned;
    UCHAR ApcDisable;
} KMUTANT, *PKMUTANT, *RESTRICTED_POINTER PRKMUTANT, KMUTEX, *PKMUTEX, *RESTRICTED_POINTER PRKMUTEX;

typedef struct _DEFERRED_REVERSE_BARRIER
{
    ULONG Barrier;
    ULONG TotalProcessors;
} DEFERRED_REVERSE_BARRIER;

    struct _KWAIT_BLOCK *RESTRICTED_POINTER NextWaitBlock;
    USHORT WaitKey;
    USHORT WaitType;
} KWAIT_BLOCK, *PKWAIT_BLOCK, *RESTRICTED_POINTER PRKWAIT_BLOCK;

        struct
        {
            ULONG IoPriorityBoosted : 1;
            ULONG OwnerReferenced : 1;
            ULONG IoQoSPriorityBoosted : 1;
            ULONG OwnerCount : 29;
        };

        struct
        {
            UCHAR ReservedLowFlags;
            UCHAR WaiterPriority;
        };

typedef struct _KAPC_STATE
{
     LIST_ENTRY ApcListHead[2];
     PKPROCESS Process;
     UCHAR KernelApcInProgress;
     UCHAR KernelApcPending;
     UCHAR UserApcPending;
} KAPC_STATE, *PKAPC_STATE;

typedef struct _FAST_MUTEX
{
    LONG Count;
    PKTHREAD Owner;
    ULONG Contention;
    KEVENT Event;
    ULONG OldIrql;
} FAST_MUTEX, *PFAST_MUTEX;

          struct
          {
               SHORT KernelApcDisable;
               SHORT SpecialApcDisable;
          };

  struct _DEVICE_OBJECT  *RealDevice;
  ULONG  SerialNumber;
  ULONG  ReferenceCount;
  WCHAR  VolumeLabel[MAXIMUM_VOLUME_LABEL_LENGTH / sizeof(WCHAR)];
} VPB, *PVPB;

```

*(Archivo completo guardado localmente en `C:\Users\andre\OneDrive\Documentos\SigDead\combo_Window_Extractor\MAPA de Window\WINE_Reference_DNA\wdm.h`)*

---

## `shobjidl.h`
Total de estructuras extraídas: 13

### Muestra de Estructuras Clave:
```c
    typedef struct
    {
        GUID guidSearch;
        WCHAR wszFriendlyName[80];
        /*
         *WCHAR wszMenuText[80];
         *WCHAR wszHelpText[MAX_PATH];
         */
        WCHAR wszUrl[2084];
        /*
         *WCHAR wszIcon[MAX_PATH+10];
         *WCHAR wszGreyIcon[MAX_PATH+10];
         *WCHAR wszClrIcon[MAX_PATH+10];
         */
    } EXTRASEARCH, *LPEXTRASEARCH;

    typedef struct
    {
        GUID    fmtid;
        DWORD   pid;
    } SHCOLUMNID, *LPSHCOLUMNID;

typedef struct
{
    UINT ViewMode;
    UINT fFlags;
} FOLDERSETTINGS, *PFOLDERSETTINGS, *LPFOLDERSETTINGS;

    typedef struct _SV2CVW2_PARAMS
    {
        DWORD cbSize;
        IShellView *psvPrev;
        LPCFOLDERSETTINGS pfs;
        IShellBrowser *psbOwner;
        RECT *prcView;
        SHELLVIEWID const *pvid;
        HWND hwndView;
    } SV2CVW2_PARAMS, *LPSV2CVW2_PARAMS;

typedef struct SORTCOLUMN
{
    PROPERTYKEY propkey;
    SORTDIRECTION direction;
} SORTCOLUMN;

    typedef struct
    {
        LPITEMIDLIST	pidlTargetFolder;
	WCHAR		szTargetParsingName[MAX_PATH];
	WCHAR		szNetworkProvider[MAX_PATH];
	DWORD		dwAttributes;
	int		csidl;
    } PERSIST_FOLDER_TARGET_INFO;

typedef struct
{
    SIZE     sizeDragImage;
    POINT    ptOffset;
    HBITMAP  hbmpDragImage;
    COLORREF crColorKey;
} SHDRAGIMAGE, *LPSHDRAGIMAGE;

    typedef struct tagCMINVOKECOMMANDINFO
    {
        DWORD cbSize;
        DWORD fMask;
        HWND hwnd;
        LPCSTR lpVerb;
        LPCSTR lpParameters;
        LPCSTR lpDirectory;
        INT nShow;
        DWORD dwHotKey;
        HANDLE hIcon;
    } CMINVOKECOMMANDINFO, *LPCMINVOKECOMMANDINFO;

    typedef struct tagCMInvokeCommandInfoEx
    {
        DWORD cbSize;
        DWORD fMask;
        HWND hwnd;
        LPCSTR lpVerb;
        LPCSTR lpParameters;
        LPCSTR lpDirectory;
        INT nShow;
        DWORD dwHotKey;
        HANDLE hIcon;
        LPCSTR lpTitle;
        LPCWSTR lpVerbW;
        LPCWSTR lpParametersW;
        LPCWSTR lpDirectoryW;
        LPCWSTR lpTitleW;
        POINT ptInvoke;
    } CMINVOKECOMMANDINFOEX, *LPCMINVOKECOMMANDINFOEX;

typedef struct THUMBBUTTON {
    THUMBBUTTONMASK dwMask;
    UINT iId;
    UINT iBitmap;
    HICON hIcon;
    WCHAR szTip[260];
    THUMBBUTTONFLAGS dwFlags;
} THUMBBUTTON, *LPTHUMBBUTTON;

    typedef struct NSTCCUSTOMDRAW
    {
        IShellItem *psi;
        UINT uItemState;
        NSTCITEMSTATE nstcis;
        LPCWSTR pszText;
        int iImage;
        HIMAGELIST himl;
        int iLevel;
        int iIndent;
    } NSTCCUSTOMDRAW;

typedef struct tagKNOWNFOLDER_DEFINITION
{
    KF_CATEGORY         category;
    LPWSTR              pszName;
    LPWSTR              pszDescription;
    KNOWNFOLDERID       fidParent;
    LPWSTR              pszRelativePath;
    LPWSTR              pszParsingName;
    LPWSTR              pszTooltip;
    LPWSTR              pszLocalizedName;
    LPWSTR              pszIcon;
    LPWSTR              pszSecurity;
    DWORD               dwAttributes;
    KF_DEFINITION_FLAGS kfdFlags;
    FOLDERTYPEID        ftidType;
} KNOWNFOLDER_DEFINITION;

    typedef struct PREVIEWHANDLERFRAMEINFO
    {
        HACCEL haccel;
        UINT   cAccelEntries;
    } PREVIEWHANDLERFRAMEINFO;

```

*(Archivo completo guardado localmente en `C:\Users\andre\OneDrive\Documentos\SigDead\combo_Window_Extractor\MAPA de Window\WINE_Reference_DNA\shobjidl.h`)*

---

## `dwmapi.h`
Total de estructuras extraídas: 6

### Muestra de Estructuras Clave:
```c
typedef struct _UNSIGNED_RATIO {
    UINT32 uiNumerator;
    UINT32 uiDenominator;
} UNSIGNED_RATIO;

typedef struct _DWM_TIMING_INFO {
    UINT32 cbSize;
    UNSIGNED_RATIO rateRefresh;
    QPC_TIME qpcRefreshPeriod;
    UNSIGNED_RATIO rateCompose;
    QPC_TIME qpcVBlank;
    DWM_FRAME_COUNT cRefresh;
    UINT cDXRefresh;
    QPC_TIME qpcCompose;
    DWM_FRAME_COUNT cFrame;
    UINT cDXPresent;
    DWM_FRAME_COUNT cRefreshFrame;
    DWM_FRAME_COUNT cFrameSubmitted;
    UINT cDXPresentSubmitted;
    DWM_FRAME_COUNT cFrameConfirmed;
    UINT cDXPresentConfirmed;
    DWM_FRAME_COUNT cRefreshConfirmed;
    UINT cDXRefreshConfirmed;
    DWM_FRAME_COUNT cFramesLate;
    UINT cFramesOutstanding;
    DWM_FRAME_COUNT cFrameDisplayed;
    QPC_TIME qpcFrameDisplayed;
    DWM_FRAME_COUNT cRefreshFrameDisplayed;
    DWM_FRAME_COUNT cFrameComplete;
    QPC_TIME qpcFrameComplete;
    DWM_FRAME_COUNT cFramePending;
    QPC_TIME qpcFramePending;
    DWM_FRAME_COUNT cFramesDisplayed;
    DWM_FRAME_COUNT cFramesComplete;
    DWM_FRAME_COUNT cFramesPending;
    DWM_FRAME_COUNT cFramesAvailable;
    DWM_FRAME_COUNT cFramesDropped;
    DWM_FRAME_COUNT cFramesMissed;
    DWM_FRAME_COUNT cRefreshNextDisplayed;
    DWM_FRAME_COUNT cRefreshNextPresented;
    DWM_FRAME_COUNT cRefreshesDisplayed;
    DWM_FRAME_COUNT cRefreshesPresented;
    DWM_FRAME_COUNT cRefreshStarted;
    ULONGLONG cPixelsReceived;
    ULONGLONG cPixelsDrawn;
    DWM_FRAME_COUNT cBuffersEmpty;
} DWM_TIMING_INFO;

typedef struct _MilMatrix3x2D
{
    DOUBLE S_11;
    DOUBLE S_12;
    DOUBLE S_21;
    DOUBLE S_22;
    DOUBLE DX;
    DOUBLE DY;
} MilMatrix3x2D;

typedef struct _DWM_BLURBEHIND
{
    DWORD dwFlags;
    BOOL fEnable;
    HRGN hRgnBlur;
    BOOL fTransitionOnMaximized;
} DWM_BLURBEHIND, *PDWM_BLURBEHIND;

typedef struct _DWM_THUMBNAIL_PROPERTIES
{
    DWORD dwFlags;
    RECT  rcDestination;
    RECT  rcSource;
    BYTE  opacity;
    BOOL  fVisible;
    BOOL  fSourceClientAreaOnly;
} DWM_THUMBNAIL_PROPERTIES, *PDWM_THUMBNAIL_PROPERTIES;

typedef struct _DWM_PRESENT_PARAMETERS {
    UINT32 cbSize;
    BOOL fQueue;
    DWM_FRAME_COUNT cRefreshStart;
    UINT cBuffer;
    BOOL fUseSourceRate;
    UNSIGNED_RATIO rateSource;
    UINT cRefreshesPerFrame;
    DWM_SOURCE_FRAME_SAMPLING eSampling;
} DWM_PRESENT_PARAMETERS;

```

*(Archivo completo guardado localmente en `C:\Users\andre\OneDrive\Documentos\SigDead\combo_Window_Extractor\MAPA de Window\WINE_Reference_DNA\dwmapi.h`)*

---

## `uxtheme.h`
Total de estructuras extraídas: 6

### Muestra de Estructuras Clave:
```c
typedef struct _DTBGOPTS {
    DWORD dwSize;
    DWORD dwFlags;
    RECT rcClip;
} DTBGOPTS, *PDTBGOPTS;

typedef struct _DTTOPTS {
    DWORD dwSize;
    DWORD dwFlags;
    COLORREF crText;
    COLORREF crBorder;
    COLORREF crShadow;
    int iTextShadowType;
    POINT ptShadowOffset;
    int iBorderSize;
    int iFontPropId;
    int iColorPropId;
    int iStateId;
    BOOL fApplyOverlay;
    int iGlowSize;
    DTT_CALLBACK_PROC pfnDrawTextCallback;
    LPARAM lParam;
} DTTOPTS, *PDTTOPTS;

typedef struct _INTLIST {
    int iValueCount;
    int iValues[MAX_INTLIST_COUNT];
} INTLIST, *PINTLIST;

typedef struct _MARGINS {
    int cxLeftWidth;
    int cxRightWidth;
    int cyTopHeight;
    int cyBottomHeight;
} MARGINS, *PMARGINS;

typedef struct _BP_PAINTPARAMS
{
	DWORD cbSize;
	DWORD dwFlags;
	const RECT *prcExclude;
	const BLENDFUNCTION *pBlendFunction;
} BP_PAINTPARAMS, *PBP_PAINTPARAMS;

typedef struct _BP_ANIMATIONPARAMS
{
    DWORD cbSize;
    DWORD dwFlags;
    BP_ANIMATIONSTYLE style;
    DWORD dwDuration;
} BP_ANIMATIONPARAMS, *PBP_ANIMATIONPARAMS;

```

*(Archivo completo guardado localmente en `C:\Users\andre\OneDrive\Documentos\SigDead\combo_Window_Extractor\MAPA de Window\WINE_Reference_DNA\uxtheme.h`)*

---

