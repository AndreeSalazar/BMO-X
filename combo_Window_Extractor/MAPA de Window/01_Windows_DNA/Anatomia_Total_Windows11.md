# Anatomía Total de Windows 11 (Pure OS / UEFI Focus)

Este mapa ignora todo lo relacionado con gráficos (GPU/WDDM) y se centra en los órganos vitales de Windows 11 para operar bajo UEFI puro.

## 1. UEFI y Boot Chain

### `winload.exe`
- **Dependencias (Imports):** 0
- **Funciones Ofrecidas (Exports):** 0

### `winload.exe`
- **Dependencias (Imports):** 0
- **Funciones Ofrecidas (Exports):** 0

---

## 2. Virtualization Based Security (VBS)

### `hvix64.exe`
- **Dependencias (Imports):** 1
- **Funciones Ofrecidas (Exports):** 0

### `securekernel.exe`
- **Dependencias (Imports):** 0
- **Funciones Ofrecidas (Exports):** 0

---

## 3. Core NT Executive (El Cerebro)

### `hal.dll`
- **Dependencias (Imports):** 0
- **Funciones Ofrecidas (Exports):** 0

### `ntoskrnl.exe`
- **Dependencias (Imports):** 22
- **Funciones Ofrecidas (Exports):** 3352

```c
void AlpcCreateSecurityContext();
void AlpcGetHeaderSize();
void AlpcGetMessageAttribute();
void AlpcInitializeMessageAttribute();
void BgkDisplayCharacter();
void BgkGetConsoleState();
void BgkGetCursorState();
void BgkSetCursor();
void CarCopyRuleViolationDetails();
void CarCreateRuleViolationDetails();
void CarDeleteRuleViolationDetails();
void CarDeregisterRuleClassConfiguration();
void CarDeregisterRuleOverride();
void CarInitializeRuleViolationDetails();
void CarQueryReportAction();
void CarQueryReportActionForTriage();
void CarRegisterDefaultRuleClassConfiguration();
void CarRegisterRuleClassConfiguration();
void CarRegisterRuleOverride();
void CarRegisterRuleOverrideAllContexts();
void CarRegisterRuleOverridesAllContexts();
void CarReportDifPluginRuleViolation();
void CarSetCustomIdInRuleOverride();
void CarSetCustomRuleIdRange();
void CcAddDirtyPagesToExternalCache();
void CcAsyncCopyRead();
void CcCanIWrite();
void CcCoherencyFlushAndPurgeCache();
void CcCopyRead();
void CcCopyReadEx();
void CcCopyWrite();
void CcCopyWriteEx();
void CcCopyWriteWontFlush();
void CcDeductDirtyPagesFromExternalCache();
void CcDeferWrite();
void CcErrorCallbackRoutine();
void CcFastCopyRead();
void CcFastCopyWrite();
void CcFastMdlReadWait();
void CcFlushCache();
void CcFlushCacheToLsn();
void CcGetCachedDirtyPageCountForFile();
void CcGetDirtyPages();
void CcGetFileObjectFromBcb();
void CcGetFileObjectFromSectionPtrs();
void CcGetFileObjectFromSectionPtrsRef();
void CcGetFlushedValidData();
void CcGetLsnForFileObject();
void CcGetNumberOfMappedPages();
void CcInitializeCacheMap();
// ... 3302 más
```

---

## 4. Storage y Filesystem (Acceso a Disco)

### `Classpnp.sys`
- **Dependencias (Imports):** 2
- **Funciones Ofrecidas (Exports):** 64

```c
void ClassAcquireChildLock();
void ClassAcquireRemoveLockEx();
void ClassAsynchronousCompletion();
void ClassBuildRequest();
void ClassCheckMediaState();
void ClassClaimDevice();
void ClassCleanupMediaChangeDetection();
void ClassCompleteRequest();
void ClassCreateDeviceObject();
void ClassDebugPrint();
void ClassDeleteSrbLookasideList();
void ClassDeviceControl();
void ClassDisableMediaChangeDetection();
void ClassEnableMediaChangeDetection();
void ClassFindModePage();
void ClassForwardIrpSynchronous();
void ClassGetDescriptor();
void ClassGetDeviceParameter();
void ClassGetDriverExtension();
void ClassGetFsContext();
void ClassGetVpb();
void ClassInitialize();
void ClassInitializeEx();
void ClassInitializeMediaChangeDetection();
void ClassInitializeSrbLookasideList();
void ClassInitializeTestUnitPolling();
void ClassInternalIoControl();
void ClassInterpretSenseInfo();
void ClassInvalidateBusRelations();
void ClassIoComplete();
void ClassIoCompleteAssociated();
void ClassMarkChildMissing();
void ClassMarkChildrenMissing();
void ClassModeSelect();
void ClassModeSense();
void ClassModeSenseEx();
void ClassModeSenseTranslate();
void ClassNotifyFailurePredicted();
void ClassQueryTimeOutRegistryValue();
void ClassReadDriveCapacity();
void ClassReleaseChildLock();
void ClassReleaseQueue();
void ClassReleaseRemoveLock();
void ClassRemoveDevice();
void ClassResetMediaChangeTimer();
void ClassScanForSpecial();
void ClassSendDeviceIoControlSynchronous();
void ClassSendIrpSynchronous();
void ClassSendNotification();
void ClassSendSrbAsynchronous();
// ... 14 más
```

### `disk.sys`
- **Dependencias (Imports):** 2
- **Funciones Ofrecidas (Exports):** 0

### `fltMgr.sys`
- **Dependencias (Imports):** 9
- **Funciones Ofrecidas (Exports):** 0

### `ntfs.sys`
- **Dependencias (Imports):** 0
- **Funciones Ofrecidas (Exports):** 0

### `partmgr.sys`
- **Dependencias (Imports):** 3
- **Funciones Ofrecidas (Exports):** 0

### `storahci.sys`
- **Dependencias (Imports):** 1
- **Funciones Ofrecidas (Exports):** 0

### `stornvme.sys`
- **Dependencias (Imports):** 2
- **Funciones Ofrecidas (Exports):** 2

```c
void DumpPreInitialize();
void DumpUninitialize();
```

### `disk.sys`
- **Dependencias (Imports):** 2
- **Funciones Ofrecidas (Exports):** 0

### `storahci.sys`
- **Dependencias (Imports):** 1
- **Funciones Ofrecidas (Exports):** 0

### `stornvme.sys`
- **Dependencias (Imports):** 2
- **Funciones Ofrecidas (Exports):** 2

```c
void DumpPreInitialize();
void DumpUninitialize();
```

---

## 5. Process & Security Init (Primeros Procesos)

### `csrss.exe`
- **Dependencias (Imports):** 2
- **Funciones Ofrecidas (Exports):** 0

### `lsass.exe`
- **Dependencias (Imports):** 21
- **Funciones Ofrecidas (Exports):** 4

```c
void LsaGetInterface();
void LsaImpersonateKsecCaller();
void LsaRegisterExtension();
void LsaRegisterInterface();
```

### `services.exe`
- **Dependencias (Imports):** 54
- **Funciones Ofrecidas (Exports):** 0

### `smss.exe`
- **Dependencias (Imports):** 1
- **Funciones Ofrecidas (Exports):** 0

### `wininit.exe`
- **Dependencias (Imports):** 50
- **Funciones Ofrecidas (Exports):** 0

### `winlogon.exe`
- **Dependencias (Imports):** 64
- **Funciones Ofrecidas (Exports):** 0

---

## 6. Redes y Comunicaciones (Network Stack)

### `afd.sys`
- **Dependencias (Imports):** 8
- **Funciones Ofrecidas (Exports):** 0

### `ndis.sys`
- **Dependencias (Imports):** 12
- **Funciones Ofrecidas (Exports):** 0

### `netio.sys`
- **Dependencias (Imports):** 9
- **Funciones Ofrecidas (Exports):** 0

### `tcpip.sys`
- **Dependencias (Imports):** 101
- **Funciones Ofrecidas (Exports):** 0

---

