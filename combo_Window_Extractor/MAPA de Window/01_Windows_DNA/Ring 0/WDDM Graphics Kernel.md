# Mapa de Ring 0 - WDDM Graphics Kernel

**Total Binarios:** 5

## `dxgkrnl.sys`
- **Imports (Dependencias):** 20
- **Exports (Funciones que ofrece):** 0

---
## `dxgmms2.sys`
- **Imports (Dependencias):** 4
- **Exports (Funciones que ofrece):** 3

### Exportaciones Clave:
```c
void DriverUnload();
void VidMmInterface();
void VidSchInterface();
```

---
## `watchdog.sys`
- **Imports (Dependencias):** 2
- **Exports (Funciones que ofrece):** 136

### Exportaciones Clave:
```c
void ??0WatchdogTimeoutReport@@QEAA@K_K000T_WD_LIVEREPORT_FLAGS@@P6AXPEAV0@@ZP6A_N2@Z_NI@Z();
void ??1WatchdogTimeoutReport@@QEAA@XZ();
void ?Callback@WatchdogTimeoutReport@@QEAAXXZ();
void ?Cancel@WatchdogTimeoutReport@@QEAAXXZ();
void ?Filter@WatchdogTimeoutReport@@QEAA_NXZ();
void ?GetArg1@WatchdogTimeoutReport@@QEAA_KXZ();
void ?GetArg2@WatchdogTimeoutReport@@QEAA_KXZ();
void ?GetArg3@WatchdogTimeoutReport@@QEAA_KXZ();
void ?GetArg4@WatchdogTimeoutReport@@QEAA_KXZ();
void ?GetCode@WatchdogTimeoutReport@@QEAAKXZ();
void ?GetIsActive@WatchdogTimeoutReport@@QEAA?C_NXZ();
void ?GetLiveDumpFlags@WatchdogTimeoutReport@@QEAA?AT_WD_LIVEREPORT_FLAGS@@XZ();
void ?GetLiveDumpWorkItem@WatchdogTimeoutReport@@QEAAPEAU_WORK_QUEUE_ITEM@@XZ();
void ?GetLiveDumpWorkItemEvent@WatchdogTimeoutReport@@QEAAPEAU_KEVENT@@XZ();
void ?ReportCount@WatchdogTimeoutReport@@2JC();
void ?StartTimer@WatchdogTimeoutReport@@QEAAXXZ();
void DMgrAcquireGdiViewId();
void DMgrGetSmbiosInfo();
void DMgrIsSetupRunning();
void DMgrReleaseGdiViewId();
void DMgrWriteDeviceCountToRegistry();
void DisplayLogSetMonitorPowerStage();
void DisplayRestoreVidPnJournalBegin();
void DisplayRestoreVidPnJournalFinalize();
void DisplayRestoreVidPnResult();
void DisplayScenarioContextDissociate();
void DisplayScenarioContextEnsureAndAssociate();
void DisplayScenarioContextFindAndAddRef();
void DisplayScenarioContextFindAndAssociate();
void DisplayScenarioContextGetCurrentActivityId();
void DisplayScenarioContextHolding();
void DisplayScenarioContextRelease();
void DisplayScenarioJounralSetTSDDDState();
void DisplayScenarioJournalBegin();
void DisplayScenarioJournalCCDRetrieval();
void DisplayScenarioJournalDPIInfo();
void DisplayScenarioJournalDisplayUniquenessIncremented();
void DisplayScenarioJournalFinalize();
void DisplayScenarioJournalMissingActualPathModality();
void DisplayScenarioJournalRetry();
void DisplayScenarioJournalSetActualPathModality();
void DisplayScenarioJournalSetCommitVidPnStatus();
void DisplayScenarioJournalSetExpectedPathModality();
void DisplayScenarioJournalSetResult();
void DisplayScenarioJournalSetSDCPathsAndModes();
void DisplayScenarioJournalSetSetTimingPathInfo();
void DisplayScenarioJournalSetSpecializedData();
void DisplayScenarioJournalSetUniqueness();
void DisplayScenarioJournalVidPnSourceVisibility();
void DisplayScenarioSetCCDRetrievalForActivity();
void SMgrGdiCallout();
void SMgrGetActiveSessionProcess();
void SMgrGetNumberOfSessions();
void SMgrNotifySessionChange();
void SMgrRegisterSessionChangeCallout();
void SMgrUnregisterSessionChangeCallout();
void VpInitialize();
void WdAllocateDeferredWatchdog();
void WdAllocateWatchdog();
void WdAttachContext();
void WdCompleteEvent();
void WdDbgCreateSnapshot();
void WdDbgDestroySnapshot();
void WdDbgGetSecondaryDataMaxSize();
void WdDbgReportCancel();
void WdDbgReportComplete();
void WdDbgReportCreate();
void WdDbgReportQueryInfo();
void WdDbgReportRecreate();
void WdDbgReportSecondaryData();
void WdDereferenceObject();
void WdDetachContext();
void WdDiagGetEtwHandle();
void WdDiagInit();
void WdDiagIsTracingEnabled();
void WdDiagNotifyUser();
void WdDiagShutdown();
void WdEnterMonitoredSection();
void WdExitMonitoredSection();
void WdFreeDeferredWatchdog();
void WdFreeWatchdog();
void WdGetDeviceObject();
void WdGetLastEvent();
void WdGetLowestDeviceObject();
void WdInitialize();
void WdIsDebuggerPresent();
void WdLogEvent5_WdAssertion();
void WdLogEvent5_WdCriticalError();
void WdLogEvent5_WdDebug();
void WdLogEvent5_WdDmmEvent();
void WdLogEvent5_WdError();
void WdLogEvent5_WdEvent();
void WdLogEvent5_WdLowResource();
void WdLogEvent5_WdPower();
void WdLogEvent5_WdPresentTokenEvent();
void WdLogEvent5_WdTrace();
void WdLogEvent5_WdWarning();
void WdLogGetEventOrder();
void WdLogGetRecentEvents();
void WdLogNewEntry5_WdAssertion();
void WdLogNewEntry5_WdCriticalError();
void WdLogNewEntry5_WdDebug();
void WdLogNewEntry5_WdDmmEvent();
void WdLogNewEntry5_WdError();
void WdLogNewEntry5_WdEvent();
void WdLogNewEntry5_WdLowResource();
void WdLogNewEntry5_WdPower();
void WdLogNewEntry5_WdPresentTokenEvent();
void WdLogNewEntry5_WdTrace();
void WdLogNewEntry5_WdWarning();
void WdLogSingleEntry0();
void WdLogSingleEntry1();
void WdLogSingleEntry2();
void WdLogSingleEntry3();
void WdLogSingleEntry4();
void WdLogSingleEntry5();
void WdMadeAnyProgress();
void WdQueryDebugFlag();
void WdReferenceObject();
void WdRegFreeInfo();
void WdRegOpenSubkey();
void WdRegRetrieveSubkeyInfo();
void WdRegRetrieveValueInfo();
void WdResetDeferredWatch();
void WdResetWatch();
void WdResumeDeferredWatch();
void WdResumeWatch();
void WdSetEventAndWaitForSingleObject();
void WdStartDeferredWatch();
void WdStartWatch();
void WdStopDeferredWatch();
void WdStopWatch();
void WdSuspendDeferredWatch();
void WdSuspendWatch();
void WdpDbgReportCreateFromDump();
void WdpInterfaceReferenceNop();
```

---
## `BasicDisplay.sys`
- **Imports (Dependencias):** 4
- **Exports (Funciones que ofrece):** 0

---
## `BasicRender.sys`
- **Imports (Dependencias):** 2
- **Exports (Funciones que ofrece):** 0

---
