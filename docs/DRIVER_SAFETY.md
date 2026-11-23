# Driver diagnostics and backup safety

## Scope (developer preview)

DevMove's native Rust backend currently provides **read-only Windows device inventory** and **explicit driver package export**. It does not install, replace, remove, or automatically update any driver.

Device inventory combines Win32_PnPSignedDriver with Win32_PnPEntity error codes. A device without a detected signed driver remains visible when Windows reports a problem. Windows ConfigManagerErrorCode 28 normally means no driver is installed for the device. An error code does not itself prove a specific downloadable package is correct.

The **Export driver packages** action asks for a destination folder, then creates a separate DevMove_Drivers_<timestamp> directory and uses Windows built-in PnPUtil with its export-driver command. It exports eligible third-party packages from the Windows driver store, not necessarily every in-box Microsoft driver, firmware updater, device application, or OEM installer.

## Limitations

- Some Windows operations require elevated permissions; DevMove does not elevate itself or bypass Windows security.
- Driver export is a fallback for reinstalling supported existing hardware, not a guarantee of best-current driver versions.
- Official OEM hardware identification, compatible driver matching, package signature verification, release comparisons, and restore-point creation are **planned**, not implemented.
- Windows Update and OEM manufacturer tooling remain the primary ways to obtain compatible drivers until those functions are implemented and tested.
- Driver Store export may include older versions and packages not used by the current hardware. Never install them automatically.
- For Wi-Fi adapters, obtain an OEM-supported network driver installer as an independent offline fallback before reinstalling Windows.

## Future update flow

1. Read device hardware IDs and installed driver/provider/version.
2. Identify signed OEM or Microsoft-distributed candidates for the **same hardware IDs**.
3. Compare compatibility, driver date, known-blocked versions, Windows version, and architecture.
4. Present individual updates with package origin, version, size and any relevant risk.
5. Check privileges, disk free space and restore-point capability.
6. Attempt to create and verify a restore point **before** installing any driver.
7. Require explicit confirmation; install only selected trusted packages.
8. Record outcome, reboot requirements, failure and rollback guidance.

A driver restore point is not a substitute for a complete backup and may be disabled on the target system. Never claim rollback is possible without checking the real recovery state.

## Security rules

- Do not fetch arbitrary driver EXEs from search results or community mirrors.
- Do not reverse engineer proprietary drivers or updater services to bypass safeguards.
- Do not let missing/outdated flags silently trigger driver installation.
- Verify driver signatures and hardware IDs before any future update action.
- Do not upload exported binaries or hardware/device identifiers to GitHub.
