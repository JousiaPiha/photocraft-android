//! Canvas-focused painting chrome. Document edits and input stay on the normal engine/canvas path.
use egui::{Align, Layout, RichText, Stroke, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{PhotocraftApp, brush_panel, icons, menus, state::Tool, theme::Tokens};

pub const COMMAND: &str = "window.paintingMode";
const BUTTON: f32 = 36.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Panel {
    #[default]
    None,
    Brushes,
    Layers,
    Color,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PaintingMode {
    pub enabled: bool,
    pub right_handed: bool,
    pub panel: Panel,
    pub previous_screen_mode: Option<String>,
    #[serde(skip)]
    pending_screen_mode: Option<String>,
    #[serde(skip)]
    pub sampling: bool,
}

/// The painting workspace and the direct toggle share reversible screen-mode state.
pub fn set_enabled(app: &mut PhotocraftApp, enabled: bool) {
    if enabled != app.ui.painting.enabled {
        let next = if enabled {
            app.ui.painting.previous_screen_mode = Some(app.ui.view.screen_mode.clone());
            "fullScreenWithMenuBar".to_string()
        } else {
            app.ui.painting.previous_screen_mode.take().unwrap_or_else(|| "standard".into())
        };
        app.ui.view.screen_mode = next.clone();
        app.ui.painting.pending_screen_mode = Some(next);
    }
    app.ui.painting.enabled = enabled;
    app.ui.painting.panel = Panel::None;
    app.ui.painting.sampling = false;
}

/// Apply native fullscreen changes through the existing view command path.
pub fn sync_screen_mode(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if let Some(mode) = app.ui.painting.pending_screen_mode.take() {
        invoke(app, ctx, &format!("view.screenMode.{mode}"), Value::Null);
    }
}

/// Empty/null params toggle; a nonempty patch only changes its named fields.
/// Validate the entire request before changing any state.
pub fn command(app: &mut PhotocraftApp, params: Value) -> Result<Value, String> {
    if !params.is_null() && !params.is_object() {
        return Err("painting mode parameters must be an object".into());
    }
    #[derive(Default, Deserialize)]
    #[serde(default, deny_unknown_fields, rename_all = "camelCase")]
    struct Patch {
        enabled: Option<bool>,
        right_handed: Option<bool>,
        panel: Option<Panel>,
    }
    let toggle = params.is_null() || params.as_object().is_some_and(|p| p.is_empty());
    let patch: Patch = serde_json::from_value(if params.is_null() { json!({}) } else { params }).map_err(|e| e.to_string())?;
    let was_enabled = app.ui.painting.enabled;
    if let Some(enabled) = patch.enabled.or(toggle.then_some(!was_enabled)) {
        set_enabled(app, enabled);
    }
    let state = &mut app.ui.painting;
    if let Some(side) = patch.right_handed {
        state.right_handed = side;
    }
    if let Some(panel) = patch.panel {
        state.panel = panel;
    }
    if state.enabled && !was_enabled && !app.ui.tool.is_brushlike() {
        app.ui.tool = Tool::Brush;
    }
    crate::paint_mouse::sync_tool_brush(app);
    serde_json::to_value(&app.ui.painting).map_err(|e| e.to_string())
}

fn invoke(app: &mut PhotocraftApp, ctx: &egui::Context, id: &str, params: Value) {
    if let Err(error) = menus::invoke(app, ctx, id, params) {
        crate::notices::error(app, error);
    }
}

fn toggle_panel(app: &mut PhotocraftApp, ctx: &egui::Context, panel: Panel) {
    let next = if app.ui.painting.panel == panel { Panel::None } else { panel };
    invoke(app, ctx, COMMAND, json!({"panel": next}));
}

fn action(app: &mut PhotocraftApp, ui: &mut egui::Ui, id: &str, label: &str) {
    if ui.add_enabled(menus::is_enabled(app, id), egui::Button::new(tl!(label)).min_size(vec2(0.0, BUTTON))).clicked() {
        invoke(app, ui.ctx(), id, Value::Null);
        ui.close();
    }
}

fn command_button(app: &mut PhotocraftApp, ui: &mut egui::Ui, id: &str, icon: &str, label: &str) {
    if ui.add_enabled_ui(menus::is_enabled(app, id), |ui| icons::button(ui, icon, BUTTON, false, tl!(label))).inner.clicked() {
        invoke(app, ui.ctx(), id, Value::Null);
    }
}

fn tool_button(app: &mut PhotocraftApp, ui: &mut egui::Ui, tool: Tool, label: &str) {
    let selected = app.ui.tool == tool;
    if icons::button(ui, icons::tool_icon(tool), BUTTON, selected, tl!(label)).clicked() {
        if selected && matches!(tool, Tool::Brush | Tool::Smudge | Tool::Eraser) {
            toggle_panel(app, ui.ctx(), Panel::Brushes);
        } else {
            let tool = if selected && tool == Tool::Eyedropper { Tool::Brush } else { tool };
            app.ui.painting.sampling = false;
            invoke(app, ui.ctx(), &crate::tool_shortcuts::id(tool), Value::Null);
            crate::paint_mouse::sync_tool_brush(app);
        }
    }
}

fn tools(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    tool_button(app, ui, Tool::Brush, "Paint");
    tool_button(app, ui, Tool::Smudge, "Smudge");
    tool_button(app, ui, Tool::Eraser, "Erase");
    if icons::button(ui, "layers", BUTTON, app.ui.painting.panel == Panel::Layers, tl!("Layers")).clicked() {
        toggle_panel(app, ui.ctx(), Panel::Layers);
    }
    let color = app.session.tools.foreground;
    let chip = egui::Color32::from_rgba_unmultiplied((color[0] * 255.0) as u8, (color[1] * 255.0) as u8, (color[2] * 255.0) as u8, (color[3] * 255.0) as u8);
    let response = icons::button(ui, "palette", BUTTON, app.ui.painting.panel == Panel::Color, tl!("Color"));
    ui.painter().circle_filled(response.rect.center(), 12.0, chip);
    ui.painter().circle_stroke(response.rect.center(), 12.0, Stroke::new(1.5, Tokens::get(ui.ctx()).text_dim));
    if response.clicked() {
        toggle_panel(app, ui.ctx(), Panel::Color);
    }
}

fn actions(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    if app.ui.transform.is_some() {
        if ui.add(egui::Button::new(tl!("Cancel")).min_size(vec2(72.0, BUTTON))).clicked() {
            crate::transform_tool::cancel(app);
        }
        if ui.add(egui::Button::new(tl!("Apply")).min_size(vec2(72.0, BUTTON))).clicked() {
            crate::transform_tool::commit(app);
        }
        return;
    }
    if ui.add(egui::Button::new(tl!("Editor")).min_size(vec2(66.0, BUTTON))).on_hover_text(tl!("Return to editor")).clicked() {
        invoke(app, ui.ctx(), COMMAND, json!({"enabled": false}));
    }
    let button = icons::button(ui, "ellipsis", BUTTON, false, tl!("Actions"));
    egui::Popup::menu(&button).show(|ui| {
        ui.set_min_width(200.0);
        for (id, label) in [("file.new", "New…"), ("file.open", "Open…"), ("file.save", "Save"), ("file.export.exportAs", "Export As…")] {
            action(app, ui, id, label);
        }
        ui.separator();
        for (id, label) in
            [("layer.new.layer", "New Layer"), ("view.fitOnScreen", "Fit on Screen"), ("view.resetView", "Reset View"), ("edit.search", "Search Commands…")]
        {
            action(app, ui, id, label);
        }
        ui.separator();
        let mut right = app.ui.painting.right_handed;
        if ui.checkbox(&mut right, tl!("Right-hand sidebar")).changed() {
            invoke(app, ui.ctx(), COMMAND, json!({"rightHanded": right}));
        }
    });
    tool_button(app, ui, Tool::Lasso, "Selection");
    command_button(app, ui, "edit.freeTransform", "move", "Transform");
    if app.ui.tool == Tool::Lasso || app.session.active().is_some_and(|st| st.doc.selection.is_some()) {
        if ui.add_enabled(menus::is_enabled(app, "select.deselect"), egui::Button::new(tl!("Deselect")).min_size(vec2(0.0, BUTTON))).clicked() {
            invoke(app, ui.ctx(), "select.deselect", Value::Null);
        }
    } else {
        command_button(app, ui, "view.fitOnScreen", "maximize-2", "Fit on Screen");
    }
}

/// Called before the central canvas; rail space is reserved, never painted on top of the canvas.
pub fn chrome(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("painting-toolbar").frame(egui::Frame::NONE.fill(t.chrome).inner_margin(4)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let narrow = ui.available_width() < 560.0;
        ui.horizontal(|ui| {
            actions(app, ui);
            if !narrow {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    // Keep the familiar Paint / Smudge / Erase / Layers / Color reading order.
                    ui.allocate_ui_with_layout(vec2(196.0, BUTTON), Layout::left_to_right(Align::Center), |ui| tools(app, ui));
                });
            }
        });
        if narrow {
            ui.horizontal(|ui| tools(app, ui));
        }
    });
    let rail = if app.ui.painting.right_handed { egui::Panel::right("painting-rail") } else { egui::Panel::left("painting-rail") };
    rail.exact_size(52.0).resizable(false).frame(egui::Frame::NONE.fill(t.chrome).inner_margin(4)).show(ui, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("painting-rail-scroll")
            .scroll_source(egui::scroll_area::ScrollSource::MOUSE_WHEEL)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    let height = ((ui.available_height() - 260.0) / 2.0).clamp(48.0, 160.0);
                    let before = app.session.tools.brush.clone();
                    let mut brush = before.clone();
                    ui.add_enabled_ui(app.ui.tool.is_brushlike(), |ui| {
                        let size_label = ui.label(RichText::new(tl!("Size")).small().color(t.text_dim));
                        ui.label(format!("{:.0}", brush.size));
                        ui.spacing_mut().slider_width = height;
                        ui.spacing_mut().interact_size = vec2(44.0, 44.0);
                        ui.add(
                            egui::Slider::new(&mut brush.size, 1.0..=photocraft_engine::paint::MAX_BRUSH_SIZE)
                                .logarithmic(true)
                                .smart_aim(false)
                                .vertical()
                                .show_value(false),
                        )
                        .labelled_by(size_label.id)
                        .on_hover_text(tl!("Brush Size"));
                        ui.add_space(8.0);
                        if app.ui.tool == Tool::Smudge {
                            let label = ui.label(RichText::new(tl!("Strength")).small().color(t.text_dim));
                            ui.label(format!("{:.0}%", app.ui.tool_options.strength));
                            ui.add(egui::Slider::new(&mut app.ui.tool_options.strength, 1.0..=100.0).smart_aim(false).vertical().show_value(false))
                                .labelled_by(label.id)
                                .on_hover_text(tl!("Strength"));
                        } else {
                            let label = ui.label(RichText::new(tl!("Opacity")).small().color(t.text_dim));
                            ui.label(format!("{:.0}%", brush.opacity * 100.0));
                            ui.add(egui::Slider::new(&mut brush.opacity, 0.0..=1.0).smart_aim(false).vertical().show_value(false))
                                .labelled_by(label.id)
                                .on_hover_text(tl!("Opacity"));
                        }
                    });
                    brush_panel::commit_gesture(app, ui.ctx(), &before, &brush);
                    ui.add_space(8.0);
                    tool_button(app, ui, Tool::Eyedropper, "Eyedropper");
                    ui.separator();
                    command_button(app, ui, "edit.undo", "undo-2", "Undo");
                    command_button(app, ui, "edit.redo", "redo-2", "Redo");
                });
            });
    });
}

/// Reuse the editor's real brush presets, layers and color UI in a bounded overlay.
pub fn overlay(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let panel = app.ui.painting.panel;
    let title = match panel {
        Panel::None => return,
        Panel::Brushes => "Brushes",
        Panel::Layers => "Layers",
        Panel::Color => "Color",
    };
    let t = Tokens::get(ctx);
    let canvas = app.last_canvas_rect;
    let width = (canvas.width() - 24.0).clamp(220.0, 340.0);
    let height = (canvas.height() - 32.0).max(120.0);
    let mut open = true;
    egui::Window::new(tl!(title))
        .id(egui::Id::new("painting-panel"))
        .title_bar(false)
        .fixed_pos(egui::pos2((canvas.right() - width - 24.0).max(canvas.left()), canvas.top() + 8.0))
        .fixed_size(vec2(width, height.min(560.0)))
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.card).stroke(Stroke::new(1.0, t.card_border)).corner_radius(t.radius_lg).inner_margin(12))
        .show(ctx, |ui| {
            if panel != Panel::Brushes {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tl!(title)).strong().size(18.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icons::button(ui, "x", BUTTON, false, tl!("Close")).clicked() {
                            open = false;
                        }
                    });
                });
                ui.separator();
            }
            match panel {
                Panel::Brushes => {
                    ui.horizontal(|ui| {
                        let width = (ui.available_width() - BUTTON - ui.spacing().item_spacing.x).max(60.0);
                        ui.add_sized(vec2(width, BUTTON), egui::TextEdit::singleline(&mut app.ui.brushes_panel.filter).hint_text(tl!("Search Brushes")));
                        crate::brushes_tab::order_button(ui, &mut app.ui.brushes_panel, BUTTON);
                    });
                    let layout = crate::brushes_tab::ListLayout {
                        id: "painting-brushes",
                        max_height: (height - 116.0).clamp(80.0, 434.0),
                        ..crate::brushes_tab::PANEL_LIST
                    };
                    let acts = crate::brushes_tab::preset_list(
                        ui,
                        &app.session.tools.presets,
                        app.session.tools.current_preset.as_deref(),
                        &mut app.ui.brushes_panel,
                        layout,
                    );
                    crate::brushes_tab::apply(app, acts);
                    if ui.add(egui::Button::new(tl!("Brush Settings")).min_size(vec2(0.0, BUTTON))).clicked() {
                        invoke(app, ctx, "window.toggle.brushSettings", Value::Null);
                        open = false;
                    }
                }
                Panel::Layers => crate::panels::dock_body(app, ui, crate::dock::Group::Layers, 0),
                Panel::Color => {
                    egui::ScrollArea::vertical().id_salt("painting-color").show(ui, |ui| {
                        crate::panels::dock_body(app, ui, crate::dock::Group::Color, usize::from(!t.pro));
                        ui.separator();
                        crate::swatches_ui::panel(app, ui);
                    });
                }
                Panel::None => {}
            }
        });
    if !open {
        invoke(app, ctx, COMMAND, json!({"panel": "none"}));
    }
}
