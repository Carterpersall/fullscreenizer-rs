#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Set clippy to restrictive
/*#![warn(clippy::restriction)]
#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::absolute_paths)]
#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]
#![allow(clippy::min_ident_chars)]
#![allow(clippy::allow_attributes_without_reason)]
#![allow(clippy::blanket_clippy_restriction_lints)]
#![allow(clippy::shadow_unrelated)]
#![allow(clippy::single_call_fn)]
#![allow(clippy::default_numeric_fallback)]
#![allow(clippy::implicit_return)]
#![allow(clippy::semicolon_inside_block)]
#![allow(clippy::question_mark_used)]
#![allow(clippy::pub_with_shorthand)]
#![allow(clippy::ref_patterns)]
#![allow(clippy::integer_division)]
#![allow(clippy::integer_division_remainder_used)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::undocumented_unsafe_blocks)]*/

mod custom_button;
mod custom_dialog;
mod my_window;

use my_window::MyWindow;

use winsafe::prelude::{comctl_Hwnd, Handle};
use winsafe::{self as w, co};

fn main() {
    if let Err(e) = MyWindow::new().run() {
        w::HWND::NULL
            .TaskDialog(
                Some("Unhandled error"),
                None,
                Some(&e.to_string()),
                co::TDCBF::OK,
                w::IconRes::Error,
            )
            .map_err(|e| eprintln!("TaskDialog failed, something is really broken: {}", e))
            .ok();
    }
}
