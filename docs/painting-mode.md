# PhotoCraft painting mode — research and implementation design

2026-10-14. Intent: offer an optional canvas-focused native painting interface inspired by Procreate, using PhotoCraft's existing painting engine and working touch navigation.

## Observations from official documentation

- [Interface](https://help.procreate.com/procreate/handbook/interface-gestures/interface): a compact top toolbar separates editing actions from painting tools, layers, and color. Size and opacity live along the canvas edge; the sidebar can switch sides.
- [Paint, Smudge, and Erase](https://help.procreate.com/procreate/handbook/brushes/paint-smudge-erase): selecting the active tool again opens its brush library. All three tools share access to brushes. Smudge uses strength, while paint and erase use opacity.
- [Gestures](https://help.procreate.com/procreate/handbook/interface-gestures/gestures): navigation and history actions are accessible without permanent panels. PhotoCraft retains its existing touch implementation, including its navigation and pen-input behavior.
- [QuickMenu](https://help.procreate.com/procreate/handbook/interface-gestures/quickmenu): frequently used operations are available on demand. This implementation uses an ordinary labeled actions menu with existing commands, retaining keyboard and agent access.

No proprietary code, icons, brushes, artwork, or other assets are used.

## Chosen design

Extend the existing L6 workspace flow, with a separate reversible Painting Mode toggle. Selecting the Painting workspace enters it; choosing another workspace leaves it. The toggle preserves the current editor's panel layout. A visible Painting button in the regular top bar enters the mode, and an Editor button exits. Android gives the entry button a 44-point touch target. Entering selects Full Screen Mode with Menu Bar and hides document tabs and canvas scrollbars. Returning to Editor restores the previous screen mode. Existing window decorations remain usable.

A 44-point top bar provides actions, selection, transform, fit, Paint/Smudge/Erase, Layers and Color. Selection exposes a Deselect button. Narrow windows wrap into two rows. A 52-point left/right rail provides logarithmic brush size, opacity (strength for Smudge), eyedropper, Undo and Redo. Sliders respond on press across a 44-point hit area with numerical snapping disabled. The eyedropper toggles back to Brush on a second press; a canvas contact samples immediately and returns to Brush on lift without adding a stroke. One overlay at a time displays Brushes, Layers or Color, reusing the existing components and commands. Repeating the current painting tool opens Brushes. Brush lists throughout the editor use single-tap selection and drag-to-scroll with ordering locked by default, including group headers, list rows, grids and picker cards. A lock button beside Search Brushes unlocks drag organization of presets and folders; locking again restores drag-to-scroll. Painting Mode omits the brush library title and close button, using the extra space for brushes; tapping the active Paint, Smudge or Erase tool closes the library. Explicit preset-organizing commands remain available to automation. Theme tokens and existing licensed icons supply styling.

State is serialized in UiState; window.paintingMode validates optional enabled, panel and rightHanded parameters atomically. ui.inspect reports the state. Brush changes dispatch tools.setBrush. Canvas drawing and document data continue to use the existing engine; touch navigation and pen preferences remain in effect.

## Verification

Eighteen Painting Mode tests, plus shared brush-list and folder-drag regressions, cover command validation, visible mode-button round trips, reversible layout, workspace selection, per-tool brush memory, actual control clicks, immediate slider updates, pen-lift sampling, full-screen restoration, Deselect, locked library order and scrolling, native caption space, transform cancellation, responsive geometry across 14 themes, and the existing Android touch/pen pipeline. Landscape and portrait screenshots were rendered offscreen and inspected.

Validation on 2026-10-14: full ui-egui suite passed serially (2,836 tests, 12 ignored); layers, wasm, parity and engine panic_hunt passed. Strict Clippy encounters existing `collapsible_match` and `nonminimal_bool` findings outside the painting implementation. The crate-only all-targets check passes with those two lint categories allowed; source lint settings remain unchanged.

## Scope boundary

This is an alternate painting interface, not Procreate file/brush compatibility, QuickShape, ColorDrop or a new gesture recognizer. Existing PhotoCraft gestures remain available with their current preferences and platform rules.
