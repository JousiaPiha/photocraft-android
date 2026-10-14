//! System-bar lifecycle: immersive on tablets, native window controls in DeX.

#[derive(Default)]
pub(crate) struct Fullscreen {
    focused: bool,
    content_size: Option<(i32, i32)>,
    desktop: Option<bool>,
}

impl Fullscreen {
    pub(crate) fn update(&mut self, focused: bool, rect: [i32; 4], desktop: bool) -> bool {
        let [left, top, right, bottom] = rect;
        let size = right.checked_sub(left).zip(bottom.checked_sub(top)).filter(|(w, h)| *w > 0 && *h > 0);
        // A resumed Activity can briefly lack a surface. Remember focus loss
        // immediately, but consume focus gain only once its rectangle is valid.
        if !focused {
            self.focused = false;
        }
        let Some((width, height)) = size else { return false };
        let regained_focus = focused && !self.focused;
        self.focused = focused;
        let expanded = self.content_size.is_none_or(|(w, h)| width > w || height > h);
        self.content_size = Some((width, height));
        let mode_changed = self.desktop != Some(desktop);
        if focused {
            self.desktop = Some(desktop);
        }
        focused && (regained_focus || expanded || mode_changed)
    }
}

/// Request on the Android UI thread; do not block the native event loop.
#[cfg(target_os = "android")]
pub(crate) fn apply(app: &android_activity::AndroidApp, desktop: bool) {
    let owned_app = app.clone();
    app.run_on_java_main_thread(Box::new(move || {
        let result = jni::JavaVM::singleton().and_then(|vm| {
            vm.attach_current_thread(|env| -> jni::errors::Result<()> {
                use jni::{jni_sig, jni_str, objects::JValue};
                let activity = owned_app.java_activity(env)?;
                let window = env.call_method(&activity, jni_str!("getWindow"), jni_sig!(() -> android.view.Window), &[])?.l()?;
                env.call_method(&window, jni_str!("setDecorCaptionShade"), jni_sig!((int) -> void), &[JValue::Int(2)])?;
                let sdk = env.get_static_field(jni_str!("android/os/Build$VERSION"), jni_str!("SDK_INT"), jni_sig!(int))?.i()?;
                if sdk >= 30 {
                    let controller =
                        env.call_method(&window, jni_str!("getInsetsController"), jni_sig!(() -> android.view.WindowInsetsController), &[])?.l()?;
                    // DeX needs persistent native caption/maximize controls,
                    // including when maximized. Tablet navigation stays transient.
                    let behavior = if desktop { 1 } else { 2 };
                    env.call_method(&controller, jni_str!("setSystemBarsBehavior"), jni_sig!((int) -> void), &[JValue::Int(behavior)])?;
                    // statusBars | navigationBars | captionBar; never hide IME.
                    let method = if desktop { jni_str!("show") } else { jni_str!("hide") };
                    env.call_method(&controller, method, jni_sig!((int) -> void), &[JValue::Int(1 | 2 | 4)])?;
                } else {
                    let decor = env.call_method(&window, jni_str!("getDecorView"), jni_sig!(() -> android.view.View), &[])?.l()?;
                    let flags = env.call_method(&decor, jni_str!("getSystemUiVisibility"), jni_sig!(() -> int), &[])?.i()?;
                    // IMMERSIVE_STICKY | FULLSCREEN | HIDE_NAVIGATION.
                    // Keep adjustResize: do not add Window FLAG_FULLSCREEN or
                    // LAYOUT_HIDE_NAVIGATION/LAYOUT_FULLSCREEN.
                    let flags = if desktop { flags & !(0x1000 | 0x800 | 4 | 2) } else { flags | 0x1000 | 4 | 2 };
                    env.call_method(&decor, jni_str!("setSystemUiVisibility"), jni_sig!((int) -> void), &[JValue::Int(flags)])?;
                }
                Ok(())
            })
        });
        if let Err(error) = result {
            log::warn!("Could not update Android system bars (desktop={desktop}): {error}");
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    const FULL: [i32; 4] = [0, 0, 2560, 1600];
    const KEYBOARD: [i32; 4] = [0, 0, 2560, 920];

    #[test]
    fn launch_and_resume_request_fullscreen_only_once() {
        let mut state = Fullscreen::default();
        assert!(!state.update(false, FULL, false));
        assert!(state.update(true, FULL, false));
        assert!(!state.update(true, FULL, false));
        assert!(!state.update(false, FULL, false));
        assert!(state.update(true, FULL, false));
    }

    #[test]
    fn keyboard_dismissal_restores_fullscreen_without_fighting_ime() {
        let mut state = Fullscreen::default();
        assert!(state.update(true, FULL, false));
        assert!(!state.update(true, KEYBOARD, false));
        assert!(!state.update(true, KEYBOARD, false));
        assert!(state.update(true, FULL, false));
        assert!(!state.update(true, FULL, false));
    }

    #[test]
    fn rotation_restores_fullscreen_and_invalid_rects_are_ignored() {
        let mut state = Fullscreen::default();
        assert!(!state.update(true, [0; 4], false));
        assert!(state.update(true, FULL, false));
        assert!(state.update(true, [0, 0, 1600, 2560], false));
        assert!(!state.update(true, [0, 0, 1600, 2560], false));
        assert!(!state.update(true, [i32::MIN, 0, i32::MAX, 10], false));
    }

    #[test]
    fn resume_waits_for_a_valid_surface_rectangle() {
        let mut state = Fullscreen::default();
        assert!(state.update(true, FULL, false));
        assert!(!state.update(false, [0; 4], false));
        assert!(!state.update(true, [0; 4], false));
        assert!(state.update(true, FULL, false));
        assert!(!state.update(true, FULL, false));
    }

    #[test]
    fn desktop_transitions_reapply_bars_even_without_expansion() {
        let mut state = Fullscreen::default();
        assert!(state.update(true, FULL, false));
        assert!(state.update(true, KEYBOARD, true));
        assert!(!state.update(true, KEYBOARD, true));
        assert!(state.update(true, KEYBOARD, false));
        assert!(!state.update(true, KEYBOARD, false));
    }

    #[test]
    fn desktop_transition_waits_for_focus_and_valid_surface() {
        let mut state = Fullscreen::default();
        assert!(state.update(true, FULL, false));
        assert!(!state.update(false, FULL, true));
        assert!(!state.update(true, [0; 4], true));
        assert!(state.update(true, FULL, true));
        assert!(!state.update(true, FULL, true));
        assert!(state.update(true, FULL, false));
    }
}
