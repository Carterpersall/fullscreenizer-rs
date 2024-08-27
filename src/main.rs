#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod my_window;

use my_window::MyWindow;
use winsafe::{self as w, co, prelude::*};

fn main() {
    if let Err(e) = run_app() {
        w::HWND::NULL
            .TaskDialog(
                None,
                Some("Unhandled error"),
                None,
                Some(&e.to_string()),
                co::TDCBF::OK,
                w::IconRes::Error,
            )
            .unwrap();
    }
}

fn run_app() -> w::AnyResult<i32> {
    MyWindow::new().run()
}
