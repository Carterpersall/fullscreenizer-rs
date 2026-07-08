// Clippy lints used for style and correctness checks
// Some lints are commented out as they are useful for targeted checks that
// may not be applicable project-wide (e.g., `absolute_paths`).
#![warn(
    clippy::absolute_paths,
    clippy::clone_on_ref_ptr,
    clippy::collection_is_never_read,
    //clippy::doc_markdown,
    clippy::empty_structs_with_brackets,
    clippy::indexing_slicing,
    clippy::manual_string_new,
    clippy::map_err_ignore,
    clippy::match_bool,
    //clippy::multiple_unsafe_ops_per_block,
    clippy::missing_const_for_fn,
    //clippy::missing_docs_in_private_items,
    clippy::missing_inline_in_public_items,
    clippy::must_use_candidate,
    clippy::needless_bitwise_bool,
    clippy::needless_collect,
    clippy::needless_continue,
    clippy::needless_pass_by_ref_mut,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::semicolon_if_nothing_returned,
    //clippy::shadow_unrelated,
    clippy::significant_drop_in_scrutinee,
    //clippy::significant_drop_tightening,
    //clippy::single_call_fn,
    clippy::std_instead_of_core,
    clippy::str_to_string,
    clippy::trivially_copy_pass_by_ref,
    clippy::unused_self,
    clippy::unused_trait_names,
    clippy::useless_let_if_seq,
    //missing_docs,
    redundant_imports,
    redundant_lifetimes,
    unnameable_types,
    unreachable_pub,
    unused_import_braces,
    unused_qualifications,
    //unused_results,
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
