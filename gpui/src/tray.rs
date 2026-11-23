//! Windows: Iris in the notification area (the tray), as Windows apps that run in the
//! background are. The flower says Iris is running; its tooltip says how the model stands.
//! Clicking it opens Iris's window (Settings); right-clicking it gives the system's own small
//! menu: Open Iris, Quit Iris. Nothing of the Mac's menu bar slip: on Windows, Settings is Iris.

use gpui::{App, AsyncApp};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use std::cell::RefCell;

thread_local! {
    /// The icon lives as long as Iris: dropping it takes it out of the tray.
    static TRAY: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
}

/// What a click asked for, carried from the tray's window procedure to GPUI's main loop.
enum Ask {
    Open,
    Quit,
}

/// The flower at the tray's size: 16 pixels at 100% scaling, more on a sharper screen.
fn icon() -> Option<tray_icon::Icon> {
    use windows::Win32::UI::HiDpi::GetDpiForSystem;
    let dpi = unsafe { GetDpiForSystem() }.max(96) as f32;
    let px = (16.0 * dpi / 96.0).round();
    // The taskbar is dark unless Windows is set to light (SystemUsesLightTheme = 1).
    let dark = !light_taskbar();
    let png = crate::menubar::png(dark, px / 18.0);
    let rgba = image::load_from_memory(&png).ok()?.to_rgba8();
    let (w, h) = rgba.dimensions();
    tray_icon::Icon::from_rgba(rgba.into_raw(), w, h).ok()
}

fn light_taskbar() -> bool {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    use windows::core::w;
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let ok = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"), RRF_RT_REG_DWORD, None, Some(&mut value as *mut u32 as *mut _), Some(&mut size))
    };
    ok.is_ok() && value == 1
}

/// The tooltip: Iris, and how the model stands, in the words the slip's header uses.
fn tooltip(cx: &App) -> String {
    let state = match crate::brain::get(cx).map(|b| b.read(cx).engine.clone()) {
        Some(crate::brain::Engine::Ready(_)) => "ready",
        Some(crate::brain::Engine::Loading) => "waking up",
        Some(crate::brain::Engine::Downloading(_)) => "getting the model",
        Some(crate::brain::Engine::Failed(_)) => "the model needs a look",
        _ => "resting",
    };
    format!("Iris: {state}")
}

/// Puts the flower in the tray and answers its clicks.
pub fn install(cx: &mut App) {
    // GPUI quits with the last window on Windows; Iris lives on in the tray until Quit Iris.
    cx.set_quit_mode(gpui::QuitMode::Explicit);
    let (tx, rx) = async_channel::unbounded::<Ask>();
    let open = MenuItem::with_id("open", "Open Iris", true, None);
    let quit = MenuItem::with_id("quit", "Quit Iris", true, None);
    let menu = Menu::new();
    let _ = menu.append_items(&[&open, &PredefinedMenuItem::separator(), &quit]);
    let built = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        // Left click opens Iris; the menu is the right click's, as in other Windows apps.
        .with_menu_on_left_click(false)
        .with_tooltip(tooltip(cx))
        .with_icon(match icon() {
            Some(i) => i,
            None => return eprintln!("tray: no icon"),
        })
        .build();
    let tray = match built {
        Ok(t) => t,
        Err(e) => return eprintln!("tray: {e}"),
    };
    TRAY.with(|t| *t.borrow_mut() = Some(tray));

    // Both handlers run inside the tray window's procedure, on the main thread but outside
    // GPUI: they only pass the ask on.
    let menu_tx = tx.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let ask = if e.id.0 == "quit" { Ask::Quit } else { Ask::Open };
        let _ = menu_tx.try_send(ask);
    }));
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            let _ = tx.try_send(Ask::Open);
        }
    }));
    cx.spawn(async move |cx: &mut AsyncApp| {
        while let Ok(ask) = rx.recv().await {
            let _ = cx.update(|cx| match ask {
                Ask::Open => crate::open_settings(crate::settings::Page::General, cx),
                Ask::Quit => cx.quit(),
            });
        }
    })
    .detach();

    // The tooltip follows the model.
    if let Some(brain) = crate::brain::get(cx) {
        cx.observe(&brain, |_, cx| {
            let text = tooltip(cx);
            TRAY.with(|t| if let Some(t) = t.borrow().as_ref() { let _ = t.set_tooltip(Some(text)); });
        })
        .detach();
    }
}
