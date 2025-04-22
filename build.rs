fn main() {
    // Tell the linker to include the resources.res file
    // The resource file is needed for the application to run due to the manifest in it
    println!("cargo:rustc-link-lib=dylib:+verbatim=resources/resources.res");
}
