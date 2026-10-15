# How Windows picks the GPU an app renders on, and how a third-party app can change it

Research date: 2026-10-14. Machine: Windows 11 Pro build 26300 (Insider), Ryzen 9 7900X3D (iGPU "AMD Radeon(TM) Graphics", DEV_164E), RX 7800 XT (DEV_747E), IddSampleDriver virtual display. A Radeon AI PRO R9700 (32 GB) will be added later.

Evidence tags used below:

- **[DOC]**: Microsoft Learn, Khronos, NVIDIA or Tauri documentation.
- **[SRC]**: open source code, cited with file and line.
- **[LOCAL]**: measured read-only on this PC. The probe scripts are in `research/probes/`.
- **[BIN]**: strings found inside Windows or driver binaries on this PC. They show that a name exists; they do not show how the binary behaves.
- **[COMMUNITY]**: forum posts or third-party open source tools.
- **[INFERRED]**: my own conclusion, not verified.

Nothing was written to the registry and no settings were changed.

---

## 0. Short answer

1. **Apps that take "adapter 0" now land on the iGPU because the primary desktop monitor is on the iGPU.** DXGI returns, as adapter 0, the adapter with the output that shows the desktop primary [DOC]. On this PC that output is `\\.\DISPLAY6` (2560x1440 at 0,0), and it is attached to the iGPU (motherboard HDMI) [LOCAL]. D3D11 and D3D12 with a null adapter, ANGLE/Chromium/Edge/Electron/WebView2, and the Vulkan loader's sorting all follow this order (§3, §4).
2. **"High performance" also resolves to the iGPU on this PC.** `EnumAdapterByGpuPreference(HIGH_PERFORMANCE)` and `(MINIMUM_POWER)` both return the iGPU first [LOCAL]. This happens because Windows does not consider the iGPU integrated:
   - DXCore reports `IsIntegrated=False` for both GPUs [LOCAL].
   - Neither adapter carries the `HybridDiscrete` or `HybridIntegrated` kernel flag [LOCAL].
   - The WDDM "hybrid system" model needs a POST iGPU with a built-in panel and a dGPU with no display outputs [DOC]. This desktop has neither.
3. **The 16 GB UMA carve-out makes the iGPU look like the biggest GPU.** DXGI reports 15.97 GiB `DedicatedVideoMemory` for the iGPU versus 15.80 GiB for the 7800 XT [LOCAL]. The previous DirectX database entry for the iGPU, last seen 2026-09-28, showed 0.47 GiB. Any app or engine that ranks GPUs by dedicated memory now picks the iGPU. That includes this repo's own gow2 "auto" fallback (§8). [INFERRED] Windows may also rank the "default high-performance GPU" by memory when the user hasn't chosen one. That would explain why the problem started with the BIOS change; it is untested (§9).
4. **What fixes it with zero code (by likely impact):**
   - Move the monitor from the motherboard HDMI to the 7800 XT, or make the 7800 XT monitor the main display. This changes adapter 0 [DOC + INFERRED].
   - Set **Settings > System > Display > Graphics > Default graphics settings > Default high performance GPU = RX 7800 XT**. This PC qualifies for that option (`DefaultHighPerfGPUApplicable=1`) [LOCAL]. It is stored as `HighPerfAdapter=1002&747E&2427148C` inside `DirectXUserGlobalSettings` [BIN + COMMUNITY].
   - Use per-app "Specific GPU" (`SpecificAdapter=…;GpuPreference=1073741824;`) for games [BIN + COMMUNITY].
   - Lower the UMA frame buffer back to Auto or 512 MB in the BIOS.
5. **There is no documented API to set a GPU preference.** Settings writes `HKCU\Software\Microsoft\DirectX\UserGpuPreferences` (no admin needed) through the internal `directxdatabasehelper.dll` [BIN]. A third-party app can write the same values. The format is undocumented but stable since 1803 and used by many open source tools [COMMUNITY]. The documented deep links are `ms-settings:display-advancedgraphics` and `ms-settings:display-advancedgraphics-default` [DOC]. Apps must be restarted after a change (§6).

---

## 1. Measured state of this PC [LOCAL]

### 1.1 Registry `HKCU\Software\Microsoft\DirectX`

```
GraphicsSettings
    DefaultHighPerfGPUApplicable    REG_DWORD 0x1
    SpecificGPUOptionApplicable     REG_DWORD 0x1
    AutoHDROptOutApplicable         REG_DWORD 0x1
UserGpuPreferences
    C:\Program Files (x86)\ASUS\ArmouryDevice\asus_framework.exe            GpuPreference=1;
    C:\Program Files\ASUS\Aura Wallpaper Service\Aura Wallpaper Service.exe GpuPreference=1;
    ...\Tom Clancy's Rainbow Six Siege\RainbowSix.exe                        AppStatus=0;
    C:\Users\santi\AppData\Local\ubisoft\r6s\RainbowSix.exe                  AppStatus=0;
    ...\ForzaHorizon6\forzahorizon6.exe                                      AppStatus=0;AutoHDREnable=2097;
    ...\VALORANT-Win64-Shipping.exe                                          AutoHDREnable=2097;
    ...\7DaysToDie.exe                                                       AppStatus=0;AutoHDREnable=2097;
    ...\PEAK\PEAK.exe                                                        AppStatus=0;AutoHDREnable=2097;
    DirectXUserGlobalSettings      SwapEffectUpgradeEnable=1;AutoHDREnable=1;
```

- No `HighPerfAdapter` is set, so the user has never chosen a "Default high performance GPU". No app uses "Specific GPU".
- `GraphicsSettings\*Applicable` are flags that Settings reads to decide whether to show the options. The names appear in `SettingsHandlers_Gpu.dll` [BIN]. `SpecificGPUOptionApplicable=1` and `DefaultHighPerfGPUApplicable=1` mean both options are available on this PC.
- The key ACL gives the current user and SYSTEM FullControl, and "ALL APPLICATION PACKAGES" plus one capability SID ReadKey [LOCAL `Get-Acl`]. So an unpackaged process writes it without elevation, and packaged (AppContainer) apps can only read it.

### 1.2 DXGI enumeration from an exe with no preference (python.exe) — `probes/dxgi_enum.py`

| Order | EnumAdapters1 / UNSPECIFIED | MINIMUM_POWER | HIGH_PERFORMANCE |
|---|---|---|---|
| 0 | **iGPU** 164E, 15.97 GiB, LUID 0x1912e, output DISPLAY6 (0,0)-(2560,1440) = primary | **iGPU** | **iGPU** |
| 1 | 7800 XT, 15.80 GiB, LUID 0x16c9b, output DISPLAY1 (-1080,-356) portrait | 7800 XT | 7800 XT |
| 2 | "AMD Radeon RX 7800 XT", LUID 0x2764f, no outputs (this is the IddSampleDriver virtual display, see 1.4) | Basic Render | Basic Render |
| 3 | Microsoft Basic Render Driver | LUID 0x2764f | LUID 0x2764f |

### 1.3 DXCore (D3D12_GRAPHICS adapter list) — `probes/dxcore_enum.py`

| Order | No sort | Sort(Hardware, HighPerformance) | Sort(Hardware, MinimumPower) |
|---|---|---|---|
| 0 | 7800 XT, `IsIntegrated=False` | 7800 XT | 7800 XT |
| 1 | iGPU, **`IsIntegrated=False`** | iGPU | iGPU |
| 2 | Basic Render | Basic Render | Basic Render |

DXCore's order differs from DXGI's and does not depend on the monitor. DXCore also leaves out the IDD adapter. Neither sort reorders anything, because Windows does not see an integrated GPU.

### 1.4 Kernel adapter type (`D3DKMTQueryAdapterInfo(KMTQAITYPE_ADAPTERTYPE)`) — `probes/kmt_types.py`

| LUID | Value | Flags |
|---|---|---|
| 0x16c9b (7800 XT) | 0x230b | Render, Display, **PostDevice**, ACG, SetTimingsFromVidPn, RuntimePM |
| 0x1912e (iGPU) | 0x2303 | Render, Display, ACG, SetTimingsFromVidPn, RuntimePM |
| 0x2764f | 0x342 | Display, **IndirectDisplayDevice**, ACG, SetTimingsFromVidPn (no Render) = IddSampleDriver |
| 0x190bd | 0x105 | Render, SoftwareDevice, ACG (Basic Render) |

- The bit layout comes from `D3DKMT_ADAPTERTYPE` [DOC].
- `HKLM\SOFTWARE\Microsoft\DirectX\{adapter-guid}\AdapterType` stores the same value [LOCAL].
- **No adapter is HybridDiscrete or HybridIntegrated.** The 7800 XT is the POST (boot) adapter.

### 1.5 Memory reported by the DirectX database `HKLM\SOFTWARE\Microsoft\DirectX\{guid}`

- iGPU now: `DedicatedVideoMemory=0x3fe5c7000` (15.97 GiB). Old entry `{CB04D725…}`: `0x1e5c7000` (0.47 GiB), LastSeen 2026-09-28 13:12Z.
- 7800 XT: `0x3f314e000` (15.80 GiB).
- `HKLM\SOFTWARE\Microsoft\DirectX\MaxDedicatedVideoMemory = 0x3fe5c7000`, which is the iGPU's value.

### 1.6 Vulkan (`vulkaninfo --summary`)

- GPU0 is the iGPU (`PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU`, api 1.4.315). GPU1 is the 7800 XT (`DISCRETE_GPU`, api 1.4.349). This matches the DXGI default order (§4.3).
- AMD registers the implicit layer `VK_LAYER_AMD_switchable_graphics` from both driver packages. Its manifest is `amd-vulkan64.json` with `disable_environment: DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1=1` [LOCAL].
- The two GPUs run **different AMD driver packages**:
  - iGPU: 32.0.21045.5002, `u0203303.inf`.
  - dGPU: 32.0.31041.1004, `u0203304.inf`.
- Each has its own `OpenGLDriverName` (atig6pxx.dll) in its display class key `{4d36e968…}\0000|0001` [LOCAL].

---

## 2. Storage format

### 2.1 Per-app values: `HKCU\Software\Microsoft\DirectX\UserGpuPreferences`

- **Value name:**
  - Desktop app: the full path of the exe, e.g. `C:\Riot Games\...\VALORANT-Win64-Shipping.exe` [LOCAL].
  - Packaged app: the AUMID, `PackageFamilyName!AppId`, e.g. `Microsoft.Windows.Photos_8wekyb3d8bbwe!App` [COMMUNITY: GPUAssign `Services/GpuPreferenceService.cs:198-204`]. Supporting evidence: `SettingsHandlers_Gpu.dll` embeds a list of system AUMIDs such as `Microsoft.DesktopAppInstaller_8wekyb3d8bbwe!App` [BIN], and AMD's `atieclxx.exe` calls `GetWindowsAppUserModelID` next to this key [BIN]. [INFERRED]
- **Value data:** a REG_SZ of `Name=Value;` tokens, keeping any tokens you don't recognise.
- **Token names recognised by Windows** (from the string table of `C:\Windows\System32\directxdatabasehelper.dll`; source path `onecore\windows\directx\database\helperlibrary\lib\perappusersettingsqueryimpl.cpp`) [BIN]: `GpuPreference`, `SpecificAdapter`, `HighPerfAdapter`, `VRROptimizeEnable`, `VRREligibleOverride`, `AutoHDREnable`, `AutoHDRStrength`, `SwapEffectUpgradeEnable`, `Force11on12`, `DXGIEffects`, `DynamicSwitch`, `OpenGLBehavior`, `OpenCLBehavior`, `VulkanBehavior`, `ModelPackagePreference`, `SingleAdapterHybridMode`, `AppStatus`, `AutoSRConfig`, `GameMetadata`.
- **`GpuPreference` values:**
  - `0` = Let Windows decide
  - `1` = Power saving (`DXGI_GPU_PREFERENCE_MINIMUM_POWER`)
  - `2` = High performance (`DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE`)

  The 0/1/2 meanings come from [COMMUNITY]. They match the `DXGI_GPU_PREFERENCE` enum order [DOC] and AMD's own driver strings `GpuPreference=1;` / `GpuPreference=2;` [BIN].
- **"Specific GPU"** (Windows 11 24H2+; the per-app option was first announced in Insider build 20190 [DOC]) is stored as `SpecificAdapter=VVVV&DDDD&SSSSSSSS;GpuPreference=1073741824;`. That is PCI vendor & device & subsystem in hex, plus `GpuPreference=0x40000000`. Evidence:
  - [COMMUNITY] Several independent tools agree: kronis.dev; `woogi999/pious-bootstrapper src-tauri/src/core/gpupref.rs:1-6`; `MetoisTaken/QuickGPUSelectionTool QGST.Core/Models/GpuInfo.cs`; `levy-street/world-of-claudecraft electron/gpu_preference.cjs:43`.
  - [BIN] `directxdatabasehelper.dll` contains the format strings `%04hx` and `%08I32x`.
  - For the 7800 XT the token would be `SpecificAdapter=1002&747E&2427148C`, and for the iGPU `1002&164E&88771043`. [INFERRED] from PNPDeviceID.
- **Caveat:** VEN&DEV&SUBSYS identifies a card model, not one physical card. Two identical cards cannot be told apart this way. [INFERRED]
- The Settings page has warning strings for a missing card: `SystemSettings_AdvancedGraphics_OptionsFlyout_SpecificGpuNotFoundWarning` and `SystemSettings_DefaultGraphics_DefaultHighPerfGpu_NotFoundWarning` [BIN].
- `AppStatus=0` entries for games were not written by the user. Who writes them is unknown (§9).
- The `GpuPreference=1` entries for the two ASUS exes have no known writer either. AMD's kernel driver contains APU-steering strings next to `GpuPreference=1;` and `UserGpuPreferences` (`SSEcoAPUSteeringExcludePaths`, `SmartDCList`) [BIN]. ASUS installers are another possibility.

### 2.2 System-wide values: the special value name `DirectXUserGlobalSettings` (same key)

- Same token syntax. On this PC: `SwapEffectUpgradeEnable=1;AutoHDREnable=1;` [LOCAL].
- **Default high performance GPU** = `HighPerfAdapter=VVVV&DDDD&SSSSSSSS;`.
  - [BIN] The token is in `directxdatabasehelper.dll`.
  - [COMMUNITY] `shoober420/windows11-scripts` writes `HighPerfAdapter=10DE&2684&89321043`. ElevenForum thread "Find GPU VEN&DEV&SUBSYS values for HighPerfAdapter".
- Microsoft documents the user-facing feature:
  - Build 20190 [DOC]: "specify a default high performance GPU", and "pick a specific GPU on a per application basis".
  - Build 25336 [DOC]: "Customers with 2 or more discrete graphics adapters with no integrated graphics can now choose which of the discrete graphics adapters is considered high-performance", under Settings > System > Display > Graphics > Default graphics settings.
  - DirectX blog, 2023-01-19 [DOC]: "your preference for the specific app will override the system-wide setting preference".
- **This PC counts as "2 discrete, no integrated" to Windows** (DXCore `IsIntegrated=False`). That is why the option shows up here. [LOCAL + INFERRED]

### 2.3 How Settings writes and notifies

- `directxdatabasehelper.dll` contains `WriteUserSetting`, `WriteUserGlobalSetting`, `NotifyUserSettingChanged failed to publish WNF` and a `RtlSubscribeWnfStateChangeNotification` import in dxgi.dll. It also contains the AppContainer capability SID that matches the key ACL [BIN].
- [INFERRED] Settings publishes a WNF state change so running processes can refresh their cached settings. This is likely used by live toggles like Auto HDR. It does not move a device that already exists to another GPU (§6).
- `dxgi.dll` contains `NoUserSettingCaching` [BIN], so DXGI caches user settings per process.

---

## 3. What DXGI does

### 3.1 Default order

`IDXGIFactory::EnumAdapters`: "first returns the adapter with the output on which the desktop primary is displayed. This adapter corresponds with an index of zero. EnumAdapters next returns other adapters with outputs. EnumAdapters finally returns adapters without outputs." [DOC]

The factory takes a snapshot of adapters when it is created ("if you change the adapters in a system, you must destroy and recreate the IDXGIFactory") [DOC].

### 3.2 Preference order

`IDXGIFactory6::EnumAdapterByGpuPreference` [DOC]:

- UNSPECIFIED is equivalent to EnumAdapters1.
- MINIMUM_POWER returns iGPUs, then dGPUs, then xGPUs.
- HIGH_PERFORMANCE returns xGPUs, then dGPUs, then iGPUs.

The method has existed since Windows 10 1803. DXCore has the same idea in `DXCoreAdapterPreference` (Hardware / MinimumPower / HighPerformance) [DOC].

### 3.3 Who counts as "integrated"

From "Validating a Hybrid System Configuration" [DOC]:

- The integrated hybrid adapter is the POST adapter, supports WDDM 1.3+, and "has an integrated display panel".
- The discrete hybrid adapter "Set[s] DXGK_DRIVERCAPS.HybridDiscrete", supports cross-adapter resources and has "no display outputs". Only one is allowed.

From "Using Cross-Adapter Resources in a Hybrid System" [DOC]: "The discrete GPU is a render-only device, and no display outputs are connected to it."

This desktop's 7800 XT is the POST device and has outputs. So the kernel flags neither GPU as hybrid (§1.4), and DXCore says neither is integrated (§1.3). The iGPU/dGPU ordering rules of §3.2 therefore have nothing to work with. Measured result: HIGH_PERFORMANCE returns the iGPU first (§1.2).

[INFERRED] Two explanations fit the measurement equally well:

- (a) With no classification, DXGI falls back to the default order, where the primary-display adapter comes first.
- (b) With "2+ discrete GPUs" and no `HighPerfAdapter` chosen, Windows ranks by dedicated memory (iGPU 15.97 GiB > 7800 XT 15.80 GiB).

Hypothesis (b) would explain why the problem started right after the UMA change. The test plan is in §9.

### 3.4 How a per-app preference reaches apps that never ask

The Khronos loader documentation says: "On Windows, the loader will attempt to sort these objects so that the system preference will be listed first" [DOC]. The loader does this with `EnumAdapterByGpuPreference(i, DXGI_GPU_PREFERENCE_UNSPECIFIED, …)` [SRC `loader/loader_windows.c:856-932` @ 73cf5326]. For that sort to reflect the system preference, the UNSPECIFIED/EnumAdapters1 order must already carry the per-app user preference.

Adam Sawicki (AMD) measured on a hybrid laptop in 2018 that switching the setting changes the "order of these adapters as visible to my program, so that the appropriate one is visible as adapter 0" [COMMUNITY, asawicki.info, 2018-02-24].

`dxgi.dll` contains `APIGpuPreference`, `ActualGpuPreference`, `DXGIGpuPreferenceDecidingFactor` and `QueryFinalGPUPreferenceDecision returned unexpected hybridPreference` [BIN]. So DXGI computes a final decision from several inputs: the API request, the user setting, the IHV driver list (`QueryDListForApplication1/2`), and the exe exports `NvOptimusEnablement` / `AmdPowerXpressRequestHighPerformance`, whose names are in `directxdatabasehelper.dll` [BIN].

Conclusion [INFERRED, high]: a user preference reorders the default enumeration. An app that just takes adapter 0 gets the preferred GPU. But the preference can only be as good as what Windows thinks "high performance" means (§3.3). On this PC, "High performance" (2) can still pick the iGPU until `HighPerfAdapter` is set. "Specific GPU" avoids that problem.

### 3.5 Apps that pick a GPU themselves

The DXGI docs say: "This method allows developers to select which GPU they think is most appropriate" [DOC]. Settings itself (per third-party quotes) says apps always have the final choice. An engine that enumerates every adapter and chooses by its own rule ignores the preference:

- largest `DedicatedVideoMemory` (now the iGPU),
- by name,
- a saved adapter index or LUID (LUIDs change at every reboot),
- `-adapter`, `r.GraphicsAdapter` and similar options.

Real examples: kronis.dev reports that Delta Force and Ashen ignore the preference, and that the Unreal `-adapter` / `r.GraphicsAdapter` options did nothing for some games [COMMUNITY].

---

## 4. Which APIs honour it

| API | How the adapter is chosen | Honours the Windows preference? |
|---|---|---|
| D3D12 | `D3D12CreateDevice(NULL…)` uses "the first adapter that is enumerated by IDXGIFactory1::EnumAdapters" [DOC] | Yes, when the app passes NULL or adapter 0 |
| D3D11 / D3D10 (DXGI) | Same rule for `D3D11CreateDevice` with a NULL adapter [DOC] | Yes, same condition |
| D3D9 | `D3DADAPTER_DEFAULT`. `d3d9.dll` contains `QueryFinalGPUPreferenceDecision` [BIN] | Probably yes [INFERRED] |
| Vulkan (Khronos loader) | Sorts physical devices by `EnumAdapterByGpuPreference(UNSPECIFIED)` using adapter LUIDs. Only drivers that support loader interface v6 (`vk_icdEnumerateAdapterPhysicalDevices`) get sorted. Unsorted devices go last. "This mechanism does not force an application to use any particular GPU" [DOC `docs/LoaderDriverInterface.md:1035-1088`, SRC `loader_windows.c:856-932`] | Yes for apps that take device 0. No for apps that pick `DISCRETE_GPU`: Vulkan reports the iGPU as INTEGRATED, so those apps pick the 7800 XT. Here GPU0 is the iGPU [LOCAL] |
| Vulkan + AMD implicit layer | `VK_LAYER_AMD_switchable_graphics` is in both AMD packages and can be disabled with `DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1=1` [LOCAL] | [INFERRED] It reorders or filters devices following AMD's PowerXpress profile. Behaviour on desktop is unknown. The env var is useful when testing. |
| OpenGL (opengl32.dll + vendor ICD) | `opengl32.dll` imports `D3DKMTOpenAdapterFromHdc` / `D3DKMTOpenAdapterFromLuid` / `D3DKMTEnumAdapters2`. It references `DisplayMuxAppPreference`, `displaypreferencehelper.cpp`, `OpenGLOn12.dll` and `Microsoft.D3DMappingLayers` [BIN]. Each adapter registers its own ICD (`OpenGLDriverName`) [LOCAL]. `D3DKMT_OPENGLINFO` is the documented way to get the ICD name [DOC] | Generally no on desktops [INFERRED]: the ICD of the adapter behind the window's HDC (the monitor the window is on) renders. NVIDIA has its own "OpenGL rendering GPU" control [DOC]. The AMD keys show `OpenGLDynamicPX=1`, an AMD hybrid mechanism, behaviour unknown. The `OpenGLBehavior` / `VulkanBehavior` / `OpenCLBehavior` tokens most likely select native vs mapping-layer drivers (GLon12 / Dozen / CLon12) [INFERRED] |
| OpenCL, CUDA, HIP/ROCm, Vulkan compute (llama.cpp, Ollama) | The app or runtime enumerates devices and picks itself, with env vars like `HIP_VISIBLE_DEVICES`, `GGML_VK_VISIBLE_DEVICES`, `CUDA_VISIBLE_DEVICES` | No. Not governed by GpuPreference. Corral already handles this layer. |
| Chromium / Edge / Electron / CEF | Default GPU = `gpu_info.gpu`, which is DXGI `EnumAdapters` index 0 [SRC `gpu/config/gpu_info_collector_win.cc:345-376`, `gpu/ipc/service/gpu_init.cc:271-292`]. High-performance GPU = `EnumAdapterByGpuPreference(HIGH_PERFORMANCE)` index 0 [SRC `gpu_info_collector_win.cc:379-401`]. Compositing uses the default GPU. WebGL/WebGPU contexts that ask for high-performance use the high-perf GPU [SRC `gpu_init.cc:296-361`]. ANGLE D3D11 picks the requested LUID, otherwise the first non-Microsoft adapter from `EnumAdapters` [SRC ANGLE `Renderer11.cpp:684-756` @ b40b5946]. Switches: `--use-adapter-luid=<high>,<low>` [SRC `ui/gl/gl_switches.cc:163-165`, parsed in `ui/gl/gl_display.cc:74-90`], `--force-high-performance-gpu` [SRC `gpu/config/gpu_switches.cc:57`, workaround `gpu_workaround_list.txt:90-91`] | Yes for the per-exe preference of the process that hosts the GPU process. On this PC both "default" and "high performance" are the iGPU, so Chrome/Edge/Electron/WebGL all run there. `--force-high-performance-gpu` would also pick the iGPU here. |
| Electron | The GPU process is the app's own exe running with `--type=gpu-process` | Yes: a per-exe preference on the app exe steers all its processes [COMMUNITY `electron/gpu_preference.cjs:57-58`] |
| WebView2 (Tauri, Corral, many Win32 apps) | The GPU process is `msedgewebview2.exe` in the shared runtime folder, e.g. `C:\Program Files (x86)\Microsoft\EdgeWebView\Application\155.0.4283.45\` [LOCAL]. That path changes with each runtime version | [INFERRED] A preference on the host exe (corral.exe) does not affect WebView2's GPU process, and a preference on `msedgewebview2.exe` breaks on every runtime update. The supported workaround is extra browser arguments: `CoreWebView2EnvironmentOptions.AdditionalBrowserArguments`, env `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`, or policy registry `…\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments` per AUMID or exe name [DOC]. Pass `--use-adapter-luid=…`. In Tauri v2: `WindowConfig.additionalBrowserArgs`, which replaces wry's default `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`, so re-add those [DOC]. LUIDs change at every boot, so compute the value at launch. |

---

## 5. Monitors on different GPUs (cross-adapter scan-out)

- Rafael Cintron (Microsoft), public_webgl list, 2018-07-05: "On Windows, there is no such thing as 'cross adapter sharing'. You cannot allocate a texture on one adapter and render to it using another adapter." If an app renders on adapter A and its window is on a monitor of adapter B, "DWM will copy the output of your application from adapter A to adapter B behind your back." [DOC-ish: a Microsoft engineer on the Khronos list]
- CASO (Cross Adapter Scan-Out) cuts the hybrid path from two copies to one (+16% FPS, -27% latency). It applies only to **hybrid laptops** with Ryzen 6000+/Intel 11th gen and WDDM 3.0/3.1 [DOC, DirectX blog 2023-09-25].
- `dxgi.dll` contains strings for these paths: "Falling back from CASO to 2 copy hybrid path", "DXGI cross-adapter primary (iGPU)", "DXGI cross-adapter blt proxy" [BIN]. These are hybrid code paths and do not apply to this non-hybrid desktop. [INFERRED]
- Practical effect here [INFERRED]:
  - A game rendered on the 7800 XT and shown on the iGPU monitor (DISPLAY6, the primary) is copied every frame through system memory to the iGPU. That costs latency and bandwidth, and features like independent flip and VRR may be lost.
  - A game whose window is on DISPLAY1 (the 7800 XT monitor) and that renders on the 7800 XT avoids the copy.
  - The cleanest setup is all monitors on the 7800 XT and the iGPU used only for compute or not at all, or at least the primary monitor on the 7800 XT.
- Window placement also decides the OpenGL ICD (§4).

---

## 6. Restart, admin rights, public API

- **App restart is required.**
  - A D3D device is created on one adapter and cannot move. The factory snapshots adapters when it is created [DOC].
  - NVIDIA: "To allow the 'Preferred Graphics Processor' settings to take effect, you must restart the program." [DOC]
  - The Settings page text (quoted by third parties) says choices "may not take effect until the next time the app launches" [COMMUNITY].
  - An Electron app that writes its own preference before its GPU process starts gets it in the same launch, because DXGI reads the value lazily per process [COMMUNITY `electron/gpu_preference.cjs:49-51`, INFERRED].
- **Admin rights: not needed.** The key is HKCU with user FullControl [LOCAL ACL]. "Default high performance GPU" is in the same key. The Settings elevation flow (`SystemSettingsAdminFlows.exe`, `ShowRestartPrompt` in `SettingsHandlers_Gpu.dll` [BIN]) belongs to Hardware-accelerated GPU scheduling, an HKLM setting that needs a reboot. [INFERRED]
- **Public API to set the preference: none found.**
  - Documented and read-only: `EnumAdapterByGpuPreference`, DXCore `Sort`, and `ms-settings:display-advancedgraphics` / `ms-settings:display-advancedgraphics-default` [DOC].
  - The write functions are internal exports of `directxdatabasehelper.dll` [BIN]. No GPO or CSP was found.
  - A third-party app has to write the registry format of §2. Rules for doing that safely:
    - Read the value, change only the `GpuPreference` / `SpecificAdapter` tokens, and keep the others (AutoHDR, AppStatus, SwapEffectUpgrade…).
    - Never overwrite `DirectXUserGlobalSettings` wholesale.

---

## 7. Vendor control panels

- **AMD "Switchable Graphics"** in Radeon Settings / Adrenalin is a per-app choice for **laptops (PowerXpress)**: Power Saving / High Performance / Based on Power Source. It applies at the next launch [COMMUNITY, Dell and Acer KB]. Sawicki (2018) found that Radeon Settings overrode the Windows setting at that time [COMMUNITY].
- On this desktop the iGPU driver key carries PX settings (`PXACAdapter=2`, `PXDCAdapter=1`, `SmartDCSupported=1`, `OpenGLDynamicPX=1`) [LOCAL]. Since Windows does not see a hybrid pair, Adrenalin does not offer switchable graphics here. [INFERRED]
- AMD's own components read and write the Windows key [BIN]:
  - `atieclxx.exe` ("AMD External Events Client") has `SmartMux::GetProcessGpuAffinity: Unable to find GPU preference for Application`, `Software\Microsoft\DirectX\UserGpuPreferences`, `GpuPreference=2;`, `GpuPreference=1;` and `AmdPowerXpressRequestHighPerformance`.
  - The kernel driver `amdkmdag.sys` reads `\Registry\User\<SID>\Software\Microsoft\DirectX\UserGpuPreferences\` next to `SSEcoAPUSteering*` and `SmartDCList`.
  - [INFERRED] AMD has adopted the Windows key as the single source of truth for per-app GPU choice, and its "Eco"/APU steering may write `GpuPreference=1` for some apps.
- **NVIDIA:**
  - "Preferred graphics processor" exists only on Optimus systems. "OpenGL rendering GPU" lets the user pick the GPU for OpenGL. "CUDA - GPUs" limits CUDA devices [DOC].
  - The NVIDIA help cross-references "a change in functionality in newer Windows 10" around preferred GPU. Third parties report that since Windows 10 20H1 the Windows setting overrides NVIDIA's [COMMUNITY].
- **Exe exports `NvOptimusEnablement` / `AmdPowerXpressRequestHighPerformance`** are the vendor "please use the dGPU" hints. Windows' own `directxdatabasehelper.dll` checks them [BIN]. [INFERRED] On this non-hybrid desktop they map to "high performance", which currently resolves to the iGPU.

---

## 8. Implications for the planned app and for Corral

1. **A native "GPU picker" app is feasible without admin and without hooks.** It writes the HKCU tokens of §2 per exe or per AUMID:
   - "Specific GPU": `SpecificAdapter=VEN&DEV&SUBSYS;GpuPreference=1073741824;`.
   - The machine default: `HighPerfAdapter=` in `DirectXUserGlobalSettings`.
   - It enumerates GPUs with DXCore (`IsIntegrated`, `HardwareIDParts`; DXCore skips IDD display-only adapters) or with DXGI plus a D3DKMT adapter-type filter. DXGI repeats the 7800 XT name for the IDD adapter.
   - It must show which settings the app may ignore (OpenGL, compute, engines with their own selection) and that the app needs a restart.
   - It should also offer the one-click "Open Windows Graphics settings" deep link.
2. **Explain the real cause in the app.** Show which adapter Windows uses as adapter 0 (the primary display's GPU) and what Windows treats as "High performance" (`EnumAdapterByGpuPreference(HIGH_PERFORMANCE)[0]`). Warn when HIGH_PERFORMANCE is not the GPU with the most compute, or when the primary monitor sits on the iGPU. On this PC both warnings would fire.
3. **The R9700 (32 GB) will need explicit handling.**
   - With 3 GPUs and no integrated flag, "High performance" picks whatever Windows ranks first, so `HighPerfAdapter` should be set explicitly.
   - The R9700 and 7800 XT have different DEV IDs, so `SpecificAdapter` can tell them apart.
   - A future second identical card cannot be told apart by `SpecificAdapter`.
4. **Corral hierarchy.** Corral already handles the compute side (Ollama env vars: `HIP_VISIBLE_DEVICES`, `GGML_VK_VISIBLE_DEVICES`, `OLLAMA_IGPU_ENABLE`…; `C:\personal\corral\src-tauri\src\ollama_gpus.rs:5-8, 85-105`). A "GPU hierarchy" there can drive both layers from one ordered list:
   - **Compute:** the visible-devices order for Ollama, as today.
   - **Graphics (opt-in):** write `HighPerfAdapter` = top GPU of the list, and optionally `SpecificAdapter` per chosen exe.
   - These are separate mechanisms that apps honour in different ways. The UI should not suggest that one list controls everything.
5. **Corral bug (found while reading).** `C:\personal\corral\src-tauri\src\gpu\mod.rs:58-60` (`is_real_adapter`) filters `IddSampleDriver` by name, but DXGI reports the IDD adapter as "AMD Radeon RX 7800 XT" with 15.8 GiB (LUID 0x2764f, no outputs) [LOCAL]. `gpu\windows.rs:36-58` therefore lists the 7800 XT twice, and the copy always shows ~0 MB used. Corral also shows the iGPU as a "16 GB dedicated" GPU even though that memory is carved from system RAM.
6. **gow2-recomp "auto" choice is wrong on this PC.** `C:\personal\gow2-recomp\src\gow2_gpu_pick.cpp:69-92, 131-137` (cvar `gow2_gpu`, default `auto`) uses `EnumAdapterByGpuPreference(HIGH_PERFORMANCE)` first, which returns the iGPU here, and falls back to "largest dedicated memory", which also returns the iGPU here. The comment at line 69-71 already says memory ranking picks the iGPU. Fixes:
   - Prefer DXCore `!IsIntegrated` or D3D12 `D3D12_FEATURE_DATA_ARCHITECTURE.UMA == FALSE`, then the largest **non-UMA** memory.
   - Or let the user set `gow2_gpu=7800` until `HighPerfAdapter` is configured.
   - It also needs a fullscreen / borderless / windowed option and a check that its window is on the 7800 XT monitor, to avoid the cross-adapter copy of §5.
7. **bipolar-code.** `llamacpp_service.py:31-117` already drops iGPUs from the tensor-split. The "bipolar uses the iGPU" symptom is therefore most likely the browser or WebView rendering the UI on adapter 0. [INFERRED]

---

## 9. Open questions and how to test them (each needs a registry write or a settings change, so the user has to approve)

1. **Why does HIGH_PERFORMANCE return the iGPU: default order, or ranking by memory?**
   - Test A: set Default high performance GPU = 7800 XT in Settings, then run `probes/dxgi_enum.py`. HIGH_PERFORMANCE[0] should become LUID 0x16c9b.
   - Test B: without A, make DISPLAY1 the primary monitor. If HIGH_PERFORMANCE[0] follows the primary, hypothesis (a) holds. If it stays on the iGPU, (b) holds.
   - Test C: set the BIOS UMA back to 512 MB and compare.
2. **Does `GpuPreference=2` on an exe reorder `EnumAdapters1` on this non-hybrid desktop, and to which GPU?** Copy python.exe to a scratch path, add a preference for that path, run `dxgi_enum.py`, then remove the value.
3. **Does `SpecificAdapter=1002&747E&2427148C;GpuPreference=1073741824;` put the 7800 XT first in EnumAdapters1 for that exe?** Same test with the exact token, which also confirms the format on build 26300.
4. **Do OpenGL apps ever honour a per-app preference on AMD desktops (`OpenGLDynamicPX`)?** Test with a GL renderer-string dump (e.g. glview) with the window on each monitor.
5. **What exactly does `VK_LAYER_AMD_switchable_graphics` do on this desktop?** Compare `vulkaninfo --summary` with and without `DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1=1`. This is a read-only env var on one process.
6. **Who writes `AppStatus=0` and the ASUS `GpuPreference=1` entries?** Candidates are the Windows game detection / Auto SR database and AMD APU steering. Process Monitor on the key would show the writer.
7. **What does WebView2 do with a per-app preference set on the host exe?** It is unlikely to work, but this is unverified.
8. **Where are the per-app tokens for packaged apps stored and named?** The AUMID naming is only community-verified.

---

## 10. Sources

Microsoft (primary):
- IDXGIFactory6::EnumAdapterByGpuPreference — https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_6/nf-dxgi1_6-idxgifactory6-enumadapterbygpupreference
- DXGI_GPU_PREFERENCE — https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_6/ne-dxgi1_6-dxgi_gpu_preference
- IDXGIFactory::EnumAdapters (adapter 0 = primary desktop output) — https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgifactory-enumadapters
- D3D12CreateDevice / D3D11CreateDevice (NULL = first EnumAdapters adapter) — https://learn.microsoft.com/en-us/windows/win32/api/d3d12/nf-d3d12-d3d12createdevice , https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-d3d11createdevice
- DXCoreAdapterPreference — https://learn.microsoft.com/en-us/windows/win32/api/dxcore_interface/ne-dxcore_interface-dxcoreadapterpreference
- D3DKMT_ADAPTERTYPE — https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/d3dkmthk/ns-d3dkmthk-_d3dkmt_adaptertype
- KMTQUERYADAPTERINFOTYPE — https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/d3dkmthk/ne-d3dkmthk-_kmtqueryadapterinfotype
- Using cross-adapter resources in a hybrid system — https://learn.microsoft.com/en-us/windows-hardware/drivers/display/using-cross-adapter-resources-in-a-hybrid-system
- Validating a hybrid system configuration — https://learn.microsoft.com/en-us/windows-hardware/drivers/display/validating-a-hybrid-system-configuration
- Windows Insider build 20190 (default high-perf GPU + specific GPU per app) — https://blogs.windows.com/windows-insider/2020/08/12/announcing-windows-10-insider-preview-build-20190/
- Windows Insider build 25336 (choose high-perf among 2+ dGPUs) — https://blogs.windows.com/windows-insider/2023/04/07/announcing-windows-11-insider-preview-build-25336/
- DirectX blog, "Navigating the Redesigned Graphics Settings page" (2023-01-19) — https://devblogs.microsoft.com/directx/?p=7369
- DirectX blog, "Optimizing hybrid laptop performance with Cross Adapter Scan-Out (CASO)" (2023-09-25) — https://devblogs.microsoft.com/directx/wp-json/wp/v2/posts/8407
- Launch Windows Settings (ms-settings:display-advancedgraphics[-default]) — https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings-app
- WebView2 CreateCoreWebView2EnvironmentWithOptions (AdditionalBrowserArguments, env/registry overrides) — https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/webview2-idl#createcorewebview2environmentwithoptions
- Rafael Cintron (Microsoft) on public_webgl, 2018-07-05 — https://www.khronos.org/webgl/public-mailing-list/public_webgl/1807/msg00012.php

Khronos / Vulkan loader (commit 73cf5326e8c87d7922b235666198988064b0f294):
- `docs/LoaderDriverInterface.md:1035-1088` (Physical Device Sorting) — https://github.com/KhronosGroup/Vulkan-Loader/blob/main/docs/LoaderDriverInterface.md
- `loader/loader_windows.c:856-932` (`windows_read_sorted_physical_devices`) — https://github.com/KhronosGroup/Vulkan-Loader/blob/main/loader/loader_windows.c

Chromium (HEAD ddfc2cdb06f901c04a829b2219ddc863cad35106) / ANGLE (HEAD b40b5946cd4ca423c65f9efe4bb0f8b4dc6ae52c):
- `gpu/config/gpu_info_collector_win.cc:345-401`, `gpu/ipc/service/gpu_init.cc:271-361`, `ui/gl/gl_display.cc:74-90,117-135`, `ui/gl/gl_switches.cc:163-165`, `gpu/config/gpu_switches.cc:57`, `gpu/config/gpu_workaround_list.txt:90-91` — https://chromium.googlesource.com/chromium/src/
- ANGLE `src/libANGLE/renderer/d3d/d3d11/Renderer11.cpp:684-756` — https://chromium.googlesource.com/angle/angle/

Vendors:
- NVIDIA Control Panel, Manage 3D Settings (OpenGL rendering GPU, Preferred graphics processor, restart note) — https://www.nvidia.com/content/Control-Panel-Help/vLatest/en-us/mergedProjects/nv3D/Manage_3D_Settings_(reference).htm
- Tauri v2 config, `WindowConfig.additionalBrowserArgs` — https://v2.tauri.app/reference/config/

Community / third-party (used only where no primary source exists):
- Adam Sawicki, "Switchable graphics versus D3D11 adapters" (2018) — https://asawicki.info/news_1675_switchable_graphics_versus_d3d11_adapters
- kronis.dev, "What is ruining dual GPU setups" (SpecificAdapter format, games that ignore it) — https://blog.kronis.dev/blog/what-is-ruining-dual-gpu-setups/
- ElevenForum, HighPerfAdapter VEN&DEV&SUBSYS thread — https://www.elevenforum.com/t/find-gpu-ven-dev-subsys-values-for-highperfadapter-option-using-wmic-or-other-method.30127/
- GitHub code using the formats: woogi999/pious-bootstrapper `src-tauri/src/core/gpupref.rs`, MetoisTaken/QuickGPUSelectionTool, levy-street/world-of-claudecraft `electron/gpu_preference.cjs`, shoober420/windows11-scripts, ryuya0124/GPUAssign.

Local binaries inspected (string tables only, build 26300): `C:\Windows\System32\directxdatabasehelper.dll`, `dxgi.dll`, `d3d9.dll`, `opengl32.dll`, `SettingsHandlers_Gpu.dll`, `atieclxx.exe`, `DriverStore\...\u0203731.inf_amd64_05ec0aa74f68ef13\B026470\amdkmdag.sys`.
