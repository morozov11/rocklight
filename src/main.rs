#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
compile_error!("rockLight предназначен для Windows 11.");

use hidapi::{HidApi, HidDevice};
use std::{
    error::Error,
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
    ptr,
    sync::atomic::{AtomicI32, Ordering},
    thread,
    time::Duration,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIconBuilder,
};
use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM},
    System::{
        LibraryLoader::GetModuleHandleW,
        Power::{RegisterPowerSettingNotification, UnregisterPowerSettingNotification},
    },
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, PeekMessageW, RegisterClassW,
        TranslateMessage, WNDCLASSW, MSG, HWND_MESSAGE, PM_REMOVE, PBT_POWERSETTINGCHANGE,
        WM_POWERBROADCAST,
    },
};

const LID_GUID: Guid = Guid {
    data1: 0xBA3E0F4D,
    data2: 0xB817,
    data3: 0x4094,
    data4: [0xA2, 0xD1, 0xD5, 0x63, 0x79, 0xE6, 0xA0, 0xF3],
};
const MSI_VID: u16 = 0x1462;
const MSI_PID: u16 = 0x1563;
static LID_STATE: AtomicI32 = AtomicI32::new(-1);

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct PowerBroadcastSetting {
    setting: Guid,
    data_length: u32,
    data: [u8; 1],
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_POWERBROADCAST && wparam == PBT_POWERSETTINGCHANGE as usize {
        let setting = lparam as *const PowerBroadcastSetting;
        if !setting.is_null() {
            let setting = &*setting;
            if same_guid(setting.setting, LID_GUID) && setting.data_length >= 4 {
                let value = ptr::read_unaligned(setting.data.as_ptr() as *const u32);
                LID_STATE.store(if value == 0 { 0 } else { 1 }, Ordering::Relaxed);
            }
        }
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

fn same_guid(a: Guid, b: Guid) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}

struct Lighting(HidDevice);

impl Lighting {
    fn open() -> Result<Self, Box<dyn Error>> {
        let api = HidApi::new()?;
        let devices: Vec<_> = api
            .device_list()
            .filter(|d| d.vendor_id() == MSI_VID && d.product_id() == MSI_PID)
            .collect();
        if devices.is_empty() {
            return Err(io::Error::other("не найдено HID-устройство MSI MysticLight MS-1563 (1462:1563)").into());
        }

        let info = devices
            .iter()
            .find(|d| d.product_string().is_some_and(|s| s.contains("MysticLight MS-1563")))
            .copied()
            .unwrap_or(devices[0]);
        let name = info.product_string().unwrap_or("MSI MysticLight MS-1563");
        log(&format!(
            "HID устройство: {name}; интерфейс {}; usage page {:04X}, usage {:04X}",
            info.interface_number(), info.usage_page(), info.usage()
        ));
        Ok(Self(info.open_device(&api)?))
    }

    fn set(&self, on: bool) -> Result<(), Box<dyn Error>> {
        // OpenRGB MSIMysticLight64Controller: FeaturePacket_64, report 0x02.
        let mut packet = [0u8; 64];
        packet[0] = 0x02;
        packet[2] = if on { 1 } else { 0 }; // static / disable
        packet[3] = 0; // speed
        packet[4] = 10; // full brightness; OFF mode disables the LEDs
        packet[5] = if on { 1 } else { 0 }; // number of colors
        if on {
            packet[6..9].copy_from_slice(&[52, 235, 153]); // #34EB99
        }
        self.0.send_feature_report(&packet)?;
        log(if on { "Отправлена команда: светло-зелёный" } else { "Отправлена команда: выключить" });
        Ok(())
    }
}

fn log(message: &str) {
    let directory = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from)).unwrap_or_default());
    let directory = directory.join("rockLight");
    let _ = std::fs::create_dir_all(&directory);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(directory.join("rockLight.log")) {
        let _ = writeln!(file, "{message}");
    }
}

fn icon() -> Icon {
    let mut rgba = vec![0u8; 32 * 32 * 4];
    for y in 0..32usize {
        for x in 0..32usize {
            let body = (5..27).contains(&x) && (4..22).contains(&y);
            let base = (3..29).contains(&x) && (23..27).contains(&y);
            let border = body && (x == 5 || x == 26 || y == 4 || y == 21);
            let key = body && (7..25).contains(&x) && (6..20).contains(&y)
                && ((x - 7) % 5 <= 2) && ((y - 6) % 5 <= 2);
            let lit = (x == 15 || x == 16) && (y == 11 || y == 12);
            let (r, g, b, a) = if lit || key {
                (52, 235, 153, 255)
            } else if border || base {
                (42, 57, 49, 255)
            } else if body {
                (22, 31, 27, 255)
            } else {
                (0, 0, 0, 0)
            };
            let offset = (y * 32 + x) * 4;
            rgba[offset..offset + 4].copy_from_slice(&[r, g, b, a]);
        }
    }
    Icon::from_rgba(rgba, 32, 32).expect("32x32 tray icon")
}

fn main() {
    log("Запуск rockLight");
    if let Err(error) = run() {
        log(&format!("Ошибка запуска: {error}"));
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let lighting = Lighting::open()?;
    let menu = Menu::new();
    let turn_on = MenuItem::new("Включить светло-зелёную подсветку", true, None);
    let turn_off = MenuItem::new("Выключить подсветку", true, None);
    let quit = MenuItem::new("Выйти", true, None);
    menu.append(&turn_on)?;
    menu.append(&turn_off)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit)?;
    let _tray = TrayIconBuilder::new()
        .with_icon(icon())
        .with_tooltip("rockLight — подсветка MSI Pulse GL76")
        .with_menu(Box::new(menu))
        .build()?;

    let class_name: Vec<u16> = "rockLightPowerWindow\0".encode_utf16().collect();
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    if instance.is_null() { return Err(io::Error::other(format!("GetModuleHandleW: {}", unsafe { GetLastError() })).into()); }
    let class = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: ptr::null_mut(),
        hCursor: ptr::null_mut(),
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err(io::Error::other(format!("RegisterClassW: {}", unsafe { GetLastError() })).into());
    }
    let window = unsafe {
        CreateWindowExW(0, class_name.as_ptr(), class_name.as_ptr(), 0, 0, 0, 0, 0,
            HWND_MESSAGE, ptr::null_mut(), instance, ptr::null())
    };
    if window.is_null() { return Err(io::Error::other(format!("CreateWindowExW: {}", unsafe { GetLastError() })).into()); }
    let power_notify = unsafe {
        RegisterPowerSettingNotification(window, &LID_GUID as *const Guid as *const _, 0)
    };
    if power_notify == 0 {
        return Err(io::Error::other(format!("Не удалось подписаться на датчик крышки: {}", unsafe { GetLastError() })).into());
    }

    if let Err(error) = lighting.set(true) { log(&format!("Стартовое включение: {error}")); }
    let mut light_on = true;
    let menu_events = MenuEvent::receiver();

    loop {
        unsafe {
            let mut message: MSG = std::mem::zeroed();
            while PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        while let Ok(event) = menu_events.try_recv() {
            if event.id == turn_on.id() {
                light_on = true;
                if let Err(error) = lighting.set(true) { log(&format!("Включение: {error}")); }
            } else if event.id == turn_off.id() {
                light_on = false;
                if let Err(error) = lighting.set(false) { log(&format!("Выключение: {error}")); }
            } else if event.id == quit.id() {
                unsafe { UnregisterPowerSettingNotification(power_notify); }
                return Ok(());
            }
        }

        match LID_STATE.swap(-1, Ordering::Relaxed) {
            0 if light_on => {
                light_on = false;
                if let Err(error) = lighting.set(false) { log(&format!("Закрытие крышки: {error}")); }
            }
            1 => {
                light_on = true;
                if let Err(error) = lighting.set(true) { log(&format!("Открытие крышки: {error}")); }
            }
            _ => {}
        }
        thread::sleep(Duration::from_millis(40));
    }
}