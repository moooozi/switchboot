//! Native NSIS UI with feature-gated installer OS adapters.
//! Payload, registration and service actions remain in the NSIS scripts.
#![cfg(windows)]
#![allow(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "installer")]
mod installer;
mod painting;
#[cfg(feature = "native-svg")]
pub mod svg;
use painting::{BufferedDc, rounded};

use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    mem::size_of,
    path::Path,
    ptr::*,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::DwmSetWindowAttribute, Gdi::*},
    System::{Com::*, LibraryLoader::GetModuleHandleW, Memory::*},
    UI::{
        Accessibility::*, Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*,
        WindowsAndMessaging::*,
    },
};

const BACKGROUND: u32 = 0x171717;
const SURFACE: u32 = 0x252525;
const BORDER: u32 = 0x404040;
const TEXT: u32 = 0xf5f5f5;
const ACCENT: u32 = 0xe8b900; // COLORREF, not RGB.
const BROWSE: usize = 100;
const PATH_EDIT: usize = 101;
const CLASS: &[u16] = &[
    83, 119, 105, 116, 99, 104, 98, 111, 111, 116, 78, 83, 73, 83, 80, 114, 101, 118, 105, 101,
    119, 0,
];

// The NSIS ABI owns stack nodes, allocated with GlobalAlloc (not Rust's heap).
#[repr(C)]
pub struct Stack {
    next: *mut Stack,
    text: [u16; 1],
}

unsafe fn pop(stack: *mut *mut Stack, capacity: usize) -> String {
    if stack.is_null() || (*stack).is_null() {
        return String::new();
    }
    let node = *stack;
    let text = std::slice::from_raw_parts(addr_of!((*node).text).cast::<u16>(), capacity);
    let end = text.iter().position(|c| *c == 0).unwrap_or(capacity);
    let value = String::from_utf16_lossy(&text[..end]);
    *stack = (*node).next;
    GlobalFree(node.cast());
    value
}

unsafe fn push(stack: *mut *mut Stack, capacity: usize, value: &str) {
    if stack.is_null() || capacity == 0 {
        return;
    }
    let node = GlobalAlloc(GPTR, size_of::<*mut Stack>() + capacity * 2).cast::<Stack>();
    if node.is_null() {
        return;
    }
    (*node).next = *stack;
    let output = std::slice::from_raw_parts_mut(addr_of_mut!((*node).text).cast::<u16>(), capacity);
    for (slot, code) in output[..capacity - 1].iter_mut().zip(value.encode_utf16()) {
        *slot = code;
    }
    *stack = node;
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

struct Ui {
    dpi: Cell<i32>,
    edit: Cell<HWND>,
    hovered: Cell<HWND>,
    normal: Cell<HFONT>,
    bold: Cell<HFONT>,
    heading: Cell<HFONT>,
    background: HBRUSH,
    surface: HBRUSH,
    icon: Cell<HICON>,
    small_icon: Cell<HICON>,
    large_icon: Cell<HICON>,
    icon_path: String,
    #[cfg(feature = "native-svg")]
    svg: RefCell<Option<svg::SvgLogo>>,
    svg_ready: Cell<bool>,
    complete: bool,
    success: bool,
    accepted: Cell<bool>,
    done: Cell<bool>,
    path: RefCell<String>,
    high_contrast: bool,
    light: bool,
    maintenance: bool,
    production: bool,
    action: Cell<&'static str>,
}

impl Ui {
    fn px(&self, value: i32) -> i32 {
        scale(value, self.dpi.get())
    }
    fn bg(&self) -> u32 {
        if self.high_contrast {
            unsafe { GetSysColor(COLOR_WINDOW) }
        } else {
            if self.light { 0xf5f5f5 } else { BACKGROUND }
        }
    }
    fn surface_color(&self) -> u32 {
        if self.high_contrast {
            self.bg()
        } else if self.light {
            0xffffff
        } else {
            SURFACE
        }
    }
    fn border(&self) -> u32 {
        if self.high_contrast {
            self.fg()
        } else if self.light {
            0xd4d4d4
        } else {
            BORDER
        }
    }
    fn fg(&self) -> u32 {
        if self.high_contrast {
            unsafe { GetSysColor(COLOR_WINDOWTEXT) }
        } else {
            if self.light { 0x171717 } else { TEXT }
        }
    }
}

fn light_mode() -> bool {
    use windows_sys::Win32::System::Registry::*;
    // The override is limited to this process, for safe visual fixture testing.
    match std::env::var("SWITCHBOOT_NSIS_THEME").as_deref() {
        Ok("light") => return true,
        Ok("dark") => return false,
        _ => {}
    }
    unsafe {
        let mut value = 1u32;
        let mut bytes = 4;
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize").as_ptr(),
            wide("AppsUseLightTheme").as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            (&mut value as *mut u32).cast(),
            &mut bytes,
        );
        value != 0
    }
}

fn scale(value: i32, dpi: i32) -> i32 {
    (value * dpi + 48) / 96
}

const WINDOW_STYLE: u32 =
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
const CONTROL_LAYOUT: &[(i32, [i32; 4])] = &[
    (PATH_EDIT as i32, [72, 218, 538, 23]),
    (102, [54, 168, 300, 22]),
    (BROWSE as i32, [644, 208, 98, 42]),
    (103, [54, 310, 680, 22]),
    (IDOK, [572, 340, 98, 42]),
    (IDCANCEL, [685, 340, 98, 42]),
    (104, [459, 340, 98, 42]),
    (105, [572, 340, 98, 42]),
];

unsafe fn window_extent(dpi: i32) -> RECT {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: scale(796, dpi),
        bottom: scale(394, dpi),
    };
    AdjustWindowRectExForDpi(&mut rect, WINDOW_STYLE, 0, WS_EX_CONTROLPARENT, dpi as u32);
    rect
}

unsafe fn load_icon(ui: &Ui, pixels: i32) -> HICON {
    if ui.icon_path == "@103" {
        // NSIS already embeds its application icon as RT_GROUP_ICON 103.
        LoadImageW(
            GetModuleHandleW(null()),
            103usize as *const u16,
            IMAGE_ICON,
            pixels,
            pixels,
            0,
        ) as HICON
    } else {
        LoadImageW(
            null_mut(),
            wide(&ui.icon_path).as_ptr(),
            IMAGE_ICON,
            pixels,
            pixels,
            LR_LOADFROMFILE,
        ) as HICON
    }
}

unsafe fn update_logo(window: HWND, ui: &Ui) {
    ui.svg_ready.set(false);
    #[cfg(feature = "native-svg")]
    if std::env::var_os("SWITCHBOOT_NSIS_FORCE_ICO").is_none() {
        let mut renderer = ui.svg.borrow_mut();
        let result = (|| -> windows::core::Result<()> {
            if renderer.is_none() {
                *renderer = Some(svg::SvgLogo::new()?);
            }
            let logo = renderer.as_mut().expect("renderer initialized");
            if logo.render(ui.px(60) as u32).is_err() {
                // Rebuild device-dependent resources once; otherwise use ICO.
                *logo = svg::SvgLogo::new()?;
                logo.render(ui.px(60) as u32)?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => ui.svg_ready.set(true),
            Err(error) => {
                *renderer = None;
                windows_sys::Win32::System::Diagnostics::Debug::OutputDebugStringW(
                    wide(&format!("Switchboot preview SVG fallback: {error}")).as_ptr(),
                );
            }
        }
    }
    let renderer = if ui.svg_ready.get() {
        1
    } else if cfg!(feature = "native-svg") {
        3
    } else {
        2
    };
    SetPropW(
        window,
        wide("Switchboot.NsisLogoRenderer").as_ptr(),
        renderer as HANDLE,
    );
}

unsafe fn paint_logo(dc: HDC, ui: &Ui) {
    #[cfg(feature = "native-svg")]
    if ui.svg_ready.get()
        && ui
            .svg
            .borrow()
            .as_ref()
            .is_some_and(|logo| unsafe { logo.paint(dc, ui.px(702), ui.px(34)) })
    {
        return;
    }
    DrawIconEx(
        dc,
        ui.px(702),
        ui.px(34),
        ui.icon.get(),
        ui.px(60),
        ui.px(60),
        0,
        null_mut(),
        DI_NORMAL,
    );
}

unsafe fn relayout(window: HWND, ui: &Ui) {
    for &(id, [x, y, width, height]) in CONTROL_LAYOUT {
        let control = GetDlgItem(window, id);
        if !control.is_null() {
            let x = if ui.maintenance && id == IDOK { 685 } else { x };
            SetWindowPos(
                control,
                null_mut(),
                ui.px(x),
                ui.px(y),
                ui.px(width),
                ui.px(height),
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW,
            );
            let font = if id == 102 {
                ui.bold.get()
            } else {
                ui.normal.get()
            };
            SendMessageW(control, WM_SETFONT, font as usize, 0);
        }
    }
}

unsafe fn update_dpi(window: HWND, ui: &Ui, dpi: i32) {
    let normal = font(scale(15, dpi), FW_NORMAL);
    let bold = font(scale(15, dpi), FW_BOLD);
    let heading = font(scale(36, dpi), FW_BOLD);
    let previous = [
        ui.normal.replace(normal),
        ui.bold.replace(bold),
        ui.heading.replace(heading),
    ];
    ui.dpi.set(dpi);
    // Reload from original frames, never rescale an icon from the previous monitor.
    let logo = load_icon(ui, ui.px(60));
    let small = load_icon(ui, GetSystemMetricsForDpi(SM_CXSMICON, dpi as u32));
    let large = load_icon(ui, GetSystemMetricsForDpi(SM_CXICON, dpi as u32));
    let old_icons = [
        ui.icon.replace(logo),
        ui.small_icon.replace(small),
        ui.large_icon.replace(large),
    ];
    SendMessageW(window, WM_SETICON, ICON_SMALL as usize, small as isize);
    SendMessageW(window, WM_SETICON, ICON_BIG as usize, large as isize);
    relayout(window, ui);
    update_logo(window, ui);
    for handle in previous {
        if !handle.is_null() {
            DeleteObject(handle);
        }
    }
    for handle in old_icons {
        if !handle.is_null() {
            DestroyIcon(handle);
        }
    }
}

unsafe fn style_button(window: HWND, ui: &Ui) {
    if !ui.high_contrast {
        SetWindowSubclass(window, Some(button_proc), 1, ui as *const Ui as usize);
    }
}

unsafe fn font(height: i32, weight: u32) -> HFONT {
    CreateFontW(
        -height,
        0,
        0,
        0,
        weight as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        DEFAULT_PITCH as u32,
        wide("Segoe UI").as_ptr(),
    )
}

unsafe fn text(dc: HDC, ui: &Ui, value: &str, bounds: [i32; 4], font: HFONT, flags: u32) {
    let [x, y, w, h] = bounds;
    let previous = SelectObject(dc, font);
    SetBkMode(dc, TRANSPARENT as i32);
    SetTextColor(dc, ui.fg());
    let mut rect = RECT {
        left: ui.px(x),
        top: ui.px(y),
        right: ui.px(x + w),
        bottom: ui.px(y + h),
    };
    let content = wide(value);
    DrawTextW(dc, content.as_ptr(), -1, &mut rect, flags | DT_NOPREFIX);
    SelectObject(dc, previous);
}

unsafe fn child(
    window: HWND,
    ui: &Ui,
    class: &str,
    caption: &str,
    id: usize,
    bounds: [i32; 4],
    style: u32,
) -> HWND {
    let handle = CreateWindowExW(
        0,
        wide(class).as_ptr(),
        wide(caption).as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        ui.px(bounds[0]),
        ui.px(bounds[1]),
        ui.px(bounds[2]),
        ui.px(bounds[3]),
        window,
        id as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
    SendMessageW(handle, WM_SETFONT, ui.normal.get() as usize, 1);
    handle
}

unsafe extern "system" fn button_proc(
    window: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    if msg == WM_ERASEBKGND {
        return 1;
    }
    let ui = &*(data as *const Ui);
    let hovered = ui.hovered.get() == window;
    if msg == WM_MOUSEMOVE && !hovered {
        let previous = ui.hovered.replace(window);
        if !previous.is_null() {
            InvalidateRect(previous, null(), 0);
        }
        let mut tracking = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: window,
            dwHoverTime: 0,
        };
        TrackMouseEvent(&mut tracking);
        InvalidateRect(window, null(), 0);
    } else if msg == WM_MOUSELEAVE && hovered {
        ui.hovered.set(null_mut());
        InvalidateRect(window, null(), 0);
    } else if msg == WM_SETFOCUS || msg == WM_KILLFOCUS {
        InvalidateRect(window, null(), 0);
    }
    if msg == WM_MOUSEMOVE && GetCapture() != window {
        // Owner drawing owns hover. The native button otherwise invalidates on
        // every move; still forward captured moves for press/drag cancellation.
        return 0;
    }
    if msg == WM_NCDESTROY {
        RemoveWindowSubclass(window, Some(button_proc), 1);
    }
    DefSubclassProc(window, msg, w, l)
}

unsafe extern "system" fn browse_callback(
    window: HWND,
    msg: u32,
    _: LPARAM,
    initial: LPARAM,
) -> i32 {
    if msg == BFFM_INITIALIZED {
        SendMessageW(window, BFFM_SETSELECTIONW, 1, initial);
    }
    0
}

unsafe fn browse(window: HWND, ui: &Ui) {
    let mut path = [0u16; 1024];
    GetWindowTextW(ui.edit.get(), path.as_mut_ptr(), path.len() as i32);
    let title = wide("Choose a preview installation folder (nothing will be installed here)");
    let info = BROWSEINFOW {
        hwndOwner: window,
        pidlRoot: null_mut(),
        pszDisplayName: null_mut(),
        lpszTitle: title.as_ptr(),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX,
        lpfn: Some(browse_callback),
        lParam: path.as_ptr() as isize,
        iImage: 0,
    };
    let selected = SHBrowseForFolderW(&info);
    if !selected.is_null() {
        let mut output = [0u16; 260];
        if SHGetPathFromIDListW(selected, output.as_mut_ptr()) != 0 {
            let len = output.iter().position(|c| *c == 0).unwrap_or(output.len());
            let selected_path = String::from_utf16_lossy(&output[..len]);
            let selected_path = Path::new(&selected_path);
            let destination = if selected_path
                .file_name()
                .is_some_and(|v| v.eq_ignore_ascii_case("Switchboot"))
            {
                selected_path.to_owned()
            } else {
                selected_path.join("Switchboot")
            };
            SetWindowTextW(ui.edit.get(), wide(&destination.to_string_lossy()).as_ptr());
        }
        CoTaskMemFree(selected.cast());
    }
}

unsafe extern "system" fn window_proc(window: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let create = &*(l as *const CREATESTRUCTW);
        SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as _);
    }
    let state = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut Ui;
    if state.is_null() {
        return DefWindowProcW(window, msg, w, l);
    }
    // Avoid holding a Rust mutable reference across reentrant Win32 calls.
    let ui = &*state;
    match msg {
        WM_CREATE => {
            update_dpi(window, ui, GetDpiForWindow(window) as i32);
            let button_style = WS_TABSTOP
                | if ui.high_contrast {
                    BS_PUSHBUTTON as u32
                } else {
                    BS_OWNERDRAW as u32
                };
            if !ui.complete && !ui.maintenance {
                let edit = child(
                    window,
                    ui,
                    "EDIT",
                    &ui.path.borrow(),
                    PATH_EDIT,
                    [72, 218, 538, 23],
                    WS_TABSTOP | ES_AUTOHSCROLL as u32,
                );
                SendMessageW(edit, EM_SETLIMITTEXT, 1023, 0);
                ui.edit.set(edit);
                child(
                    window,
                    ui,
                    "STATIC",
                    "Installation folder",
                    102,
                    [54, 168, 300, 22],
                    0,
                );
                let browse = child(
                    window,
                    ui,
                    "BUTTON",
                    "&Browse...",
                    BROWSE,
                    [644, 208, 98, 42],
                    button_style,
                );
                style_button(browse, ui);
                child(window, ui, "STATIC", "", 103, [54, 310, 680, 22], 0);
            }
            let install = child(
                window,
                ui,
                "BUTTON",
                if ui.complete {
                    "Close"
                } else if ui.maintenance {
                    "&Launch"
                } else {
                    "&Install"
                },
                IDOK as usize,
                [572, 340, 98, 42],
                button_style,
            );
            style_button(install, ui);
            if !ui.complete {
                let cancel = child(
                    window,
                    ui,
                    "BUTTON",
                    if ui.maintenance {
                        "&Reinstall"
                    } else {
                        "Cancel"
                    },
                    if ui.maintenance {
                        105
                    } else {
                        IDCANCEL as usize
                    },
                    [685, 340, 98, 42],
                    button_style,
                );
                style_button(cancel, ui);
            }
            if ui.maintenance {
                let uninstall = child(
                    window,
                    ui,
                    "BUTTON",
                    "&Uninstall",
                    104,
                    [459, 340, 98, 42],
                    button_style,
                );
                style_button(uninstall, ui);
            }
            relayout(window, ui);
            0
        }
        WM_GETDPISCALEDSIZE => {
            let rect = window_extent(w as i32);
            let size = &mut *(l as *mut SIZE);
            size.cx = rect.right - rect.left;
            size.cy = rect.bottom - rect.top;
            1
        }
        WM_DPICHANGED => {
            let suggested = *(l as *const RECT);
            update_dpi(window, ui, (w & 0xffff) as i32);
            SetWindowPos(
                window,
                null_mut(),
                suggested.left,
                suggested.top,
                suggested.right - suggested.left,
                suggested.bottom - suggested.top,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW,
            );
            RedrawWindow(
                window,
                null(),
                null_mut(),
                RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW | RDW_NOERASE,
            );
            0
        }
        WM_PAINT => {
            let mut paint = std::mem::zeroed();
            let target = BeginPaint(window, &mut paint);
            let mut rect = std::mem::zeroed();
            GetClientRect(window, &mut rect);
            let buffer = BufferedDc::new(target, rect);
            let dc = buffer.dc();
            FillRect(dc, &rect, ui.background);
            let heading = if ui.maintenance {
                "Switchboot is already installed"
            } else if !ui.complete {
                "Install Switchboot"
            } else if ui.production && ui.success {
                "Switchboot installed"
            } else if ui.production {
                "Installation failed"
            } else if ui.success {
                "Preview complete"
            } else {
                "Preview failed"
            };
            text(
                dc,
                ui,
                heading,
                [32, 32, 650, 50],
                ui.heading.get(),
                DT_LEFT,
            );
            text(
                dc,
                ui,
                if ui.maintenance {
                    "Choose what you would like to do with your installation."
                } else {
                    "Switchboot lets you manage your boot order and choose which operating system starts next."
                },
                [32, 92, 660, 48],
                ui.normal.get(),
                DT_WORDBREAK,
            );
            paint_logo(dc, ui);
            if !ui.complete && !ui.maintenance {
                rounded(
                    dc,
                    RECT {
                        left: ui.px(54),
                        top: ui.px(206),
                        right: ui.px(629),
                        bottom: ui.px(252),
                    },
                    ui.surface_color(),
                    ui.border(),
                    ui.px(32),
                    ui.bg(),
                );
                text(
                    dc,
                    ui,
                    if ui.production {
                        ""
                    } else {
                        "UI proof of concept. Destination is preview-only; no application or service is installed."
                    },
                    [54, 272, 690, 42],
                    ui.normal.get(),
                    DT_WORDBREAK,
                );
                text(
                    dc,
                    ui,
                    if ui.production {
                        "Installs Switchboot for all users."
                    } else {
                        "Temporary text fixture only. No elevation required."
                    },
                    [54, 351, 500, 24],
                    ui.normal.get(),
                    DT_LEFT,
                );
            } else if ui.complete {
                let message = if ui.production && ui.success {
                    "Switchboot and its Windows service have been installed."
                } else if ui.production {
                    "Installation did not complete. See the installer error for details."
                } else if ui.success {
                    "The stock NSIS engine extracted the harmless text fixture into its temporary directory.\n\nNo files were written to the displayed destination. No service, firmware, shortcuts or installation registrations were changed.\n\nThe fixture and UI plug-in are cleaned up when Setup exits."
                } else {
                    "NSIS could not extract the temporary fixture. No application installation was attempted."
                };
                text(
                    dc,
                    ui,
                    message,
                    [54, 170, 680, 145],
                    ui.normal.get(),
                    DT_WORDBREAK,
                );
            }
            drop(buffer);
            EndPaint(window, &paint);
            0
        }
        WM_DRAWITEM => {
            let draw = &*(l as *const DRAWITEMSTRUCT);
            let primary = draw.CtlID == IDOK as u32;
            let buffer = BufferedDc::new(draw.hDC, draw.rcItem);
            let dc = buffer.dc();
            let hovered = ui.hovered.get() == draw.hwndItem;
            let pressed = draw.itemState & ODS_SELECTED != 0;
            let fill = if primary {
                if pressed {
                    0xc59d00
                } else if hovered {
                    0xf4d865
                } else {
                    ACCENT
                }
            } else if pressed {
                if ui.light { 0xc5c5c5 } else { 0x303030 }
            } else if hovered {
                if ui.light { 0xd4d4d4 } else { 0x484848 }
            } else {
                if ui.light { 0xe5e5e5 } else { 0x383838 }
            };
            FillRect(dc, &draw.rcItem, ui.background);
            let focus = draw.itemState & ODS_FOCUS != 0 && draw.itemState & ODS_NOFOCUSRECT == 0;
            rounded(
                dc,
                draw.rcItem,
                fill,
                if focus { ui.fg() } else { fill },
                ui.px(42),
                ui.bg(),
            );
            let mut caption = [0u16; 64];
            GetWindowTextW(draw.hwndItem, caption.as_mut_ptr(), 64);
            let old = SelectObject(dc, ui.normal.get());
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextColor(dc, if primary { 0 } else { ui.fg() });
            let mut rect = draw.rcItem;
            DrawTextW(
                dc,
                caption.as_ptr(),
                -1,
                &mut rect,
                DT_CENTER
                    | DT_VCENTER
                    | DT_SINGLELINE
                    | if draw.itemState & ODS_NOACCEL != 0 {
                        DT_HIDEPREFIX
                    } else {
                        0
                    },
            );
            SelectObject(dc, old);
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
            let dc = w as HDC;
            SetTextColor(dc, ui.fg());
            let edit = l as HWND == ui.edit.get();
            SetBkColor(
                dc,
                if edit && !ui.high_contrast {
                    ui.surface_color()
                } else {
                    ui.bg()
                },
            );
            if edit {
                ui.surface as isize
            } else {
                ui.background as isize
            }
        }
        WM_COMMAND => {
            match w & 0xffff {
                BROWSE => browse(window, ui),
                id if ui.maintenance && (id == IDOK as usize || id == 105 || id == 104) => {
                    if id != IDOK as usize {
                        let message = if id == 104 {
                            "Uninstall Switchboot and its Windows service?"
                        } else {
                            "Remove and reinstall Switchboot and its Windows service?"
                        };
                        if MessageBoxW(
                            window,
                            wide(message).as_ptr(),
                            wide("Switchboot Setup").as_ptr(),
                            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
                        ) != IDYES
                        {
                            return 0;
                        }
                    }
                    ui.action.set(if id == IDOK as usize {
                        "launch"
                    } else if id == 105 {
                        "reinstall"
                    } else {
                        "uninstall"
                    });
                    ui.accepted.set(true);
                    DestroyWindow(window);
                }
                id if id == IDOK as usize => {
                    if !ui.complete {
                        let mut value = [0u16; 1024];
                        GetWindowTextW(ui.edit.get(), value.as_mut_ptr(), value.len() as i32);
                        let end = value.iter().position(|c| *c == 0).unwrap_or(value.len());
                        let path = String::from_utf16_lossy(&value[..end]).trim().to_owned();

                        if !Path::new(&path).is_absolute() {
                            SetWindowTextW(
                                GetDlgItem(window, 103),
                                wide("Enter an absolute installation folder path.").as_ptr(),
                            );
                            SetFocus(ui.edit.get());
                            return 0;
                        }
                        *ui.path.borrow_mut() = path;
                        ui.accepted.set(true);
                    }
                    DestroyWindow(window);
                }
                id if id == IDCANCEL as usize => {
                    DestroyWindow(window);
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(window);
            0
        }
        WM_DESTROY => {
            RemovePropW(window, wide("Switchboot.NsisLogoRenderer").as_ptr());
            ui.done.set(true);
            0
        }
        WM_ERASEBKGND => 1,
        _ => DefWindowProcW(window, msg, w, l),
    }
}

unsafe fn show(
    path: String,
    icon: &str,
    complete: bool,
    success: bool,
    maintenance: bool,
    production: bool,
) -> Result<Option<String>, u32> {
    let previous_dpi = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    let com = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
    let instance = GetModuleHandleW(null());
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: CLASS.as_ptr(),
        hCursor: LoadCursorW(null_mut(), IDC_ARROW),
        ..std::mem::zeroed()
    };
    RegisterClassW(&class);
    let mut contrast = HIGHCONTRASTW {
        cbSize: size_of::<HIGHCONTRASTW>() as u32,
        ..std::mem::zeroed()
    };
    SystemParametersInfoW(
        SPI_GETHIGHCONTRAST,
        contrast.cbSize,
        (&mut contrast as *mut HIGHCONTRASTW).cast(),
        0,
    );
    let high_contrast = contrast.dwFlags & HCF_HIGHCONTRASTON != 0;
    let light = light_mode();
    let dpi = GetDpiForSystem() as i32;
    let mut ui = Box::new(Ui {
        dpi: Cell::new(dpi),
        edit: Cell::new(null_mut()),
        hovered: Cell::new(null_mut()),
        normal: Cell::new(null_mut()),
        bold: Cell::new(null_mut()),
        heading: Cell::new(null_mut()),
        background: CreateSolidBrush(if high_contrast {
            GetSysColor(COLOR_WINDOW)
        } else {
            if light { 0xf5f5f5 } else { BACKGROUND }
        }),
        surface: CreateSolidBrush(if high_contrast {
            GetSysColor(COLOR_WINDOW)
        } else {
            if light { 0xffffff } else { SURFACE }
        }),
        icon: Cell::new(null_mut()),
        small_icon: Cell::new(null_mut()),
        large_icon: Cell::new(null_mut()),
        icon_path: icon.to_owned(),
        #[cfg(feature = "native-svg")]
        svg: RefCell::new(None),
        svg_ready: Cell::new(false),
        complete,
        success,
        accepted: Cell::new(false),
        done: Cell::new(false),
        path: RefCell::new(path),
        high_contrast,
        light,
        maintenance,
        production,
        action: Cell::new("cancel"),
    });
    let style = WINDOW_STYLE;
    let rect = window_extent(dpi);
    let mut cursor = std::mem::zeroed();
    GetCursorPos(&mut cursor);
    let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
    let mut monitor_info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..std::mem::zeroed()
    };
    GetMonitorInfoW(monitor, &mut monitor_info);
    let work = monitor_info.rcWork;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    let window = CreateWindowExW(
        WS_EX_CONTROLPARENT,
        CLASS.as_ptr(),
        wide(if production {
            "Setup - Switchboot"
        } else {
            "Setup - Switchboot (NSIS preview)"
        })
        .as_ptr(),
        style,
        work.left + (work.right - work.left - width) / 2,
        work.top + (work.bottom - work.top - height) / 2,
        width,
        height,
        null_mut(),
        null_mut(),
        instance,
        (&mut *ui as *mut Ui).cast(),
    );
    let creation_error = GetLastError();
    if !window.is_null() {
        if !high_contrast {
            let dark = i32::from(!light);
            DwmSetWindowAttribute(window, 20, (&dark as *const i32).cast(), 4);
            DwmSetWindowAttribute(window, 35, (&ui.bg() as *const u32).cast(), 4);
        }
        let rect = window_extent(ui.dpi.get());
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        SetWindowPos(
            window,
            null_mut(),
            work.left + (work.right - work.left - width) / 2,
            work.top + (work.bottom - work.top - height) / 2,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SendMessageW(
            window,
            WM_CHANGEUISTATE,
            (((UISF_HIDEACCEL | UISF_HIDEFOCUS) as usize) << 16) | UIS_SET as usize,
            0,
        );
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        SetFocus(GetDlgItem(window, IDOK));
        let mut message = std::mem::zeroed();
        while !ui.done.get() {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(message.wParam as i32);
                }
                DestroyWindow(window);
                break;
            }
            if IsDialogMessageW(window, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    UnregisterClassW(CLASS.as_ptr(), instance);
    DeleteObject(ui.normal.get());
    DeleteObject(ui.bold.get());
    DeleteObject(ui.heading.get());
    DeleteObject(ui.background);
    DeleteObject(ui.surface);
    for icon in [ui.icon.get(), ui.small_icon.get(), ui.large_icon.get()] {
        if !icon.is_null() {
            DestroyIcon(icon);
        }
    }
    #[cfg(feature = "native-svg")]
    ui.svg.borrow_mut().take();
    if com >= 0 {
        CoUninitialize();
    }
    if !previous_dpi.is_null() {
        SetThreadDpiAwarenessContext(previous_dpi);
    }
    if window.is_null() {
        Err(creation_error)
    } else if ui.accepted.get() {
        Ok(Some(if maintenance {
            ui.action.get().to_owned()
        } else {
            ui.path.borrow().clone()
        }))
    } else {
        Ok(None)
    }
}

/// NSIS plug-in entry points use cdecl, including on x86.
///
/// # Safety
/// Called by NSIS with its Unicode stack ABI and valid string capacity.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Choose(
    _: HWND,
    size: i32,
    _: *mut u16,
    stack: *mut *mut Stack,
    _: *mut c_void,
) {
    if size <= 0 {
        return;
    }
    let path = pop(stack, size as usize);
    let icon = pop(stack, size as usize);
    let (status, selected) = match show(path, &icon, false, true, false, false) {
        Ok(Some(path)) => ("install".to_owned(), path),
        Ok(None) => ("cancel".to_owned(), String::new()),
        Err(code) => (
            format!("Window creation failed: Win32 error {code}"),
            String::new(),
        ),
    };
    push(stack, size as usize, &selected);
    push(stack, size as usize, &status);
}

/// # Safety
/// Called by NSIS with its Unicode stack ABI and valid string capacity.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Complete(
    _: HWND,
    size: i32,
    _: *mut u16,
    stack: *mut *mut Stack,
    _: *mut c_void,
) {
    if size <= 0 {
        return;
    }
    let status = pop(stack, size as usize);
    let icon = pop(stack, size as usize);
    let _ = show(
        String::new(),
        &icon,
        true,
        status == "success",
        false,
        false,
    );
}

/// Production selection adapter. Returns action followed by destination on the NSIS stack.
/// Mode is `install` or `maintenance`; no system changes are made here.
///
/// # Safety
/// Called by NSIS with its Unicode stack ABI and valid string capacity.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Select(
    _: HWND,
    size: i32,
    _: *mut u16,
    stack: *mut *mut Stack,
    _: *mut c_void,
) {
    if size <= 0 {
        return;
    }
    let mode = pop(stack, size as usize);
    let path = pop(stack, size as usize);
    let maintenance = mode == "maintenance" || mode == "maintenance-preview";
    let (status, selected) = match show(
        path.clone(),
        "@103",
        false,
        true,
        maintenance,
        mode != "maintenance-preview",
    ) {
        Ok(Some(result)) if maintenance => (result, path),
        Ok(Some(result)) => ("install".into(), result),
        Ok(None) => ("cancel".into(), String::new()),
        Err(code) => (format!("UI failed: {code}"), String::new()),
    };
    push(stack, size as usize, &selected);
    push(stack, size as usize, &status);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_uses_base_coordinates_at_every_dpi() {
        for dpi in [96, 120, 144, 168, 192, 216, 240, 288, 384] {
            for &(_, [x, y, width, height]) in CONTROL_LAYOUT {
                assert!(scale(x, dpi) + scale(width, dpi) <= scale(796, dpi));
                assert!(scale(y, dpi) + scale(height, dpi) <= scale(394, dpi));
            }
        }
        assert_eq!(
            [96, 120, 144, 168, 192, 216].map(|dpi| scale(60, dpi)),
            [60, 75, 90, 105, 120, 135]
        );
        // A return to 100% is computed from DIPs, not a rounded previous size.
        assert_eq!(scale(796, 96), 796);
    }

    #[test]
    fn nsis_stack_preserves_unicode_and_lifo_order() {
        unsafe {
            let mut stack = null_mut();
            push(&mut stack, 1024, r"C:\预览\Switchboot");
            push(&mut stack, 1024, "install");
            assert_eq!(pop(&mut stack, 1024), "install");
            assert_eq!(pop(&mut stack, 1024), r"C:\预览\Switchboot");
            assert!(stack.is_null());
            assert_eq!(pop(&mut stack, 1024), "");
        }
    }

    #[test]
    fn nsis_stack_respects_capacity_and_null_termination() {
        unsafe {
            let mut stack = null_mut();
            push(&mut stack, 4, "abcdef");
            assert_eq!(pop(&mut stack, 4), "abc");
            push(&mut stack, 1, "abcdef");
            assert_eq!(pop(&mut stack, 1), "");
            push(&mut stack, 0, "ignored");
            assert!(stack.is_null());
        }
    }
}
