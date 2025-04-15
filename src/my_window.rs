extern crate alloc;

use crate::custom_button::CustomButton;
use crate::custom_dialog::CustomDialog;

use windows::Win32::Foundation::{BOOL, HWND};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::UI::WindowsAndMessaging::{SetClassLongPtrW, GCLP_HBRBACKGROUND};

use winsafe::co::COLOR;
use winsafe::gui::Brush;
use winsafe::prelude::{advapi_Hkey, gdi_Hbrush, gdi_Hdc, user_Hwnd, GuiParent, GuiWindow, Handle};
use winsafe::{self as w, co, gui, HwndPlace, COLORREF, HBRUSH, POINT, SIZE};

#[derive(Clone)]
pub struct MyWindow {
    wnd: gui::WindowMain,
    /*label: gui::Label,
    process_list: gui::ListView,
    top_toggle: gui::ListView,*/
    refresh_btn: CustomButton,
    help_btn: CustomButton,
    fullscreenize_btn: CustomButton,
}

impl MyWindow {
    pub fn new() -> Self {
        let wnd = gui::WindowMain::new(gui::WindowMainOpts {
            title: "Fullscreenizer".to_owned(),
            class_icon: gui::Icon::Id(101),
            size: (305, 400),
            style: gui::WindowMainOpts::default().style
                | co::WS::OVERLAPPEDWINDOW
                | co::WS::SIZEBOX, // Window can be resized
            // Set the background color
            class_bg_brush: Brush::Color(COLOR::C3DDKSHADOW), // TODO: Port to program
            ..Default::default()
        });

        /*let label = gui::Label::new(
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
        );*/

        let refresh_btn = CustomButton::new(
            &wnd,
            (13, 368),
            (90, 26),
            "Refresh",
        );

        let help_btn = CustomButton::new(
            &wnd,
            (108, 368),
            (90, 26),
            "Help",
        );

        let fullscreenize_btn = CustomButton::new(
            &wnd,
            (203, 368),
            (90, 26),
            "Fullscreenize",
        );

        let mut new_self = Self {
            wnd,
            /*label,
            process_list,
            top_toggle,*/
            refresh_btn,
            help_btn,
            fullscreenize_btn,
        };

        new_self.events();
        new_self
    }

    pub fn run(&self) -> w::AnyResult<i32> {
        self.wnd.run_main(None)
    }

    fn events(&mut self) {
        // Window events //

        self.wnd.on().wm_create({
            let self2 = self.clone();
            move |create| {
                #[cfg(debug_assertions)]
                println!("WM_CREATE");

                // Set the title bar to dark mode
                unsafe {
                    DwmSetWindowAttribute(
                        HWND(self2.wnd.hwnd().ptr()),
                        DWMWA_USE_IMMERSIVE_DARK_MODE,
                        &1 as *const _ as *const core::ffi::c_void, // TODO: Cast more 'safely'
                        size_of::<BOOL>() as u32,
                    )
                }
                .map_err(|e| eprintln!("DwmSetWindowAttribute failed: {}", e))
                .ok();

                // Set the background color of the window by setting GCLP_HBRBACKGROUND
                unsafe {
                    SetClassLongPtrW(
                        HWND(self2.wnd.hwnd().ptr()),
                        GCLP_HBRBACKGROUND,
                        COLORREF::new(0x1E, 0x1E, 0x1E).raw() as isize,
                    )
                };

                // Call the default window procedure
                unsafe { self2.wnd.hwnd().DefWindowProc(create) };

                Ok(0)
            }
        });

        self.wnd.on().wm_erase_bkgnd({
            let self2 = self.clone();
            move |erase_bkgnd| {
                // Paint a custom background color
                erase_bkgnd.hdc.FillRect(
                    self2.wnd.hwnd().GetClientRect()?,
                    &HBRUSH::CreateSolidBrush(COLORREF::new(0x1E, 0x1E, 0x1E))?.leak(),
                )?;

                Ok(1)
            }
        });

        self.wnd.on().wm_size({
            let self2 = self.clone();
            move |size| {
                // If the window is being resized
                if size.request == co::SIZE_R::RESTORED {
                    // Determine the new size of each button
                    let btn_size: SIZE = if size.client_area.cx >= 381 {
                        SIZE::new(110, 33)
                    } else {
                        SIZE::new((size.client_area.cx / 3) - 16, 33)
                    };

                    // Align and resize each button
                    self2
                        .refresh_btn
                        .hwnd()
                        .SetWindowPos(
                            HwndPlace::None,
                            POINT::new(13, size.client_area.cy - 40),
                            btn_size,
                            co::SWP::NOZORDER,
                        )?;

                    self2
                        .help_btn
                        .hwnd()
                        .SetWindowPos(
                            HwndPlace::None,
                            POINT::new(
                                (size.client_area.cx / 2) - (btn_size.cx / 2),
                                size.client_area.cy - 40,
                            ),
                            btn_size,
                            co::SWP::NOZORDER,
                        )?;

                    self2
                        .fullscreenize_btn
                        .hwnd()
                        .SetWindowPos(
                            HwndPlace::None,
                            POINT::new(
                                size.client_area.cx - btn_size.cx - 13,
                                size.client_area.cy - 40,
                            ),
                            btn_size,
                            co::SWP::NOZORDER,
                        )?;

                    // Clear and update the window
                    self2.refresh_btn.hwnd().InvalidateRect(None, true)?;
                    self2.help_btn.hwnd().InvalidateRect(None, true)?;
                    self2.fullscreenize_btn.hwnd().InvalidateRect(None, true)?;
                }
                // Move and resize the elements that automatically resize
                unsafe { self2.wnd.hwnd().DefWindowProc(size) };

                Ok(())
            }
        });

        // Button events //

        self.refresh_btn.on_click({
            move |_| {
                // Refresh the list of windows
                println!("  Refresh button clicked");

                Ok(())
            }
        });

        self.help_btn.on_click({
            move |_| {
                // Create a custom dialog for the help message
                CustomDialog::new(
                    "Fullscreenizer Help", 
                    "Open the game you want to force in borderless-windowed-fullscreen mode, \
                    set it to windowed mode to the resolution you want, hit the Refresh button \
                    to refresh the windows list, select the game window from the list and press \
                    the Fullscreenize button.  The window will be resized to the desktop area and \
                    the border will be removed.  Note that using a different in-game resolution \
                    from the desktop resolution may not work properly (or at all) depending on the game.\n\n\n\
                    Made by Carter Persall\n\
                    Based on the program by Kostas \"Bad Sector\" Michalopoulos"
                ).run()?;

                Ok(())
            }
        });

        self.fullscreenize_btn.on_click({
            move |_| {
                // Fullscreenize the selected window
                println!("  Fullscreenize button clicked");

                Ok(())
            }
        });
    }
}


// Helper Functions //

fn get_system_theme() -> bool {
    // Check if dark mode is enabled using the registry
    w::HKEY::CURRENT_USER
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
        )
}
