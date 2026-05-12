# Mapa de Ring 0 - Core NT Kernel

**Total Binarios:** 4

## `ci.dll`
- **Imports (Dependencias):** 6
- **Exports (Funciones que ofrece):** 0

---
## `hal.dll`
- **Imports (Dependencias):** 0
- **Exports (Funciones que ofrece):** 0

---
## `kdcom.dll`
- **Imports (Dependencias):** 2
- **Exports (Funciones que ofrece):** 5

### Exportaciones Clave:
```c
void KdInitialize();
void KdPower();
void KdReceivePacket();
void KdSendPacket();
void KdSetHiberRange();
```

---
## `ntoskrnl.exe`
- **Imports (Dependencias):** 22
- **Exports (Funciones que ofrece):** 3352

### Exportaciones Clave:
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
void CcInitializeCacheMapEx();
void CcInitializeCacheMapEx2();
void CcIsCacheManagerCallbackNeeded();
void CcIsThereDirtyData();
void CcIsThereDirtyDataEx();
void CcIsThereDirtyLoggedPages();
void CcMapData();
void CcMdlRead();
void CcMdlReadComplete();
void CcMdlWriteAbort();
void CcMdlWriteComplete();
void CcPinMappedData();
void CcPinRead();
void CcPrepareMdlWrite();
void CcPreparePinWrite();
void CcPurgeCacheSection();
void CcRegisterExternalCache();
void CcRemapBcb();
void CcRepinBcb();
void CcScheduleReadAhead();
void CcScheduleReadAheadEx();
void CcSetAdditionalCacheAttributes();
void CcSetAdditionalCacheAttributesEx();
void CcSetBcbOwnerPointer();
void CcSetDirtyPageThreshold();
void CcSetDirtyPinnedData();
void CcSetFileSizes();
void CcSetFileSizesEx();
void CcSetLogHandleForFile();
void CcSetLogHandleForFileEx();
void CcSetLoggedDataThreshold();
void CcSetParallelFlushFile();
void CcSetReadAheadGranularity();
void CcSetReadAheadGranularityEx();
void CcTestControl();
void CcUninitializeCacheMap();
void CcUnmapFileOffsetFromSystemCache();
void CcUnpinData();
void CcUnpinDataForThread();
void CcUnpinRepinnedBcb();
void CcUnregisterExternalCache();
void CcWaitForCurrentLazyWriterActivity();
void CcZeroData();
void CcZeroDataOnDisk();
void CmCallbackGetKeyObjectID();
void CmCallbackGetKeyObjectIDEx();
void CmCallbackReleaseKeyObjectIDEx();
void CmGetBoundTransaction();
void CmGetCallbackVersion();
void CmKeyObjectType();
void CmRegisterCallback();
void CmRegisterCallbackEx();
void CmRegisterMachineHiveLoadedNotification();
void CmSetCallbackObjectContext();
void CmUnRegisterCallback();
void CmUnregisterMachineHiveLoadedNotification();
void CsanRead16NoCheck();
void CsanRead64NoCheck();
void CsanRead8NoCheck();
void CsanReadNoCheck();
void CsanWrite16NoCheck();
void CsanWrite64NoCheck();
void CsanWrite8NoCheck();
void CsanWriteNoCheck();
void DbgBreakPoint();
void DbgBreakPointWithStatus();
void DbgCommandString();
void DbgLoadImageSymbols();
void DbgPrint();
void DbgPrintEx();
void DbgPrintReturnControlC();
void DbgPrompt();
void DbgQueryDebugFilterState();
void DbgSetDebugFilterState();
void DbgSetDebugPrintCallback();
void DbgkLkmdRegisterCallback();
void DbgkLkmdUnregisterCallback();
void DbgkWerCaptureLiveKernelDump();
void DbgkWerCaptureLiveKernelDump2();
void DifEnumeratePluginData();
void DifFindThreadContextData();
void DifGetPluginPerDriverData();
void DifObjTrkInsertItem();
void DifObjTrkQeuryInvokeDeleteRange();
void DifObjTrkRemoveItem();
void DifPluginSimplePerfControl();
void DifPopThreadContextData();
void DifPushThreadContextData();
void DifRegisterClassDriverPlugin();
void DifRegisterObjectTracking();
void DifRegisterPlugin();
void DifUtilDbgPrint();
void EmClientQueryRuleState();
void EmClientRuleDeregisterNotification();
void EmClientRuleEvaluate();
void EmClientRuleRegisterNotification();
void EmProviderDeregister();
void EmProviderDeregisterEntry();
void EmProviderRegister();
void EmProviderRegisterEntry();
void EmpProviderRegister();
void EtwActivityIdControl();
void EtwEnableTrace();
void EtwEventEnabled();
void EtwProviderEnabled();
void EtwRegister();
void EtwRegisterClassicProvider();
void EtwSendTraceBuffer();
void EtwSetInformation();
void EtwTelemetryCoverageReport();
void EtwUnregister();
void EtwWrite();
void EtwWriteEndScenario();
void EtwWriteEx();
void EtwWriteStartScenario();
void EtwWriteString();
void EtwWriteTransfer();
void EtwpDisableStackWalkApc();
void EtwpReenableStackWalkApc();
void ExAccessByte();
void ExAcquireAutoExpandPushLockExclusive();
void ExAcquireAutoExpandPushLockShared();
void ExAcquireCacheAwarePushLockExclusive();
void ExAcquireCacheAwarePushLockExclusiveEx();
void ExAcquireCacheAwarePushLockSharedEx();
void ExAcquireFastMutex();
void ExAcquireFastMutexUnsafe();
void ExAcquireFastResourceExclusive();
void ExAcquireFastResourceShared();
void ExAcquireFastResourceSharedStarveExclusive();
void ExAcquireFastResourceWithFlags();
void ExAcquirePushLockExclusiveEx();
void ExAcquirePushLockSharedEx();
void ExAcquireResourceExclusiveLite();
void ExAcquireResourceSharedLite();
void ExAcquireRundownProtection();
void ExAcquireRundownProtectionCacheAware();
void ExAcquireRundownProtectionCacheAwareEx();
void ExAcquireRundownProtectionEx();
void ExAcquireSharedStarveExclusive();
void ExAcquireSharedWaitForExclusive();
void ExAcquireSpinLockExclusive();
void ExAcquireSpinLockExclusiveAtDpcLevel();
void ExAcquireSpinLockShared();
void ExAcquireSpinLockSharedAtDpcLevel();
void ExActivationObjectType();
void ExAllocateAutoExpandPushLock();
void ExAllocateCacheAwarePushLock();
void ExAllocateCacheAwareRundownProtection();
void ExAllocateFromLookasideListEx();
void ExAllocateFromNPagedLookasideList();
// ... y 3152 más
```

---
