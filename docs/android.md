# PhotoCraft on Android

Status: experimental native tablet port, introduced 2026-10-13. The Android app runs the
same Rust engine and egui/wgpu editor as desktop. No Java/Kotlin application code,
JavaScript, webview, Tauri or Electron is used. The launcher is
`apps/photocraft-android`, packaged as a NativeActivity shared library.

## Build and install

The packaging script currently runs on Linux x86-64. Install Rust 1.95 or newer, a JDK
(`java` and `keytool`), `zip`, Android SDK command-line tools and adb. Then:

```sh
rustup target add aarch64-linux-android
export ANDROID_HOME=/path/to/android-sdk
"$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" \
  'platforms;android-35' 'build-tools;35.0.0' 'ndk;28.2.13676358'
packaging/android/build.sh
adb install -r target/android-apk/PhotoCraft.apk
adb shell am start -n ai.storyteller.photocraft/android.app.NativeActivity
```

The APK supports ARM64 devices, minimum Android 8/API 26, targeting API 35. The NDK can
be overridden with `ANDROID_NDK_HOME`. Cargo uses the committed lockfile. The existing
PhotoCraft icon is reused; no new third-party assets are included.

This is a **debuggable development APK**, signed with a generated local debug key at
`~/.android/photocraft-debug.keystore`. Keep the key to install updates over this build.
It is not a Play Store release or a signed production distribution. Build artifacts and
keys are not committed. The native library and APK use 16 KiB alignment (NDK r28c,
`zipalign -P 16`); a 16 KiB-page device has not yet been tested.

## Documents and editing

Use File → New, the existing drawing/editing tools, and File → Save As. The touch-friendly
file browser opens and saves files in:

```text
Internal storage/Android/media/ai.storyteller.photocraft/Documents
```

This is an app-owned shared folder, accessible through USB or a file manager. Copy images
there, then use Open (Refresh updates the list). Files can also be transferred with:

```sh
adb push example.psd /sdcard/Android/media/ai.storyteller.photocraft/Documents/
adb pull /sdcard/Android/media/ai.storyteller.photocraft/Documents/ ./PhotoCraft-documents/
```

Open supports selecting several files. Folder requests select this document folder;
nested folder navigation is not implemented. Save As accepts `.pcraft`, `.psd`, `.png`,
`.jpg` and the other formats supported by the shared editor; changing the extension chooses
the format. Existing files require Replace confirmation. Saves use the existing atomic
writer, and failed imports/exports return visible errors through the desktop crash guard.

Preferences and recovery data use Android's private app storage. Modified documents are
checkpointed every five seconds and when focus is lost. The NativeActivity suspension
hook queues the latest changes and waits for recovery writers; unchanged revisions are
not rewritten. Launch recovery uses the same recovery UI as desktop. A process killed
while actively editing can still lose changes since its last completed checkpoint.

Landscape gives the existing desktop-style editor more canvas space. Portrait and rotation
are supported; View → Fit on Screen refits an existing zoom after rotation.

## Fullscreen and DeX windows

In normal tablet mode, Android launches in immersive fullscreen and restores it after focus returns or the
content area grows (for example, after dismissing the keyboard). The status and
navigation bars are hidden; an edge swipe can temporarily reveal system navigation.
This is normal Android immersive behavior, not kiosk mode.

The launcher uses `WindowInsetsController` on Android 11/API 30 and newer, with
sticky immersive view flags on API 26–29. It hides only system bars, never the IME,
and keeps the NativeActivity content rectangle for keyboard-aware layout. It does
not use the `FLAG_FULLSCREEN` window flag, which disables `adjustResize`.
The `vendor/android-activity` patch exposes an owned Activity reference so all
application JNI calls remain safe Rust and run on Android's UI thread.
See Android's [immersive mode guide](https://developer.android.com/develop/ui/views/layout/immersive).

In Samsung DeX, PhotoCraft shows the native caption with minimize, maximize/restore
and close controls. The Activity explicitly supports resizing. The public `desk`
configuration selects this behavior even when the window is maximized; leaving DeX
restores tablet immersive mode. See Samsung's [DeX configuration guide](https://developer.samsung.com/samsung-dex/modify-optimizing.html).

The Rust launcher creates an Android `SurfaceView` inside NativeActivity's content
area, so wgpu draws the editor while Android draws its own window decorations.
The original native content view and input queue remain in place for keyboard,
focus and lifecycle handling. At startup, an already registered decor root is
re-registered once, preserving the content-layout listener. Pointer/touch positions
are translated from window coordinates to the child surface below the caption.
No Java/Kotlin sources, custom title-bar artwork or hidden Android APIs are used.

## Finger painting and canvas gestures

In **Edit → Preferences → Enhanced Controls**, turn **Paint with finger** off
and leave **Touch gestures** on. The setting is saved between launches.

- One finger still operates menus, panels and controls, but cannot edit the canvas.
- The S Pen (or a mouse) continues to operate canvas tools.
- Two fingers pan, pinch to zoom and twist to rotate around their midpoint. These
  touchscreen gestures are independent of the trackpad zoom/rotation preferences.
- With **Paint with finger** off and **Touch gestures** on, double-tap the canvas
  with one finger to reset rotation to 0° and Fit on Screen. Two nearby short taps
  count; dragging, holding, pen input, panel taps and multi-finger gestures do not.
  The gesture changes only the view and adds no paint marks or undo entries.
- A gesture owns the contacts until every finger lifts, so its final finger cannot
  leave a stray mark. With finger painting enabled, adding a second finger cancels
  the pending brush stroke before navigating.

Finger painting defaults to on for compatibility with existing installations.
These two settings are shown only on Android; other platform input is unchanged.
The pinned `vendor/winit` patch distinguishes finger contacts from pen pressure
before egui receives them. This does not provide full palm rejection or add pen tilt.

## Agent control

The existing JSON control protocol is disabled by default. For a development device:

```sh
packaging/android/control.py enable
# Save work, close PhotoCraft, then relaunch it to apply the configuration.
```

This writes an unpredictable token to private app storage and to the local mode-0600 file
`target/android-apk/control-token`, and forwards localhost port 7878 with adb. It does not
force-stop an editing session. Authenticate as described in [control-protocol.md](control-protocol.md).
The listener binds only Android loopback; automation file access is restricted to the
Documents folder. Use `app.open` and `app.save`, not ambient filesystem engine commands.
Set `ANDROID_SERIAL` when several devices are connected. `control.py disable` removes the
configuration and forwarding; close/relaunch the app to stop an existing listener.

## Validation and remaining work

Test device: Samsung Galaxy Tab S6 SM-T865, Android 12L/API 32, ARM64,
2560 × 1600 display, Adreno 640 Vulkan renderer. Device checks cover launch/rendering,
a touch brush stroke, undo/redo, `.pcraft` save/reopen, PNG export/import, the on-device
file browser (including multiple selection), keyboard input with the Save dialog above the IME,
background/resume, recovery after background process termination, and portrait/landscape rotation. The nine automated host tests
cover storage-name validation, 8/16/32-bit document round trips, corrupt imports, failed
writes, export options and suspension recovery completion/error handling.

Remaining Android-specific work:

- System document-provider/SAF picker, Gallery imports, share intents and image clipboard.
  The current in-app browser deliberately uses app-owned storage without broad permissions.
- Android accessibility: eframe's AccessKit adapter requires GameActivity, while this
  Rust-only build uses NativeActivity. Desktop/web accessibility remains enabled.
- Full physical S Pen pressure/tilt/eraser and palm-rejection validation; synthetic touch
  strokes do not establish stylus fidelity. No Android-specific tablet-driver support is claimed.
- A phone-oriented layout, Android printing, detach-to-another-window workflows, other
  CPU architectures, newer Android device coverage, production signing and store packaging.
- Optional HEIF decoding is not enabled in this initial build.

The existing unsafe-code ban remains in force except for the one Android-required
`#[unsafe(no_mangle)]` entry-point attribute, explicitly approved for this port. Its
function body and all platform-service code are safe Rust.

### Verification on 2026-10-13

- APK built, signature verified, installed and visually inspected on the device. ELF load
  segments and APK alignment pass 16 KiB checks. Test control configuration and adb forwarding
  were removed after verification; the installed app is left running normally.
- `cargo test -p photocraft-android`: 9 passed. Android launcher Clippy with `--no-deps`
  passes on both the Linux host (`--all-targets`) and `aarch64-linux-android`.
- `cargo xtask layers`: 30 crates, no violations. `cargo xtask wasm`: all 24 checks pass.
- Regenerated `docs/scorecard.md`: the new launcher raises never-crash coverage from
  28/28 to 29/29 crate roots; the scorecard staleness test passes.
- Full UI tests: 2,551 passed, 12 ignored, two existing translation failures (duplicate
  Japanese “Paper Width” and missing Serbian mask-selection labels).
- Full xtask tests: 60 passed, one existing translation-coverage failure.
- Unrestricted strict Clippy is blocked by existing `collapsible_match` warnings in
  `raw/src/cr3.rs`, `ui-egui/src/analysis_ui.rs`, `ui-egui/src/canvas.rs`, and a
  `nonminimal_bool` warning in `ui-egui/src/liquify_ui.rs`. Those warnings predate the Android changes.

Device screenshots and check logs are local build artifacts under
`target/android-apk/evidence/`; none are committed.

### Touch navigation verification on 2026-10-13

- Installed and visually checked the updated APK on the same Tab S6. With finger
  painting off, a native touchscreen swipe left document revision 1 unchanged.
  An injected two-finger gesture changed zoom 225% → 292.5%, rotation 0° → 20.05°,
  and view center, with the document still at revision 1. A native Android stylus
  swipe produced a visible stroke and revision 2. These are Android input-injection
  checks, not a physical S Pen pressure/tilt fidelity assessment.
- The actual preference checkbox was toggled by touch. Turning it on allowed a
  finger stroke (revision 3); turning it off was persisted. Touch gestures remained
  on. After restarting, the user's recovered document was left open and the test
  document was closed. The opt-in control listener and temporary device test driver
  were removed.
- New UI regressions cover finger/pen routing, gesture contact transitions, pan/zoom/
  rotation anchoring, interrupted strokes, a resting finger during pen release,
  cancellation batched with a press, and Crop/Transform gesture takeover.
- Combined library tests: Android 9 passed; engine 1,536 passed / 13 ignored;
  UI 2,563 passed / 12 ignored with the same two pre-existing translation failures.
  The new preference persistence test and all touch tests passed. A subsequent
  targeted Frame/Artboard gesture-takeover regression also passed. The engine's
  adversarial command test and all four preference-usage checks passed.
- Layers and all 24 wasm checks passed. Strict Clippy is still blocked by existing
  collapsible-match/nonminimal-bool diagnostics (also `engine/src/color_to_alpha_cmds.rs`).
  With only those existing lint classes allowed, strict no-dependency Clippy passes
  for Android, engine and UI, all targets. Scorecard: 25 inactive settings of 157
  (the existing touch-gestures setting is now implemented on Android).

### Immersive fullscreen verification on 2026-10-13

- Updated APK built, signed, installed and visually checked on the Tab S6/API 32.
  Both system bars hide on launch and after Home/resume. An edge swipe reveals
  transient bars over the editor; they auto-hide again. WindowManager confirms
  `SHOW_TRANSIENT_BARS_BY_SWIPE` and both bars requested invisible.
- All 13 Android tests pass, including four lifecycle regressions for launch,
  resume, content-area expansion, rotation and temporarily invalid surface bounds.
  Strict no-dependency Android-launcher Clippy passes on host and ARM64 Android;
  formatting, dependency layering and all 24 wasm checks pass.
- Keyboard layout could not be revalidated on the current device configuration:
  Gboard reports the IME open but displays no keys. A comparison APK with immersive
  mode disabled reproduces it. Samsung Keyboard and Hacker's Keyboard also did not
  display their keys during this check. This remains a separate Android input issue;
  the user's original Gboard and hardware-keyboard settings were restored.
- API 26–29's legacy immersive fallback and newer Android versions remain untested
  on hardware. The user's drawing was backed up and recovered unchanged; no test
  saves were made to it. Evidence is local under `target/android-apk/evidence/fullscreen/`.

### Finger double-tap verification on 2026-10-13

- Native Android touch injection on the Tab S6 changed an 800 × 600 test canvas from
  rotation 38°, zoom 400%, center [250, 220] to rotation 0°, Fit on Screen 206.1%,
  center [400, 300]. A single tap left the view unchanged; double-tap left the
  document clean at revision 1. The fitted view was visually inspected.
- With finger painting enabled, two taps painted normally and kept the view unchanged.
  Disabling Touch gestures also prevented reset. Stylus taps painted without resetting;
  drag-then-tap did not reset. The test document was closed and the user's drawing
  restored; finger painting remains off, Touch gestures on, control listener disabled.
- Eight new regressions pass: actual canvas reset/fit/history preservation, preference/
  pen/dialog routing, nearby/timely tap pairs, pair consumption, movement/hold/cancel,
  multi-contact interruption, focus/document/preference changes, malformed timestamps,
  device identity and coalesced input batches. Full UI suite: 2,572 passed, 12 ignored,
  the same two existing Japanese/Serbian translation failures. Android: 13 passed.
- Strict dependency-inclusive Clippy encounters existing `collapsible_match` warnings
  in Affinity/Raw; no-dependency UI/Android Clippy passes with only the previously known
  `collapsible_match` and `nonminimal_bool` classes allowed. Format and layering checks pass.
  All 24 wasm checks pass.
  Local evidence: `target/android-apk/evidence/double-tap/`.

### DeX window verification on 2026-10-13

- Tab S6/API 32: native caption visible in both freeform and maximized windows;
  maximize and restore respond to caption-button taps, and File menu taps align
  with the editor beneath the caption. DeX system bars remain available.
- Leaving DeX restores tablet immersive fullscreen; entering DeX restores native
  controls. The drawing remains open after Android recreates the Activity.
- A comparison APK delayed surface installation by two seconds, exercising decor
  reattachment after the first traversal. Caption rendering, resize and menu input
  passed after preserving NativeActivity's global-layout listener. The final APK
  removes that test delay.
- All 17 Android tests pass, including mode transitions, invalid bounds, and
  caption-offset pointer/touch mapping. Dependency layering passes. Other Android
  versions and external-display DeX still need hardware coverage.
- The user's private data was backed up before updating. No test drawing was added.
  Local evidence: `target/android-apk/evidence/dex/`.

### Painting mode verification on 2026-10-14

- Built, signature/alignment checked, updated and visually tested the APK on the
  Tab S6/API 32. A visible **Painting** button in the normal top bar and **Editor**
  in painting mode switch interfaces without altering the document or panel flags.
  The Android entry button has a 44-point touch target in a 48-point top bar.
- Native Android input injection verified finger suppression, stylus painting,
  two-finger pan/pinch/rotation, double-tap fit/reset, undo/redo, size/opacity and
  smudge-strength controls, and moving the sidebar. Navigation and panel taps
  preserved document revision; a stylus stroke advanced it. The final build's
  mode switch was checked by touch in landscape and portrait.
- Brushes, Layers and Color overlays and transform Cancel/Apply were inspected
  on-device. Transform cancellation, save/reopen, and background/relaunch passed.
  These are injected Android input tests, not physical S Pen pressure/tilt tests.
- Original drawing state was restored unchanged; rotation settings restored,
  temporary document copied to local evidence then removed from the device,
  test driver and control configuration removed. The app is left in Painting Mode
  with the original drawing open. No app panic/fatal-signal reports were logged.
- Evidence: `target/android-apk/evidence/painting/` (local, not committed).
- Full UI suite: 2,694 passed, 12 ignored; layers, wasm and formatting passed.
  Strict Clippy retains the existing Raw `collapsible_match` blocker; scoped
  all-targets Clippy passes with the known baseline lint categories allowed.


### Painting Mode refinements — 2026-10-14

On the Galaxy Tab S6 (SM-T865, Android 12L), verified the compact 44-point top bar and
52-point side rail in landscape and portrait, with no document tabs or canvas scrollbars.
Entering Painting Mode selects Full Screen Mode with Menu Bar; Editor restores Standard.
The eyedropper cancels on its second button press. A stationary injected pen contact sampled
`#dc3545` immediately and returned to Brush on lift with the color retained and no document
revision change. Deselect cleared the test selection through its visible button.

Finger and pen drags starting over brush rows scrolled the Painting Mode library and the
normal editor's Brush Preset picker, preserving the preset order. One tap selected Chalk.
A tap near the opacity slider's outer edge changed opacity to 51.55%; a pen drag changed it
to 74.50%. A size-slider edge tap changed size to 422 px without touching the document.
Existing finger-painting preferences, two-contact pan/pinch/rotation and pen strokes passed.
The test used an isolated document; the original recovered document and selection were
preserved, rotation settings restored, and the temporary control listener and input drivers
removed. A cold restart succeeded and the app was left in Painting Mode.

Evidence: `target/android-apk/evidence/painting-refinements/` (local, gitignored).
APK SHA-256: `130d3fd6cb4e68e98dc6410984663387b1a85d34f37918f9979114f64bcbdd84`.

### Brush order unlock and compact library — 2026-10-14

The lock beside Search Brushes now enables deliberate drag organization in Painting Mode,
Window → Brushes and the normal Brush Preset picker. The default remains locked, allowing
finger and pen drags anywhere on the list to scroll. Painting Mode's Paint, Smudge and Erase
libraries omit the title and close button; tapping the active tool closes the library.

On the Galaxy Tab S6, native finger input moved Hard Round below Hard Round Fine, and native
stylus input moved it back. Relocking restored scrolling for both input sources without
changing the library. The normal picker also passed native unlock/reorder/relock/scroll
checks. Landscape and portrait screenshots were inspected. The final localized APK passed
installation, cold restart and native lock/tool-toggle checks. Original document data, the
full brush library and active brush settings were preserved. Rotation settings were restored,
the isolated test document was removed, control was disabled, and Painting Mode was left open.

Evidence: `target/android-apk/evidence/brush-unlock/` (local, gitignored).
APK SHA-256: `2247931390e08d07260afddfef4c46c600c601af47f903af8f68571ab3f0de8d`.

Validation: full UI suite passed serially (2,701 passed, 12 ignored), layers, wasm, engine
`panic_hunt`, formatting and scoped Clippy passed. The parallel GPU integration test received
SIGSEGV in the VM; both its isolated serial run and full serial rerun passed. Strict Clippy
retains the pre-existing Raw `collapsible_match` blocker.

### Upstream integration — 2026-10-14

Integrated `storytold/photocraft` through `706638b2` (190 commits newer than the Android
work's original base). Kept upstream brush persistence, canvas/rendering improvements,
brush-card geometry fixes, translations and dependency pins alongside the native Android
launcher, touch navigation and Painting Mode. The pre-integration source remains available
locally as `backup/android-tested-2026-10-14` at `76d532bc`.

The merged APK was installed on the Galaxy Tab S6 and exercised with native finger/pen
input: brush unlock/reorder/relock, list scrolling, tool-toggle panel closure, pen painting,
eyedropper cancellation and return to Brush on pen lift. Original document data, brush
settings and the full library were preserved; the isolated test document was removed.
Evidence and the previous APK are in `target/android-apk/evidence/upstream-integration/`
(local, gitignored).

Final validation: 4,628 engine/UI tests passed (31 intentionally ignored); the separate
adversarial `panic_hunt` test passed. Layering, WebAssembly, formatting, parity and scorecard
generation passed. The preference audit initially missed reads below a test-module declaration;
moving Painting Mode's test declaration into the existing test section fixed the audit without
changing app behavior. The VM required stripping debug sections from generated test binaries
to fit the full suite; no repository build settings changed.

Final APK SHA-256: `5d6d80f0b6bfbf642d674ecac9d14875df4a721b097103dfcaa4e422850ed7bb`.
The final APK passed installation, cold restart and native lock/tool-toggle checks. The control
listener was disabled and PhotoCraft was left open in Painting Mode.

Scoped all-targets Clippy passed for engine/UI with the existing `collapsible_match` and
`nonminimal_bool` categories allowed. Strict Clippy still stops at the pre-existing
`crates/raw/src/cr3.rs:145` `collapsible_match` warning; source lint settings remain unchanged.
