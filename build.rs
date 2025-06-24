fn main() {
    // Tell the linker to include the resources.res file
    // The resource file contains the icon and manifest
    // The manifest does the following:
    //  1. requestedExecutionLevel:
    //      - level = "asInvoker"
    //          - The application will run with the same privileges as the user who started it.
    //      - uiAccess = "false"
    //          - The application will not be able to access protected UI elements.
    //  2. supportedOS:
    //      - Id = "{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"
    //          - Windows 10 and later
    //  3. assemblyIdentity:
    //      - name = "Microsoft.Windows.Common-Controls"
    //          - The application will use the common controls library.
    //          - This is required for the application to use the common controls (like buttons, edit boxes, etc.)
    //  4. windowsSettings:
    //      - dpiAware = "true/pm"
    //          - The application will be DPI aware on a per-monitor basis.
    //          - Only applies before Windows 10 1607.
    //      - dpiAwareness = "permonitorV2,permonitor"
    //          - The application will be DPI aware on a per-monitor basis.
    //          - Windows 10 1607 and later.
    println!("cargo:rustc-link-lib=dylib:+verbatim=resources/resources.res");
}
