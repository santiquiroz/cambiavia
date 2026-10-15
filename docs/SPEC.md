# Cambiavía: design spec (v0.1)

**Date:** 2026-10-14
**Status:** proposed, pending the owner's review
**Repo (to create):** `santiquiroz/cambiavia` (local: `C:\personal\cambiavia`)
**License:** AGPL-3.0-only (same as Corral)
**UI language:** Spanish. **Code, comments, docs:** English.
**Research this spec is built on:** `C:\personal\gpu-app-design\research\`
`windows-gpu-preference.md`, `prior-art.md`, `compute-stacks.md`, `game-discovery.md`, plus the read-only probes in `research\probes\` and `research\tools\`.

Tags used below: **[DOC]** vendor documentation, **[SRC]** source code at file:line, **[LOCAL]** measured read-only on the reference PC, **[COMMUNITY]** third-party tools or posts, **[INFERRED]** reasoning not yet verified.

---

## 1. Problem

Reference PC: Windows 11 Pro build 26300, Ryzen 9 7900X3D with its iGPU ("AMD Radeon(TM) Graphics", `1002&164E&88771043`, 16 GB UMA carve-out set in BIOS), RX 7800 XT 16 GB (`1002&747E&2427148C`). A Radeon AI PRO R9700 32 GB (`DEV_7551`) will be added.

Since the UMA change, most apps render on the iGPU. The research established why [LOCAL, `windows-gpu-preference.md` §1, §3]:

1. **Adapter 0 is the iGPU.** `IDXGIFactory::EnumAdapters` returns the adapter that shows the desktop primary at index 0 [DOC https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgifactory-enumadapters]. The primary monitor (`\\.\DISPLAY6`, 2560x1440 at 0,0) is on the motherboard HDMI, so it belongs to the iGPU. D3D11/D3D12 with a NULL adapter, Chromium/Edge/Electron/WebView2 compositing and the Vulkan loader's device order all follow adapter 0 [DOC, SRC].
2. **"High performance" also means the iGPU here.** `EnumAdapterByGpuPreference(HIGH_PERFORMANCE)` and `(MINIMUM_POWER)` both return the iGPU first [LOCAL]. Windows does not see a hybrid pair: neither adapter has the `HybridDiscrete`/`HybridIntegrated` kernel flags, DXCore says `IsIntegrated=False` for both, and the 7800 XT is the POST device with outputs, which the WDDM hybrid model excludes [DOC https://learn.microsoft.com/en-us/windows-hardware/drivers/display/validating-a-hybrid-system-configuration]. So per-app "High performance" (`GpuPreference=2`) does not help.
3. **The iGPU looks like the biggest GPU.** DXGI reports 15.97 GiB `DedicatedVideoMemory` for the iGPU against 15.80 GiB for the 7800 XT [LOCAL]. Engines that rank by memory (gow2-recomp's fallback, Godot's D3D12 driver [SRC godot `drivers/d3d12/rendering_context_driver_d3d12.cpp:187-188`, INFERRED]) pick the iGPU.
4. **A virtual display clones the 7800 XT's identity.** The IddSampleDriver adapter (LUID `0x2764F`, kernel type `0x342` = IndirectDisplayDevice, no RenderSupported) is reported by DXGI as a second "AMD Radeon RX 7800 XT" with the same VEN/DEV/SUBSYS [LOCAL `probes\kmt_types.py`]. Tools that filter by name list the card twice (Corral does, `src-tauri/src/gpu/mod.rs:52-54`).
5. **Compute stacks ignore all of this** and pick devices by positional indices that moved after the BIOS change (Vulkan iGPU went from `Vulkan1` to `Vulkan0` between 2026-10-12 and 2026-10-13) [LOCAL Ollama logs, `compute-stacks.md` §0].

Windows Settings can fix part of it (Settings > System > Display > Graphics: "Default high performance GPU" and per-app "Specific GPU"), one exe at a time, with no game discovery, no undo, no verification, no explanation of the root cause and nothing for compute tools. Of about 18 community tools surveyed, none orders 3+ GPUs, only a few write "Specific GPU" safely, and the one closest to right (Yukina's GPU Preference Manager) would refuse "Specific GPU" for the 7800 XT on this PC because it treats the IDD clone as a second physical card [INFERRED from SRC `DisplayConfigAdapterIdentityResolver.cs:65`, which only marks `ROOT\DISPLAY\` paths as virtual while this IDD is `ROOT\IDDSAMPLEDRIVER\0000`; `prior-art.md` verification].

## 2. Goal and success criteria

Cambiavía is a small native-feeling Windows app that lets the user say **which GPU each game or app uses** and **which GPU is the default for games**, explains why Windows picked the wrong one, and proves that a change worked.

Success criteria for v0.1, on the reference PC:

1. From the first screen the user sees which GPU Windows uses as adapter 0 and as "high performance", and why (primary monitor, UMA, missing hybrid flags).
2. One click sets the system "Default high performance GPU" to the 7800 XT; the next new process of the probe reports `HIGH_PERFORMANCE[0]` = 7800 XT (gated by Stage 0, §13).
3. Choosing "RX 7800 XT" for a Steam game writes the right exe (not its launcher or stub), and the next launch is verified on the 7800 XT through PDH GPU Engine counters.
4. Every change can be previewed, undone and exported; the rest of `UserGpuPreferences` stays byte-identical.
5. When the R9700 is installed, the app notices it, asks where it goes in the hierarchy and re-applies the rules that follow the hierarchy, in one undoable batch.
6. Idle in the background: under 60 MB RAM and under 1 % CPU (same budget as Corral).

## 3. Name

Three candidates, checked on 2026-10-14 with `gh search repos <name>` and a web search:

| Candidate | Meaning | Availability | Verdict |
|---|---|---|---|
| **Cambiavía** (`cambiavia`) | Railway switch: the piece that sends each train to its track. Here, each app to its GPU. | `gh search repos cambiavia`: 0 results. Web search: no software product with that name. | **Chosen** |
| Cauce | Riverbed, channel ("darle cauce") | 8+ small repos, including `hypereikon-lab/ComfyUI-Cauce`, a ComfyUI node pack: same AI/GPU audience, confusion risk | Rejected |
| Aguja | Railway points ("cambio de agujas"), but first meaning is "needle" | Several small repos, including `ttrotta/aguja` (a RAG debugger) | Rejected: ambiguous metaphor |

Display name "Cambiavía"; ASCII slug `cambiavia` for the repo, the crate, the exe (`cambiavia.exe`), the CLI and the Tauri identifier `dev.cambiavia.app`. The dictionary status of the word (rae.es answered 403 to the fetch) is unverified; the word is transparent in Spanish ("cambia vía"). It fits the owner's naming style (Corral, Nodo, Lumina).

## 4. Stack decision

| Criterion | **Tauri 2 + Rust + React/TS** | WinUI 3 + C# (.NET) | WPF + C# | Rust + raw Win32/XAML Islands |
|---|---|---|---|---|
| Native Windows feel | Good, not perfect: Mica backdrop (`windowEffects: mica`, "Windows 11 Only", needs a transparent window [DOC https://v2.tauri.app/reference/config/]), Segoe UI Variable, Fluent-like CSS tokens, system accent and theme, native tray, toasts, Start-menu shortcuts | Best: real Fluent controls | Dated look without a theme library | Native but very slow to build |
| Size | Measured on Corral 0.1.0: NSIS installer 2.9 MB, MSI 4.0 MB, exe 8.3 MB [LOCAL release assets] | Needs the Windows App SDK runtime and .NET, bundled or installed; tens of MB [INFERRED, not measured] | .NET runtime | Small |
| Reuse | Shares GPU enumeration, PDH parsing, toolchain, CI and test patterns with Corral (Rust, `windows` 0.62); can borrow Rust code from rai-pal (GPL-3.0, Tauri + Rust game discovery) and win_global_gpu (AGPL-3.0) | Could borrow Yukina's MIT C# code; nothing shared with Corral | Same as WinUI | Shares Rust core only |
| Testability | Pure Rust functions + `cargo test`; Vitest + Testing Library; Playwright against mocked Tauri commands (already working in Corral) | Unit tests fine; UI automation needs WinAppDriver/Appium | Same | UI hard to test |
| Owner context | Corral is Tauri 2; same build notes (MSVC BuildTools DevShell) | New toolchain | New toolchain | — |

**Decision: Tauri 2 + Rust core + React/TypeScript UI**, the same stack as Corral. Reuse and testability outweigh the last bit of native polish, and the GPU code must be shared with Corral anyway (§6.1). Native feel comes from Mica, system theme/accent, Fluent spacing and type, keyboard-first controls, tray, toasts and a CLI in the same exe.

Known irony: Cambiavía's own WebView2 GPU process (`msedgewebview2.exe` from the shared runtime folder) renders on adapter 0, the iGPU, on this PC; a per-exe preference on `cambiavia.exe` does not reach it [LOCAL + INFERRED, `windows-gpu-preference.md` §4]. That is harmless for a settings UI. Steering it with `--use-adapter-luid=<HighPart>,<LowPart>` (decimal) through `WebviewWindowBuilder::additional_browser_args`, computed at launch and re-adding wry's default `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`, is v0.2 [SRC Chromium `ui/gl/gl_display.cc:74-90`; DOC Tauri config].

## 5. Scope

### 5.1 v0.1 features

1. **GPU inventory with live telemetry**: every GPU with stable identity, kind (discrete / integrated / virtual / software), monitors attached and which one is primary, real memory (for a UMA iGPU, "reserved from system RAM", not VRAM), live dedicated/shared memory use and 3D/compute engine utilization per GPU, top processes per GPU.
2. **Diagnosis** ("Resumen"): adapter 0, `HIGH_PERFORMANCE[0]`, `MINIMUM_POWER[0]`, primary monitor's GPU, monitors split across GPUs (DWM cross-adapter copy), UMA larger than a dGPU, apps pinned by existing values, and orphan values. Each finding has a fix button or a plain-language instruction.
3. **GPU hierarchy**: the user orders the GPUs (drag or Up/Down buttons). The top enabled present GPU is the "primary" GPU that rules can follow.
4. **System default GPU for games**: writes `HighPerfAdapter` in `DirectXUserGlobalSettings` to the primary GPU, token-safe. Because that only reaches apps that ask for high performance [DOC build 20190: "an application that asks for a high-performance GPU will by default use the high-performance GPU you specify"], the default choice for discovered games is the per-app rule "Seguir jerarquía" (follow hierarchy), with a one-click bulk apply.
5. **Per-app choice**: "Windows decide", "Ahorro de energía", "Alto rendimiento", **"GPU específica: <name>"** (Specific GPU, shown only when Windows supports it, §7.2) and "Seguir jerarquía". Applies to desktop exes (by full path) and packaged apps (by AUMID).
6. **Game discovery** from the launchers present on this PC: Steam (libraries, manifests, binary `appinfo.vdf` launch entries, EA `installerdata.xml` cross-check), Xbox/GDK (`.GamingRoot`, `MicrosoftGame.config`, Gaming Services registry cross-check), Windows' own `GameConfigStore` hints, a **"Corriendo ahora"** view from PDH GPU Engine counters, and manual add (file picker / drag-and-drop). Resolves the exe that actually renders, not the launcher (§8).
7. **Compute launch profiles**: per-tool environment variables (`HIP_VISIBLE_DEVICES`, `VK_LOADER_DEVICE_ID_FILTER`, `GGML_VK_VISIBLE_DEVICES`, `DXVK_FILTER_DEVICE_UUID`), resolved from stable GPU identity at every launch, applied only to the launched process, exposed as Start-menu shortcuts that run `cambiavia run <profile>` (§9). Ollama is left to Corral.
8. **Undo, history, import/export** of every change (§7.4, §7.5).
9. **Change detection**: new or removed GPUs (`IDXGIFactory7::RegisterAdaptersChangedEvent`, Windows 10 1809+ [DOC https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_6/nf-dxgi1_6-idxgifactory7-registeradapterschangedevent], plus a startup check) and new or removed game installs (file watchers + periodic reconciliation).
10. **Verification**: read-back after every write; a "Probar" self-test that runs the probe exe under a temporary preference; and observed verification of real apps on their next launch (§7.6).
11. **CLI** in the same exe: `probe`, `list-gpus`, `get`, `set`, `undo`, `run`, `export`, `import`, all writes with `--dry-run`.

### 5.2 Out of scope (explicit)

- **Anything outside HKCU and the app's own files.** No HKLM, no `Policies` keys (WebView2's per-app policy needs elevation even under HKCU here [LOCAL Get-Acl]), no display-class keys, no `GraphicsSettings\*Applicable` flags (third-party scripts force them; Cambiavía only reads them).
- **Drivers and vendor panels**: no driver installs or settings, no Adrenalin/NVIDIA profile editing, no `EnableMsHybrid`.
- **OpenGL override.** The only known lever is the global ICD hack (HKLM `OpenGLDrivers\MSOGL`, admin + reboot, breaks with GLOn12) [SRC nethe `select_default_GPU.bat:143-157`]. Cambiavía explains that OpenGL follows the monitor's GPU on this desktop and stops there.
- **Injection**: no proxy DLLs (SetGPU-style) or hooks; anti-cheat risk.
- **Display topology and BIOS**: Cambiavía never moves the primary monitor or changes display settings; it explains and opens `ms-settings:display`.
- **Writing a value for every exe** (the win_global_gpu anti-pattern [SRC `src/registry.rs:53-64`]); only exes the user chose or confirmed.
- **Ollama**: Corral owns Ollama's GPU choice (`C:\personal\corral\docs\superpowers\specs\2026-10-14-gpu-hierarchy-design.md`).
- **Temperatures, clocks, fan curves** (no vendor-neutral source); Linux and macOS.
- Deferred to v0.2: Epic/GOG/EA app/Ubisoft/Battle.net/Riot providers, one-shot "run on GPU X then revert", Explorer context menu, engine launch arguments (Unity `-force-device-index`, Godot `--gpu-index`), following versioned install paths automatically (Discord `app-1.0.xxxx`, WindowsApps versions, Minecraft Java runtime), steering its own WebView2, importing Corral's order.

## 6. Architecture

```
                     ┌─────────────── Rust core (no Tauri) ───────────────┐
 DXGI/D3DKMT/D3D12 ─▶│ gpu-inventory ──▶ Inventory ─┐                     │
 PDH counters ──────▶│   telemetry, watch           │                     │
                     │                              ▼                     │
 HKCU UserGpu… ◀────▶│ prefs (tokens, choice) ◀── planner ──▶ ChangeSet   │──▶ Tauri commands ──▶ React UI
                     │      ▲   store (trait)        ▲          │         │──▶ tray + toasts
                     │      │                        │          ▼         │──▶ CLI (same exe)
                     │   journal + backups      discovery    verify      │
 Steam/Xbox/GCS ────▶│   (write-ahead, undo)    (catalog)   (probe, PDH) │
 Vulkan/HIP probes ─▶│ launch (env profiles, shortcuts, run)               │
                     └─────────────── %APPDATA%\Cambiavia\ ───────────────┘
```

### 6.1 Repository layout

```
cambiavia/
  crates/gpu-inventory/     lib, shared with Corral: enumeration, identity, classification, telemetry, watch
  crates/cambiavia-core/    lib: prefs, journal, backups, discovery, diagnose, launch, profiles (no Tauri)
  crates/cambiavia-probe/   tiny bin (no window, no WebView): prints DXGI adapter order as JSON
  src-tauri/                Tauri app + CLI entry; thin commands that call cambiavia-core
  src/                      React UI (Spanish strings in src/i18n/es.ts)
  e2e/                      Playwright against the dev UI with mocked commands
  docs/                     README assets, registry-format.md, screenshots
```

Corral consumes `gpu-inventory` as a git dependency pinned to a tag (`gpu-inventory-v0.1.0`). Its first code is Corral's `gpu/windows.rs` and `gpu/pdh_names.rs`, moved and extended.

### 6.2 Modules

Every module does one thing, keeps the logic in pure functions and takes its dependencies (registry, process list, clock, PDH sampler, inventory source) as explicit parameters or traits, so it can be tested with fakes.

**`gpu-inventory`**
- `dxgi`: `EnumAdapters1` + `GetDesc1` (VEN/DEV/SUBSYS/REV, LUID, memory, flags) + `EnumOutputs` (`DXGI_OUTPUT_DESC`: device name, desktop rect, `HMONITOR` → `GetMonitorInfo` primary flag). A new factory per enumeration, because the factory snapshots the adapter set [DOC EnumAdapters remarks].
- `kmt`: `D3DKMTOpenAdapterFromLuid` + `D3DKMTQueryAdapterInfo` for `KMTQAITYPE_ADAPTERTYPE` (bit layout [DOC https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/d3dkmthk/ns-d3dkmthk-_d3dkmt_adaptertype]) and `KMTQAITYPE_ADAPTERADDRESS` (PCI bus/device/function). Measured: 7800 XT `0x230b` bus 3, iGPU `0x2303` bus 15, IDD `0x342` bus `0xFFFFFFFF`, Basic Render `0x105` [LOCAL `probes\kmt_types.py`, `probes\kmt_address.py`].
- `uma`: `D3D12CreateDevice` + `CheckFeatureSupport(D3D12_FEATURE_ARCHITECTURE)` → `UMA`. Measured: iGPU `UMA=True`, 7800 XT and IDD `UMA=False` [LOCAL `probes\d3d12_uma.py`]. This is the integrated discriminator that works here; DXCore `IsIntegrated` and the hybrid flags do not. Run only on inventory refresh, never per telemetry tick.
- `classify` (pure): `classify_adapter(raw) -> AdapterKind`: `Software` if the DXGI software flag or KMT SoftwareDevice; `Virtual` if KMT IndirectDisplayDevice, or no RenderSupported, or bus `0xFFFFFFFF`; `Integrated` if UMA; else `Discrete`. Never by name.
- `identity` (pure): `GpuKey`, `AdapterId` parse/format (§7.2), ambiguity detection.
- `telemetry`: PDH `\GPU Adapter Memory(*)\Dedicated Usage` and `Shared Usage`, `\GPU Engine(*)\Utilization Percentage` filtered to `engtype_3D` and `engtype_Compute`, `\GPU Process Memory(*)`; reuses Corral's `pdh_names` parsing (`pid_<pid>_luid_<hi>_<lo>_phys_<n>…`). 1 s cadence with the window visible, off when hidden.
- `watch`: `IDXGIFactory7::RegisterAdaptersChangedEvent` on a dedicated thread; emits `AdaptersChanged`; the consumer re-enumerates.
- `merge` (pure): `merge_inventory(dxgi, kmt, uma) -> Vec<GpuIdentity>`.

**`cambiavia-core`**
- `prefs::tokens` (pure): parse and format `Name=Value;` strings (§7.1).
- `prefs::choice` (pure): `apply_choice(tokens, choice, &AdapterIdFormat) -> Vec<Token>`, `apply_high_perf(global_tokens, Option<AdapterId>) -> Vec<Token>`, `resolve_choice(choice, hierarchy, inventory) -> ResolvedChoice`.
- `prefs::store`: trait `PrefStore { list() -> Vec<(name, value)>, get(name), set(name, value), delete(name) }`; `WinRegStore` (HKCU `Software\Microsoft\DirectX\UserGpuPreferences` via `RegGetValueW`/`RegSetKeyValueW`/`RegDeleteKeyValueW`, never `reg.exe`) and `MemoryStore` for tests.
- `planner` (pure): turns user intents into a `ChangeSet` (list of `{value_name, before, after}`), refusing unsafe ones (§10).
- `journal`: write-ahead JSON-lines log, apply, recovery, undo with conflict detection (§7.4).
- `backup`: `.reg` export of the whole key (pure formatter + writer).
- `diagnose` (pure): `diagnose(inputs) -> Vec<Finding { id, level, message, fix }>` (§11.1).
- `discovery::{steam, ea, xbox, gameconfigstore, running, engine, manual, catalog}` (§8).
- `verify`: probe self-test and observed-launch verdicts (§7.6).
- `launch::{env, probes, shortcut, run}` (§9).
- `profile`: export/import schema and diff (§7.5).
- `procs`: running processes by normalized image path (for restart prompts).

**`cambiavia-probe`** prints `{exe, enum_adapters1: [{index, luid, ven, dev, subsys, outputs}], by_preference: {unspecified, minimum_power, high_performance}}`. It is copied to a temp path when a test needs a fresh value name, so it must stay tiny and window-less.

**`src-tauri`**: commands, tray, notifications (`tauri-plugin-notification`), single instance, autostart (opt-in, `--hidden`), CLI parsing before the Tauri builder starts.

### 6.3 Data model

```rust
// gpu-inventory
struct GpuKey { vendor: u16, device: u16, subsys: u32, revision: u8 }   // what Windows stores
struct PciLocation { bus: u32, device: u16, function: u16 }
enum AdapterKind { Discrete, Integrated, Virtual, Software }
struct GpuIdentity {
    key: GpuKey, pci: Option<PciLocation>, name: String, kind: AdapterKind,
    luid: u64,                       // session only: changes on reboot, driver upgrade, disable/enable [DOC DXCoreAdapterProperty InstanceLuid]
    dedicated_bytes: u64, shared_bytes: u64, uma: bool,
    outputs: Vec<OutputInfo>,        // { device_name, rect, primary }
}

// cambiavia-core
struct HierarchyEntry { key: GpuKey, pci_hint: Option<PciLocation>, label: String, enabled: bool }
struct Hierarchy { entries: Vec<HierarchyEntry> }            // order = rank

enum AppTarget { Exe(PathBuf), Aumid(String) }
enum GpuChoice { WindowsDecides, PowerSaving, HighPerformance, Specific(GpuKey), FollowHierarchy }
enum TargetOrigin { StoreManifest, EngineHeuristic, Observed, WindowsHint, User }
struct RenderTarget { target: AppTarget, origin: TargetOrigin }
struct AppEntry {
    id: String,                       // "steam:990080", "xbox:Microsoft.MinecraftUWP_8wekyb3d8bbwe", "exe:<normalized path>"
    name: String, source: Source, install_dir: Option<PathBuf>,
    launch_target: Option<String>,    // exe, URI or AUMID, informational
    render_targets: Vec<RenderTarget>,
    choice: Option<GpuChoice>,        // None = Cambiavía has no rule for it
}

struct Change { value_name: String, before: Option<String>, after: Option<String> }
struct Batch { id: Ulid, at: DateTime, reason: String, changes: Vec<Change>, state: Pending | Committed | Aborted | Conflict }

struct LaunchProfile {
    id: String, name: String, exe: PathBuf, args: Vec<String>, cwd: Option<PathBuf>,
    gpus: Vec<GpuKey>,                // in hierarchy order
    levers: Levers,                   // { hip, vulkan_device_filter, ggml_vk, dxvk }
    extra_env: Vec<(String, String)>,
}
```

Files under `%APPDATA%\Cambiavia\`: `config.toml` (hierarchy, settings), `catalog.json` (discovered apps, user overrides; a cache that can be rebuilt), `journal.jsonl` (append-only), `backups\UserGpuPreferences-<UTC>.reg` (first run and before every batch; keep the last 30 plus the first one forever), `profiles\` (launch profiles).

## 7. Applying a change

### 7.1 Registry format

Key `HKCU\Software\Microsoft\DirectX\UserGpuPreferences`. One REG_SZ per app; name = full exe path, or AUMID (`PackageFamilyName!AppId`) for packaged apps [SRC godot `platform/windows/os_windows.cpp:843-906`; COMMUNITY for AUMID]. Data = `Name=Value;` tokens. Token names Windows recognises are in `directxdatabasehelper.dll` (19 names, including `GpuPreference`, `SpecificAdapter`, `HighPerfAdapter`, `AutoHDREnable`, `AppStatus`, `SwapEffectUpgradeEnable`) [LOCAL binary strings].

| Choice | Tokens written |
|---|---|
| Windows decide | remove `GpuPreference` and `SpecificAdapter`; delete the value if nothing remains |
| Ahorro de energía | `GpuPreference=1;` |
| Alto rendimiento | `GpuPreference=2;` |
| GPU específica / Seguir jerarquía | `SpecificAdapter=<VEN>&<DEV>&<SUBSYS>;GpuPreference=1073741824;` |
| Default high performance GPU | `HighPerfAdapter=<VEN>&<DEV>&<SUBSYS>;` inside the value named `DirectXUserGlobalSettings` |

Formats confirmed by several independent tools, not by Microsoft [COMMUNITY, `prior-art.md` §4.1; SRC Yukina `SpecificAdapterKey.cs:11-12`, QGST `PreferenceStoreAdapter.cs:223`, gpu-selector `gpu_core.py:162`].

Editing rules (pure, unit-tested):
- Parse tokens; trim whitespace around each (the ASUS value reads `GpuPreference=1; ` [LOCAL]); keep unknown tokens and their order.
- Only `GpuPreference` and `SpecificAdapter` are touched in app values; only `HighPerfAdapter` in `DirectXUserGlobalSettings`. Never rewrite that value wholesale (select_default_GPU does and drops `AutoHDREnable` [SRC `select_default_GPU.bat:80-82`]).
- GPU tokens go first, in the order `SpecificAdapter`, `GpuPreference`, then the other tokens in their original order. Reason: DagorEngine reads only values that start with `GpuPreference` and takes the character after `=` [SRC `dxgi_utils.cpp:373-379`]; `GpuPreference=1073741824` first would read as "1" (power saving), and `AppStatus=0;GpuPreference=2;` reads as unspecified. Windows itself parses by token: a value on this PC reads `AppStatus=0;AutoHDREnable=2097;GpuPreference=2;` (PEAK.exe, appeared on 2026-10-14, writer unknown) [LOCAL], so order does not matter to Windows [INFERRED; confirmed by the acceptance test].
- A value Cambiavía does not change is never rewritten, so its exact bytes survive.

### 7.2 Adapter IDs and when "Specific GPU" is offered

- **Parse** `A&B&C` by splitting on `&` and reading each part as hex (case-insensitive, padded or not) into `(u16, u16, u32)`. Compare numerically.
- **Format** is not settled: community tools pad (`%04X&%04X&%08X`); `SettingsHandlers_Gpu.dll` contains `%X&%X&%X` (no padding) next to the AdvancedGraphics strings [LOCAL, INFERRED that Settings uses it]. Both forms are identical for this PC's GPUs (`1002&747E&2427148C`, `1002&164E&88771043`); they may differ for a subsystem ID with leading zeros, possibly the R9700. Rule: if any value on the machine already holds a `SpecificAdapter`/`HighPerfAdapter` that Windows wrote, copy its style; otherwise use the style Stage 0 observes from a Settings round-trip (§12). The style is a config value (`adapter_id_style = "padded" | "compact"`), so it can change without a release.
- **Offer "GPU específica"** only when `HKCU\Software\Microsoft\DirectX\GraphicsSettings\SpecificGPUOptionApplicable = 1`, and "Default high performance GPU" only when `DefaultHighPerfGPUApplicable = 1` [LOCAL: both are 1 here]. Cambiavía never writes these flags.
- **Ambiguity guard**: if two present, non-virtual, non-software adapters share a `GpuKey`, "Specific GPU" for that model is disabled with the reason "Dos tarjetas iguales: Windows no puede distinguirlas". The IDD clone is `Virtual`, so the 7800 XT is not ambiguous here; whether Windows resolves the token to the real card or to the IDD is checked by the acceptance test. The guard re-runs on every enumeration, because an IDD could later clone the R9700 [INFERRED].

### 7.3 Apply procedure (one batch)

1. **Plan**: `planner` computes the `ChangeSet` from the current registry values (read now, not from cache) and the intent. Nothing is written yet.
2. **Preview**: the UI shows each value's before/after string, the app it belongs to and what the change does not control (§7.7). The CLI prints the same with `--dry-run`.
3. **Backup**: export the whole key to `backups\` (once per batch).
4. **Journal (write-ahead)**: append the batch with `state = Pending` and fsync.
5. **Write**: for each change, `set` or `delete` through `WinRegStore`.
6. **Read back**: each value must equal `after` exactly (or be absent). Any mismatch marks the batch `Conflict` and offers immediate undo.
7. **Commit**: append `state = Committed`.
8. **Restart prompt**: list running processes whose normalized image path equals a changed value name, with the text "El cambio se aplica la próxima vez que abras <app>". No process is closed or killed. The user can tick "Comprobar cuando lo vuelva a abrir" (default on), which arms observed verification.

A GPU preference only affects processes started after the change, because a D3D device is bound to one adapter and the DXGI factory snapshots adapters [DOC; NVIDIA: "you must restart the program" https://www.nvidia.com/content/Control-Panel-Help/vLatest/en-us/mergedProjects/nv3D/Manage_3D_Settings_(reference).htm].

### 7.4 Journal and undo

- Recovery at startup: for a `Pending` batch, compare each value with `before`/`after`: all equal to `after` → `Committed`; all equal to `before` → `Aborted`; mixed → `Conflict`, shown in Historial.
- **Undo a change**: if the current value equals `after`, write `before` (delete when `before` is absent). If it differs (the user or Windows changed it since), show before/after/current and let the user choose "Restaurar de todos modos" or skip. Undo is itself a journaled batch.
- **Undo a batch** undoes its changes in reverse order. "Deshacer todo desde <fecha>" undoes batches newest first.
- Backups are the last resort: Historial has "Abrir carpeta de copias" and the `.reg` files import with Windows' own `reg import` if everything else fails.

### 7.5 Import and export

- **Export** (`cambiavia export <file>`, Historial → Exportar): JSON `{schema: 1, exported_at, adapter_id_style, hierarchy: [{key, label, enabled}], rules: [{app_id, targets, choice}], launch_profiles: [...]}`. GPUs by `GpuKey`, never LUID or index. Steam targets also carry `appid` and the path relative to the library, so they can be remapped to another library or PC.
- **Import** computes the `ChangeSet` against the current machine, shows the preview (including rules whose GPU or exe is missing, listed as skipped) and applies it as one undoable batch.
- A raw `.reg` export of the key is also available, for people who prefer Windows' format.

### 7.6 Verification

Three levels, shown on every app row:

1. **Escrito** (written): read-back passed (§7.3 step 6).
2. **Probado** (self-test, button "Probar"): copy `cambiavia-probe.exe` to `%TEMP%\cambiavia-test-<ulid>\probe.exe`, write the same tokens for that path, run it, check that `EnumAdapters1[0]` is the chosen GPU's LUID (for "Specific GPU") or report the order it got, then delete the temporary value and folder. This proves what Windows does with the token on this machine, not what the game does with it.
3. **Verificado** (observed): when a process with that image path starts, sample PDH `\GPU Engine(pid_<pid>_luid_*_engtype_3D)\Utilization Percentage` for up to 2 minutes. PDH creates engine instances for every adapter a process opened, even at 0 % (explorer has instances on three LUIDs) [LOCAL], so the verdict uses utilization above 2 % summed per LUID, not instance presence. Verdicts: "✓ dibujó en RX 7800 XT", "✗ dibujó en AMD Radeon(TM) Graphics" (with the likely reason from §7.7), or "Sin actividad 3D todavía". Child processes of the same image path count (Electron and Chromium GPU processes run the same exe [LOCAL Discord]).

The global default is verified the same way: after writing `HighPerfAdapter`, run the probe and show `HIGH_PERFORMANCE[0]` and `EnumAdapters1[0]` before and after.

### 7.7 What a preference does not control (shown in the UI, per app when known)

- **OpenGL**: follows the GPU that drives the window's monitor on this desktop [INFERRED from `opengl32.dll` imports; COMMUNITY Lossless Scaling guide].
- **Compute** (HIP, CUDA, OpenCL, Vulkan compute, llama.cpp, Ollama, PyTorch): ignore it; use a launch profile (§9) or Corral.
- **Engines that choose by themselves**: Godot honours only `GpuPreference=1` [SRC `os_windows.cpp:902`]; DagorEngine reads only `GpuPreference` [SRC]; games that rank by memory or remember an adapter index.
- **WebView2 hosts** (Tauri apps, many Win32 apps): their GPU process is the shared `msedgewebview2.exe` [LOCAL]. Electron apps are fine: the preference on their own exe covers their GPU process.
- **Monitor on another GPU**: a game rendering on the 7800 XT shown on the iGPU's monitor is copied by DWM every frame ("DWM will copy the output of your application from adapter A to adapter B behind your back", Rafael Cintron, Microsoft [https://www.khronos.org/webgl/public-mailing-list/public_webgl/1807/msg00012.php]); single-copy CASO exists only on hybrid laptops [DOC DirectX blog 2023-09-25].

## 8. Game and app discovery

### 8.1 Providers in v0.1

| Provider | Source | Verified here |
|---|---|---|
| Steam | `HKCU\Software\Valve\Steam\SteamPath` → `steamapps\libraryfolders.vdf` (new and old formats) → `appmanifest_<id>.acf` (`installdir`, `StateFlags`) → `appcache\appinfo.vdf` v41 (magic `0x07564429`, string table) `config/launch/<n>` [DOC https://github.com/SteamDatabase/SteamAppInfo; SRC Playnite `SteamLocalService.cs:72-126, 316-403`] | 25 manifests; v41 parser verified (`research\tools\steam_appinfo_probe.py`, run with `python -I -X utf8`) |
| EA (cross-check for Steam) | `<installdir>\__Installer\installerdata.xml` `runtime/launcher/filePath`, strip the `[HKLM\…]` prefix, resolve against the game folder; UTF-8 or UTF-16 [SRC SRM `ea-desktop.parser.ts:198-218`] | Jedi Survivor |
| Xbox / GDK | `<drive>:\.GamingRoot` (`RGBX`, u32 count, UTF-16 folder names) → `<game>\Content\MicrosoftGame.config` `ExecutableList/Executable` (keep `TargetDeviceFamily=PC`, skip `IsDevOnly`) [DOC https://learn.microsoft.com/en-us/gaming/gdk/docs/reference/system/microsoftgameconfig/elements/microsoftgameconfig-element-executable]; cross-check `HKLM\SOFTWARE\Microsoft\GamingServices\PackageRepository\Root` (read-only) because `GameConfig` keeps stale packages [LOCAL: three old Forza Horizon 6 entries] | 3 packages |
| Windows hints | `HKCU\System\GameConfigStore\Children\*\MatchedExeFullPath` (undocumented; 17 entries here, 12 still valid) [LOCAL]. Hint only, file must exist | yes |
| Corriendo ahora | PDH GPU Engine `engtype_3D` utilization per pid → image path (§7.6) | Discord, wallpaper64 on the iGPU |
| Manual | file picker, drag-and-drop of `.exe`/`.lnk`, Start-menu shortcuts | — |

Epic is installed here with no games and no `Manifests` folder [LOCAL], so its provider waits for v0.2 together with GOG, EA app, Ubisoft, Battle.net and Riot (formats documented in `game-discovery.md` §3).

### 8.2 Resolving the exe that renders

Preferences are keyed per process image, so a rule on a launcher or stub does nothing for the game [INFERRED; Stage 0 checks the Unreal stub case]. Cascade, first hit wins, each candidate must exist on disk:

1. Store hint: GDK `ExecutableList`; EA `installerdata.xml`.
2. Steam launch entries with `oslist` empty or containing `windows`, in the order `type=default` → empty type → `config`/`none` → `option*`. Many games have an empty type (RE4, RE6, P5R) or only options (PEAK) [LOCAL]. URIs (`link2ea://`, `steam://`) are skipped; if `arguments` names an exe that exists in the folder (`launcher.exe -run wallpaper64.exe`, `bootstrapper.exe cod.exe`), that child exe is added.
3. Engine heuristics: Unreal stub (declared exe under ~2 MB and a `<Project>\Binaries\Win64\*.exe` or `*-Win64-Shipping.exe` exists; Hogwarts Legacy → `Phoenix\Binaries\Win64\HogwartsLegacy.exe`, Halo → `Meteorite\Binaries\Win64\HaloCampaignEvolved.exe`). Unity (`<exe>_Data` present): the declared exe is the renderer. Never "largest exe": on every Unity game here the largest is `UnityCrashHandler64.exe` [LOCAL].
4. Always excluded: `CrashReportClient.exe`, `UnityCrashHandler*.exe`, `crashpad_handler.exe`, `EpicWebHelper.exe`, redistributables, installers.
5. Observed: a process under the game's install dir with 3D activity becomes a render target with origin `Observed` (covers `cod.exe`, JVMs, anything the cascade missed).

A rule applies to every render target of the app. If the cascade finds none, the row shows "Ábrelo una vez y Cambiavía detectará el exe".

### 8.3 Names, icons, orphans

- Names: Steam `appinfo` `common/name`; GDK `ShellVisuals@DefaultDisplayName`; else `VS_VERSIONINFO` `ProductName`/`FileDescription`.
- Icons: `SHCreateItemFromParsingName(path or "shell:AppsFolder\<AUMID>")` + `IShellItemImageFactory::GetImage`; Steam `librarycache\<appid>\` art one folder deep (`library_header.jpg` or `header.jpg`) [LOCAL].
- **Orphans**: values whose exe no longer exists **and** that contain GPU tokens are listed with "Quitar la regla de GPU" (removes only GPU tokens). Values that only hold Windows' own tokens (`AppStatus`, `AutoHDREnable`) are not Cambiavía's and are not offered for cleanup.

### 8.4 New installs

Full scan at startup and when the window opens; `notify` watchers on each Steam `steamapps\` folder (debounced 5 s; a game counts as installed when `StateFlags` has value 4 (FullyInstalled) and none of 1024/1048576/2097152 (UpdateStarted/Downloading/Staging)); a 15-minute reconciliation for Xbox (PackageCatalog events for unpackaged apps are unverified). A new game raises a toast "Nuevo juego: <name>. ¿Usar <GPU principal>?" with Aplicar / Ignorar; Ajustes has "Aplicar la jerarquía a juegos nuevos automáticamente" (opt-in, still journaled).

## 9. Compute launch profiles

The registry preference does not reach compute runtimes. They read environment variables, and the useful ones are positional indices that move when hardware or the primary monitor changes [LOCAL, `compute-stacks.md` §0]. So a profile stores GPUs by `GpuKey` and resolves indices **at every launch**:

| Lever | Value computed at launch | Source |
|---|---|---|
| `HIP_VISIBLE_DEVICES` | HIP indices of the chosen GPUs, ascending (HIP on Windows filters, does not reorder) | child-process HIP probe: `hipGetDeviceCount` + `hipDeviceGetPCIBusId`, joined to the inventory by PCI bus [LOCAL measured filter-only; DOC https://rocm.docs.amd.com/projects/HIP/en/latest/reference/env_variables.html] |
| `VK_LOADER_DEVICE_ID_FILTER` | hex PCI device IDs (`0x747e`); refused when two chosen/unchosen cards share a device ID | Vulkan loader ≥ 1.4.326; verified with 1.4.341 here [DOC LoaderInterfaceArchitecture.md; LOCAL] |
| `GGML_VK_VISIBLE_DEVICES` | Vulkan indices, computed in a clean env (no loader filter set at the same time) | child-process Vulkan probe (`ash`): `deviceLUID` joins DXGI, `deviceType` [SRC ggml-vulkan.cpp:5277-5294] |
| `DXVK_FILTER_DEVICE_UUID` | Vulkan `deviceUUID` of the chosen GPU | same probe [DOC DXVK README] |

- `cambiavia run <profile>` resolves the env, starts the exe with `CreateProcess` (env set or removed only for that child), and exits. A profile can create a Start-menu shortcut `Cambiavía\<name>.lnk` (`IShellLinkW`) pointing at that command.
- Index-based variables are never written to `HKCU\Environment`, where they would reach every HIP or Vulkan app at the next logon (LM Studio autostarts here and would inherit them [LOCAL HKCU Run]).
- Limits shown in the profile editor: elevated processes ignore `VK_LOADER_*` (the loader uses `loader_secure_getenv`) [SRC `loader_environment.c:151-162`]; apps that ship their own old `vulkan-1.dll` ignore it; the HIP probe uses the driver's System32 runtime, while PyTorch ships its own (both gave the same order today [LOCAL]); apps that save their own device choice (LM Studio `visibleDevices`, Blender ticks, DirectML `device_id`) are not changed.
- Choosing `ollama.exe` or `ollama app.exe` shows "Corral gestiona las GPUs de Ollama" with a link, and creates nothing.

## 10. Safety

- **HKCU only.** Writes go to `HKCU\Software\Microsoft\DirectX\UserGpuPreferences`, the app's own `%APPDATA%\Cambiavia\` folder, Start-menu shortcuts under the user profile and, if the user opts in, `HKCU\…\Run` for autostart. The user already has FullControl on the preferences key [LOCAL Get-Acl]; no elevation is ever needed.
- **Manifest `asInvoker`; refuse to write when elevated.** An over-the-shoulder elevation can run as another account, so HKCU would be the wrong hive [INFERRED from Dual-GPU-App-Manager `gpu_application_manager_gui.py:66-79`]. Read-only features keep working.
- **Never**: drivers, HKLM, `Policies`, display topology, BIOS, process termination, DLL injection, deleting the key, writing values for exes the user did not choose, rewriting values it does not change, touching tokens other than its own.
- **Preview before every write**, write-ahead journal, read-back, `.reg` backup before every batch, undo with conflict detection.
- **No network**, no telemetry, no auto-update in v0.1 (releases on GitHub).
- Untrusted inputs (VDF, XML, registry strings, JSON imports) go through parsers with size limits and no panics; a malformed file disables that provider with a visible reason.

## 11. UI (Spanish)

Window: Mica backdrop on Windows 11, system light/dark and accent, Segoe UI Variable, left navigation. Every status uses an icon **and** text. All lists work with the keyboard.

### 11.1 Resumen (first screen and onboarding)

Top: a strip with one card per GPU (name, kind badge "Discreta"/"Integrada (UMA)", memory, monitors, live 3D %). Below, "Lo que Windows hace hoy":
- "Adaptador 0 (lo usan casi todas las apps): AMD Radeon(TM) Graphics, porque la pantalla principal está conectada a la placa base."
- "«Alto rendimiento» para Windows: AMD Radeon(TM) Graphics."

Findings (from `diagnose`), each with a fix:
1. "Windows trata a la integrada como GPU de alto rendimiento" → **[Usar RX 7800 XT como GPU de alto rendimiento]** (global default flow).
2. "La pantalla principal está en la integrada" → "Haz principal la pantalla conectada a la RX 7800 XT, o conecta todas las pantallas a ella" + [Abrir configuración de pantalla] (`ms-settings:display`). Cambiavía does not change it.
3. "La integrada reserva 16 GB de RAM y parece más grande que la RX 7800 XT" → "Si no la usas para IA, baja la memoria UMA en la BIOS" (text only).
4. "Tus pantallas están en GPUs distintas: un juego en una pantalla de la integrada se copia cada fotograma" (text only).
5. "N apps fijadas en ahorro de energía" (here: two ASUS services, `GpuPreference=1`) → link to their rows. Neutral: background apps on the iGPU can be a good idea, since they leave VRAM free for games and models.
6. "N reglas huérfanas" → Juegos y apps, filter "Huérfanas".

### 11.2 GPUs

Ordered list (drag handle plus Subir/Bajar buttons), checkbox "Usar en reglas «Seguir jerarquía»", kind badge, PCI location, live dedicated/shared memory bars and engine utilization, top 5 processes per GPU. Collapsed section "Adaptadores ignorados": the IDD clone ("Pantalla virtual (IddSampleDriver): no dibuja") and Basic Render. Absent GPUs stay listed as "Ausente" with their rank.

### 11.3 Juegos y apps

Grouped list (Steam, Xbox, Corriendo ahora, Manual), search, filters (Con regla, Sin regla, Huérfanas, No verificadas). Row: icon, name, render exe(s) with origin badge ("Manifiesto", "Motor", "Observado", "Windows", "Tú"), GPU selector, status chip ("Pendiente de reiniciar", "✓ Verificado en RX 7800 XT", "✗ Dibujó en la integrada"), and a "?" with §7.7 notes when they apply. Toolbar: **"Aplicar la GPU principal a todos los juegos"** (one batch, one preview, one undo).

### 11.4 Cómputo

Profile list and editor: exe, arguments, GPUs (checklist in hierarchy order), levers, live "Variables que se aplicarán" preview, buttons "Probar" (runs the probes and shows the resolved env), "Crear acceso directo", "Abrir ahora".

### 11.5 Historial

Batches newest first, each expandable to before/after per value; Deshacer (change or batch), "Deshacer todo desde…", Exportar, Importar (preview diff), Abrir carpeta de copias.

### 11.6 Events

- **New GPU** (e.g. the R9700): banner and toast "Se detectó AMD Radeon AI PRO R9700 (32 GB)". The dialog shows the hierarchy with the new card highlighted at the end; after the user places it, the preview lists every rule that changes (global default + "Seguir jerarquía" rules) → Aplicar.
- **GPU gone**: "RX 7800 XT no está presente": rules that point at it are listed; "Seguir jerarquía" rules move to the next present GPU only after the user confirms.
- **New game**: toast (§8.4).

### 11.7 Ajustes

Run in background (tray, verification of launches; off by default, offered in onboarding), start with Windows, auto-apply hierarchy to new games, adapter-ID style (advanced), telemetry cadence.

## 12. Testing

### 12.1 Unit tests (Rust, pure functions; target 80 % of `cambiavia-core` and `gpu-inventory`)

- Tokens: parse/format round trip; trailing spaces; unknown and future tokens kept in order; GPU tokens placed first (`SpecificAdapter` before `GpuPreference`); "Windows decide" empties to delete; idempotence; malformed input never panics.
- Adapter IDs: padded, compact, lowercase, malformed; both format styles; numeric equality.
- Global settings: set/replace/remove `HighPerfAdapter` while `SwapEffectUpgradeEnable=1;AutoHDREnable=1;` survives byte-for-byte.
- Choice resolution: `FollowHierarchy` with the top GPU disabled or absent; ambiguity guard with twins and with the IDD clone.
- `classify_adapter` and KMT decoding with the measured values `0x230b`, `0x2303`, `0x342`, `0x105`, bus `0xFFFFFFFF`, UMA flags.
- `diagnose` with a fixture of this PC (expects findings 1-5 of §11.1) and with a fixed setup (primary on the 7800 XT, `HighPerfAdapter` set) that yields none.
- Steam: `libraryfolders.vdf` both formats; `StateFlags` 4, 518, 1026, 1048580; `appinfo.vdf` v41 on a trimmed fixture; launch selection on the measured cases (Hogwarts Legacy, Chained Together, CoD, Wallpaper Engine, Jedi Survivor, HELLDIVERS 2, PEAK, RE4); EA prefix strip, UTF-16 XML.
- GDK: `.GamingRoot` bytes; `MicrosoftGame.config` with PC/Xbox entries and `IsDevOnly`.
- GameConfigStore filtering; orphan detection (GPU tokens only).
- PDH engine instance parsing, per-LUID aggregation, threshold verdicts.
- Journal: write-ahead, recovery of Pending into Committed/Aborted/Conflict, undo conflict, batch undo order.
- `.reg` formatter: header, escaping of `\` and `"`, UTF-16LE with BOM.
- Profile export/import: schema round trip, diff against a `MemoryStore`, skipped entries.
- Launch env: HIP indices from a PCI map (including a synthetic R9700 on bus 3 pushing the 7800 XT to bus 4), device-ID filter, refusal for identical device IDs.
- Path normalization: case, `\\?\` prefix, trailing spaces, AUMIDs.

### 12.2 Integration tests

`planner` + `journal` + `MemoryStore` end to end (apply, crash between write-ahead and write, recover, undo); discovery against a fixture Steam tree in a temp folder; CLI `--dry-run` output snapshots.

### 12.3 UI tests

Vitest + Testing Library for each page with fixture data (selector states, preview dialog, conflict dialog, keyboard reordering). Playwright E2E against the dev UI with mocked Tauri commands, one flow per page (pattern from Corral's `e2e/app.spec.ts`).

### 12.4 Acceptance test on the reference PC

`crates/cambiavia-core/tests/acceptance.rs`, `#[ignore]`, run with `cargo test -p cambiavia-core -- --ignored acceptance --test-threads=1`. It writes one registry value whose name is a fresh temp path, so it cannot collide with a real app. It needs the user's go-ahead the first time.

`acceptance_specific_adapter_roundtrip`:
1. Inventory; choose `T` = a present discrete GPU that is not the current `EnumAdapters1[0]` (the 7800 XT here). Skip if none.
2. Snapshot every value of `UserGpuPreferences` (`S0`).
3. Copy `cambiavia-probe.exe` to `%TEMP%\cambiavia-acceptance-<ulid>\probe.exe`; assert no value exists for that path. A drop guard deletes that one value and the folder if the test panics.
4. Run the probe: baseline `B` (`EnumAdapters1[0]` expected = iGPU here).
5. Apply `Specific(T)` through `planner` + `journal` + `WinRegStore`.
6. Assert the value reads `SpecificAdapter=<id>;GpuPreference=1073741824;`.
7. Run the probe in a new process: assert `EnumAdapters1[0].luid == T.luid`, which also proves Windows did not resolve the token to the IDD clone (a `Virtual` adapter with the same IDs, LUID `0x2764F` today); record `HIGH_PERFORMANCE[0]`.
8. Undo the batch; assert the value is gone.
9. Run the probe: assert `EnumAdapters1[0] == B`.
10. Snapshot `S1`; assert `S1 == S0` byte for byte.

`acceptance_high_perf_roundtrip` runs only with `CAMBIAVIA_ACCEPT_GLOBAL=1` because it changes a system-wide setting for a few seconds: set `HighPerfAdapter` = 7800 XT, assert the probe's `HIGH_PERFORMANCE[0]` = 7800 XT, record `EnumAdapters1[0]`, undo, assert `DirectXUserGlobalSettings` equals its original string exactly.

CI (GitHub Actions `windows-latest`, no GPU) runs everything except the acceptance tests: `cargo test`, `cargo clippy -- -D warnings`, `npm test`, `npm run e2e`, Tauri build.

### 12.5 Manual release checklist (reference PC)

Hogwarts Legacy (stub → Phoenix exe), Wallpaper Engine (launcher → `wallpaper64.exe`), CoD (bootstrapper → `cod.exe`), Minecraft for Windows (GDK: AUMID vs path), one Electron app, the global default, a compute profile running `vulkaninfo --summary` with only the 7800 XT visible, undo of everything, and `S0 == S_end`.

## 13. Staged plan

**Stage 0 — spike, gated on the user's approval (registry writes).** Answer the open questions that decide the design, with the Python probes already in `research\probes\`:
- A. Does `SpecificAdapter=1002&747E&2427148C;GpuPreference=1073741824;` on a scratch copy of `python.exe` make `EnumAdapters1[0]` the 7800 XT on this non-hybrid desktop, and not the IDD clone? (`windows-gpu-preference.md` §9 Q3)
- B. Does `GpuPreference=2` change anything here? (Q2)
- C. Does `HighPerfAdapter` make `HIGH_PERFORMANCE[0]` the 7800 XT, and does it change `UNSPECIFIED`? (Q1, test A)
- D. Settings round-trip: the user sets "Specific GPU" for the scratch exe in Settings once; read what Windows wrote (padding, token order).
- E. GDK key: add Minecraft as a Store app in Settings and read the value name (AUMID or path); check where `Minecraft.Windows.exe` renders.
- F. Unreal stub: preference on the stub only; check the child's LUID.
**Gate:** if A fails, per-app "GPU específica" is re-scoped to "Alto rendimiento + global default", and the per-API levers (§9, DXVK) become the per-game path; the spec is updated before Stage 2.

**Stage 1 — repo and shared crate.** Scaffold `cambiavia` (Tauri 2, AGPL-3.0, README in Spanish with English dev docs, CI); create `gpu-inventory` from Corral's `gpu/windows.rs` + `pdh_names.rs`, add KMT type/address, UMA, outputs, classification, watch; `cambiavia-probe`.

**Stage 2 — preferences core.** Tokens, adapter IDs, choice, `PrefStore`, planner, journal, backups, undo, CLI `get/set/undo --dry-run`, acceptance test.

**Stage 3 — UI shell, GPUs and Resumen.** Navigation, Mica/theme, GPU page with telemetry, hierarchy, diagnosis, global default flow with verification.

**Stage 4 — discovery.** Steam (+EA cross-check), Xbox/GDK, GameConfigStore hints, Corriendo ahora, manual; render-exe cascade; Juegos y apps page; bulk apply; observed verification; orphans.

**Stage 5 — compute profiles.** HIP and Vulkan probes, env resolution, `run`, shortcuts, Cómputo page.

**Stage 6 — events, history, import/export.** Adapter watch, install watchers, toasts, tray/background mode, Historial page, export/import.

**Stage 7 — release v0.1.0.** Manual checklist (§12.5), screenshots, README, `.msi` and NSIS `.exe` attached to the GitHub release (a release without binaries is not done), `gh api repos/santiquiroz/cambiavia --jq .license.spdx_id` = `AGPL-3.0`.

**v0.2**: launchers Epic/GOG/EA app/Ubisoft/Battle.net/Riot; one-shot "Abrir con GPU…" with revert, Explorer context menu (`HKCU\Software\Classes`); engine arguments; following versioned paths; own WebView2 GPU; import of Corral's hierarchy.

## 14. Risks

| Risk | Mitigation |
|---|---|
| The token format is undocumented and may change in a Windows build | Formats in one module with tests; adapter-ID style is config; Stage 0 Settings round-trip; read-back plus probe self-test catch drift |
| "Specific GPU" may not reorder adapters on a non-hybrid desktop | Stage 0 gate A before building on it; fallback plan in §13 |
| Windows may resolve `SpecificAdapter` to the IDD clone (same IDs) | Acceptance asserts the real LUID; if it picks the IDD, show it and recommend removing the virtual display |
| R9700 subsystem ID with leading zeros (padding question) | Copy Windows' own style; parser accepts both |
| Games ignore the preference (OpenGL, Godot D3D12, Dagor, memory ranking) | Observed verification makes it visible; §7.7 explanations; engine args in v0.2 |
| Cross-adapter copy while the primary monitor stays on the iGPU | Diagnosis finding 2 and 4; Cambiavía cannot fix topology |
| LUIDs and HIP/Vulkan indices change between boots or with new hardware | Store `GpuKey` + PCI hint only; resolve at every launch |
| PDH GPU Engine sampling costs CPU on machines with many processes | Sample only on demand or for armed apps; 15 s background cadence; measure against the 1 % budget in Stage 4 |
| Versioned install paths (Discord `app-1.0.9261`, WindowsApps, Java runtime) drop path-keyed rules | Orphan view + observed re-detection in v0.1; automatic follow in v0.2 |
| Elevated run maps HKCU to another account | Refuse writes when elevated |
| PackageCatalog events may not reach unpackaged apps | Periodic reconciliation |
| Anti-cheat | Only the same registry value Windows Settings writes; no injection |

## 15. Decisions

- Tauri 2 + Rust + React/TS, AGPL-3.0-only, Spanish UI, English code and docs.
- One shared crate (`gpu-inventory`) with Corral; GPUs identified by `GpuKey` (VEN/DEV/SUBSYS/REV, what Windows stores) plus PCI location, never by name, LUID or index.
- "Specific GPU" is the main per-app lever on this kind of desktop; "High performance" is offered but explained.
- The global default is set to the hierarchy's primary GPU, and discovered games get "Seguir jerarquía" rules because the global default alone does not reach apps that take adapter 0.
- Cambiavía never touches Ollama; Corral's hierarchy is separate and covers compute for Ollama.
