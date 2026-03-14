fn main() {
    glib_build_tools::compile_resources(
        &["resources"],
        "resources/seemux.gresource.xml",
        "seemux.gresource",
    );
}
