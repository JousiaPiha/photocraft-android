//! Android's visible content rectangle excludes the system bars and soft keyboard.
pub fn content_rect([left, top, right, bottom]: [i32; 4], scale: f32) -> Option<egui::Rect> {
    if !scale.is_finite() || scale <= 0.0 || right <= left || bottom <= top {
        return None;
    }
    let rect = egui::Rect::from_min_max(egui::pos2(left as f32 / scale, top as f32 / scale), egui::pos2(right as f32 / scale, bottom as f32 / scale));
    rect.is_finite().then_some(rect)
}

pub fn surface_input(input: &mut egui::RawInput, bounds: [i32; 4], scale: f32) {
    let Some(rect) = content_rect(bounds, scale) else { return };
    input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, rect.size()));
    // The NativeActivity input queue reports window coordinates, while wgpu's
    // SurfaceView starts below the system caption (and any other decor insets).
    let offset = rect.min.to_vec2();
    for event in &mut input.events {
        match event {
            egui::Event::PointerMoved(pos) | egui::Event::PointerButton { pos, .. } | egui::Event::Touch { pos, .. } => *pos -= offset,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ime_rectangle_keeps_dialogs_above_the_keyboard() {
        let full = content_rect([0, 48, 2560, 1552], 2.0).unwrap();
        let ime = content_rect([0, 48, 2560, 740], 2.0).unwrap();
        assert_eq!(full.min, egui::pos2(0.0, 24.0));
        assert_eq!(ime.max, egui::pos2(1280.0, 370.0));
        assert!(ime.height() < full.height());
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MIN_POSITIVE] {
            assert!(content_rect([0, 0, 2560, 1600], scale).is_none());
        }
        assert!(content_rect([0, 0, 0, 0], 2.0).is_none());
    }
    #[test]
    fn child_surface_input_excludes_caption_and_aligns_pointer_and_touch() {
        let mut input = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(egui::pos2(50.0, 80.0)),
                egui::Event::PointerButton { pos: egui::pos2(50.0, 80.0), button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
                egui::Event::Touch {
                    device_id: egui::TouchDeviceId(1),
                    id: egui::TouchId(1),
                    phase: egui::TouchPhase::Start,
                    pos: egui::pos2(50.0, 80.0),
                    force: Some(0.7),
                },
                egui::Event::MouseMoved(egui::vec2(2.0, 3.0)),
            ],
            ..Default::default()
        };
        surface_input(&mut input, [10, 70, 1010, 1470], 2.0);
        assert_eq!(input.screen_rect, Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 700.0))));
        assert_eq!(input.events[0], egui::Event::PointerMoved(egui::pos2(45.0, 45.0)));
        assert!(matches!(input.events[1], egui::Event::PointerButton { pos, pressed: true, .. } if pos == egui::pos2(45.0, 45.0)));
        assert!(matches!(input.events[2], egui::Event::Touch { pos, force: Some(0.7), .. } if pos == egui::pos2(45.0, 45.0)));
        assert_eq!(input.events[3], egui::Event::MouseMoved(egui::vec2(2.0, 3.0)));
    }

    #[test]
    fn invalid_surface_bounds_leave_input_unchanged() {
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::PointerMoved(egui::pos2(50.0, 80.0)));
        surface_input(&mut input, [0; 4], 2.0);
        surface_input(&mut input, [0, 70, 1000, 1400], f32::NAN);
        assert!(input.screen_rect.is_none());
        assert_eq!(input.events, vec![egui::Event::PointerMoved(egui::pos2(50.0, 80.0))]);
    }
}
