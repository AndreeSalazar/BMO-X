# Análisis Forense de Drivers (Debug Strings)

Este documento contiene los secretos extraídos directamente de los binarios `.sys` compilados. Estas cadenas revelan rutinas ocultas e IOCTLs que no están exportados públicamente.

## `acpi.sys`
- **Total Strings Legibles:** 4373
- **Cadenas de Interés (Rutinas/IOCTLs/Errores):** 567

### Muestra de ADN Oculto:
```text
 Failed
%1: ACPI BIOS indicates that the machine has suffered a fatal error and needs to be shutdown as quickly as possible. Please contact your system vendor for technical assistance.
%1: The embedded controller (EC) did not respond within the specified timeout period. This may indicate that there is an error in the EC hardware or firmware or that the BIOS is accessing the EC incorrectly. You should check with your computer manufacturer for an upgraded BIOS. In some situations, this error may cause the computer to function incorrectly.
%s failed with the following error:
%s: Failed parsing buffer length for descriptor %x
%s: Failed parsing connection name %x
(WIRP_MN_START_DEVICE
ACPIBuildDiscoverDeviceCompletion
ACPIBuildDiscoverPowerNodeCompletion
ACPIInternalNotifyAvailableDeviceObject
ACPIMOFRESOURCE
ACPIMOFResource
ACPIRootDeviceNotifyPepDiscoverDevice
ACPI_TRIAGE_DUMP_COMPONENT
AMLIAddNamespaceOverride: fail to allocate name space object for override root
AMLIGlobalHeapSize
AMLIInitFlags
AMLIInitiaize: failed to allocate \_OSI name object
AMLIInitialize: failed to allocate \_OS name object
AMLIMaxCTObjs
AMLI_ERROR(%08x): %s
APEIOSCGranted
ARBITER_RANGE_BOOT_ALLOCATED
AccessBaseField: RegionSpace %x read handler returned error %x
AccumulatedFailureCount
AcpiFirmwareWatchDog
AcpiIrqArbAffinitizedInterrupt
AcpiSetupNativeMethodContext
AcpiTranslatePepDeviceControlResourcesInternal
AcquireASLMutex: failed to allocate context resource
AcquireGL: failed to acquire global lock
AllocationOrder
AmlMethodName
AmliMethodStatistics
AmliWatchdogAction
AmliWatchdogTimeout
Assertion failure
AssignmentSetOverride
Attributes
BankField
BankField: failed to allocate BankField object
BatteryFeaturesGranted
BreakPoint
BuffField
Buffer: failed to allocate data buffer (size=%I64d)
ButtonInstanceID
CPPCRevisionGranted
CheckSystemIOAddressValidity: Failed to allocate a workitem to spin off delayed logging.
CheckSystemIOAddressValidity: Failed to allocate contxt block from pool to spin off a logging work item.
CheckSystemIOAddressValidity: Failed to get ACPI root DeviceObject.
CheckSystemIOAddressValidity: failing illegal IO address (0x%x).
CompanyName
Concat: failed to allocate target buffer
Concatenate
ConcatenateResTemplate
ConcatenateResTemplate: failed to allocate target buffer
CondRefOf
CopyObject
CopyObject: failed because target object is not a supername
CopyObject: failed to duplicate objdata
CreateBitField
CreateByteField
CreateDWordField
CreateField
CreateNameSpaceObject: fail to allocate name space object
CreateQWordField
CreateWordField
CreateXField: failed to allocate BuffField object
D3ColdSupported
DDBHandle
DataAlias
DataField
DataObject
DbgCommandString
DbgPrintEx
DbgPrompt
DbgSetDebugFilterState
DbgkWerCaptureLiveKernelDump
DeRegisterOpRegionHandler
Decrement
DeviceInstanceId
DevicePolicy
DevicePriority
DisplayMux
DockDevice
DosDeviceName
DupObjData: failed to allocate destination buffer
EMcaL1DirectoryBase
EMcaLoggingSupport
EjectFailure
ElapsedTime
EmClientQueryRuleState
EmClientRuleEvaluate
EmProviderRegister
EmbeddedController
EtwEventEnabled
EtwRegister
EtwRegisterClassicProvider
EtwSetInformation
EtwUnregister
EtwWriteTransfer
EventData
ExAcquireFastMutex
ExAcquirePushLockExclusiveEx
ExAcquireResourceExclusiveLite
ExAcquireResourceSharedLite
ExAcquireRundownProtection
ExAcquireSpinLockExclusive
ExAcquireSpinLockShared
ExAllocateFromNPagedLookasideList
ExAllocatePool2
ExCreateCallback
ExDeleteNPagedLookasideList
ExFreePool2
ExFreePoolWithTag
ExFreeToNPagedLookasideList
ExInitializeNPagedLookasideList
ExInitializeResourceLite
ExInitializeRundownProtection
ExInterlockedRemoveHeadList
ExNotifyCallback
ExQueryWnfStateData
ExQueueWorkItem
ExReInitializeRundownProtection
ExRegisterCallback
ExReleaseFastMutex
ExReleasePushLockEx
ExReleaseResourceLite
ExReleaseRundownProtection
ExReleaseSpinLockExclusive
ExReleaseSpinLockShared
ExRundownCompleted
ExSetTimer
ExSubscribeWnfStateChange
ExTryQueueWorkItem
ExUnregisterCallback
ExUnsubscribeWnfStateChange
ExWaitForRundownProtectionRelease
Failed address translation
Failed to acquire global lock
Failed to allocate memory
Failed to register event handler
FailureCountSinceLastSent
Fatal error
Field: failed to allocate Field object
FieldUnit
FileDescription
FileVersion
FindSetLeftBit
FindSetRightBit
... y 417 más
```

## `stornvme.sys`
- **Total Strings Legibles:** 1380
- **Cadenas de Interés (Rutinas/IOCTLs/Errores):** 306

### Muestra de ADN Oculto:
```text
AERLimitTooSmall
AERToAllocate
APSTASupport
ActiveNSCount
ActiveNSCountInNSIDList
ActiveNSIDListStatus
Admin Cmd Error Handle
Admin Command Error
Admin Queue Initialize failed
AdminCmdsBeingProcessedCount
AdminQueueDepth
AllocResQCount
AllocStreamCount
Allocate IO queues failed
AllocatedReservedQueueCount
ArbitrationBurst
AsyncEventMask
AvailableSpare
AvailableSpareThreshold
BufferLength
BufferOffset
BufferSize
Bus specific reset failed
BusSpecificResetFailureCount
BypassSgl
Chatham2H
CmdOpCode
CompanyName
CompletionEntrySize
Context Resource Fail
ContiguousMemoryFromAnyNode
Controller Error
Controller Get-Feature Namespace Metadata Failed
Controller Panic Reset failed
Controller Reset failed
Controller Reset failed due to surprise remove
Controller Set-Feature Host Identifier Failed
Controller enable failed
Controller initialize part1 failed
Controller reset failed
ControllerBasicInit
ControllerConfiguration
ControllerErrorState
ControllerFlags
ControllerMaxTransferSize
ControllerNumber
ControllerReadyTimeout
ControllerResetFailureCount
ControllerResetWaitTimeCushion
ControllerState
ControllerStates
ControllerStatus
CreateStatus
CriticalWarning
CtrlInfoNOPS
CtrlInfoSet1
CtrlInfoSet2
CtrlInfoSet3
DataInconsistent
DeallocateMaxLbaCount
DeleteStatus
DevPoll_Count
DevPoll_Hints
DevPoll_Size
DevPoll_Ver
DeviceSVID
DeviceVID
DiagnosticFlags
Directive Recv Fail
Directive Send Fail
Directive Send/Receive Error
Disable HostMemoryBuffer failed
DisableActivateFWWithoutReset
DisableBypassIO
DisableDSTThrottle
DisableDeallocate
DisableF0TimestampSync
DisableForwardedIO
DisableGetActiveNSIDList
DisableMFNDCCDuringRemoval
DisableNamespacePreferredValueCheck
DmaBuffer Fail Count
DmaBuffer Failed
DumpPreInitialize
DumpUninitialize
EnableIntelTSESplitIOWorkaround
EnableSingleDpcForIoCompletion
End2EndProtection
EnduranceGroupId
EnduranceWarningLogged
EnforceActiveNamespaceIdentification
ErrorEtwThrottleInterval
ErrorState
EventInfo
EventType
FLR reset failed
FLRFailureCount
FWActivate
Failed to Allocate Async Event Commands
Failed to allocate AER for MFND
FailureReason
FailureReasonUnknown
FileDescription
FileVersion
Firmware Ioctl
ForceCryptoEraseToUseFormatNVM
ForcedPhysicalSectorSizeInBytes
Format NVM Error
Get Sanitize Log Page Failed
Get interrupt message information failed
Get processor group information failed
Get processor information failed
HMBSupport
HostIdentifier
HostMemoryBufferBytes
IO Completion Queue deletion failed
IO Submission Queue deletion failed
IO queues async creation failed
IO queues sync creation failed
IOQ-Async Creation Failed
Identify controller failed
IdentifyNameSpace
IdentityInformation
IdlePowerMode
IgnoreNamespacePreferredValues
Information
Initialize IO queues failed
Initialize perf options failed
Initialize reserved IO queues failed
InsufficientAdminQueueDepth
InsufficientDMABuffer
InsufficientNonPagedPool
InterlockedFlags
InternalName
InterruptCoalescingEntry
InterruptCoalescingTime
InterruptMasked
InterruptMode
IoCompletionCapInDPC
IoCompletionQueueCount
IoPollingInterval
IoPollingSize
IoQueueCountInPollingMode
IoQueueDepth
IoQueuePercentageInPollingMode
IoRecord.OtherErrorCount
IoStripeAlignment
IoSubmissionQueueCount
IsAdminCmd
Issue Async Cmd Fail Count
... y 156 más
```

## `usbxhci.sys`
- **Total Strings Legibles:** 4478
- **Cadenas de Interés (Rutinas/IOCTLs/Errores):** 574

### Muestra de ADN Oculto:
```text
                    %nFailure Parameter 3: %19
                    %nFailure Parameter 4: %20
                    %nFailure Reason: %17
AcpiDeviceID
AcpiRevisionID
AcpiVendorID
After endpoint reset, Set Dequeue Pointer command failed.
Allocate(sendCommandTrbToRingIn) failed
Allocation for LocalUsbDeviceHandleArray failed
AllocationCount
AlternateSetting
An xHCI controller command failed to complete after the command timeout abort.
AssertWER
AssertWERWithArgs
AssertWithArgs
BIOS Handoff failed. Check with your computer manufacturer for an updated BIOS, or an updated firmware for the controller.
Babble Detected Error
Bandwidth Error
Bandwidth Overrun Error
COMMAND_DATA
COMMON_BUFFER_DATA
COMMON_BUFFER_NONSECURE_DMA_PAGE
COMMON_BUFFER_SECURE_DMA_PAGE
COMPLETED_IN_TRUSTLET
CONTROLLER_DATA
CachedHCCParams1
CachedHCCParams2
CachedHCSParams1
CachedHCSParams2
CachedHCSParams3
CommandAbortInProgress
CommandRingFull
CommandWaitlistReason
CommandsSerialized
Common buffer allocation failure at DISPATCH LEVEL
Common buffer allocation failure for large buffer (only asserting at the smallest allocation size failure)
CompanyName
ComplexData
ConfigurationValue
Configure Endpoints command failed when only disabling endpoints
Context State Error
ContextStateErrorCount
ContextStateErrorCountTotal
Controller Restore State failed to complete
Controller failed a SetAddress command with BSR1.
Controller failed a USB device reset command.
Controller failed a disable slot command.
Controller failed an endpoints configure command where the endpoints were being deconfigured.
Controller failed to enable a slot during a usbdevice reset.
Controller reported Host Controller Error
Controller reported Host System Error
Controller restore state operation failed
Controller save state operation failed
ControllerCommand
ControllerResetInProgress
ControllerSuspendResumeCount
D0 Exit - Exit to D3Final due to failure to connect interrupts for internal XHCI
D0Entry failed for an internal xHCI controller.
D0Exit for xHCI failed.
DEVICESLOT_DATA
DEVICE_DATA
DMAModeInVSM
DMA_ENABLER_DATA
DYNAMIC_LOCK_DATA
Data Buffer Error
DbgPrintEx
DbgkWerCaptureLiveKernelDump
DeviceDescLength
DeviceDescription
DeviceFlags_0
DeviceFlags_1
DeviceSpeed
Disable Slot Command failed
DisableHCS0Idle
DpcRequeueCount
DriverEntry failed 0x%x for driver %wZ
Dropping and adding the same endpoint (as 1 command) failed.
ENDPOINT_DATA
ERROR
Endpoint Not Enabled Error
Endpoint Reset Command failed
Enumeration
Error
ErrorPortNumber
EtwActivityIdControl
EtwEventEnabled
EtwRegister
EtwRegisterClassicProvider
EtwSetInformation
EtwUnregister
EtwWriteTransfer
Event Lost Error
Event Ring Full Error
EventData
EventRingFullCount
EventType
EvtDeviceD0EntryPostInterruptsEnabled for xHCI failed.
EvtDeviceFilterRemoveResourceRequirements for xHCI failed.
ExAllocatePool2
ExAllocatePoolWithTag
ExAllocateTimer
ExDeleteTimer
ExFreePoolWithTag
ExSetTimer
ExSystemTimeToLocalTime
FILE_OBJECT_DATA
Failed Transfer Count
Failed to clear port status changes.
FailureReason
FileDescription
FileVersion
FirmwareHashFromDevice
FirmwareHashFromSDEVEntry
FirmwareVersion
For an Isoch Endpoint, a Stop Endpoint Command failed with Context State Error and Endpoint state is Halted.
FreeCount
FullDataBusTrace
FxGetNextClassBindInfo failed
FxGetNextObjectContextTypeInfo failed
Gt9GpuwL95
HCRecoveryCount
HCRestoreStateFailureCount
HWVerifyDevice
HWVerifyHost
HWVerifyHub
HW_COMPLIANCE: Port %2d Resume failed to complete before timeout
HeadersBusTrace
HealthCheckEventCountTotal
HealthCheckEventV4
INFORMATION
INTELPPT_FILTER_DATA
INTERRUPTER_DATA
IOCONTROL_DATA
IOCTL succeeded but CommandAbortRing failed in VTL-1 failed
IOCTL succeeded but CommandAddCommandTRBToRing failed in VTL-1 failed
IOCTL succeeded but CommandAdvanceDequeuePointer failed in VTL-1 failed
IOCTL succeeded but CommandAllocateResources failed in VTL-1 failed
IOCTL succeeded but CommandCreate failed in VTL-1 failed
IOCTL succeeded but CommandFreeResources failed in VTL-1 failed
IOCTL succeeded but CommandQueryIsRingRunning failed in VTL-1 failed
IOCTL succeeded but DeviceSlotAllocateResources failed in VTL-1 failed
IOCTL succeeded but DeviceSlotClearDeviceContext failed in VTL-1 failed
IOCTL succeeded but DeviceSlotCreate failed in VTL-1 failed
IOCTL succeeded but DeviceSlotFreeResources failed in VTL-1 failed
IOCTL succeeded but DeviceSlotInitialize failed in VTL-1 failed
IOCTL succeeded but DeviceSlotInitializeScratchpadBuffers failed in VTL-1 failed
IOCTL succeeded but DeviceSlotQueryInfoFromEndpointContext failed in VTL-1 failed
IOCTL succeeded but DeviceSlotQueryInfoFromSlotContext failed in VTL-1 failed
IOCTL succeeded but DeviceSlotSetDeviceContext failed in VTL-1 failed
IOCTL succeeded but EndpointCreate failed in VTL-1 failed
... y 424 más
```

## `kbdclass.sys`
- **Total Strings Legibles:** 448
- **Cadenas de Interés (Rutinas/IOCTLs/Errores):** 107

### Muestra de ADN Oculto:
```text
AllowDisable
BaseClassName
CompanyName
Configuration
ConnectMultiplePorts
ConnectOneClassToOnePort
DeniedCreateForReadWithSFAC
DesiredAccess
EtwRegister
EtwRegisterClassicProvider
EtwSetInformation
EtwUnregister
EtwWriteTransfer
ExAcquireFastMutex
ExAllocatePool2
ExFreePoolWithTag
ExReleaseFastMutex
FileDescription
FileVersion
InternalName
IoAcquireRemoveLockEx
IoAllocateErrorLogEntry
IoAllocateIrp
IoAllocateWorkItem
IoAttachDeviceToDeviceStack
IoBuildDeviceIoControlRequest
IoCancelIrp
IoCreateDevice
IoDeleteDevice
IoDetachDevice
IoFreeIrp
IoFreeWorkItem
IoGetDeviceObjectPointer
IoGetDeviceProperty
IoInitializeRemoveLockEx
IoOpenDeviceRegistryKey
IoOpenDriverRegistryKey
IoQueueWorkItem
IoRegisterDeviceInterface
IoRegisterDriverReinitialization
IoRegisterPlugPlayNotification
IoReleaseCancelSpinLock
IoReleaseRemoveLockAndWaitEx
IoReleaseRemoveLockEx
IoSetDeviceInterfaceState
IoUnregisterPlugPlayNotification
IoWMIRegistrationControl
IoWriteErrorLogEntry
IofCallDriver
IofCompleteRequest
KeAcquireSpinLockAtDpcLevel
KeAcquireSpinLockRaiseToDpc
KeGetCurrentIrql
KeInitializeEvent
KeInitializeSpinLock
KeReleaseSpinLock
KeReleaseSpinLockFromDpcLevel
KeSetEvent
KeWaitForSingleObject
KeyboardClass
KeyboardDataQueueSize
KeyboardDeviceBaseName
LegalCopyright
MaximumPortsServiced
Microsoft
MmGetSystemRoutineAddress
ObfDereferenceObject
OriginalFilename
PoCallDriver
PoRequestPowerIrp
PoSetPowerState
PoStartNextPowerIrp
ProductName
ProductVersion
PsGetVersion
RtlAppendUnicodeToString
RtlCopyUnicodeString
RtlFreeUnicodeString
RtlInitUnicodeString
RtlQueryRegistryValues
RtlQueryRegistryValuesEx
RtlVerifyVersionInfo
RtlWriteRegistryValue
SeSinglePrivilegeCheck
SendOutputToAllPorts
StringFileInfo
The driver could not obtain resources required to create a propper WaitWake IRP.
The driver for device %1 encountered an internal driver error.
Translation
UVWATAUAVAWH
VS_VERSION_INFO
VWATAVAWH
VarFileInfo
VerSetConditionMask
WATAUAVAWH
WaitWakeEnabled
Washington1
WmiCompleteRequest
WmiQueryTraceInformation
WmiSystemControl
WmiTraceMessage
WppAutoLogStart
WppAutoLogStop
WppAutoLogTrace
ZwPowerInformation
ZwQueryValueKey
ZwSetValueKey
```

## `hdaudbus.sys`
- **Total Strings Legibles:** 688
- **Cadenas de Interés (Rutinas/IOCTLs/Errores):** 156

### Muestra de ADN Oculto:
```text
BUS_ENUMERATE_WORK_ITEM_CONTEXT
BUS_SGPC_TIMER_CONTEXT
ChildDeviceExtension
CodecTimeoutOverrideMsec
CompanyName
DRIVER_CONTEXT
DbgPrintEx
DeviceDescription_Default
DeviceLocation
DriverEntry failed 0x%x for driver %wZ
EnableD3Cold
EnableNoWakeIdleTimeout
EnableS0Standby
EnumWorkaround
EtwRegister
EtwRegisterClassicProvider
EtwSetInformation
EtwUnregister
EtwWriteTransfer
ExAllocatePool2
ExAllocatePoolWithTag
ExFreePoolWithTag
ExpInterlockedFlushSList
ExpInterlockedPopEntrySList
ExpInterlockedPushEntrySList
FileDescription
FileVersion
FirstEntrySList
FxGetNextClassBindInfo failed
FxGetNextObjectContextTypeInfo failed
GfxSharedCodecAddress
GraphicsPowerEnabled
GraphicsPowerInterfaceArrival
GraphicsPowerInterfaceRemoval
HDAUDIOMOFNAME
HDAudioBus
HDAudioFlags
HDAudioMofName
InitializeSListHead
InternalName
IoAcquireRemoveLockEx
IoAllocateMdl
IoBuildSynchronousFsdRequest
IoConnectInterruptEx
IoDisconnectInterruptEx
IoFreeMdl
IoGetAttachedDeviceReference
IoGetDevicePropertyData
IoGetDmaAdapter
IoInitializeRemoveLockEx
IoInvalidateDeviceRelations
IoRegisterPlugPlayNotification
IoReleaseRemoveLockAndWaitEx
IoReleaseRemoveLockEx
IoSetDevicePropertyData
IoUnregisterPlugPlayNotification
IoWMIRegistrationControl
IoWMIWriteEvent
IofCallDriver
IofCompleteRequest
KeAcquireSpinLockAtDpcLevel
KeAcquireSpinLockRaiseToDpc
KeCancelTimer
KeClearEvent
KeDelayExecutionThread
KeGetCurrentIrql
KeInitializeDpc
KeInitializeEvent
KeInitializeMutex
KeInitializeTimer
KeInsertQueueDpc
KeLowerIrql
KeQueryPerformanceCounter
KeQueryTimeIncrement
KeReleaseMutex
KeReleaseSpinLock
KeReleaseSpinLockFromDpcLevel
KeRemoveQueueDpc
KeSetEvent
KeSetImportanceDpc
KeSetTimer
KeStallExecutionProcessor
KeWaitForSingleObject
KfRaiseIrql
KmdfLibrary
KseQueryDeviceFlags
LegalCopyright
MemoryMapSizeOverride
Microsoft
MmAllocateContiguousNodeMemory
MmAllocatePagesForMdl
MmAllocatePagesForMdlEx
MmBuildMdlForNonPagedPool
MmFreeContiguousMemorySpecifyCache
MmFreePagesFromMdlEx
MmGetPhysicalAddress
MmGetSystemRoutineAddress
MmMapIoSpaceEx
MmMapLockedPagesSpecifyCache
MmUnmapIoSpace
MmUnmapLockedPages
ObfDereferenceObject
ObfDereferenceObjectWithTag
ObfReferenceObject
ObfReferenceObjectWithTag
OriginalFilename
PartA_PrivTags
PcAddStreamResource
PcRemoveStreamResource
PoRegisterPowerSettingCallback
PoUnregisterPowerSettingCallback
PowerSettings
ProductName
ProductVersion
PsGetVersion
RtlArmFeatureUsageProviderFlushNotification
RtlCopyUnicodeString
RtlInitUnicodeString
RtlNotifyFeatureUsage
RtlQueryFeatureConfiguration
RtlQueryFeatureConfigurationChangeStamp
RtlRecordFeatureUsage
RtlRegisterFeatureConfigurationChangeNotification
RtlRegisterFeatureUsageProvider
RtlUnregisterFeatureConfigurationChangeNotification
RtlUnregisterFeatureUsageProvider
SUVWATAUAVAWH
SUVWATAVAWH
StringFileInfo
Translation
UATAUAVAWH
USVWATAUAVAWH
UVWATAUAVAWH
UWATAUAVH
UWATAVAWH
UWAUAVAWH
UseHalDMA
VATAUAVAWH
VS_VERSION_INFO
VWATAVAWH
VWAUAVAWH
VarFileInfo
WATAUAVAWH
WdfLdrQueryInterface
WdfVersionBind
WdfVersionBindClass
WdfVersionUnbind
WdfVersionUnbindClass
WmiCompleteRequest
WmiQueryTraceInformation
... y 6 más
```

