#[cfg(target_os = "macos")]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};

    use objc2::{
        ClassType, DeclaredClass, declare_class, extern_methods, rc::Retained, runtime::AnyObject,
        runtime::NSObject, sel,
    };
    use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
    use objc2_foundation::{MainThreadMarker, ns_string};

    static INSTALLED: AtomicBool = AtomicBool::new(false);
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

    pub(super) fn take_open_project_request() -> bool {
        OPEN_PROJECT_REQUESTED.swap(false, Ordering::SeqCst)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub(super) fn install_file_menu() {}

    pub(super) fn take_open_project_request() -> bool {
        false
    }
}

pub(crate) fn install_file_menu() {
    imp::install_file_menu();
}

pub(crate) fn take_open_project_request() -> bool {
    imp::take_open_project_request()
}
