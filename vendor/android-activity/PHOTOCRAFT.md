# PhotoCraft safe Activity access

Vendored crates.io android-activity 0.6.1. Original `.crate` SHA-256:
`0f2a1bb052857d5dd49572219344a7332b31b76405648eabac5bc68978251bcd`.
See LICENSE, LICENSE-APACHE and LICENSE-MIT for upstream licensing.

Only `src/native_activity/mod.rs` is changed: `AndroidApp::java_activity(env)`
duplicates the existing owned JNI global reference under its read lock. PhotoCraft
uses it with JNI 0.22's safe `JavaVM::singleton()` and Android's UI-thread
callback to enter immersive mode without raw pointer conversions or new unsafe
code. All upstream unsafe internals are unchanged. Revisit this patch when
android-activity exposes an equivalent safe accessor upstream.
