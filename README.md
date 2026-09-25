# rockLight

A small Windows tray app for controlling the keyboard backlight on the MSI Pulse GL76 12UEK.

## Download

[Download rocklight.exe from the latest release](https://github.com/morozov11/rocklight/releases/latest/download/rocklight.exe).

## Why this app exists

MSI's Mystic Light software is unreliable on this laptop: sometimes it fails to turn the backlight on, and sometimes the keyboard lighting stops responding altogether. OpenRGB can detect and control the `MSI MysticLight MS-1563` device, but it does not automatically turn the backlight off when the laptop lid is closed. rockLight combines direct lighting control with automatic lid handling.

## Features

- Turn the keyboard backlight on in green (`#34EB99`) or off from the system tray.
- Turn the light off when the lid closes and back on when it opens.
- Exit from the tray menu.
- Uses the laptop's HID device (`1462:1563`) directly, following the 64-byte feature-report protocol used by OpenRGB.

rockLight does not require the OpenRGB SDK server or the MSI Mystic Light SDK. Close OpenRGB while rockLight is running so both apps do not compete to control the same device.

## Build

Install the Rust stable toolchain, then run in PowerShell from the project directory:

```powershell
cargo build --release --locked
```

The executable will be at `target\release\rocklight.exe`.

## Run at sign-in

Create a shortcut to `target\release\rocklight.exe` in the Windows Startup folder. To open that folder, press `Win+R`, enter `shell:startup`, and press Enter.

## Log

The application writes its log to `%LOCALAPPDATA%\rockLight\rockLight.log`.
