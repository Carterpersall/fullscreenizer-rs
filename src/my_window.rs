extern crate alloc;

use alloc::sync::Arc;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, RwLock};

use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, GENERIC_READ, TRUE};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICBitmapDecoder,
    IWICBitmapFrameDecode, IWICFormatConverter, IWICImagingFactory, WICBitmapDitherTypeNone,
    WICBitmapPaletteTypeMedianCut, WICDecodeMetadataCacheOnLoad,
};
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::Storage::Packaging::Appx::{
    FindPackagesByPackageFamily, GetPackagePathByFullName, PACKAGE_FILTER_HEAD,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, ICONINFO};
use windows::core::{PCWSTR, PWSTR};

use winsafe::co::SWP;
use winsafe::guard::ImageListDestroyGuard;
use winsafe::gui::dpi;
use winsafe::msg::lvm::{SetBkColor, SetTextBkColor, SetTextColor};
use winsafe::prelude::{GuiParent, GuiWindow, Handle};
use winsafe::{
    self as w, AdjustWindowRectEx, COLORREF, DwmAttr, EnumWindows, HBRUSH, HICON, HIMAGELIST,
    HwndPlace, POINT, RECT, SIZE, co, gui,
};

#[derive(Clone)]
pub struct MyWindow {
    // Window elements
    wnd: gui::WindowMain,
    label: gui::Label,
    process_list: gui::ListView,
    top_toggle: gui::ListView,
    btn_canvas: gui::Label,
    refresh_btn: gui::Button,
    help_btn: gui::Button,
    fullscreenize_btn: gui::Button,

    // Settings
    is_dark_mode: Arc<AtomicBool>,
    use_icons: Arc<AtomicBool>,
    excluded_apps: Arc<[String]>,

    // Shared resources
    app_font: Rc<RwLock<Option<w::guard::DeleteObjectGuard<w::HFONT>>>>,
    app_dpi: Arc<AtomicU32>,
    imagelist: Arc<Mutex<Option<w::guard::ImageListDestroyGuard>>>,
    window_icons: Arc<Mutex<Vec<w::guard::DestroyIconGuard>>>,
}

impl MyWindow {
    pub fn new() -> Self {
        let wnd = gui::WindowMain::new(gui::WindowMainOpts {
            title: "Fullscreenizer".to_owned(),
            class_icon: gui::Icon::Id(101),
            size: dpi(305, 400),
            style: gui::WindowMainOpts::default().style
                | co::WS::OVERLAPPEDWINDOW
                | co::WS::CLIPCHILDREN
                | co::WS::SIZEBOX, // window can be resized
            ..Default::default()
        });

        let label = gui::Label::new(
            &wnd,
            gui::LabelOpts {
                text: "Toplevel windows:".to_string(),
                position: dpi(10, 9),
                size: dpi(200, 20),
                control_style: co::SS::LEFTNOWORDWRAP,
                window_style: co::WS::CHILD | co::WS::VISIBLE,
                window_ex_style: co::WS_EX::NoValue,
                ctrl_id: 10000,
                resize_behavior: (gui::Horz::Resize, gui::Vert::Repos),
            },
        );

        let process_list = gui::ListView::new(
            &wnd,
            gui::ListViewOpts {
                position: dpi(8, 29),
                size: dpi(289, 307),
                columns: vec![("".to_owned(), 999)],
                control_style: co::LVS::NOSORTHEADER
                    | co::LVS::SHOWSELALWAYS
                    | co::LVS::NOCOLUMNHEADER
                    | co::LVS::NOLABELWRAP
                    | co::LVS::SINGLESEL
                    | co::LVS::REPORT,
                control_ex_style: co::LVS_EX::DOUBLEBUFFER,
                window_style: co::WS::CHILD
                    | co::WS::VISIBLE
                    | co::WS::TABSTOP
                    | co::WS::GROUP
                    | co::WS::VSCROLL
                    | co::WS::CLIPSIBLINGS,
                // Resize horizontally and vertically together with parent window.
                resize_behavior: (gui::Horz::Resize, gui::Vert::Resize),
                ..Default::default()
            },
        );

        let top_toggle = gui::ListView::new(
            &wnd,
            gui::ListViewOpts {
                position: dpi(2, 342),
                size: dpi(300, 20),
                columns: vec![("".to_owned(), 999)],
                control_style: co::LVS::NOSORTHEADER
                    | co::LVS::SHOWSELALWAYS
                    | co::LVS::NOCOLUMNHEADER
                    | co::LVS::NOLABELWRAP
                    | co::LVS::SINGLESEL
                    | co::LVS::REPORT
                    | co::LVS::NOSCROLL
                    | co::LVS::SHAREIMAGELISTS,
                control_ex_style: co::LVS_EX::DOUBLEBUFFER
                    | co::LVS_EX::BORDERSELECT
                    | co::LVS_EX::AUTOSIZECOLUMNS
                    | co::LVS_EX::CHECKBOXES,
                window_style: co::WS::CHILD
                    | co::WS::VISIBLE
                    | co::WS::TABSTOP
                    | co::WS::GROUP
                    | co::WS::CLIPSIBLINGS,
                // Resize horizontally and vertically together with parent window.
                resize_behavior: (gui::Horz::Resize, gui::Vert::Repos),
                ..Default::default()
            },
        );

        // Label that will be the parent of the buttons
        // This will allow for the buttons' undrawn background color to be configured
        let btn_canvas = gui::Label::new(
            &wnd,
            gui::LabelOpts {
                text: "".to_owned(),
                position: dpi(8, 360),
                size: dpi(290, 40),
                window_style: co::WS::CHILD | co::WS::VISIBLE | co::WS::CLIPSIBLINGS,
                window_ex_style: co::WS_EX::CONTROLPARENT,
                ..Default::default()
            },
        );

        let refresh_btn = gui::Button::new(
            &wnd,
            gui::ButtonOpts {
                text: "&Refresh".to_owned(),
                position: dpi(13, 368),
                ..Default::default()
            },
        );

        let help_btn = gui::Button::new(
            &wnd,
            gui::ButtonOpts {
                text: "&Help".to_owned(),
                position: dpi(108, 368),
                ..Default::default()
            },
        );

        let fullscreenize_btn = gui::Button::new(
            &wnd,
            gui::ButtonOpts {
                text: "&Fullscreenize".to_owned(),
                position: dpi(202, 368),
                ..Default::default()
            },
        );

        /* Settings */
        // Whether dark mode is enabled
        let is_dark_mode = Arc::new(AtomicBool::new(false));
        // Whether to use icons in the process list
        let use_icons = Arc::new(AtomicBool::new(true));
        // Apps excluded from the process list
        let excluded_apps = Arc::new(
            [
                "Program Manager",
                "Windows Input Experience",
                "PopupHost",
                "System tray overflow window.",
            ]
            .map(String::from),
        );

        /* Shared Resources */
        // The application's font
        let app_font = Rc::new(RwLock::new(None));
        // The current DPI of the window
        // This is used to scale the window elements based on a 125% (120 DPI) display
        let app_dpi = Arc::new(AtomicU32::new(120));
        // The image list for the window icons
        let imagelist = Arc::new(Mutex::new(None));
        // A vector to store the icons of the windows
        let window_icons = Arc::new(Mutex::new(Vec::new()));

        let new_self = Self {
            wnd,
            label,
            process_list,
            top_toggle,
            btn_canvas,
            refresh_btn,
            help_btn,
            fullscreenize_btn,
            is_dark_mode,
            use_icons,
            excluded_apps,
            app_font,
            app_dpi,
            imagelist,
            window_icons,
        };

        new_self.events();
        new_self
    }

    pub fn run(&self) -> w::AnyResult<i32> {
        self.wnd.run_main(None)
    }

    fn update_font(&self) {
        // Get the current DPI
        let app_dpi = self.app_dpi.load(Ordering::Relaxed);

        // Create a new font based on the current DPI
        let font = match w::HFONT::CreateFont(
            SIZE {
                cx: 0,
                cy: -w::MulDiv(15, app_dpi as i32, 120),
            },
            0,
            0,
            co::FW::MEDIUM,
            false,
            false,
            false,
            co::CHARSET::DEFAULT,
            co::OUT_PRECIS::DEFAULT,
            co::CLIP::DEFAULT_PRECIS,
            co::QUALITY::DEFAULT,
            co::PITCH::DEFAULT,
            "Segoe UI",
        ) {
            Ok(hfont) => hfont,
            Err(e) => {
                eprintln!("Failed to create font - CreateFont failed: {e}");
                return;
            }
        };

        // Store the font in the shared resource
        if let Ok(mut app_font) = self.app_font.write() {
            *app_font = Some(font);
        }

        // Update the font for all controls
        match self.app_font.read() {
            Ok(app_font) => {
                if let Some(font) = app_font.as_ref() {
                    unsafe {
                        self.label.hwnd().SendMessage(w::msg::wm::SetFont {
                            hfont: font.raw_copy(),
                            redraw: true,
                        });
                        self.refresh_btn.hwnd().SendMessage(w::msg::wm::SetFont {
                            hfont: font.raw_copy(),
                            redraw: true,
                        });
                        self.help_btn.hwnd().SendMessage(w::msg::wm::SetFont {
                            hfont: font.raw_copy(),
                            redraw: true,
                        });
                        self.fullscreenize_btn
                            .hwnd()
                            .SendMessage(w::msg::wm::SetFont {
                                hfont: font.raw_copy(),
                                redraw: true,
                            });
                    }
                }
            }
            Err(e) => eprintln!("Failed to lock app_font mutex: {e}"),
        }
    }

    fn enable_dark_mode(&self) {
        // Get a handle to the window
        let wnd = self.wnd.hwnd();

        // Get a handle to the process list
        let process_list = self.process_list.hwnd();

        // Enable dark mode on the window
        wnd.DwmSetWindowAttribute(DwmAttr::UseImmersiveDarkMode(true))
            .map_err(|e| eprintln!("DwmSetWindowAttribute failed: {e}"))
            .ok();

        // Enable dark mode on the elements in the window
        wnd.SetWindowTheme("DarkMode_Explorer", None)
            .map_err(|e| eprintln!("SetWindowTheme on window failed: {e}"))
            .ok();
        process_list
            .SetWindowTheme("DarkMode_Explorer", None)
            .map_err(|e| eprintln!("SetWindowTheme on process list failed: {e}"))
            .ok();
        self.refresh_btn
            .hwnd()
            .SetWindowTheme("DarkMode_Explorer", None)
            .map_err(|e| eprintln!("SetWindowTheme on refresh button failed: {e}"))
            .ok();
        self.help_btn
            .hwnd()
            .SetWindowTheme("DarkMode_Explorer", None)
            .map_err(|e| eprintln!("SetWindowTheme on help button failed: {e}"))
            .ok();
        self.fullscreenize_btn
            .hwnd()
            .SetWindowTheme("DarkMode_Explorer", None)
            .map_err(|e| eprintln!("SetWindowTheme on fullscreenize button failed: {e}"))
            .ok();

        // Set the background color of the listview
        unsafe {
            process_list.SendMessage(SetBkColor {
                color: Option::from(COLORREF::from_rgb(0x3C, 0x3C, 0x3C)), //0xC4, 0xC4, 0xC4)),
            })
        }
        .map_err(|e| eprintln!("SetBkColor failed: {e}"))
        .ok();

        // Set the background color of the elements in the listview
        unsafe {
            process_list.SendMessage(SetTextBkColor {
                color: Option::from(COLORREF::from_rgb(0x3C, 0x3C, 0x3C)), //0xC4, 0xC4, 0xC4)),
            })
        }
        .map_err(|e| eprintln!("WM_CTLCOLORLISTBOX failed: {e}"))
        .ok();

        // Set the text color of the elements in the listview
        unsafe {
            process_list.SendMessage(SetTextColor {
                color: Option::from(COLORREF::from_rgb(0xF0, 0xF0, 0xF0)),
            })
        }
        .map_err(|e| eprintln!("SetTextColor failed: {e}"))
        .ok();

        // Get the handle of the top toggle
        let top_toggle = self.top_toggle.hwnd();

        // Set the background color of the checkbox listview to the same as the window background
        unsafe {
            top_toggle.SendMessage(SetBkColor {
                color: Option::from(COLORREF::from_rgb(0x1E, 0x1E, 0x1E)),
            })
        }
        .map_err(|e| eprintln!("SetBkColor failed: {e}"))
        .ok();

        // Set the background color of the element in the checkbox listview
        unsafe {
            top_toggle.SendMessage(SetTextBkColor {
                color: Option::from(COLORREF::from_rgb(0x1E, 0x1E, 0x1E)),
            })
        }
        .map_err(|e| eprintln!("WM_CTLCOLORLISTBOX failed: {e}"))
        .ok();

        // Set the text color of the elements in the checkbox listview
        unsafe {
            top_toggle.SendMessage(SetTextColor {
                color: Option::from(COLORREF::from_rgb(0xF0, 0xF0, 0xF0)),
            })
        }
        .map_err(|e| eprintln!("SetTextColor failed: {e}"))
        .ok();
    }

    fn set_system_theme(&self) {
        // Check if dark mode is enabled using the registry
        let dark_mode = w::HKEY::CURRENT_USER
            .RegOpenKeyEx(
                Some("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                co::REG_OPTION::default(),
                co::KEY::READ,
            )
            .and_then(|key| key.RegQueryValueEx(Some("AppsUseLightTheme")))
            .map_or_else(
                |e| {
                    eprintln!("Getting the system theme failed: {e}");
                    // Default to light mode
                    false
                },
                |result| {
                    match result {
                        // If the value is 1, light mode is enabled
                        w::RegistryValue::Dword(value) => value != 1,
                        _ => {
                            // Default to light mode
                            false
                        }
                    }
                },
            );

        // Store the dark mode state
        self.is_dark_mode.store(dark_mode, Ordering::Relaxed);

        if dark_mode {
            // Enable dark mode on the window
            self.enable_dark_mode();
        } else {
            // Set the background color of the checkbox listview to the same as the window background
            unsafe {
                self.top_toggle.hwnd().SendMessage(SetBkColor {
                    color: Option::from(COLORREF::from_rgb(0xF0, 0xF0, 0xF0)),
                })
            }
            .map_err(|e| eprintln!("SetBkColor failed: {e}"))
            .ok();

            // Set the background color of the element in the checkbox listview
            unsafe {
                self.top_toggle.hwnd().SendMessage(SetTextBkColor {
                    color: Option::from(COLORREF::from_rgb(0xF0, 0xF0, 0xF0)),
                })
            }
            .map_err(|e| eprintln!("WM_CTLCOLORLISTBOX failed: {e}"))
            .ok();

            // Set the listview to use the Explorer theme to make the item selection boxes stretch to the right edge of the window
            self.process_list
                .hwnd()
                .SetWindowTheme("Explorer", None)
                .map_err(|e| eprintln!("SetWindowTheme failed: {e}"))
                .ok();
        }
    }

    fn refresh_process_list(
        &self,
        windows: &mut MutexGuard<Vec<w::HWND>>,
        scan_windows: bool,
    ) -> w::AnyResult<()> {
        // Get the current DPI
        let dpi = self.app_dpi.load(Ordering::Relaxed) as i32;

        // Create an image list to store the icons
        let image_list = HIMAGELIST::Create(
            SIZE::with(16 * dpi / 120, 16 * dpi / 120),
            co::ILC::COLOR32,
            0,
            100,
        )
        .unwrap_or_else(|e| {
            // If creating the image list failed, disable the use of icons
            self.use_icons.store(false, Ordering::SeqCst);
            eprintln!("Imagelist Creation failed {e}");
            unsafe { ImageListDestroyGuard::new(HIMAGELIST::NULL) }
        });

        // Enumerate over all open windows
        if scan_windows {
            // Clear the process list, window vector, and icon cache
            self.process_list.items().delete_all()?;
            windows.clear();
            if let Ok(mut window_icons) = self.window_icons.lock() {
                window_icons.clear();
            }

            EnumWindows(|hwnd: w::HWND| -> bool {
                // Skip invisible windows
                if !hwnd.IsWindowVisible() {
                    return true;
                }

                // Get the window title and return if an error occurred
                let Ok(title) = hwnd.GetWindowText() else {
                    return true;
                };
                if title.is_empty() || self.excluded_apps.contains(&title) {
                    return true;
                }

                let icon_id = if self.use_icons.load(Ordering::SeqCst) {
                    // Get the window icon
                    let icon = match unsafe {
                        HICON::from_ptr(hwnd.SendMessage(w::msg::WndMsg::new(
                            co::WM::GETICON,
                            co::ICON_SZ::SMALL.raw() as usize,
                            0,
                        )) as *mut _)
                    } {
                        icon if icon.as_opt().is_some() => icon,
                        _ => {
                            // If retrieving the icon failed, try a different method
                            // See https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-geticon#remarks
                            let icon = unsafe {
                                HICON::from_ptr(hwnd.GetClassLongPtr(co::GCLP::HICONSM) as *mut _)
                            };

                            if icon == HICON::NULL || icon == HICON::INVALID {
                                // Try retrieving the large icon
                                let icon = unsafe {
                                    HICON::from_ptr(hwnd.GetClassLongPtr(co::GCLP::HICON) as *mut _)
                                };

                                if icon == HICON::NULL || icon == HICON::INVALID {
                                    // Check if the title is already in the list
                                    if let Some(existing_item) = self
                                        .process_list
                                        .items()
                                        .iter()
                                        .find(|item| item.text(0) == title)
                                    {
                                        // UWP apps can sometimes show up multiple times in the list
                                        // This is due to one being the ApplicationFrameHost.exe that manages the UWP app container
                                        // ApplicationFrameHost.exe seems to always be the first instance of the app in the list
                                        // Remove the existing item from the list
                                        unsafe {
                                            self.process_list
                                                .hwnd()
                                                .SendMessage(w::msg::lvm::DeleteItem {
                                                    index: existing_item.index(),
                                                })
                                        }
                                        .map_err(|e| {
                                            eprintln!("Failed to remove duplicate item from process list - DeleteItem failed: {e}");
                                        })
                                        .ok();
                                    }

                                    // Likely a UWP app, try retrieving the icon from the app package
                                    create_hicon_from_hwnd(&hwnd)
                                } else {
                                    icon
                                }
                            } else {
                                icon
                            }
                        }
                    };

                    // Cache the icon
                    if let Ok(mut window_icons) = self.window_icons.lock() {
                        window_icons.push(icon.CopyIcon().unwrap_or_else(|_| unsafe {
                            w::guard::DestroyIconGuard::new(HICON::NULL)
                        }));
                    }
                    // Add the icon to the image list
                    match image_list.AddIcon(&icon) {
                        Ok(id) => Some(id),
                        Err(e) => {
                            eprintln!(
                                "AddIcon failed: '{e}' - GetLastError: '{}'",
                                w::GetLastError()
                            );
                            None
                        }
                    }
                } else {
                    None
                };

                // Add the window to the vector
                windows.push(hwnd);

                // Add the window to the list
                self.process_list
                    .items()
                    .add(&[title], icon_id, ())
                    .map_err(|e| {
                        eprintln!("Failed to add item to process list - Add failed: {e}");
                    })
                    .ok();

                // Return true to continue enumerating
                true
            })
            .map_err(|e| eprintln!("EnumWindows failed: {e}"))
            .ok();
        } else {
            // Add icons to the new image list from the icon cache
            if self.use_icons.load(Ordering::SeqCst)
                && let Ok(window_icons) = self.window_icons.lock()
            {
                for icon in window_icons.iter() {
                    image_list.AddIcon(icon).unwrap_or_else(|e| {
                        eprintln!("AddIcon failed {e}\n");
                        u32::MAX
                    });
                }
            }
        }

        // Set the image list for the listview
        let _ = unsafe {
            self.process_list
                .hwnd()
                .SendMessage(w::msg::lvm::SetImageList {
                    himagelist: Some(image_list.raw_copy()),
                    kind: co::LVSIL::SMALL,
                })
        };

        // Store the image list, dropping the old one
        if let Ok(mut imagelist) = self.imagelist.lock() {
            imagelist.replace(image_list);
        }

        Ok(())
    }

    fn events(&self) {
        // Create a vector in a mutex to store the open windows
        let windows: Arc<Mutex<Vec<w::HWND>>> = Arc::new(Mutex::new(Vec::new()));

        self.wnd.on().wm_create({
            let self2 = self.clone();
            move |create| -> w::AnyResult<i32> {
                // Store the current DPI
                self2
                    .app_dpi
                    .store(self2.wnd.hwnd().GetDpiForWindow(), Ordering::Relaxed);

                // Change the font in the buttons and label
                self2.update_font();

                // Set the theme of the window
                self2.set_system_theme();

                // Refresh the process list
                self2.refresh_btn.trigger_click();

                // Start an one-shot timer for some post-creation tasks
                self2.wnd.hwnd().SetTimer(1, 1, None).ok();

                // Call the default window procedure
                unsafe { self2.wnd.hwnd().DefWindowProc(create) };

                Ok(0)
            }
        });

        // Handle post-creation tasks
        self.wnd.on().wm_timer(1, {
            let self2 = self.clone();
            move || {
                // Stop the timer
                self2.wnd.hwnd().KillTimer(1).ok();

                // Add text to the checkbox listview
                self2.top_toggle.items().add(
                    &["Apply \"stay on top\" flag to avoid taskbar flickering"],
                    None,
                    (),
                )?;

                // Set the canvas as the button's parent
                self2.refresh_btn.hwnd().SetParent(self2.btn_canvas.hwnd()).ok();
                self2.help_btn.hwnd().SetParent(self2.btn_canvas.hwnd()).ok();
                self2.fullscreenize_btn.hwnd().SetParent(self2.btn_canvas.hwnd()).ok();

                // Force the buttons to repaint
                // Without this, the buttons are not visible until they are updated by hovering over them
                self2.refresh_btn.hwnd().InvalidateRect(None, true).map_err(|e| {
                    eprintln!("Failed to trigger a paint of the refresh button - InvalidateRect Failed: {e}")
                }).ok();
                self2.help_btn.hwnd().InvalidateRect(None, true).map_err(|e| {
                    eprintln!("Failed to trigger a paint of the help button - InvalidateRect Failed: {e}")
                }).ok();
                self2.fullscreenize_btn.hwnd().InvalidateRect(None, true).map_err(|e| {
                    eprintln!("Failed to trigger a paint of the fullscreenize button - InvalidateRect Failed: {e}")
                }).ok();

                // Hide the process list's horizontal scrollbar
                // The scrollbar would otherwise appear since the process list's column is wider than the listview
                self2.process_list.hwnd().ShowScrollBar(
                    co::SBB::HORZ,
                    false
                ).map_err(|e| {
                    eprintln!("Failed to hide horizontal scrollbar - ShowScrollBar Failed: {e}");
                }).ok();

                Ok(())
            }
        });

        // Receive the button click events and forward them to the main window
        // This is necessary to ensure that the main window receives the button click events
        self.btn_canvas.on_subclass().wm(w::co::WM::COMMAND, {
            let self2 = self.clone();
            move |a| {
                // Forward the message to the main window
                unsafe {
                    self2.wnd.hwnd().SendMessage(w::msg::WndMsg::new(
                        co::WM::COMMAND,
                        a.wparam,
                        a.lparam,
                    ));
                }
                Ok(1)
            }
        });

        // Handle DPI changes
        self.wnd.on().wm(co::WM::DPICHANGED, {
            let self2 = self.clone();
            let windows = windows.clone();
            move |dpi_changed: w::msg::WndMsg| {
                // Store the new DPI of the window
                // LOWORD and HIWORD of the wParam contains the X and Y DPI values, which should be the same
                self2
                    .app_dpi
                    .store((dpi_changed.wparam & 0xFFFF) as u32, Ordering::Relaxed);

                // Change the font of the label
                self2.update_font();

                // Refresh the process list without scanning for new windows
                match windows.lock() {
                    Ok(mut windows) => {
                        // Refresh the process list
                        self2.refresh_process_list(&mut windows, false)?;
                    }
                    Err(e) => {
                        // Show a popup window with the error message
                        show_error_message(
                            format!("Failed to refresh process list - Mutex lock failed: {e}")
                                .as_str(),
                        );
                    }
                }

                // Call the default window procedure
                unsafe { self2.wnd.hwnd().DefWindowProc(dpi_changed) };

                Ok(0)
            }
        });

        self.wnd.on().wm_get_min_max_info({
            let self2 = self.clone();
            move |min_max| {
                // Get the current dpi of the window
                let app_dpi = self2.app_dpi.load(Ordering::Relaxed);

                // Set the minimum size of the window
                min_max.info.ptMinTrackSize.x = (305 * app_dpi / 120) as i32;
                min_max.info.ptMinTrackSize.y = (200 * app_dpi / 120) as i32;

                Ok(())
            }
        });

        self.wnd.on().wm_size({
            let self2 = self.clone();
            move |size| -> w::AnyResult<()> {
                // Get the current dpi of the window
                let app_dpi = self2.app_dpi.load(Ordering::Relaxed);

                // Get the new window dimensions
                let new_size = RECT {
                    left: 0,
                    top: 0,
                    right: size.client_area.cx,
                    bottom: size.client_area.cy,
                };

                // Move the label to be in between the top of the window and the top of the process list
                self2
                    .label
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::with(
                            (10 * app_dpi / 120) as i32,
                            (((29 - 20) * app_dpi / 120) / 2) as i32,
                        ),
                        SIZE::with(
                            (new_size.right - new_size.left) - (20 * app_dpi / 120) as i32,
                            (20 * app_dpi / 120) as i32,
                        ),
                        SWP::NOZORDER,
                    )
                    .map_err(|e| eprintln!("Failed to move label - SetWindowPos Failed: {e}"))
                    .ok();

                // Move the process list to be below the label
                self2
                    .process_list
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::with((8 * app_dpi / 120) as i32, (29 * app_dpi / 120) as i32),
                        SIZE::with(
                            (new_size.right - new_size.left) - (16 * app_dpi / 120) as i32,
                            (new_size.bottom - new_size.top)
                                - ((29 + 25 + 33 + 20) * app_dpi / 120) as i32,
                        ),
                        SWP::NOZORDER,
                    )
                    .map_err(|e| {
                        eprintln!("Failed to resize process list - SetWindowPos Failed: {e}")
                    })
                    .ok();

                // Resize the only column in the process list to be slightly wider than the listview
                // This prevents a vertical line indicating the end of the column from being visible
                self2
                    .process_list
                    .cols()
                    .get(0)
                    .set_width((new_size.right - new_size.left) - (16 * app_dpi / 120) as i32)
                    .map_err(|e| {
                        eprintln!("Failed to resize process list column - SetWidth Failed: {e}")
                    })
                    .ok();

                // Hide the process list's horizontal scrollbar
                // The scrollbar would otherwise appear since the process list's column is wider than the listview
                self2
                    .process_list
                    .hwnd()
                    .ShowScrollBar(co::SBB::HORZ, false)
                    .map_err(|e| {
                        eprintln!(
                            "Failed to hide horizontal scrollbar - ShowScrollBar Failed: {e}"
                        );
                    })
                    .ok();

                // Resize and move the checkbox listview
                self2
                    .top_toggle
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::with(
                            (2 * app_dpi / 120) as i32,
                            (29 * app_dpi / 120) as i32
                                + ((new_size.bottom - new_size.top)
                                    - ((29 + 20 + 20 + 33) * app_dpi / 120) as i32),
                        ),
                        SIZE::with(
                            (new_size.right - new_size.left) - (4 * app_dpi / 120) as i32,
                            (25 * app_dpi / 120) as i32,
                        ),
                        SWP::NOZORDER,
                    )
                    .map_err(|e| {
                        eprintln!("Failed to resize checkbox listview - SetWindowPos Failed: {e}")
                    })
                    .ok();

                // Determine the new size of the buttons
                let btn_size: SIZE =
                    if new_size.right - new_size.left >= (381 * app_dpi / 120) as i32 {
                        SIZE::with((110 * app_dpi / 120) as i32, (33 * app_dpi / 120) as i32)
                    } else {
                        SIZE::with(
                            ((new_size.right - new_size.left) / 3) - 16,
                            (33 * app_dpi / 120) as i32,
                        )
                    };

                // Resize and move the button canvas
                self2
                    .btn_canvas
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::with(0, new_size.bottom - (40 * app_dpi / 120) as i32),
                        SIZE::with(new_size.right - new_size.left, (33 * app_dpi / 120) as i32),
                        SWP::NOZORDER,
                    )
                    .map_err(|e| {
                        eprintln!("Failed to resize button canvas - SetWindowPos Failed: {e}")
                    })
                    .ok();

                // Resize and align the buttons
                self2
                    .help_btn
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::with(
                            ((new_size.right - new_size.left) / 2) - (btn_size.cx / 2),
                            0,
                        ),
                        btn_size,
                        SWP::NOZORDER,
                    )
                    .map_err(|e| eprintln!("Failed to move help button - SetWindowPos Failed: {e}"))
                    .ok();
                self2
                    .refresh_btn
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::with((13 * app_dpi / 120) as i32, 0),
                        btn_size,
                        SWP::NOZORDER,
                    )
                    .map_err(|e| {
                        eprintln!("Failed to move refresh button - SetWindowPos Failed: {e}")
                    })
                    .ok();
                self2
                    .fullscreenize_btn
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::with(
                            new_size.right - btn_size.cx - (13 * app_dpi / 120) as i32,
                            0,
                        ),
                        btn_size,
                        SWP::NOZORDER,
                    )
                    .map_err(|e| {
                        eprintln!("Failed to move fullscreenize button - SetWindowPos Failed: {e}")
                    })
                    .ok();

                Ok(())
            }
        });

        // Stores the brush used to paint the label's background
        let label_hbrush: Arc<Mutex<HBRUSH>> = Arc::new(Mutex::new(HBRUSH::NULL));
        self.wnd.on().wm_ctl_color_static({
            let self2 = self.clone();
            move |ctl| {
                // Light mode background color and dark mode text color
                let mut color = COLORREF::from_rgb(0xF0, 0xF0, 0xF0);

                if self2.is_dark_mode.load(Ordering::Relaxed) {
                    // Set the text color of the label to white
                    let _old_color = ctl
                        .hdc
                        .SetTextColor(color)
                        .map_err(|e| eprintln!("SetTextColor on the label failed: {e}"));

                    // Set the color to the dark mode background color
                    color = COLORREF::from_rgb(0x1E, 0x1E, 0x1E);
                }

                // Set the background color of the label's text
                let _old_bk_color = ctl
                    .hdc
                    .SetBkColor(color)
                    .map_err(|e| eprintln!("SetBkColor on the label failed: {e}"));

                // If the brush in the Arc Mutex is NULL, create a new solid brush
                if let Ok(mut label_hbrush) = label_hbrush.lock() {
                    if *label_hbrush == HBRUSH::NULL {
                        HBRUSH::CreateSolidBrush(color).map_or_else(
                            |e| {
                                eprintln!("CreateSolidBrush failed: {e}");
                            },
                            |mut hbrush| {
                                // Set the brush in the Arc Mutex
                                *label_hbrush = hbrush.leak();
                            },
                        );
                    }
                }

                // Set the background color of the label
                Ok(label_hbrush
                    .lock()
                    .map_or(HBRUSH::NULL, |hbrush| unsafe { hbrush.raw_copy() }))
            }
        });

        self.wnd.on().wm_erase_bkgnd({
            let self2 = self.clone();
            move |erase_bkgnd| -> w::AnyResult<i32> {
                // Set the background color of the window in dark mode
                if self2.is_dark_mode.load(Ordering::Relaxed) {
                    // Create a solid brush with the dark mode background color
                    match HBRUSH::CreateSolidBrush(COLORREF::from_rgb(0x1E, 0x1E, 0x1E)) {
                        Ok(hbrush) => {
                            match self2.wnd.hwnd().GetClientRect() {
                                Ok(rect) => {
                                    // Set the background color of the window
                                    erase_bkgnd
                                        .hdc
                                        .FillRect(rect, &hbrush)
                                        .map_err(|e| eprintln!("FillRect failed: {e}"))
                                        .ok();

                                    return Ok(1);
                                }
                                Err(e) => {
                                    eprintln!("GetClientRect failed: {e}");
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("CreateSolidBrush failed: {e}");
                        }
                    }
                }

                // Call the default window procedure
                unsafe { self2.wnd.hwnd().DefWindowProc(erase_bkgnd) };

                Ok(0)
            }
        });

        self.top_toggle.on().nm_click({
            let self2 = self.clone();
            move |_| {
                // Disable highlighting the item by clicking on it (Selecting with the arrow keys still works)
                self2
                    .top_toggle
                    .items()
                    .get(0)
                    .select(false)
                    .map_err(|e| eprintln!("Failed to deselect the top toggle item: {e}"))
                    .ok();

                Ok(())
            }
        });

        self.refresh_btn.on().bn_clicked({
            let self2 = self.clone();
            let windows = windows.clone();
            move || {
                // Lock the processes mutex
                match windows.lock() {
                    Ok(mut windows) => {
                        // Refresh the process list
                        self2.refresh_process_list(&mut windows, true)?;
                    }
                    Err(e) => {
                        // Show a popup window with the error message
                        show_error_message(
                            format!("Failed to refresh process list - Mutex lock failed: {e}")
                                .as_str(),
                        );
                    }
                }

                Ok(())
            }
        });

        self.help_btn.on().bn_clicked({
            move || {
                // TODO: Maybe replace with settings
                show_help_message();
                Ok(())
            }
        });

        self.fullscreenize_btn.on().bn_clicked({
            let self2 = self.clone();
            move || {
                // Get the selected item
                let selected_item = match self2.process_list.items().iter_selected().next() {
                    Some(selected_item) => {
                        // Fullscreenize the selected window
                        selected_item
                    }
                    None => {
                        eprintln!("Failed to fullscreenize window - Could not get selected item (no item selected?)");
                        return Ok(());
                    }
                };

                // Lock the window mutex
                let window = match windows.lock() {
                    Ok(windows) => windows,
                    Err(e) => {
                        show_error_message(format!("Failed to fullscreenize window - Mutex lock failed: {e}").as_str());
                        return Ok(());
                    }
                };

                // Get the selected window
                let window = match window.get(selected_item.index() as usize) {
                    Some(window) => window,
                    None => {
                        show_error_message("Failed to fullscreenize window - Could not get the selected window from the list");
                        return Ok(());
                    }
                };

                // Get the dimensions of the monitor the window is on
                let Ok(monitor_info) = window
                    .MonitorFromWindow(co::MONITOR::DEFAULTTONEAREST)
                    .GetMonitorInfo()
                    .map_err(|e| show_error_message(&format!("Failed to fullscreenize window - GetMonitorInfo failed with error: {e}")))
                    else {
                        return Ok(());
                    };
                let mut rect = RECT {
                    left: 0,
                    top: 0,
                    right: monitor_info.rcMonitor.right - monitor_info.rcMonitor.left,
                    bottom: monitor_info.rcMonitor.bottom - monitor_info.rcMonitor.top,
                };

                // Set the window style
                window.set_style(co::WS::POPUP | co::WS::VISIBLE);

                // Set the window size
                match AdjustWindowRectEx(rect, window.style(), false, window.style_ex()) {
                    Ok(rct) => rect = rct,
                    Err(e) => {
                        show_error_message(&format!("Failed to fullscreenize window - AdjustWindowRectEx failed with error: {e}"));
                        return Ok(());
                    }
                }

                // Set window to stay on top if checkbox is checked
                if unsafe {
                    self2.top_toggle.hwnd().SendMessage(
                        w::msg::lvm::GetItemState {
                            index: 0,
                            mask: co::LVIS::STATEIMAGEMASK
                        }
                    )
                }
                // 0x1000 = Unchecked, 0x2000 = Checked
                .raw() & 0x2000 != 0 {
                    window.set_style_ex(window.style_ex() | co::WS_EX::TOPMOST);
                }

                // Set the window position
                window
                    .MoveWindow(
                        POINT::with(rect.left, rect.top),
                        SIZE::with(rect.right - rect.left, rect.bottom - rect.top),
                        true,
                    )
                    .map_err(|e| show_error_message(&format!("Failed to fullscreenize window - MoveWindow failed with error: {e}")))
                    .ok();

                Ok(())
            }
        });
    }
}

/// Function to show an error message in a popup window
/// # Arguments
/// * `message` - The error message to display
/// # Returns
/// * None
fn show_error_message(message: &str) {
    // Show a popup window with the error message
    w::HWND::NULL
        .TaskDialog(
            Some("Error"),
            None,
            Some(message),
            co::TDCBF::OK,
            w::IconRes::Error,
        )
        .map_err(|e| eprintln!("TaskDialog failed: {e}"))
        .ok();
}

/// Function to show a help message in a popup window
fn show_help_message() {
    // Show a popup window with the help message
    // TODO: Create custom window so dark mode can be implemented
    w::HWND::NULL
        .TaskDialog(
            Some("Fullscreenizer"),
            None,
            Some("Open the game you want to force in borderless-windowed-fullscreen mode, \
                 set it to windowed mode to the resolution you want, hit the Refresh button \
                 to refresh the windows list, select the game window from the list and press \
                 the Fullscreenize button.  The window will be resized to the desktop area and \
                 the border will be removed.  Note that using a different in-game resolution \
                 from the desktop resolution may not work properly (or at all) depending on the game.\n\n\n\
                 Made by Carter Persall\n\
                 Based on the program by Kostas \"Bad Sector\" Michalopoulos"),
            co::TDCBF::OK,
            w::IconRes::None,
        )
        .map_err(|e| eprintln!("TaskDialog failed: {e}"))
        .ok();
}

/// Attempts to retrieve the icon from a UWP app's window handle.
/// If successful, creates an HICON from the icon file path using WIC.
/// Returns HICON::NULL if any step fails.
fn create_hicon_from_hwnd(hwnd: &w::HWND) -> w::HICON {
    match get_uwp_icon_path_from_hwnd(hwnd) {
        Ok(Some(path)) => match create_hicon_from_path(&path) {
            Ok(icon) => icon,
            Err(e) => {
                eprintln!("create_hicon_from_path failed: {e}");
                HICON::NULL
            }
        },
        Ok(None) => HICON::NULL,
        Err(e) => {
            eprintln!("get_uwp_icon_path_from_hwnd failed: {e}");
            HICON::NULL
        }
    }
}

/// Attempts to retrieve the path to a UWP app's icon from its window handle.
/// Returns Ok(None) if the window does not belong to a packaged UWP app.
/// Returns Err if any Win32 API call fails unexpectedly.
fn get_uwp_icon_path_from_hwnd(hwnd: &w::HWND) -> Result<Option<PathBuf>, String> {
    // Get the package full name from the process handle
    let package_family_name = unsafe {
        SHGetPropertyStoreForWindow::<IPropertyStore>(windows::Win32::Foundation::HWND(hwnd.ptr()))
            .map_err(|e| format!("SHGetPropertyStoreForWindow failed with error: {e}"))?
            .GetValue(&PKEY_AppUserModel_ID)
            .map_err(|e| format!("GetValue failed with error: {e}"))?
            .Anonymous
            .Anonymous
            .Anonymous
            .pwszVal
            .to_string()
            .map_err(|e| format!("Failed to convert package full name to string: {e}"))?
            .split("!")
            .next()
            .ok_or("Package full name is empty, cannot determine if UWP app")?
            .to_string()
    };

    let package_full_name = get_package_full_name_from_family_name(&package_family_name)
        .map_err(|e| format!("get_package_full_name_from_family_name failed: {e}"))?
        .ok_or("Could not find package full name from family name")?;

    // Get the package installation directory from the full name.
    let package_path = get_package_path_by_full_name(&package_full_name)
        .map_err(|e| format!("GetPackagePathByFullName failed with error code: {e}"))?;

    // Construct path to the manifest and parse it.
    let manifest_path = package_path.join("AppxManifest.xml");
    let xml_content = match std::fs::read_to_string(manifest_path) {
        Ok(content) => content,
        Err(e) => {
            return Err(format!(
                "Failed to read AppxManifest.xml (likely a permissions issue): {e}"
            ));
        }
    };

    println!("UWP package path: {package_path:?}");

    // Parse the manifest to find the icon's relative path.
    let relative_icon_path = parse_manifest_for_icon_path(&xml_content)
        .ok_or("Could not find a suitable icon in the manifest")?;
    // Check if the file exists
    // UWP apps often provide multiple icon sizes, so we look for the largest available if needed
    let icon_path = package_path.join(&relative_icon_path);
    if icon_path.exists() {
        Ok(Some(icon_path))
    } else {
        // Try to find the icon with a suffix indicating a size, e.g. "icon.targetsize-256.png"
        // Find all files that start with the relative icon path (without extension)
        let icon_stem = Path::new(&relative_icon_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("Failed to get icon file stem")?;
        let icon_dir = package_path.join(
            Path::new(&relative_icon_path)
                .parent()
                .ok_or("Failed to get icon file parent")?,
        );
        if icon_dir.is_dir() {
            let mut candidates: Vec<PathBuf> = Vec::new();
            for entry in std::fs::read_dir(icon_dir)
                .map_err(|e| format!("Failed to read icon directory: {e}"))?
            {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
                let file_name = entry.file_name();
                let file_name_str = file_name
                    .to_str()
                    .ok_or("Failed to convert file name to string")?;
                // Check if the file name starts with the icon stem and has a valid image extension
                if file_name_str.starts_with(icon_stem)
                    && (file_name_str.ends_with(".png")
                        || file_name_str.ends_with(".jpg")
                        || file_name_str.ends_with(".ico"))
                {
                    candidates.push(entry.path());
                }
            }
            // Filter the candidates by the following conditions:
            // 1. Sort by shortest path length
            // 2. Only include files that end with a number (indicating size) before the extension
            // 3. Take the first one, and filter by it's name, excluding the size suffix
            // 4. Find the file among the candidates with the largest size suffix
            // Sort by shortest path length
            candidates.sort_by_key(|p| p.as_os_str().len());
            // Only include files that end with a number (indicating size) before the extension
            candidates.retain(|p| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.chars().next_back().is_some_and(|c| c.is_ascii_digit()))
            });
            // Take the first one, and filter by it's name, excluding the size suffix
            if let Some(base_icon_name) = candidates
                .first()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .and_then(|s| s.rsplit_once('.').map(|(name, _)| name))
            {
                // Find the file among the candidates with the largest size suffix
                let mut largest_icon: Option<PathBuf> = None;
                let mut largest_size: u32 = 0;
                for candidate in candidates.iter().filter(|p| {
                    p.file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.starts_with(base_icon_name))
                }) {
                    if let Some(size_str) =
                        candidate
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .and_then(|s| {
                                s.rsplit_once('-').map(|(_, size)| {
                                    size.trim_start_matches("targetsize")
                                        .trim_start_matches("scale")
                                })
                            })
                    {
                        if let Ok(size) = size_str.parse::<u32>() {
                            if size > largest_size {
                                largest_size = size;
                                largest_icon = Some(candidate.clone());
                            }
                        }
                    }
                }
                if let Some(largest_icon) = largest_icon {
                    if largest_icon.exists() {
                        return Ok(Some(largest_icon));
                    }
                }
            }
        }
        Err("Icon file does not exist".to_string())
    }
}

/// Creates an HICON from an image file path using WIC.
fn create_hicon_from_path(path: &Path) -> w::AnyResult<w::HICON> {
    /* 1. Create a WIC Imaging Factory. */
    let factory: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };

    /* 2. Create a decoder from the file path. */
    let decoder: IWICBitmapDecoder = unsafe {
        factory.CreateDecoderFromFilename(
            PCWSTR(
                path.as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect::<Vec<u16>>()
                    .as_ptr(),
            ),
            None,
            GENERIC_READ,
            WICDecodeMetadataCacheOnLoad,
        )?
    };

    /* 3. Get the first frame of the image. */
    let frame: IWICBitmapFrameDecode = unsafe { decoder.GetFrame(0)? };

    /* 4. Create a format converter to ensure the image is in 32bpp PBGRA format. */
    let converter: IWICFormatConverter = unsafe { factory.CreateFormatConverter()? };
    unsafe {
        converter
            .Initialize(
                &frame,                         // Input source
                &GUID_WICPixelFormat32bppBGRA, // Destination format
                WICBitmapDitherTypeNone,
                None, // No custom palette
                0.0,  // Alpha threshold
                WICBitmapPaletteTypeMedianCut,
            )
            .map_err(|e| format!("Failed to initialize WIC format converter: {e}"))?;
    }

    let mut width = 0;
    let mut height = 0;
    unsafe { converter.GetSize(&mut width, &mut height)? };
    let stride = width * 4; // 4 bytes per pixel (B, G, R, A)
    let mut buffer = vec![0u8; (stride * height) as usize];
    unsafe { converter.CopyPixels(std::ptr::null(), stride, buffer.as_mut_slice()) }
        .map_err(|e| format!("Failed to copy pixels from WIC converter: {e}"))?;

    /* 5. Create a 32-bit HBITMAP using CreateDIBSection for the color part. */
    let bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32), // Negative height for a top-down DIB
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut bitmap_pixels: *mut std::ffi::c_void = std::ptr::null_mut();
    let hbmp_color = unsafe {
        CreateDIBSection(
            None,
            &bitmap_info as *const _,
            DIB_RGB_COLORS,
            &mut bitmap_pixels,
            None,
            0,
        )
    // Wrap the HBITMAP in a DeleteObjectGuard to ensure it gets deleted
    }.map(|hbmp_color| unsafe {
        w::guard::DeleteObjectGuard::new(w::HBITMAP::from_ptr(hbmp_color.0))
    })?;

    /* 6. Copy the WIC pixel data into the DIB section's memory. */
    unsafe {
        std::ptr::copy_nonoverlapping(buffer.as_ptr(), bitmap_pixels as *mut u8, buffer.len());
    }

    /* 7. Create a monochrome mask bitmap (all black is fine for 32bpp alpha icons). */
    let hbmp_mask = w::HBITMAP::CreateBitmap(
        w::SIZE::with(width as i32, height as i32),
        1,
        1,
        vec![0u8; (width * height / 8) as usize].as_mut_ptr(),
    )?;

    /* 8. Create the icon using ICONINFO. */
    let icon_info = ICONINFO {
        fIcon: TRUE,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: windows::Win32::Graphics::Gdi::HBITMAP(hbmp_mask.ptr()),
        hbmColor: windows::Win32::Graphics::Gdi::HBITMAP(hbmp_color.ptr()),
    };

    let hicon = unsafe { CreateIconIndirect(&icon_info) }
        .map_err(|e| format!("CreateIconIndirect failed with error: {e}"))?;

    Ok(unsafe { HICON::from_ptr(hicon.0) })
}

/// Finds the first package full name for a given package family name.
fn get_package_full_name_from_family_name(family_name: &str) -> Result<Option<String>, u32> {
    let family_name_wide: Vec<u16> = family_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut count: u32 = 0;
    let mut buffer_len: u32 = 0;
    let package_types = PACKAGE_FILTER_HEAD;

    // First, get the count and required buffer size.
    let result = unsafe {
        FindPackagesByPackageFamily(
            PCWSTR(family_name_wide.as_ptr()),
            package_types,
            &mut count,
            None,
            &mut buffer_len,
            None,
            None,
        )
    };

    if result.0 != ERROR_INSUFFICIENT_BUFFER.0 || count == 0 {
        return Ok(None); // No packages found or another error occurred.
    }

    let mut full_names_buffer = vec![0u16; buffer_len as usize];
    let mut full_names_ptrs: Vec<PWSTR> = vec![PWSTR::null(); count as usize];
    let mut properties_buf: u32 = 0;

    // Now, get the actual package full names.
    let result = unsafe {
        FindPackagesByPackageFamily(
            PCWSTR(family_name_wide.as_ptr()),
            package_types,
            &mut count,
            Some(full_names_ptrs.as_mut_ptr()),
            &mut buffer_len,
            Some(PWSTR(full_names_buffer.as_mut_ptr())),
            Some(&mut properties_buf),
        )
    };

    if result.is_err() {
        return Err(result.0 as u32);
    }

    // We only need the first one.
    if !full_names_ptrs.is_empty() && !full_names_ptrs[0].is_null() {
        let name_str = unsafe { full_names_ptrs[0].to_string().unwrap_or_default() };
        return Ok(Some(name_str));
    }

    Ok(None)
}

/// A wrapper for GetPackagePathByFullName to handle buffer sizing.
fn get_package_path_by_full_name(package_full_name: &str) -> Result<PathBuf, u32> {
    let mut buffer_len: u32 = 0;
    let wide_name: Vec<u16> = package_full_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let result =
        unsafe { GetPackagePathByFullName(PCWSTR(wide_name.as_ptr()), &mut buffer_len, None) };

    if result.0 != ERROR_INSUFFICIENT_BUFFER.0 {
        return Err(result.0 as u32);
    }

    let mut buffer = vec![0u16; buffer_len as usize];
    let result = unsafe {
        GetPackagePathByFullName(
            PCWSTR(wide_name.as_ptr()),
            &mut buffer_len,
            Some(PWSTR(buffer.as_mut_ptr())),
        )
    };

    if result.is_err() {
        return Err(result.0 as u32);
    }

    Ok(PathBuf::from(String::from_utf16_lossy(
        &buffer[..(buffer_len as usize - 1)],
    )))
}

/// Parses XML content to find the logo path. Prefers 'Square44x44Logo'.
fn parse_manifest_for_icon_path(xml_content: &str) -> Option<String> {
    // Prefer the VisualElements logo if it exists
    {
        let start_tag = "<uap:VisualElements";
        let end_tag = ">";
        // Search for the <uap:VisualElements> tag
        xml_content.find(start_tag).and_then(|start_idx| {
            xml_content[start_idx..].find(end_tag).and_then(|end_idx| {
                // Extract the tag content
                let tag_content = &xml_content[start_idx..start_idx + end_idx + end_tag.len()];
                // First try to find the Square44x44Logo attribute
                tag_content.find("Square44x44Logo=\"").and_then(|logo_attr_start| {
                    let logo_value_start = logo_attr_start + "Square44x44Logo=\"".len();
                    tag_content[logo_value_start..].find('"').map(|logo_value_end| {
                        tag_content[logo_value_start..logo_value_start + logo_value_end]
                            .to_string()
                    })
                }).or_else(|| {
                    // If not found, try to find the Logo attribute
                    tag_content.find("Logo=\"").and_then(|logo_attr_start| {
                        let logo_value_start = logo_attr_start + "Logo=\"".len();
                        tag_content[logo_value_start..].find('"').map(|logo_value_end| {
                            tag_content[logo_value_start..logo_value_start + logo_value_end]
                                .to_string()
                        })
                    })
                })
            })
        })
    }
    // If not found, fall back to the Application logo
    .or({
        let start_tag = "<uap:Application";
        let end_tag = ">";
        // Search for the <uap:Application> tag
        xml_content.find(start_tag).and_then(|start_idx| {
            xml_content[start_idx..].find(end_tag).and_then(|end_idx| {
                // Extract the tag content
                let tag_content = &xml_content[start_idx..start_idx + end_idx + end_tag.len()];
                tag_content.find("Logo=\"").and_then(|logo_attr_start| {
                    // Extract the logo attribute value, which contains the relative path to the icon
                    let logo_value_start = logo_attr_start + "Logo=\"".len();
                    tag_content[logo_value_start..].find('"').map(|logo_value_end| {
                        tag_content[logo_value_start..logo_value_start + logo_value_end]
                            .to_string()
                    })
                })
            })
        })
    })
}
