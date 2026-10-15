# Cambiavía

Cambiavía es una app open-source para Windows que deja elegir qué GPU usa cada juego o aplicación y cuál es la GPU predeterminada para juegos, explicando por qué Windows eligió la que eligió y comprobando que el cambio funcionó. Lee y escribe las preferencias de GPU por app de Windows (`HKCU\Software\Microsoft\DirectX\UserGpuPreferences`) de forma segura: vista previa antes de escribir, copia de seguridad, diario para deshacer y verificación de lectura.

El proyecto está en desarrollo, todavía no hay versión. El núcleo de preferencias vive en `crates/cambiavia-core` (Rust, solo std, sin dependencias) y el diseño completo está en [docs/](docs/) (`docs/SPEC.md` y `docs/research/`).

Licencia: AGPL-3.0-only, ver [LICENSE](LICENSE).
