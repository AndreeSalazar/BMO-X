#![allow(non_camel_case_types, non_snake_case, dead_code)]
use core::ffi::c_void;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DRIVER_INITIALIZATION_DATA {
    pub Version: u32,
    pub DxgkDdiAddDevice: Option<unsafe extern "C" fn() -> i32>,
    pub DxgkDdiStartDevice: Option<unsafe extern "C" fn() -> i32>,
    pub DxgkDdiQueryAdapterInfo: Option<unsafe extern "C" fn() -> i32>,
    pub padding: [u8; 1024],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Other {
    // Nested struct/union omitted for brevity
    pub VideoOutput: u32,
    // Nested struct/union omitted for brevity
    pub MustBeZero: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct HotPlug {
    pub Type: u32,
    pub ChildUid: u32,
    // Nested struct/union omitted for brevity
    // Nested struct/union omitted for brevity
    pub Connected: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SyncLockEnableSync {
    pub Header: u32,
    // Nested struct/union omitted for brevity
    // Nested struct/union omitted for brevity
// //     pub 1: u32,
// //     pub 1: u32,
// //     pub 1: u32,
//     pub :29: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct EldInfo {
    pub ContainerId: u32,
    // Nested struct/union omitted for brevity
    pub PortId: u32,
    pub ManufacturerName: u16,
    pub ProductCode: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_DISPLAYSTATE_NONINTRUSIVE {
    pub In: u32,
    pub Out: u32,
    pub Out: u32,
    pub Out: u32,
    pub Out: u32,
    pub Out: u32,
    pub DXGK_DIAG_GETDISPLAYSTATE_SUBSTATUS_FLAGS: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_DIAG_DISPLAY_SAMPLED_GAMMA {
    pub Out: u32,
    pub Out: u32,
    pub Out: u32,
    pub Out: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_DIAG_DISPLAY_SCANOUT_BUFFER_HISTOGRAM {
    pub Out: i32,
    pub Out: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_DIAG_SCANOUT_BUFFER_CONTENT {
    pub Out: u32,
    pub Out: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_DISPLAYSTATE_INTRUSIVE {
    pub In: u32,
    pub Out: u32,
    pub Out: u32,
    pub Out: u32,
    pub Out: u32,
    pub Out: u32,
    pub Out: u32,
    pub DXGK_DIAG_GETDISPLAYSTATE_SUBSTATUS_FLAGS: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_RENDER {
    pub pCommand: u32,
    pub CommandLength: u32,
    pub pDmaBuffer: *mut c_void,
    pub DmaSize: u32,
    pub pDmaBufferPrivateData: *mut c_void,
    pub DmaBufferPrivateDataSize: u32,
    pub pAllocationList: *mut c_void,
    pub AllocationListSize: u32,
    pub pPatchLocationListIn: *mut c_void,
    pub PatchLocationListInSize: u32,
    pub pPatchLocationListOut: *mut c_void,
    pub PatchLocationListOutSize: u32,
    pub MultipassOffset: u32,
    pub DmaBufferSegmentId: u32,
    pub DmaBufferPhysicalAddress: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_PRESENTALLOCATIONINFO {
    pub hDeviceSpecificAllocation: *mut c_void,
    pub AllocationVirtualAddress: u32,
    pub PhysicalAddress: u64,
    pub SegmentId: u16,
    pub PhysicalAdapterIndex: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_PRESENTMULTIPLANEOVERLAYINFO {
    pub VidPnSourceId: u32,
    pub PlaneListCount: u32,
    pub pPlaneList: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_ATTRIBUTES {
    pub Flags: u32,
    pub SrcRect: u32,
    pub DstRect: u32,
    pub ClipRect: u32,
    pub Rotation: u32,
    pub Blend: u32,
    pub NumFilters: u32,
    pub pFilters: *mut c_void,
    pub VideoFrameFormat: u32,
    pub YCbCrFlags: u32,
    pub StereoFormat: u32,
    pub StereoLeftViewFrame0: i32,
    pub StereoBaseViewFrame0: i32,
    pub StereoFlipMode: u32,
    pub StretchQuality: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_CHECK_MULTIPLANE_OVERLAY_SUPPORT_PLANE {
    pub hAllocation: *mut c_void,
    pub VidPnSourceId: u32,
    pub PlaneAttributes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_PLANE {
    pub LayerIndex: u32,
    pub Enabled: i32,
    pub AllocationSegment: u32,
    pub AllocationAddress: u64,
    pub hAllocation: *mut c_void,
    pub PlaneAttributes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_ATTRIBUTES2 {
    pub Reserved1: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_PLANE_WITH_SOURCE {
    pub number: u32,
    pub LayerIndex: u32,
    // Nested struct/union omitted for brevity
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CHECKMULTIPLANEOVERLAYSUPPORT2 {
    pub DXGK_MULTIPLANE_OVERLAY_PLANE_WITH_SOURCE: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_GDIARG_BITBLT {
    pub rectangle: u32,
    pub rectangle: u32,
    pub list: u32,
    pub list: u32,
    pub space: *mut c_void,
    pub DXGK_GDIROP_BITBLT: u16,
    pub DXGK_GDIROP_ROP3: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_GDIARG_COLORFILL {
    pub rectangle: u32,
    pub list: u32,
    pub space: u32,
    pub space: *mut c_void,
    pub surface: u32,
    pub DXGK_GDIROP_COLORFILL: u16,
    pub DXGK_GDIROP_ROP3: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_GDIARG_ALPHABLEND {
    pub rectangle: u32,
    pub rectangle: u32,
    pub list: u32,
    pub list: u32,
    pub space: *mut c_void,
    pub SourceConstantAlpha: u8,
    pub SourceHasAlpha: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_GDIARG_TRANSPARENTBLT {
    pub rectangle: u32,
    pub rectangle: u32,
    pub list: u32,
    pub list: u32,
    pub space: *mut c_void,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_GDIARG_CLEARTYPEBLEND {
    pub rectangle: u32,
    pub TmpSurfAllocationIndex: u32,
    pub GammaSurfAllocationIndex: u32,
    pub AlphaSurfAllocationIndex: u32,
    pub DstAllocationIndex: u32,
    pub tables: u32,
    pub space: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Command {
    pub OpCode: u32,
    pub DXGK_RENDERKM_COMMAND: u32,
    // Nested struct/union omitted for brevity
    pub BitBlt: u32,
    pub ColorFill: u32,
    pub AlphaBlend: u32,
    pub StretchBlt: u32,
    pub TransparentBlt: u32,
    pub ClearTypeBlend: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_STOPCAPTURE {
    pub hAllocation: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_VSYNC_INFO {
    pub LayerIndex: u32,
    pub Enabled: i32,
    pub PhysicalAddress: u64,
    pub PlaneAttributes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_VSYNC_INFO2 {
    pub LayerIndex: u32,
    pub PresentId: u64,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_FLIPQUEUE_LOG_ENTRY {
    pub PresentId: u64,
    pub PresentTimestamp: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETFLIPQUEUELOGBUFFER {
    pub array: u32,
    pub memory: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_UPDATEFLIPQUEUELOG {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CANCELQUEUEDFLIPS {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_CANCELFLIPS_PLANE {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CANCELFLIPS {
    pub cancel: u32,
    pub request: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETINTERRUPTTARGETPRESENTID {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_VSYNC_INFO3 {
    pub LayerIndex: u32,
    pub FirstFreeFlipQueueLogEntryIndex: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETALLOCATIONBACKINGSTORE {
    pub DxgkDdiCreateAllocation: *mut c_void,
    pub space: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CREATECPUEVENT {
    pub r#in: *mut c_void,
    pub r#in: *mut c_void,
    pub r#in: u32,
    pub out: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_OPENALLOCATIONINFO {
    pub handle: u32,
    pub driver: *mut c_void,
    pub data: u32,
    pub it: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_OPENALLOCATION {
    pub NumAllocations: u32,
    pub pOpenAllocation: *mut c_void,
    pub pPrivateDriverData: *mut c_void,
    pub PrivateDriverSize: u32,
    pub Flags: u32,
    pub SubresourceIndex: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CLOSEALLOCATION {
    pub NumAllocations: u32,
    pub list: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_DEVICEINFO {
    pub DmaBufferSize: u32,
    pub DmaBufferSegmentSet: u32,
    pub DmaBufferPrivateDataSize: u32,
    pub AllocationListSize: u32,
    pub PatchLocationListSize: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_CONTEXTINFO {
    pub DmaBufferSize: u32,
    pub DmaBufferSegmentSet: u32,
    pub DmaBufferPrivateDataSize: u32,
    pub AllocationListSize: u32,
    pub PatchLocationListSize: u32,
    pub Reserved: u32,
    pub context: u32,
    pub operations: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CREATECONTEXT {
    pub handle: *mut c_void,
    pub data: *mut c_void,
    pub data: u32,
    pub driver: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CREATEHWCONTEXT {
    pub handle: *mut c_void,
    pub data: u32,
    pub data: *mut c_void,
    pub driver: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CREATEHWQUEUE {
    pub handle: *mut c_void,
    pub data: u32,
    pub data: *mut c_void,
    pub CPU: *mut c_void,
    pub GPU: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETPOINTERSHAPE {
    pub Flags: u32,
    pub Width: u32,
    pub Height: u32,
    pub Pitch: u32,
    pub VidPnSourceId: u32,
    pub pPixels: u32,
    pub XHot: u32,
    pub YHot: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETPOINTERPOSITION {
    pub VidPnSourceId: u32,
    pub X: i32,
    pub Y: i32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_PAGE_TABLE_LEVEL_DESC {
    pub PageTableIndexBitCount: u32,
    pub PageTableSegmentId: u32,
    pub PagingProcessPageTableSegmentId: u32,
    pub PageTableSizeInBytes: u32,
    pub segment: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_UPDATEPAGETABLEFLAGS {
// //     pub 1: u32,
// //     pub 1: u32,
// //     pub 1: u32,
// //     pub 1: u32,
// //     pub 27: u32,
// //     pub 28: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYGPUMMUCAPSIN {
    pub PhysicalAdapterIndex: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYPAGETABLELEVELDESCIN {
    pub LevelIndex: u16,
    pub PhysicalAdapterIndex: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYHISTORYBUFFERPRECISIONIN {
    pub PhysicalAdapterIndex: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYPHYSICALADAPTERCAPSIN {
    pub PhysicalAdapterIndex: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_PHYSICALADAPTERCAPS {
    pub NumExecutionNodes: u16,
    pub PagingNodeIndex: u16,
    pub DxgkPhysicalAdapterHandle: *mut c_void,
    pub Flags: u32,
    pub VPRPagingNode: u32,
    pub VirtualCopyNodeIndex: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_CPUHOSTAPERTURE {
    pub address: u64,
    pub pages: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_FRAMEBUFFERSAVEAREA {
    pub MaximumSize: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_PHYSICAL_MEMORY_RANGE {
    pub BaseAddress: u64,
    pub NumberOfBytes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_HARDWARERESERVEDRANGES {
    pub NumRanges: u32,
    pub pPhysicalRanges: *mut u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_GPUENGINETOPOLOGY {
    pub NbAsymetricProcessingNodes: u32,
    pub Reserved: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_WDDMDEVICECAPSIN {
    pub DxgkDdiStartDevice: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_WDDMDEVICECAPS {
//     pub DXGK_DRIVERCAPS::WDDMVersion: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_SEGMENTDESCRIPTOR {
    pub r#for: u64,
    pub address: u64,
    pub be: usize,
    pub flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYSEGMENTIN {
    pub flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYSEGMENTOUT {
    pub buffer: u32,
    pub PagingBufferPrivateDataSize: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_SEGMENTDESCRIPTOR2 {
    pub flags: u32,
    pub segment: u32,
    pub TEMPORARY: u64,
    pub TEMPORARY: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYSEGMENTOUT2 {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_SEGMENTDESCRIPTOR3 {
    pub flags: u32,
    pub r#for: u64,
    pub address: u64,
    pub be: usize,
    pub composed: usize,
    pub Reserved: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYSEGMENTOUT3 {
    pub buffer: u32,
    pub PagingBufferPrivateDataSize: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYSEGMENTIN4 {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYSEGMENTOUT4 {
    pub element: *mut c_void,
    pub buffer: u32,
    pub PagingBufferPrivateDataSize: u32,
    pub the: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MEMORYRANGE {
    pub segment: u64,
    pub bytes: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QAITARGETIN {
    pub TargetId: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QAISOURCEIN {
    pub Source: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYDISPLAYIDIN {
    pub TargetId: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYDISPLAYIDOUT {
    pub Length: u32,
    pub pDescriptor: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_PHYSICAL_MEMORY_CAPS {
    pub HighestVisibleAddress: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_NATIVE_FENCE_CAPS {
    pub to: u8,
    pub Reserved: [u8; 28],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CREATENATIVEFENCE {
    pub handle: *mut c_void,
    pub object: u32,
    pub space: u32,
    pub space: u32,
    pub UMD: u8,
    pub Flags: u32,
    pub Reserved: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_OPENNATIVEFENCE {
    pub DdiCreateNativeFence: *mut c_void,
    pub object: *mut c_void,
    pub object: *mut c_void,
    pub space: u32,
    pub space: u32,
    pub Flags: u32,
    pub UMD: u8,
    pub Reserved: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CLOSENATIVEFENCE {
    pub DdiOpenNativeFence: *mut c_void,
    pub Flags: u32,
    pub Reserved: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_DESTROYNATIVEFENCE {
    pub DdiCreateNativeFence: *mut c_void,
    pub Flags: u32,
    pub Reserved: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_UPDATEMONITOREDVALUES {
    pub Flags: u32,
    pub Reserved: [u8; 28],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_UPDATECURRENTVALUESFROMCPU {
    pub Flags: u32,
    pub Reserved: [u8; 28],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETNATIVEFENCELOGBUFFER {
    pub to: *mut c_void,
    pub array: u32,
    pub buffer: *mut c_void,
    pub space: u32,
    pub space: u32,
    pub Flags: u32,
    pub Reserved: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_UPDATENATIVEFENCELOGS {
    pub array: u32,
    pub Flags: u32,
    pub Reserved: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_POWER_RUNTIME_STATE {
    pub TransitionLatency: u64,
    pub ResidencyRequirement: u64,
    pub NominalPower: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct EngineDesc {
    pub ComponentType: u32,
    // Nested struct/union omitted for brevity
    // Nested struct/union omitted for brevity
    pub NodeIndex: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_POWER_RUNTIME_COMPONENT {
    pub StateCount: u32,
    pub States: [u32; 1],
    pub ComponentMapping: u32,
    pub Flags: u32,
    pub ComponentGuid: u32,
    pub ComponentName: [u8; 1],
    pub ProviderCount: u32,
    pub Providers: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_POWER_P_STATE {
    pub OperatingFrequency: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_POWER_P_COMPONENT {
    pub StateCount: u32,
    pub States: [u32; 1],
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_QUERYADAPTERINFO {
    pub Type: u32,
    pub pInputData: *mut c_void,
    pub InputDataSize: u32,
    pub pOutputData: *mut c_void,
    pub OutputDataSize: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_ACQUIRESWIZZLINGRANGE {
    pub hAllocation: *mut c_void,
    pub LockCB: u32,
    pub RangeId: u32,
    pub SegmentId: u32,
    pub RangeSize: usize,
    pub CPUTranslatedAddress: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_RELEASESWIZZLINGRANGE {
    pub hAllocation: *mut c_void,
    pub PrivateDriverData: u32,
    pub RangeId: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_ALLOCATIONUSAGEHINT {
    pub Version: u32,
    pub v1: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_ALLOCATIONINFO_TEST {
    pub Alignment: u32,
    pub size: u64,
    pub HintedBank: u32,
    pub PreferredSegment: u32,
    pub SupportedReadSegmentSet: u32,
    pub SupportedWriteSegmentSet: u32,
    pub EvictionSegmentSet: u32,
    pub only: u32,
    pub fields: u32,
    pub Flags2: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CREATEALLOCATION {
    pub pPrivateDriverData: u32,
    pub PrivateDriverDataSize: u32,
    pub NumAllocations: u32,
    pub pAllocationInfo: *mut c_void,
    pub hResource: *mut c_void,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_DESCRIBEALLOCATION {
    pub CreateAllocation: *mut c_void,
    pub allocation: u32,
    pub allocation: u32,
    pub allocation: u32,
    pub allocation: u32,
    pub applicable: u32,
    pub allocation: u32,
    pub Flags: u32,
    pub mode: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_FENCESTORAGEFLAGS {
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_DESTROYALLOCATION {
    pub NumAllocations: u32,
    pub pAllocationList: u32,
    pub hResource: *mut c_void,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_PREEMPTCOMMAND {
    pub id: u32,
    pub preempt: u32,
    pub preempt: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CANCELCOMMAND {
    pub beginning: u32,
    pub beginning: u32,
    pub private: *mut c_void,
    pub the: u32,
    pub the: u32,
    pub list: u32,
    pub associated: u32,
    pub contexts: u32,
    pub UMD: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_QUERYCURRENTFENCE {
    pub CurrentFence: u32,
    pub NodeOrdinal: u32,
    pub EngineOrdinal: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BUILDPAGINGBUFFER_COPY_RANGE {
    pub NumPageTableEntries: u32,
    pub SrcPageTableAddress: u32,
    pub DstPageTableAddress: u32,
    pub SrcStartPteIndex: u32,
    pub DstStartPteIndex: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BUILDPAGINGBUFFER_FLUSHTLB {
    pub RootPageTableAddress: u32,
    pub hProcess: *mut c_void,
    pub StartVirtualAddress: u32,
    pub EndVirtualAddress: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BUILDPAGINGBUFFER_UPDATEPAGETABLE {
    pub PageTableLevel: u32,
    pub hAllocation: *mut c_void,
    pub PageTableAddress: u32,
    pub pPageTableEntries: *mut c_void,
    pub StartIndex: u32,
    pub NumPageTableEntries: u32,
    pub Reserved0: u32,
    pub Flags: u32,
    pub DriverProtection: u64,
    pub AllocationOffsetInBytes: u64,
    pub hProcess: *mut c_void,
    pub UpdateMode: u32,
    pub pPageTableEntries64KB: *mut c_void,
    pub FirstPteVirtualAddress: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BUILDPAGINGBUFFER_FILLVIRTUAL {
    pub hAllocation: *mut c_void,
    pub AllocationOffsetInBytes: u64,
    pub FillSizeInBytes: u64,
    pub FillPattern: u32,
    pub DestinationVirtualAddress: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BUILDPAGINGBUFFER_TRANSFERVIRTUAL {
    pub hAllocation: *mut c_void,
    pub AllocationOffsetInBytes: u64,
    pub TransferSizeInBytes: u64,
    pub SourceVirtualAddress: u32,
    pub DestinationVirtualAddress: u32,
    pub SourcePageTable: u32,
    pub TransferDirection: u32,
    pub Flags: u32,
    pub DestinationPageTable: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BUILDPAGINGBUFFER_COPYPAGETABLEENTRIES {
    pub NumRanges: u32,
    pub pRanges: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BUILDPAGINGBUFFER_UPDATECONTEXTALLOCATION {
    pub ContextAllocation: u32,
    pub ContextAllocationSize: u64,
    pub pDriverPrivateData: *mut c_void,
    pub DriverPrivateDataSize: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BUILDPAGINGBUFFER_SIGNALMONITOREDFENCE {
    pub MonitoredFenceGpuVa: u32,
    pub MonitoredFenceValue: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_FENCE_RESIDENCY_INFO {
    pub updated: *mut c_void,
    pub fence: u32,
    pub fence: u32,
    pub value: *mut c_void,
    pub value: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Flags {
    pub MinimumSize: u32,
    pub MaximumSize: u32,
    // Nested struct/union omitted for brevity
    // Nested struct/union omitted for brevity
    // Nested struct/union omitted for brevity
// //     pub 0x00000001: u32,
// //     pub 0xFFFFFFFE: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BUILDPAGINGBUFFER_MAPMMU {
    pub hAllocation: *mut c_void,
    pub VirtualAddress: u64,
    pub MmuId: u32,
    pub SegmentId: u32,
    pub AllocationOffsetInPages: u32,
    pub Adl: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BUILDPAGINGBUFFER_UNMAPMMU {
    pub hAllocation: *mut c_void,
    pub VirtualAddress: u64,
    pub MmuId: u32,
    pub Reserved0: u32,
    pub AllocationOffset: u32,
    pub NumberOfPages: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BUILDPAGINGBUFFER_NOTIFYALLOC {
    pub hAllocation: *mut c_void,
    pub hKmdProcessHandle: *mut c_void,
    pub Flags: u32,
    pub OffsetInBytes: u64,
    pub SizeInBytes: u64,
    pub GpuVirtualAddressAtOffset: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETROOTPAGETABLE {
    pub hContext: *mut c_void,
    pub Address: u32,
    pub NumEntries: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_GETROOTPAGETABLESIZE {
    pub In: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CREATEPROCESS {
    pub r#in: *mut c_void,
    pub out: *mut c_void,
    pub r#in: u32,
    pub r#in: u32,
    pub r#in: *mut c_void,
    pub r#in: u32,
    pub NULL: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SUBMITCOMMANDVIRTUAL {
    pub hContext: *mut c_void,
    pub DmaBufferVirtualAddress: u32,
    pub DmaBufferSize: u32,
    pub pDmaBufferPrivateData: *mut c_void,
    pub DmaBufferPrivateDataSize: u32,
    pub DmaBufferUmdPrivateDataSize: u32,
    pub SubmissionFenceId: u32,
    pub VidPnSourceId: u32,
    pub FlipInterval: u32,
    pub Flags: u32,
    pub EngineOrdinal: u32,
    pub NodeOrdinal: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SUBMITCOMMANDTOHWQUEUE {
    pub hHwQueue: *mut c_void,
    pub DmaBufferVirtualAddress: u32,
    pub DmaBufferSize: u32,
    pub DmaBufferPrivateDataSize: u32,
    pub pDmaBufferPrivateData: *mut c_void,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SWITCHTOHWCONTEXTLIST {
    pub GPU: *mut c_void,
    // Nested struct/union omitted for brevity
    // Nested struct/union omitted for brevity
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_UPDATEHWCONTEXTSTATE {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_SCHEDULING_LOG_CONTEXT_STATE_CHANGE {
    pub hKmdContext: *mut c_void,
    pub newContextState: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_SCHEDULING_LOG_BUFFER {
    pub Header: u32,
    pub Entries: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETSCHEDULINGLOGBUFFER {
    pub ordinal: u32,
    pub ordinal: u32,
    pub array: u32,
    pub buffer: *mut c_void,
    pub buffer: u32,
    pub to: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETUPPRIORITYBANDS {
    pub gracePeriodForBand: [u64; 1],
    pub processQuantumForBand: [u64; 1],
    pub processGracePeriodForBand: [u64; 1],
    pub targetNormalBandPercentage: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETCONTEXTSCHEDULINGPROPERTIES {
    pub hContext: *mut c_void,
    pub priorityBand: u32,
    pub realtimeBandPriorityLevel: i32,
    pub inProcessPriority: i32,
    pub quantum: u64,
    pub gracePeriodSamePriority: u64,
    pub gracePeriodLowerPriority: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SUSPENDCONTEXT {
    pub hContext: *mut c_void,
    pub contextSuspendFence: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_RESUMECONTEXT {
    pub hContext: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETVIRTUALMACHINEDATA {
    pub r#in: *mut c_void,
    pub r#in: *mut c_void,
    pub r#in: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SIGNALMONITOREDFENCE {
    pub KernelSubmissionType: u32,
    pub pDmaBuffer: *mut c_void,
    pub DmaBufferGpuVirtualAddress: u32,
    pub DmaSize: u32,
    pub pDmaBufferPrivateData: *mut c_void,
    pub DmaBufferPrivateDataSize: u32,
    pub MultipassOffset: u32,
    pub MonitoredFenceGpuVa: u32,
    pub MonitoredFenceValue: u64,
    pub hHwQueue: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_VALIDATESUBMITCOMMAND {
    pub Commands: u32,
    pub CommandLength: u32,
    pub Flags: u32,
    pub ContextCount: u32,
    pub Context: [*mut c_void; 1],
    pub pPrivateDriverData: *mut c_void,
    pub PrivateDriverDataSize: u32,
    pub UmdPrivateDataSize: u32,
    pub HwQueueProgressFenceId: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_RENDERGDI {
    pub pCommand: u32,
    pub CommandLength: u32,
    pub pDmaBuffer: *mut c_void,
    pub DmaBufferGpuVirtualAddress: u32,
    pub DmaSize: u32,
    pub pDmaBufferPrivateData: *mut c_void,
    pub DmaBufferPrivateDataSize: u32,
    pub pAllocationList: *mut c_void,
    pub AllocationListSize: u32,
    pub MultipassOffset: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_MAPCPUHOSTAPERTURE {
    pub hAllocation: *mut c_void,
    pub SegmentId: u16,
    pub PhysicalAdapterIndex: u16,
    pub NumberOfPages: u64,
    pub pCpuHostAperturePages: *mut c_void,
    pub pMemorySegmentPages: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_UNMAPCPUHOSTAPERTURE {
    pub NumberOfPages: u64,
    pub pCpuHostAperturePages: *mut c_void,
    pub SegmentId: u16,
    pub PhysicalAdapterIndex: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETVIDEOPROTECTEDREGION {
    pub In: u32,
    pub In: u32,
    pub In: u32,
    pub In: usize,
    pub In: usize,
    pub In: usize,
    pub In: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_ESCAPE {
    pub handle: *mut c_void,
    pub flags: u32,
    pub data: *mut c_void,
    pub data: u32,
    pub handle: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_COLLECTDBGINFO_EXT {
    pub bucketing: u32,
    pub buffer: u32,
    pub Reserved2: u32,
    pub Reserved3: u32,
    pub Reserved4: u32,
    pub Reserved5: u32,
    pub Reserved6: u32,
    pub Reserved7: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_COLLECTDBGINFO {
    pub report: u32,
    pub info: *mut c_void,
    pub bytes: usize,
    pub extension: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_OVERLAYINFO {
    pub displayed: *mut c_void,
    pub allocation: u64,
    pub allocation: u32,
    pub rect: u32,
    pub rect: u32,
    pub data: *mut c_void,
    pub data: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CREATEOVERLAY {
    pub displayed: u32,
    pub info: u32,
    pub handle: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_UPDATEOVERLAY {
    pub info: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_FLIPOVERLAY {
    pub allocation: *mut c_void,
    pub allocation: u64,
    pub allocation: u32,
    pub data: *mut c_void,
    pub data: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_GETSCANLINE {
    pub ID: u32,
    pub blank: u8,
    pub line: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_ISSUPPORTEDVIDPN {
    pub hDesiredVidPn: u32,
    pub IsVidPnSupported: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_ENUM_PIVOT {
    pub VidPnSourceId: u32,
    pub VidPnTargetId: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_ENUMVIDPNCOFUNCMODALITY {
    pub hConstrainingVidPn: u32,
    pub EnumPivotType: u32,
    pub EnumPivot: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_RECOMMENDFUNCTIONALVIDPN {
    pub NumberOfVidPnTargets: u32,
    pub pVidPnTargetPrioritizationVector: u32,
    pub hRecommendedFunctionalVidPn: u32,
    pub RequestReason: u32,
    pub pPrivateDriverData: u32,
    pub PrivateDriverDataSize: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_ATTRIBUTES3 {
    pub Flags: u32,
    pub SrcRect: u32,
    pub DstRect: u32,
    pub ClipRect: u32,
    pub Rotation: u32,
    pub Blend: u32,
    pub ColorSpaceType: u32,
    pub StretchQuality: u32,
    pub SDRWhiteLevel: u32,
    pub DirtyRectCnt: u32,
    pub pDirtyRects: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_HDR_METADATA {
    pub Type: u32,
    pub Size: u32,
    pub pMetaData: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_PRIMARYDATA {
    pub hAllocation: *mut c_void,
    pub SegmentId: u16,
    pub SegmentAddress: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETVIDPNSOURCEADDRESS {
    pub VidPnSourceId: u32,
    pub PrimarySegment: u32,
    pub PrimaryAddress: u64,
    pub hAllocation: *mut c_void,
    pub ContextCount: u32,
    pub Context: [*mut c_void; 1],
    pub Flags: u32,
    pub Duration: u32,
    pub PrimaryData: [u32; 1],
    pub DriverPrivateDataSize: u32,
    pub pDriverPrivateData: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETVIDPNSOURCEADDRESSWITHMULTIPLANEOVERLAY {
    pub ContextCount: u32,
    pub Context: [*mut c_void; 1],
    pub Flags: u32,
    pub VidPnSourceId: u32,
    pub PlaneCount: u32,
    pub pPlanes: *mut c_void,
    pub Duration: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_PLANE2 {
    // Nested struct/union omitted for brevity
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETVIDPNSOURCEADDRESSWITHMULTIPLANEOVERLAY2 {
    // Nested struct/union omitted for brevity
    // Nested struct/union omitted for brevity
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_PLANE3 {
    pub LayerIndex: u32,
    pub PresentId: u64,
    pub InputFlags: u32,
    pub OutputFlags: u32,
    pub MaxImmediateFlipLine: u32,
    pub ContextCount: u32,
    pub ppContextData: u32,
    pub DriverPrivateDataSize: u32,
    pub pDriverPrivateData: *mut c_void,
    pub PlaneAttributes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_POST_COMPOSITION {
    pub Flags: u32,
    pub SrcRect: u32,
    pub DstRect: u32,
    pub Rotation: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETVIDPNSOURCEADDRESSWITHMULTIPLANEOVERLAY3 {
    pub VidPnSourceId: u32,
    pub InputFlags: u32,
    pub OutputFlags: u32,
    pub PlaneCount: u32,
    pub ppPlanes: u32,
    pub pPostComposition: *mut c_void,
    pub Duration: u32,
    pub pHDRMetaData: *mut c_void,
    pub TargetFlipTime: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_POSTMULTIPLANEOVERLAYPRESENT {
    pub VidPnTargetId: u32,
    pub PhysicalAdapterMask: u32,
    pub LayerIndex: u32,
    pub PresentID: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CREATEPERIODICFRAMENOTIFICATION {
    pub VidPnSource: *mut c_void,
    pub to: u32,
    pub notification: u32,
    pub destroy: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_DESTROYPERIODICFRAMENOTIFICATION {
    pub destroy: *mut c_void,
    pub notification: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_GETMULTIPLANEOVERLAYCAPS {
//     pub : [u32; 1],
    pub supported: u32,
    pub capabilities: u32,
//     pub : [u32; 1],
//     pub : [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_GETPOSTCOMPOSITIONCAPS {
//     pub : [u32; 1],
//     pub : [u32; 1],
//     pub : [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CHECKMULTIPLANEOVERLAYSUPPORT {
    pub PlaneCount: u32,
    pub pPlanes: *mut c_void,
    pub Supported: i32,
    pub ReturnInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_PLANE_WITH_SOURCE2 {
    pub hAllocation: *mut c_void,
    pub VidPnSourceId: u32,
    pub LayerIndex: u32,
    pub PlaneAttributes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MULTIPLANE_OVERLAY_POST_COMPOSITION_WITH_SOURCE {
    pub VidPnSourceId: u32,
    pub PostComposition: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CHECKMULTIPLANEOVERLAYSUPPORT3 {
    pub PlaneCount: u32,
    pub ppPlanes: u32,
    pub PostCompositionCount: u32,
    pub ppPostComposition: u32,
    pub Supported: i32,
    pub ReturnInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETVIDPNSOURCEVISIBILITY {
    pub VidPnSourceId: u32,
    pub Visible: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_COMMITVIDPN_FLAGS {
// //     pub 1: u32,
// //     pub 1: u32,
// //     pub 30: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_COMMITVIDPN {
    pub hFunctionalVidPn: u32,
    pub AffectedVidPnSourceId: u32,
    pub MonitorConnectivityChecks: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_UPDATEACTIVEVIDPNPRESENTPATH {
    pub VidPnPresentPathInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_RECOMMENDVIDPNTOPOLOGY {
    pub hVidPn: u32,
    pub VidPnSourceId: u32,
    pub RequestReason: u32,
    pub hFallbackTopology: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_VIDPNTOPOLOGY_INTERFACE {
    pub pfnGetNumPaths: u32,
    pub pfnGetNumPathsFromSource: u32,
    pub pfnEnumPathTargetsFromSource: u32,
    pub pfnGetPathSourceFromTarget: u32,
    pub pfnAcquirePathInfo: u32,
    pub pfnAcquireFirstPathInfo: u32,
    pub pfnAcquireNextPathInfo: u32,
    pub pfnUpdatePathSupportInfo: u32,
    pub pfnReleasePathInfo: u32,
    pub pfnCreateNewPathInfo: u32,
    pub pfnAddPath: u32,
    pub pfnRemovePath: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_VIDPNSOURCEMODESET_INTERFACE {
    pub pfnGetNumModes: u32,
    pub pfnAcquireFirstModeInfo: u32,
    pub pfnAcquireNextModeInfo: u32,
    pub pfnAcquirePinnedModeInfo: u32,
    pub pfnReleaseModeInfo: u32,
    pub pfnCreateNewModeInfo: u32,
    pub pfnAddMode: u32,
    pub pfnPinMode: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_VIDPNTARGETMODESET_INTERFACE {
    pub pfnGetNumModes: u32,
    pub pfnAcquireFirstModeInfo: u32,
    pub pfnAcquireNextModeInfo: u32,
    pub pfnAcquirePinnedModeInfo: u32,
    pub pfnReleaseModeInfo: u32,
    pub pfnCreateNewModeInfo: u32,
    pub pfnAddMode: u32,
    pub pfnPinMode: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_VIDPN_INTERFACE {
    pub Version: u32,
    pub pfnGetTopology: u32,
    pub pfnAcquireSourceModeSet: u32,
    pub pfnReleaseSourceModeSet: u32,
    pub pfnCreateNewSourceModeSet: u32,
    pub pfnAssignSourceModeSet: u32,
    pub pfnAssignMultisamplingMethodSet: u32,
    pub pfnAcquireTargetModeSet: u32,
    pub pfnReleaseTargetModeSet: u32,
    pub pfnCreateNewTargetModeSet: u32,
    pub pfnAssignTargetModeSet: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MONITORSOURCEMODESET_INTERFACE {
    pub pfnGetNumModes: u32,
    pub pfnAcquirePreferredModeInfo: u32,
    pub pfnAcquireFirstModeInfo: u32,
    pub pfnAcquireNextModeInfo: u32,
    pub pfnCreateNewModeInfo: u32,
    pub pfnAddMode: u32,
    pub pfnReleaseModeInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MONITORFREQUENCYRANGESET_INTERFACE {
    pub pfnGetNumFrequencyRanges: u32,
    pub pfnAcquireFirstFrequencyRangeInfo: u32,
    pub pfnAcquireNextFrequencyRangeInfo: u32,
    pub pfnReleaseFrequencyRangeInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MONITORDESCRIPTORSET_INTERFACE {
    pub pfnGetNumDescriptors: u32,
    pub pfnAcquireFirstDescriptorInfo: u32,
    pub pfnAcquireNextDescriptorInfo: u32,
    pub pfnReleaseDescriptorInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MONITOR_INTERFACE {
    pub Version: u32,
    pub pfnAcquireMonitorSourceModeSet: u32,
    pub pfnReleaseMonitorSourceModeSet: u32,
    pub pfnGetMonitorFrequencyRangeSet: u32,
    pub pfnGetMonitorDescriptorSet: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MONITOR_INTERFACE_V2 {
    pub Version: u32,
    pub pfnAcquireMonitorSourceModeSet: u32,
    pub pfnReleaseMonitorSourceModeSet: u32,
    pub pfnGetMonitorFrequencyRangeSet: u32,
    pub pfnGetMonitorDescriptorSet: u32,
    pub pfnGetAdditionalMonitorModeSet: u32,
    pub pfnReleaseAdditionalMonitorModeSet: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MONITOR_INTERFACE_V3 {
    pub Version: u32,
    pub pfnAcquireMonitorSourceModeSet: u32,
    pub pfnReleaseMonitorSourceModeSet: u32,
    pub pfnGetMonitorFrequencyRangeSet: u32,
    pub pfnGetAdditionalMonitorModeSet: u32,
    pub pfnReleaseAdditionalMonitorModeSet: u32,
    pub pfnGetMonitorDescriptor: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_QUERYVIDPNHWCAPABILITY {
    pub hFunctionalVidPn: u32,
    pub SourceId: u32,
    pub TargetId: u32,
    pub VidPnHWCaps: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_PRESENT_DISPLAYONLY {
    pub presented: u32,
    pub image: *mut c_void,
    pub image: u32,
    pub image: i32,
    pub moves: u32,
    pub moves: *mut c_void,
    pub rects: u32,
    pub rects: *mut c_void,
    pub callback: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_QUERYDEPENDENTENGINEGROUP {
    pub reset: u32,
    pub reset: u32,
    pub reset: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_QUERYENGINESTATUS {
    pub ordinal: u32,
    pub ordinal: u32,
    pub status: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_RESETENGINE {
    pub ordinal: u32,
    pub ordinal: u32,
    pub execution: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_RESETHWENGINE {
    pub ordinal: u32,
    pub ordinal: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_INTERFACESPECIFICDATA {
    pub object: *mut c_void,
    pub pfnGetHandleDataCb: u32,
    pub pfnGetHandleParentCb: u32,
    pub pfnEnumHandleChildrenCb: u32,
    pub pfnNotifyInterruptCb: u32,
    pub pfnNotifyDpcCb: u32,
    pub pfnQueryVidPnInterfaceCb: u32,
    pub pfnGetCaptureAddressCb: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETDISPLAYPRIVATEDRIVERFORMAT {
    pub of: u32,
    pub VidPn: *mut c_void,
    pub VidPn: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_RECOMMENDMONITORMODES {
    pub VideoPresentTargetId: u32,
    pub hMonitorSourceModeSet: u32,
    pub pMonitorSourceModeSetInterface: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_HISTORYBUFFERPRECISION {
    pub PrecisionBits: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_HISTORY_BUFFER_HEADER {
    pub RenderCbSequence: u32,
    pub NumTimestamps: u32,
    pub PrivateDataSize: u32,
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_HISTORY_BUFFER {
    pub Header: u32,
    pub DriverPrivateData: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_FORMATHISTORYBUFFER {
    pub pHistoryBuffer: *mut c_void,
    pub HistoryBufferSize: u32,
    pub pFormattedBuffer: *mut c_void,
    pub FormattedBufferSize: u32,
    pub NumTimestamps: u32,
    pub Precision: u32,
    pub Offset: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CONTROLMODEBEHAVIOR {
    pub Request: u32,
    pub Satisfied: u32,
    pub NotSatisfied: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MONITORLINKINFO {
    pub UsageHints: u32,
    pub Capabilities: u32,
    pub DitheringSupport: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_UPDATEMONITORLINKINFO {
    pub VideoPresentTargetId: u32,
    pub MonitorLinkInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MonitorConnect {
    pub ConnectionChangeId: u64,
//     pub :24: u32,
// //     pub 4: u32,
// //     pub 4: u32,
    // Nested struct/union omitted for brevity
    // Nested struct/union omitted for brevity
    pub LinkTargetType: u32,
    pub MonitorConnectFlags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETTIMINGSFROMVIDPN {
    pub hFunctionalVidPn: u32,
    pub SetFlags: u32,
    pub pResultsFlags: u32,
    pub PathCount: u32,
    pub pSetTimingPathInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETTARGETGAMMA {
    pub TargetId: u32,
    pub GammaRamp: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETTARGETCONTENTTYPE {
    pub TargetId: u32,
    pub ContentType: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETTARGETANALOGCOPYPROTECTION {
    pub TargetId: u32,
    pub CopyProtectionType: u32,
    pub APSTriggerBits: u32,
    pub CopyProtectionSupport: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_DISPLAYDETECTCONTROL {
//     pub :24: u32,
// //     pub 4: u32,
    // Nested struct/union omitted for brevity
// //     pub 3: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_QUERYCONNECTIONCHANGE {
    pub driver: u32,
    // Nested struct/union omitted for brevity
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CREATEPROTECTEDSESSION {
    pub to: *mut c_void,
    pub pPrivateDriverData: *mut c_void,
    pub PrivateDriverDataSize: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_BEGINEXCLUSIVEACCESS {
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_ENDEXCLUSIVEACCESS {
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_RESUMEHWENGINE {
    pub ordinal: u32,
    pub ordinal: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETTRACKEDWORKLOADPOWERLEVEL {
    pub level: u32,
    pub level: u32,
    pub DXGK_TRACKEDWORKLOAD_STATE_FLAGS: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SAVEMEMORYFORHOTUPDATE {
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_RESTOREMEMORYFORHOTUPDATE {
    pub Flags: u32,
    pub pDataMdl: u32,
    pub MetaDataSize: u32,
    pub pMetaData: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CREATEDOORBELL {
    pub created: *mut c_void,
    pub handle: *mut c_void,
    pub data: u32,
    pub data: *mut c_void,
    pub allocation: *mut c_void,
    pub allocation: *mut c_void,
    pub flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CONNECTDOORBELL {
    pub connected: *mut c_void,
    pub flags: u32,
    pub doorbell: *mut c_void,
    pub doorbell: *mut c_void,
//     pub out:: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_DISCONNECTDOORBELL {
    pub disconnected: *mut c_void,
    pub flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_DESTROYDOORBELL {
    pub destroyed: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_NOTIFYWORKSUBMISSION {
    pub HWQueue: *mut c_void,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_DIRTY_BIT_TRACKING_SEGMENT_CAPS {
    pub PageSize: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_CREATEMEMORYBASIS {
    pub ID: u32,
    pub includes: u64,
    pub basis: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_QUERYDIRTYBITDATA {
    pub of: *mut c_void,
    pub from: u64,
    pub size: u64,
    pub be: u64,
    pub data: *mut c_void,
    pub bytes: usize,
    pub DXGKARG_QUERYDIRTYBITDATAFLAGS: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYSCATTERRESERVEIN {
    pub SegmentId: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYSCATTERRESERVEOUT {
    pub SetVGPUResourcesPageSize: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_GPUP_PREPARE_LIVE_MIGRATION {
    pub vfIndex: u32,
    pub MigrationType: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_GPUP_SAVE_IMMUTABLE_MIGRATION_DATA {
    pub Index: u32,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_GPUP_SAVE_MUTABLE_MIGRATION_DATA {
    pub Index: u32,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_GPUP_RESTORE_IMMUTABLE_MIGRATION_DATA {
    pub Functions: u32,
    pub buffer: u64,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_GPUP_RESTORE_MUTABLE_MIGRATION_DATA {
    pub Function: u32,
    pub buffer: u64,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_INTERRUPT_TABLE_ENTRY {
    pub MessageAddress: u64,
    pub MessageData: u32,
    pub VectorControl: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_GPUP_WRITE_VIRTUALIZED_MSIX {
    pub Function: u32,
    pub Table: u32,
    pub Entry: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_GPU_PHYSICAL_RESERVE_DESCRIPTOR {
    pub DriverAllocationHandle: *mut c_void,
    pub MemoryBasis: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETVIRTUALGPURESOURCES2 {
    pub Function: u32,
    pub descriptors: u32,
    pub descriptors: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETVIRTUALFUNCTIONPAUSESTATE {
    pub Function: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_QUERYFEATURESUPPORT {
    pub queried: u32,
    pub driver: u32,
    pub driver: u32,
    pub feature: u8,
    pub feature: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_QUERYFEATUREINTERFACE {
    pub query: u32,
    pub query: u32,
    pub the: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_FEATURE_SAMPLE_ADDVALUE {
    pub InputValue: u32,
    pub OutputValue: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_FEATURE_SAMPLE_SUBTRACTVALUE {
    pub InputValue: u32,
    pub OutputValue: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_TDR_PAYLOAD_ENGINE_TIMEOUT {
    pub reset: u32,
    pub reset: u32,
    pub GPU: u64,
    pub GPU: u64,
    pub TDR: u32,
    pub TDR: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_TDR_PAYLOAD_VSYNC_TIMEOUT {
    pub out: u32,
    pub out: u32,
    pub out: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_COLLECTDBGINFO2 {
    pub report: u32,
    pub info: *mut c_void,
    pub bytes: usize,
    pub extension: *mut c_void,
    pub TDR: u32,
    // Nested struct/union omitted for brevity
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_BUILDTESTCOMMANDBUFFER {
    pub queue: *mut c_void,
//     pub : [u32; 1],
//     pub : [*mut c_void; 1],
//     pub : [*mut c_void; 1],
//     pub : [u32; 1],
//     pub : [u32; 1],
//     pub : [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYPAGINGBUFFERINFOIN {
    pub PhysicalAdapterIndex: u32,
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYPAGINGBUFFERINFOOUT {
    pub PagingBufferSize: u32,
    pub PagingBufferPrivateDataSize: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYSEGMENTCOUNTIN {
    pub PhysicalAdapterIndex: u32,
    pub Padding: u32,
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYSEGMENTCOUNTOUT {
    pub SegmentCount: u32,
    pub Padding: u32,
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYSEGMENTIN5 {
    pub PhysicalAdapterIndex: u32,
    pub Padding: u32,
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYSEGMENTOUT5 {
    pub SegmentDescriptors: *mut c_void,
    pub Reserved: [u32; 4],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYMMUCOUNTIN {
    pub PhysicalAdapterIndex: u32,
    pub Padding: u32,
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYMMUCOUNTOUT {
    pub MmuCount: u32,
    pub Padding: u32,
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_MMUDESCRIPTOR {
    pub Flags: u32,
    pub Size: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYMMUSIN {
    pub PhysicalAdapterIndex: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_QUERYMMUSOUT {
    pub MmuDescriptors: *mut c_void,
    pub DisplayMmuId: u32,
    pub Reserved0: u32,
    pub Reserved: [u32; 4],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_NOTIFYCONTEXTPRIORITYCHANGE {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_RESETDISPLAYENGINE {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_TARGETMODE_DETAIL_TIMING {
    pub VideoStandard: u32,
    pub TimingId: u32,
    pub DetailTiming: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_SETPALETTE {
    pub VidPnSourceId: u32,
    pub FirstEntry: u32,
    pub NumEntries: u32,
    pub pLookupTable: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MOVE_RECT {
    pub SourcePoint: u32,
    pub DestRect: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BACKLIGHT_INFO {
    pub BacklightUsersetting: u32,
    pub BacklightEffective: u32,
    pub GammaRamp: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BRIGHTNESS_SENSOR_DATA_CHROMATICITY {
    pub ChromaticityX: u32,
    pub ChromaticityY: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_BRIGHTNESS_NIT_RANGE {
    pub MinimumLevelMillinit: u32,
    pub MaximumLevelMillinit: u32,
    pub StepSizeMillinit: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_NODEMETADATA {
    pub EngineType: u32,
    pub FriendlyName: [u32; 1],
    pub Flags: u32,
    pub Reserved: u32,
    pub GpuMmuSupported: u8,
    pub IoMmuSupported: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_NODE_PERFDATA {
    pub hertz: u64,
    pub frequency: u64,
    pub frequency: u64,
    pub mV: u32,
    pub nanoseconds: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_ADAPTER_PERFDATA {
    pub hertz: u64,
    pub frequency: u64,
    pub bytes: u64,
    pub bytes: u64,
    pub rpm: u32,
    pub percentage: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_ADAPTER_PERFDATACAPS {
    pub second: u64,
    pub second: u64,
    pub rpm: u32,
    pub levels: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_GPUVERSION {
    pub version: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_GPUCLOCKDATA {
    pub GpuFrequency: u64,
    pub GpuClockCounter: u64,
    pub CpuClockCounter: u64,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_TRACKEDWORKLOAD_SUPPORT {
    pub PhysicalAdapterIndex: u32,
    pub EngineType: u32,
    pub Support: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_NODEMETADATA {
    pub ordinal: u32,
    pub NodeData: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYCLOCKCALIBRATION {
    pub r#for: u32,
    pub chain: u32,
    pub engine: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATEDEVICEFLAGS {
// //     pub 0x00000001: u32,
// //     pub 0x00000002: u32,
// //     pub 0x00000004: u32,
// //     pub 0xFFFFFFF0: u32,
// //     pub 0xFFFFFFF8: u32,
// //     pub 0xFFFFFFFC: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROYDEVICE {
    pub device: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATECONTEXT {
    pub data: u32,
    pub data: u32,
    pub this: u32,
    pub _ADVSCH_: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROYCONTEXT {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATESYNCHRONIZATIONOBJECT {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATESYNCHRONIZATIONOBJECT2 {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATENATIVEFENCE {
    pub Flags: u32,
    pub Reserved: [u8; 28],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETNATIVEFENCELOGDETAIL {
    pub requested: u32,
//     pub r#in:: u32,
//     pub out:: u32,
//     pub out:: u32,
//     pub out:: u32,
//     pub out:: u32,
    pub Reserved: [u8; 64],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROYSYNCHRONIZATIONOBJECT {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENSYNCHRONIZATIONOBJECT {
    pub Reserved: [u32; 8],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_WAITFORSYNCHRONIZATIONOBJECT {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Fence {
    pub context: u32,
    pub to: u32,
    // Nested struct/union omitted for brevity
    // Nested struct/union omitted for brevity
    pub signaled: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SIGNALSYNCHRONIZATIONOBJECT {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_LOCK {
    pub device: u32,
    pub lock: u32,
    pub AcquireAperture: u32,
    pub NumPages: u32,
    pub memory: u32,
    pub D3DDDI_LOCKFLAGS: u32,
    pub _ADVSCH_: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_UNLOCK {
    pub device: u32,
    pub array: u32,
    pub unlock: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DISPLAYMODE {
    pub Width: u32,
    pub Height: u32,
    pub Format: u32,
    pub IntegerRefreshRate: u32,
    pub RefreshRate: u32,
    pub ScanLineOrdering: u32,
    pub DisplayOrientation: u32,
    pub DisplayFixedOutput: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETDISPLAYMODELIST {
    pub handle: u32,
    pub ID: u32,
//     pub out:: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DISPLAYMODELIST {
    pub VidPnSourceId: u32,
    pub ModeCount: u32,
    pub pModeList: [u32; 0],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETDISPLAYMODE_FLAGS {
// //     pub 1: u8,
// //     pub 31: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETDISPLAYMODE {
    pub device: u32,
//     pub r#in:: u32,
//     pub r#in:: u32,
//     pub r#in:: u32,
    pub STATUS_GRAPHICS_INCOMPATIBLE_PRIVATE_FORMAT: u32,
//     pub r#in:: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MULTISAMPLEMETHOD {
    pub NumSamples: u32,
    pub NumQualityLevels: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETMULTISAMPLEMETHODLIST {
    pub handle: u32,
    pub ID: u32,
//     pub r#in:: u32,
//     pub r#in:: u32,
//     pub r#in:: u32,
//     pub out:: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DIRTYREGIONS {
    pub NumRects: u32,
    pub Rects: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_COMPOSITION_PRESENTHISTORYTOKEN {
    pub hPrivateData: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_AUXILIARYPRESENTINFO {
    pub size: u32,
    pub r#type: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_FLIPMANAGER_AUXILIARYPRESENTINFO {
    pub auxiliaryPresentInfo: u32,
    pub flipManagerTracingId: u32,
    pub customDurationChanged: i32,
    pub pFlipManagerProcessedEvent: *mut c_void,
    pub FlipAdapterLuid: u32,
    pub VidPnSourceId: u32,
    pub independentFlipStage: u32,
    pub FlipCompletedQpc: u32,
    pub HwPresentDurationQpc: u32,
    pub WasCanceled: i32,
    pub ConvertedToNonIFlip: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GDIMODEL_PRESENTHISTORYTOKEN {
    pub hLogicalSurface: u32,
    pub hPhysicalSurface: u32,
    pub ScrollRect: u32,
    pub ScrollOffset: u32,
    pub DirtyRegions: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GDIMODEL_SYSMEM_PRESENTHISTORYTOKEN {
    pub hlsurf: u32,
    pub dwDirtyFlags: u32,
    pub uiCookie: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_FENCE_PRESENTHISTORYTOKEN {
    pub Key: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_BLTMODEL_PRESENTHISTORYTOKEN {
    pub hLogicalSurface: u32,
    pub hPhysicalSurface: u32,
    pub EventId: u32,
    pub DirtyRegions: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SCATTERBLT {
    pub hLogicalSurfaceDestination: u32,
    pub hDestinationCompSurfDWM: u32,
    pub DestinationCompositionBindingId: u32,
    pub SourceRect: u32,
    pub DestinationOffset: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SCATTERBLTS {
    pub NumBlts: u32,
    pub Blts: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SURFACECOMPLETE_PRESENTHISTORYTOKEN {
    pub hLogicalSurface: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Token {
    pub Model: u32,
    pub TokenSize: u32,
    pub CompositionBindingId: u64,
    // Nested struct/union omitted for brevity
    pub MaxSize: [u8; 1064],
    pub Flip: u32,
    pub Blt: u32,
    pub VistaBlt: u32,
    pub Gdi: u32,
    pub Fence: u32,
    pub GdiSysMem: u32,
    pub Composition: u32,
    pub FlipManager: u32,
    pub SurfaceComplete: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PRESENT_RGNS {
    pub DirtyRectCount: u32,
    pub MoveRectCount: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PRESENT_REDIRECTED {
    pub on: u32,
    pub present: u32,
    pub on: u32,
    pub PresentHistoryToken: u32,
    pub Flags: u32,
    pub from: u32,
//     pub r#in:: u32,
    pub DdiSetVidPnSourceAddress: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ReprogramInterrupt {
    // Nested struct/union omitted for brevity
    // Nested struct/union omitted for brevity
// //     pub 1: u32,
// //     pub 31: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CANCEL_PRESENTS {
    pub cbSize: u32,
    pub hDevice: u32,
    pub Flags: u32,
    pub Operation: u32,
    pub CancelFromPresentId: u32,
    pub CompSurfaceLuid: u32,
    pub BindId: u32,
    pub hFlipManagerProcessedEvent: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATE_DOORBELL {
    pub required: u32,
    pub handle: u32,
    pub handle: u32,
    pub flags: u32,
    pub data: u32,
    pub data: u32,
    pub doorbell: u32,
    pub doorbell: u32,
    pub doorbell: u32,
    pub write: u32,
    pub object: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CONNECT_DOORBELL {
    pub connected: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROY_DOORBELL {
    pub destroyed: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_NOTIFY_WORK_SUBMISSION {
    pub doorbell: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ISFEATUREENABLED {
    pub hAdapter: u32,
    pub FeatureId: u32,
    pub Result: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SUBMITPRESENTBLTTOHWQUEUE {
    pub hHwQueue: u32,
    pub HwQueueProgressFenceId: u32,
    pub PrivatePresentData: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SUBMITPRESENTTOHWQUEUE {
    pub PrivatePresentData: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CHECKMULTIPLANEOVERLAYSUPPORT {
    pub device: u32,
    pub pin: u32,
    pub pin: u32,
    pub Supported: i32,
    pub ReturnInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MULTIPLANE_OVERLAY_ATTRIBUTES2 {
    pub D3DKMT_MULTIPLANE_OVERLAY_FLAGS: u32,
    pub DirtyRectCount: u32,
    pub DXGK_MULTIPLANE_OVERLAY_VIDEO_FRAME_FORMAT: u32,
    pub ColorSpace: u32,
    pub DXGK_MULTIPLANE_OVERLAY_STEREO_FORMAT: u32,
    pub DXGK_MULTIPLANE_OVERLAY_STEREO_FLIP_MODE: u32,
    pub DXGK_MULTIPLANE_OVERLAY_STRETCH_QUALITY: u32,
    pub Reserved1: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CHECK_MULTIPLANE_OVERLAY_PLANE2 {
    pub LayerIndex: u32,
    pub hResource: u32,
    pub CompSurfaceLuid: u32,
    pub VidPnSourceId: u32,
    pub PlaneAttributes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CHECKMULTIPLANEOVERLAYSUPPORT2 {
    pub handle: u32,
    pub device: u32,
    pub pin: u32,
    pub pin: u32,
    pub Supported: i32,
    pub ReturnInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MULTIPLANE_OVERLAY2 {
    pub LayerIndex: u32,
    pub Enabled: i32,
    pub hAllocation: u32,
    pub PlaneAttributes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MULTIPLANE_OVERLAY_ATTRIBUTES3 {
    pub D3DKMT_MULTIPLANE_OVERLAY_FLAGS: u32,
    pub DirtyRectCount: u32,
    pub ColorSpace: u32,
    pub DXGK_MULTIPLANE_OVERLAY_STRETCH_QUALITY: u32,
    pub SDRWhiteLevel: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CHECK_MULTIPLANE_OVERLAY_PLANE3 {
    pub LayerIndex: u32,
    pub hResource: u32,
    pub CompSurfaceLuid: u32,
    pub VidPnSourceId: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MULTIPLANE_OVERLAY_POST_COMPOSITION {
    pub Flags: u32,
    pub SrcRect: u32,
    pub DstRect: u32,
    pub Rotation: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MULTIPLANE_OVERLAY_POST_COMPOSITION_WITH_SOURCE {
    pub VidPnSourceId: u32,
    pub PostComposition: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CHECKMULTIPLANEOVERLAYSUPPORT3 {
    pub handle: u32,
    pub device: u32,
    pub pin: u32,
    pub planes: *mut c_void,
    pub pin: u32,
    pub planes: *mut c_void,
    pub Supported: i32,
    pub ReturnInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MULTIPLANE_OVERLAY3 {
    pub LayerIndex: u32,
    pub InputFlags: u32,
    pub FlipInterval: u32,
    pub MaxImmediateFlipLine: u32,
    pub AllocationCount: u32,
    pub DriverPrivateDataSize: u32,
    pub hFlipToFence: u32,
    pub hFlipAwayFence: u32,
    pub FlipToFenceValue: u32,
    pub FlipAwayFenceValue: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PRESENT_MULTIPLANE_OVERLAY3 {
    pub handle: u32,
    pub ContextCount: u32,
    pub flagged: u32,
    pub counter: u32,
//     pub r#in:: u32,
    pub PresentPlaneCount: u32,
    pub ppPresentPlanes: *mut c_void,
    pub Duration: u32,
    pub HDRMetaDataType: u32,
    pub HDRMetaDataSize: u32,
    pub BoostRefreshRateMultiplier: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GET_MULTIPLANE_OVERLAY_CAPS {
    pub handle: u32,
    pub r#in: u32,
    pub supported: u32,
    pub supported: u32,
    pub supported: u32,
    pub capabilities: u32,
    pub out: u32,
    pub out: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GET_POST_COMPOSITION_CAPS {
    pub handle: u32,
    pub r#in: u32,
    pub out: u32,
    pub out: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MULTIPLANEOVERLAY_STRETCH_SUPPORT {
    pub VidPnSourceId: u32,
    pub Update: i32,
    pub Supported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_RENDERFLAGS {
// //     pub 0x00000001: u32,
// //     pub 0x00000002: u32,
// //     pub 0x00000004: u32,
// //     pub 0x00000008: u32,
// //     pub 0x00000010: u32,
    pub DxgkRender: u32,
    pub DxgkRender: u32,
// //     pub 0xFFFFFF80: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OUTPUTDUPLPRESENT {
    pub context: u32,
    pub from: u32,
    pub VidPnSourceId: u32,
    pub context: u32,
    pub to: u32,
    pub regions: u32,
    pub Flags: u32,
    pub hIndirectContext: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OUTPUTDUPLPRESENTTOHWQUEUE {
    pub from: u32,
    pub VidPnSourceId: u32,
    pub BroadcastHwQueueCount: u32,
    pub regions: u32,
    pub Flags: u32,
    pub hIndirectHwQueue: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_STANDARDALLOCATION_EXISTINGHEAP {
    pub heap: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATEALLOCATIONFLAGS {
// //     pub 0x00000001: u32,
// //     pub 0x00000002: u32,
// //     pub 0x00000004: u32,
// //     pub 0x00000010: u32,
// //     pub 0x00000040: u32,
// //     pub 0x00000080: u32,
// //     pub 0x00000800: u32,
// //     pub 0x00002000: u32,
    pub pages: u32,
    pub allocation: u32,
    pub pPrivateDriverData: u32,
    pub D3DDI_ALLOCATIONINFO2: u32,
    pub required: u32,
    pub contguous: u32,
    pub allocation: u32,
// //     pub 0x00200000: u32,
    pub allocation: u32,
// //     pub 0xFF800000: u32,
// //     pub 0xFFC00000: u32,
// //     pub 0xFFE00000: u32,
// //     pub 0xFFF80000: u32,
// //     pub 0xFFFC0000: u32,
// //     pub 0xFFFF0000: u32,
// //     pub 0xFFFFF800: u32,
// //     pub 0xFFFFFFC0: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENRESOURCEFROMNTHANDLE {
    pub device: u32,
    pub handle: u32,
    pub resource: u32,
    // Nested struct/union omitted for brevity
    pub buffer: u32,
    pub copied: u32,
    pub buffer: u32,
    pub copied: u32,
    pub pTotalPrivateDriverDataBuffer: u32,
    pub stored: u32,
    pub process: u32,
    pub process: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENSYNCOBJECTFROMNTHANDLE {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MonitoredFence {
    // Nested struct/union omitted for brevity
    // Nested struct/union omitted for brevity
    pub CPU: u32,
    pub GPU: u32,
    pub mapped: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENNATIVEFENCEFROMNTHANDLE {
    pub mapped: u32,
    pub object: u32,
    pub object: u32,
    pub UMD: u8,
    pub Reserved: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENSYNCOBJECTNTHANDLEFROMNAME {
    pub dwDesiredAccess: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENNTHANDLEFROMNAME {
    pub dwDesiredAccess: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYRESOURCEINFOFROMNTHANDLE {
    pub device: u32,
    pub open: u32,
    pub resource: u32,
    pub buffer: u32,
    pub resource: u32,
    pub data: u32,
    pub resource: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYRESOURCEINFO {
    pub device: u32,
    pub open: u32,
    pub resource: u32,
    pub buffer: u32,
    pub resource: u32,
    pub data: u32,
    pub resource: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROYALLOCATION {
    pub device: u32,
    pub hResource: u32,
    pub destroy: u32,
    pub phAllocationList: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROYALLOCATION2 {
    pub device: u32,
    pub hResource: u32,
    pub destroy: u32,
    pub phAllocationList: u32,
    pub D3DDDICB_DESTROYALLOCATION2FLAGS: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETALLOCATIONPRIORITY {
    pub device: u32,
    pub destroy: u32,
    pub phAllocationList: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYALLOCATIONRESIDENCY {
    pub device: u32,
    pub phAllocationList: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETRUNTIMEDATA {
    pub hAdapter: u32,
    pub handle: u32,
//     pub r#in:: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_UMDFILENAMEINFO {
    pub version: u32,
    pub name: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENGLINFO {
    pub UmdOpenGlIcdFileName: [u32; 1],
    pub Version: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SEGMENTSIZEINFO {
    pub DedicatedVideoMemorySize: u32,
    pub DedicatedSystemMemorySize: u32,
    pub SharedSystemMemorySize: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SEGMENTGROUPSIZEINFO {
    pub PhysicalAdapterIndex: u32,
    pub LegacyInfo: u32,
    pub LocalMemory: u32,
    pub NonLocalMemory: u32,
    pub NonBudgetMemory: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_WORKINGSETFLAGS {
// //     pub 0x00000001: u32,
// //     pub 0xFFFFFFFE: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_WORKINGSETINFO {
    pub Flags: u32,
    pub MinimumWorkingSetPercentile: u32,
    pub MaximumWorkingSetPercentile: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_FLIPINFOFLAGS {
    pub natively: u32,
// //     pub 0xFFFFFFFE: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_FLIPQUEUEINFO {
    pub FlipFlags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ADAPTERADDRESS {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ADAPTERREGISTRYINFO {
    pub AdapterString: [u32; 1],
    pub BiosString: [u32; 1],
    pub DacType: [u32; 1],
    pub ChipType: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CURRENTDISPLAYMODE {
    pub VidPnSourceId: u32,
    pub DisplayMode: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OUTPUTDUPLCONTEXTSCOUNT {
    pub VidPnSourceId: u32,
    pub OutputDuplicationCount: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_UMD_DRIVER_VERSION {
    pub DriverVersion: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_KMD_DRIVER_VERSION {
    pub DriverVersion: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DIRECTFLIP_SUPPORT {
    pub Supported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MULTIPLANEOVERLAY_SUPPORT {
    pub Supported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MULTIPLANEOVERLAY_HUD_SUPPORT {
    pub Update: i32,
    pub KernelSupported: i32,
    pub HudSupported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DLIST_DRIVER_NAME {
    pub name: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CPDRIVERNAME {
    pub ContentProtectionFileName: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MIRACASTCOMPANIONDRIVERNAME {
    pub MiracastCompanionDriverName: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_XBOX {
    pub IsXBOX: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_INDEPENDENTFLIP_SUPPORT {
    pub Supported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MULTIPLANEOVERLAY_DECODE_SUPPORT {
    pub Supported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ISBADDRIVERFORHWPROTECTIONDISABLED {
    pub Disabled: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MULTIPLANEOVERLAY_SECONDARY_SUPPORT {
    pub Supported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_INDEPENDENTFLIP_SECONDARY_SUPPORT {
    pub Supported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PANELFITTER_SUPPORT {
    pub Supported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PHYSICAL_ADAPTER_COUNT {
    pub Count: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DEVICE_IDS {
    pub VendorID: u32,
    pub DeviceID: u32,
    pub SubVendorID: u32,
    pub SubSystemID: u32,
    pub RevisionID: u32,
    pub BusType: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERY_DEVICE_IDS {
//     pub r#in:: u32,
//     pub out:: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERY_PHYSICAL_ADAPTER_PNP_KEY {
    pub PhysicalAdapterIndex: u32,
    pub PnPKeyType: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERY_MIRACAST_DRIVER_TYPE {
    pub MiracastDriverType: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERY_GPUMMU_CAPS {
//     pub r#in:: u32,
//     pub out:: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MPO3DDI_SUPPORT {
    pub Supported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_HWDRM_SUPPORT {
    pub Supported: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MPOKERNELCAPS_SUPPORT {
    pub Supported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GET_DEVICE_VIDPN_OWNERSHIP_INFO {
    pub device: u32,
    pub ownership: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_BLOCKLIST_INFO {
    pub Size: u32,
    pub BlockList: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERY_ADAPTER_UNIQUE_GUID {
    pub AdapterUniqueGUID: [u32; 40],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_NODE_PERFDATA {
    pub chain: u32,
    pub hertz: u32,
    pub frequency: u32,
    pub frequency: u32,
    pub mV: u32,
    pub nanoseconds: u32,
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ADAPTER_PERFDATA {
    pub chain: u32,
    pub hertz: u32,
    pub frequency: u32,
    pub bytes: u32,
    pub bytes: u32,
    pub rpm: u32,
    pub percentage: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ADAPTER_PERFDATACAPS {
    pub chain: u32,
    pub second: u32,
    pub second: u32,
    pub rpm: u32,
    pub levels: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GPUVERSION {
    pub chain: u32,
    pub version: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DRIVER_DESCRIPTION {
    pub description: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERY_SCANOUT_CAPS {
    pub VidPnSourceId: u32,
    pub Caps: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DISPLAY_UMD_FILENAMEINFO {
    pub version: u32,
    pub name: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_WSAUMDIMAGENAME {
    pub name: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_VGPUINTERFACEID {
    pub name: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PARAVIRTUALIZATION {
    pub SecureContainer: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_HYBRID_DLIST_DLL_SUPPORT {
    pub Supported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_HYBRID_DLIST_DLL_MUX_SUPPORT {
    pub Supported: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CROSSADAPTERRESOURCE_SUPPORT {
    pub SupportTier: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYADAPTERINFO {
    pub hAdapter: u32,
    pub Type: u32,
    pub PrivateDriverDataSize: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENADAPTERFROMHDC {
    pub display: u32,
    pub handle: u32,
    pub LUID: u32,
    pub display: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENADAPTERFROMGDIDISPLAYNAME {
    pub instance: u32,
    pub handle: u32,
    pub LUID: u32,
    pub display: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENADAPTERFROMDEVICENAME {
    pub open: u32,
    pub handle: u32,
    pub LUID: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ADAPTERINFO {
    pub hAdapter: u32,
    pub AdapterLuid: u32,
    pub NumOfSources: u32,
    pub bPrecisePresentRegionsPreferred: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ENUMADAPTERS {
    pub NumAdapters: u32,
    pub Adapters: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ENUMADAPTERS2 {
    pub elements: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENADAPTERFROMLUID {
    pub AdapterLuid: u32,
    pub hAdapter: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYREMOTEVIDPNSOURCEFROMGDIDISPLAYNAME {
    pub instance: u32,
    pub display: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ENUMADAPTERS3 {
    pub filter: u32,
    pub elements: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CLOSEADAPTER {
    pub handle: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETSHAREDPRIMARYHANDLE {
    pub handle: u32,
    pub ID: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SHAREDPRIMARYLOCKNOTIFICATION {
    pub AdapterLuid: u32,
    pub VidPnSourceId: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SHAREDPRIMARYUNLOCKNOTIFICATION {
    pub AdapterLuid: u32,
    pub VidPnSourceId: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PINDIRECTFLIPRESOURCES {
    pub device: u32,
    pub pin: u32,
    pub pin: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_UNPINDIRECTFLIPRESOURCES {
    pub device: u32,
    pub unpin: u32,
    pub unpin: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DOD_SET_DIRTYRECT_MODE {
    pub present: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_HISTORY_BUFFER_STATUS {
    pub Enabled: u8,
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_VAD_DESC {
    pub address: u32,
    pub r#in: u32,
    pub out: u32,
    pub Mapped: u32,
    pub out: u32,
    pub out: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_VA_RANGE_DESC {
    pub r#in: u32,
    pub r#in: u32,
    pub r#in: u32,
    pub out: u32,
    pub out: u32,
    pub out: u32,
    pub VIDMM_VAD_OWNER_TYPE: u32,
    pub out: u32,
    pub out: u32,
    pub D3DDDIGPUVIRTUALADDRESS_PROTECTION_TYPE: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PAGE_TABLE_LEVEL_DESC {
    pub IndexBitCount: u32,
    pub IndexMask: u32,
    pub IndexShift: u32,
    pub LowerLevelsMask: u32,
    pub EntryCoverageInPages: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_ESCAPE_GPUMMUCAPS {
    pub ReadOnlyMemorySupported: u8,
    pub NoExecuteMemorySupported: u8,
    pub ZeroInPteSupported: u8,
    pub CacheCoherentMemorySupported: u8,
    pub LargePageSupported: u8,
    pub DualPteSupported: u8,
    pub AllowNonAlignedLargePageAddress: u8,
// //     pub 7: u8,
    pub VirtualAddressBitCount: u32,
    pub PageTableLevelCount: u32,
    pub PageTableLevelDesk: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GET_GPUMMU_CAPS {
    pub In: u32,
    pub Out: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GET_PTE {
    pub In: u32,
    pub In: u32,
    pub In: u32,
    pub PTEs: u32,
    pub Out: u32,
    pub Out: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GET_PTE_EXT {
    pub Out: u64,
    pub Out: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SEGMENT_CAPS {
    pub Size: u32,
    pub PageSize: u32,
    pub SegmentId: u32,
    pub bAperture: u8,
    pub bReservedSysMem: u8,
    pub BudgetGroup: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GET_SEGMENT_CAPS {
    pub In: u32,
    pub Out: u32,
    pub Out: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ESCAPE_VIRTUAL_REFRESH_RATE {
    pub Type: u32,
    pub VidPnSourceId: u32,
    pub ProcessBoostEligible: u8,
    pub VSyncMultiplier: u32,
    pub BaseDesktopDuration: u32,
    pub Reserved: [u8; 16],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DMM_ESCAPE {
    pub Type: u32,
    pub Data: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_BRIGHTNESS_POSSIBLE_LEVELS {
    pub LevelCount: u8,
    pub BrightnessLevels: [u8; 256],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_BDDFALLBACK_CTL {
    pub ForceBddHeadlessNextFallback: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_REQUEST_MACHINE_CRASH_ESCAPE {
    pub Param1: u32,
    pub Param2: u32,
    pub Param3: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PROCESS_VERIFIER_VIDMM_RESTRICT_BUDGET {
    pub LocalBudget: u32,
    pub NonLocalBudget: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PROCESS_VERIFIER_OPTION {
    pub Type: u32,
    pub Mode: u32,
    pub Data: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ADAPTER_VERIFIER_VIDMM_TRIM_INTERVAL {
    pub MinimumTrimInterval: u32,
    pub MaximumTrimInterval: u32,
    pub IdleTrimInterval: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ADAPTER_VERIFIER_OPTION {
    pub Type: u32,
    pub Mode: u32,
    pub Data: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct VidPnFromAllocation {
    pub Type: u32,
    // Nested struct/union omitted for brevity
    // Nested struct/union omitted for brevity
    pub handle: u32,
    pub allocation: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DEBUG_SNAPSHOT_ESCAPE {
    pub Buffer: u32,
    pub snapshot: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OUTPUTDUPL_SNAPSHOT {
    // Nested struct/union omitted for brevity
    pub adapter: u32,
    pub index: u32,
    pub index: u32,
    pub Padding: u32,
    pub OutputDuplDebugInfos: [u32; 0],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ACTIVATE_SPECIFIC_DIAG_ESCAPE {
    pub deactivate: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ESCAPE {
    pub handle: u32,
//     pub : [u32; 1],
    pub flags: u32,
    pub data: u32,
    pub data: u32,
//     pub : [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_COUNTER {
    pub Count: u32,
    pub Bytes: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_DMA_PACKET_TYPE_INFORMATION {
    pub PacketSubmited: u32,
    pub PacketCompleted: u32,
    pub PacketPreempted: u32,
    pub PacketFaulted: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_QUEUE_PACKET_TYPE_INFORMATION {
    pub PacketSubmited: u32,
    pub PacketCompleted: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_PACKET_INFORMATION {
    pub D3DKMT_QueuePacketTypeMax: u32,
    pub D3DKMT_DmaPacketTypeMax: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_PREEMPTION_INFORMATION {
    pub PreemptionCounter: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_PROCESS_NODE_INFORMATION {
    pub ContextSwitch: u32,
    pub PreemptionStatistics: u32,
    pub PacketStatistics: u32,
    pub Reserved: [u32; 8],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_NODE_INFORMATION {
    pub statistics: u32,
    pub thread: u32,
    pub NodePerfData: u32,
    pub Reserved: [u32; 3],
    pub Reserved: [u32; 8],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_PROCESS_VIDPNSOURCE_INFORMATION {
    pub Padding: u32,
    pub IsVSyncEnabled: u32,
    pub VSyncOnTotalTimeMs: u32,
    pub VSyncOffKeepPhaseTotalTimeMs: u32,
    pub VSyncOffNoPhaseTotalTimeMs: u32,
    pub Reserved: [u32; 4],
    pub Reserved: [u32; 8],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_VIDPNSOURCE_INFORMATION {
    pub statistics: u32,
    pub thread: u32,
    pub Reserved: [u32; 8],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATSTICS_REFERENCE_DMA_BUFFER {
    pub NbCall: u32,
    pub NbAllocationsReferenced: u32,
    pub MaxNbAllocationsReferenced: u32,
    pub NbNULLReference: u32,
    pub NbWriteReference: u32,
    pub NbRenamedAllocationsReferenced: u32,
    pub NbIterationSearchingRenamedAllocation: u32,
    pub NbLockedAllocationReferenced: u32,
    pub NbAllocationWithValidPrepatchingInfoReferenced: u32,
    pub NbAllocationWithInvalidPrepatchingInfoReferenced: u32,
    pub NbDMABufferSuccessfullyPrePatched: u32,
    pub NbPrimariesReferencesOverflow: u32,
    pub NbAllocationWithNonPreferredResources: u32,
    pub NbAllocationInsertedInMigrationTable: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATSTICS_RENAMING {
    pub NbAllocationsRenamed: u32,
    pub NbAllocationsShrinked: u32,
    pub NbRenamedBuffer: u32,
    pub MaxRenamingListLength: u32,
    pub NbFailuresDueToRenamingLimit: u32,
    pub NbFailuresDueToCreateAllocation: u32,
    pub NbFailuresDueToOpenAllocation: u32,
    pub NbFailuresDueToLowResource: u32,
    pub NbFailuresDueToNonRetiredLimit: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATSTICS_PREPRATION {
    pub BroadcastStall: u32,
    pub NbDMAPrepared: u32,
    pub NbDMAPreparedLongPath: u32,
    pub ImmediateHighestPreparationPass: u32,
    pub AllocationsTrimmed: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATSTICS_PAGING_FAULT {
    pub Faults: u32,
    pub FaultsFirstTimeAccess: u32,
    pub FaultsReclaimed: u32,
    pub FaultsMigration: u32,
    pub FaultsIncorrectResource: u32,
    pub FaultsLostContent: u32,
    pub FaultsEvicted: u32,
    pub AllocationsMEM_RESET: u32,
    pub AllocationsUnresetSuccess: u32,
    pub AllocationsUnresetFail: u32,
    pub AllocationsUnresetSuccessRead: u32,
    pub AllocationsUnresetFailRead: u32,
    pub Evictions: u32,
    pub EvictionsDueToPreparation: u32,
    pub EvictionsDueToLock: u32,
    pub EvictionsDueToClose: u32,
    pub EvictionsDueToPurge: u32,
    pub EvictionsDueToSuspendCPUAccess: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATSTICS_PAGING_TRANSFER {
    pub BytesFilled: u32,
    pub BytesDiscarded: u32,
    pub BytesMappedIntoAperture: u32,
    pub BytesUnmappedFromAperture: u32,
    pub BytesTransferredFromMdlToMemory: u32,
    pub BytesTransferredFromMemoryToMdl: u32,
    pub BytesTransferredFromApertureToMemory: u32,
    pub BytesTransferredFromMemoryToAperture: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATSTICS_SWIZZLING_RANGE {
    pub NbRangesAcquired: u32,
    pub NbRangesReleased: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATSTICS_LOCKS {
    pub NbLocks: u32,
    pub NbLocksWaitFlag: u32,
    pub NbLocksDiscardFlag: u32,
    pub NbLocksNoOverwrite: u32,
    pub NbLocksNoReadSync: u32,
    pub NbLocksLinearization: u32,
    pub NbComplexLocks: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATSTICS_ALLOCATIONS {
    pub Created: u32,
    pub Destroyed: u32,
    pub Opened: u32,
    pub Closed: u32,
    pub MigratedSuccess: u32,
    pub MigratedFail: u32,
    pub MigratedAbandoned: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATSTICS_TERMINATIONS {
    pub TerminatedShared: u32,
    pub TerminatedNonShared: u32,
    pub DestroyedShared: u32,
    pub DestroyedNonShared: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_ADAPTER_INFORMATION {
    pub NbSegments: u32,
    pub NodeCount: u32,
    pub VidPnSourceCount: u32,
    pub VSyncEnabled: u32,
    pub TdrDetectedCount: u32,
    pub ZeroLengthDmaBuffers: u32,
    pub RestartedPeriod: u32,
    pub ReferenceDmaBuffer: u32,
    pub Renaming: u32,
    pub Preparation: u32,
    pub PagingFault: u32,
    pub PagingTransfer: u32,
    pub SwizzlingRange: u32,
    pub Locks: u32,
    pub Allocations: u32,
    pub Terminations: u32,
    pub Flags: u32,
    pub Reserved: [u32; 7],
    pub Reserved: [u32; 8],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_PHYSICAL_ADAPTER_INFORMATION {
    pub AdapterPerfData: u32,
    pub AdapterPerfDataCaps: u32,
    pub GpuVersion: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_SYSTEM_MEMORY {
    pub BytesAllocated: u32,
    pub BytesReserved: u32,
    pub SmallAllocationBlocks: u32,
    pub LargeAllocationBlocks: u32,
    pub WriteCombinedBytesAllocated: u32,
    pub WriteCombinedBytesReserved: u32,
    pub CachedBytesAllocated: u32,
    pub CachedBytesReserved: u32,
    pub SectionBytesAllocated: u32,
    pub SectionBytesReserved: u32,
    pub BytesZeroed: u32,
    pub Reserved: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_PROCESS_INFORMATION {
    pub NodeCount: u32,
    pub VidPnSourceCount: u32,
    pub SystemMemory: u32,
    pub Reserved: [u32; 7],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_DMA_BUFFER {
    pub Size: u32,
    pub AllocationListBytes: u32,
    pub PatchLocationListBytes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_COMMITMENT_DATA {
    pub TotalBytesEvictedFromProcess: u32,
    pub BytesBySegmentPreference: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_POLICY {
    pub PreferApertureForRead: [u32; 1],
    pub PreferAperture: [u32; 1],
    pub MemResetOnPaging: u32,
    pub RemovePagesFromWorkingSetOnPaging: u32,
    pub MigrationEnabled: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_PROCESS_INTERFERENCE_COUNTERS {
    pub InterferenceCount: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_PROCESS_ADAPTER_INFORMATION {
    pub NbSegments: u32,
    pub NodeCount: u32,
    pub VidPnSourceCount: u32,
    pub VirtualMemoryUsage: u32,
    pub DmaBuffer: u32,
    pub CommitmentData: u32,
    pub _Policy: u32,
    pub ProcessInterferenceCounters: u32,
    pub Reserved: [u32; 9],
    pub ClientHint: u32,
    pub Reserve: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_MEMORY {
    pub TotalBytesEvicted: u32,
    pub AllocsCommitted: u32,
    pub AllocsResident: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PowerFlags {
    pub CommitLimit: u32,
    pub BytesCommitted: u32,
    pub BytesResident: u32,
    pub CommitLimit: u32,
    pub BytesCommitted: u32,
    pub BytesResident: u32,
    pub Memory: u32,
    pub Aperture: u32,
    pub D3DKMT_MaxAllocationPriorityClass: u32,
    pub SystemMemoryEndAddress: u32,
    // Nested struct/union omitted for brevity
// //     pub 1: u64,
// //     pub 1: u64,
// //     pub 1: u64,
// //     pub 61: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_VIDEO_MEMORY {
    pub AllocsCommitted: u32,
    pub AllocsResidentInP: [u32; 1],
    pub AllocsResidentInNonPreferred: u32,
    pub TotalBytesEvictedDueToPreparation: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_PROCESS_SEGMENT_POLICY {
    pub UseMRU: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_PROCESS_SEGMENT_INFORMATION {
    pub BytesCommitted: u32,
    pub MaximumWorkingSet: u32,
    pub MinimumWorkingSet: u32,
    pub NbReferencedAllocationEvictedInPeriod: u32,
    pub Padding: u32,
    pub BytesCommitted: u32,
    pub NbReferencedAllocationEvictedInPeriod: u32,
    pub MaximumWorkingSet: u32,
    pub MinimumWorkingSet: u32,
    pub VideoMemory: u32,
    pub _Policy: u32,
    pub Reserved: [u32; 8],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_PROCESS_SEGMENT_GROUP_INFORMATION {
    pub Budget: u32,
    pub Requested: u32,
    pub Usage: u32,
    pub Demoted: [u32; 1],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_QUERY_SEGMENT {
    pub r#for: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_QUERY_NODE {
    pub NodeId: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_QUERY_VIDPNSOURCE {
    pub r#for: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_QUERY_PHYSICAL_ADAPTER {
    pub PhysicalAdapterIndex: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_MEMORY_USAGE {
    pub AllocatedBytes: u32,
    pub FreeBytes: u32,
    pub ZeroBytes: u32,
    pub ModifiedBytes: u32,
    pub StandbyBytes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_QUERY_SEGMENT2 {
    pub PhysicalAdapterIndex: u32,
    pub SegmentId: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_QUERY_SEGMENT_USAGE {
    pub PhysicalAdapterIndex: u32,
    pub SegmentId: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_QUERY_SEGMENT_GROUP_USAGE {
    pub PhysicalAdapterIndex: u32,
    pub D3DKMT_MEMORY_SEGMENT_GROUP: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_QUERY_ADAPTER2 {
    pub PhysicalAdapterIndex: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_QUERY_ADAPTER_INFORMATION2 {
    pub PhysicalAdapterIndex: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_QUERY_PROCESS_SEGMENT_GROUP2 {
    pub PhysicalAdapterIndex: u32,
    pub D3DKMT_MEMORY_SEGMENT_GROUP: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYSTATISTICS_QUERY_NODE2 {
    pub PhysicalAdapterIndex: u32,
    pub NodeOrdinal: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PRESENT_STATS_DWM2 {
    // Nested struct/union omitted for brevity
    pub PresentCount: u32,
    pub PresentRefreshCount: u32,
    pub PresentQPCTime: u32,
    pub SyncRefreshCount: u32,
    pub SyncQPCTime: u32,
    pub CustomPresentDuration: u32,
    pub VirtualSyncRefreshCount: u32,
    pub VirtualSyncQPCTime: u32,
    pub VSyncDurationQPCTime: u32,
    pub VSyncMultiplier: u32,
    pub VirtualPresentRefreshCount: u32,
    pub VirtualPresentQPCTime: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETVIDPNSOURCEOWNER {
    pub handle: u32,
    pub array: u32,
    pub array: u32,
    pub array: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETVIDPNSOURCEOWNER1 {
    pub Version0: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETVIDPNSOURCEOWNER2 {
    pub Version1: u32,
    pub handles: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CHECKVIDPNEXCLUSIVEOWNERSHIP {
    pub handle: u32,
    pub array: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETPRESENTHISTORY {
    pub adapter: u32,
    pub buffer: u32,
    pub token: u32,
    pub token: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATEOVERLAY {
    pub r#in: u32,
    pub device: u32,
    pub r#in: u32,
    pub handle: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_UPDATEOVERLAY {
    pub device: u32,
    pub handle: u32,
    pub r#in: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_FLIPOVERLAY {
    pub device: u32,
    pub handle: u32,
    pub displayed: u32,
    pub data: u32,
    pub data: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETOVERLAYSTATE {
    pub device: u32,
    pub handle: u32,
    pub OverlayEnabled: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROYOVERLAY {
    pub device: u32,
    pub handle: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_WAITFORVERTICALBLANKEVENT {
    pub handle: u32,
//     pub : [u32; 1],
    pub ID: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_WAITFORVERTICALBLANKEVENT2 {
    pub handle: u32,
//     pub : [u32; 1],
    pub ID: u32,
    pub NumObjects: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETVERTICALBLANKEVENT {
    pub handle: u32,
//     pub : [u32; 1],
    pub ID: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETSYNCREFRESHCOUNTWAITTARGET {
    pub handle: u32,
//     pub : [u32; 1],
    pub ID: u32,
    pub TargetSyncRefreshCount: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ADJUSTFULLSCREENGAMMA {
    pub handle: u32,
    pub ID: u32,
    pub Scale: u32,
    pub Offset: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETVIDPNSOURCEHWPROTECTION {
    pub handle: u32,
    pub ID: u32,
    pub status: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETHWPROTECTIONTEARDOWNRECOVERY {
    pub handle: u32,
    pub recovery: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PRESENT_STATS {
    pub PresentCount: u32,
    pub PresentRefreshCount: u32,
    pub SyncRefreshCount: u32,
    pub SyncQPCTime: u32,
    pub SyncGPUTime: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DEVICEPRESENT_STATE {
    pub id: u32,
    pub stats: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_PRESENT_STATS_DWM {
    pub PresentCount: u32,
    pub PresentRefreshCount: u32,
    pub PresentQPCTime: u32,
    pub SyncRefreshCount: u32,
    pub SyncQPCTime: u32,
    pub CustomPresentDuration: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DEVICEPAGEFAULT_STATE {
    pub fault: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DEVICEPRESENT_STATE_DWM {
    pub id: u32,
// //     pub 2: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DEVICEPRESENT_QUEUE_STATE {
    pub id: u32,
    pub reached: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATEDCFROMMEMORY {
    pub DC: u32,
    pub format: u32,
    pub Width: u32,
    pub Height: u32,
    pub pitch: u32,
    pub device: u32,
    pub Palette: u32,
    pub HDC: u32,
    pub bitmap: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROYDCFROMMEMORY {
//     pub r#in:: u32,
//     pub r#in:: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETCONTEXTSCHEDULINGPRIORITY {
    pub handle: u32,
    pub priority: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETCONTEXTINPROCESSSCHEDULINGPRIORITY {
    pub handle: u32,
    pub priority: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CHANGESURFACEPOINTER {
    pub handle: u32,
    pub handle: u32,
    pub pointer: u32,
    pub Width: u32,
    pub Height: u32,
    pub pitch: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETCONTEXTSCHEDULINGPRIORITY {
    pub handle: u32,
    pub priority: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETCONTEXTINPROCESSSCHEDULINGPRIORITY {
    pub handle: u32,
    pub priority: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETSCANLINE {
    pub handle: u32,
    pub ID: u32,
    pub blank: u8,
    pub line: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_POLLDISPLAYCHILDREN {
    pub handle: u32,
    // Nested struct/union omitted for brevity
    pub not: u32,
    pub event: u32,
    pub adapters: u32,
// //     pub 0xffffffc0: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_INVALIDATEACTIVEVIDPN {
    pub handle: u32,
    pub data: u32,
    pub data: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CHECKOCCLUSION {
    pub handle: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_WAITFORIDLE {
    pub idle: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CHECKMONITORPOWERSTATE {
    pub on: u32,
    pub ID: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETDISPLAYPRIVATEDRIVERFORMAT {
    pub device: u32,
    pub r#for: u32,
    pub specified: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATEKEYEDMUTEX {
    pub to: u32,
    pub mutex: u32,
    pub process: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENKEYEDMUTEX {
    pub mutex: u32,
    pub process: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROYKEYEDMUTEX {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ACQUIREKEYEDMUTEX {
    pub mutex: u32,
    pub Acquire: u32,
    pub value: u32,
    pub object: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_RELEASEKEYEDMUTEX {
    pub mutex: u32,
    pub to: u32,
    pub object: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATEKEYEDMUTEX2 {
    pub to: u32,
    pub process: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENKEYEDMUTEX2 {
    pub mutex: u32,
    pub process: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENKEYEDMUTEXFROMNTHANDLE {
    pub mutex: u32,
    pub process: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_ACQUIREKEYEDMUTEX2 {
    pub mutex: u32,
    pub Acquire: u32,
    pub value: u32,
    pub object: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_RELEASEKEYEDMUTEX2 {
    pub mutex: u32,
    pub to: u32,
    pub object: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CONFIGURESHAREDRESOURCE {
    pub resource: u32,
    pub resource: u32,
    pub DWM: u8,
    pub case: u32,
    pub access: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CHECKSHAREDRESOURCEACCESS {
    pub resource: u32,
    pub PID: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OFFERALLOCATIONS {
    pub allocations: u32,
    pub allocations: u32,
    pub behavior: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_RECLAIMALLOCATIONS {
    pub allocations: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OUTPUTDUPL_KEYEDMUTEX {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATE_OUTPUTDUPL {
    pub hAdapter: u32,
    pub VidPnSourceId: u32,
    pub check: u32,
    pub needed: u32,
    pub KeyedMutexs: [u32; 1],
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROY_OUTPUTDUPL {
    pub hAdapter: u32,
    pub VidPnSourceId: u32,
    pub bDestroyAllContexts: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OUTPUTDUPL_POINTER_POSITION {
    pub Position: u32,
    pub Visible: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OUTPUTDUPL_FRAMEINFO {
    pub LastPresentTime: u32,
    pub LastMouseUpdateTime: u32,
    pub AccumulatedFrames: u32,
    pub RectsCoalesced: i32,
    pub ProtectedContentMaskedOut: i32,
    pub PointerPosition: u32,
    pub TotalMetadataBufferSize: u32,
    pub PointerShapeBufferSize: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OUTPUTDUPL_GET_FRAMEINFO {
    pub hAdapter: u32,
    pub VidPnSourceId: u32,
    pub FrameInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OUTPUTDUPL_METADATA {
    pub hAdapter: u32,
    pub VidPnSourceId: u32,
    pub Type: u32,
    pub BufferSizeSupplied: u32,
    pub BufferSizeRequired: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OUTDUPL_POINTER_SHAPE_INFO {
    pub Type: u32,
    pub Width: u32,
    pub Height: u32,
    pub Pitch: u32,
    pub HotSpot: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OUTPUTDUPL_GET_POINTER_SHAPE_DATA {
    pub hAdapter: u32,
    pub VidPnSourceId: u32,
    pub BufferSizeSupplied: u32,
    pub BufferSizeRequired: u32,
    pub ShapeInfo: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OUTPUTDUPL_RELEASE_FRAME {
    pub hAdapter: u32,
    pub VidPnSourceId: u32,
    pub r#use: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETSHAREDRESOURCEADAPTERLUID {
    pub handle: u32,
    pub handle: u32,
    pub LUID: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_HYBRID_LIST {
    pub state: u32,
    pub D3DKMT_GPU_PREFERENCE_TYPE_IHV_DLIST: u32,
    pub query: i32,
    pub D3DKMT_GPU_PREFERENCE_QUERY_TYPE: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_WAITFORSYNCHRONIZATIONOBJECTFROMCPU {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SIGNALSYNCHRONIZATIONOBJECTFROMCPU {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATEPAGINGQUEUE {
    pub device: u32,
    pub CPU: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_EVICT {
    pub allocations: u32,
    pub handles: u32,
    pub handles: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_LOCK2 {
    pub lock: u32,
    pub D3DDDI_LOCK2FLAGS: u32,
    pub allocation: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_UNLOCK2 {
    pub unlock: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_INVALIDATECACHE {
    pub hDevice: u32,
    pub hAllocation: u32,
    pub Offset: u32,
    pub Length: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_FREEGPUVIRTUALADDRESS {
    pub bytes: u32,
    pub bytes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATECONTEXTVIRTUAL {
//     pub r#in:: u32,
//     pub r#in:: u32,
//     pub r#in:: u32,
//     pub r#in:: u32,
//     pub r#in:: u32,
//     pub r#in:: u32,
    pub context: u32,
//     pub out:: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SUBMITCOMMANDFLAGS {
// //     pub 0x00000001: u32,
// //     pub 0x00000002: u32,
// //     pub 0x00000004: u32,
// //     pub 0xFFFFFFF8: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SUBMITCOMMAND {
    pub Commands: u32,
    pub CommandLength: u32,
    pub Flags: u32,
    pub calls: u32,
    pub BroadcastContextCount: u32,
    pub BroadcastContext: [u32; 1],
    pub PrivateDriverDataSize: u32,
    pub NumPrimaries: u32,
    pub WrittenPrimaries: [u32; 1],
    pub NumHistoryBuffers: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SUBMITCOMMANDTOHWQUEUE {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SUBMITWAITFORSYNCOBJECTSTOHWQUEUE {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SUBMITSIGNALSYNCOBJECTSTOHWQUEUE {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYVIDEOMEMORYINFO {
    pub process: u32,
    pub r#use: u32,
    pub device: u32,
    pub device: u32,
    pub reserve: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CHANGEVIDEOMEMORYRESERVATION {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETSTABLEPOWERSTATE {
    pub r#for: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SHAREOBJECTWITHHOST {
    pub r#in: u32,
    pub r#in: u32,
    pub r#use: u32,
    pub out: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATESYNCFILE {
    pub object: u32,
    pub r#for: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_WAITSYNCFILE {
    pub inserted: u32,
    pub r#use: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENSYNCOBJECTFROMSYNCFILE {
    pub object: u32,
    pub handle: u32,
    pub file: u32,
    pub CPU: u32,
    pub GPU: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_TRIMNOTIFICATION {
    pub Register: u32,
    pub flags: u32,
    pub VidMm: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_REGISTERTRIMNOTIFICATION {
    pub AdapterLuid: u32,
    pub hDevice: u32,
    pub Callback: u32,
    pub context: u32,
    pub Unregister: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_UNREGISTERTRIMNOTIFICATION {
    pub Callback: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_BUDGETCHANGENOTIFICATION {
    pub Register: u32,
    pub budget: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_REGISTERBUDGETCHANGENOTIFICATION {
    pub hDevice: u32,
    pub context: u32,
    pub Unregister: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_UNREGISTERBUDGETCHANGENOTIFICATION {
    pub register: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYVIDPNEXCLUSIVEOWNERSHIP {
    pub handle: u32,
    pub handle: u32,
    pub ID: u32,
    pub LUID: u32,
    pub Type: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_MARKDEVICEASERROR {
    pub handle: u32,
    pub code: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_FLUSHHEAPTRANSITIONS {
    pub hAdapter: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYPROCESSOFFERINFO {
    pub cbSize: u32,
    pub DecommitUniqueness: u32,
    pub DecommittableBytes: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_TRIMPROCESSCOMMITMENT {
    pub cbSize: u32,
    pub Flags: u32,
    pub DecommitRequested: u32,
    pub NumBytesDecommitted: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATEHWCONTEXT {
    pub data: u32,
    pub data: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROYHWCONTEXT {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATEHWQUEUE {
    pub data: u32,
    pub data: u32,
    pub CPU: u32,
    pub GPU: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROYHWQUEUE {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETALLOCATIONPRIORITY {
    pub device: u32,
    pub phAllocationList: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_SETFSEBLOCK {
    pub AdapterLuid: u32,
    pub hAdapter: u32,
    pub VidPnSourceId: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYFSEBLOCK {
    pub AdapterLuid: u32,
    pub hAdapter: u32,
    pub VidPnSourceId: u32,
    pub Flags: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_CREATEPROTECTEDSESSION {
    pub handle: u32,
    pub data: u32,
    pub data: u32,
    pub data: u32,
    pub data: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_DESTROYPROTECTEDSESSION {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYPROTECTEDSESSIONSTATUS {
    pub status: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_QUERYPROTECTEDSESSIONINFOFROMNTHANDLE {
    pub data: u32,
    pub data: u32,
    pub data: u32,
    pub data: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_OPENPROTECTEDSESSIONFROMNTHANDLE {
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct D3DKMT_GETPROCESSDEVICEREMOVALSUPPORT {
    pub handle: u32,
    pub detached: u32,
    pub removal: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_NATIVE_FENCE_LOG_ENTRY {
    pub value: u64,
    pub belongs: u32,
    pub r#type: u32,
    pub alignment: u64,
    pub HWQueue: u64,
    pub alignment: u64,
    pub GPU: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_NATIVE_FENCE_LOG_BUFFER {
    pub Header: u32,
    pub Entries: [u32; 1],
}


#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_ADD_DEVICE {
    pub device_object: *mut c_void,
    pub miniport_device_context: *mut *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGKARG_START_DEVICE {
    pub start_info: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_DRIVERCAPS {
    pub wddm_version: u32,
    pub highest_target_id: u32,
    pub max_allocation_list_count: u32,
    pub max_patch_location_list_count: u32,
    pub scheduling_caps: u32,
    pub memory_management_caps: u32,
    pub security_caps: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DXGK_GPUMMUCAPS {
    pub read_only_memory_supported: u32,
    pub no_execute_memory_supported: u32,
    pub cache_coherent_memory_supported: u32,
    pub virtual_address_bit_count: u32,
    pub physical_address_bit_count: u32,
    pub page_table_level_count: u32,
    pub page_table_page_size: u32,
}
