extern crate alloc;

use alloc::sync::Arc;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, RwLock};

use winsafe::co::SWP;
use winsafe::guard::ImageListDestroyGuard;
use winsafe::gui::dpi;
use winsafe::msg::lvm::{SetBkColor, SetTextBkColor, SetTextColor};
use winsafe::prelude::{
    GuiEventsButton, GuiEventsLabel, GuiEventsParent, GuiEventsWindow, GuiWindow, Handle,
};
use winsafe::{
    self as w, AdjustWindowRectExForDpi, COLORREF, DwmAttr, EnumWindows, HBRUSH, HICON, HIMAGELIST,
    HwndPlace, POINT, RECT, SIZE, co, gui,
};

#[derive(Clone)]
pub struct MyWindow {
    // Window elements
    wnd: gui::WindowMain,
    label: gui::Label,
    process_list: gui::ListView,
    top_toggle: gui::CheckBox,
    top_label: gui::Label,
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
    background_hbrush: Arc<Mutex<Option<w::guard::DeleteObjectGuard<w::HBRUSH>>>>,
    imagelist: Arc<Mutex<Option<w::guard::ImageListDestroyGuard>>>,
    window_icons: Arc<Mutex<Vec<w::guard::DestroyIconGuard>>>,
}

impl MyWindow {
    pub fn new() -> Self {
        let wnd = gui::WindowMain::new(gui::WindowMainOpts {
            title: "Fullscreenizer",
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
                text: "Toplevel windows:",
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
                // Make the single column very wide, so that the end of the column is never visible
                columns: &[("", 32000)],
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
                    | co::WS::CLIPSIBLINGS,
                // Resize horizontally and vertically together with parent window.
                resize_behavior: (gui::Horz::Resize, gui::Vert::Resize),
                ..Default::default()
            },
        );

        // Checkbox to toggle the "stay on top" flag
        let top_toggle = gui::CheckBox::new(
            &wnd,
            gui::CheckBoxOpts {
                position: dpi(8, 342),
                size: dpi(20, 20),
                window_style: co::WS::CHILD
                    | co::WS::VISIBLE
                    | co::WS::TABSTOP
                    | co::WS::GROUP
                    | co::WS::CLIPSIBLINGS,
                check_state: co::BST::UNCHECKED,
                ..Default::default()
            },
        );

        // Label for the top_toggle checkbox
        // While setting the text of the checkbox can be done, the resulting text's color cannot be changed
        // Therefore, a label is used instead
        let top_label = gui::Label::new(
            &wnd,
            gui::LabelOpts {
                text: "Apply \"stay on top\" flag to avoid taskbar flickering",
                position: dpi(32, 342),
                size: dpi(338, 20),
                control_style: co::SS::LEFTNOWORDWRAP | co::SS::NOTIFY,
                window_style: co::WS::CHILD | co::WS::VISIBLE,
                ..Default::default()
            },
        );

        // Label that will be the parent of the buttons
        // This will allow for the buttons' undrawn background color to be configured
        let btn_canvas = gui::Label::new(
            &wnd,
            gui::LabelOpts {
                text: "",
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
                text: "&Refresh",
                position: dpi(13, 368),
                ..Default::default()
            },
        );

        let help_btn = gui::Button::new(
            &wnd,
            gui::ButtonOpts {
                text: "&Help",
                position: dpi(108, 368),
                ..Default::default()
            },
        );

        let fullscreenize_btn = gui::Button::new(
            &wnd,
            gui::ButtonOpts {
                text: "&Fullscreenize",
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
        // Stores the brush used to paint the background of the labels and window
        let background_hbrush = Arc::new(Mutex::new(None));
        // The image list for the window icons
        let imagelist = Arc::new(Mutex::new(None));
        // A vector to store the icons of the windows
        let window_icons = Arc::new(Mutex::new(Vec::new()));

        let new_self = Self {
            wnd,
            label,
            process_list,
            top_toggle,
            top_label,
            btn_canvas,
            refresh_btn,
            help_btn,
            fullscreenize_btn,
            is_dark_mode,
            use_icons,
            excluded_apps,
            app_font,
            app_dpi,
            background_hbrush,
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
                        self.top_label.hwnd().SendMessage(w::msg::wm::SetFont {
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
        // Get a handle to the window and process list
        let wnd = self.wnd.hwnd();
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
        self.top_toggle
            .hwnd()
            .SetWindowTheme("DarkMode_Explorer", None)
            .map_err(|e| eprintln!("SetWindowTheme on top toggle failed: {e}"))
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
            SIZE::with(20 * dpi / 120, 20 * dpi / 120),
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
                                unsafe {
                                    HICON::from_ptr(hwnd.GetClassLongPtr(co::GCLP::HICON) as *mut _)
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
                            eprintln!("AddIcon failed: '{e}'",);
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
                    image_list
                        .AddIcon(icon)
                        .map_err(|e| {
                            eprintln!("AddIcon failed {e}\n");
                        })
                        .ok();
                }
            }
        }

        // Set the image list for the listview
        let _ = unsafe {
            self.process_list
                .hwnd()
                .SendMessage(w::msg::lvm::SetImageList {
                    himagelist: if self.use_icons.load(Ordering::SeqCst) {
                        Some(image_list.raw_copy())
                    } else {
                        None
                    },
                    kind: co::LVSIL::SMALL,
                })
        };

        // Store the image list, dropping the old one
        if let Ok(mut imagelist) = self.imagelist.lock() {
            imagelist.replace(image_list);
        }

        Ok(())
    }

    fn toggle_label_focus_rectangle(&self) -> Result<(), String> {
        // Get the rectangle of the checkbox label relative to the window's client area
        let ctrl_rect = match self.top_label.hwnd().GetWindowRect() {
            Ok(rect) => match self
                .wnd
                .hwnd()
                .ScreenToClient(POINT::with(rect.left, rect.top))
            {
                // Expand the rectangle slightly to make it more visible
                Ok(pt) => RECT {
                    left: pt.x - 2,
                    top: pt.y - 1,
                    right: pt.x + (rect.right - rect.left) + 1,
                    bottom: pt.y + (rect.bottom - rect.top) + 1,
                },
                Err(e) => {
                    eprintln!("ScreenToClient failed: {e}");
                    return Err(format!("ScreenToClient failed: {e}").to_owned());
                }
            },
            Err(e) => {
                eprintln!("GetWindowRect failed: {e}");
                return Err(format!("GetWindowRect failed: {e}").to_owned());
            }
        };

        // Draw a focus rectangle around the checkbox label
        // The focus rectangle does not draw over controls, so space is left between the checkbox and the label
        self.wnd
            .hwnd()
            .GetDC()
            .unwrap()
            .DrawFocusRect(ctrl_rect)
            .map_err(|e| format!("DrawFocusRect failed: {e}"))?;

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

                // Send a message to handle post-creation tasks
                unsafe {
                    self2
                        .wnd
                        .hwnd()
                        .PostMessage(w::msg::WndMsg::new(
                            co::WM::APP,
                            co::WM::USER.raw() as usize,
                            0,
                        ))
                        .map_err(|e| {
                            eprintln!("Failed to post WM_APP message - PostMessage Failed: {e}");
                        })
                        .ok();
                };

                // Call the default window procedure
                unsafe { self2.wnd.hwnd().DefWindowProc(create) };

                Ok(0)
            }
        });

        // Handle post-creation tasks
        self.wnd.on().wm(co::WM::APP,{
            let self2 = self.clone();
            move |msg| {
                if msg.wparam == co::WM::USER.raw() as usize {
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
                }

                Ok(0)
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

                let top_label_focused = w::HWND::GetFocus().map_or_else(|| false, |hwnd| {
                    &hwnd == self2.top_toggle.hwnd()
                });

                if top_label_focused {
                    // If the checkbox label has focus, draw the focus rectangle again
                    // The focus rectangle is drawn using XOR, so drawing it again will erase the previous one
                    self2.toggle_label_focus_rectangle().map_err(|e| {
                        eprintln!("Failed to erase stay on to toggle's label focus rectangle: {e}");
                    }).ok();
                }

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

                // Resize and move the checkbox
                self2
                    .top_toggle
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::with(
                            (8 * app_dpi / 120) as i32,
                            (31 * app_dpi / 120) as i32
                                + ((new_size.bottom - new_size.top)
                                    - ((29 + 20 + 20 + 33) * app_dpi / 120) as i32),
                        ),
                        SIZE::with((20 * app_dpi / 120) as i32, (20 * app_dpi / 120) as i32),
                        SWP::NOZORDER,
                    )
                    .map_err(|e| eprintln!("Failed to resize checkbox - SetWindowPos Failed: {e}"))
                    .ok();

                // Resize and move the label for the checkbox
                self2
                    .top_label
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::with(
                            // Leave a small gap between the checkbox and the label for the selection box
                            ((8 + 22) * app_dpi / 120) as i32,
                            (31 * app_dpi / 120) as i32
                                + ((new_size.bottom - new_size.top)
                                    - ((29 + 20 + 20 + 33) * app_dpi / 120) as i32),
                        ),
                        SIZE::with((338 * app_dpi / 120) as i32, (20 * app_dpi / 120) as i32),
                        // Don't use the SWP::NOZORDER flag, otherwise the previous frame of the listview may be visible
                        SWP::default(),
                    )
                    .map_err(|e| {
                        eprintln!("Failed to resize label for checkbox - SetWindowPos Failed: {e}")
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

                // Check if the checkbox has focus
                if let Some(hwnd) = w::HWND::GetFocus() && &hwnd == self2.top_toggle.hwnd() {
                    // Redraw the focus rectangle around the checkbox at its new position
                    self2.toggle_label_focus_rectangle().map_err(|e| {
                        eprintln!("Failed to draw focus rectangle around stay on top toggle's label: {e}");
                    }).ok();
                }

                Ok(())
            }
        });

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

                // Set the background color of the label by returning a handle to a brush
                Ok(match self2.background_hbrush.lock() {
                    Ok(mut background_hbrush) => {
                        // Create the brush if it does not exist
                        if background_hbrush.is_none() {
                            HBRUSH::CreateSolidBrush(color).map_or_else(
                                |e| {
                                    eprintln!("CreateSolidBrush failed: {e}");
                                },
                                |hbrush| {
                                    // Set the brush in the Arc Mutex
                                    *background_hbrush = Some(hbrush);
                                },
                            )
                        }

                        // Return a handle to the brush, if it exists
                        background_hbrush
                            .as_ref()
                            .map_or_else(|| HBRUSH::NULL, |hbrush| unsafe { hbrush.raw_copy() })
                    }
                    Err(e) => {
                        eprintln!("Failed to lock background brush mutex: {e}");
                        HBRUSH::NULL
                    }
                })
            }
        });

        self.wnd.on().wm_erase_bkgnd({
            let self2 = self.clone();
            move |erase_bkgnd| -> w::AnyResult<i32> {
                // Set the background color of the window in dark mode
                if self2.is_dark_mode.load(Ordering::Relaxed) {
                    match self2.background_hbrush.lock() {
                        Ok(mut background_hbrush) => {
                            // Create the brush if it does not exist
                            if background_hbrush.is_none() {
                                HBRUSH::CreateSolidBrush(COLORREF::from_rgb(0x1E, 0x1E, 0x1E))
                                    .map_or_else(
                                        |e| {
                                            eprintln!("CreateSolidBrush failed: {e}");
                                        },
                                        |hbrush| {
                                            // Set the brush in the Arc Mutex
                                            *background_hbrush = Some(hbrush);
                                        },
                                    )
                            }

                            // If the brush exists, use it to paint the window background
                            if let Some(hbrush) = background_hbrush.as_ref() {
                                match self2.wnd.hwnd().GetClientRect() {
                                    Ok(rect) => {
                                        // Set the background color of the window
                                        erase_bkgnd
                                            .hdc
                                            .FillRect(rect, hbrush)
                                            .map_err(|e| eprintln!("FillRect failed: {e}"))
                                            .ok();

                                        return Ok(1);
                                    }
                                    Err(e) => {
                                        eprintln!("GetClientRect failed: {e}");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to lock background brush mutex: {e}");
                        }
                    }
                }

                // If not in dark mode, or if an error occurred, call the default window procedure
                // This will paint the window background with the default system color
                unsafe { self2.wnd.hwnd().DefWindowProc(erase_bkgnd) };

                Ok(0)
            }
        });

        self.process_list.on_subclass().wm_kill_focus({
            let self2 = self.clone();
            move |hwnd| {
                unsafe { self2.process_list.hwnd().DefSubclassProc(hwnd) };
                // For some reason, the selected listviewitem's icon is not redrawn when the window loses focus
                // This is causes the icon's background to remain the focused selection's blue color
                // Forcing a redraw of the selected listviewitem fixes this
                if self2.process_list.hwnd().IsWindowVisible() {
                    self2
                        .process_list
                        .hwnd()
                        .InvalidateRect(None, true)
                        .map_err(|e| {
                            eprintln!("Failed to trigger a paint of the process list - InvalidateRect Failed: {e}")
                        })
                        .ok();
                }

                Ok(())
            }
        });

        self.process_list.on_subclass().wm_nc_calc_size({
            let self2 = self.clone();
            move |calc_size| {
                // Hide the process list's horizontal scrollbar
                // The scrollbar would otherwise appear since the process list's column is wider than the listview
                // Performing this in the WM_NCCALCSIZE handler prevents the scrollbar from flickering
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

                Ok(unsafe { self2.process_list.hwnd().DefWindowProc(calc_size) })
            }
        });

        self.top_toggle.on_subclass().wm_set_focus({
            let self2 = self.clone();
            move |_| {
                // Draw a focus rectangle around the checkbox label
                self2
                    .toggle_label_focus_rectangle()
                    .map_err(|e| {
                        eprintln!("Failed to draw focus rectangle on checkbox label: {e}");
                    })
                    .ok();

                Ok(())
            }
        });

        self.top_toggle.on_subclass().wm_kill_focus({
            let self2 = self.clone();
            move |hwnd| {
                unsafe { self2.wnd.hwnd().DefSubclassProc(hwnd) };
                // Erase the focus rectangle around the checkbox label
                // The focus rectangle is drawn using XOR, so drawing it again will erase it
                self2
                    .toggle_label_focus_rectangle()
                    .map_err(|e| {
                        eprintln!("Failed to erase focus rectangle on checkbox label: {e}");
                    })
                    .ok();

                Ok(())
            }
        });

        // Toggle the checkbox state when the label is clicked
        self.top_label.on().stn_clicked({
            let self2 = self.clone();
            move || {
                // Toggle the checkbox state
                self2.top_toggle.trigger_click();

                Ok(())
            }
        });

        // Double-clicking the label fires a separate event, so handle that too
        self.top_label.on().stn_dbl_clk({
            let self2 = self.clone();
            move || {
                // Toggle the checkbox state
                self2.top_toggle.trigger_click();

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
                match AdjustWindowRectExForDpi(rect, window.style(), false, window.style_ex(), window.GetDpiForWindow()) {
                    Ok(rct) => rect = rct,
                    Err(e) => {
                        show_error_message(&format!("Failed to fullscreenize window - AdjustWindowRectExForDpi failed with error: {e}"));
                        return Ok(());
                    }
                }

                // Set window to stay on top if checkbox is checked
                if self2.top_toggle.is_checked() {
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
