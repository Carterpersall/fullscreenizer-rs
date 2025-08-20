fn main() {
    // Compile the resources.rc file into a .res file
    embed_resource::compile("resources/resources.rc", embed_resource::NONE)
        .manifest_required()
        // Panic on error, as the app will fail to launch without the compiled resource file
        .expect("Failed to compile resources.rc");

    // Rerun when anything under ./resources/ changes
    println!("cargo:rerun-if-changed=resources");
}
