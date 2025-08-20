extern crate alloc;

use alloc::sync::Arc;
use std::ops::Shr;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, MutexGuard, RwLock};

use winsafe::co::SWP;
use winsafe::guard::ImageListDestroyGuard;
use winsafe::gui::dpi;
use winsafe::msg::lvm::{SetBkColor, SetTextBkColor, SetTextColor};
use winsafe::msg::wm::Paint;
use winsafe::prelude::{GuiParent, GuiWindow, Handle};
use winsafe::{
    self as w, AdjustWindowRectEx, COLORREF, DwmAttr, EnumWindows, GetLastError, HBRUSH, HICON,
    HIMAGELIST, HwndPlace, POINT, RECT, SIZE, co, gui,
};

/// Macro to handle the result of a mutex lock
/// # Arguments
/// * `result` - The result of the mutex lock
/// # Returns
/// * If the lock is successful, returns Some(guard)
/// * If the lock fails, prints an error message and returns None
macro_rules! handle_lock_result {
    ($result:expr) => {
        match $result {
            Ok(guard) => Some(guard),
            Err(e) => {
                eprintln!("Failed to lock mutex: {}", e);
                None
            }
        }
    };
}

#[derive(Clone)]
pub struct MyWindow {
    // Window elements
    wnd: gui::WindowMain,
    label: gui::Label,
    process_list: gui::ListView,
    top_toggle: gui::ListView,
    refresh_btn: gui::Button,
    help_btn: gui::Button,
    fullscreenize_btn: gui::Button,

    // Settings
    is_dark_mode: Arc<Mutex<bool>>,
    use_icons: Arc<AtomicBool>,

    // Shared resources
    app_font: Arc<Mutex<Option<w::guard::DeleteObjectGuard<w::HFONT>>>>,
    app_dpi: Arc<RwLock<u32>>,
}

impl MyWindow {
    pub fn new() -> Self {
        let wnd = gui::WindowMain::new(gui::WindowMainOpts {
            title: "Fullscreenizer".to_owned(),
            class_icon: gui::Icon::Id(101),
            size: dpi(305, 400),
            style: gui::WindowMainOpts::default().style
                | co::WS::OVERLAPPEDWINDOW
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
                control_ex_style: co::LVS_EX::DOUBLEBUFFER | co::LVS_EX::AUTOSIZECOLUMNS,
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

        let refresh_btn = gui::Button::new(
            &wnd,
            gui::ButtonOpts {
                text: "&Refresh".to_owned(),
                position: dpi(13, 368),
                window_ex_style: co::WS_EX::LAYERED,
                ..Default::default()
            },
        );

        let help_btn = gui::Button::new(
            &wnd,
            gui::ButtonOpts {
                text: "&Help".to_owned(),
                position: dpi(108, 368),
                window_ex_style: co::WS_EX::LAYERED,
                ..Default::default()
            },
        );

        let fullscreenize_btn = gui::Button::new(
            &wnd,
            gui::ButtonOpts {
                text: "&Fullscreenize".to_owned(),
                position: dpi(202, 368),
                window_ex_style: co::WS_EX::LAYERED,
                ..Default::default()
            },
        );

        /* Settings */
        // Whether dark mode is enabled
        let is_dark_mode = Arc::new(Mutex::new(false));
        // Whether to use icons in the process list
        let use_icons = Arc::new(AtomicBool::new(true));

        /* Shared Resources */
        // The application's font
        let app_font = Arc::new(Mutex::new(None));
        // The current DPI of the window
        // This is used to scale the window elements based on a 1440p (120 DPI) display
        let app_dpi = Arc::new(RwLock::new(120));

        let new_self = Self {
            wnd,
            label,
            process_list,
            top_toggle,
            refresh_btn,
            help_btn,
            fullscreenize_btn,
            is_dark_mode,
            app_font,
            app_dpi,
            use_icons,
        };

        new_self.events();
        new_self
    }

    pub fn run(&self) -> w::AnyResult<i32> {
        self.wnd.run_main(None)
    }

    fn update_font(&self) {
        // Get the current DPI
        let app_dpi = match self.app_dpi.read() {
            Ok(app_dpi) => *app_dpi,
            Err(e) => {
                eprintln!("Failed to read DPI - Failed to read from RwLock: {e}");
                120
            }
        };

        // Create a new font based on the current DPI
        let font = match w::HFONT::CreateFont(
            SIZE {
                cx: 0,
                cy: -((15 * app_dpi / 120) as i32),
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
            co::QUALITY::DRAFT,
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
        if let Ok(mut app_font) = self.app_font.lock() {
            *app_font = Some(font);
        }

        // Update the font for all controls
        match self.app_font.lock() {
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
                        self.fullscreenize_btn.hwnd().SendMessage(w::msg::wm::SetFont {
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
        if let Some(mut is_dark_mode) = handle_lock_result!(self.is_dark_mode.lock()) {
            *is_dark_mode = w::HKEY::CURRENT_USER
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
            if is_dark_mode.to_owned() {
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
    }

    fn refresh_process_list(&self, windows: &mut MutexGuard<Vec<w::HWND>>) -> w::AnyResult<()> {
        // Clear the process list and window vector
        self.process_list.items().delete_all()?;
        windows.clear();

        // Create an image list to store the icons
        let mut image_list = HIMAGELIST::Create(SIZE::with(16, 16), co::ILC::COLOR32, 0, 100)
            .unwrap_or_else(|e| {
                // If creating the image list failed, disable the use of icons
                self.use_icons.store(false, Ordering::SeqCst);
                eprintln!("Imagelist Creation failed {e}");
                unsafe { ImageListDestroyGuard::new(HIMAGELIST::NULL) }
            });

        // Enumerate over all open windows
        EnumWindows(|hwnd: w::HWND| -> bool {
            // Skip invisible windows
            if !hwnd.IsWindowVisible() {
                return true;
            }

            // Get the window title and return if an error occurred
            let Ok(title) = hwnd.GetWindowText() else {
                return true;
            };
            if title.is_empty() {
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
                            unsafe {
                                HICON::from_ptr(hwnd.GetClassLongPtr(co::GCLP::HICON) as *mut _)
                            }
                        } else {
                            icon
                        }
                    }
                };

                // Add the icon to the image list
                Option::from(image_list.AddIcon(&icon).unwrap_or_else(|e| {
                    eprintln!("AddIcon failed {e}\n");
                    u32::MAX
                }))
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

        // Set the image list for the listview
        let hil = image_list.leak();
        let old_hil = unsafe {
            self.process_list
                .hwnd()
                .SendMessage(w::msg::lvm::SetImageList {
                    himagelist: Some(hil),
                    kind: co::LVSIL::SMALL,
                })
        };

        // Drop the old imagelist
        if let Some(old_hil) = old_hil {
            unsafe {
                let _ = ImageListDestroyGuard::new(old_hil);
            }
        }

        Ok(())
    }

    fn events(&self) {
        // Indicates if the first paint event has occurred
        let first_paint = Arc::new(Mutex::new(true));

        // Some actions can't be performed in the window creation event, so they are done in the first paint event
        self.wnd.on().wm_paint({
            let self2 = self.clone();
            move || -> w::AnyResult<()> {
                // Get a handle to the window
                let wnd = self2.wnd.hwnd();

                // Check if this is the first paint event
                if let Some(mut first_paint) = handle_lock_result!(first_paint.clone().lock()) {
                    *first_paint = false;

                    // Add text to the checkbox listview
                    self2.top_toggle.items().add(
                        &["Apply \"stay on top\" flag to avoid taskbar flickering"],
                        None,
                        (),
                    )?;

                    // Paint the buttons to ensure they are visible initially
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
                }

                // Call the default window procedure
                unsafe { wnd.DefWindowProc(Paint {}) };

                Ok(())
            }
        });

        self.wnd.on().wm_create({
            let self2 = self.clone();
            move |create| -> w::AnyResult<i32> {
                // Store the current DPI
                self2.app_dpi.write()
                    .map(|mut app_dpi| {
                        *app_dpi = self2.wnd.hwnd().GetDpiForWindow();
                    })
                    .map_err(|e| {
                        eprintln!("Failed to set window DPI - Failed to write to RwLock: {e}")
                    })
                    .ok();

                // Change the font in the buttons and label
                self2.update_font();

                // Set the theme of the window
                self2.set_system_theme();

                // Refresh the process list
                self2.refresh_btn.trigger_click();

                // Call the default window procedure
                unsafe { self2.wnd.hwnd().DefWindowProc(create) };

                Ok(0)
            }
        });

        // Handle DPI changes
        self.wnd.on().wm(co::WM::DPICHANGED, {
            let self2 = self.clone();
            move |dpi_changed: w::msg::WndMsg| {
                println!("DPI changed to {}", dpi_changed.wparam & 0xFFFF);
                // Store the new DPI of the window
                self2.app_dpi.write()
                    .map(|mut app_dpi| {
                        // LOWORD and HIWORD of the wParam both contain the new DPI
                        *app_dpi = (dpi_changed.wparam & 0xFFFF) as u32;
                    })
                    .map_err(|e| {
                        eprintln!("Failed to set window DPI - Failed to write to RwLock: {e}")
                    })
                    .ok();

                // Change the font of the label
                self2.update_font();

                // Call the default window procedure
                unsafe { self2.wnd.hwnd().DefWindowProc(dpi_changed) };

                Ok(0)
            }
        });

        self.wnd.on().wm_get_min_max_info({
            let self2 = self.clone();
            move |min_max| {
                // Get the current dpi of the window
                let app_dpi = match self2.app_dpi.read() {
                    Ok(app_dpi) => *app_dpi,
                    Err(e) => {
                        eprintln!("Failed to read DPI - Failed to read from RwLock: {e}");
                        120
                    }
                };

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
                let app_dpi = match self2.app_dpi.read() {
                    Ok(app_dpi) => *app_dpi,
                    Err(e) => {
                        eprintln!("Failed to read DPI - Failed to read from RwLock: {e}");
                        120
                    }
                };

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

                // Resize the process list column
                self2
                    .process_list
                    .cols()
                    .get(0)
                    .set_width((new_size.right - new_size.left) - (16 * app_dpi / 120) as i32)
                    .map_err(|e| {
                        eprintln!("Failed to resize process list column - SetWidth Failed: {e}")
                    })
                    .ok();

                // Resize and move the checkbox listview
                // TODO: Fix the end of the checkbox turning white when changing DPI
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
                let btn_size: SIZE = if new_size.right - new_size.left >= (381 * app_dpi / 120) as i32 {
                    SIZE::with((110 * app_dpi / 120) as i32, (33 * app_dpi / 120) as i32)
                } else {
                    SIZE::with(
                        ((new_size.right - new_size.left) / 3) - 16,
                        (33 * app_dpi / 120) as i32,
                    )
                };

                // Resize and center align the help button
                // TODO: Fix the buttons wobbling when resizing vertically from the top border
                self2
                    .help_btn
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::with(
                            ((new_size.right - new_size.left) / 2) - (btn_size.cx / 2),
                            new_size.bottom - (40 * app_dpi / 120) as i32,
                        ),
                        btn_size,
                        SWP::NOZORDER,
                    )
                    .map_err(|e| eprintln!("Failed to move help button - SetWindowPos Failed: {e}"))
                    .ok();

                // Resize and align the other buttons
                self2
                    .refresh_btn
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::with(
                            (13 * app_dpi / 120) as i32,
                            new_size.bottom - (40 * app_dpi / 120) as i32,
                        ),
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
                            new_size.bottom - (40 * app_dpi / 120) as i32,
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

                if let Some(is_dark_mode) = handle_lock_result!(self2.is_dark_mode.lock()) {
                    if *is_dark_mode {
                        // Set the text color of the label to white
                        let _old_color = ctl
                            .hdc
                            .SetTextColor(color)
                            .map_err(|e| eprintln!("SetTextColor on the label failed: {e}"));

                        // Set the color to the dark mode background color
                        color = COLORREF::from_rgb(0x1E, 0x1E, 0x1E);
                    }
                }

                // Set the background color of the label's text
                let _old_bk_color = ctl
                    .hdc
                    .SetBkColor(color)
                    .map_err(|e| eprintln!("SetBkColor on the label failed: {e}"));

                // If the brush in the Arc Mutex is NULL, create a new solid brush
                if let Some(mut label_hbrush) = handle_lock_result!(label_hbrush.lock()) {
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
                Ok(handle_lock_result!(label_hbrush.lock())
                    .map_or(HBRUSH::NULL, |hbrush| unsafe { hbrush.raw_copy() }))
            }
        });

        self.wnd.on().wm_erase_bkgnd({
            let self2 = self.clone();
            move |erase_bkgnd| -> w::AnyResult<i32> {
                // Set the background color of the window in dark mode
                if handle_lock_result!(self2.is_dark_mode.lock())
                    .map_or(false, |is_dark_mode| *is_dark_mode)
                {
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

        // Indicates if the first process list paint event has occurred
        let process_list_paint_count: Arc<AtomicU8> = Arc::new(AtomicU8::new(0));

        self.process_list.on_subclass().wm_paint({
            let self2 = self.clone();
            move || {
                // Call the default window procedure to paint the process list normally
                unsafe { self2.process_list.hwnd().DefSubclassProc(Paint {}) };

                // The listview shows a vertical line when column width < listview width, and a
                // horizontal scrollbar when column width > listview width. LVS_EX::AUTOSIZECOLUMNS
                // removes the scrollbar but creates a timing problem: auto-sizing happens after
                // WM_SIZE on the initial paint, so the vertical line persists until the next resize.
                // Solution: trigger WM_SIZE after the second paint event to override the auto-sizing.
                // Checking on every paint event is suboptimal but safer than using self-modifying code.
                let paint_count = process_list_paint_count.load(Ordering::Relaxed);

                if paint_count == 1 {
                    // Increment the first paint counter
                    process_list_paint_count.store(2, Ordering::Relaxed);

                    // Trigger a resize event
                    match self2.wnd.hwnd().GetClientRect() {
                        Ok(rect) => unsafe {
                            self2.wnd.hwnd().SendMessage(w::msg::wm::Size {
                                request: co::SIZE_R::RESTORED,
                                client_area: SIZE::with(
                                    rect.right - rect.left,
                                    rect.bottom - rect.top,
                                ),
                            });
                        },
                        Err(e) => {
                            eprintln!("Failed to get client rect - GetClientRect Failed: {e}");
                        }
                    };
                } else if paint_count == 0 {
                    // Increment the first paint counter
                    process_list_paint_count.store(1, Ordering::Relaxed);
                }

                Ok(())
            }
        });

        // Create a vector in a mutex to store the open windows
        let windows: Arc<Mutex<Vec<w::HWND>>> = Arc::new(Mutex::new(Vec::new()));

        self.refresh_btn.on_subclass().wm_paint({
            let self2 = self.clone();
            move || {
                // Make the undrawn area of the button transparent
                self2
                    .refresh_btn
                    .hwnd()
                    .SetLayeredWindowAttributes(
                        COLORREF::from_rgb(0xF0, 0xF0, 0xF0),
                        255,
                        co::LWA::COLORKEY,
                    )
                    .map_err(|e| {
                        eprintln!("SetLayeredWindowAttributes on refresh button failed: {e}")
                    })
                    .ok();

                unsafe { self2.refresh_btn.hwnd().DefSubclassProc(Paint {}) };

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
                        self2.refresh_process_list(&mut windows)?;
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

        self.help_btn.on_subclass().wm_paint({
            let self2 = self.clone();
            move || {
                // Make the undrawn area of the button transparent
                self2
                    .help_btn
                    .hwnd()
                    .SetLayeredWindowAttributes(
                        COLORREF::from_rgb(0xF0, 0xF0, 0xF0),
                        255,
                        co::LWA::COLORKEY,
                    )
                    .map_err(|e| eprintln!("SetLayeredWindowAttributes on help button failed: {e}"))
                    .ok();

                unsafe { self2.help_btn.hwnd().DefSubclassProc(Paint {}) };

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

        self.fullscreenize_btn.on_subclass().wm_paint({
            let self2 = self.clone();
            move || {
                // Make the undrawn area of the button transparent
                self2
                    .fullscreenize_btn
                    .hwnd()
                    .SetLayeredWindowAttributes(
                        COLORREF::from_rgb(0xF0, 0xF0, 0xF0),
                        255,
                        co::LWA::COLORKEY,
                    )
                    .map_err(|e| {
                        eprintln!("SetLayeredWindowAttributes on fullscreenize button failed: {e}")
                    })
                    .ok();

                unsafe { self2.fullscreenize_btn.hwnd().DefSubclassProc(Paint {}) };

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
                if unsafe { window.SetWindowLongPtr(co::GWLP::STYLE, (co::WS::POPUP.raw() | co::WS::VISIBLE.raw()) as isize) } == 0 {
                    show_error_message(&format!("Failed to fullscreenize window - SetWindowLongPtr failed with error: {}", GetLastError()));
                    return Ok(());
                }

                // Set the window size
                match AdjustWindowRectEx(rect, window.style(), false, window.style_ex()) {
                    Ok(rct) => rect = rct, // TODO: Test this
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
                .raw().shr(12u32) - 1 == 1u32 &&
                unsafe {
                    window.SetWindowLongPtr(
                        co::GWLP::EXSTYLE,
                        (window.style_ex().raw() | co::WS_EX::TOPMOST.raw()) as isize,
                    )
                } == 0 {
                    show_error_message(&format!("Failed to set window to stay on top - SetWindowLongPtr failed with error: {}", GetLastError()));
                    return Ok(());
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
