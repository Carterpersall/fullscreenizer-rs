fn main() {
    // Tell the linker to include the resources.res file
    println!("cargo:rustc-link-lib=dylib:+verbatim=resources/resources.res");
}
