#[cfg(target_os = "macos")]
mod imp {
    use std::{
        ptr::NonNull,
        sync::atomic::{AtomicBool, Ordering},
    };

    use block2::RcBlock;
    use objc2::{
        ClassType, DeclaredClass, declare_class, extern_methods, rc::Retained, runtime::AnyObject,
        runtime::NSObject, sel,
    };
    use objc2_app_kit::{
        NSAboutPanelOptionCredits, NSApplication, NSEvent, NSEventMask, NSEventModifierFlags,
        NSMenu, NSMenuItem, NSWindowButton,
    };
    use objc2_foundation::{
        MainThreadMarker, NSAttributedString, NSDictionary, NSPoint, ns_string,
    };

    static INSTALLED: AtomicBool = AtomicBool::new(false);
    static TITLEBAR_MONITOR_INSTALLED: AtomicBool = AtomicBool::new(false);
    static OPEN_PROJECT_REQUESTED: AtomicBool = AtomicBool::new(false);

    declare_class!(
        struct MtdNativeFileMenuTarget;

        unsafe impl ClassType for MtdNativeFileMenuTarget {
            type Super = NSObject;
            type Mutability = objc2::mutability::Immutable;
            const NAME: &'static str = "MtdNativeFileMenuTarget";
        }

        impl DeclaredClass for MtdNativeFileMenuTarget {}

        unsafe impl MtdNativeFileMenuTarget {
            #[method(showMtdAbout:)]
            fn show_about(&self, _sender: &AnyObject) {
                show_about_panel();
            }

            #[method(openMtdProject:)]
            fn open_project(&self, _sender: &AnyObject) {
                OPEN_PROJECT_REQUESTED.store(true, Ordering::SeqCst);
            }
        }
    );

    extern_methods!(
        unsafe impl MtdNativeFileMenuTarget {
            #[method_id(new)]
            fn new() -> Retained<Self>;
        }
    );

    pub(super) fn install_file_menu() {
        if INSTALLED.swap(true, Ordering::SeqCst) {
            return;
        }

        let Some(mtm) = MainThreadMarker::new() else {
            INSTALLED.store(false, Ordering::SeqCst);
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        install_titlebar_event_monitor();
        let Some(main_menu) = (unsafe { app.mainMenu() }) else {
            INSTALLED.store(false, Ordering::SeqCst);
            return;
        };
        if unsafe { main_menu.itemWithTitle(ns_string!("文件")) }.is_some() {
            return;
        }

        let target = MtdNativeFileMenuTarget::new();
        let target_object =
            unsafe { &*(target.as_ref() as *const MtdNativeFileMenuTarget as *const AnyObject) };
        install_about_items(&main_menu, target_object);

        let file_menu_item = NSMenuItem::new(mtm);
        unsafe { file_menu_item.setTitle(ns_string!("文件")) };

        let file_menu = NSMenu::new(mtm);
        let open_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                ns_string!("打开项目..."),
                Some(sel!(openMtdProject:)),
                ns_string!("o"),
            )
        };
        open_item.setKeyEquivalentModifierMask(NSEventModifierFlags::NSEventModifierFlagCommand);
        unsafe { open_item.setTarget(Some(target_object)) };

        file_menu.addItem(&open_item);
        file_menu_item.setSubmenu(Some(&file_menu));
        unsafe { main_menu.insertItem_atIndex(&file_menu_item, 1) };

        let _ = Retained::into_raw(target);
    }

    fn install_titlebar_event_monitor() {
        if TITLEBAR_MONITOR_INSTALLED.swap(true, Ordering::SeqCst) {
            return;
        }

        let block: RcBlock<dyn Fn(NonNull<NSEvent>) -> *mut NSEvent> = RcBlock::new(|event| {
            handle_titlebar_double_click(event);
            event.as_ptr()
        });
        let Some(monitor) = (unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::LeftMouseDown,
                &block,
            )
        }) else {
            TITLEBAR_MONITOR_INSTALLED.store(false, Ordering::SeqCst);
            return;
        };

        let _ = Retained::into_raw(monitor);
        std::mem::forget(block);
    }

    fn handle_titlebar_double_click(event: NonNull<NSEvent>) {
        const TITLEBAR_HIT_HEIGHT: f64 = 40.0;

        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let event = unsafe { event.as_ref() };
        if unsafe { event.clickCount() } < 2 {
            return;
        }
        let Some(window) = (unsafe { event.window(mtm) }) else {
            return;
        };
        let location = unsafe { event.locationInWindow() };
        let frame = window.frame();
        if location.y >= frame.size.height - TITLEBAR_HIT_HEIGHT {
            window.zoom(None);
        }
    }

    pub(super) fn adjust_window_controls() {
        position_window_controls();
    }

    fn position_window_controls() -> bool {
        const LEFT_INSET: f64 = 24.0;
        const TOP_INSET: f64 = 18.0;

        let Some(mtm) = MainThreadMarker::new() else {
            return false;
        };
        let app = NSApplication::sharedApplication(mtm);
        let Some(window) = (unsafe { app.mainWindow() }).or_else(|| app.keyWindow()) else {
            return false;
        };

        let Some(close) = window.standardWindowButton(NSWindowButton::NSWindowCloseButton) else {
            return false;
        };
        let Some(minimize) = window.standardWindowButton(NSWindowButton::NSWindowMiniaturizeButton)
        else {
            return false;
        };
        let Some(zoom) = window.standardWindowButton(NSWindowButton::NSWindowZoomButton) else {
            return false;
        };
        let Some(button_container) = (unsafe { close.superview() }) else {
            return false;
        };

        let close_frame = close.frame();
        let minimize_frame = minimize.frame();
        let container_bounds = button_container.bounds();
        let button_gap =
            (minimize_frame.origin.x - close_frame.origin.x).max(close_frame.size.width + 6.0);
        let button_y = if button_container.isFlipped() {
            TOP_INSET
        } else {
            (container_bounds.size.height - TOP_INSET - close_frame.size.height).max(0.0)
        };

        unsafe {
            close.setFrameOrigin(NSPoint::new(LEFT_INSET, button_y));
            minimize.setFrameOrigin(NSPoint::new(LEFT_INSET + button_gap, button_y));
            zoom.setFrameOrigin(NSPoint::new(LEFT_INSET + button_gap * 2.0, button_y));
        }

        true
    }

    pub(super) fn zoom_main_window() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        if let Some(window) = (unsafe { app.mainWindow() }).or_else(|| app.keyWindow()) {
            window.zoom(None);
        }
    }

    fn install_about_items(main_menu: &NSMenu, target: &AnyObject) {
        let Some(app_menu_item) = (unsafe { main_menu.itemAtIndex(0) }) else {
            return;
        };
        let Some(app_menu) = (unsafe { app_menu_item.submenu() }) else {
            return;
        };

        if let Some(about_item) = unsafe { app_menu.itemAtIndex(0) } {
            unsafe {
                about_item.setAction(Some(sel!(showMtdAbout:)));
                about_item.setTarget(Some(target));
            }
        }
    }

    fn show_about_panel() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        let credits_text = objc2_foundation::NSString::from_str(ABOUT_CREDITS);
        let credits = NSAttributedString::initWithString(mtm.alloc(), &credits_text);
        let options =
            NSDictionary::from_vec(&[unsafe { NSAboutPanelOptionCredits }], vec![credits]);
        let options = unsafe {
            &*(options.as_ref()
                as *const NSDictionary<objc2_app_kit::NSAboutPanelOptionKey, NSAttributedString>
                as *const NSDictionary<objc2_app_kit::NSAboutPanelOptionKey, AnyObject>)
        };
        unsafe { app.orderFrontStandardAboutPanelWithOptions(options) };
    }

    pub(super) fn take_open_project_request() -> bool {
        OPEN_PROJECT_REQUESTED.swap(false, Ordering::SeqCst)
    }

    const ABOUT_CREDITS: &str = "外部组件与版权声明\n\n\
        HarmonyOS Sans SC：华为字体，保留原许可。\n\
        FFmpeg：音频/字幕处理，LGPL-compatible。\n\
        Rust crates：MIT/Apache-2.0/Zlib 等。\n\
        MOSS API：受服务方条款约束。\n\
        完整许可文本内置于 .app。";
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub(super) fn adjust_window_controls() {}

    pub(super) fn install_file_menu() {}

    pub(super) fn zoom_main_window() {}

    pub(super) fn take_open_project_request() -> bool {
        false
    }
}

pub(crate) fn install_file_menu() {
    imp::install_file_menu();
}

pub(crate) fn adjust_window_controls() {
    imp::adjust_window_controls();
}

pub(crate) fn zoom_main_window() {
    imp::zoom_main_window();
}

pub(crate) fn take_open_project_request() -> bool {
    imp::take_open_project_request()
}
