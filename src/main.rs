#![windows_subsystem = "windows"]
#![allow(unsafe_op_in_unsafe_fn)]
mod alpha;
mod backdrop;
mod battery;
mod media;
mod render;
mod settings;
mod sync;
use settings::Settings;
use std::{
    cell::{Cell, RefCell},
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{LibraryLoader::*, Registry::*, Threading::*},
    UI::{
        Controls::Dialogs::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*,
        WindowsAndMessaging::*,
    },
};

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
const UPDATE: u32 = WM_APP + 1;
const TRAY: u32 = WM_APP + 2;
const MOUSE_LEAVE: u32 = 0x02A3;
const MODE_BAR: u32 = 10;
const MODE_FLOAT: u32 = 11;
const SHOW_PLAYER: u32 = 12;
const BATTERY_ONLY: u32 = 13;
const AUTO_HIDE: u32 = 14;
const BG_AUTO: u32 = 20;
const BG_COLOR: u32 = 21;
const ICON_COLOR: u32 = 22;
const PROGRESS_COLOR: u32 = 23;
const RESET_COLORS: u32 = 24;
const BG_TRANSPARENT: u32 = 25;
const FLOAT_BLUR: u32 = 26;
const BAT_REFRESH: u32 = 30;
const AUTO_SYNC: u32 = 31;
const STARTUP: u32 = 32;
const REPOSITORY: u32 = 33;
const CONNECT: u32 = 34;
const EXIT: u32 = 99;
#[derive(Clone)]
struct Data {
    music: media::Music,
    battery: battery::Battery,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            music: media::Music {
                title: "Nada tocando agora".into(),
                artist: "Abra seu player de música".into(),
                ..Default::default()
            },
            battery: battery::Battery::default(),
        }
    }
}
#[derive(Clone, Copy)]
struct Host {
    shell: HWND,
    rebar: HWND,
    buttons: HWND,
    original_rebar: RECT,
    original_buttons: RECT,
}
struct App {
    controller: Cell<HWND>,
    widget: Cell<HWND>,
    settings: RefCell<Settings>,
    host: RefCell<Option<Host>>,
    data: Arc<Mutex<Data>>,
    cache: RefCell<render::Cache>,
    hover: Cell<u32>,
    background: Cell<u32>,
    running: Arc<AtomicBool>,
    sync_enabled: Arc<AtomicBool>,
    media_enabled: Arc<AtomicBool>,
    sync_status: Arc<Mutex<String>>,
    media_tx: mpsc::Sender<u32>,
    battery_tx: mpsc::Sender<()>,
    taskbar_message: u32,
    quitting: Cell<bool>,
    dragging: Cell<bool>,
    blurred: Cell<bool>,
}
thread_local! {static APP:Cell<*const App>=const{Cell::new(null())};}
unsafe fn relative(hwnd: HWND) -> RECT {
    let mut r = zeroed();
    GetWindowRect(hwnd, &mut r);
    let mut p = POINT {
        x: r.left,
        y: r.top,
    };
    ScreenToClient(GetParent(hwnd), &mut p);
    RECT {
        left: p.x,
        top: p.y,
        right: p.x + r.right - r.left,
        bottom: p.y + r.bottom - r.top,
    }
}
unsafe fn restore(host: &Host) {
    for (h, r) in [
        (host.rebar, host.original_rebar),
        (host.buttons, host.original_buttons),
    ] {
        if IsWindow(h) != 0 {
            SetWindowPos(
                h,
                null_mut(),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }
    }
}
unsafe fn create_widget(app: &App) {
    let style = if app.settings.borrow().floating {
        WS_POPUP
    } else {
        WS_CHILD
    };
    let parent = if app.settings.borrow().floating {
        null_mut()
    } else {
        FindWindowW(wide("Shell_TrayWnd").as_ptr(), null())
    };
    if style == WS_CHILD && parent.is_null() {
        return;
    }
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
        wide("CantoRustWidget").as_ptr(),
        wide("Canto").as_ptr(),
        style,
        0,
        0,
        420,
        40,
        parent,
        null_mut(),
        GetModuleHandleW(null()),
        null(),
    );
    app.widget.set(hwnd);
    app.blurred
        .set(backdrop::apply(hwnd, app.settings.borrow().is_blurred()));
}
impl App {
    unsafe fn layout(&self) {
        if self.quitting.get() {
            return;
        }
        let s = self.settings.borrow().clone();
        if s.floating && self.dragging.get() {
            return;
        }
        let available = self.data.lock().unwrap().music.available;
        let width = s.width(available);
        let mut hwnd = self.widget.get();
        if hwnd.is_null() || IsWindow(hwnd) == 0 {
            create_widget(self);
            hwnd = self.widget.get();
            if hwnd.is_null() {
                return;
            }
        }
        if s.floating {
            let previous_host = self.host.borrow_mut().take();
            if let Some(host) = previous_host {
                restore(&host);
            }
            if GetParent(hwnd) != null_mut() {
                SetParent(hwnd, null_mut());
                SetWindowLongPtrW(hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE) as isize);
            }
            let dpi = GetDpiForWindow(hwnd).max(96);
            let w = width * dpi as i32 / 96;
            let h = 64 * dpi as i32 / 96;
            let mut current: RECT = zeroed();
            GetWindowRect(hwnd, &mut current);
            let mut area: RECT = zeroed();
            SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut area as *mut RECT).cast(), 0);
            let x = s
                .left
                .unwrap_or(area.right - w - 12)
                .clamp(area.left, (area.right - w).max(area.left));
            let y = s
                .top
                .unwrap_or(area.bottom - h - 8)
                .clamp(area.top, (area.bottom - h).max(area.top));
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                x,
                y,
                w,
                h,
                SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
            );
            let region = CreateRoundRectRgn(0, 0, w + 1, h + 1, 14, 14);
            if SetWindowRgn(hwnd, region, 1) == 0 {
                DeleteObject(region);
            }
            self.background
                .set(if s.is_blurred() { 0x202226 } else { 0x072234 });
        } else {
            let shell = FindWindowW(wide("Shell_TrayWnd").as_ptr(), null());
            if shell.is_null() {
                return;
            }
            let need = self
                .host
                .borrow()
                .as_ref()
                .is_none_or(|h| h.shell != shell || IsWindow(h.rebar) == 0);
            if need {
                let previous_host = self.host.borrow_mut().take();
                if let Some(old) = previous_host {
                    restore(&old);
                }
                let rebar =
                    FindWindowExW(shell, null_mut(), wide("ReBarWindow32").as_ptr(), null());
                let buttons =
                    FindWindowExW(rebar, null_mut(), wide("MSTaskSwWClass").as_ptr(), null());
                if rebar.is_null() || buttons.is_null() {
                    ShowWindow(hwnd, SW_HIDE);
                    return;
                }
                *self.host.borrow_mut() = Some(Host {
                    shell,
                    rebar,
                    buttons,
                    original_rebar: relative(rebar),
                    original_buttons: relative(buttons),
                });
            }
            if GetParent(hwnd) != shell {
                SetWindowLongPtrW(
                    hwnd,
                    GWL_STYLE,
                    (WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS) as isize,
                );
                SetParent(hwnd, shell);
                SetWindowRgn(hwnd, null_mut(), 1);
            }
            let mut client: RECT = zeroed();
            GetClientRect(shell, &mut client);
            if client.right < client.bottom * 3 {
                ShowWindow(hwnd, SW_HIDE);
                return;
            }
            let tray = FindWindowExW(shell, null_mut(), wide("TrayNotifyWnd").as_ptr(), null());
            if tray.is_null() {
                return;
            }
            let right = relative(tray).left;
            let dpi = GetDpiForWindow(shell).max(96) as i32;
            let pixels = width * dpi / 96;
            let left = right - pixels;
            if left < 100 {
                ShowWindow(hwnd, SW_HIDE);
                return;
            }
            let host = *self.host.borrow();
            if let Some(h) = host.as_ref() {
                let r = h.original_rebar;
                let b = h.original_buttons;
                let available = left - r.left;
                SetWindowPos(
                    h.rebar,
                    null_mut(),
                    r.left,
                    r.top,
                    available,
                    r.bottom - r.top,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
                SetWindowPos(
                    h.buttons,
                    null_mut(),
                    b.left,
                    b.top,
                    (available - b.left).max(1),
                    b.bottom - b.top,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
            }
            SetWindowPos(
                hwnd,
                HWND_TOP,
                left,
                0,
                pixels,
                client.bottom,
                SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
            );
            let mut rect: RECT = zeroed();
            GetWindowRect(hwnd, &mut rect);
            let dc = GetDC(null_mut());
            let mut channels = [0u32; 3];
            let mut count = 0;
            for offset in [4, 6, 8, 10, 12] {
                let c = GetPixel(dc, rect.left - offset, rect.top + 2);
                if c != 0xffffffff {
                    let rgb = render::rgb(c);
                    channels[0] += (rgb >> 16) & 255;
                    channels[1] += (rgb >> 8) & 255;
                    channels[2] += rgb & 255;
                    count += 1;
                }
            }
            ReleaseDC(null_mut(), dc);
            if count > 0 {
                let bg = ((channels[0] / count) << 16)
                    | ((channels[1] / count) << 8)
                    | (channels[2] / count);
                if self.background.replace(bg) != bg {
                    InvalidateRect(hwnd, null(), 0);
                }
            }
        }
    }
    unsafe fn apply(&self) {
        self.settings.borrow().save();
        self.media_enabled
            .store(self.settings.borrow().show_player, Ordering::Relaxed);
        let _ = self.media_tx.send(0);
        self.layout();
        let blurred = self.settings.borrow().is_blurred();
        self.blurred
            .set(backdrop::apply(self.widget.get(), blurred));
        InvalidateRect(self.widget.get(), null(), 0);
    }
    unsafe fn action(&self, id: u32) {
        if (1..=3).contains(&id) {
            if self.data.lock().unwrap().music.available {
                let _ = self.media_tx.send(id);
            }
            return;
        }
        match id {
            MODE_BAR | MODE_FLOAT => {
                self.settings.borrow_mut().floating = id == MODE_FLOAT;
                // Create a window with its final parent/style instead of moving
                // an existing HWND across Explorer's DPI and ownership context.
                let old = self.widget.replace(null_mut());
                let previous_host = self.host.borrow_mut().take();
                if let Some(host) = previous_host {
                    restore(&host);
                }
                if !old.is_null() {
                    DestroyWindow(old);
                }
                self.apply();
            }
            SHOW_PLAYER | BATTERY_ONLY => {
                self.settings.borrow_mut().show_player = id == SHOW_PLAYER;
                self.apply();
            }
            AUTO_HIDE => {
                let mut s = self.settings.borrow_mut();
                s.auto_hide_player = !s.auto_hide_player;
                drop(s);
                self.apply();
            }
            BG_AUTO | BG_TRANSPARENT => {
                let mut s = self.settings.borrow_mut();
                s.background = None;
                s.transparent = id == BG_TRANSPARENT;
                s.floating_blur = id == BG_AUTO;
                drop(s);
                self.apply();
            }
            FLOAT_BLUR => {
                let mut s = self.settings.borrow_mut();
                s.floating_blur = !s.floating_blur;
                if s.floating_blur {
                    s.transparent = false;
                }
                drop(s);
                self.apply();
            }
            BG_COLOR | ICON_COLOR | PROGRESS_COLOR => {
                let s = self.settings.borrow().clone();
                let mut colors = [0u32; 16];
                let initial = match id {
                    BG_COLOR => s.background.unwrap_or(self.background.get()),
                    ICON_COLOR => s.icon_color,
                    _ => s.progress_color,
                };
                let mut dialog: CHOOSECOLORW = zeroed();
                dialog.lStructSize = size_of::<CHOOSECOLORW>() as u32;
                dialog.hwndOwner = self.controller.get();
                dialog.rgbResult = render::color(initial);
                dialog.lpCustColors = colors.as_mut_ptr();
                dialog.Flags = CC_RGBINIT | CC_FULLOPEN | CC_ENABLEHOOK;
                dialog.lpfnHook = Some(color_hook);
                let chosen = ChooseColorW(&mut dialog);
                if chosen != 0 {
                    let mut s = self.settings.borrow_mut();
                    let c = render::rgb(dialog.rgbResult);
                    match id {
                        BG_COLOR => {
                            s.background = Some(c);
                            s.transparent = false;
                        }
                        ICON_COLOR => s.icon_color = c,
                        _ => s.progress_color = c,
                    };
                    drop(s);
                    self.apply();
                }
            }
            RESET_COLORS => {
                let mut s = self.settings.borrow_mut();
                s.background = None;
                s.transparent = false;
                s.floating_blur = true;
                s.icon_color = 0x85e4cf;
                s.progress_color = 0x538e91;
                drop(s);
                self.apply();
            }
            BAT_REFRESH => {
                let _ = self.battery_tx.send(());
            }
            AUTO_SYNC => {
                let mut s = self.settings.borrow_mut();
                s.auto_sync = !s.auto_sync;
                self.sync_enabled.store(s.auto_sync, Ordering::Relaxed);
                s.save();
            }
            STARTUP => toggle_startup(),
            REPOSITORY => open("https://github.com/andradehub/canto-widget"),
            CONNECT => {
                if let Some(repo) = sync::repository() {
                    let script = repo.join("scripts/Connect-GitHub.ps1");
                    let args = format!(
                        "-NoProfile -ExecutionPolicy Bypass -File \"{}\"",
                        script.display()
                    );
                    ShellExecuteW(
                        self.controller.get(),
                        wide("open").as_ptr(),
                        wide("powershell.exe").as_ptr(),
                        wide(&args).as_ptr(),
                        null(),
                        SW_SHOWNORMAL,
                    );
                }
            }
            EXIT => {
                self.quitting.set(true);
                self.running.store(false, Ordering::Relaxed);
                let previous_host = self.host.borrow_mut().take();
                if let Some(h) = previous_host {
                    restore(&h);
                }
                notify(self.controller.get(), NIM_DELETE);
                DestroyWindow(self.widget.get());
                DestroyWindow(self.controller.get());
            }
            _ => {}
        }
    }
    unsafe fn menu(&self) {
        let menu = CreatePopupMenu();
        let s = self.settings.borrow().clone();
        let data = self.data.lock().unwrap().clone();
        let battery = data
            .battery
            .percent
            .map(|v| format!("{v}%"))
            .unwrap_or_else(|| "—".into());
        item(
            menu,
            0,
            &format!("AJAZZ AJ139 Pro · {battery}"),
            false,
            true,
        );
        if data.music.available {
            item(menu, 0, &data.music.title, false, true);
        }
        separator(menu);
        let view = CreatePopupMenu();
        item(view, SHOW_PLAYER, "Player e bateria", s.show_player, false);
        item(view, BATTERY_ONLY, "Somente bateria", !s.show_player, false);
        separator(view);
        item(
            view,
            AUTO_HIDE,
            "Ocultar player quando não houver mídia",
            s.auto_hide_player,
            false,
        );
        submenu(menu, view, "Exibição");
        let position = CreatePopupMenu();
        item(
            position,
            MODE_BAR,
            "Integrado à barra de tarefas",
            !s.floating,
            false,
        );
        item(
            position,
            MODE_FLOAT,
            "Flutuante · arrastar livremente",
            s.floating,
            false,
        );
        submenu(menu, position, "Posição");
        let colors = CreatePopupMenu();
        item(
            colors,
            BG_AUTO,
            "Automático · transparente na barra",
            s.background.is_none() && !s.transparent,
            false,
        );
        item(
            colors,
            BG_TRANSPARENT,
            "Fundo totalmente transparente",
            s.background.is_none() && s.transparent,
            false,
        );
        item(
            colors,
            FLOAT_BLUR,
            "Desfoque no modo flutuante",
            s.floating_blur && !s.transparent,
            false,
        );
        item(colors, BG_COLOR, "Escolher cor do fundo…", false, false);
        item(colors, ICON_COLOR, "Escolher cor dos ícones…", false, false);
        item(
            colors,
            PROGRESS_COLOR,
            "Escolher cor do progresso…",
            false,
            false,
        );
        separator(colors);
        item(colors, RESET_COLORS, "Restaurar cores padrão", false, false);
        submenu(menu, colors, "Cores");
        separator(menu);
        item(menu, BAT_REFRESH, "Atualizar bateria agora", false, false);
        item(
            menu,
            STARTUP,
            "Iniciar com o Windows",
            startup_enabled(),
            false,
        );
        item(
            menu,
            AUTO_SYNC,
            "Sincronização automática",
            s.auto_sync,
            false,
        );
        item(menu, 0, &self.sync_status.lock().unwrap(), false, true);
        item(menu, REPOSITORY, "Abrir projeto no GitHub", false, false);
        item(menu, CONNECT, "Conectar e publicar no GitHub", false, false);
        separator(menu);
        item(menu, EXIT, "Sair", false, false);
        let hwnd = self.widget.get();
        let mut point = POINT::default();
        GetCursorPos(&mut point);
        SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            point.x,
            point.y,
            0,
            hwnd,
            null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
        if chosen > 0 {
            self.action(chosen as u32);
        }
    }
}
unsafe extern "system" fn color_hook(hwnd: HWND, message: u32, _: WPARAM, _: LPARAM) -> usize {
    if message == WM_INITDIALOG {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        SetForegroundWindow(hwnd);
    }
    0
}
unsafe fn item(menu: HMENU, id: u32, label: &str, checked: bool, disabled: bool) {
    AppendMenuW(
        menu,
        MF_STRING | if checked { MF_CHECKED } else { 0 } | if disabled { MF_GRAYED } else { 0 },
        id as usize,
        wide(label).as_ptr(),
    );
}
unsafe fn separator(menu: HMENU) {
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
}
unsafe fn submenu(menu: HMENU, child: HMENU, label: &str) {
    AppendMenuW(menu, MF_POPUP, child as usize, wide(label).as_ptr());
}
unsafe fn open(url: &str) {
    ShellExecuteW(
        null_mut(),
        wide("open").as_ptr(),
        wide(url).as_ptr(),
        null(),
        null(),
        SW_SHOWNORMAL,
    );
}
unsafe fn notify(hwnd: HWND, message: u32) {
    let mut data: NOTIFYICONDATAW = zeroed();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    data.uCallbackMessage = TRAY;
    data.hIcon = LoadIconW(null_mut(), IDI_INFORMATION);
    let tip = wide("Canto · Rust · música e bateria");
    data.szTip[..tip.len()].copy_from_slice(&tip);
    Shell_NotifyIconW(message, &data);
}
unsafe fn startup_enabled() -> bool {
    let mut key = null_mut();
    if RegOpenKeyExW(
        HKEY_CURRENT_USER,
        wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run").as_ptr(),
        0,
        KEY_QUERY_VALUE,
        &mut key,
    ) != 0
    {
        return false;
    }
    let result = RegQueryValueExW(
        key,
        wide("CantoWidget").as_ptr(),
        null(),
        null_mut(),
        null_mut(),
        null_mut(),
    ) == 0;
    RegCloseKey(key);
    result
}
unsafe fn toggle_startup() {
    let mut key = null_mut();
    if RegCreateKeyExW(
        HKEY_CURRENT_USER,
        wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run").as_ptr(),
        0,
        null(),
        0,
        KEY_SET_VALUE | KEY_QUERY_VALUE,
        null(),
        &mut key,
        null_mut(),
    ) != 0
    {
        return;
    }
    if startup_enabled() {
        RegDeleteValueW(key, wide("CantoWidget").as_ptr());
    } else if let Ok(exe) = std::env::current_exe() {
        let text = wide(&format!("\"{}\"", exe.display()));
        RegSetValueExW(
            key,
            wide("CantoWidget").as_ptr(),
            0,
            REG_SZ,
            text.as_ptr().cast(),
            (text.len() * 2) as u32,
        );
    }
    RegCloseKey(key);
}
unsafe extern "system" fn wndproc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let pointer = APP.with(|p| p.get());
    if pointer.is_null() {
        return DefWindowProcW(hwnd, message, w, l);
    }
    let app = &*pointer;
    if message == app.taskbar_message {
        notify(app.controller.get(), NIM_ADD);
        app.layout();
        return 0;
    }
    match message {
        WM_PAINT if hwnd == app.widget.get() => {
            let mut ps: PAINTSTRUCT = zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            let mut r: RECT = zeroed();
            GetClientRect(hwnd, &mut r);
            let buffer = CreateCompatibleDC(dc);
            let width = r.right.max(1);
            let height = r.bottom.max(1);
            let mut info: BITMAPINFO = zeroed();
            info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = width;
            info.bmiHeader.biHeight = -height;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;
            let mut bits = null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            if bitmap.is_null() || buffer.is_null() {
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
                if !buffer.is_null() {
                    DeleteDC(buffer);
                }
                EndPaint(hwnd, &ps);
                return 0;
            }
            let old = SelectObject(buffer, bitmap);
            let s = app.settings.borrow().clone();
            let d = app.data.lock().unwrap().clone();
            let dpi = GetDpiForWindow(hwnd).max(96) as i32;
            let mut cache = app.cache.borrow_mut();
            render::paint(
                buffer,
                r.right,
                r.bottom,
                dpi,
                &s,
                &d.music,
                &d.battery,
                app.background.get(),
                app.hover.get(),
                &mut cache,
            );
            GdiFlush();
            let cover = if s.player_visible(d.music.available) && cache.cover.is_some() {
                let side = (if s.floating { 38 } else { 28 }) * dpi / 96;
                let left = 10 * dpi / 96;
                let top = (height - side) / 2;
                Some([left, top, left + side, top + side])
            } else {
                None
            };
            drop(cache);
            alpha::prepare(
                std::slice::from_raw_parts_mut(bits.cast(), width as usize * height as usize * 4),
                width,
                s.background.unwrap_or(app.background.get()),
                if s.is_blurred() && app.blurred.get() {
                    160
                } else if s.is_transparent() {
                    1
                } else {
                    255
                },
                cover,
            );
            let size = SIZE {
                cx: width,
                cy: height,
            };
            let source = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(
                hwnd,
                null_mut(),
                null(),
                &size,
                buffer,
                &source,
                0,
                &blend,
                ULW_ALPHA,
            );
            SelectObject(buffer, old);
            DeleteObject(bitmap);
            DeleteDC(buffer);
            EndPaint(hwnd, &ps);
            0
        }
        WM_ERASEBKGND => 1,
        WM_TIMER | UPDATE | WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            app.layout();
            if message == UPDATE {
                InvalidateRect(app.widget.get(), null(), 0);
            }
            0
        }
        WM_LBUTTONUP => {
            let mut r: RECT = zeroed();
            GetClientRect(hwnd, &mut r);
            let dpi = GetDpiForWindow(hwnd).max(96) as i32;
            let x = (l as i16 as i32) * 96 / dpi;
            let player = app
                .settings
                .borrow()
                .player_visible(app.data.lock().unwrap().music.available);
            let action = render::hit(x, r.right * 96 / dpi, player);
            if action == 4 {
                app.menu();
            } else if action > 0 {
                app.action(action);
            }
            0
        }
        WM_LBUTTONDOWN => {
            let floating = app.settings.borrow().floating;
            if floating {
                let mut r: RECT = zeroed();
                GetClientRect(hwnd, &mut r);
                let dpi = GetDpiForWindow(hwnd).max(96) as i32;
                let player = app
                    .settings
                    .borrow()
                    .player_visible(app.data.lock().unwrap().music.available);
                if render::hit(l as i16 as i32 * 96 / dpi, r.right * 96 / dpi, player) == 0 {
                    ReleaseCapture();
                    SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
                }
            }
            0
        }
        WM_ENTERSIZEMOVE => {
            app.dragging.set(true);
            0
        }
        WM_EXITSIZEMOVE => {
            app.dragging.set(false);
            let floating = app.settings.borrow().floating;
            if floating {
                let mut r: RECT = zeroed();
                GetWindowRect(hwnd, &mut r);
                let mut s = app.settings.borrow_mut();
                s.left = Some(r.left);
                s.top = Some(r.top);
                s.save();
            }
            0
        }
        WM_MOUSEMOVE => {
            let mut r: RECT = zeroed();
            GetClientRect(hwnd, &mut r);
            let dpi = GetDpiForWindow(hwnd).max(96) as i32;
            let player = app
                .settings
                .borrow()
                .player_visible(app.data.lock().unwrap().music.available);
            let hit = render::hit(l as i16 as i32 * 96 / dpi, r.right * 96 / dpi, player);
            if app.hover.replace(hit) != hit {
                InvalidateRect(hwnd, null(), 0);
            }
            let mut track = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut track);
            SetCursor(LoadCursorW(
                null_mut(),
                if hit > 0 { IDC_HAND } else { IDC_ARROW },
            ));
            0
        }
        MOUSE_LEAVE => {
            app.hover.set(0);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_COMMAND => {
            app.action((w & 0xffff) as u32);
            0
        }
        WM_CONTEXTMENU => {
            app.menu();
            0
        }
        TRAY => {
            if l as u32 == WM_RBUTTONUP || l as u32 == WM_LBUTTONUP {
                app.menu();
            }
            0
        }
        WM_CLOSE => {
            app.action(EXIT);
            0
        }
        WM_DESTROY => {
            if hwnd == app.controller.get() {
                PostQuitMessage(0);
            } else if hwnd == app.widget.get() {
                app.widget.set(null_mut());
            }
            0
        }
        _ => DefWindowProcW(hwnd, message, w, l),
    }
}
fn workers(
    controller: HWND,
    data: Arc<Mutex<Data>>,
    running: Arc<AtomicBool>,
    media_enabled: Arc<AtomicBool>,
    media_rx: mpsc::Receiver<u32>,
    battery_rx: mpsc::Receiver<()>,
) {
    let target = controller as usize;
    let md = data.clone();
    let mr = running.clone();
    thread::Builder::new()
        .name("canto-media".into())
        .stack_size(512 * 1024)
        .spawn(move || {
            let mut media: Option<media::Media> = None;
            while mr.load(Ordering::Relaxed) {
                if media_enabled.load(Ordering::Relaxed) {
                    let music = media.get_or_insert_with(media::Media::new).refresh();
                    let mut d = md.lock().unwrap();
                    if d.music != music {
                        d.music = music;
                        unsafe {
                            PostMessageW(target as HWND, UPDATE, 0, 0);
                        }
                    }
                } else if media.take().is_some() {
                    let mut d = md.lock().unwrap();
                    d.music = media::Music::default();
                    unsafe {
                        PostMessageW(target as HWND, UPDATE, 0, 0);
                    }
                }
                let timeout = if media_enabled.load(Ordering::Relaxed) {
                    2
                } else {
                    30
                };
                match media_rx.recv_timeout(Duration::from_secs(timeout)) {
                    Ok(id) if id > 0 => {
                        if let Some(media) = &media {
                            media.command(id);
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    _ => {}
                }
            }
        })
        .expect("media thread");
    thread::Builder::new()
        .name("canto-battery".into())
        .stack_size(256 * 1024)
        .spawn(move || {
            let mut last = None;
            while running.load(Ordering::Relaxed) {
                let mut battery = battery::read();
                if battery.percent.is_some() {
                    last = Some((battery.clone(), Instant::now()));
                } else if let Some((old, at)) = &last {
                    if at.elapsed() < Duration::from_secs(180) {
                        battery = old.clone();
                        battery.stale = true;
                        battery.message = "Última leitura · mouse sem resposta".into();
                    }
                }
                let mut d = data.lock().unwrap();
                if d.battery != battery {
                    d.battery = battery;
                    unsafe {
                        PostMessageW(target as HWND, UPDATE, 0, 0);
                    }
                }
                drop(d);
                if battery_rx.recv_timeout(Duration::from_secs(30))
                    == Err(mpsc::RecvTimeoutError::Disconnected)
                {
                    break;
                }
            }
        })
        .expect("battery thread");
}
fn main() {
    std::panic::set_hook(Box::new(|info| {
        use std::io::Write;
        let dir = settings::data_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("rust-errors.log"))
        {
            let _ = writeln!(file, "{info}");
        }
    }));
    let args = std::env::args().collect::<Vec<_>>();
    if let Some(i) = args.iter().position(|a| a == "--sync-once") {
        if let Some(repo) = args.get(i + 1) {
            if let Err(e) = sync::run_once(std::path::Path::new(repo)) {
                let _ = std::fs::write(settings::data_dir().join("sync-error.txt"), e);
                std::process::exit(1);
            }
            return;
        }
    }
    if let Some(i) = args.iter().position(|a| a == "--diagnostics") {
        let mut media = media::Media::new();
        let report = serde_json::json!({"battery":battery::read(),"music":media.refresh(),"implementation":"Rust"});
        let path = args
            .get(i + 1)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| settings::data_dir().join("diagnostics.json"));
        let _ = std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap());
        return;
    }
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mutex = CreateMutexW(null(), 0, wide("Local\\CantoWidget.Desktop").as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(mutex);
            return;
        }
        let settings = Settings::load();
        let running = Arc::new(AtomicBool::new(true));
        let sync_enabled = Arc::new(AtomicBool::new(settings.auto_sync));
        let media_enabled = Arc::new(AtomicBool::new(settings.show_player));
        let status = Arc::new(Mutex::new("Verificando conexão GitHub…".into()));
        let data = Arc::new(Mutex::new(Data::default()));
        let (mt, mr) = mpsc::channel();
        let (bt, br) = mpsc::channel();
        let app = Box::new(App {
            controller: Cell::new(null_mut()),
            widget: Cell::new(null_mut()),
            settings: RefCell::new(settings),
            host: RefCell::new(None),
            data: data.clone(),
            cache: RefCell::new(render::Cache::new()),
            hover: Cell::new(0),
            background: Cell::new(0x181818),
            running: running.clone(),
            sync_enabled: sync_enabled.clone(),
            media_enabled: media_enabled.clone(),
            sync_status: status.clone(),
            media_tx: mt,
            battery_tx: bt,
            taskbar_message: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            quitting: Cell::new(false),
            dragging: Cell::new(false),
            blurred: Cell::new(false),
        });
        APP.with(|p| p.set(&*app));
        for class in ["CantoRustController", "CantoRustWidget"] {
            let class_name = wide(class);
            let wc = WNDCLASSW {
                style: CS_DBLCLKS,
                lpfnWndProc: Some(wndproc),
                hInstance: GetModuleHandleW(null()),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                lpszClassName: class_name.as_ptr(),
                ..zeroed()
            };
            RegisterClassW(&wc);
        }
        let controller = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            wide("CantoRustController").as_ptr(),
            wide("Canto Rust Controller").as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        );
        app.controller.set(controller);
        app.layout();
        notify(controller, NIM_ADD);
        SetTimer(controller, 1, 2000, None);
        workers(controller, data, running.clone(), media_enabled, mr, br);
        sync::start(sync::repository(), sync_enabled, running, status);
        let mut message: MSG = zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        APP.with(|p| p.set(null()));
        CloseHandle(mutex);
    }
}
