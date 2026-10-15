# How compute and AI stacks pick a GPU on Windows with AMD, and how Corral should map a GPU hierarchy onto Ollama

Research date: 2026-10-14. Machine: Windows 11 Pro 26300, Ryzen 9 7900X3D with iGPU "AMD Radeon(TM) Graphics" (PCI `DEV_164E`, gfx1036, 16 GB UMA carve-out), RX 7800 XT (`DEV_747E`, gfx1101). A Radeon AI PRO R9700 (`DEV_7551`, Navi 48, gfx1201) will be added later.

Companion note: `windows-gpu-preference.md` (same folder) covers DXGI/D3D, the Windows graphics settings and Chromium. This note covers the compute stacks and Corral.

Tags: **[DOC]** vendor documentation, **[SRC]** source code read at the given file:line, **[LOCAL]** measured on this PC today (read-only), **[INFERRED]** my reasoning, not verified.

Pinned source versions:
- Ollama `v0.40.4` = commit `45e6011` (the version installed here, `ollama --version` → 0.40.4).
- llama.cpp `master` @ `a0e6dc7` (2026-10-14).
- Vulkan-Loader `main` (2026-10-14). The installed loader is 1.4.341 [LOCAL].
- onnxruntime `main` (2026-10-14).
- Blender `main` (2026-10-14) and manual 5.2 LTS.

---

## 0. Short answer

1. **None of the compute stacks follow the Windows graphics preference.** Ollama, llama.cpp, PyTorch-ROCm, Blender HIP and LM Studio each list the devices themselves. They choose with their own rules and environment variables. A per-app "graphics preference" in Windows only reaches them indirectly:
   - Through DXGI's adapter order, which the Vulkan loader copies.
   - Through ONNX Runtime DirectML's `device_id`, which is a DXGI `EnumAdapters1` index.
2. **Every device-selection knob these stacks expose is a positional index,** and the indices on this PC moved after the BIOS change:
   - `HIP_VISIBLE_DEVICES`, `GGML_VK_VISIBLE_DEVICES`, llama.cpp `--device Vulkan0`, DirectML `device_id` and LM Studio's `visibleDevices` are all indices.
   - The Vulkan iGPU was `Vulkan1` on 2026-10-12. It is `Vulkan0` since 2026-10-13 [LOCAL, Ollama logs].
   - The HIP order also flipped between 2026-08-29 and 2026-10-13 (user memory note).
   - A GPU chooser must store a **stable identity** (PCI location + hardware IDs) and turn it into indices **at launch time**, every time.
3. **What actually makes Ollama use the iGPU today is Corral.**
   - `%APPDATA%\corral\config.toml` has `igpu_enabled = true`.
   - So Corral wrote `OLLAMA_IGPU_ENABLE=1` to `HKCU\Environment` [LOCAL `reg query`].
   - Ollama has kept the Vulkan iGPU (105.3 GiB "total") since 14:34 today. Before that, Ollama dropped it by itself [LOCAL server-1/2.log].
   - With the iGPU admitted, any model too big for 80 % of the 7800 XT's free VRAM is placed whole on the iGPU, instead of being partly offloaded to the 7800 XT [SRC + INFERRED, §2.5].
4. **Two index-free levers exist:**
   - `OLLAMA_IGPU_ENABLE` (unset or `0`) excludes iGPUs inside Ollama.
   - `VK_LOADER_DEVICE_ID_FILTER=<pci device ids>` hides GPUs from any Vulkan app at the loader level. Verified here: `VK_LOADER_DEVICE_ID_FILTER=0x747e vulkaninfo` lists only the 7800 XT [LOCAL].
   - A hierarchy should prefer these levers and fall back to indices only when it has to exclude one of two discrete GPUs.
5. **Ollama has no notion of GPU priority.**
   - The scheduler picks a single GPU by "discrete first, then most available memory". It splits a model only within one library group (ROCm or Vulkan, never both) [SRC `server/sched.go:1014-1166`].
   - On Windows, HIP does not reorder devices: `HIP_VISIBLE_DEVICES=1,0` keeps the physical order [LOCAL].
   - So a user-ordered list can map onto Ollama only as an **allow-list**, plus an optional "first only" mode. The order is not a ranking Ollama obeys.

---

## 1. Measured state of this PC [LOCAL]

| Layer | Order / value | How measured |
|---|---|---|
| DXGI `EnumAdapters1` | 0 = iGPU (16357 MiB "dedicated"), 1 = 7800 XT (16177 MiB), 2 = "AMD Radeon RX 7800 XT" LUID 0x2764f with no outputs (the IddSampleDriver virtual display, see companion note §1.4), 3 = Basic Render | `scratchpad/dxgi-order.ps1` (C# vtable calls) |
| DXGI `EnumAdapterByGpuPreference` | UNSPECIFIED, MINIMUM_POWER and HIGH_PERFORMANCE **all put the iGPU first** | same |
| DXCore (`D3D12_CORE_COMPUTE` list) | 7800 XT, then iGPU, for every `Sort()` preference. `IsIntegrated` (1-byte property) = **0 for the iGPU** | `scratchpad/dxcore-order.ps1` |
| Vulkan (`vulkaninfo --summary`) | GPU0 = iGPU (`INTEGRATED_GPU`, deviceUUID `00000000-0f00-…`), GPU1 = 7800 XT (`DISCRETE_GPU`, deviceUUID `00000000-0300-…`). Disabling `VK_LAYER_AMD_switchable_graphics` does not change the order | vulkaninfo 1.4.341 |
| Vulkan with `VK_LOADER_DEVICE_ID_FILTER=0x747e` | only the 7800 XT (GPU0) | vulkaninfo |
| HIP (torch 2.12.0+rocm7.14.0, `hunyuan3d-env`) | 0 = 7800 XT gfx1101 (pci bus 3), 1 = iGPU gfx1036 (pci bus 15, 84988 MiB) | `scratchpad/hipenum.py` |
| HIP with `HIP_VISIBLE_DEVICES=1,0` | **same order** (7800 XT, iGPU) → filter only, no reordering | same |
| HIP with `HIP_VISIBLE_DEVICES=1` | only the iGPU | same |
| Ollama 0.40.4 discovery | ROCm0 = 7800 XT (`pci_id=0000:03:00.0`). ROCm1 = gfx1036, dropped with "no rocblas support". Vulkan0 = iGPU (`pci_id=""`, total 105.3 GiB), kept because `OLLAMA_IGPU_ENABLE:1`. `total_vram=121.3 GiB` | `%LOCALAPPDATA%\Ollama\server.log` lines 1-11 |
| Ollama, 2026-10-12 | the iGPU was **Vulkan1** (`id=1`), dropped by default | `server-5.log` |
| LM Studio engine index | Older surveys: device 0 = 7800 XT, 1 = iGPU. Newest (engines 2.55/2.57): 0 = iGPU, 1 = 7800 XT, saved as `visibleDevicesConfig: {visibleDevices: [1, 0], changesOrder: true}`. The device LUIDs differ in every survey; the deviceUUIDs stay the same | `~/.lmstudio/.internal/internal-engine-index.json` (read only) |
| `HKCU\Environment` | `OLLAMA_IGPU_ENABLE=1`, `OLLAMA_NUM_PARALLEL=2`. No `HIP_*`/`GGML_*` | `reg query` |
| Corral config | `igpu_enabled = true`, `gpu_profile.kind = "auto"` | `%APPDATA%\corral\config.toml` |

[INFERRED] AMD's Vulkan `deviceUUID` on Windows encodes the PCI bus number in bytes 4-5 (`0300` = bus 3, `0f00` = bus 15). This matches HIP's `pci_bus_id` (3 and 15) and Ollama's `pci_id=0000:03:00.0`. That gives a join key between Vulkan, HIP and PnP without trusting indices.

[LOCAL] PCI device IDs from `pci.ids`: `164e` = Raphael iGPU, `747e` = Navi 32 RX 7700 XT/7800 XT, `7551` = Navi 48 **Radeon AI PRO R9700**, `7550` = RX 9070 family. Source: https://github.com/pciutils/pciids/blob/master/pci.ids

---

## 2. Ollama 0.40.x (Windows, AMD)

### 2.1 Architecture since 0.40

Ollama now runs upstream llama.cpp's `llama-server.exe` as its runner [LOCAL log: `"starting llama-server" cmd=…\lib\ollama\llama-server.exe … --split-mode none --main-gpu 0`].

Discovery bootstraps one llama-server per library directory (`lib/ollama/rocm_v7_1`, `lib/ollama/vulkan`). It then filters and deduplicates the devices [SRC `discover/runner.go:37-260`].

The discovery runs **once per server process**, guarded by `bootstrapped` [SRC `discover/runner.go:28`, `:45`, `:265`]. So a GPU added or removed needs an Ollama restart [INFERRED].

### 2.2 Environment variables (server side) [SRC `envconfig/config.go:228-258`, `:321-363`]

| Variable | Effect |
|---|---|
| `OLLAMA_IGPU_ENABLE` | Tri-state. Explicit true keeps iGPUs. Explicit false drops them. **Unset:** CUDA iGPUs are kept; ROCm iGPUs are kept only for `gfx1151` (Strix Halo); **Vulkan iGPUs are dropped** [SRC `discover/runner.go:31-34`, `:382-438`] |
| `OLLAMA_VULKAN` | Default true. False skips the Vulkan lib dir entirely [SRC `discover/runner.go:108`] |
| `OLLAMA_LLM_LIBRARY` | Restricts discovery to one lib dir, e.g. `rocm_v7_1` or `vulkan` [SRC `discover/runner.go:67`, `:100`] |
| `OLLAMA_SCHED_SPREAD` | Always spread a model across the selected GPU group |
| `HIP_VISIBLE_DEVICES` / `GPU_DEVICE_ORDINAL` / `CUDA_VISIBLE_DEVICES` | Numeric ROCm filter on Windows. The first one set wins [SRC `ml/device.go:463-470`, `discover/runner.go:621-625`] |
| `ROCR_VISIBLE_DEVICES` | Linux only (index or UUID). Ignored on Windows [SRC `discover/runner.go:611-619`]. Note: Corral's `MANAGED_VARS` includes it, which is harmless |
| `GGML_VK_VISIBLE_DEVICES` | Vulkan filter by index [SRC `discover/runner.go:626-627`]. The docs say `-1` disables all Vulkan GPUs [DOC `docs/gpu.mdx:167-176`] |
| `HSA_OVERRIDE_GFX_VERSION` | Linux-only workaround to map an unsupported gfx target [DOC `docs/gpu.mdx:88-100`] |

### 2.3 Identity of a device in Ollama

- `id` is the post-filter index within a library. `filter_id` is the pre-filter index [SRC `discover/runner.go:189-206`].
- If the user set a visibility variable, `filter_id` is rewritten to the **user's token** at that position. So it always names a physical index in the user's numbering [SRC `discover/runner.go:589-605`].
- `pci_id` is filled for ROCm (`0000:03:00.0`). It is **empty for Vulkan on Windows** [LOCAL].
- ROCm and Vulkan copies of the same GPU are deduplicated:
  - by `pci_id` when both copies have one;
  - otherwise by a fuzzy "same description + similar memory" check [SRC `ml/device.go:180-227`].
  - CUDA and ROCm are preferred over Vulkan [SRC `ml/device.go:382-389`, `discover/runner.go:209-243`].

### 2.4 The child-process environment

For each model load, Ollama builds the runner environment from the chosen devices [SRC `ml/device.go:335-436`, `llm/llama_server.go:949`]:
- ROCm devices **always** get `HIP_VISIBLE_DEVICES=<filter_ids>` (log: `extra_envs=map[HIP_VISIBLE_DEVICES:0]`).
- Vulkan devices are filtered only when a single device is selected.

The Ollama tray app copies its **own** `os.Environ()` into `ollama serve` and adds its UI settings (`OLLAMA_HOST`, `OLLAMA_CONTEXT_LENGTH`, …) [SRC `app/server/server.go:230-274`]. So:
- Corral launching `ollama app.exe` with a custom environment reaches the server.
- An Ollama started from the Start menu gets Explorer's environment block instead.

### 2.5 Placement: what "priority" means to Ollama [SRC `server/sched.go`]

- `selectLlamaServerPlacement` (`:1014`) groups the GPUs by library (`ml.ByLibrary`). It never mixes ROCm and Vulkan in one model.
- If the request sets `main_gpu`, Ollama uses that GPU (`:1027-1048`). This is a **per-request API option**; there is no server-wide equivalent.
- Otherwise, unless `OLLAMA_SCHED_SPREAD` is set, it looks for a single GPU where the predicted VRAM is ≤ 80 % of available memory (`bestSingleGPUFit`, `:1102-1118`). Ties are broken by `betterPlacementGPU` (`:1120-1126`): **discrete beats integrated, then more available memory wins**.
- Otherwise it takes the whole best library group: groups with a discrete GPU first, then the most memory (`:1128-1158`).
- For an iGPU, "available" = min(iGPU free, system free RAM) (`availableMemoryForGPU`, `:1161-1166`).
- The device order in `HIP_VISIBLE_DEVICES` does not change any of this. On Windows HIP keeps the physical order anyway [LOCAL §1].

**Consequence with the iGPU admitted [INFERRED from the code above; not yet seen in the logs]:**
- Take a model that predicts more than ~12.6 GiB (80 % of 15.8 GiB free on the 7800 XT) but less than 80 % of free system RAM (52.4 GiB free → ~42 GiB).
- No discrete GPU passes the single-GPU test, but the iGPU does.
- The ROCm and Vulkan groups cannot be combined.
- So Ollama loads the whole model on the iGPU through Vulkan, rather than spreading it across the 7800 XT and the CPU.

**Consequence for the R9700 [INFERRED]:**
- The 7800 XT (gfx1101) and the R9700 (gfx1201) are both in the ROCm group. The R9700 is in the rocBLAS support map of this Ollama build [LOCAL log: `supported="map[… gfx1200:true gfx1201:true …]"`], although `docs/gpu.mdx:77-83` only lists RDNA3 for Windows.
- A model that fits will go to whichever card has more free memory — normally the R9700 — whatever rank the user gave.
- Larger models will be split across both ROCm cards.

### 2.6 A trap for any hierarchy [INFERRED from §2.3 dedup + §2.4]

Hiding a discrete GPU from ROCm with `HIP_VISIBLE_DEVICES` does **not** hide it from Vulkan:
- Vulkan discovery still sees the card.
- Its ROCm twin is gone, so the dedup step no longer removes it.
- Ollama then uses the "excluded" GPU through Vulkan.

So excluding a dGPU needs a filter on both backends: `HIP_VISIBLE_DEVICES` and also one of `GGML_VK_VISIBLE_DEVICES`, `VK_LOADER_DEVICE_ID_FILTER` or `OLLAMA_VULKAN=0`.

Today this does not bite: with the 7800 XT + iGPU, the 7800 XT's Vulkan copy is removed as a duplicate. It **will** bite with the R9700: Corral's current `Single{ROCm, 7800}` profile would still let Ollama see the R9700 through Vulkan.

---

## 3. llama.cpp (master @ a0e6dc7)

- **`-dev/--device <dev1,dev2,..>`** — a list of backend device **names** (`ROCm0`, `Vulkan1`, …), or `none`. Environment variable `LLAMA_ARG_DEVICE`. `--list-devices` prints them [SRC `common/arg.cpp:1122-1142`, `:2740-2756`].
  - The names are positional, so `Vulkan0` changed meaning on this PC when the loader order flipped.
- **Default device selection:** all `GPU`-type devices in registry order, deduplicated by `props.device_id` across backends, RPC servers first. **`IGPU`-type devices are added only if no discrete GPU was found** [SRC `src/llama.cpp:185-286`, the rule is at `:281-285`].
- **Split options:**
  - `-sm/--split-mode {none,layer,row,tensor}`; `layer` is the default [SRC `common/arg.cpp:2820-2842`].
  - `-ts/--tensor-split` gives proportions (`:2844-2869`).
  - `-mg/--main-gpu` is the GPU for split-mode none, or for intermediate results with row split (`:2871-2879`).
  - `-fit/--fit` adjusts unset parameters to fit device memory (`:2881-2893`).
- **ggml-vulkan** [SRC `ggml/src/ggml-vulkan/ggml-vulkan.cpp`]:
  - `GGML_VK_VISIBLE_DEVICES` is a comma- or space-separated list of indices into `vkEnumeratePhysicalDevices`. They are kept **in the given order**, and an out-of-range index throws (`:5277-5293`).
  - Without the variable, ggml keeps every supported **discrete and integrated** GPU, deduplicated by UUID/LUID with a driver priority (`:5300-5401`). It falls back to the first non-CPU device (`:5403-5414`).
  - An integrated Vulkan device is reported as `GGML_BACKEND_DEVICE_TYPE_IGPU` (`:15356`), so llama.cpp's loader skips it when a dGPU exists.
- The `--list-devices` names `Vulkan<N>` follow ggml's own list. That list equals the raw loader indices only while ggml skips no device [INFERRED]. True here (AMD only, no CPU Vulkan device).

**bipolar-code** (`backend/app/services/llamacpp_service.py:73-204`):
- It parses `--list-devices`, keyed by name (`Vulkan0`).
- `"auto"` drops iGPUs by regex/type (`:31-45`, `:108-117`).
- A saved list such as `["Vulkan0"]` would now point at the iGPU.
- On this PC the `llamacpp` provider is not configured (no exe/model in `C:\litellm\providers.json`). So bipolar-code's "uses the iGPU" symptom is probably on the graphics side (browser/WebView) [INFERRED].

---

## 4. Vulkan loader on Windows (applies to Ollama-Vulkan, llama.cpp-Vulkan, LM Studio Vulkan, games)

- **Ordering:** the loader sorts physical devices by `IDXGIFactory6::EnumAdapterByGpuPreference(i, DXGI_GPU_PREFERENCE_UNSPECIFIED)`, matching on LUIDs [SRC `loader/loader_windows.c:856-932`]. The companion note covers DXGI order.
  - This is why `Vulkan0` became the iGPU: the primary desktop moved to the iGPU's HDMI port [INFERRED + companion note §0].
  - `VK_LOADER_DEVICE_SELECT` / `VK_LOADER_DISABLE_SELECT` are **Linux only** [DOC `docs/LoaderInterfaceArchitecture.md:770-798`].
- **Filtering (cross-platform):** the loader filters enumerated physical devices by `deviceID`, `vendorID` or `driverID` [SRC `loader/loader.c:8259-8300`].
  - Variables: `VK_LOADER_DEVICE_ID_FILTER`, `VK_LOADER_VENDOR_ID_FILTER`, `VK_LOADER_DRIVER_ID_FILTER`. They need a loader built with headers ≥ 1.4.326 [DOC `docs/LoaderInterfaceArchitecture.md:998-1016`].
  - Verified with loader 1.4.341 [LOCAL §1].
  - Limitation: two identical cards share a device ID.
  - Doc: https://github.com/KhronosGroup/Vulkan-Loader/blob/main/docs/LoaderInterfaceArchitecture.md
- **AMD implicit layer** `VK_LAYER_AMD_switchable_graphics`:
  - Present twice; the duplicate is removed by the loader. Can be disabled with `DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1=1` [LOCAL manifest `amdvlk.inf_amd64_914ba89eaaafdf60\amd-vulkan64.json`].
  - Disabling it does not change the order on this desktop [LOCAL].

---

## 5. PyTorch with ROCm on Windows

- AMD's HIP docs: "On Linux, use `ROCR_VISIBLE_DEVICES`. On Windows, use `HIP_VISIBLE_DEVICES`."
  - `HIP_VISIBLE_DEVICES` takes "device indices exposed to HIP applications".
  - `CUDA_VISIBLE_DEVICES` "has the same effect as `HIP_VISIBLE_DEVICES` on the AMD platform".
  - [DOC https://rocm.docs.amd.com/projects/HIP/en/latest/reference/env_variables.html, https://rocm.docs.amd.com/en/latest/reference/system-optimization/gpu-isolation.html]
  - The docs warn that environment variables are not an isolation boundary.
- **On Windows it is a filter, not a reorder** [LOCAL §1]. `torch.cuda` device 0 is the first visible device in HIP's own order.
- The HIP order is not stable:
  - Today: 7800 XT = 0, iGPU = 1 (PCI order).
  - 2026-08-29: iGPU gfx1036 = 0 (user memory `reference_image_to_3d_engines_2026_08.md`).
  - A wrong device 0 does not fail at startup. It crashes on the first kernel with a stack trace that never mentions the GPU (same memory note).
- `torch.cuda.get_device_properties(i)` exposes `pci_bus_id` and `gcnArchName` [LOCAL], so a launcher can turn "PCI bus 3" into a HIP index.
- A GPU chooser for a PyTorch app should, at launch time:
  1. Resolve the chosen GPU's PCI bus to its HIP index (see §10.2: use the HIP runtime's `hipDeviceGetPCIBusId`, like Ollama does).
  2. Set `HIP_VISIBLE_DEVICES=<that index>`.

---

## 6. ONNX Runtime: DirectML and Windows ML

- **DirectML EP `device_id`:**
  - Docs: "corresponds to the enumeration order of hardware adapters" (`IDXGIFactory::EnumAdapters`), and "A `device_id` of 0 always corresponds to the default adapter". They warn that "the primary display (GPU 0) is often not the most performant one".
  - DirectML "is in sustained engineering" [DOC https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html].
  - Code: `EnumAdapters1(device_id)` [SRC `onnxruntime/core/providers/dml/dml_provider_factory.cc:499-516`].
  - **On this PC `device_id=0` is now the iGPU** [LOCAL §1].
- **No `device_id`:**
  - DML builds a DXCore adapter list and sorts it `HighPerformance` [SRC `dml_provider_factory.cc:230-251`, `:460-497`].
  - Options `performance_preference=high_performance|minimum_power` and `device_filter` (any, gpu, npu) [SRC `:379-434`].
  - On this PC the DXCore order is 7800 XT first under every sort [LOCAL]. So the default path picks the 7800 XT, but only because DXCore lists the 7800 XT first [INFERRED].
- **Windows ML / ORT "EP devices":**
  - Apps enumerate `GetEpDevices()` and append a chosen device, or set a policy (`PREFER_GPU`, `MAX_PERFORMANCE`, `MAX_EFFICIENCY`, `PREFER_NPU`, `MIN_OVERALL_POWER`, `PREFER_CPU`, `DEFAULT`). "List of devices can dynamically change" [DOC https://learn.microsoft.com/en-us/windows/ai/new-windows-ml/select-execution-providers].
  - Device metadata on Windows: `LUID`, `DxgiAdapterNumber`, `DxgiVideoMemory`, `DxgiHighPerformanceIndex`, and `Discrete` = `!DXCore IsIntegrated` [SRC `onnxruntime/core/platform/windows/device_discovery.cc:346-428`, `:524-528`].
  - Policy sort order: NPU before GPU before CPU, then discrete first, then vendor match, then an "arbitrary" order by EP name with unstable `std::sort` [SRC `onnxruntime/core/session/provider_policy_context.cc:37-48`, `:84-141`, `:530-545`].
  - **Here the iGPU reports `IsIntegrated=0`, so ORT tags it `Discrete=1`** [LOCAL + SRC → INFERRED]. With two "discrete" GPUs, `PREFER_GPU` / `MAX_PERFORMANCE` have no defined tie-break.
  - A chooser can't steer ORT apps from outside except through what they read: DXGI order, the per-app preference (companion note) and the app's own `device_id` setting.
  - User measurement 2026-10-07: on this AMD box the Windows ML 2.7 catalog only offers the WebGPU EP (memory `reference_windows_ml_amd_2026_10.md`).

---

## 7. Blender Cycles (HIP)

- Manual (5.2 LTS) [DOC https://docs.blender.org/manual/en/latest/render/cycles/gpu_rendering.html; source `manual/render/cycles/gpu_rendering.rst:49-73`, `:147`]:
  - HIP needs "RDNA1 architecture or newer. Both discrete GPUs and APUs are supported", on Windows with Radeon Software ≥ 24.9.1.
  - Devices are chosen in *Preferences → System → Cycles Render Devices*: pick a backend, then tick each device.
- Command line: `-- --cycles-device HIP` (or `HIP+CPU`) picks the **backend** only, not the device [DOC `manual/advanced/command_line/arguments.rst:86-96`].
- Device IDs are stable and PCI-based: `HIP_<name>_<domain>:<bus>:<device>` [SRC `intern/cycles/device/hip/device.cpp:198-206`]. Blender saves the ticked devices by these IDs in userpref [INFERRED].
- Any device with gfx major ≥ 10 is listed [SRC `intern/cycles/device/hip/util.h:62-69`]. Kernels ship for gfx1036 and gfx1201 [SRC `CMakeLists.txt:713-719`]. So the iGPU and the R9700 will both appear.
- A device with a kernel timeout gets a " (Display)" suffix [SRC `device.cpp:220-236`].
- `HIP_VISIBLE_DEVICES` should also hide devices from Blender, since it uses the HIP runtime [INFERRED].

---

## 8. LM Studio

- Runtime here: `llama.cpp-win-x86_64-vulkan-avx2` 2.57.0 [LOCAL `backend-preferences-v1.json`].
- GPU controls (since 0.3.14; Ctrl+Shift+H) [DOC https://lmstudio.ai/changelog/lmstudio-v0.3.14]:
  - "Enable/disable individual GPUs".
  - "Priority order" ("allocate more on GPUs listed first").
  - "Limit Model Offload to Dedicated GPU memory".
  - 0.3.14 lists these as **CUDA-specific**.
- 0.4.16 fixes "multi-GPU selection bugs affecting GPU ON/OFF and Priority Order on some CUDA 12, ROCm, and Vulkan setups" [DOC https://lmstudio.ai/changelog/lmstudio-v0.4.16].
- LM Studio saves the per-engine choice **as Vulkan indices** (`visibleDevices: [1, 0], changesOrder: true`). When the order flipped, it re-surveyed and rewrote the list so the 7800 XT stays first [LOCAL; the re-mapping logic is INFERRED].
- From outside, a chooser can only set launch-time environment for LM Studio: `VK_LOADER_DEVICE_ID_FILTER` (index-free) or `GGML_VK_VISIBLE_DEVICES`. These may conflict with LM Studio's own saved indices [INFERRED].

---

## 9. What a per-app GPU chooser can actually set

| Stack | Lever | Stable key? | Scope | Notes |
|---|---|---|---|---|
| D3D11/12, DXGI-default apps, Chromium | `HKCU\…\DirectX\UserGpuPreferences` per exe | yes (SpecificAdapter = VEN&DEV&SUBSYS) | per exe | companion note §2-4 |
| Any Vulkan app (loader ≥ 1.4.326) | `VK_LOADER_DEVICE_ID_FILTER` env at launch | **yes (PCI device ID)** | per process tree | identical cards can't be told apart. Also hides the iGPU from Vulkan games |
| ggml-vulkan (Ollama, llama.cpp, LM Studio) | `GGML_VK_VISIBLE_DEVICES` | no (loader index) | per process | resolve at launch: Vulkan `deviceLUID` ↔ DXGI LUID ↔ PCI |
| HIP (PyTorch-ROCm, Ollama-ROCm, Blender HIP) | `HIP_VISIBLE_DEVICES` | no (HIP index; filter only on Windows) | per process | resolve at launch with `hipDeviceGetPCIBusId` |
| ONNX Runtime DirectML | app's `device_id` | no (EnumAdapters1 index) | app setting | follows DXGI order, which the per-app preference also affects (companion note §3.4) |
| Ollama | `OLLAMA_IGPU_ENABLE`, `OLLAMA_VULKAN`, `OLLAMA_LLM_LIBRARY`, `OLLAMA_SCHED_SPREAD` + the HIP/GGML filters | `OLLAMA_*` are index-free | server process | per-request `main_gpu` exists but is index-based within a group |
| llama.cpp | `--device`, `LLAMA_ARG_DEVICE`, `-sm`, `-ts`, `-mg` | no (names are positional) | per process | |
| Blender | userpref device ticks | yes (`HIP_<name>_<pci>`) | Blender profile | `--cycles-device` picks the backend only |

**Design implication [INFERRED]:** the cleanest per-app compute control is a **launcher**:
- The chooser starts the app with a per-process environment block.
- It resolves the GPU indices right before `CreateProcess`.
- The alternative, user-wide `HKCU\Environment`, has two problems:
  - It leaks into every app at the next logon.
  - It goes stale when indices move.

---

## 10. Corral today (code read at `C:\personal\corral`, HEAD `6434f39`)

### 10.1 What exists

| Piece | Where | What it does |
|---|---|---|
| Profile type | `src-tauri/src/config.rs:4-11` | `GpuProfile = Auto \| Spread \| Single{library, filter_id}`, plus `igpu_enabled: bool` (`:32`, default false `:47`) |
| Profile can't change via plain save | `config.rs:138-143`, `commands.rs:161-172` | `save_config` rejects GPU changes ("requires Apply and restart") |
| GPU list | `ollama_gpus.rs:21-83` | Parses `server.log` lines `inference compute` and `dropping integrated GPU` within 10 s of the newest one. Fields: id, filter_id, library, description, kind, total. **No `pci_id`** |
| Env mapping | `ollama_gpus.rs:5-8`, `:85-106` | Manages 6 vars: `OLLAMA_SCHED_SPREAD`, `HIP_VISIBLE_DEVICES`, `ROCR_VISIBLE_DEVICES`, `CUDA_VISIBLE_DEVICES`, `GGML_VK_VISIBLE_DEVICES`, `OLLAMA_IGPU_ENABLE`. `Single` sets exactly one visibility var to `filter_id`. `OLLAMA_IGPU_ENABLE=1` if enabled, otherwise **removed** (never `0`) |
| Validation | `ollama_gpus.rs:108-125` | `Single` must name a (library, filter_id) present in the last discovery |
| Persistence | `user_env.rs:1-43` | `reg add/delete HKCU\Environment` for each var, with a hidden window. No `WM_SETTINGCHANGE` broadcast |
| Apply flow | `commands.rs:187-229` | `gpu_profile_apply` lock → validate against the discovered GPUs → compute env → save config.toml → persist HKCU → take the `lifecycle` lock → pause (unload models, kill Ollama processes, wait for exit) → resume |
| Resume | `commands.rs:69-80`, `control.rs:112-135` | Recomputes env from config **each time**. Launches `ollama app.exe` (or `ollama serve`) with set/remove applied to the child env (`control.rs:43-78`). If Ollama already answers `/api/version` within 2 s, nothing is launched |
| Telemetry adapters | `gpu/windows.rs:101-124`, `gpu/mod.rs:52-54` | DXGI `EnumAdapters1`, total = `DedicatedVideoMemory`. Filters software adapters and the name "IddSampleDriver". **Misses the IDD adapter that DXGI names "AMD Radeon RX 7800 XT"** (companion note §10.5) |

### 10.2 Problems that matter for a hierarchy and for the R9700

1. **`Single` stores an index.** `filter_id` "0" today is the 7800 XT in ROCm numbering. After the R9700 goes in, HIP numbering can change. The validation only checks that index 0 still exists, not that it is the same card [SRC `ollama_gpus.rs:108-115`].
2. **`Single{ROCm,…}` filters only HIP.** With the R9700 present, the excluded card comes back through Vulkan (§2.6).
3. **User-wide persistence:**
   - `HIP_VISIBLE_DEVICES` / `GGML_VK_VISIBLE_DEVICES` in `HKCU\Environment` reach every HIP and ggml-vulkan app (PyTorch, Blender, LM Studio) from the next logon.
   - Without a `WM_SETTINGCHANGE` "Environment" broadcast, Explorer and apps started from it keep the old values until then. Microsoft documents the broadcast for exactly this purpose [DOC https://learn.microsoft.com/en-us/windows/win32/procthread/environment-variables].
   - The Ollama FAQ tells users to set server variables there and start Ollama from the Start menu [DOC `docs/faq.mdx:113-127`]. So an Ollama started at logon will also use the stored (possibly stale) indices.
4. **The iGPU flag is a single global boolean.** It maps to `OLLAMA_IGPU_ENABLE=1`, which today admits a 105 GiB Vulkan iGPU into Ollama's scheduler (§2.5).
5. **Identity from the Ollama log is partial:**
   - No `pci_id` is parsed.
   - Vulkan entries have no PCI on Windows.
   - Devices hidden by the current profile do not appear in the log at all. So Corral can't build a full inventory from Ollama's log alone.

---

## 11. Mapping a user-ordered GPU hierarchy onto Ollama

### 11.1 Data model (config.toml) [proposal]

```toml
[gpu_hierarchy]
mode = "allowed"            # "first_only" | "allowed" | "spread"
new_discrete = "enabled"    # what to do with a GPU seen for the first time: "enabled" | "disabled"

[[gpu_hierarchy.devices]]   # list order = user rank
key = "pci:0000:03:00.0"    # stable identity (see 11.2)
hwid = "1002:747e:2427148c" # vendor:device:subsystem, used to re-match if the bus number moves
name = "AMD Radeon RX 7800 XT"
kind = "discrete"           # from Vulkan deviceType / HIP integrated attr; NOT from DXCore (it lies here)
enabled = true

[[gpu_hierarchy.devices]]
key = "pci:0000:0f:00.0"
hwid = "1002:164e:88771043"
name = "AMD Radeon(TM) Graphics"
kind = "integrated"
enabled = false             # iGPU excluded by default
```

`GpuProfile` and `igpu_enabled` become a migration source:
- `Auto` → all discrete GPUs enabled, iGPU = `igpu_enabled`.
- `Spread` → same, with `mode = "spread"`.
- `Single` → only the GPU whose current (library, filter_id) resolves to a PCI key, `mode = "first_only"`.

### 11.2 Inventory: building stable keys (pure merge of three probes)

1. **PnP / DXGI:**
   - Corral already enumerates DXGI with LUIDs.
   - Add PCI bus/device/function per LUID: `D3DKMTQueryAdapterInfo(KMTQAITYPE_ADAPTERADDRESS)` gives `D3DKMT_ADAPTERADDRESS` [DOC, untested here]. Hardware IDs come from `DXGI_ADAPTER_DESC1` Vendor/Device/SubSys.
   - Drop display-only adapters (the IDD) with the `KMTQAITYPE_ADAPTERTYPE` check the companion note describes.
   - **LUIDs change on every boot** (LM Studio's surveys show a new LUID each session) [LOCAL]. Use them only within one probe, never as a stored key.
2. **HIP:**
   - Load Ollama's bundled `lib\ollama\rocm_v7_1\amdhip64_7.dll`, exactly as Ollama does [SRC `discover/native_probe_windows.go:242-300`].
   - For each index, read `hipDeviceGetPCIBusId`, the name, total memory and the integrated attribute.
   - Run it in a **child process with `HIP_VISIBLE_DEVICES` removed**, so Corral's own inherited env can't hide devices. Result: HIP index ↔ PCI.
3. **Vulkan:**
   - `vkEnumeratePhysicalDevices` with the system loader and a clean env.
   - Read `VkPhysicalDeviceIDProperties.deviceLUID` (joins DXGI) and `deviceType` (the honest integrated flag).
   - Result: Vulkan index ↔ LUID ↔ PCI.
   - Caveat [INFERRED]: Ollama's process sees the same order only if its loader env (`VK_LOADER_*`), implicit layers and DXGI order are the same. A per-exe GPU preference on `ollama.exe` could change the DXGI order for that process. Re-check after launch against the `inference compute` lines (§11.5).
4. **Match stored entries to the inventory:**
   - First by `key` (PCI location).
   - If none matches, by a unique `hwid`; the bus may move when a card is added [INFERRED: PCIe bus numbers are assigned at enumeration].
   - If still ambiguous (two identical cards that both moved), mark the entry "needs confirmation" and do not apply index-based filters.

### 11.3 Pure function: hierarchy → environment

`hierarchy_env(hierarchy, inventory) -> Vec<(String, Option<String>)>`

Keep the `(name, Option<value>)` contract so `control::apply_launch_env` and `user_env::persist` keep working. Rules:

1. `enabled_present` = devices that are enabled and found in the inventory, in rank order.
2. `OLLAMA_IGPU_ENABLE`:
   - `"1"` if any enabled device is integrated.
   - Otherwise **`"0"`**: an explicit false, which survives a future change of Ollama's default and also blocks ROCm iGPUs.
3. ROCm devices = enabled devices that HIP can see:
   - `first_only`: `HIP_VISIBLE_DEVICES` = the HIP index of rank 1. If rank 1 is not ROCm-capable, `HIP_VISIBLE_DEVICES=-1` so ROCm sees nothing.
   - Otherwise, if **every** ROCm-capable discrete GPU in the inventory is enabled → leave `HIP_VISIBLE_DEVICES` unset. This is the **index-free default**, and it covers "all dGPUs, iGPU excluded".
   - Otherwise `HIP_VISIBLE_DEVICES` = the enabled HIP indices, ascending. Order is irrelevant on Windows.
4. Vulkan:
   - If no enabled device needs Vulkan (each enabled device has ROCm) and some dGPU was excluded from ROCm in step 3 → `OLLAMA_VULKAN=0`. Alternative: `VK_LOADER_DEVICE_ID_FILTER` with the enabled device IDs. Either way the excluded card can't come back via Vulkan (§2.6).
   - If some enabled device needs Vulkan (the iGPU when enabled, or a card without rocBLAS) → `GGML_VK_VISIBLE_DEVICES` = the Vulkan indices of the enabled devices only.
   - If nothing was excluded → leave the Vulkan vars unset.
5. `OLLAMA_SCHED_SPREAD = "1"` only for `mode = "spread"`.
6. Manage the full var set every time, so stale values are removed:
   - Add `OLLAMA_VULKAN` and, if used, `VK_LOADER_DEVICE_ID_FILTER` to `MANAGED_VARS`.
   - Drop `ROCR_VISIBLE_DEVICES` and `CUDA_VISIBLE_DEVICES` on Windows. Keep removing them, since `CUDA_VISIBLE_DEVICES` is also read as a ROCm filter on Windows [SRC `ml/device.go:463-470`].

What the UI must say about rank [from §2.5]:
- In `allowed`/`spread` mode Ollama chooses among the allowed GPUs by free memory, discrete first. The rank only matters in `first_only`.
- A model never spans ROCm + Vulkan. So enabling the iGPU next to the dGPUs adds a slow single-GPU target for big models, not extra memory for split models.

### 11.4 Persistence and restart (fits the existing apply flow)

- Keep `apply_gpu_profile_with` as is: lock → validate → save config.toml → persist → `lifecycle` lock → pause/resume [SRC `commands.rs:187-208`]. Only the env computation changes.
- `resume_action` already recomputes the env from config on every launch [SRC `commands.rs:69-80`]. Make it recompute **from a fresh inventory**, so indices are correct for this boot even when nobody pressed Apply.
- **Persist only index-free vars to HKCU** (`OLLAMA_IGPU_ENABLE`, `OLLAMA_VULKAN`, `OLLAMA_SCHED_SPREAD`), and broadcast `WM_SETTINGCHANGE("Environment")` after writing.
- Keep the index-based vars (`HIP_VISIBLE_DEVICES`, `GGML_VK_VISIBLE_DEVICES`) **process-scoped** in Corral's launcher. They must not leak to PyTorch, Blender or LM Studio [INFERRED, §10.2-3].
  - Trade-off: an Ollama started outside Corral (Start menu, logon autostart) won't get the dGPU exclusion.
  - Mitigation: at Corral start, compare the running Ollama's `inference compute` set with the expected one. If they differ, offer "restart Ollama with the hierarchy", or restart automatically if the user opted in.
- Remove, on first run of the new version, any index-based vars an older Corral wrote to HKCU.

### 11.5 Add / remove / reorder events (R9700)

Detection:
- The tray poller already reads DXGI every tick (`gpu/windows.rs`).
- Compare the set of **stable keys**, not LUIDs or indices. A change triggers a notice and a fresh inventory.

New GPU (R9700, `DEV_7551`, gfx1201):
- Append it at the end of the list.
- `enabled` follows `new_discrete`. A new integrated device is always disabled.
- If the effective env is the index-free default (all dGPUs, no HIP filter), Ollama will use the R9700 after the next restart. It will usually prefer the R9700 for single-GPU loads because it has the most free memory, whatever its rank (§2.5).
- If the effective env contains HIP indices, recompute them: the new card may take index 0.
- Then restart Ollama under the lifecycle lock, because discovery runs once per server process (§2.1).
- Verify after restart: the `inference compute` lines must list exactly the expected set by `pci_id` (ROCm) and description (Vulkan). If not, roll back to the index-free default and notify.

Removed GPU:
- Keep the entry, shown as "absent", so its rank and choice survive re-installation.
- Recompute indices: when a card disappears, every higher HIP/Vulkan index shifts down. A stale `HIP_VISIBLE_DEVICES=1` would silently select a different card — possibly the iGPU, which ROCm then drops for lacking rocBLAS, leaving Ollama on CPU.
- If no enabled device remains, fall back to "CPU" or to the index-free default, after asking the user.

Bus renumbering:
- Adding a card may change the PCI bus numbers of existing devices [INFERRED].
- Re-match by `hwid` (unique) and update the stored `key` silently.
- Two identical cards whose buses both changed → ask the user.

DXGI/Vulkan order flips (monitor moved to another GPU, like today):
- No action is needed with stored keys.
- Indices are recomputed at the next launch.
- Corral should flag it, because Vulkan-index users outside Corral (LM Studio, bipolar-code `--device`) are affected.

### 11.6 Testability (fits the user's "pure functions" rule)

- `merge_inventory(dxgi, hip, vulkan) -> Vec<GpuIdentity>` is pure. Unit-test it with today's measured tables, plus a synthetic R9700 at bus 03 that pushes the 7800 XT to bus 04.
- `match_hierarchy(stored, inventory) -> Vec<Resolved>` is pure. Cases: exact key, moved bus, identical twins, missing device.
- `hierarchy_env(resolved, mode) -> Vec<(String, Option<String>)>` is pure. Cases: default (no indices), exclude 7800 XT with the R9700 present (HIP index + `OLLAMA_VULKAN=0`), iGPU only (Vulkan index + `OLLAMA_IGPU_ENABLE=1` + `HIP_VISIBLE_DEVICES=-1`), spread.
- `parse_ollama_gpus` should also capture `pci_id`. The post-restart check compares the expected and observed sets.

---

## 12. Open questions

1. Does a per-exe `UserGpuPreferences` entry for `ollama.exe` / `llama-server.exe` / `python.exe` change the DXGI (and so Vulkan) order inside that process? The companion note infers yes (§3.4). A Vulkan probe run under the same exe name would confirm it. Untested, because testing needs a registry write.
2. Is HIP's Windows device order PCI order, adapter-enumeration order, or something else? It flipped between August and October. Measuring needs a probe before and after the R9700 install.
3. Does `GGML_VK_VISIBLE_DEVICES=-1` cleanly disable Vulkan in Ollama 0.40.4, or does ggml throw? `-1` parsed as `size_t` exceeds the device count [SRC `ggml-vulkan.cpp:5285-5291`]. Prefer `OLLAMA_VULKAN=0`.
4. Does `HIP_VISIBLE_DEVICES=-1` (ROCm sees nothing) make Ollama's ROCm bootstrap fail fast, or does it hit the 30 s watchdog? Today's log already shows one watchdog timeout with `HIP_VISIBLE_DEVICES:0`.
5. Will Ollama's Windows ROCm path really accept the R9700? The runtime's rocBLAS map lists gfx1201 [LOCAL], but `docs/gpu.mdx:77-83` only lists RDNA3 for Windows.
6. Does `D3DKMTQueryAdapterInfo(KMTQAITYPE_ADAPTERADDRESS)` return the same bus numbers as HIP's `hipDeviceGetPCIBusId` and the bytes in Vulkan's deviceUUID? The last two agree today (3 and 15) [LOCAL]; KMT is unverified.
7. Is DXCore's `IsIntegrated=0` for the iGPU caused by the 16 GB UMA carve-out (the companion note's hypothesis), or was it always so on this desktop? No pre-change baseline exists.
