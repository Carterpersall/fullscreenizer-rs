extern crate alloc;

use alloc::sync::Arc;
use core::ffi::c_void;
use core::mem::size_of;
use std::ops::Shr;
use std::sync::{Mutex, MutexGuard};

use windows::core::BOOL;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::UI::WindowsAndMessaging::{
    SetClassLongPtrW, GCLP_HBRBACKGROUND, ICON_SMALL, WS_POPUP, WS_VISIBLE,
};

use winsafe::co::{COLOR, SWP};
use winsafe::guard::ImageListDestroyGuard;
use winsafe::msg::lvm::{SetBkColor, SetTextBkColor, SetTextColor};
use winsafe::msg::wm::Paint;
use winsafe::prelude::{
    advapi_Hkey, comctl_Himagelist, comctl_Hwnd, gdi_Hbrush, gdi_Hdc, gdi_Hfont, user_Hmonitor,
    user_Hwnd, uxtheme_Hwnd, GuiNativeControl, GuiNativeControlEvents, GuiParent, GuiWindow,
    Handle,
};
use winsafe::{
    self as w, co, gui, AdjustWindowRectEx, EnumWindows, GetLastError, HwndPlace, COLORREF, HBRUSH,
    HICON, HIMAGELIST, POINT, RECT, SIZE,
};

#[derive(Clone)]
pub struct MyWindow {
    wnd: gui::WindowMain,
    label: gui::Label,
    process_list: gui::ListView,
    top_toggle: gui::ListView,
    refresh_btn: gui::Button,
    help_btn: gui::Button,
    fullscreenize_btn: gui::Button,
    is_dark_mode: Arc<Mutex<bool>>,
}

impl MyWindow {
    pub fn new() -> Self {
        let wnd = gui::WindowMain::new(gui::WindowMainOpts {
            title: "Fullscreenizer".to_owned(),
            class_icon: gui::Icon::Id(101),
            size: (305, 400),
            style: gui::WindowMainOpts::default().style
                | co::WS::OVERLAPPEDWINDOW
                | co::WS::SIZEBOX, // window can be resized
            ..Default::default()
        });

        let label = gui::Label::new(
            &wnd,
            gui::LabelOpts {
                text: "Toplevel windows:".to_string(),
                position: (10, 9),
                size: (200, 15),
                label_style: co::SS::LEFTNOWORDWRAP,
                window_style: co::WS::CHILD | co::WS::VISIBLE,
                window_ex_style: co::WS_EX::LEFT,
                ctrl_id: 10000,
                resize_behavior: (gui::Horz::Resize, gui::Vert::Repos),
            },
        );

        let process_list = gui::ListView::new(
            &wnd,
            gui::ListViewOpts {
                position: (8, 24),
                size: (289, 312),
                columns: vec![("".to_owned(), 999)],
                list_view_style: co::LVS::NOSORTHEADER
                    | co::LVS::SHOWSELALWAYS
                    | co::LVS::NOCOLUMNHEADER
                    | co::LVS::NOLABELWRAP
                    | co::LVS::SINGLESEL
                    | co::LVS::REPORT
                    | co::LVS::SHAREIMAGELISTS,
                list_view_ex_style: co::LVS_EX::DOUBLEBUFFER | co::LVS_EX::AUTOSIZECOLUMNS,
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
                position: (2, 342),
                size: (300, 20),
                columns: vec![("".to_owned(), 999)],
                list_view_style: co::LVS::NOSORTHEADER
                    | co::LVS::SHOWSELALWAYS
                    | co::LVS::NOCOLUMNHEADER
                    | co::LVS::NOLABELWRAP
                    | co::LVS::SINGLESEL
                    | co::LVS::REPORT
                    | co::LVS::NOSCROLL
                    | co::LVS::SHAREIMAGELISTS,
                list_view_ex_style: co::LVS_EX::DOUBLEBUFFER
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
                position: (13, 368),
                // Move anchored at right/bottom as parent window resizes.
                resize_behavior: (gui::Horz::None, gui::Vert::Repos),
                window_ex_style: co::WS_EX::LAYERED,
                ..Default::default()
            },
        );

        let help_btn = gui::Button::new(
            &wnd,
            gui::ButtonOpts {
                text: "&Help".to_owned(),
                position: (108, 368),
                // Move anchored at right/bottom as parent window resizes.
                resize_behavior: (gui::Horz::None, gui::Vert::Repos),
                window_ex_style: co::WS_EX::LAYERED,
                ..Default::default()
            },
        );

        let fullscreenize_btn = gui::Button::new(
            &wnd,
            gui::ButtonOpts {
                text: "&Fullscreenize".to_owned(),
                position: (202, 368),
                // Move anchored at right/bottom as parent window resizes.
                resize_behavior: (gui::Horz::Repos, gui::Vert::Repos),
                window_ex_style: co::WS_EX::LAYERED,
                ..Default::default()
            },
        );

        let is_dark_mode = Arc::new(Mutex::new(false));

        let new_self = Self {
            wnd,
            label,
            process_list,
            top_toggle,
            refresh_btn,
            help_btn,
            fullscreenize_btn,
            is_dark_mode,
        };

        new_self.events();
        new_self
    }

    pub fn run(&self) -> w::AnyResult<i32> {
        self.wnd.run_main(None)
    }

    fn enable_dark_mode(&self) {
        // Get a handle to the window
        let wnd = self.wnd.hwnd();

        // Get a handle to the process list
        let process_list = self.process_list.hwnd();

        // Get an unsafe handle to the window
        let hwnd = HWND(wnd.ptr());

        // Enable dark mode on the window
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &1 as *const _ as *const c_void,
                size_of::<BOOL>() as u32,
            )
        }
        .map_err(|e| eprintln!("DwmSetWindowAttribute failed: {}", e))
        .ok();

        // Enable dark mode on the elements in the window
        // Calling SetWindowTheme in too quick succession can cause the theme to not be applied
        enable_dark_mode_for_element(wnd);
        enable_dark_mode_for_element(process_list);
        enable_dark_mode_for_element(self.refresh_btn.hwnd());
        enable_dark_mode_for_element(self.help_btn.hwnd());
        enable_dark_mode_for_element(self.fullscreenize_btn.hwnd());

        // Set the background color of the window
        unsafe {
            SetClassLongPtrW(
                hwnd,
                GCLP_HBRBACKGROUND,
                HBRUSH::from_sys_color(COLOR::WINDOWTEXT).ptr() as isize,
            )
        };

        // Set the background color of the listview
        unsafe {
            process_list.SendMessage(SetBkColor {
                color: Option::from(COLORREF::new(0x3C, 0x3C, 0x3C)), //0xC4, 0xC4, 0xC4)),
            })
        }
        .map_err(|e| eprintln!("SetBkColor failed: {}", e))
        .ok();

        // Set the background color of the elements in the listview
        unsafe {
            process_list.SendMessage(SetTextBkColor {
                color: Option::from(COLORREF::new(0x3C, 0x3C, 0x3C)), //0xC4, 0xC4, 0xC4)),
            })
        }
        .map_err(|e| eprintln!("WM_CTLCOLORLISTBOX failed: {}", e))
        .ok();

        // Set the text color of the elements in the listview
        unsafe {
            process_list.SendMessage(SetTextColor {
                color: Option::from(COLORREF::new(0xF0, 0xF0, 0xF0)),
            })
        }
        .map_err(|e| eprintln!("SetTextColor failed: {}", e))
        .ok();

        // Get the handle of the top toggle
        let top_toggle = self.top_toggle.hwnd();

        // Set the background color of the checkbox listview to the same as the window background
        unsafe {
            top_toggle.SendMessage(SetBkColor {
                color: Option::from(COLORREF::new(0x1E, 0x1E, 0x1E)), //0x6D, 0x6D, 0x6D)),
            })
        }
        .map_err(|e| eprintln!("SetBkColor failed: {}", e))
        .ok();

        // Set the background color of the element in the checkbox listview
        unsafe {
            top_toggle.SendMessage(SetTextBkColor {
                color: Option::from(COLORREF::new(0x1E, 0x1E, 0x1E)), //(0x6D, 0x6D, 0x6D)),
            })
        }
        .map_err(|e| eprintln!("WM_CTLCOLORLISTBOX failed: {}", e))
        .ok();

        // Set the text color of the elements in the checkbox listview
        unsafe {
            top_toggle.SendMessage(SetTextColor {
                color: Option::from(COLORREF::new(0xF0, 0xF0, 0xF0)),
            })
        }
        .map_err(|e| eprintln!("SetTextColor failed: {}", e))
        .ok();
    }

    fn set_system_theme(&self) {
        let mut is_dark_mode = match self.is_dark_mode.lock() {
            Ok(is_dark_mode) => is_dark_mode,
            Err(e) => {
                eprintln!("Failed to get dark mode status - Mutex lock failed: {}", e);
                return;
            }
        };

        // Check if dark mode is enabled using the registry
        if !is_dark_mode.to_owned() {
            *is_dark_mode = w::HKEY::CURRENT_USER
                .RegOpenKeyEx(
                    Some("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                    co::REG_OPTION::default(),
                    co::KEY::READ,
                )
                .and_then(|key| key.RegQueryValueEx(Some("AppsUseLightTheme")))
                .map_or_else(
                    |e| {
                        eprintln!("Getting the system theme failed: {}", e);
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
        }
        if is_dark_mode.to_owned() {
            // Enable dark mode on the window
            self.enable_dark_mode();
        } else {
            // Set the background of the label to the same as the window background
            unsafe {
                self.label.hwnd().SendMessage(SetBkColor {
                    color: Option::from(COLORREF::new(0xF0, 0xF0, 0xF0)),
                })
            }
            .map_err(|e| eprintln!("SetBkColor failed: {}", e))
            .ok();

            // Set the background color of the checkbox listview to the same as the window background
            unsafe {
                self.top_toggle.hwnd().SendMessage(SetBkColor {
                    color: Option::from(COLORREF::new(0xF0, 0xF0, 0xF0)),
                })
            }
            .map_err(|e| eprintln!("SetBkColor failed: {}", e))
            .ok();

            // Set the background color of the element in the checkbox listview
            unsafe {
                self.top_toggle.hwnd().SendMessage(SetTextBkColor {
                    color: Option::from(COLORREF::new(0xF0, 0xF0, 0xF0)),
                })
            }
            .map_err(|e| eprintln!("WM_CTLCOLORLISTBOX failed: {}", e))
            .ok();

            // Set the listview to use the Explorer theme to make the item selection boxes stretch to the right edge of the window
            self.process_list
                .hwnd()
                .SetWindowTheme("Explorer", None)
                .map_err(|e| eprintln!("SetWindowTheme failed: {}", e))
                .ok();
        }
    }

    fn refresh_process_list(&self, windows: &mut MutexGuard<Vec<w::HWND>>) {
        // Clear the process list and window vector
        self.process_list.items().delete_all();
        windows.clear();

        // Whether to use icons
        let mut use_icons = true; // TODO: Make this a setting

        // Create an image list to store the icons
        let image_list = HIMAGELIST::Create(SIZE::new(16, 16), co::ILC::COLOR32, 0, 100)
            .unwrap_or_else(|e| {
                // If creating the image list failed, disable the use of icons
                use_icons = false;
                eprintln!("Imagelist Creation failed {}", e);
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

            let icon_id = if use_icons {
                // Get the window icon
                let icon = match unsafe {
                    HICON::from_ptr(hwnd.SendMessage(w::msg::WndMsg::new(
                        co::WM::GETICON,
                        ICON_SMALL as usize,
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
                Option::from(
                    image_list
                        .AddIcon(&icon) // TODO: Does this leak the icon?
                        .unwrap_or_else(|e| {
                            eprintln!("AddIcon failed {}\n", e);
                            u32::MAX
                        }),
                )
            } else {
                None
            };

            // Add the window to the vector
            windows.push(hwnd);

            // Add the window to the list
            self.process_list.items().add(&[title], icon_id, ());

            // Return true to continue enumerating
            true
        })
        .map_err(|e| eprintln!("EnumWindows failed: {}", e))
        .ok();

        // Set the image list for the listview
        let _ = self
            .process_list
            .set_image_list(co::LVSIL::SMALL, image_list);
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
                if first_paint.lock().ok().unwrap().to_owned() {
                    // TODO: Unwrap correctly
                    // TODO: Surely there is a better way to do this
                    *first_paint.lock().ok().unwrap() = false;

                    // Add text to the checkbox listview
                    self2.top_toggle.items().add(
                        &["Apply \"stay on top\" flag to avoid taskbar flickering"],
                        None,
                        (),
                    );
                }

                // Call the default window procedure
                unsafe { wnd.DefWindowProc(Paint {}) };

                Ok(())
            }
        });

        self.wnd.on().wm_create({
            let self2 = self.clone();
            move |create| -> w::AnyResult<i32> {
                // Change the font of the label to a smaller one
                match w::HFONT::CreateFont(
                    SIZE { cx: 0, cy: 17 },
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
                    "Arial",
                ) {
                    Ok(mut hfont) => {
                        unsafe {
                            self2.label.hwnd().SendMessage(w::msg::wm::SetFont {
                                hfont: hfont.leak(),
                                redraw: true,
                            })
                        };
                        drop(hfont);
                    }
                    Err(e) => eprintln!("Failed to create font - CreateFont failed: {}", e),
                }

                // Set the theme of the window
                self2.set_system_theme();

                // Refresh the process list
                self2.refresh_btn.trigger_click();

                // Call the default window procedure
                unsafe { self2.wnd.hwnd().DefWindowProc(create) };

                Ok(0)
            }
        });

        self.wnd.on().wm_size({
            let self2 = self.clone();
            move |size| -> w::AnyResult<()> {
                // Get a handle to the window
                let wnd = self2.wnd.hwnd();

                // Move and resize the elements that automatically resize
                unsafe { wnd.DefWindowProc(size) };

                // Move the label to the correct position
                self2
                    .label
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::new(10, 9),
                        SIZE::new(200, 15),
                        SWP::NOZORDER,
                    )
                    .map_err(|e| eprintln!("Failed to move label - SetWindowPos Failed: {}", e))
                    .ok();

                // Get the new window dimensions
                let new_size = match wnd.GetClientRect() {
                    Ok(size) => size,
                    Err(e) => {
                        eprintln!("Failed to get window size - GetClientRect Failed: {}", e);
                        return Ok(());
                    }
                };

                // Determine the new size of the buttons
                let btn_size: SIZE = if new_size.right - new_size.left >= 381 {
                    SIZE::new(110, 33)
                } else {
                    SIZE::new(((new_size.right - new_size.left) / 3) - 16, 33)
                };

                // Resize and center align the help button
                self2
                    .help_btn
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::new(
                            ((new_size.right - new_size.left) / 2) - (btn_size.cx / 2),
                            new_size.bottom - 40,
                        ),
                        btn_size,
                        SWP::NOZORDER,
                    )
                    .map_err(|e| {
                        eprintln!("Failed to move help button - SetWindowPos Failed: {}", e)
                    })
                    .ok();

                // Resize and align the other buttons
                self2
                    .refresh_btn
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::new(13, new_size.bottom - 40),
                        btn_size,
                        SWP::NOZORDER,
                    )
                    .map_err(|e| {
                        eprintln!("Failed to move refresh button - SetWindowPos Failed: {}", e)
                    })
                    .ok();
                self2
                    .fullscreenize_btn
                    .hwnd()
                    .SetWindowPos(
                        HwndPlace::None,
                        POINT::new(new_size.right - btn_size.cx - 13, new_size.bottom - 40),
                        btn_size,
                        SWP::NOZORDER,
                    )
                    .map_err(|e| {
                        eprintln!(
                            "Failed to move fullscreenize button - SetWindowPos Failed: {}",
                            e
                        )
                    })
                    .ok();

                Ok(())
            }
        });

        self.wnd.on().wm_ctl_color_static({
            let self2 = self.clone();
            move |ctl| -> w::AnyResult<HBRUSH> {
                // Light mode background color and dark mode text color
                let mut color = COLORREF::new(0xF0, 0xF0, 0xF0);

                if self2.is_dark_mode.lock().unwrap().to_owned() {
                    // TODO: Unwrap correctly
                    // Set the text color of the label to white
                    ctl.hdc.SetTextColor(color).unwrap(); // TODO: Unwrap correctly

                    // Set the color to the dark mode background color
                    color = COLORREF::new(0x1E, 0x1E, 0x1E); //0x6D, 0x6D, 0x6D);
                }

                // Set the background color of the label
                ctl.hdc.SetBkColor(color).unwrap(); // TODO: Unwrap correctly

                // Set the color of the rest of the element's background
                Ok(HBRUSH::CreateSolidBrush(color).unwrap().leak()) // TODO: Unwrap correctly
            }
        });

        self.top_toggle.on().nm_click({
            let self2 = self.clone();
            move |_| {
                // Disable highlighting the item by clicking on it (Selecting with the arrow keys still works)
                self2.top_toggle.items().get(0).select(false);

                Ok(())
            }
        });

        // Create a vector in a mutex to store the open windows
        let windows: Arc<Mutex<Vec<w::HWND>>> = Arc::new(Mutex::new(Vec::new()));

        self.refresh_btn.on_subclass().wm_paint({
            let self2 = self.clone();
            move || {
                // Make the undrawn area of the button transparent
                self2.refresh_btn.hwnd().SetLayeredWindowAttributes(
                    COLORREF::new(0xF0, 0xF0, 0xF0),
                    255,
                    co::LWA::COLORKEY,
                )?;

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
                        self2.refresh_process_list(&mut windows);
                    }
                    Err(e) => {
                        // Show a popup window with the error message
                        show_error_message(
                            format!("Failed to refresh process list - Mutex lock failed: {}", e)
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
                self2.help_btn.hwnd().SetLayeredWindowAttributes(
                    COLORREF::new(0xF0, 0xF0, 0xF0),
                    255,
                    co::LWA::COLORKEY,
                )?;

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
                self2.fullscreenize_btn.hwnd().SetLayeredWindowAttributes(
                    COLORREF::new(0xF0, 0xF0, 0xF0),
                    255,
                    co::LWA::COLORKEY,
                )?;

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
                        show_error_message(format!("Failed to fullscreenize window - Mutex lock failed: {}", e).as_str());
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
                    .map_err(|e| show_error_message(&format!("Failed to fullscreenize window - GetMonitorInfo failed with error: {}", e)))
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
                if unsafe { window.SetWindowLongPtr(co::GWLP::STYLE, (WS_POPUP.0 | WS_VISIBLE.0) as isize) } == 0 {
                    show_error_message(&format!("Failed to fullscreenize window - SetWindowLongPtr failed with error: {}", GetLastError()));
                    return Ok(());
                }

                // Set the window size
                match AdjustWindowRectEx(rect, window.style(), false, window.style_ex()) {
                    Ok(rct) => rect = rct, // TODO: Test this
                    Err(e) => {
                        show_error_message(&format!("Failed to fullscreenize window - AdjustWindowRectEx failed with error: {}", e));
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
                        POINT::new(rect.left, rect.top),
                        SIZE::new(rect.right - rect.left, rect.bottom - rect.top),
                        true,
                    )
                    .map_err(|e| show_error_message(&format!("Failed to fullscreenize window - MoveWindow failed with error: {}", e)))
                    .ok();

                Ok(())
            }
        })
    }
}

fn enable_dark_mode_for_element(element: &w::HWND) {
    // Enable dark mode on the element
    element
        .SetWindowTheme("DarkMode_Explorer", None)
        .map_err(|e| eprintln!("SetWindowTheme failed: {}", e))
        .ok();
}

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
        .map_err(|e| eprintln!("TaskDialog failed: {}", e))
        .ok();
}

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
        .map_err(|e| eprintln!("TaskDialog failed: {}", e))
        .ok();
}
