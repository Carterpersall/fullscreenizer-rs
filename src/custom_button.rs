extern crate alloc;

use alloc::rc::Rc;
use core::cell::RefCell;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::UI::Controls::{
    BeginBufferedAnimation, BufferedPaintInit, BufferedPaintRenderAnimation,
    BufferedPaintStopAllAnimations, EndBufferedAnimation, BPAS_LINEAR, BPBF_COMPATIBLEBITMAP,
    BP_ANIMATIONPARAMS,
};

use winsafe::co::COLOR;
use winsafe::gui::Brush;
use winsafe::msg::wm::{Paint, SetFont};
use winsafe::prelude::{gdi_Hbrush, gdi_Hdc, gdi_Hfont, user_Hdc, user_Hwnd, GuiParent, GuiWindow, Handle};
use winsafe::{self as w, co, gui, PtInRect, SysResult, TrackMouseEvent, COLORREF, HFONT, SIZE, TRACKMOUSEEVENT};
use winsafe::guard::DeleteObjectGuard;

macro_rules! paint {
    ($self:ident, $hdc:ident, $rect:ident, $bk_color:expr, $text_color:expr, $parent_color:expr) => {
        // Clear the button's current state
        $hdc.FillRect($rect, &w::HBRUSH::CreateSolidBrush($parent_color)?.leak())?;

        // Define and select a new brush
        let brush = w::HBRUSH::CreateSolidBrush($bk_color)?;
        let _old_brush = $hdc.SelectObject(&*brush)?;

        // Draw the button with rounded corners
        $hdc.RoundRect($rect, SIZE::new(15, 15))?;

        // Make the background of the text transparent
        $hdc.SetBkMode(co::BKMODE::TRANSPARENT)?;
        // Set the text color
        $hdc.SetTextColor($text_color)?;

        // Draw the text
        $hdc.DrawText(
            $self.text.as_str(),
            $rect,
            co::DT::CENTER | co::DT::VCENTER | co::DT::SINGLELINE | co::DT::INTERNAL,
        )?;
    };
}

// Create an implementation of a custom button control
#[derive(Clone)]
pub(crate) struct CustomButton {
    btn: gui::WindowControl,
    text: String,
    fn_click: Rc<RefCell<Option<Box<dyn Fn(usize) -> w::AnyResult<()> + 'static>>>>,
    hovering: Rc<RefCell<bool>>,
    hover_animation: Rc<RefCell<bool>>,
}

static mut _OLD_FONT: Option<w::guard::SelectObjectGuard<w::HDC, HFONT>> = None;

impl CustomButton {
    pub fn new(
        parent: &impl GuiParent,
        position: (i32, i32),
        size: (u32, u32),
        text: &str,
    ) -> Self {
        let wnd = gui::WindowControl::new(
            parent,
            gui::WindowControlOpts {
                position,
                size,
                class_bg_brush: Brush::Color(COLOR::C3DSHADOW),
                style: co::WS::CHILD | co::WS::VISIBLE | co::WS::TABSTOP,
                ..Default::default()
            },
        );

        let new_self = Self {
            btn: wnd,
            text: text.to_owned(),
            fn_click: Rc::new(RefCell::new(None)),
            hovering: Rc::new(RefCell::new(false)),
            hover_animation: Rc::new(RefCell::new(false)),
        };

        // Inform the Windows API that the control will be using buffered painting
        unsafe { BufferedPaintInit() }
            .map_err(|err| {
                eprintln!("BufferedPaintInit failed: {}", err);

                // This should disable the hover animation code
                *new_self.hover_animation.borrow_mut() = false;
            })
            .ok();

        new_self.events();
        new_self
    }

    pub fn on_click<F>(&mut self, func: F)
		where F: Fn(usize) -> w::AnyResult<()> + 'static,
	{
		*self.fn_click.borrow_mut() = Some(Box::new(func)); // store user callback
	}

    pub fn hwnd(&self) -> &w::HWND {
        self.btn.hwnd()
    }

    fn events(&self) {
        /*self.btn.on().wm_create({
            let self2 = self.clone();
            move |create| {
                // Change the font of the label to a smaller one
                match w::HFONT::CreateFont(
                    SIZE { cx: 0, cy: 17 },
                    0,
                    0,
                    co::FW::MEDIUM,
                    true,
                    true,
                    true,
                    co::CHARSET::DEFAULT,
                    co::OUT_PRECIS::DEFAULT,
                    co::CLIP::DEFAULT_PRECIS,
                    co::QUALITY::DRAFT,
                    co::PITCH::DEFAULT,
                    "Arial",
                ) {
                    Ok(mut hfont) => {
                        // Send the WM_SETFONT message to the button
                        unsafe { self2.btn.hwnd().SendMessage(SetFont { hfont: hfont.leak(), redraw: true }) };
                        
                        // Select the font into the device context
                        //let hdc = self2.btn.hwnd().GetDC()?;
                        //unsafe { _OLD_FONT = Some(hdc.SelectObject(&hfont.raw_copy())?) };
                    }
                    Err(e) => eprintln!("Failed to create font - CreateFont failed: {}", e),
                }

                // Run the default event handler
                unsafe { self2.btn.hwnd().DefWindowProc(create) };

                Ok(0)
            }
        });*/

        /*self.btn.on().wm_set_font({
            let self2 = self.clone();
            move |set_font| {
                let hdc = self2.btn.hwnd().GetDC()?;
                
                // Select the font into the device context
                unsafe { _OLD_FONT = Some(hdc.SelectObject(&set_font.hfont)?) };

                // Run the default event handler
                unsafe { self2.btn.hwnd().DefWindowProc(set_font) };

                Ok(())
            }
        });*/

        // Paint the button's background and text
        self.btn.on().wm_paint({
            let self2 = self.clone();
            move || {
                // Get the HDC of the button, and begin painting
                let hdc = self2.btn.hwnd().BeginPaint()?;

                // The hover animation takes care of drawing the button's background
                if !unsafe {
                    BufferedPaintRenderAnimation(
                        HWND(self2.btn.hwnd().ptr()),
                        HDC(self2.btn.hwnd().GetDC()?.leak().ptr()),
                    )
                }
                .as_bool() {
                    // Get the button's client area
                    let rect = self2.btn.hwnd().GetClientRect()?;

                    // Paint the button's background and text
                    paint!(
                        self2,
                        hdc,
                        rect,
                        COLORREF::new(0x33, 0x33, 0x33),
                        COLORREF::new(0xE0, 0xE0, 0xE0),
                        COLORREF::new(0x1E, 0x1E, 0x1E)
                    );

                    // Run the default event handler
                    unsafe { self2.hwnd().DefWindowProc(Paint {}) };
                }

                Ok(())
            }
        });

        self.btn.on().wm_l_button_down({
            let self2 = self.clone();
            move |_| {
                // Call the user callback
                if let Some(ref fn_click) = *self2.fn_click.borrow() {
                    fn_click(0)?;
                }

                Ok(())
            }
        });

        self.btn.on().wm_mouse_move({
            let self2 = self.clone();
            move |mouse| {
                // Get the button's client area
                let rect = self2.btn.hwnd().GetClientRect()?;

                // TODO: Remove after determining if the check is necessary
                #[cfg(debug_assertions)]
                {
                    if !PtInRect(rect, mouse.coords) {
                        println!("              Button: WM_MOUSEMOVE outside client area");
                    }
                }

                // TODO: Determine if this check is necessary
                // Check if the mouse is within the button's client area
                if !*self2.hovering.borrow() && PtInRect(rect, mouse.coords) {
                    // Register the WM_LEAVE and WM_HOVER events
                    let mut tme = TRACKMOUSEEVENT::default();
                    // TODO: Why do I have to do this?
                    tme.hwndTrack = unsafe { w::HWND::from_ptr(self2.btn.hwnd().ptr()) };
                    tme.dwFlags = co::TME::LEAVE | co::TME::HOVER;
                    tme.dwHoverTime = 0xFFFF_FFFF; // HOVER_DEFAULT
                    TrackMouseEvent(&mut tme)?;

                    // We are now hovering over the button
                    *self2.hovering.borrow_mut() = true;
                }

                Ok(())
            }
        });

        self.btn.on().wm_mouse_hover({
            let self2 = self.clone();
            move |_| {
                // Is the hover animation already active or disabled?
                if !*self2.hover_animation.borrow() {
                    // Begin the hover animation
                    *self2.hover_animation.borrow_mut() = true;

                    let anim_params = BP_ANIMATIONPARAMS {
                        cbSize: size_of::<BP_ANIMATIONPARAMS>() as u32,
                        style: BPAS_LINEAR,
                        dwDuration: 1000,
                        ..Default::default()
                    };
                    let mut hdcfrom = HDC::default();
                    let mut hdcto = HDC::default();
                    let rect = self2.btn.hwnd().GetClientRect()?;
                    let buffer = unsafe {
                        BeginBufferedAnimation(
                            HWND(self2.btn.hwnd().ptr()),
                            HDC(self2.btn.hwnd().GetDC()?.leak().ptr()),
                            &RECT {
                                left: rect.left,
                                top: rect.top,
                                right: rect.right,
                                bottom: rect.bottom,
                            },
                            BPBF_COMPATIBLEBITMAP,
                            None,
                            &anim_params,
                            &mut hdcfrom,
                            &mut hdcto,
                        )
                    };

                    // TODO: What is buffer set to if BeginBufferedAnimation fails?
                    if buffer != 0 {
                        // The initial frame of the animation
                        if !hdcfrom.is_invalid() {
                            // Return to safety by converting the HDC to the WinSafe type
                            let hdc = unsafe { w::HDC::from_ptr(hdcfrom.0) };

                            // Paint the button's background and text
                            paint!(
                                self2,
                                hdc,
                                rect,
                                COLORREF::new(0x33, 0x33, 0x33),
                                COLORREF::new(0xE0, 0xE0, 0xE0),
                                COLORREF::new(0x1E, 0x1E, 0x1E)
                            );
                        }

                        // The final frame of the animation
                        if !hdcto.is_invalid() {
                            // Return to safety by converting the HDC to the WinSafe type
                            let hdc = unsafe { w::HDC::from_ptr(hdcto.0) };

                            // Paint the button's background and text
                            paint!(
                                self2,
                                hdc,
                                rect,
                                COLORREF::new(0x45, 0x45, 0x45),
                                COLORREF::new(0xE0, 0xE0, 0xE0),
                                COLORREF::new(0x1E, 0x1E, 0x1E)
                            );
                        }

                        // Start the animation
                        unsafe { EndBufferedAnimation(buffer, true) }?;
                    }
                }

                Ok(())
            }
        });

        self.btn.on().wm_mouse_leave({
            let self2 = self.clone();
            move || {
                // End the hover animation, if it is active
                if *self2.hover_animation.borrow() {
                    *self2.hover_animation.borrow_mut() = false;
                    unsafe { BufferedPaintStopAllAnimations(HWND(self2.btn.hwnd().ptr())) }?;

                    // Trigger a repaint of the button
                    self2.btn.hwnd().InvalidateRect(None, true)?;
                }

                // We are no longer hovering over the button
                *self2.hovering.borrow_mut() = false;

                Ok(())
            }
        });
    }
}
