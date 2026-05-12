# Mapa de Ring 3 - WDDM User Mode / DirectX

**Total Binarios:** 9

## `d3d10core.dll`
- **Imports (Dependencias):** 12
- **Exports (Funciones que ofrece):** 40

### Exportaciones Clave:
```c
void D3D10CoreCreateDevice();
void D3D10CoreGetSupportedVersions();
void D3D10CoreGetVersion();
void D3D10CoreRegisterLayers();
void D3DKMTCloseAdapter();
void D3DKMTCreateAllocation();
void D3DKMTCreateContext();
void D3DKMTCreateDevice();
void D3DKMTCreateSynchronizationObject();
void D3DKMTDestroyAllocation();
void D3DKMTDestroyContext();
void D3DKMTDestroyDevice();
void D3DKMTDestroySynchronizationObject();
void D3DKMTEscape();
void D3DKMTGetContextSchedulingPriority();
void D3DKMTGetDeviceState();
void D3DKMTGetDisplayModeList();
void D3DKMTGetMultisampleMethodList();
void D3DKMTGetRuntimeData();
void D3DKMTGetSharedPrimaryHandle();
void D3DKMTLock();
void D3DKMTOpenAdapterFromHdc();
void D3DKMTOpenResource();
void D3DKMTPresent();
void D3DKMTQueryAdapterInfo();
void D3DKMTQueryAllocationResidency();
void D3DKMTQueryResourceInfo();
void D3DKMTRender();
void D3DKMTSetAllocationPriority();
void D3DKMTSetContextSchedulingPriority();
void D3DKMTSetDisplayMode();
void D3DKMTSetDisplayPrivateDriverFormat();
void D3DKMTSetGammaRamp();
void D3DKMTSetVidPnSourceOwner();
void D3DKMTSignalSynchronizationObject();
void D3DKMTUnlock();
void D3DKMTWaitForSynchronizationObject();
void D3DKMTWaitForVerticalBlankEvent();
void OpenAdapter10();
void OpenAdapter10_2();
```

---
## `d3d10level9.dll`
- **Imports (Dependencias):** 26
- **Exports (Funciones que ofrece):** 113

### Exportaciones Clave:
```c
void D3D10CheckLevel9Hardware();
void D3D10Level9DumpJournal();
void D3D11CreateDeviceExternalImplementation();
void D3DKMTAcquireKeyedMutex();
void D3DKMTAcquireKeyedMutex2();
void D3DKMTChangeVideoMemoryReservation();
void D3DKMTCheckMultiPlaneOverlaySupport();
void D3DKMTCheckMultiPlaneOverlaySupport2();
void D3DKMTCloseAdapter();
void D3DKMTConfigureSharedResource();
void D3DKMTCreateAllocation();
void D3DKMTCreateAllocation2();
void D3DKMTCreateContext();
void D3DKMTCreateContextVirtual();
void D3DKMTCreateDevice();
void D3DKMTCreateKeyedMutex();
void D3DKMTCreateKeyedMutex2();
void D3DKMTCreatePagingQueue();
void D3DKMTCreateSynchronizationObject();
void D3DKMTCreateSynchronizationObject2();
void D3DKMTDestroyAllocation();
void D3DKMTDestroyAllocation2();
void D3DKMTDestroyContext();
void D3DKMTDestroyDevice();
void D3DKMTDestroyKeyedMutex();
void D3DKMTDestroyPagingQueue();
void D3DKMTDestroySynchronizationObject();
void D3DKMTEscape();
void D3DKMTEvict();
void D3DKMTFlushHeapTransitions();
void D3DKMTFreeGpuVirtualAddress();
void D3DKMTGetAllocationPriority();
void D3DKMTGetContextInProcessSchedulingPriority();
void D3DKMTGetContextSchedulingPriority();
void D3DKMTGetDeviceSchedulingPriority();
void D3DKMTGetDeviceState();
void D3DKMTGetDisplayModeList();
void D3DKMTGetMultisampleMethodList();
void D3DKMTGetResourcePresentPrivateDriverData();
void D3DKMTGetRuntimeData();
void D3DKMTGetSharedPrimaryHandle();
void D3DKMTGetThunkVersion();
void D3DKMTInvalidateCache();
void D3DKMTLock();
void D3DKMTLock2();
void D3DKMTMakeResident();
void D3DKMTMapGpuVirtualAddress();
void D3DKMTMarkDeviceAsError();
void D3DKMTOfferAllocations();
void D3DKMTOpenAdapterFromDeviceName();
void D3DKMTOpenAdapterFromGdiDisplayName();
void D3DKMTOpenKeyedMutex();
void D3DKMTOpenKeyedMutex2();
void D3DKMTOpenNtHandleFromName();
void D3DKMTOpenResource();
void D3DKMTOpenResource2();
void D3DKMTOpenResourceFromNtHandle();
void D3DKMTOpenSyncObjectFromNtHandle();
void D3DKMTOpenSyncObjectFromNtHandle2();
void D3DKMTOpenSyncObjectNtHandleFromName();
void D3DKMTOpenSynchronizationObject();
void D3DKMTOutputDuplPresent();
void D3DKMTPinDirectFlipResources();
void D3DKMTPresent();
void D3DKMTPresentMultiPlaneOverlay();
void D3DKMTPresentMultiPlaneOverlay2();
void D3DKMTQueryAdapterInfo();
void D3DKMTQueryAllocationResidency();
void D3DKMTQueryClockCalibration();
void D3DKMTQueryResourceInfo();
void D3DKMTQueryResourceInfoFromNtHandle();
void D3DKMTQueryVideoMemoryInfo();
void D3DKMTReclaimAllocations();
void D3DKMTReclaimAllocations2();
void D3DKMTRegisterTrimNotification();
void D3DKMTReleaseKeyedMutex();
void D3DKMTReleaseKeyedMutex2();
void D3DKMTRender();
void D3DKMTReserveGpuVirtualAddress();
void D3DKMTSetAllocationPriority();
void D3DKMTSetContextInProcessSchedulingPriority();
void D3DKMTSetContextSchedulingPriority();
void D3DKMTSetDeviceSchedulingPriority();
void D3DKMTSetDisplayMode();
void D3DKMTSetDisplayPrivateDriverFormat();
void D3DKMTSetGammaRamp();
void D3DKMTSetQueuedLimit();
void D3DKMTSetStablePowerState();
void D3DKMTSetVidPnSourceOwner();
void D3DKMTSetVidPnSourceOwner1();
void D3DKMTShareObjects();
void D3DKMTSignalSynchronizationObject();
void D3DKMTSignalSynchronizationObject2();
void D3DKMTSignalSynchronizationObjectFromCpu();
void D3DKMTSignalSynchronizationObjectFromGpu();
void D3DKMTSignalSynchronizationObjectFromGpu2();
void D3DKMTSubmitCommand();
void D3DKMTUnlock();
void D3DKMTUnlock2();
void D3DKMTUnpinDirectFlipResources();
void D3DKMTUnregisterTrimNotification();
void D3DKMTUpdateAllocationProperty();
void D3DKMTUpdateGpuVirtualAddress();
void D3DKMTWaitForSynchronizationObject();
void D3DKMTWaitForSynchronizationObject2();
void D3DKMTWaitForSynchronizationObjectFromCpu();
void D3DKMTWaitForSynchronizationObjectFromGpu();
void D3DKMTWaitForVerticalBlankEvent();
void D3DKMTWaitForVerticalBlankEvent2();
void LogMarkerStringTable();
void OpenAdapter10();
void OpenAdapter10_2();
void RetrieveFilteredOpenAdapter();
```

---
## `d3d10warp.dll`
- **Imports (Dependencias):** 44
- **Exports (Funciones que ofrece):** 10

### Exportaciones Clave:
```c
void D3D11RefGetLastCreation();
void D3DLayerGetInterface();
void OpenAdapter();
void OpenAdapter10_2();
void OpenAdapter12();
void OpenDisplayAdapter1();
void QueryDListForApplication1();
void QueryDListForApplication2();
void QueryMuxDListForApplication();
void VSD3DDebugConnectionBuffer();
```

---
## `d3d10_1core.dll`
- **Imports (Dependencias):** 12
- **Exports (Funciones que ofrece):** 40

### Exportaciones Clave:
```c
void D3D10CoreCreateDevice1();
void D3D10CoreGetSupportedVersions();
void D3D10CoreGetVersion();
void D3D10CoreRegisterLayers();
void D3DKMTCloseAdapter();
void D3DKMTCreateAllocation();
void D3DKMTCreateContext();
void D3DKMTCreateDevice();
void D3DKMTCreateSynchronizationObject();
void D3DKMTDestroyAllocation();
void D3DKMTDestroyContext();
void D3DKMTDestroyDevice();
void D3DKMTDestroySynchronizationObject();
void D3DKMTEscape();
void D3DKMTGetContextSchedulingPriority();
void D3DKMTGetDeviceState();
void D3DKMTGetDisplayModeList();
void D3DKMTGetMultisampleMethodList();
void D3DKMTGetRuntimeData();
void D3DKMTGetSharedPrimaryHandle();
void D3DKMTLock();
void D3DKMTOpenAdapterFromHdc();
void D3DKMTOpenResource();
void D3DKMTPresent();
void D3DKMTQueryAdapterInfo();
void D3DKMTQueryAllocationResidency();
void D3DKMTQueryResourceInfo();
void D3DKMTRender();
void D3DKMTSetAllocationPriority();
void D3DKMTSetContextSchedulingPriority();
void D3DKMTSetDisplayMode();
void D3DKMTSetDisplayPrivateDriverFormat();
void D3DKMTSetGammaRamp();
void D3DKMTSetVidPnSourceOwner();
void D3DKMTSignalSynchronizationObject();
void D3DKMTUnlock();
void D3DKMTWaitForSynchronizationObject();
void D3DKMTWaitForVerticalBlankEvent();
void OpenAdapter10();
void OpenAdapter10_2();
```

---
## `d3d11.dll`
- **Imports (Dependencias):** 38
- **Exports (Funciones que ofrece):** 51

### Exportaciones Clave:
```c
void CreateDirect3D11DeviceFromDXGIDevice();
void CreateDirect3D11SurfaceFromDXGISurface();
void D3D11CoreCreateDevice();
void D3D11CoreCreateLayeredDevice();
void D3D11CoreGetLayeredDeviceSize();
void D3D11CoreRegisterLayers();
void D3D11CreateDevice();
void D3D11CreateDeviceAndSwapChain();
void D3D11CreateDeviceForD3D12();
void D3D11On12CreateDevice();
void D3DKMTCloseAdapter();
void D3DKMTCreateAllocation();
void D3DKMTCreateContext();
void D3DKMTCreateDevice();
void D3DKMTCreateSynchronizationObject();
void D3DKMTDestroyAllocation();
void D3DKMTDestroyContext();
void D3DKMTDestroyDevice();
void D3DKMTDestroySynchronizationObject();
void D3DKMTEscape();
void D3DKMTGetContextSchedulingPriority();
void D3DKMTGetDeviceState();
void D3DKMTGetDisplayModeList();
void D3DKMTGetMultisampleMethodList();
void D3DKMTGetRuntimeData();
void D3DKMTGetSharedPrimaryHandle();
void D3DKMTLock();
void D3DKMTOpenAdapterFromHdc();
void D3DKMTOpenResource();
void D3DKMTPresent();
void D3DKMTQueryAdapterInfo();
void D3DKMTQueryAllocationResidency();
void D3DKMTQueryResourceInfo();
void D3DKMTRender();
void D3DKMTSetAllocationPriority();
void D3DKMTSetContextSchedulingPriority();
void D3DKMTSetDisplayMode();
void D3DKMTSetDisplayPrivateDriverFormat();
void D3DKMTSetGammaRamp();
void D3DKMTSetVidPnSourceOwner();
void D3DKMTSignalSynchronizationObject();
void D3DKMTUnlock();
void D3DKMTWaitForSynchronizationObject();
void D3DKMTWaitForVerticalBlankEvent();
void D3DPerformance_BeginEvent();
void D3DPerformance_EndEvent();
void D3DPerformance_GetStatus();
void D3DPerformance_SetMarker();
void EnableFeatureLevelUpgrade();
void OpenAdapter10();
void OpenAdapter10_2();
```

---
## `D3D12.dll`
- **Imports (Dependencias):** 25
- **Exports (Funciones que ofrece):** 18

### Exportaciones Clave:
```c
void D3D12CoreCreateLayeredDevice();
void D3D12CoreGetLayeredDeviceSize();
void D3D12CoreRegisterLayers();
void D3D12CreateDevice();
void D3D12CreateRootSignatureDeserializer();
void D3D12CreateVersionedRootSignatureDeserializer();
void D3D12DeviceRemovedExtendedData();
void D3D12EnableExperimentalFeatures();
void D3D12GetDebugInterface();
void D3D12GetInterface();
void D3D12PIXEventsReplaceBlock();
void D3D12PIXGetThreadInfo();
void D3D12PIXNotifyWakeFromFenceSignal();
void D3D12PIXReportCounter();
void D3D12SerializeRootSignature();
void D3D12SerializeVersionedRootSignature();
void GetBehaviorValue();
void SetAppCompatStringPointer();
```

---
## `D3D12Core.dll`
- **Imports (Dependencias):** 47
- **Exports (Funciones que ofrece):** 2

### Exportaciones Clave:
```c
void D3D12GetInterface();
void D3D12SDKVersion();
```

---
## `d3d9.dll`
- **Imports (Dependencias):** 18
- **Exports (Funciones que ofrece):** 17

### Exportaciones Clave:
```c
void D3DPERF_BeginEvent();
void D3DPERF_EndEvent();
void D3DPERF_GetStatus();
void D3DPERF_QueryRepeatFrame();
void D3DPERF_SetMarker();
void D3DPERF_SetOptions();
void D3DPERF_SetRegion();
void DebugSetLevel();
void DebugSetMute();
void Direct3D9EnableMaximizedWindowedModeShim();
void Direct3DCreate9();
void Direct3DCreate9Ex();
void Direct3DCreate9On12();
void Direct3DCreate9On12Ex();
void Direct3DShaderValidatorCreate9();
void PSGPError();
void PSGPSampleTexture();
```

---
## `dxgi.dll`
- **Imports (Dependencias):** 48
- **Exports (Funciones que ofrece):** 20

### Exportaciones Clave:
```c
void ApplyCompatResolutionQuirking();
void CompatString();
void CompatValue();
void CreateDXGIFactory();
void CreateDXGIFactory1();
void CreateDXGIFactory2();
void DXGID3D10CreateDevice();
void DXGID3D10CreateLayeredDevice();
void DXGID3D10GetLayeredDeviceSize();
void DXGID3D10RegisterLayers();
void DXGIDeclareAdapterRemovalSupport();
void DXGIDisableVBlankVirtualization();
void DXGIDumpJournal();
void DXGIGetDebugInterface1();
void DXGIReportAdapterConfiguration();
void PIXBeginCapture();
void PIXEndCapture();
void PIXGetCaptureState();
void SetAppCompatStringPointer();
void UpdateHMDEmulationStatus();
```

---
