extern crate alloc;

use crate::custom_button::CustomButton;

use windows::Win32::Foundation::{BOOL, HWND};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};

use winsafe::co::COLOR;
use winsafe::gui::Brush;
use winsafe::prelude::{
    gdi_Hbrush, gdi_Hdc, user_Hwnd, GuiParent, GuiParentPopup, GuiWindow, Handle,
};
use winsafe::{self as w, co, gui, HwndPlace, COLORREF, HBRUSH, POINT, SIZE};

// Create an implementation of a custom dialog
#[derive(Clone)]
pub(crate) struct CustomDialog {
    wnd: gui::WindowMain,
    label: gui::Label,
    ok_btn: CustomButton,
}

impl CustomDialog {
    pub fn new(title: &str, text: &str) -> Self {
        let wnd = gui::WindowMain::new(gui::WindowMainOpts {
            title: title.to_owned(),
            class_icon: gui::Icon::Id(101),
            size: (400, 300),
            style: co::WS::POPUP | co::WS::CAPTION | co::WS::SYSMENU | co::WS::SIZEBOX,
            class_bg_brush: Brush::Color(COLOR::C3DDKSHADOW),
            ..Default::default()
        });

        let label = gui::Label::new(
            &wnd,
            gui::LabelOpts {
                text: text.to_owned(),
                position: (10, 10),
                size: (380, 250),
                label_style: co::SS::CENTER,
                window_style: co::WS::CHILD | co::WS::VISIBLE,
                window_ex_style: co::WS_EX::LEFT,
                ctrl_id: 10000,
                resize_behavior: (gui::Horz::Resize, gui::Vert::Resize),
            },
        );

        let ok_btn = CustomButton::new(
            &wnd,
            (162, 270),
            (75, 20),
            "OK",
        );

        let mut new_self = Self {
            wnd,
            label,
            ok_btn,
        };

        new_self.events();
        new_self
    }

    pub fn run(&self) -> w::AnyResult<i32> {
        self.wnd.run_main(None)
    }

    fn events(&mut self) {
        self.wnd.on().wm_create({
            let self2 = self.clone();
            move |create| {
                #[cfg(debug_assertions)]
                println!("Dialog: WM_CREATE");

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

                // Call the default window procedure
                Ok(unsafe { self2.wnd.hwnd().DefWindowProc(create) })
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

        self.wnd.on().wm_ctl_color_static({
            move |ctl| {
                // Set the text color of the label
                ctl.hdc.SetTextColor(COLORREF::new(0xE0, 0xE0, 0xE0))?;

                // Set the background of the label to transparent
                ctl.hdc.SetBkMode(co::BKMODE::TRANSPARENT)?;

                // TODO: Only create the brush once
                Ok(HBRUSH::CreateSolidBrush(COLORREF::new(0x1E, 0x1E, 0x1E))?.leak())
            }
        });

        // Handle resizing the window
        self.wnd.on().wm_size({
            let self2 = self.clone();
            move |size| {
                if size.request == co::SIZE_R::RESTORED {
                    // Clear and update the window
                    self2.wnd.hwnd().InvalidateRect(None, true)?;

                    // Determine the new size of the OK button
                    let btn_size: SIZE = if size.client_area.cx >= 381 {
                        SIZE::new(110, 33)
                    } else {
                        SIZE::new((size.client_area.cx / 3) - 16, 33)
                    };

                    // Center and resize the OK button
                    self2
                        .ok_btn
                        .hwnd()
                        .SetWindowPos(
                            HwndPlace::None,
                            POINT::new(
                                (size.client_area.cx / 2) - (btn_size.cx / 2),
                                size.client_area.cy - 40,
                            ),
                            btn_size,
                            co::SWP::NOZORDER,
                        )
                        .map_err(|e| {
                            eprintln!("Failed to center and resize OK button - SetWindowPos Failed: {}", e)
                        })
                        .ok();
                }

                // Call the default window procedure
                unsafe { self2.wnd.hwnd().DefWindowProc(size) };

                Ok(())
            }
        });

        self.ok_btn.on_click({
            let self2 = self.clone();
            move |_| {
                // Close the dialog
                self2.wnd.close();

                Ok(())
            }
        });
    }
}
