// Clippy lints used for style and correctness checks
// Some lints are commented out as they are useful for targeted checks that
// may not be applicable project-wide (e.g., `absolute_paths`).
#![warn(
    //clippy::absolute_paths,
    clippy::collection_is_never_read,
    clippy::doc_markdown,
    clippy::indexing_slicing,
    clippy::map_err_ignore,
    //clippy::multiple_unsafe_ops_per_block,
    clippy::missing_const_for_fn,
    //clippy::missing_docs_in_private_items,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::redundant_pub_crate,
    clippy::semicolon_if_nothing_returned,
    //clippy::shadow_unrelated,
    //clippy::significant_drop_tightening,
    //clippy::single_call_fn,
    clippy::std_instead_of_core,
    clippy::unused_trait_names,
    clippy::useless_let_if_seq,
)]

mod my_window;

use my_window::MyWindow;
use winsafe::{self as w, co, prelude::*};

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
            .unwrap();
    }
}