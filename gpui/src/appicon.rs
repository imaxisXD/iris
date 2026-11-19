//! An app's icon as the Mac draws it (`NSWorkspace.icon(forFile:)`, as the Swift app asks for
//! it): the current icon, in the current light or dark appearance, drawn `n` pixels square.

/// The app at `path`'s icon, `n` pixels square, straight RGBA.
#[cfg(target_os = "macos")]
pub fn of_app(path: &str, n: u32) -> Option<Vec<u8>> {
    use core_graphics::color_space::CGColorSpace;
    use core_graphics::context::CGContext;
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use foreign_types::ForeignType;
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSRect, NSString};

    let ws = NSWorkspace::sharedWorkspace();
    let image = ws.iconForFile(&NSString::from_str(path));
    let side = n as f64;
    let mut rect = NSRect::new(objc2_foundation::NSPoint::new(0.0, 0.0), objc2_foundation::NSSize::new(side, side));
    let cg: *mut objc2_core_graphics::CGImage = unsafe { msg_send![&image, CGImageForProposedRect: &mut rect, context: std::ptr::null::<AnyObject>(), hints: std::ptr::null::<AnyObject>()] };
    if cg.is_null() {
        return None;
    }
    let space = CGColorSpace::create_device_rgb();
    let mut ctx = CGContext::create_bitmap_context(None, n as usize, n as usize, 8, n as usize * 4, &space,
        core_graphics::base::kCGImageAlphaPremultipliedLast);
    let cg_image = unsafe { core_graphics::image::CGImage::from_ptr(cg as *mut _) };
    ctx.draw_image(CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(side, side)), &cg_image);
    // The image belongs to the NSImage: don't release it with ours.
    std::mem::forget(cg_image);
    let mut out = ctx.data().to_vec();
    // Premultiplied to straight alpha.
    for p in out.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Some(out)
}

#[cfg(not(target_os = "macos"))]
pub fn of_app(_path: &str, _n: u32) -> Option<Vec<u8>> {
    None
}

/// Where the app with this bundle ID is installed.
#[cfg(target_os = "macos")]
pub fn path_of(bundle: &str) -> Option<String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;
    let url = NSWorkspace::sharedWorkspace().URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle))?;
    url.path().map(|p| p.to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn path_of(_bundle: &str) -> Option<String> {
    None
}

/// An app's name as Finder shows it, without ".app" (`InstalledApps.name(at:)`).
pub fn display_name(path: &str) -> String {
    std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().trim_end_matches(".app").to_string()).unwrap_or_default()
}

/// The name of the app with this bundle ID, as Finder shows it, or the ID when it isn't on
/// this Mac (`AppIconView.name(for:)`).
pub fn name_of(bundle: &str) -> String {
    path_of(bundle).map(|p| display_name(&p)).unwrap_or_else(|| bundle.to_string())
}

/// Every app in the Applications folders, open or not, by name (`InstalledApps.all`): (bundle
/// ID, name, path). Iris itself is left out.
pub fn installed() -> Vec<(String, String, String)> {
    let home = std::env::var("HOME").unwrap_or_default();
    let roots = ["/Applications".to_string(), "/System/Applications".into(), "/System/Applications/Utilities".into(), format!("{home}/Applications")];
    let me = crate::store::bundle_id();
    let mut seen = std::collections::HashSet::new();
    let mut apps = vec![];
    for root in roots {
        let Ok(dir) = std::fs::read_dir(&root) else { continue };
        let mut entries: Vec<_> = dir.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".app")).collect();
        entries.sort();
        for entry in entries {
            let path = format!("{root}/{entry}");
            let Some(id) = bundle_id_at(&path) else { continue };
            if id == me || id == "local.iris.app" || !seen.insert(id.clone()) {
                continue;
            }
            apps.push((id, display_name(&path), path));
        }
    }
    apps.sort_by_key(|a| a.1.to_lowercase());
    apps
}

/// The bundle ID of the app at `path`, from its Info.plist.
#[cfg(target_os = "macos")]
fn bundle_id_at(path: &str) -> Option<String> {
    use objc2_foundation::{NSBundle, NSString};
    Some(NSBundle::bundleWithPath(&NSString::from_str(path))?.bundleIdentifier()?.to_string())
}

#[cfg(not(target_os = "macos"))]
fn bundle_id_at(_path: &str) -> Option<String> {
    None
}
