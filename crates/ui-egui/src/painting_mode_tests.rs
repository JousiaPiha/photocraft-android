use crate::{PhotocraftApp, menus};
use serde_json::{Value, json};

#[test]
fn painting_mode_is_drivable_without_changing_the_editor_layout() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    let ctx = egui::Context::default();
    let before = serde_json::to_value(&app.ui.panels).unwrap();
    assert!(menus::is_enabled(&app, "window.paintingMode"));
    let result = menus::invoke(&mut app, &ctx, "window.paintingMode", json!({"enabled": true, "panel": "layers", "rightHanded": true})).unwrap();
    assert_eq!(result["enabled"], true);
    assert_eq!(result["panel"], "layers");
    assert_eq!(menus::menu_items(&app).into_iter().find(|i| i.id == "window.paintingMode").and_then(|i| i.checked), Some(true));
    let result = menus::invoke(&mut app, &ctx, "window.paintingMode", Value::Null).unwrap();
    assert_eq!(result["enabled"], false);
    assert_eq!(serde_json::to_value(&app.ui.panels).unwrap(), before);
}

#[test]
fn painting_mode_rejects_invalid_parameters_atomically() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    let ctx = egui::Context::default();
    for params in [json!({"enabled": true, "panel": "nope"}), json!({"enabled": "yes"}), json!({"rightHanded": 1}), json!({"unknown": true}), json!([])] {
        assert!(menus::invoke(&mut app, &ctx, "window.paintingMode", params).is_err());
        assert_eq!(menus::menu_items(&app).into_iter().find(|i| i.id == "window.paintingMode").and_then(|i| i.checked), Some(false));
    }
}

fn harness(size: egui::Vec2, theme: crate::theme::ThemeKind) -> egui_kittest::Harness<'static, PhotocraftApp> {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.run("file.new", json!({"width":400,"height":300})).unwrap();
    app.sync_views();
    app.ui.painting.enabled = true;
    let mut h = egui_kittest::Harness::builder().with_size(size).build_ui_state(
        |ui, app: &mut PhotocraftApp| {
            let ctx = ui.ctx().clone();
            if !ctx.fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                return;
            }
            crate::paint_mouse::sync_tool_brush(app);
            crate::painting_mode::chrome(app, ui);
            egui::CentralPanel::default().show(ui, |ui| crate::canvas::document_area(app, ui));
            crate::painting_mode::overlay(app, &ctx);
        },
        app,
    );
    PhotocraftApp::setup_context(&h.ctx, theme);
    h.run_steps(4);
    h
}

#[test]
fn painting_toolbar_selects_tools_opens_panels_and_keeps_per_tool_brushes() {
    use crate::{painting_mode::Panel, state::Tool};
    use egui_kittest::kittest::Queryable;
    let mut h = harness(egui::vec2(1024.0, 768.0), crate::theme::ThemeKind::Studio);
    h.state_mut().run("tools.setBrush", json!({"brush":{"size":42.0}})).unwrap();
    h.get_by_label("Erase").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.tool, Tool::Eraser);
    h.state_mut().run("tools.setBrush", json!({"brush":{"size":12.0}})).unwrap();
    h.get_by_label("Paint").click();
    h.run_steps(3);
    assert_eq!(h.state().session.tools.brush.size, 42.0);
    h.get_by_label("Paint").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.painting.panel, Panel::Brushes);
    assert!(h.query_by_label("Close").is_none());
    h.get_by_label("Paint").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.painting.panel, Panel::None);
    h.get_by_label("Layers").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.painting.panel, Panel::Layers);
    h.get_by_label("Color").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.painting.panel, Panel::Color);
    h.get_by_label("Editor").click();
    h.run_steps(3);
    assert!(!h.state().ui.painting.enabled);
}

#[test]
fn painting_toolbar_fits_portrait_in_every_theme_and_moves_the_rail() {
    use egui_kittest::kittest::Queryable;
    for theme in crate::theme::ThemeKind::ALL {
        let mut h = harness(egui::vec2(390.0, 700.0), theme);
        let canvas = h.state().last_canvas_rect;
        assert!(canvas.width() > 280.0 && canvas.height() > 500.0, "{theme:?}: {canvas:?}");
        for label in ["Editor", "Paint", "Smudge", "Erase", "Layers", "Color", "Eyedropper"] {
            let bounds = h.get_by_label(label).rect();
            assert!(bounds.left() >= 0.0 && bounds.right() <= 390.0, "{theme:?}: {label}: {bounds:?}");
        }
        let ctx = h.ctx.clone();
        menus::invoke(h.state_mut(), &ctx, "window.paintingMode", json!({"rightHanded":true})).unwrap();
        h.run_steps(3);
        assert!(h.state().last_canvas_rect.left() < canvas.left());
        assert!(h.get_by_label("Eyedropper").rect().left() > 320.0);
    }
}

#[test]
fn painting_workspace_and_state_round_trip_keep_old_settings_compatible() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    let ctx = egui::Context::default();
    menus::invoke(&mut app, &ctx, "window.workspace.painting", Value::Null).unwrap();
    assert!(app.ui.painting.enabled);
    menus::invoke(&mut app, &ctx, "window.workspace.essentials", Value::Null).unwrap();
    assert!(!app.ui.painting.enabled);
    menus::invoke(&mut app, &ctx, "window.paintingMode", json!({"enabled":true,"panel":"color","rightHanded":true})).unwrap();
    let saved = serde_json::to_value(&app.ui).unwrap();
    let restored: crate::state::UiState = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(restored.painting, app.ui.painting);
    let mut old = saved;
    old.as_object_mut().unwrap().remove("painting");
    let restored: crate::state::UiState = serde_json::from_value(old).unwrap();
    assert!(!restored.painting.enabled);
    let (req, _) = crate::control::ControlRequest::new("ui.inspect", json!({}));
    let crate::control::Outcome::Done(result) = crate::control::handle(&mut app, &ctx, &req) else { panic!("inspect must finish") };
    assert_eq!(result["result"]["paintingMode"]["panel"], "color");
}

#[test]
fn painting_panel_changes_do_not_replace_the_eyedropper() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    let ctx = egui::Context::default();
    menus::invoke(&mut app, &ctx, "window.paintingMode", json!({"enabled":true})).unwrap();
    menus::invoke(&mut app, &ctx, "tools.select.eyedropper", Value::Null).unwrap();
    menus::invoke(&mut app, &ctx, "window.paintingMode", json!({"panel":"color"})).unwrap();
    assert_eq!(app.ui.tool, crate::state::Tool::Eyedropper);
}

fn android_contact(h: &mut egui_kittest::Harness<'static, PhotocraftApp>, id: u64, phase: egui::TouchPhase, p: egui::Pos2, force: Option<f32>) {
    let mut events = vec![egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(id), phase, pos: p, force }];
    // egui-winit emulates its pointer from the first contact only.
    if id == 0 {
        if matches!(phase, egui::TouchPhase::Start | egui::TouchPhase::Move) {
            events.push(egui::Event::PointerMoved(p));
        }
        if matches!(phase, egui::TouchPhase::Start | egui::TouchPhase::End) {
            events.push(egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: phase == egui::TouchPhase::Start,
                modifiers: egui::Modifiers::NONE,
            });
        }
        if matches!(phase, egui::TouchPhase::End | egui::TouchPhase::Cancel) {
            events.push(egui::Event::PointerGone);
        }
    }
    let mut raw = egui::RawInput { events, focused: true, ..Default::default() };
    let ctx = h.ctx.clone();
    h.state_mut().android_touch_input(&ctx, &mut raw);
    h.input_mut().events.extend(raw.events);
    h.step();
}

#[test]
fn painting_mode_keeps_existing_touch_navigation_and_pen_painting() {
    use egui::TouchPhase::{End, Move, Start};
    use egui::vec2;
    let mut h = harness(vec2(1024.0, 768.0), crate::theme::ThemeKind::Studio);
    h.state_mut().ui.tool = crate::state::Tool::Brush;
    h.state_mut().run("prefs.set", json!({"path":"enhancedControls.paintWithFinger", "value":false})).unwrap();
    let a = h.state().last_canvas_rect.center();
    let revision = h.state().session.active().unwrap().revision;
    android_contact(&mut h, 0, Start, a, None);
    android_contact(&mut h, 0, Move, a + vec2(20.0, 20.0), None);
    android_contact(&mut h, 0, End, a + vec2(20.0, 20.0), None);
    assert_eq!(h.state().session.active().unwrap().revision, revision, "finger must not paint");
    assert!(h.state().drag.is_none());
    let view = h.state().ui.views[0].clone();
    android_contact(&mut h, 0, Start, a - vec2(80.0, 0.0), None);
    android_contact(&mut h, 1, Start, a + vec2(80.0, 0.0), None);
    android_contact(&mut h, 1, Move, a + vec2(120.0, 60.0), None);
    let after = &h.state().ui.views[0];
    assert!(after.zoom > view.zoom, "pinch zooms");
    assert!(after.rotation.abs() > 1.0, "twist rotates independently of trackpad preference");
    assert_ne!(after.center, view.center, "centroid pans");
    android_contact(&mut h, 1, End, a + vec2(120.0, 60.0), None);
    android_contact(&mut h, 0, Move, a, None);
    android_contact(&mut h, 0, End, a, None);
    assert_eq!(h.state().session.active().unwrap().revision, revision, "gesture including last finger leaves no marks");
    android_contact(&mut h, 0, Start, a, Some(0.7));
    android_contact(&mut h, 0, Move, a + vec2(40.0, 20.0), Some(0.7));
    android_contact(&mut h, 0, End, a + vec2(40.0, 20.0), Some(0.0));
    assert!(h.state().session.active().unwrap().revision > revision, "pen must still paint");
}

#[test]
fn painting_rail_updates_brush_commands_and_smudge_strength_without_painting() {
    use egui::accesskit::Role;
    use egui_kittest::kittest::Queryable;
    let mut h = harness(egui::vec2(1024.0, 768.0), crate::theme::ThemeKind::Studio);
    let revision = h.state().session.active().unwrap().revision;
    let size = h.query_all_by_role(Role::Slider).next().unwrap().rect();
    click(&mut h, size.center());
    h.run_steps(3);
    assert!(h.state().session.tools.brush.size > 40.0);
    assert_eq!(h.state().session.journal.last().unwrap().0, "tools.setBrush");
    let opacity = h.query_all_by_role(Role::Slider).nth(1).unwrap().rect();
    click(&mut h, opacity.center());
    h.run_steps(3);
    assert!((h.state().session.tools.brush.opacity - 0.5).abs() < 0.1);
    h.get_by_label("Smudge").click();
    h.run_steps(3);
    let strength = h.query_all_by_role(Role::Slider).nth(1).unwrap().rect();
    click(&mut h, egui::pos2(strength.center().x, strength.top() + 20.0));
    h.run_steps(3);
    assert!(h.state().ui.tool_options.strength > 70.0);
    assert_eq!(h.state().session.active().unwrap().revision, revision, "chrome clicks never paint");
}

fn click(h: &mut egui_kittest::Harness<'_, PhotocraftApp>, pos: egui::Pos2) {
    h.hover_at(pos);
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        h.run_steps(1);
    }
}

fn app_harness(integrated_titlebar: bool) -> egui_kittest::Harness<'static, PhotocraftApp> {
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(800.0, 600.0)).with_max_steps(64).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, crate::theme::ThemeKind::Studio);
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width":400,"height":300})).unwrap();
        app.run("shape.create", json!({"kind":"rect","rect":[100,100,100,100],"fill":"#ff0000"})).unwrap();
        app.integrated_titlebar = integrated_titlebar;
        app.set_theme(&cc.egui_ctx, crate::theme::ThemeKind::Studio);
        app.ui.painting.enabled = true;
        app
    });
    h.run_steps(4);
    h
}

#[test]
fn painting_keeps_native_caption_space_and_editor_return_in_the_real_app() {
    use egui_kittest::kittest::Queryable;
    let plain = app_harness(false);
    let mut integrated = app_harness(true);
    assert!(integrated.get_by_label("Editor").rect().top() > plain.get_by_label("Editor").rect().top() + 20.0);
    let panels = serde_json::to_value(&integrated.state().ui.panels).unwrap();
    assert!(integrated.query_by_label("Properties").is_none(), "the editor's floating Properties stays hidden in painting mode");
    integrated.get_by_label("Editor").click();
    integrated.run_steps(3);
    assert!(integrated.query_by_label("Editor").is_none());
    assert_eq!(serde_json::to_value(&integrated.state().ui.panels).unwrap(), panels);
    assert!(integrated.query_all_by_label("Properties").next().is_some());
}

#[test]
fn painting_transform_has_touch_accessible_cancel_and_apply() {
    use egui_kittest::kittest::Queryable;
    let mut h = app_harness(false);
    let revision = h.state().session.active().unwrap().revision;
    h.get_by_label("Transform").click();
    h.run_steps(3);
    assert!(h.state().ui.transform.is_some());
    for point in &mut h.state_mut().ui.transform.as_mut().unwrap().quad {
        point[0] += 20.0;
    }
    assert!(h.query_by_label("Apply").is_some());
    h.get_by_label("Cancel").click();
    h.run_steps(3);
    assert!(h.state().ui.transform.is_none());
    assert_eq!(h.state().session.active().unwrap().revision, revision);
    assert!(h.query_by_label("Editor").is_some());
}

#[test]
fn a_custom_workspace_exits_painting_mode() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    let ctx = egui::Context::default();
    menus::invoke(&mut app, &ctx, "window.workspace.newWorkspace", json!({"name":"My editor"})).unwrap();
    menus::invoke(&mut app, &ctx, "window.workspace.painting", Value::Null).unwrap();
    assert!(app.ui.painting.enabled);
    menus::invoke(&mut app, &ctx, "window.workspace.select", json!({"name":"My editor"})).unwrap();
    assert!(!app.ui.painting.enabled);
}

#[test]
fn visible_mode_buttons_round_trip_without_changing_the_document_or_panels() {
    use egui_kittest::kittest::Queryable;
    let mut h = app_harness(false);
    h.get_by_label("Editor").click();
    h.run_steps(3);
    let panels = serde_json::to_value(&h.state().ui.panels).unwrap();
    let revision = h.state().session.active().unwrap().revision;
    h.get_by_label("Painting").click();
    h.run_steps(3);
    assert!(h.state().ui.painting.enabled);
    h.get_by_label("Editor").click();
    h.run_steps(3);
    assert!(!h.state().ui.painting.enabled);
    assert_eq!(serde_json::to_value(&h.state().ui.panels).unwrap(), panels);
    assert_eq!(h.state().session.active().unwrap().revision, revision);
}

#[test]
fn painting_refinement_enters_full_screen_without_tabs_and_restores_editor_mode() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    let ctx = egui::Context::default();
    menus::invoke(&mut app, &ctx, "window.paintingMode", json!({"enabled":true})).unwrap();
    assert_eq!(app.ui.view.screen_mode, "fullScreenWithMenuBar");
    assert!(app.ui.view.hides_tabs() && !app.ui.view.shows_scrollbars());
    menus::invoke(&mut app, &ctx, "window.paintingMode", json!({"panel":"color"})).unwrap();
    menus::invoke(&mut app, &ctx, "window.paintingMode", json!({"enabled":false})).unwrap();
    assert_eq!(app.ui.view.screen_mode, "standard");
    menus::invoke(&mut app, &ctx, "window.workspace.painting", Value::Null).unwrap();
    assert_eq!(app.ui.view.screen_mode, "fullScreenWithMenuBar");
    menus::invoke(&mut app, &ctx, "window.workspace.essentials", Value::Null).unwrap();
    assert_eq!(app.ui.view.screen_mode, "standard");
    let h = app_harness(false);
    assert!(h.state().tab_strip.is_none());
}

#[test]
fn painting_refinement_eyedropper_toggles_and_returns_to_brush_on_pen_lift() {
    use crate::state::Tool;
    use egui_kittest::kittest::Queryable;
    let mut h = harness(egui::vec2(1024.0, 768.0), crate::theme::ThemeKind::Studio);
    h.get_by_label("Eyedropper").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.tool, Tool::Eyedropper);
    h.get_by_label("Eyedropper").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.tool, Tool::Brush);
    h.get_by_label("Eyedropper").click();
    h.run_steps(3);
    let revision = h.state().session.active().unwrap().revision;
    let at = h.state().last_canvas_rect.center();
    android_contact(&mut h, 0, egui::TouchPhase::Start, at, Some(0.7));
    h.run_steps(40); // A held pen must sample immediately, even without a drag or click.
    assert_eq!(h.state().session.tools.foreground, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(h.state().ui.tool, Tool::Eyedropper);
    android_contact(&mut h, 0, egui::TouchPhase::End, at, Some(0.0));
    h.run_steps(3);
    assert_eq!(h.state().ui.tool, Tool::Brush);
    assert_eq!(h.state().session.active().unwrap().revision, revision);
    assert!(h.state().drag.is_none());
}

#[test]
fn painting_refinement_lasso_has_a_deselect_button() {
    use egui_kittest::kittest::Queryable;
    let mut h = app_harness(false);
    h.state_mut().run("select.rect", json!({"x":20,"y":20,"width":100,"height":100})).unwrap();
    h.get_by_label("Selection").click();
    h.run_steps(3);
    h.get_by_label("Deselect").click();
    h.run_steps(3);
    assert!(h.state().session.active().unwrap().doc.selection.is_none());
}

#[test]
fn painting_refinement_compact_chrome_has_instant_wide_sliders() {
    use egui::{accesskit::Role, vec2};
    use egui_kittest::kittest::Queryable;
    let mut h = harness(vec2(1024.0, 768.0), crate::theme::ThemeKind::Studio);
    assert!(h.state().last_canvas_rect.left() <= 68.0, "52-point rail plus harness/central margins: {:?}", h.state().last_canvas_rect);
    assert!(h.get_by_label("Editor").rect().bottom() <= 48.0);
    let slider = h.query_all_by_role(Role::Slider).nth(1).unwrap().rect();
    assert!(slider.width() >= 42.0);
    let at = egui::pos2(slider.left() + 2.0, slider.center().y);
    h.hover_at(at);
    h.run_steps(1);
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.run_steps(1);
    assert!((h.state().session.tools.brush.opacity - 0.5).abs() < 0.1, "press responds before movement or release");
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run_steps(2);
}

#[test]
fn painting_brush_order_unlocks_for_drag_and_relocks_for_scrolling() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness(egui::vec2(1024.0, 768.0), crate::theme::ThemeKind::Studio);
    h.get_by_label("Paint").click();
    h.run_steps(3);
    assert!(h.query_by_label("Close").is_none());
    h.get_by_label("Unlock brush order").click();
    h.run_steps(3);
    assert!(h.query_by_label("Lock brush order").is_some());
    let from = h.get_by_label("Hard Round").rect().center();
    let to = h.get_by_label("Hard Round Fine").rect();
    let to = egui::pos2(to.center().x, to.bottom() - 5.0);
    h.hover_at(from);
    h.run_steps(1);
    h.drag_at(from);
    h.run_steps(2);
    for k in 1..=6 {
        h.hover_at(from + (to - from) * (k as f32 / 6.0));
        h.run_steps(2);
    }
    h.drop_at(to);
    h.run_steps(3);
    let names: Vec<_> = h.state().session.tools.presets.iter().map(|p| p.name.as_str()).collect();
    assert!(names.iter().position(|n| *n == "Hard Round").unwrap() > names.iter().position(|n| *n == "Hard Round Fine").unwrap());
    assert!(h.state().session.journal.iter().any(|e| e.0 == "brush.presets.move"));
    h.get_by_label("Lock brush order").click();
    h.run_steps(3);
    assert!(h.query_by_label("Unlock brush order").is_some());
    h.get_by_label("Paint").click();
    h.run_steps(3);
    h.get_by_label("Erase").click();
    h.run_steps(3);
    h.get_by_label("Erase").click();
    h.run_steps(3);
    assert!(h.query_by_label("Close").is_none());
    assert!(h.query_by_label("Unlock brush order").is_some());
    h.get_by_label("Erase").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.painting.panel, crate::painting_mode::Panel::None);
}

#[test]
fn brush_order_control_is_atomic_and_defaults_to_locked() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    let ctx = egui::Context::default();
    let old: crate::brush_panel::BrushesPanelState = serde_json::from_value(json!({})).unwrap();
    assert!(!old.reorder);
    let (req, _) = crate::control::ControlRequest::new("ui.set", json!({"brushesReorder":true,"brushPickerReorder":"yes"}));
    let crate::control::Outcome::Done(result) = crate::control::handle(&mut app, &ctx, &req) else { panic!("synchronous control") };
    assert_eq!(result["ok"], false);
    assert!(!app.ui.brushes_panel.reorder && !app.ui.brush_picker_list.reorder);
    let (req, _) = crate::control::ControlRequest::new("ui.set", json!({"brushesReorder":true,"brushPickerReorder":true,"brushPicker":[120,80]}));
    let crate::control::Outcome::Done(result) = crate::control::handle(&mut app, &ctx, &req) else { panic!("synchronous control") };
    assert_eq!(result["ok"], true);
    assert!(app.ui.brushes_panel.reorder && app.ui.brush_picker_list.reorder);
    let (req, _) = crate::control::ControlRequest::new("ui.inspect", json!({}));
    let crate::control::Outcome::Done(result) = crate::control::handle(&mut app, &ctx, &req) else { panic!("synchronous control") };
    assert_eq!(result["result"]["brushesPanel"]["reorder"], true);
    assert_eq!(result["result"]["brushPicker"]["list"]["reorder"], true);
}
