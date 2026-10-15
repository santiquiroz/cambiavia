# Descubrimiento de juegos y de su ejecutable real (para elegir GPU por juego)

Investigación del 2026-10-14. Todo lo marcado **[verificado en este PC]** salió de lecturas de solo lectura en el PC de Santiago (registro, archivos, PDH, DXGI). Lo marcado **[inferido]** no está documentado por el fabricante ni se pudo probar sin escribir el registro.

Scripts usados (reproducibles, solo lectura):
- `C:\personal\gpu-app-design\research\tools\steam_appinfo_probe.py`: parser de `appinfo.vdf` v0x29 (magic `0x07564429`), escribe JSON con `config/launch` de cada juego instalado.
- `C:\personal\gpu-app-design\research\tools\steam_launch_vs_disk.py`: compara el exe que declara Steam con los exe más grandes de la carpeta.
- `C:\personal\gpu-app-design\research\tools\dxgi_luids.ps1`: lista adaptadores DXGI con su LUID.

---

## 1. Hallazgo principal

Encontrar los juegos instalados es la parte fácil. Lo difícil es saber **qué exe dibuja**, porque `HKCU\Software\Microsoft\DirectX\UserGpuPreferences` guarda la preferencia por ruta completa de exe (o por AUMID en apps empaquetadas). Si se marca el exe equivocado (un lanzador, un stub o un bootstrapper), la preferencia no tiene efecto.

Casos reales de este PC **[verificado en este PC]**:

| Juego (tienda) | Lo que declara la tienda | Proceso que dibuja | Tipo de problema |
|---|---|---|---|
| Hogwarts Legacy (Steam 990080) | `HogwartsLegacy.exe` (0,3 MB, raíz) | `Phoenix\Binaries\Win64\HogwartsLegacy.exe` (450 MB) | Stub de Unreal |
| Chained Together (Steam 2567870) | `ChainedTogether.exe` (0,3 MB) | `ChainedTogether\Binaries\Win64\ChainedTogether-Win64-Shipping.exe` | Stub de Unreal |
| Call of Duty (Steam 1938090) | `bootstrapper.exe cod.exe` | `cod.exe` (Windows lo registró así en GameConfigStore) | Bootstrapper |
| Wallpaper Engine (Steam 431960) | `launcher.exe -run wallpaper64.exe` | `wallpaper64.exe`; PDH lo muestra dibujando en la iGPU (LUID `0x1912E`) | Lanzador con argumento |
| STAR WARS Jedi: Survivor (Steam 1774580) | `link2ea://launchgame/1774580?...` | `SwGame\Binaries\Win64\JediSurvivor.exe` (sale de `__Installer\installerdata.xml`) | URI de otra tienda |
| HELLDIVERS 2 (Steam 553850) | `bin\helldivers2.exe` + opciones `start_protected_game.exe` que no existen en disco | `bin\helldivers2.exe` | Entradas de launch obsoletas |
| Minecraft for Windows (Xbox/GDK) | AppxManifest: `GameLaunchHelper.exe` | `Minecraft.Windows.exe` (MicrosoftGame.config y `GamingServices\GameConfig`) | Ayudante de GDK |
| Minecraft Java (Xbox) | `Minecraft.exe` / `MCPlaceholderStub.exe` | `...\Packages\Microsoft.4297127D64EC6_8wekyb3d8bbwe\LocalCache\Local\runtime\java-runtime-epsilon\...\javaw.exe` | JVM descargada; la ruta cambia con cada runtime |
| Arena Breakout: Infinite (Steam, desinstalado) | n/a | `ABInfinite\Binaries\Win64\UAGame.exe` | Exe con nombre distinto al juego |

Además hay entradas **huérfanas** en `UserGpuPreferences`: `RainbowSix.exe` en dos rutas y `VALORANT-Win64-Shipping.exe`, cuyos juegos ya no están instalados (`C:\Riot Games` solo tiene `Riot Client.old`). La app debería mostrarlas y ofrecer limpiarlas.

Conclusión de diseño: el descubrimiento debe combinar (a) el manifiesto de cada tienda, (b) heurísticas por motor (Unreal, Unity) y (c) **aprendizaje en ejecución**: ver qué proceso del directorio del juego abre un dispositivo 3D y en qué GPU (PDH `GPU Engine` por `pid_X_luid_Y`, que Corral ya interpreta en `src-tauri/src/gpu/pdh_names.rs:15-17`).

---

## 2. Inventario de este PC **[verificado en este PC]**

- **Steam**: `HKCU\Software\Valve\Steam\SteamPath = c:/program files (x86)/steam` (también `HKLM\SOFTWARE\WOW6432Node\Valve\Steam\InstallPath`). Una biblioteca en `libraryfolders.vdf` (`C:\Program Files (x86)\Steam`), 25 `appmanifest_*.acf` (22 juegos o apps más "Steamworks Common Redistributables"). Dos con `StateFlags=518` (= 512 UpdatePaused + 4 FullyInstalled + 2 UpdateRequired): Jedi Survivor y Crimson Desert. Sin `shortcuts.vdf` (no hay juegos no-Steam agregados).
- **Epic Games Launcher**: instalado (`C:\Program Files\Epic Games\`), `LauncherInstalled.dat` con `"InstallationList": []` y sin carpeta `Data\Manifests`: ningún juego de Epic instalado.
- **Xbox / Game Pass**: `C:\.GamingRoot` existe y apunta a `XboxGames`. Tres paquetes en `C:\XboxGames`: Minecraft for Windows, Minecraft Launcher y Minecraft Java Edition. `C:\Program Files\WindowsApps\Microsoft.MinecraftUWP_1.26.5203.0_x64__8wekyb3d8bbwe` es un **junction** a `C:\XboxGames\Minecraft for Windows\Content\` (`fsutil reparsepoint query`: tag `0xa0000003`, punto de montaje).
- **Riot**: `C:\Riot Games\Riot Client.old`, acceso directo a `RiotClientServices.exe`; VALORANT desinstalado.
- **GOG Galaxy, EA app, Ubisoft Connect, Battle.net, itch, Amazon**: no instalados (no existen sus claves ni sus carpetas de datos).
- **Exes sueltos**: `C:\personal\gow2-recomp\out\...\gow2.exe` existe en **seis** carpetas distintas (release, test-run, fsr4-test, staging del paquete, snapshot-v010, stock-test). Como la preferencia va por ruta completa, cada build es una entrada distinta.
- **Windows GameConfigStore**: `HKCU\System\GameConfigStore\Children` tiene 89 entradas; 17 con `MatchedExeFullPath` (ver sección 4.2).
- **Adaptadores DXGI** (orden de enumeración): `0` AMD Radeon(TM) Graphics, LUID `0x1912E`, 16,0 GB "dedicada"; `1` RX 7800 XT `0x16C9B`; `2` RX 7800 XT `0x2764F`; `3` Microsoft Basic Render Driver. Hay un `IddSampleDriver` (pantalla virtual) en `Win32_VideoController`; la segunda entrada de la 7800 XT probablemente es el adaptador que expone esa pantalla indirecta **[inferido]**. En este momento Discord y `wallpaper64.exe` dibujan en la iGPU.

---

## 3. Tienda por tienda

### 3.1 Steam (fuente principal en este PC)

**Dónde está Steam**: `HKCU\Software\Valve\Steam\SteamPath` (con `/`, normalizar a `\`). Playnite hace lo mismo: [Steam.cs:34-38](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/SteamLibrary/Steam.cs#L34-L38).

**Bibliotecas**: `<Steam>\steamapps\libraryfolders.vdf` (VDF de texto). Cada hijo numérico tiene `path` y un bloque `apps { appid  tamaño }`. El formato viejo era `"1" "D:\\Juegos"` (valor plano); Playnite acepta los dos: [SteamLocalService.cs:316-374](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/SteamLibrary/Services/SteamLocalService.cs#L316-L374). Siempre se agrega la carpeta de Steam aunque no esté en el archivo.

**Juegos instalados**: `<biblioteca>\steamapps\appmanifest_<appid>.acf` (VDF de texto). Campos útiles: `appid`, `name`, `installdir` (relativo a `steamapps\common\`), `StateFlags`, `LastPlayed`, `SizeOnDisk`. Un juego está instalado si `StateFlags` tiene el bit 4 (`FullyInstalled`); la enumeración completa de bits está en [SteamLocalService.cs:378-403](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/SteamLibrary/Services/SteamLocalService.cs#L378-L403), y la lectura del manifiesto en [SteamLocalService.cs:72-126](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/SteamLibrary/Services/SteamLocalService.cs#L72-L126). Hay que excluir herramientas (appid 228980, Proton, redistribuibles); `common.type` en `appinfo.vdf` dice `Game`, `Application` o `Tool`. Abrir los archivos con `FileShare.ReadWrite`, porque Steam los tiene abiertos.

Atajo alternativo: `HKCU\Software\Valve\Steam\Apps\<appid>` con `Installed`, `Running` y `Name` (aunque `Name` viene vacío en muchos) **[verificado en este PC]**. Sirve para enterarse de cambios con `RegNotifyChangeKeyValue`, pero no da rutas.

**Exe a lanzar**: no está en el `.acf`. Está en `<Steam>\appcache\appinfo.vdf` (binario, caché local de PICS). Formato documentado por SteamDatabase ([SteamAppInfo README](https://github.com/SteamDatabase/SteamAppInfo), MIT):
- Cabecera: magic `u32` (`0x07564427` v39, `0x07564428` v40, `0x07564429` v41), universe `u32`; en v41, `i64` con el offset de una **tabla de strings** (`u32` cantidad + strings terminadas en NUL) que deduplica las claves de todos los blobs.
- Por app: appid, size, infoState, lastUpdated, picsToken `u64`, sha1[20], changeNumber, binarySha1[20] (v40+), y luego KeyValues binario. Termina cuando appid = 0.
- KeyValues binario: tipo `u8` (0 sección, 1 string, 2 int32, 3 float, 7 uint64, 8 fin); en v41 la clave es un índice `u32` a la tabla de strings.
- **[verificado en este PC]**: el archivo es v41 (`0x07564429`); el parser de `tools\steam_appinfo_probe.py` lee las 25 apps sin errores mientras Steam está abierto.

La ruta útil es `appinfo/config/launch/<n>/` con `executable`, `arguments`, `type` (`default`, `option1`…, `none`), `description` y `config/oslist`, `config/osarch`, `config/betakey`. Filtrar por `oslist` vacío o que contenga `windows`, preferir `type=default` y verificar que el archivo exista: Hogwarts Legacy declara 18 entradas `Phoenix-Win64-Test.exe` que no están en disco, y HELLDIVERS 2 declara `start_protected_game.exe`, que tampoco. `executable` puede ser una URI (`link2ea://…`, `steam://open/tools`), y `arguments` puede llevar el exe real (`launcher.exe -run wallpaper64.exe`, `bootstrapper.exe cod.exe`).

Implementación de referencia en Rust: [rai-pal steam/appinfo.rs:95-210](https://github.com/Raicuparta/rai-pal/blob/e4211d12727bbd4bfc63c0343b24fea084ee81e2/backend/core/src/game_providers/steam/appinfo.rs#L95-L210) (GPL-3.0, compatible con AGPL-3.0 de Corral) y la resolución de launch options en [steam_provider.rs:108-170](https://github.com/Raicuparta/rai-pal/blob/e4211d12727bbd4bfc63c0343b24fea084ee81e2/backend/core/src/game_providers/steam/steam_provider.rs#L108-L170), que resuelve rutas sin distinguir mayúsculas y quita duplicados x86/x86_64. Para bibliotecas y manifiestos existe el crate `steamlocate` 2.1.1 (MIT, [docs.rs](https://docs.rs/steamlocate/latest/steamlocate/)), que no lee `appinfo.vdf`.

**Juegos no-Steam agregados a Steam**: `<Steam>\userdata\<id32>\config\shortcuts.vdf` (VDF binario); rai-pal los lee en [steam_provider.rs:31-100](https://github.com/Raicuparta/rai-pal/blob/e4211d12727bbd4bfc63c0343b24fea084ee81e2/backend/core/src/game_providers/steam/steam_provider.rs#L31-L100). En este PC no hay ninguno.

**Imágenes**: `common.clienticon` (hash) → `<Steam>\steam\games\<hash>.ico` (**[verificado en este PC]**: existe el de RE6, `42d4f0f1….ico`); `<Steam>\appcache\librarycache\<appid>\` con `header.jpg`, `library_600x900.jpg`, `library_hero.jpg` y `logo.png` (**[verificado en este PC]** para RE4).

### 3.2 Epic Games Launcher

- Lista: `%ProgramData%\Epic\UnrealEngineLauncher\LauncherInstalled.dat` (JSON `InstallationList[]` con `InstallLocation`, `AppName`). Playnite dice que a veces trae datos viejos: [EpicLibrary.cs:62-73](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/EpicLibrary/EpicLibrary.cs#L62-L73).
- Manifiestos: `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON, uno por instalación): [EpicLauncher.cs:103-142](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/EpicLibrary/EpicLauncher.cs#L103-L142). Campos ([LauncherInstalled.cs:22-49](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/EpicLibrary/Models/LauncherInstalled.cs#L22-L49)): `LaunchExecutable` (relativo a `InstallLocation`), `LaunchCommand`, `DisplayName`, `AppName`, `CatalogNamespace`, `CatalogItemId`, `AppCategories`, `bIsIncompleteInstall`, `MainGameAppName` (DLC) y, lo más útil para este proyecto, **`MainWindowProcessName` y `ProcessNames`**: el nombre del proceso que realmente corre, que resuelve los stubs de Unreal sin heurística.
- Filtros: saltar DLC (`AppCategories` con `addons` sin `addons/launchable`) y plugins de Unreal: [EpicLibrary.cs:44-58](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/EpicLibrary/EpicLibrary.cs#L44-L58).
- Iconos: el `.item` no trae icono local; usar el del exe.
- Lanzamiento: `com.epicgames.launcher://apps/<ns>:<id>:<appName>?action=launch&silent=true` (el patrón exacto es `EpicLauncher.GameLaunchUrlMask`, no lo leí) **[inferido]**.
- No se pudo probar aquí: no hay juegos de Epic instalados.

### 3.3 GOG Galaxy

- Cliente: `HKLM\SOFTWARE\WOW6432Node\GOG.com\GalaxyClient\paths` ([Gog.cs:39-75](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/GogLibrary/Gog.cs#L39-L75)).
- Juegos instalados, sin Galaxy: claves de desinstalación `<gameId>_is1` con `Publisher = GOG.com` (los instaladores sueltos de GOG también las crean): [GogLibrary.cs:219-249](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/GogLibrary/GogLibrary.cs#L219-L249).
- Exe: `<InstallLocation>\goggame-<gameId>.info` (JSON) con `playTasks[]` (`isPrimary`, `path`, `workingDir`, `arguments`) y `rootGameId` para distinguir DLC: [GogLibrary.cs:134-168 y 259-273](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/GogLibrary/GogLibrary.cs#L134-L168).
- Alternativa con Galaxy 2.0: SQLite `%ProgramData%\GOG.com\Galaxy\storage\galaxy-2.0.db` (abrir en solo lectura). Tablas `Builds`, `GamePieces` (`originalTitle`, `originalImages` con `squareIcon`), `PlayTasks`, `PlayTaskLaunchParameters.executablePath`. Consulta de referencia: [rai-pal gog_provider.rs:108-160](https://github.com/Raicuparta/rai-pal/blob/e4211d12727bbd4bfc63c0343b24fea084ee81e2/backend/core/src/game_providers/gog_provider.rs#L108-L160). El esquema no es público **[inferido]**.
- GOG suele dejar `goggame-<id>.ico` en la carpeta del juego **[inferido, no verificado]**.

### 3.4 EA app (y juegos de EA vendidos en Steam)

- Cada juego de EA trae `<juego>\__Installer\installerdata.xml`. `<runtime><launcher><filePath>` lleva un prefijo de registro y la ruta relativa del exe real. **[verificado en este PC]** con Jedi Survivor (Steam): `[HKEY_LOCAL_MACHINE\SOFTWARE\Respawn\Jedi Survivor\Install Dir]SwGame\Binaries\Win64\JediSurvivor.exe`. La clave `HKLM\SOFTWARE\Respawn\Jedi Survivor` **no existe** en este PC (la EA app no está instalada), así que hay que quitar el prefijo `[...]` y resolver contra la carpeta del juego, como hace Steam ROM Manager: [ea-desktop.parser.ts:47-51 y 198-218](https://github.com/SteamGridDB/steam-rom-manager/blob/f68289a770ff8537ac8235618b3399844580359b/src/lib/parsers/ea-desktop.parser.ts#L198-L218) (GPL-3.0). Ese parser también acepta el XML en UTF-16.
- Carpeta por defecto de la EA app: `C:\Program Files\EA Games\*` (SRM usa `C:\Program Files (x86)\EA Games` por defecto; conviene revisar las dos y hacerla configurable).
- Este archivo sirve como **heurística transversal**: si un juego de Steam declara `link2ea://`, se busca `__Installer\installerdata.xml` en su `installdir`.

### 3.5 Ubisoft Connect

- Instalados: `HKLM\SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs\<launcherId>\InstallDir` (Playnite prueba primero la vista de 32 bits y luego la de 64): [UplayLibrary.cs:87-108](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/UplayLibrary/UplayLibrary.cs#L87-L108).
- Exe: `%LOCALAPPDATA%\Ubisoft Game Launcher\cache\configuration\configurations` es un protobuf con un YAML por producto: `root.start_game.(online|offline).executables[].path.relative` y `working_directory.register`. Ver [Uplay.cs:46-56 y 95-140](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/UplayLibrary/Uplay.cs#L95-L140), [ProductSchema.cs:11-39](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/UplayLibrary/Models/ProductSchema.cs#L11-L39) y [SRM uplay.parser.ts:59-74, 216](https://github.com/SteamGridDB/steam-rom-manager/blob/f68289a770ff8537ac8235618b3399844580359b/src/lib/parsers/uplay.parser.ts#L216).
- Dato de este PC: había `RainbowSix.exe` en `C:\Users\santi\AppData\Local\Ubisoft\r6s\` y en `steamapps\common\Tom Clancy's Rainbow Six Siege\`. Hoy ninguno está instalado; quedan las dos entradas en `UserGpuPreferences`.

### 3.6 Battle.net

- `C:\ProgramData\Battle.net\Agent\product.db` (protobuf) con `uid`, `productCode` e `installPath` de cada producto; descartar `battle.net` y `agent`. Playnite lo usa como respaldo de las claves de desinstalación (`UninstallString` con `--uid=`): [BattleNetLibrary.cs:68-82 y 139-170](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/BattleNetLibrary/BattleNetLibrary.cs#L139-L170); SRM igual: [battle-net.parser.ts:128-156](https://github.com/SteamGridDB/steam-rom-manager/blob/f68289a770ff8537ac8235618b3399844580359b/src/lib/parsers/battle-net.parser.ts#L128-L156).
- El exe no está en `product.db`: Playnite mantiene una **tabla fija** de juegos de Blizzard ([BattleNetGames.cs](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/BattleNetLibrary/BattleNetGames.cs)). Para GPU hace falta tabla propia o aprendizaje en ejecución.

### 3.7 Riot (VALORANT, LoL)

- `C:\ProgramData\Riot Games\Metadata\<producto>.live\<producto>.live.product_settings.yaml` con `product_install_full_path` (lo usan [ValorantPorting](https://github.com/Ka1serM/ValorantPorting) y [LeagueJax, Tauri/Rust](https://github.com/tarnishablec/LeagueJax)) **[inferido, no verificado aquí]**. El exe real de VALORANT es `live\ShooterGame\Binaries\Win64\VALORANT-Win64-Shipping.exe` (**[verificado en este PC]** por la entrada vieja de GameConfigStore). Vanguard (anticheat de kernel) no cambia la ruta.

### 3.8 Xbox app, Microsoft Store y Game Pass (apps empaquetadas)

**Tres formas de enumerarlos**, de la más simple a la más completa:

1. **`.GamingRoot`** en la raíz de cada unidad: magic `RGBX`, `u32` cantidad, y luego rutas UTF-16 terminadas en NUL relativas a la unidad. **[verificado en este PC]**: `C:\.GamingRoot` = `52 47 42 58 01 00 00 00` + `"XboxGames"`. Debajo está `<juego>\Content\` con `MicrosoftGame.config` y `appxmanifest.xml`. SRM recorre `C:\XboxGames\*\{,Content/}appxmanifest.xml`: [UWP.parser.ts:51-52](https://github.com/SteamGridDB/steam-rom-manager/blob/f68289a770ff8537ac8235618b3399844580359b/src/lib/parsers/UWP.parser.ts#L51-L52).
2. **Registro de Gaming Services**: `HKLM\SOFTWARE\Microsoft\GamingServices\PackageRepository\Root\<…>\<…>` con `Package` (nombre completo) y `Root`, y `HKLM\SOFTWARE\Microsoft\GamingServices\GameConfig\<PackageFullName>\Executable\<n>` con `Name`, `Id`, `TargetDeviceFamily`, `IsDevOnly`, `Alias` y `OverrideDisplayName`. Referencia: [rai-pal xbox_provider.rs:56-120](https://github.com/Raicuparta/rai-pal/blob/e4211d12727bbd4bfc63c0343b24fea084ee81e2/backend/core/src/game_providers/xbox_provider.rs#L56-L120). **[verificado en este PC]**: Root lista los 3 paquetes instalados. `GameConfig` además conserva **tres versiones viejas de Forza Horizon 6** (`Microsoft.ForteBaseGame_2.360/2.364/2.379…`, `forzahorizon6.exe`) aunque hoy el juego es de Steam. Hay que cruzar con `PackageRepository\Root` o con `PackageManager`, porque `GameConfig` sola da falsos positivos. Clave no documentada **[inferido]**.
3. **API de paquetes** (documentada): `PackageManager.FindPackagesForUser("")` o `PackageCatalog`, filtrando `IsFramework`, `IsResourcePackage` y `SignatureKind != Store`, como hace Playnite en [Programs2.cs:294-397](https://github.com/JosefNemec/Playnite/blob/ecee01452a25db1fb2674a69f883d298d4c7be8a/source/Playnite/Common/Programs2.cs#L294-L397). El AUMID es `<PackageFamilyName>!<Application Id>` y se lanza con `explorer.exe shell:AppsFolder\<AUMID>`. Este camino lista TODAS las apps de la Store (Calculadora, Fotos…), así que para "solo juegos" hay que cruzarlo con GamingServices o con la presencia de `MicrosoftGame.config`.

**Exe real de juegos GDK**: el AppxManifest declara `GameLaunchHelper.exe` como `Executable` (**[verificado en este PC]**: Minecraft, `Id=Game`, `EntryPoint=Windows.FullTrustApplication`). El exe del juego está en `MicrosoftGame.config` → `<ExecutableList><Executable Name="Minecraft.Windows.exe" Id="Game"/>`. Microsoft documenta que `Name` es la ruta relativa a la raíz del juego: [Executable element](https://learn.microsoft.com/en-us/gaming/gdk/docs/reference/system/microsoftgameconfig/elements/microsoftgameconfig-element-executable), [ExecutableList](https://learn.microsoft.com/en-us/gaming/gdk/docs/reference/system/microsoftgameconfig/elements/microsoftgameconfig-element-executablelist). Nombre y logos: `<ShellVisuals DefaultDisplayName=… Square150x150Logo=… Square44x44Logo=…>` (**[verificado en este PC]**).

**Cómo los guarda `UserGpuPreferences`**: para apps de la Store, guías de terceros documentan que el nombre del valor es el AUMID (`Microsoft.WindowsCalculator_8wekyb3d8bbwe!App`, `Microsoft.MSPaint_8wekyb3d8bbwe!Microsoft.MSPaint`): [TrishTech](https://www.trishtech.com/2018/12/how-to-set-graphics-performance-preference-for-apps-in-windows-10/), [Tabmode](https://www.tabmode.com/windows-11/registry-editor-windows-11/set-graphics-performance-preference-for-apps-registry-edit/). Microsoft solo documenta la UI ("Desktop app" o "Microsoft Store app"): [Optimizations for windowed games (Microsoft Support)](https://support.microsoft.com/en-gb/windows/hardware/display-graphics/optimizations-for-windowed-games-in-windows-11), [Q&A de Microsoft](https://learn.microsoft.com/en-us/answers/questions/4336196/graphics-performance-preferences). En este PC no hay ninguna entrada con AUMID, así que no se pudo confirmar.

**Riesgo de las rutas versionadas** **[inferido]**: la ruta de WindowsApps lleva la versión (`Microsoft.MinecraftUWP_1.26.5203.0_x64__…`). Una preferencia por ruta se perdería en cada actualización; el AUMID (`Microsoft.MinecraftUWP_8wekyb3d8bbwe!Game`) y la ruta destino del junction (`C:\XboxGames\Minecraft for Windows\Content\Minecraft.Windows.exe`) no cambian. Falta saber cuál de los dos consulta DXGI para un proceso GDK (ver Preguntas abiertas).

### 3.9 itch

- `%APPDATA%\itch\db\butler.db` (SQLite). En la tabla `caves`, la columna `verdict` es JSON con `basePath` y `candidates[]` (`path`, `flavor`, `arch`): [SRM itch-io.parser.ts:58-88](https://github.com/SteamGridDB/steam-rom-manager/blob/f68289a770ff8537ac8235618b3399844580359b/src/lib/parsers/itch-io.parser.ts#L58-L88). Playnite usa el daemon `butler` por JSON-RPC ([Butler.cs](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/ItchioLibrary/Butler.cs)).
- El desarrollador puede incluir `.itch.toml` con `[[actions]] path = "…exe"`: [docs de itch](https://itch.io/docs/itch/integrating/manifest.html).
- No instalado en este PC.

### 3.10 Amazon Games (de paso)

- `%LOCALAPPDATA%\Amazon Games\Data\Games\Sql\GameInstallInfo.sqlite`, `SELECT * FROM DbSet WHERE Installed = 1` (`InstallDirectory`, `ProductTitle`): [AmazonGamesLibrary.cs:118-140](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/AmazonGamesLibrary/AmazonGamesLibrary.cs#L118-L140). Exe: `<InstallDirectory>\fuel.json` → `Main.Command` ([GameConfiguration.cs:11-23](https://github.com/JosefNemec/PlayniteExtensions/blob/886468f65724e6fd104f3a09847a17ba57cfb430/source/Libraries/AmazonGamesLibrary/Models/GameConfiguration.cs#L11-L23)).

### 3.11 Exes sueltos: emuladores, ports recompilados, builds propios

Ningún manifiesto los cubre. Opciones, de mejor a peor:

1. **"Corriendo ahora"** (la más útil): listar los procesos con actividad en motores 3D desde PDH `\GPU Engine(pid_<pid>_luid_<hi>_<lo>_phys_<n>_eng_<n>_engtype_3D)\Utilization Percentage` y mostrar en qué GPU dibuja cada uno. Un clic fija la GPU para ese exe. **[verificado en este PC]**: muestra Discord y `wallpaper64.exe` en LUID `0x1912E` (la iGPU, confirmada con `tools\dxgi_luids.ps1`). Corral ya interpreta los nombres `pid_X_luid_Y` en `src-tauri/src/gpu/pdh_names.rs:15-17` y ya enlaza `Win32_System_Performance` en `Cargo.toml`.
2. **Selector de archivo / arrastrar y soltar** (lo mismo que hace Settings > Display > Graphics).
3. **Accesos directos del menú Inicio** (`%APPDATA%` y `%ProgramData%\Microsoft\Windows\Start Menu\Programs`, `.lnk` resueltos con `IShellLink`), filtrando instaladores y redistribuibles con las máscaras de Playnite ([Programs.cs:52-78](https://github.com/JosefNemec/Playnite/blob/ecee01452a25db1fb2674a69f883d298d4c7be8a/source/Playnite/Common/Programs.cs#L52-L78): `uninst`, `setup`, `unins\d+`, `Config`, `DXSETUP`, `vc_redist`, `vcredist`, `update`, `crashhandler`, `crashreport`, `helper`, `python(w).exe`…). **[verificado en este PC]**: los accesos de juegos de Steam son `.url` con `steam://rungameid/<appid>`, así que se deduplican contra Steam por appid.
4. **Carpetas vigiladas** elegidas por el usuario (`C:\Emulators`, `C:\personal\gow2-recomp\out\build\release`).

Problema propio de los builds: `gow2.exe` vive en 6 carpetas. La preferencia de Windows no admite comodines (no hay nada documentado), así que la app debería ofrecer "aplicar a todas las copias con el mismo nombre en este árbol" o que el port elija adaptador él mismo (el ReXGlue SDK es propio). Eso último es otra línea de investigación.

---

## 4. Cómo resolver el exe que dibuja (orden recomendado)

### 4.1 Cascada por juego

1. **Pista explícita de la tienda**: Epic `MainWindowProcessName`/`ProcessNames`; GDK `MicrosoftGame.config` `ExecutableList` o `GamingServices\GameConfig`; EA `installerdata.xml` `runtime/launcher/filePath`; GOG `playTasks[isPrimary]`; Ubisoft `executables[].path.relative`; Amazon `fuel.json`.
2. **Steam `appinfo.vdf`**: `launch` de Windows con `type=default` cuyo archivo exista; si `arguments` contiene `-run X.exe` o un `.exe` existente en la carpeta, ese es candidato a hijo.
3. **Heurística de motor**:
   - Unreal: si el exe declarado mide menos de unos 2 MB y existe `<Proyecto>\Binaries\Win64\*-Win64-Shipping.exe` o un exe con el mismo nombre en `Binaries\Win64` o `WinGDK`, se usa ese. rai-pal hace esta cascada en [unreal.rs:217-310](https://github.com/Raicuparta/rai-pal/blob/e4211d12727bbd4bfc63c0343b24fea084ee81e2/backend/core/src/game_engines/unreal.rs#L217-L310). **[verificado en este PC]** con Hogwarts Legacy y Chained Together; Halo Campaign Evolved ya declara el exe dentro de `Binaries\Win64`.
   - Unity: el exe es pequeño y dibuja él mismo con `UnityPlayer.dll`; señal: carpeta `<exe>_Data` y `UnityCrashHandler64.exe` ([unity.rs:91-130](https://github.com/Raicuparta/rai-pal/blob/e4211d12727bbd4bfc63c0343b24fea084ee81e2/backend/core/src/game_engines/unity.rs#L91-L130)). **[verificado en este PC]**: PEAK, Desktop Mate y Gamble With Your Friends tienen un exe de 0,7 MB y el exe más grande de la carpeta es `UnityCrashHandler64.exe`; aquí "el más grande" falla y el declarado es el correcto.
   - Excluir siempre: `CrashReportClient.exe`, `UnityCrashHandler*.exe`, `crashpad_handler.exe`, redistribuibles, `EAappInstaller.exe`, instaladores.
4. **Aprendizaje en ejecución** (cierra todos los casos raros: bootstrappers, JVM, anticheat): cuando corre un proceso cuya imagen está dentro del `installdir` de un juego conocido (o es hijo de su lanzador) y aparece en PDH `GPU Engine … engtype_3D`, se guarda como "exe que dibuja" de ese juego. Eso además **comprueba** si la preferencia funcionó (LUID observado contra LUID elegido).

### 4.2 Fuente pasiva de Windows: `HKCU\System\GameConfigStore\Children`

**[verificado en este PC, no documentado]**: Game Bar/Game Mode guarda una subclave por juego que reconoce (89 aquí). Las que tienen `Flags=17` traen `MatchedExeFullPath` (exe que corrió), `ExeParentDirectory`, `TitleId`, `GameDVR_GameGUID` y `LastAccessed` (FILETIME). El valor `Parent` es un blob DPAPI (cabecera `01 00 00 00 D0 8C 9D DF 01 15 D1 11 8C 7A 00 C0 4F C2 97 EB`). Las 17 rutas son justo los exes que dibujan: `cod.exe` (no `bootstrapper.exe`), `helldivers2.exe`, `UAGame.exe`, `VALORANT-Win64-Shipping.exe` y el `javaw.exe` de Minecraft Java. Es una sugerencia gratis, pero **no documentada**: usarla solo como pista y nunca como única fuente. También guarda juegos ya desinstalados (VALORANT, Skate, ABInfinite), así que hay que comprobar que el archivo exista.

---

## 5. Detectar instalaciones nuevas

Recomendación: **escaneo completo al iniciar y al abrir la ventana**, más **vigilancia por eventos** con reconciliación periódica (cada 10-15 min) como red de seguridad. Leer todos los manifiestos cuesta milisegundos: aquí son 25 `.acf` más un `appinfo.vdf` de 5 MB.

| Fuente | Qué vigilar | API |
|---|---|---|
| Steam | `<lib>\steamapps\` (no recursivo): crear, renombrar o modificar `appmanifest_*.acf`; `libraryfolders.vdf` para bibliotecas nuevas | `ReadDirectoryChangesW` (crate `notify`) |
| Steam (alternativa) | `HKCU\Software\Valve\Steam\Apps` (subárbol, `Installed`) | `RegNotifyChangeKeyValue` |
| Epic | `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests\*.item` | `ReadDirectoryChangesW` |
| GOG, Ubisoft, EA, Battle.net | `HKLM\…\Uninstall` (32 y 64 bits), `HKLM\…\Ubisoft\Launcher\Installs` | `RegNotifyChangeKeyValue` |
| Xbox / Store | eventos de paquetes | `PackageCatalog.OpenForCurrentUser()` → `PackageInstalling` (con `IsComplete`), `PackageUninstalling`, `PackageUpdating`, `PackageStatusChanged` |
| Exes sueltos | proceso nuevo con uso de 3D | muestreo de PDH (Corral ya muestrea) |

Detalles que obligan a reconciliar:
- `ReadDirectoryChangesW`: si el búfer se desborda, devuelve 0 bytes o `ERROR_NOTIFY_ENUM_DIR` y hay que volver a enumerar; con búferes de más de 64 KB falla en unidades de red ([docs](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw)).
- `RegNotifyChangeKeyValue` avisa **una sola vez**: hay que volver a registrarse después de cada aviso, no dice qué cambió, el evento se dispara si muere el hilo que se registró (salvo con `REG_NOTIFY_THREAD_AGNOSTIC`, Win8+) y registrarse dos veces con los mismos parámetros pierde recursos ([docs](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regnotifychangekeyvalue)).
- `PackageCatalog.OpenForCurrentUser` funciona desde una app de escritorio sin identidad de paquete: necesita la capacidad `packageQuery`/`packageManagement` **o** nivel de integridad Medium o mayor ([docs](https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.packagecatalog.openforcurrentuser)).
- Durante una descarga, Steam reescribe el `.acf` muchas veces: aplicar *debounce* y solo dar el juego por nuevo cuando `StateFlags` tenga el bit 4 y ninguno de 1024/1048576/2097152 (UpdateStarted/Downloading/Staging).
- Al desinstalar, el exe desaparece pero la entrada de `UserGpuPreferences` queda (caso RainbowSix/VALORANT): marcarla como huérfana en la siguiente reconciliación.

---

## 6. Nombres e iconos

| Fuente | Nombre | Icono / arte |
|---|---|---|
| Steam | `appinfo.vdf` `common/name` (si no, `.acf` `name`) | `steam\games\<clienticon>.ico`; `appcache\librarycache\<appid>\header.jpg` / `library_600x900.jpg` / `logo.png` |
| Epic | `.item` `DisplayName` | icono del exe |
| GOG | `GamePieces.originalTitle` / Uninstall `DisplayName` | `originalImages.squareIcon` (URL) o el icono del exe |
| GDK / Store | `MicrosoftGame.config` `ShellVisuals@DefaultDisplayName`; AppxManifest `Properties/DisplayName` (puede ser `ms-resource:`; Playnite lo resuelve en [Programs2.cs:370-378](https://github.com/JosefNemec/Playnite/blob/ecee01452a25db1fb2674a69f883d298d4c7be8a/source/Playnite/Common/Programs2.cs#L370-L378)) | `Square150x150Logo` → `Square70x70Logo` → `Square44x44Logo` → `Logo`, con calificadores de escala en el nombre del archivo ([Programs2.cs:349-368](https://github.com/JosefNemec/Playnite/blob/ecee01452a25db1fb2674a69f883d298d4c7be8a/source/Playnite/Common/Programs2.cs#L349-L368)) |
| Exe suelto | `VS_VERSIONINFO` `ProductName`/`FileDescription` (Playnite usa `ProductName` y, si falta, la carpeta: [Programs2.cs:61-73](https://github.com/JosefNemec/Playnite/blob/ecee01452a25db1fb2674a69f883d298d4c7be8a/source/Playnite/Common/Programs2.cs#L61-L73)) | `IShellItemImageFactory::GetImage` sobre la ruta; sirve también para `shell:AppsFolder\<AUMID>` en apps empaquetadas |

Unificado: `SHCreateItemFromParsingName(ruta o "shell:AppsFolder\<AUMID>")` + `IShellItemImageFactory::GetImage(SIIGBF_ICONONLY, 64x64)` da un icono para cualquier entrada sin código por tienda **[inferido: API estándar de la shell, no probada aquí]**.

---

## 7. Implicaciones para la app y para Corral

- Modelo de datos: `Game { source, source_id, name, install_dir, launch_target (exe, URI o AUMID), render_exes: Vec<PathBuf | AUMID>, render_exe_origin: manifest|engine-heuristic|observed|user, icon }`. La preferencia de GPU se escribe por cada `render_exe`, nunca por el lanzador.
- Prioridad de implementación para este PC: Steam (22 juegos) → aprendizaje en ejecución con PDH (cubre exes sueltos, gow2 y bootstrappers) → Xbox/GDK (3 paquetes) → Epic/GOG/EA/Ubisoft/Battle.net/Riot como proveedores que se activan solos si existen sus rutas.
- Rust: `steamlocate` (MIT) para bibliotecas y manifiestos, parser propio de `appinfo.vdf` (unas 100 líneas; el de `tools\steam_appinfo_probe.py` ya está verificado contra v41), crate `windows` (`Management_Deployment`, `ApplicationModel` para `PackageManager`/`PackageCatalog`, `Win32_System_Registry` para `RegNotifyChangeKeyValue`, `Win32_UI_Shell` para iconos), `rusqlite` en solo lectura para GOG/itch/Amazon, `notify` para carpetas. rai-pal (GPL-3.0, Tauri + Rust) es la referencia más cercana y su licencia es compatible con AGPL-3.0-only si se copia código; Playnite, SteamAppInfo y steamlocate son MIT.
- UX: lista de juegos agrupada por tienda + pestaña "Corriendo ahora" con GPU observada contra GPU elegida + sección "Entradas huérfanas en Windows" (UserGpuPreferences cuyo exe ya no existe) + "Aplicar a todas las copias" para builds.

---

## 8. Preguntas abiertas y cómo cerrarlas en este PC

1. **¿Qué clave consulta DXGI para un juego GDK** (`Minecraft.Windows.exe` lanzado por `GameLaunchHelper.exe`): el AUMID `Microsoft.MinecraftUWP_8wekyb3d8bbwe!Game`, la ruta versionada de WindowsApps o la ruta de `C:\XboxGames\…\Content`? Prueba: el usuario agrega Minecraft como "Microsoft Store app" en Configuración > Pantalla > Gráficos, después se lee `UserGpuPreferences` (solo lectura) para ver el nombre del valor, se abre el juego y se mira en PDH en qué LUID dibuja `Minecraft.Windows.exe`.
2. **¿Una preferencia sobre el stub de Unreal llega al hijo?** (Hogwarts Legacy raíz contra `Phoenix\Binaries\Win64`). Se espera que no, porque la consulta es por proceso **[inferido]**. Prueba: preferencia solo en el stub y mirar el LUID del hijo con PDH.
3. **¿La ruta de `javaw.exe` de Minecraft Java cambia** al actualizarse el runtime (`java-runtime-epsilon` → siguiente)? Si cambia, la preferencia se pierde; el aprendizaje en ejecución debería volver a aplicarla.
4. **Riot `product_settings.yaml`**: confirmar ruta y campos cuando VALORANT vuelva a estar instalado.
5. **Epic `.item` y GOG `galaxy-2.0.db`**: probar con al menos un juego instalado (aquí no hay ninguno).
6. **Segunda entrada DXGI de la RX 7800 XT** (LUID `0x2764F`): confirmar si es el adaptador de la pantalla virtual `IddSampleDriver`. Importa para quien diseñe "elegir GPU": puede salir una 7800 XT repetida en la lista.
