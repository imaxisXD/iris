// Windows only: the flower built into iris.exe as its icon (windows/res/iris.rc). Nothing on the
// Mac, whose app icon is the bundle's.
fn main() {
    println!("cargo:rerun-if-changed=windows/res/iris.rc");
    println!("cargo:rerun-if-changed=windows/res/iris.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("windows/res/iris.rc", embed_resource::NONE).manifest_optional().unwrap();
    }
}
