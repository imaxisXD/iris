//! The menu bar flower (`MenuBarIcon`) and, on the Mac, the status item that shows it.
//!
//! The mark is the logo's iris coloured in with a crayon: three petals scribbled in level
//! strokes over a pink plate printed up-left, a stem and a quick leaf; white ink on a dark bar,
//! near-black on a light one. Rune's tray covers Windows and Linux; on the Mac Iris makes its
//! own `NSStatusItem` and reports where the flower is when it's clicked, so the slip can hang
//! from it.

use tiny_skia::{LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform, FillRule};

const PLATES: [&[(f32, f32)]; 3] = [
    &[(11.2, 4.7), (10.97, 6.19), (10.43, 7.5), (9.54, 8.39), (8.45, 8.48), (7.5, 7.65), (6.96, 6.24), (6.86, 4.7), (7.08, 3.25), (7.61, 1.97), (8.47, 1.1), (9.54, 1.01), (10.47, 1.82), (10.99, 3.2)],
    &[(8.21, 7.28), (8.06, 8.35), (7.33, 9.38), (6.26, 10.27), (4.92, 10.96), (3.41, 11.29), (2.12, 11.06), (1.47, 10.28), (1.62, 9.24), (2.34, 8.23), (3.38, 7.37), (4.68, 6.7), (6.15, 6.37), (7.42, 6.59)],
    &[(16.39, 10.22), (15.58, 10.81), (14.46, 11.07), (13.08, 10.99), (11.57, 10.44), (10.34, 9.46), (9.83, 8.33), (10.09, 7.42), (10.88, 6.85), (11.97, 6.59), (13.32, 6.67), (14.79, 7.2), (15.99, 8.16), (16.49, 9.26)],
];
const HATCHES: [&[(f32, f32)]; 3] = [
    &[(7.83, 1.6), (10.64, 2.32), (7.02, 3.04), (11.12, 3.76), (6.71, 4.48), (11.16, 5.2), (6.82, 5.92), (10.82, 6.64), (7.63, 7.36)],
    &[(4.3, 6.75), (7.85, 7.47), (2.14, 8.19), (7.74, 8.91), (1.81, 9.63), (6.29, 10.35)],
    &[(10.53, 6.75), (14.92, 7.47), (10.06, 8.19), (15.98, 8.91), (11.01, 9.63), (16.29, 10.35)],
];
const STEM: &[(f32, f32)] = &[(9.0, 10.4), (8.7, 13.6), (9.2, 17.4)];
const LEAF: &[(f32, f32)] = &[(9.2, 16.6), (11.4, 14.8), (13.6, 13.6), (12.6, 15.6), (9.5, 16.3)];

/// A Catmull-Rom curve through the points, as cubic Béziers.
fn smooth(p: &[(f32, f32)], closed: bool) -> tiny_skia::Path {
    let n = p.len() as i64;
    let at = |i: i64| if closed { p[i.rem_euclid(n) as usize] } else { p[i.clamp(0, n - 1) as usize] };
    let mut b = PathBuilder::new();
    b.move_to(p[0].0, p[0].1);
    for i in 0..(if closed { n } else { n - 1 }) {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        b.cubic_to(p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0, p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0, p2.0, p2.1);
    }
    if closed {
        b.close();
    }
    b.finish().unwrap()
}

/// The mark, 18 points at `scale`, as PNG bytes. `dark`: the bar is dark, so the crayon is white.
pub fn png(dark: bool, scale: f32) -> Vec<u8> {
    let px = (18.0 * scale) as u32;
    let mut pm = Pixmap::new(px, px).unwrap();
    let t = Transform::from_scale(scale, scale);
    let mut pink = Paint::default();
    pink.set_color_rgba8(240, 134, 176, 255);
    pink.anti_alias = true;
    for plate in PLATES {
        pm.fill_path(&smooth(plate, true), &pink, FillRule::Winding, t.pre_translate(-0.8, -0.6), None);
    }
    let mut ink = Paint::default();
    if dark { ink.set_color_rgba8(255, 255, 255, 255) } else { ink.set_color_rgba8(0, 0, 0, 217) }
    ink.anti_alias = true;
    let pen = |w: f32| Stroke { width: w, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
    for h in HATCHES {
        pm.stroke_path(&smooth(h, false), &ink, &pen(0.7), t, None);
    }
    pm.stroke_path(&smooth(STEM, false), &ink, &pen(1.25), t, None);
    pm.stroke_path(&smooth(LEAF, false), &ink, &pen(1.05), t, None);
    pm.encode_png().unwrap()
}

/// Where the flower was when it was clicked: its frame in screen points, top-left origin.
pub type Clicked = Box<dyn Fn([f64; 4]) + 'static>;

#[cfg(target_os = "macos")]
mod mac {
    use std::cell::RefCell;

    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{NSApplication, NSImage, NSScreen, NSStatusBar, NSStatusItem};
    use objc2_foundation::{NSData, NSSize, NSString};

    use super::Clicked;

    thread_local! {
        static ON_CLICK: RefCell<Option<Clicked>> = const { RefCell::new(None) };
        static ITEM: RefCell<Option<(Retained<NSStatusItem>, Retained<Target>)>> = const { RefCell::new(None) };
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "IrisStatusTarget"]
        pub struct Target;

        impl Target {
            #[unsafe(method(clicked:))]
            fn clicked(&self, _sender: &AnyObject) {
                let frame = frame();
                // Tests: hang the slip from this frame instead (the test display's bar), so a
                // click through accessibility never puts it on someone's screen.
                let frame = std::env::var("IRIS_SLIP_ANCHOR").ok()
                    .and_then(|v| { let n: Vec<f64> = v.split(',').filter_map(|x| x.parse().ok()).collect(); (n.len() == 4).then(|| [n[0], n[1], n[2], n[3]]) })
                    .or(frame);
                if let Some(frame) = frame {
                    ON_CLICK.with(|c| if let Some(f) = c.borrow().as_ref() { f(frame) });
                }
            }
        }
    );

    /// The flower's frame in screen points, top-left origin.
    pub fn frame() -> Option<[f64; 4]> {
        ITEM.with(|i| {
            let item = i.borrow();
            let (item, _) = item.as_ref()?;
            let mtm = MainThreadMarker::new()?;
            let button = item.button(mtm)?;
            let window = button.window()?;
            let f = window.frame();
            // Screen points have their origin bottom-left of the first screen; flip.
            let top = NSScreen::screens(mtm).firstObject().map_or(0.0, |s| s.frame().size.height);
            Some([f.origin.x, top - f.origin.y - f.size.height, f.size.width, f.size.height])
        })
    }

    /// Puts the flower in the menu bar; `on_click` hears where it is each time it's clicked.
    pub fn install(on_click: Clicked) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        ON_CLICK.with(|c| *c.borrow_mut() = Some(on_click));
        let _ = NSApplication::sharedApplication(mtm);
        // Both inks made once; the image picks one each time the bar draws it, from the bar's own
        // appearance (`NSAppearance.currentDrawing()`), as a template image would. Iris itself is
        // always light, so the app's appearance can't say which the bar is.
        let make = |dark: bool| -> Option<Retained<NSImage>> {
            let data = NSData::with_bytes(&super::png(dark, 2.0));
            let image = NSImage::initWithData(mtm.alloc::<NSImage>(), &data)?;
            image.setSize(NSSize::new(18.0, 18.0));
            Some(image)
        };
        let (Some(light), Some(dark)) = (make(false), make(true)) else { return };
        let draw = block2::RcBlock::new(move |rect: objc2_foundation::NSRect| -> objc2::runtime::Bool {
            unsafe {
                let class = objc2::runtime::AnyClass::get(c"NSAppearance").unwrap();
                let current: *mut AnyObject = msg_send![class, currentDrawingAppearance];
                let names = objc2_foundation::NSArray::from_retained_slice(&[NSString::from_str("NSAppearanceNameDarkAqua"), NSString::from_str("NSAppearanceNameAqua")]);
                let best: *mut NSString = if current.is_null() { std::ptr::null_mut() } else { msg_send![current, bestMatchFromAppearancesWithNames: &*names] };
                let is_dark = !best.is_null() && (*best).to_string().contains("Dark");
                let pick = if is_dark { &dark } else { &light };
                let _: () = msg_send![&**pick, drawInRect: rect];
            }
            objc2::runtime::Bool::YES
        });
        let image: Retained<NSImage> = unsafe {
            msg_send![<NSImage as objc2::ClassType>::class(), imageWithSize: NSSize::new(18.0, 18.0), flipped: false, drawingHandler: &*draw]
        };
        image.setTemplate(false);
        image.setAccessibilityDescription(Some(&NSString::from_str("Iris")));
        let item = NSStatusBar::systemStatusBar().statusItemWithLength(-2.0);
        let target: Retained<Target> = unsafe { msg_send![Target::alloc(mtm), init] };
        if let Some(button) = item.button(mtm) {
            button.setImage(Some(&image));
            unsafe {
                button.setTarget(Some(&target));
                button.setAction(Some(sel!(clicked:)));
            }
        }
        ITEM.with(|i| *i.borrow_mut() = Some((item, target)));
    }
}

#[cfg(target_os = "macos")]
pub use mac::install;

/// The frame to hang the slip from when it's opened without a click on the flower: the test
/// display's (`IRIS_SLIP_ANCHOR=x,y,w,h`), or the flower's own.
pub fn anchor() -> [f64; 4] {
    if let Some(n) = std::env::var("IRIS_SLIP_ANCHOR").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect::<Vec<f64>>())
        && n.len() == 4
    {
        return [n[0], n[1], n[2], n[3]];
    }
    #[cfg(target_os = "macos")]
    if let Some(f) = mac::frame() {
        return f;
    }
    [0.0, 0.0, 24.0, 24.0]
}

/// Where the pointer is, in screen points from the top-left of the first screen.
#[cfg(target_os = "macos")]
pub fn pointer() -> (f64, f64) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSEvent, NSScreen};
    let p = NSEvent::mouseLocation();
    let top = MainThreadMarker::new()
        .and_then(|mtm| NSScreen::screens(mtm).firstObject().map(|s| s.frame().size.height))
        .unwrap_or(0.0);
    (p.x, top - p.y)
}

#[cfg(not(target_os = "macos"))]
pub fn pointer() -> (f64, f64) {
    (0.0, 0.0)
}

/// Elsewhere the tray is Rune's (`rune::Tray`); not wired in this trial yet.
#[cfg(not(target_os = "macos"))]
pub fn install(_on_click: Clicked) {}
