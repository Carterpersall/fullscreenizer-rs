fn main() {
    // Compile the resources.rc file into a .res file
    embed_resource::compile("resources/resources.rc", embed_resource::NONE)
        .manifest_required().unwrap();
    // Rerun when anything under ./resources/ changes
    println!("cargo:rerun-if-changed=resources");
}
