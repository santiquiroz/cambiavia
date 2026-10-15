# Stage 0 spike (2026-10-14, reference PC, Windows build 26300)

Probe: a copy of python.exe in a scratch folder running `probes/dxgi_enum.py`; the preference value
is written for that copy's full path in HKCU\Software\Microsoft\DirectX\UserGpuPreferences and
removed afterwards. `DirectXUserGlobalSettings` already held `HighPerfAdapter=1002&747E&2427148C;`
(set at the user's request the same day for PEAK).

| Value for the probe exe | EnumAdapters1[0] |
|---|---|
| none | AMD Radeon(TM) Graphics, LUID 0x1912e (iGPU, primary display) |
| `SpecificAdapter=1002&747E&2427148C;GpuPreference=1073741824;` | AMD Radeon RX 7800 XT, LUID 0x16c9b (the real card, not the IDD clone 0x2764f) |
| `GpuPreference=2;` with HighPerfAdapter set | AMD Radeon RX 7800 XT, LUID 0x16c9b |
| removed | AMD Radeon(TM) Graphics, LUID 0x1912e |

Before HighPerfAdapter was set, EnumAdapterByGpuPreference(HIGH_PERFORMANCE)[0] was the iGPU
(ranked by dedicated memory: 15.97 GiB UMA reservation vs 15.80 GiB); after it, the 7800 XT.

Conclusions: the SPEC §13 gate passes. `SpecificAdapter=VVVV&DDDD&SSSSSSSS` with
`GpuPreference=1073741824` reorders adapter enumeration for that exe on this non-hybrid desktop, and
`HighPerfAdapter` fixes what "High performance" means system-wide. Existing tokens in a value
(AppStatus, AutoHDREnable) must be kept: overwriting PEAK's value lost them once and had to be
restored by hand.
