# El Mapa Total de Subsistemas de Windows 11

Este documento cubre los últimos eslabones perdidos para crear un SO BMO completo: Energía, USB, Teclado/Mouse (Con enfoques en Inglés/Español) y Audio.

## 1. Gestión de Energía y UEFI (ACPI)

### `acpi.sys`
- **Dependencias (Imports):** 0
- **Funciones Ofrecidas (Exports):** 0

### `Wdf01000.sys`
- **Dependencias (Imports):** 7
- **Funciones Ofrecidas (Exports):** 0

### `acpi.sys`
- **Dependencias (Imports):** 0
- **Funciones Ofrecidas (Exports):** 0

---

## 2. Almacenamiento NVMe Bare Metal

### `stornvme.sys`
- **Dependencias (Imports):** 2
- **Funciones Ofrecidas (Exports):** 2

```c
void DumpPreInitialize();
void DumpUninitialize();
```

### `storport.sys`
- **Dependencias (Imports):** 16
- **Funciones Ofrecidas (Exports):** 0

### `stornvme.sys`
- **Dependencias (Imports):** 2
- **Funciones Ofrecidas (Exports):** 2

```c
void DumpPreInitialize();
void DumpUninitialize();
```

---

## 3. Stack USB Universal

### `Ucx01000.sys`
- **Dependencias (Imports):** 0
- **Funciones Ofrecidas (Exports):** 0

### `usbd.sys`
- **Dependencias (Imports):** 1
- **Funciones Ofrecidas (Exports):** 22

```c
void DllInitialize();
void DllUnload();
void USBD_AddDeviceToGlobalList();
void USBD_AllocateHubNumber();
void USBD_CalculateUsbBandwidth();
void USBD_CreateConfigurationRequest();
void USBD_CreateConfigurationRequestEx();
void USBD_GetInterfaceLength();
void USBD_GetPdoRegistryParameter();
void USBD_GetRegistryKeyValue();
void USBD_GetUSBDIVersion();
void USBD_MarkDeviceAsDisconnected();
void USBD_ParseConfigurationDescriptor();
void USBD_ParseConfigurationDescriptorEx();
void USBD_ParseConfigurationDescriptorEx2();
void USBD_ParseDescriptors();
void USBD_ParseDescriptors2();
void USBD_QueryBusTime();
void USBD_RegisterHcFilter();
void USBD_ReleaseHubNumber();
void USBD_RemoveDeviceFromGlobalList();
void USBD_ValidateConfigurationDescriptor();
```

### `USBHUB3.SYS`
- **Dependencias (Imports):** 5
- **Funciones Ofrecidas (Exports):** 1

```c
void Microsoft_USBD_Compat_Version();
```

### `USBXHCI.SYS`
- **Dependencias (Imports):** 4
- **Funciones Ofrecidas (Exports):** 0

### `USBHUB3.SYS`
- **Dependencias (Imports):** 5
- **Funciones Ofrecidas (Exports):** 1

```c
void Microsoft_USBD_Compat_Version();
```

### `usbd.sys`
- **Dependencias (Imports):** 1
- **Funciones Ofrecidas (Exports):** 22

```c
void DllInitialize();
void DllUnload();
void USBD_AddDeviceToGlobalList();
void USBD_AllocateHubNumber();
void USBD_CalculateUsbBandwidth();
void USBD_CreateConfigurationRequest();
void USBD_CreateConfigurationRequestEx();
void USBD_GetInterfaceLength();
void USBD_GetPdoRegistryParameter();
void USBD_GetRegistryKeyValue();
void USBD_GetUSBDIVersion();
void USBD_MarkDeviceAsDisconnected();
void USBD_ParseConfigurationDescriptor();
void USBD_ParseConfigurationDescriptorEx();
void USBD_ParseConfigurationDescriptorEx2();
void USBD_ParseDescriptors();
void USBD_ParseDescriptors2();
void USBD_QueryBusTime();
void USBD_RegisterHcFilter();
void USBD_ReleaseHubNumber();
void USBD_RemoveDeviceFromGlobalList();
void USBD_ValidateConfigurationDescriptor();
```

### `USBXHCI.SYS`
- **Dependencias (Imports):** 4
- **Funciones Ofrecidas (Exports):** 0

---

## 4. Input & HID (Teclado y Mouse)

### `hidusb.sys`
- **Dependencias (Imports):** 4
- **Funciones Ofrecidas (Exports):** 0

### `i8042prt.sys`
- **Dependencias (Imports):** 4
- **Funciones Ofrecidas (Exports):** 0

### `kbdclass.sys`
- **Dependencias (Imports):** 3
- **Funciones Ofrecidas (Exports):** 0

### `kbdhid.sys`
- **Dependencias (Imports):** 4
- **Funciones Ofrecidas (Exports):** 0

### `mouclass.sys`
- **Dependencias (Imports):** 3
- **Funciones Ofrecidas (Exports):** 0

### `mouhid.sys`
- **Dependencias (Imports):** 4
- **Funciones Ofrecidas (Exports):** 0

### `hidusb.sys`
- **Dependencias (Imports):** 4
- **Funciones Ofrecidas (Exports):** 0

### `i8042prt.sys`
- **Dependencias (Imports):** 4
- **Funciones Ofrecidas (Exports):** 0

### `kbdclass.sys`
- **Dependencias (Imports):** 3
- **Funciones Ofrecidas (Exports):** 0

### `kbdhid.sys`
- **Dependencias (Imports):** 4
- **Funciones Ofrecidas (Exports):** 0

### `mouclass.sys`
- **Dependencias (Imports):** 3
- **Funciones Ofrecidas (Exports):** 0

### `mouhid.sys`
- **Dependencias (Imports):** 4
- **Funciones Ofrecidas (Exports):** 0

---

## 5. Idiomas de Teclado (US / ES)

### `KBDLA.DLL`
- **Dependencias (Imports):** 0
- **Funciones Ofrecidas (Exports):** 1

```c
void KbdLayerDescriptor();
```

### `KBDSP.DLL`
- **Dependencias (Imports):** 0
- **Funciones Ofrecidas (Exports):** 1

```c
void KbdLayerDescriptor();
```

### `KBDUS.DLL`
- **Dependencias (Imports):** 0
- **Funciones Ofrecidas (Exports):** 1

```c
void KbdLayerDescriptor();
```

---

## 6. Audio Bare Metal

### `hdaudbus.sys`
- **Dependencias (Imports):** 6
- **Funciones Ofrecidas (Exports):** 0

### `portcls.sys`
- **Dependencias (Imports):** 6
- **Funciones Ofrecidas (Exports):** 40

```c
void DllInitialize();
void DllUnload();
void PcAddAdapterDevice();
void PcAddContentHandlers();
void PcAddStreamResource();
void PcAssignPowerFrameworkSettings();
void PcCompleteIrp();
void PcCompletePendingPropertyRequest();
void PcCreateContentMixed();
void PcDestroyContent();
void PcDispatchIrp();
void PcForwardContentToDeviceObject();
void PcForwardContentToFileObject();
void PcForwardContentToInterface();
void PcForwardIrpSynchronous();
void PcGetContentRights();
void PcGetDeviceProperty();
void PcGetPhysicalDeviceObject();
void PcGetTimeInterval();
void PcInitializeAdapterDriver();
void PcNewDmaChannel();
void PcNewInterruptSync();
void PcNewMiniport();
void PcNewPort();
void PcNewRegistryKey();
void PcNewResourceList();
void PcNewResourceSublist();
void PcNewServiceGroup();
void PcRegisterAdapterPnpManagement();
void PcRegisterAdapterPowerManagement();
void PcRegisterIoTimeout();
void PcRegisterPhysicalConnection();
void PcRegisterPhysicalConnectionFromExternal();
void PcRegisterPhysicalConnectionToExternal();
void PcRegisterSubdevice();
void PcRemoveStreamResource();
void PcRequestNewPowerState();
void PcUnregisterAdapterPnpManagement();
void PcUnregisterAdapterPowerManagement();
void PcUnregisterIoTimeout();
```

### `hdaudbus.sys`
- **Dependencias (Imports):** 6
- **Funciones Ofrecidas (Exports):** 0

### `portcls.sys`
- **Dependencias (Imports):** 6
- **Funciones Ofrecidas (Exports):** 40

```c
void DllInitialize();
void DllUnload();
void PcAddAdapterDevice();
void PcAddContentHandlers();
void PcAddStreamResource();
void PcAssignPowerFrameworkSettings();
void PcCompleteIrp();
void PcCompletePendingPropertyRequest();
void PcCreateContentMixed();
void PcDestroyContent();
void PcDispatchIrp();
void PcForwardContentToDeviceObject();
void PcForwardContentToFileObject();
void PcForwardContentToInterface();
void PcForwardIrpSynchronous();
void PcGetContentRights();
void PcGetDeviceProperty();
void PcGetPhysicalDeviceObject();
void PcGetTimeInterval();
void PcInitializeAdapterDriver();
void PcNewDmaChannel();
void PcNewInterruptSync();
void PcNewMiniport();
void PcNewPort();
void PcNewRegistryKey();
void PcNewResourceList();
void PcNewResourceSublist();
void PcNewServiceGroup();
void PcRegisterAdapterPnpManagement();
void PcRegisterAdapterPowerManagement();
void PcRegisterIoTimeout();
void PcRegisterPhysicalConnection();
void PcRegisterPhysicalConnectionFromExternal();
void PcRegisterPhysicalConnectionToExternal();
void PcRegisterSubdevice();
void PcRemoveStreamResource();
void PcRequestNewPowerState();
void PcUnregisterAdapterPnpManagement();
void PcUnregisterAdapterPowerManagement();
void PcUnregisterIoTimeout();
```

---

