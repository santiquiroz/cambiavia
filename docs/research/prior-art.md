# Prior art: tools that choose the GPU per app, or a default GPU for games, on Windows

Research date: 2026-10-14. Scope: built-in Windows settings, vendor control panels, open-source tools on GitHub, launchers, game-side and translation-layer switches, and compute apps. For each tool: what it does, how it works, license, maintenance, and gaps. Goal: decide what a new native app (and Corral's GPU hierarchy) should do that nobody does well.

Tags: **[DOC]** primary vendor documentation. **[SRC]** project source code that I read, with file:line at the commit I cloned. **[LOCAL]** read-only measurement on this PC. **[COMMUNITY]** forum or guide, not authoritative. **[INFERRED]** my own conclusion; needs a test.

The Windows mechanism (registry format, DXGI ordering, which APIs honour it) is covered in depth in `windows-gpu-preference.md` in this folder. This file only repeats what is needed to judge the tools.

---

## 0. Conclusions first

1. **Every serious tool writes the same thing: `HKCU\Software\Microsoft\DirectX\UserGpuPreferences`.** That is the store behind Settings > System > Display > Graphics. Nobody has a better lever for DirectX apps; the differences are in UX (bulk edit, context menu, discovery, undo) and in whether they write the right token. [SRC, all tools below]
2. **Most tools only write `GpuPreference=1` (power saving) or `GpuPreference=2` (high performance). That cannot fix this PC.** Here DXGI ranks the iGPU first for both MINIMUM_POWER and HIGH_PERFORMANCE. The 16 GB UMA carve-out (17,152,372,736 B) is larger than the 7800 XT's 16,963,133,440 B, and neither adapter carries the kernel hybrid flags. [LOCAL; see `windows-gpu-preference.md` §1-3] Only two settings point an app at a named GPU: per-app "Specific GPU" (`SpecificAdapter=…;GpuPreference=1073741824;`) and the global `HighPerfAdapter=` in `DirectXUserGlobalSettings`.
   - Among the GPU-preference managers surveyed, three write `SpecificAdapter`: GPU Preference Manager, Dual-GPU-Application-Manager and QGST. Code search also finds it in a few app-specific tools (Discord "gpubinder" mods, ShadowPlay-NEXT, a game optimizer).
   - One lineage writes `HighPerfAdapter`: select_default_GPU → gpu-selector, plus the niche TeslaCMP-MultiGraphicsManager.
   - None does both.
3. **No tool covers more than DirectX well.**
   - OpenGL on a non-hybrid desktop ignores the Windows setting. [COMMUNITY: Lossless Scaling dual-GPU guide, note 5] The only fixes are NVIDIA's "OpenGL rendering GPU" (NVIDIA-only) or a global, admin-only ICD hack (`select_default_GPU` / `gpu-selector`).
   - Vulkan apps follow DXGI's default order only when they take device 0. The Windows loader sorts devices with `EnumAdapterByGpuPreference(…, DXGI_GPU_PREFERENCE_UNSPECIFIED, …)`. [SRC Vulkan-Loader `loader/loader_windows.c:878` @ main 24f17bd] Engines that score device types (Godot) or read the registry themselves (Godot, DagorEngine) understand only `GpuPreference=1/2`, not `SpecificAdapter`. [SRC]
   - Compute apps (Ollama/llama.cpp, ROCm/HIP, CUDA, DirectML) choose their own device. Per-process environment variables are the only lever. No general tool manages them; Corral does it for Ollama only.
4. **Nobody discovers games well.** Tools either make you browse to an exe or scan whole folders/MFT for every exe. `win_global_gpu` writes a value for every exe on every NTFS volume and deletes the whole key first. Windows' own game list (`HKCU\System\GameConfigStore\Children`, 89 entries here) keeps only a folder name in clear text; the exe path is a DPAPI blob. [LOCAL] The best evidence of "which app actually renders on which GPU" is the per-process `\GPU Engine(*)` PDH counters (pid + LUID). Two tools use them for display, but none uses them to learn the right exe to configure.
5. **Multi-dGPU is barely handled.** `SpecificAdapter` keys on vendor, device and subsystem IDs only, so two identical cards collide. An IndirectDisplay adapter (IddSampleDriver here) reports the same name and IDs as the 7800 XT under a different LUID. [LOCAL; SRC Yukina `docs/RESEARCH_NOTES.md`] Only one tool detects that ambiguity (GPU Preference Manager). The 7800 XT + R9700 combination will have different DEV IDs, so `SpecificAdapter` will work for it. [INFERRED]
6. **What a new app should do that none does well:** treat the problem as one ordered GPU hierarchy and apply it on every layer the app uses:
   - DXGI: write `SpecificAdapter` per app and `HighPerfAdapter` globally.
   - Vulkan: `VK_LOADER_DEVICE_ID_FILTER` / `DXVK_FILTER_DEVICE_*` and engine arguments, through a launcher shim.
   - OpenGL: tell the user which monitor/ICD decides, rather than silently failing.
   - Compute: `HIP_VISIBLE_DEVICES` / `CUDA_VISIBLE_DEVICES` / `GGML_VK_VISIBLE_DEVICES` in a given order.

   Add: discovery driven by actual GPU usage, verification after apply (which LUID the process really used), token-preserving edits with undo, and an explanation when Windows' own idea of "high performance" is wrong (as on this PC).

---

## 1. Facts about this PC that decide which prior art can work  [LOCAL]

Measured read-only on 2026-10-14. Full tables are in `windows-gpu-preference.md` §1; my own DXGI probe (C# via PowerShell, run as `pwsh.exe` with no per-app preference) gave the same result:

| DXGI order | UNSPECIFIED (= EnumAdapters1) | MINIMUM_POWER | HIGH_PERFORMANCE |
|---|---|---|---|
| 0 | AMD Radeon(TM) Graphics, DEV 164E, 17,152,372,736 B, LUID 0x1912E | iGPU | **iGPU** |
| 1 | AMD Radeon RX 7800 XT, DEV 747E, SUBSYS 2427148C, LUID 0x16C9B | 7800 XT | 7800 XT |
| 2 | "AMD Radeon RX 7800 XT", same IDs, **LUID 0x2764F** (IddSampleDriver virtual display) | Basic Render | Basic Render |
| 3 | Microsoft Basic Render Driver | IDD copy | IDD copy |

- `HKLM\SOFTWARE\Microsoft\DirectX\{GUID}` adapter cache:
  - `MaxDedicatedVideoMemory = 0x3fe5c7000`, which is the iGPU's value.
  - A stale iGPU entry with 509,374,464 B, from before the BIOS change.
  - AdapterType for the 7800 XT is `0x230B` (Render, Display, **PostDevice**, ACG, SetTimings, RuntimePM). For the iGPU it is `0x2303` (no Post).
  - For the IDD it is `0x342` (Display, **IndirectDisplayDevice**, …).
  - No `HybridDiscrete` (0x10) or `HybridIntegrated` (0x20) bit on either GPU.
  - The bit positions come from [`D3DKMT_ADAPTERTYPE`](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/d3dkmthk/ns-d3dkmthk-_d3dkmt_adaptertype) [DOC]. That the registry `AdapterType` mirrors that bitfield is [INFERRED], but it matches the KMT query in `windows-gpu-preference.md` §1.4.
- `HKCU\Software\Microsoft\DirectX\GraphicsSettings`: `DefaultHighPerfGPUApplicable=1`, `SpecificGPUOptionApplicable=1`. Settings therefore offers both "Default high performance GPU" and per-app "Specific GPU" here.
- `UserGpuPreferences` today:
  - `GpuPreference=1;` for two ASUS apps.
  - `AppStatus=0;` / `AutoHDREnable=2097;` for Steam games, which look like Windows' own game detection. [INFERRED]
  - `DirectXUserGlobalSettings = SwapEffectUpgradeEnable=1;AutoHDREnable=1;`, with no `HighPerfAdapter`.
  - So the values hold tokens other than the GPU choice. A tool that overwrites the whole string destroys Auto HDR and windowed-game settings.
- `vulkaninfo --summary` (instance 1.4.341):
  - GPU0 = iGPU, `PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU`, driver API 1.4.315.
  - GPU1 = 7800 XT, `DISCRETE_GPU`, driver API 1.4.349.
  - Vulkan device 0 is the iGPU. Vulkan's own type field still identifies it correctly.
- Game Bar game list:
  - `HKCU\System\GameConfigStore\Children` has 89 entries.
  - Readable fields: `Type`, `Flags`, `ExeParentDirectory` (folder name only), `GameDVR_GameGUID`.
  - `Parent` is a binary blob starting with the DPAPI header `01000000D08C9DDF0115D1118C7A00C04FC297EB`.
  - Windows' detected-games list does not expose exe paths in clear text. The DPAPI reading is [INFERRED] from the well-known provider GUID.

Consequence for the tool survey: anything that only offers "Power saving / High performance" is a no-op on this PC. Both resolve to the iGPU.

---

## 2. Windows built-in: Settings > System > Display > Graphics

**What it does.**
- Per-app list (classic exe by path, Store app by AUMID) with "Let Windows decide / Power saving / High performance". Since Insider build 20190 there is also **"Specific GPU"**: "allow users to pick a specific GPU on a per application basis" ([build 20190 blog](https://blogs.windows.com/windows-insider/2020/08/12/announcing-windows-10-insider-preview-build-20190/)) [DOC].
- System-wide **"Default high performance GPU"**: "an application that asks for a high-performance GPU will by default use the high-performance GPU you specify" (same post) [DOC]. Build 25336 widened it: "Customers with 2 or more discrete graphics adapters with no integrated graphics can now choose which of the discrete graphics adapters is considered high-performance" ([build 25336 blog](https://blogs.windows.com/windows-insider/2023/04/07/announcing-windows-11-insider-preview-build-25336/)) [DOC].
- On this PC both options are exposed, because Windows does not see an integrated GPU (§1). [LOCAL + INFERRED]
- A per-app value overrides the system-wide one: "your preference for the specific app will override the system-wide setting preference" ([DirectX blog, 2023-01-19](https://devblogs.microsoft.com/directx/navigating-the-redesigned-graphics-settings-page)) [DOC].
- The settings are requests: "Applications are always allowed to have the ultimate choice of which GPU to use". "Power saving" is "a request to run the application on the most power saving GPU available" ([build 17093 blog](https://blogs.windows.com/windows-insider/2018/02/07/announcing-windows-10-insider-preview-build-17093-pc/)) [DOC].

**How.**
- Values in `HKCU\Software\Microsoft\DirectX\UserGpuPreferences`, named by the full exe path or by the AUMID for packaged apps (`win_global_gpu` enumerates `AppListEntry.AppUserModelId()` for this [SRC `src/winapp_scan.rs:14-18`]; Godot does the same lookup [SRC godot `platform/windows/os_windows.cpp:852-874` @ d824ca9]).
- Tokens [COMMUNITY + SRC of several tools; Microsoft does not document the format]:
  - `GpuPreference=0|1|2;`
  - `SpecificAdapter=VVVV&DDDD&S…;GpuPreference=1073741824;`. The subsystem ID is printed in hex without leading zeros [SRC Yukina `SpecificAdapterKey.cs:11-12`, `docs/REGISTRY_FORMAT.md`].
  - Global `DirectXUserGlobalSettings` with `HighPerfAdapter=VVVV&DDDD&SSSSSSSS;` [SRC nethe `select_default_GPU.bat:81`].
- The DXGI side is documented: [`EnumAdapterByGpuPreference`](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_6/nf-dxgi1_6-idxgifactory6-enumadapterbygpupreference) orders MINIMUM_POWER as iGPU > dGPU > xGPU, and HIGH_PERFORMANCE as xGPU > dGPU > iGPU. UNSPECIFIED "is equivalent to calling `IDXGIFactory1::EnumAdapters1`". [DOC]

**License / maintenance.** Part of Windows, actively changed: "Specific GPU" was first announced in Insider build 20190 (2020), multi-dGPU default added in build 25336 (2023), and the page was redesigned in 2023. Which retail release first shipped "Specific GPU" is not established here. The registry format is an implementation detail with no public write API. [DOC: absence; COMMUNITY: GPU Flip README says the same]

**Gaps.**
- One app at a time, browse to an exe, no bulk edit, no discovery of games.
- "High performance" depends on Windows classifying iGPU vs dGPU, which fails on this desktop (§1).
- "Specific GPU" cannot tell identical cards apart: the key has no LUID or bus location. [SRC Yukina `RESEARCH_NOTES.md` "重复 SpecificAdapter key"]
- No effect on compute APIs. OpenGL is ignored on non-hybrid desktops [COMMUNITY], and Vulkan is only indirect (§0.3).
- No verification of what the app really used. Changes need an app restart. [DOC: NVIDIA and Microsoft both say restart]

---

## 3. Vendor control panels and app-side hints

| Tool | What / how | Coverage | Gaps |
|---|---|---|---|
| **NVIDIA Control Panel**: Preferred graphics processor | Per-program driver profile. "Only on systems using NVIDIA's power-saving GPU technology" (Optimus). ([Manage 3D Settings reference](https://www.nvidia.com/content/Control-Panel-Help/vLatest/en-us/mergedProjects/nv3d/Manage_3D_Settings_(reference).htm)) [DOC] | DX + OpenGL on Optimus laptops | Since Windows 10 20H1 "the Windows OS assigns the graphics processor … overrides any settings made in the NVIDIA Control Panel". The NVIDIA value only applies "if the OS does not have a GPU preference for an application" ([Setting the preferred graphics processor](https://www.nvidia.com/content/Control-Panel-Help/vLatest/en-us/mergedProjects/nv3d/Setting_the_preferred_graphics_processor.htm)) [DOC]. NVIDIA-only. |
| **NVIDIA Control Panel**: OpenGL rendering GPU | "lets you select which GPU to use for OpenGL applications" (global or per profile) [DOC, same reference] | The only vendor lever for OpenGL on multi-GPU desktops | NVIDIA GPUs only. The dual-GPU guide says it "doesn't always work" [COMMUNITY]. Irrelevant for an all-AMD rig. |
| **NVIDIA Control Panel**: CUDA - GPUs | "specify one or more GPUs to use for CUDA applications" [DOC] | CUDA compute | NVIDIA-only; global or per profile. |
| **NVIDIA Profile Inspector** (Orbmu2k, MIT, 7.4k stars, pushed 2026-10-13) | Edits NVIDIA driver profiles through NvAPI [SRC metadata via `gh`] | Same settings as NVCP, scriptable | NVIDIA-only. |
| **AMD Software: Adrenalin** | "Prior to … 19.12.1, assigning applications to run on either the Discrete or Integrated GPU was performed using the Switchable Graphics option … the Switchable Graphics option has been removed from Radeon Software Adrenalin Edition 19.12.1 and later". AMD recommends Windows Graphics settings ([AMD GPU-110](https://www.amd.com/en/resources/support-articles/faqs/GPU-110.html)) [DOC] | none | No per-app GPU choice. Laptop-oriented. Nothing for a desktop with Ryzen iGPU + Radeon dGPU. [DOC + INFERRED: no current AMD article found] |
| **`AmdPowerXpressRequestHighPerformance` / `NvOptimusEnablement` exports** | The app exports a symbol and the hybrid driver routes it to the dGPU ([GPUOpen](https://gpuopen.com/learn/amdpowerxpressrequesthighperformance/)) [DOC] | DX + OpenGL. GPUOpen: "these instructions do not apply to Vulkan™ or OpenCL™ applications" [DOC] | App-author side. Needs "a legacy PowerXpress or modern Hybrid Graphics laptop" [DOC]. A user cannot add it to someone else's exe. |
| **`EnableMsHybrid` (DWORD in the display class key)** | Community trick: 1 = performance GPU, 2 = power-save GPU, then reboot ([Lossless Scaling dual-GPU guide](https://github.com/SageInfinity/sageinfinity.github.io/blob/main/docs/Guides/6.%20Dual%20GPU%20Guide.md), Windows 10 section) [COMMUNITY] | Would make Windows classify GPUs as hybrid | Undocumented; no Microsoft source found. Results vary [COMMUNITY: Level1Techs]. Needs HKLM admin + reboot. Likely reset by driver reinstall. [INFERRED] |

---

## 4. Community tools on GitHub

Found with `gh search repos` ("GpuPreference", "gpu preference", "preferred gpu", "gpu switch", "switchable graphics") and `gh search code` (`UserGpuPreferences`, `SpecificAdapter=`, `HighPerfAdapter=`, `GpuPreference=1073741824`) on 2026-10-14. Repos were cloned shallow into the session scratchpad and read, not run. Stars and last commit are from `gh` at that date.

### 4.1 Summary table

| Tool | Stars / last commit / license | Mechanism | Specific GPU | Global HighPerf | Discovery | Undo / token-safe | Notes |
|---|---|---|---|---|---|---|---|
| [machineonamission/win_global_gpu](https://github.com/machineonamission/win_global_gpu) | 47 / 2025-11-26 / AGPL-3.0 | Rust CLI. Writes `GpuPreference=1|2` for **every exe** found by reading the NTFS MFT, plus every AUMID. Auto mode switches on AC/DC events | no | no | everything (MFT) | **No.** Calls `RegDeleteTreeW` on the whole key first [SRC `src/registry.rs:53-64`, `:93-94`] | NVIDIA UI-automation stub unfinished (`src/optimus.rs`, `todo!()`). Laptops only: "Intended only for systems with both a dedicated and an integrated GPU" [README]. |
| [thiwaK/preferred-gpu-changer](https://github.com/thiwaK/preferred-gpu-changer) | 24 / 2024-11-16 / none declared | C# context-menu "Run with graphics processor"; temporary (revert on exit) or permanent | no | no | right-click | **No.** `reg ADD … /d "GpuPreference=N;"` overwrites the value [SRC `Source/PrefGPU/Program.cs:85-86`] | Good UX idea (one-shot run). |
| [jakopo87/SetGpuPreference](https://github.com/jakopo87/SetGpuPreference) | 2 / 2020-07-08 / MIT | CLI add/delete/list + shell menu | no | no | none | ? | Abandoned. |
| [sharpjd/GPUPrefSwitcher](https://github.com/sharpjd/GPUPrefSwitcher) | 44 / 2025-02-08 / GPL-3.0 | .NET service. On plug/unplug changes per-app `GpuPreference`, swaps game settings files, runs Task Scheduler triggers [README] | no | no | manual list | backups of settings files | Laptops. Notes that child processes also need setting. |
| [Yukina-zzz/GPU_Preference_Manager](https://github.com/Yukina-zzz/GPU_Preference_Manager) | 0 / 2026-08-02 (v0.9.0) / MIT | WPF/.NET 10. DXGI adapter list with LUIDs. **PDH `\GPU Engine(*)\Utilization Percentage` per process tree** [SRC `src/GpuPreferenceManager.Windows/Metrics/PdhGpuMetricsSampler.cs:13`]. Bulk "specific adapter / power / perf / clear" | **yes**, with ambiguity check: duplicate VEN&DEV&SUBSYS across LUIDs disables exact writes. Filters `ROOT\DISPLAY` / software adapters [SRC `docs/RESEARCH_NOTES.md`] | read-only (`DirectXUserGlobalSettings` untouched) | from live GPU usage | **Yes.** `.reg` backup, read-back verify, history, rollback. Preserves unknown tokens [SRC `docs/REGISTRY_FORMAT.md`; `RegistryRuleSerializerTests.cs:18-34`] | Closest to "right". Chinese UI. "Almost entirely AI-created" [README]. Win11 only. No OpenGL/Vulkan/compute. |
| [Freeze7y/gpu-selector](https://github.com/Freeze7y/gpu-selector) (fork of [nethe-GitHub/select_default_GPU](https://github.com/nethe-GitHub/select_default_GPU), 64 stars, 2023) | 1 / 2026-09-07 (2.1.1) / AGPL-3.0 | PySide6 GUI. Global **`HighPerfAdapter`** [SRC `gpu_core.py:162`]. Per-app power/perf. **OpenGL ICD hack**: writes `HKLM\…\OpenGLDrivers\MSOGL` and renames each adapter's `OpenGLDriverName` to `_OpenGLDriverName`, so `opengl32.dll` falls back to one ICD [SRC nethe `select_default_GPU.bat:143-157`]. Separate 32/64-bit OpenGL and D3D11 **probes** that render and read back [README] | strips `SpecificAdapter` on reset, never writes it [SRC `app_preferences.py:54`] | **yes** | none | preview, backup, read-back, rollback [README] | Only tool that tries OpenGL. Needs admin + reboot. Breaks if the "OpenCL and OpenGL Compatibility Pack" (GLOn12) is installed. "Cannot load OpenGL ICD of Intel Processor Graphics" [nethe README]. Global, not per-app. Also forces `GraphicsSettings\DefaultHighPerfGPUApplicable=1` / `SpecificGPUOptionApplicable=1` to unlock the Settings UI [SRC `select_default_GPU.bat:25-27`]. |
| [SaithanaSriCharan/Dual-GPU-Application-Manager](https://github.com/SaithanaSriCharan/Dual-GPU-Application-Manager) | 0 / 2026-08-19 / none | Tkinter. Scans chosen folders + uninstall registry. Writes per-exe **`SpecificAdapter`** from WMI `PNPDeviceID` [SRC `gpu_application_manager_gui.py:184-209`] | yes | no | folder scan, installed apps | read-back verify | GPUs come from `Win32_VideoController` (VRAM field is 32-bit, wrong above 4 GB [INFERRED, well-known WMI limit]). Asks for UAC needlessly (HKCU). |
| [MetoisTaken/QuickGPUSelectionTool](https://github.com/MetoisTaken/QuickGPUSelectionTool) (QGST) | 0 / 2026-01-23 / none | .NET 10 UI + CLI. Context menu on .exe/.lnk/.bat/.url. **One-time run** with auto-revert, "crash-safe". "Distinguishes identical models" [README]. Writes `SpecificAdapter` (code search hit `QGST.Core/Services/PreferenceStoreAdapter.cs`) | yes | no | right-click | auto-revert | Best launch-once UX. Unlicensed. |
| [Vinicius-Bitnoob/gpu-flip](https://github.com/Vinicius-Bitnoob/gpu-flip) | 0 / 2026-10-05 / none | WPF, add exe, apply-and-launch, undo last [README] | **no**. "does not offer an arbitrary selector for every GPU in a three-or-more-adapter machine". Replaces existing `SpecificAdapter` [README] | no | manual | undo, preserves unrelated tokens | Defender flagged v1.0.0 (`Trojan:Win32/Sabsik.TE.A!ml`, unconfirmed) [README]. |
| [125m125/GpuPicker](https://github.com/125m125/GpuPicker) | 0 / 2026-09-06 / MIT | PowerShell + WinForms. Discovers running apps. Imports existing assignments incl. packaged-app IDs. Optional daily re-sync [README] | labels it only [SRC `GpuPicker.ps1:74`] | no | running apps | re-applies its own rules | Writes only power/perf/Windows. |
| [derekhe/gpu-optimizer](https://github.com/derekhe/gpu-optimizer) | 9 / 2026-06-27 / MIT | WPF/.NET 10. Lists processes using **dedicated GPU memory** (PDH) and moves chosen ones to `GpuPreference=1` to free dGPU VRAM [README] | no | no | live GPU memory | backup/restore | Reverse use case (keep dGPU VRAM free), relevant for LLM boxes. |
| [dimitriostzr/winlevers](https://github.com/dimitriostzr/winlevers) | 0 / 2026-10-05 (0.1.0) / MIT | WinUI 3. Bulk privacy permissions **and** per-app GPU power preference, diff preview, per-change rollback [README] | no | no | installed desktop + packaged apps | yes | Good bulk UX pattern. |
| [parikshith832/GPUPreferenceSwitcher](https://github.com/parikshith832/GPUPreferenceSwitcher) | 1 / 2026-06-29 / MIT | Toggle power/perf and restart the app [search description] | no | no | ? | ? | Laptops. |
| [Heymdale/GPUPreferences](https://github.com/Heymdale/GPUPreferences) | 0 / 2023-05-20 / GPL-3.0 | Learning project. Author: "i don't recommend to use it" [README] | no | no | – | no | Abandoned. |
| [dandy-step/AMD-Switch-Helper](https://github.com/dandy-step/AMD-Switch-Helper) | 0 / 2021-08-07 / none | Context menu that edits AMD PowerXpress profile `ATI\ACE\APL\User.blb` via `atiapfxx.exe` [SRC `src/AMDSwitchHelper.cpp:25-26`] | n/a | n/a | right-click | backs up blob | Pre-19.12.1 AMD laptops only. Dead. |
| [pinumbernumber/SetGPU](https://github.com/pinumbernumber/SetGPU) | 30 / 2019-01-09 / BSD-3-Clause | **Proxy DLLs** (`d3d11.dll`, `dxgi.dll`) dropped beside the game. Hooks adapter enumeration in `D3D11CreateDevice` [SRC `src/dx11/dx11.c:115`, `src/dxgi/`] | forces a chosen adapter | – | per game folder | delete the DLLs | "DX11 64-bit only", "may well trip battleye" [README]. Abandoned. Shows the shim approach works where the registry does not. |
| [Skyler1n/TeslaCMP-MultiGraphicsManager](https://github.com/Skyler1n/TeslaCMP-MultiGraphicsManager) | 13 / 2026-02-28 / MIT | Unlocks WDDM mode on Tesla/CMP cards and writes `HighPerfAdapter` (code search hit `GraphicsMgr.py`) | ? | yes | – | ? | Niche (mining/datacenter cards). |

Apps that write the key for themselves (code search):
- Minecraft launchers [HMCL](https://github.com/HMCL-dev/HMCL) (writes `GpuPreference=2;` for `javaw.exe` if absent [SRC `HMCL/src/main/java/org/jackhuang/hmcl/game/LauncherHelper.java:255-266`]) and PCL.
- The MGS HD Fix mod (`GpuPreference=2;` [SRC ShizCalev/MGSHDFix `src/fixes/windows_preferred_gpu.cpp:27-67`]).
- Discord client mods ("gpubinder", `SpecificAdapter`).
- Several Electron apps (`electron/gpu_preference.cjs` hits).

So writing this key per exe is already a common, accepted pattern.

Apps and engines that **read** the key and do their own selection:
- **Godot** honours only `GpuPreference=1` and otherwise prefers discrete by device-type score [SRC godot `platform/windows/os_windows.cpp:843-907`, `servers/rendering/rendering_device.cpp:103-108`]. It also has `--gpu-index` [SRC `main/main.cpp:567`].
- **DagorEngine** (Gaijin) parses values starting with `GpuPreference` only [SRC `prog/engine/drv/drv3d_commonCode/dxgi_utils.cpp:344-376`].
- [INFERRED] For these engines a `SpecificAdapter=…;GpuPreference=1073741824;` value is either ignored or mis-parsed. "Specific GPU" alone is not enough for such games; an engine argument is the fallback.

### 4.2 Patterns worth copying (and their sources)

- **Token-preserving edit.** Parse the `;`-separated tokens, replace only `GpuPreference` / `SpecificAdapter`, keep unknown tokens and their order. Yukina's serializer has tests for malformed and future tokens [SRC `tests/GpuPreferenceManager.Core.Tests/RegistryRuleSerializerTests.cs`]. GPU Flip and gpu-selector do the same.
- **Ambiguity guard.** Never pick an adapter by index or name. Join DXGI LUID to the PnP instance (DisplayConfig) and exclude IDD/`ROOT\DISPLAY`/software adapters [SRC Yukina `docs/RESEARCH_NOTES.md`]. Required on this PC: the IDD clone of the 7800 XT.
- **Verify, not just write.**
  - Read back the registry [Yukina, Dual-GPU, gpu-selector].
  - Better: watch the PDH `GPU Engine` counters for the relaunched pid, whose instance names carry `pid_…_luid_…` [SRC Yukina PDH sampler]. This shows the LUID the process actually renders on.
  - gpu-selector's render-and-read-back probes test the API path itself [README].
- **One-shot run.** Write the preference, launch, revert on exit, and stay crash-safe [QGST, preferred-gpu-changer].
- **Preview + undo + history** [WinLevers, Yukina, gpu-selector].

---

## 5. Launchers, engines, injectors, translation layers

| Item | GPU choice? | How | Gaps |
|---|---|---|---|
| **Steam** | No per-game GPU option found | Launch Options can pass engine args (below). Env-var prefixes (`VAR=x %command%`) are a Linux/Proton feature [INFERRED; not verified on Windows] | No discovery link to GPU. |
| **Playnite** ([JosefNemec/Playnite](https://github.com/JosefNemec/Playnite), MIT, 14.2k stars) | No add-on found (code search for `UserGpuPreferences` returned none; web search none) | The SDK allows custom actions/scripts [DOC: [cmdline/URI docs](https://api.playnite.link/docs/manual/advanced/cmdlineArguments.html)] | Gap = opportunity. Playnite's library plugins (Steam, Epic, GOG, Xbox…) are the best open game-discovery code; MIT, so ideas or code can be reused. |
| **Lossless Scaling** (commercial, Steam) | Own "Preferred GPU" for *its* frame-gen work only. The community dual-GPU guide uses **Windows "Default high performance GPU"** to choose the render GPU and recommends plugging the monitor into the secondary GPU [COMMUNITY: SageInfinity guide] | Its own DXGI device | Says OpenGL games "aren't affected by the Windows setting" and Vulkan games/emulators "require selecting the desired render GPU in their settings" [COMMUNITY]. Matches §0.3. |
| **Special K** ([SpecialKO/SpecialK](https://github.com/SpecialKO/SpecialK), GPL-3.0, 2.1k stars) | `dxgi.adapter_override` (index) [SRC `include/SpecialK/config.h:871`], applied in `SK_DXGI_AdapterOverride` during D3D11 device creation [SRC `src/render/d3d11/d3d11.cpp`, search hit] | DLL injection/proxy | Index-based (fragile with IDD clones). Per game. Anti-cheat risk [INFERRED]. |
| **DXVK** ([doitsujin/dxvk](https://github.com/doitsujin/dxvk), zlib) | `DXVK_FILTER_DEVICE_NAME` (substring) and `DXVK_FILTER_DEVICE_UUID` ("more precise selection, especially when using multiple identical GPUs") [DOC: README "Device filter"] | D3D8-11 → Vulkan. On Windows, DLLs go next to the game exe | Changes the API path (performance and compatibility trade-offs). Env var needs a launcher. |
| **Vulkan loader** | `VK_LOADER_DEVICE_ID_FILTER` / `VK_LOADER_VENDOR_ID_FILTER` / `VK_LOADER_DRIVER_ID_FILTER` hide non-matching devices. Available "with Loaders built with version 1.4.326 of the Vulkan headers and later". `VK_LOADER_DEVICE_SELECT` (reorder) is **"Linux Only"** [DOC: Vulkan-Loader `docs/LoaderInterfaceArchitecture.md`, env var table] | Per-process env var. This PC's loader is 1.4.341 [LOCAL] | Vendor filter useless here (iGPU and dGPU are both 0x1002). Device-ID filter works (0x747e vs 0x164e) but needs a launcher. Apps that bundle an old `vulkan-1.dll` ignore it [INFERRED]. |
| **Unity Player** | `-force-device-index <n>` for "D3D11, D3D12, Metal, and Vulkan … isn't supported for OpenGL" [DOC: [Unity player CLI](https://docs.unity.com/en-us/engine/7000.0/manual/unity-editor/command-line-arguments/player)] | Launch argument | Index base and ordering undocumented; the index presumably follows DXGI order, which includes the IDD clone here [INFERRED]. |
| **Unreal Engine** | `-graphicsadapter=<n>` / `r.GraphicsAdapter` [COMMUNITY only; no official page found] | Launch argument / ini | Unofficial, index-based. |
| **Godot 4** | `--gpu-index <device_index>` [SRC `main/main.cpp:567`] | Launch argument | Forward+/Mobile renderers only. |

---

## 6. Compute apps (LLM runtimes, renderers): nobody manages them per app

- These runtimes pick devices through their own stack, so `UserGpuPreferences` does not apply to their compute path [INFERRED from how HIP/CUDA/ggml enumerate; Corral already relies on this].
- The levers are per-process env vars:
  - `CUDA_VISIBLE_DEVICES`: "controls which GPU devices are visible … and in what order they are enumerated". Example: `=2,1` "enumerates device 2 before device 1". `CUDA_DEVICE_ORDER=FASTEST_FIRST|PCI_BUS_ID` [DOC: [CUDA env vars](https://docs.nvidia.com/cuda/cuda-programming-guide/05-appendices/environment-variables.html)].
  - On AMD: `ROCR_VISIBLE_DEVICES` (lowest level), `GPU_DEVICE_ORDINAL` (HIP + OpenCL), `HIP_VISIBLE_DEVICES` (HIP). Env vars "are not a security boundary" [DOC: [ROCm GPU isolation](https://rocm.docs.amd.com/en/latest/reference/system-optimization/gpu-isolation.html), via search snippet].
  - llama.cpp / Ollama Vulkan backend: `GGML_VK_VISIBLE_DEVICES`.
- Corral today:
  - `GpuProfile` is `Auto | Spread | Single{library, filter_id}`.
  - It maps ROCm→`HIP_VISIBLE_DEVICES` (Windows), CUDA→`CUDA_VISIBLE_DEVICES`, Vulkan→`GGML_VK_VISIBLE_DEVICES` and writes them as user env vars [SRC `C:\personal\corral\src-tauri\src\ollama_gpus.rs:5-8, 85-104`; `config.rs:6-10`].
  - It has no **ordered** list. The variables accept ordered lists (CUDA order is documented), so a hierarchy maps onto them directly. For HIP the ordering semantics are [INFERRED]; check the ROCm docs or runtime source.
- Gap across all tools: no app gives the user one place to say "for compute, use R9700, then 7800 XT, never iGPU". Each runtime (Ollama, LM Studio, llama-server, ComfyUI, Blender) has its own knob.

---

## 7. Coverage matrix

Legend: ✔ yes, ~ partial, ✘ no.

| | Specific GPU per app | Global high-perf GPU | Order of ≥3 GPUs | Game discovery | Observe actual GPU | OpenGL | Vulkan-specific | Compute env | Undo / token-safe | One-shot launch |
|---|---|---|---|---|---|---|---|---|---|---|
| Windows Settings | ✔ (one at a time) | ✔ | ✘ | ✘ | ✘ | ✘ (desktop) | ~ (via loader sort) | ✘ | n/a | ✘ |
| NVIDIA CP / Profile Inspector | ~ (Optimus only) | ✘ | ✘ | ~ (profiles DB) | ✘ | ✔ (NVIDIA) | ✘ | ✔ CUDA | ✔ | ✘ |
| AMD Adrenalin | ✘ | ✘ | ✘ | ✔ (its game list, no GPU option) | ✘ | ✘ | ✘ | ✘ | – | ✘ |
| win_global_gpu | ✘ | ✘ | ✘ | ~ (all exes) | ✘ | ✘ | ✘ | ✘ | ✘ (wipes key) | ✘ |
| GPU Preference Manager (Yukina) | ✔ (+ambiguity guard) | ✘ (read-only) | ✘ | ~ (running apps) | ✔ PDH | ✘ | ✘ | ✘ | ✔ | ✘ |
| gpu-selector / select_default_GPU | ✘ | ✔ | ✘ | ✘ | ~ (own probes) | ~ (global ICD hack) | ✘ | ✘ | ✔ | ✘ |
| Dual-GPU-App-Manager | ✔ | ✘ | ✘ | ~ (folders) | ✘ | ✘ | ✘ | ✘ | ~ | ✘ |
| QGST | ✔ | ✘ | ✘ | ✘ | ✘ | ✘ | ✘ | ✘ | ✔ revert | ✔ |
| preferred-gpu-changer | ✘ | ✘ | ✘ | ✘ | ✘ | ✘ | ✘ | ✘ | ✘ (clobbers) | ✔ |
| GPU Flip / GpuPicker / WinLevers / gpu-optimizer | ✘ | ✘ | ✘ | ~ | ~ (gpu-optimizer PDH) | ✘ | ✘ | ✘ | ✔ | ~ (GPU Flip) |
| SetGPU / Special K | ✔ (forced, DX11) | ✘ | ✘ | ✘ | ✘ | ✘ | ✘ | ✘ | – | per game |
| DXVK / Vulkan loader filters | ✔ (by name/UUID/ID) | ✘ | ✘ | ✘ | ✘ | ✘ | ✔ | ✘ | – | needs launcher |
| Corral v0.2a | ✘ | ✘ | ✘ (single only) | ✘ | ✔ (its own PDH/DXGI panel) | ✘ | ✘ | ✔ Ollama only | ✔ | ✘ |

Nobody fills the "Order of ≥3 GPUs" column, which is exactly the user's future 7800 XT + R9700 + iGPU case.

---

## 8. What the new app should do (and why)

1. **Fix the root cause on this PC first, with one switch.** Set `HighPerfAdapter=` (Settings' "Default high performance GPU") to the top non-iGPU card in the hierarchy. On this PC that should move every "High performance" app, and every Vulkan/Chromium/Unity/Godot path that asks for high performance, off the iGPU. This needs testing: `windows-gpu-preference.md` §9 test A. Pair it with a clear warning when Windows' HIGH_PERFORMANCE[0] is the iGPU, or when the primary monitor is on the iGPU (§1).
2. **Use "Specific GPU" for games, never just "High performance".** Write `SpecificAdapter=…;GpuPreference=1073741824;` while preserving other tokens. Refuse ambiguous keys (IDD clones, identical cards) and fall back to the per-API levers below.
3. **One ordered hierarchy, applied per layer** (the gap in §7):
   - DXGI via the registry (per app plus global).
   - Vulkan via `VK_LOADER_DEVICE_ID_FILTER` when the app is launched through us.
   - D3D-on-Vulkan via `DXVK_FILTER_DEVICE_UUID`.
   - Engines via arguments (`-force-device-index`, `--gpu-index`, `-graphicsadapter`), mapped from the hierarchy with the IDD-aware index.
   - Compute via ordered `HIP_VISIBLE_DEVICES` / `CUDA_VISIBLE_DEVICES` / `GGML_VK_VISIBLE_DEVICES`. Corral would own this for Ollama: turn `Single` into an ordered list.
4. **Discovery from evidence, not from scanning disks.**
   - PDH `GPU Engine` per process gives which exe used which LUID: suggest a fix, then verify after relaunch.
   - Launcher libraries give the game list: Steam `libraryfolders.vdf` + manifests, Epic, GOG, Xbox; Playnite's MIT plugins are a good reference.
   - Do not rely on GameConfigStore paths (DPAPI-protected here).
   - Never write a value for every exe (the `win_global_gpu` anti-pattern).
5. **One-shot launch with revert, context menu, CLI.** These are the best UX bits of QGST and preferred-gpu-changer.
6. **Honest OpenGL handling.** Show "OpenGL follows the GPU that drives the monitor; Windows setting ignored on this desktop". The only levers are moving the monitor or the (admin, global, reboot) ICD trick; offer neither silently. [COMMUNITY + SRC nethe]
7. **Safety and licensing.**
   - HKCU only for per-app values; no admin except for opt-in global hacks; backup/preview/undo.
   - License fit with Corral (AGPL-3.0): `win_global_gpu`, `gpu-selector` and `select_default_GPU` are AGPL. Yukina, GpuPicker, gpu-optimizer, WinLevers, SetGpuPreference and Playnite are MIT. SetGPU is BSD-3, DXVK zlib, Special K GPL-3.0. Code from all of them is license-compatible with an AGPL app.
   - QGST, GPU Flip, Dual-GPU-App-Manager and preferred-gpu-changer have no license, so ideas only, no code.

---

## 9. Open questions

1. Does `HighPerfAdapter=1002&747E&2427148C` make DXGI HIGH_PERFORMANCE[0] the 7800 XT on this non-hybrid desktop? And does it also reorder UNSPECIFIED, which is what the Vulkan loader uses? Test A in `windows-gpu-preference.md` §9 needs a Settings change approved by the user.
2. Does a per-app `SpecificAdapter` reorder `EnumAdapters1` / UNSPECIFIED for that exe? If yes, Vulkan apps taking device 0 also follow it, via the loader's sort at `loader_windows.c:878`.
3. How does Windows match `SpecificAdapter` when an IDD adapter has the same VEN&DEV&SUBSYS as the 7800 XT (LUID 0x2764F)? Which LUID wins?
4. Is `GpuPreference=1073741824` stable across builds? Only community and tool sources exist; there is no Microsoft documentation.
5. Do the AMD driver's OpenGL paths on this desktop follow the monitor's adapter (iGPU HDMI) or the POST device? This decides whether moving the HDMI cable is the only OpenGL fix.
6. Is there an official Playnite add-on database entry for GPU choice that GitHub code search missed (closed-source add-ons)?
7. What do the stale DirectX cache entries and the two AMD driver versions (iGPU UMD 0x2000005235138a vs dGPU 0x200000794103ec) mean for an app that keys on driver version? Probably nothing, but noted. [LOCAL]

---

## 10. Sources

Microsoft
- https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_6/nf-dxgi1_6-idxgifactory6-enumadapterbygpupreference
- https://learn.microsoft.com/en-us/windows/desktop/api/dxgi1_6/ne-dxgi1_6-dxgi_gpu_preference
- https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/d3dkmthk/ns-d3dkmthk-_d3dkmt_adaptertype
- https://learn.microsoft.com/en-us/windows-hardware/drivers/display/loading-an-opengl-installable-client-driver
- https://blogs.windows.com/windows-insider/2018/02/07/announcing-windows-10-insider-preview-build-17093-pc/
- https://blogs.windows.com/windows-insider/2020/08/12/announcing-windows-10-insider-preview-build-20190/
- https://blogs.windows.com/windows-insider/2023/04/07/announcing-windows-11-insider-preview-build-25336/
- https://devblogs.microsoft.com/directx/navigating-the-redesigned-graphics-settings-page

Vendors
- https://www.nvidia.com/content/Control-Panel-Help/vLatest/en-us/mergedProjects/nv3d/Manage_3D_Settings_(reference).htm
- https://www.nvidia.com/content/Control-Panel-Help/vLatest/en-us/mergedProjects/nv3d/Setting_the_preferred_graphics_processor.htm
- https://www.amd.com/en/resources/support-articles/faqs/GPU-110.html
- https://gpuopen.com/learn/amdpowerxpressrequesthighperformance/
- https://docs.nvidia.com/cuda/cuda-programming-guide/05-appendices/environment-variables.html
- https://rocm.docs.amd.com/en/latest/reference/system-optimization/gpu-isolation.html

Khronos / translation layers / engines
- https://github.com/KhronosGroup/Vulkan-Loader/blob/main/loader/loader_windows.c (lines 802, 860-932 @ 24f17bd, 2026-10-13)
- https://github.com/KhronosGroup/Vulkan-Loader/blob/main/docs/LoaderInterfaceArchitecture.md (env var table: VK_LOADER_DEVICE_SELECT "Linux Only"; *_ID_FILTER ≥ 1.4.326)
- https://docs.vulkan.org/features/latest/features/proposals/VK_MSFT_layered_driver.html ("on Windows, the loader uses LUIDs to put physical devices in the same order as DXGI adapters")
- https://github.com/doitsujin/dxvk/blob/master/README.md (Device filter)
- https://docs.unity.com/en-us/engine/7000.0/manual/unity-editor/command-line-arguments/player
- https://github.com/godotengine/godot (master d824ca9: platform/windows/os_windows.cpp:843-907; servers/rendering/rendering_device.cpp:103-108; main/main.cpp:567)
- https://github.com/GaijinEntertainment/DagorEngine/blob/main/prog/engine/drv/drv3d_commonCode/dxgi_utils.cpp (344-376)
- https://github.com/SpecialKO/SpecialK (include/SpecialK/config.h:871)

Community tools (cloned 2026-10-14; short SHA where recorded)
- https://github.com/machineonamission/win_global_gpu (d62381a)
- https://github.com/Yukina-zzz/GPU_Preference_Manager (bd8952e)
- https://github.com/Freeze7y/gpu-selector (d4190b7)
- https://github.com/nethe-GitHub/select_default_GPU (86c4fbb)
- https://github.com/SaithanaSriCharan/Dual-GPU-Application-Manager (e9167cc)
- https://github.com/MetoisTaken/QuickGPUSelectionTool
- https://github.com/thiwaK/preferred-gpu-changer
- https://github.com/sharpjd/GPUPrefSwitcher
- https://github.com/Vinicius-Bitnoob/gpu-flip
- https://github.com/125m125/GpuPicker
- https://github.com/derekhe/gpu-optimizer
- https://github.com/dimitriostzr/winlevers
- https://github.com/jakopo87/SetGpuPreference
- https://github.com/Heymdale/GPUPreferences
- https://github.com/parikshith832/GPUPreferenceSwitcher
- https://github.com/dandy-step/AMD-Switch-Helper
- https://github.com/pinumbernumber/SetGPU
- https://github.com/Skyler1n/TeslaCMP-MultiGraphicsManager
- https://github.com/Orbmu2k/nvidiaProfileInspector
- https://github.com/HMCL-dev/HMCL (LauncherHelper.java:255-266)
- https://github.com/ShizCalev/MGSHDFix (src/fixes/windows_preferred_gpu.cpp)
- https://github.com/JosefNemec/Playnite
- https://github.com/SageInfinity/sageinfinity.github.io/blob/main/docs/Guides/6.%20Dual%20GPU%20Guide.md (Lossless Scaling dual-GPU guide)

Local (read-only, 2026-10-14)
- `reg query HKCU\Software\Microsoft\DirectX /s`, `HKLM\SOFTWARE\Microsoft\DirectX`
- DXGI `EnumAdapterByGpuPreference` probe (C# via PowerShell, session scratchpad; same result as `probes/dxgi_enum.py`)
- `vulkaninfo --summary`
- `HKCU\System\GameConfigStore\Children`
- `Get-PnpDevice -Class Display`
