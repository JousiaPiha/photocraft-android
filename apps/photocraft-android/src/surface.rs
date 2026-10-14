//! Keep Android's decor on its own surface so native desktop captions can draw.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(crate) fn install(app: &android_activity::AndroidApp) -> Arc<AtomicBool> {
    let installed = Arc::new(AtomicBool::new(false));
    let ready = installed.clone();
    let owned_app = app.clone();
    app.run_on_java_main_thread(Box::new(move || {
        let result = jni::JavaVM::singleton().and_then(|vm| {
            vm.attach_current_thread(|env| -> jni::errors::Result<()> {
                use jni::{
                    jni_sig, jni_str,
                    objects::{JObject, JValue},
                };
                let activity = owned_app.java_activity(env)?;
                let window = env.call_method(&activity, jni_str!("getWindow"), jni_sig!(() -> android.view.Window), &[])?.l()?;
                let decor = env.call_method(&window, jni_str!("getDecorView"), jni_sig!(() -> android.view.View), &[])?.l()?;
                let parent = env.call_method(&decor, jni_str!("getParent"), jni_sig!(() -> android.view.ViewParent), &[])?.l()?;
                let attached = !parent.is_null();
                let manager = env.call_method(&activity, jni_str!("getWindowManager"), jni_sig!(() -> android.view.WindowManager), &[])?.l()?;
                let params = env.call_method(&window, jni_str!("getAttributes"), jni_sig!("()Landroid/view/WindowManager$LayoutParams;"), &[])?.l()?;
                let content_id = env.get_static_field(jni_str!("android/R$id"), jni_str!("content"), jni_sig!(int))?.i()?;
                let content = env.call_method(&activity, jni_str!("findViewById"), jni_sig!((int) -> android.view.View), &[JValue::Int(content_id)])?.l()?;
                let native_content = env.call_method(&content, jni_str!("getChildAt"), jni_sig!((int) -> android.view.View), &[JValue::Int(0)])?.l()?;
                let surface =
                    env.new_object(jni_str!("android/view/SurfaceView"), jni_sig!((android.content.Context) -> void), &[JValue::Object(&activity)])?;
                let holder = env.call_method(&surface, jni_str!("getHolder"), jni_sig!(() -> android.view.SurfaceHolder), &[])?.l()?;
                // NativeActivity already implements these public callbacks. Keep
                // its NativeContentView for focus, IME and content-rect reporting.
                env.call_method(&holder, jni_str!("addCallback"), jni_sig!("(Landroid/view/SurfaceHolder$Callback;)V"), &[JValue::Object(&activity)])?;
                env.call_method(&window, jni_str!("takeSurface"), jni_sig!("(Landroid/view/SurfaceHolder$Callback2;)V"), &[JValue::Object(&JObject::null())])?;
                env.call_method(
                    &content,
                    jni_str!("addView"),
                    jni_sig!((android.view.View, int, int) -> void),
                    &[JValue::Object(&surface), JValue::Int(-1), JValue::Int(-1)],
                )?;
                // ViewRoot caches takeSurface during registration. Re-register
                // once if already parented, including before the first traversal
                // when isAttachedToWindow is still false. Future resize/resume
                // callbacks are managed by SurfaceView and NativeActivity.
                if attached {
                    let observer =
                        env.call_method(&native_content, jni_str!("getViewTreeObserver"), jni_sig!(() -> android.view.ViewTreeObserver), &[])?.l()?;
                    env.call_method(
                        &observer,
                        jni_str!("removeOnGlobalLayoutListener"),
                        jni_sig!("(Landroid/view/ViewTreeObserver$OnGlobalLayoutListener;)V"),
                        &[JValue::Object(&activity)],
                    )?;
                    env.call_method(&manager, jni_str!("removeViewImmediate"), jni_sig!((android.view.View) -> void), &[JValue::Object(&decor)])?;
                    // NativeActivity registered its layout listener once during
                    // onCreate. An attached root owns that listener and destroys
                    // it on detach, so register it on the new root as well.
                    let observer =
                        env.call_method(&native_content, jni_str!("getViewTreeObserver"), jni_sig!(() -> android.view.ViewTreeObserver), &[])?.l()?;
                    env.call_method(
                        &observer,
                        jni_str!("addOnGlobalLayoutListener"),
                        jni_sig!("(Landroid/view/ViewTreeObserver$OnGlobalLayoutListener;)V"),
                        &[JValue::Object(&activity)],
                    )?;
                    env.call_method(
                        &manager,
                        jni_str!("addView"),
                        jni_sig!("(Landroid/view/View;Landroid/view/ViewGroup$LayoutParams;)V"),
                        &[JValue::Object(&decor), JValue::Object(&params)],
                    )?;
                }
                Ok(())
            })
        });
        if let Err(error) = result {
            log::error!("Could not install Android content surface: {error}");
        } else {
            ready.store(true, Ordering::Release);
        }
    }));
    installed
}
