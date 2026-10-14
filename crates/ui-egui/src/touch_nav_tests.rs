use super::*;
use egui::{Rect, pos2, vec2};

fn events(id: u64, phase: TouchPhase, pos: Pos2, pen: bool) -> Vec<Event> {
    let mut events = vec![Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(id), phase, pos, force: pen.then_some(0.0) }];
    if id == 0 {
        if matches!(phase, TouchPhase::Start | TouchPhase::Move) {
            events.push(Event::PointerMoved(pos));
        }
        if matches!(phase, TouchPhase::Start | TouchPhase::End) {
            events.push(Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: phase == TouchPhase::Start,
                modifiers: egui::Modifiers::NONE,
            });
        }
        if matches!(phase, TouchPhase::End | TouchPhase::Cancel) {
            events.push(Event::PointerGone);
        }
    }
    events
}
fn frame(nav: &mut TouchNavigation, events: Vec<Event>, paint: bool, gestures: bool) -> RawInput {
    let mut raw = RawInput { events, focused: true, ..Default::default() };
    nav.process(&mut raw, paint, gestures, 0.0, |p| p.x >= 100.0);
    raw
}
#[test]
fn finger_off_filters_canvas_but_keeps_ui_and_zero_pressure_pen() {
    let mut nav = TouchNavigation::default();
    for phase in [TouchPhase::Start, TouchPhase::Move, TouchPhase::End] {
        let raw = frame(&mut nav, events(0, phase, pos2(200.0, 200.0), false), false, true);
        assert!(raw.events.is_empty());
    }
    let ui = events(0, TouchPhase::Start, pos2(20.0, 20.0), false);
    assert_eq!(frame(&mut nav, ui.clone(), false, true).events, ui);
    frame(&mut nav, events(0, TouchPhase::End, pos2(20.0, 20.0), false), false, true);
    let pen = events(0, TouchPhase::Start, pos2(200.0, 200.0), true);
    assert_eq!(frame(&mut nav, pen.clone(), false, true).events, pen);
    assert!(!nav.block_tools);
}
#[test]
fn finger_on_paints_but_two_fingers_cancel_and_own_the_last_finger() {
    let mut nav = TouchNavigation::default();
    let first = frame(&mut nav, events(0, TouchPhase::Start, pos2(200.0, 200.0), false), true, true);
    assert!(first.events.iter().any(|e| matches!(e, Event::PointerButton { pressed: true, .. })));
    let second = frame(&mut nav, events(1, TouchPhase::Start, pos2(300.0, 200.0), false), true, true);
    assert!(nav.block_tools);
    assert!(second.events.iter().any(|e| matches!(e, Event::PointerButton { pressed: false, .. })));
    frame(&mut nav, events(1, TouchPhase::End, pos2(300.0, 200.0), false), true, true);
    assert!(frame(&mut nav, events(0, TouchPhase::Move, pos2(220.0, 200.0), false), true, true).events.is_empty());
    assert!(frame(&mut nav, events(0, TouchPhase::End, pos2(220.0, 200.0), false), true, true).events.is_empty());
    assert!(!frame(&mut nav, events(0, TouchPhase::Start, pos2(200.0, 200.0), false), true, true).events.is_empty());
}
#[test]
fn two_fingers_pan_zoom_rotate_and_contact_changes_do_not_jump() {
    let mut nav = TouchNavigation::default();
    let mut start = events(0, TouchPhase::Start, pos2(200.0, 200.0), false);
    start.extend(events(1, TouchPhase::Start, pos2(300.0, 200.0), false));
    frame(&mut nav, start, false, true);
    let mut moved = events(0, TouchPhase::Move, pos2(300.0, 100.0), false);
    moved.extend(events(1, TouchPhase::Move, pos2(300.0, 300.0), false));
    frame(&mut nav, moved, false, true);
    let g = nav.gesture.unwrap();
    assert_eq!(g.from, pos2(250.0, 200.0));
    assert_eq!(g.to, pos2(300.0, 200.0));
    assert!((g.zoom - 2.0).abs() < 1e-6);
    assert!((g.rotation - 90.0).abs() < 1e-5);
    frame(&mut nav, events(2, TouchPhase::Start, pos2(400.0, 200.0), false), false, true);
    assert!(nav.gesture.is_none());
    frame(&mut nav, events(2, TouchPhase::Cancel, pos2(400.0, 200.0), false), false, true);
    assert!(nav.gesture.is_none());
}
#[test]
fn gesture_preserves_anchor_with_rotation_flip_aspect_and_display_scale() {
    for flip in [false, true] {
        let mut view = crate::state::View { zoom: 2.0, center: [200.0, 150.0], rotation: 30.0, doc_size: [400, 300], ..Default::default() };
        let xf = crate::canvas::ViewXform {
            rect: Rect::from_min_size(pos2(10.0, 20.0), vec2(800.0, 600.0)),
            zoom: 1.0,
            center: view.center,
            rotation: view.rotation,
            flip,
            aspect: 1.5,
        };
        let g = Gesture { from: pos2(200.0, 250.0), to: pos2(250.0, 300.0), zoom: 1.5, rotation: 45.0 };
        let anchor = xf.to_doc(g.from);
        apply(&mut view, xf, 2.0, g);
        let after = crate::canvas::ViewXform { zoom: view.zoom / 2.0, center: view.center, rotation: view.rotation, ..xf };
        assert!(after.to_screen(anchor[0] as f32, anchor[1] as f32).distance(g.to) < 0.001);
        assert!(!view.fit_pending);
    }
}
#[test]
fn ui_origin_never_becomes_canvas_gesture_and_gestures_can_be_disabled() {
    let mut nav = TouchNavigation::default();
    frame(&mut nav, events(0, TouchPhase::Start, pos2(20.0, 200.0), false), false, true);
    let raw = frame(&mut nav, events(0, TouchPhase::Move, pos2(200.0, 200.0), false), false, true);
    assert!(!raw.events.is_empty());
    frame(&mut nav, events(1, TouchPhase::Start, pos2(300.0, 200.0), false), false, true);
    assert!(nav.gesture.is_none());
    let mut nav = TouchNavigation::default();
    frame(&mut nav, events(0, TouchPhase::Start, pos2(200.0, 200.0), false), false, false);
    frame(&mut nav, events(1, TouchPhase::Start, pos2(300.0, 200.0), false), false, false);
    frame(&mut nav, events(1, TouchPhase::Move, pos2(400.0, 200.0), false), false, false);
    assert!(nav.gesture.is_none());
}

#[test]
fn start_and_cancel_in_one_batch_never_forward_a_press() {
    for pen in [false, true] {
        let mut nav = TouchNavigation::default();
        let mut input = events(0, TouchPhase::Start, pos2(200.0, 200.0), pen);
        input.extend(events(0, TouchPhase::Cancel, pos2(200.0, 200.0), pen));
        let raw = frame(&mut nav, input, true, true);
        assert!(!raw.events.iter().any(|e| matches!(e, Event::PointerButton { pressed: true, .. })));
        assert!(nav.block_tools);
    }
}

fn timed(nav: &mut TouchNavigation, events: Vec<Event>, time: f64) -> bool {
    let mut raw = RawInput { events, focused: true, time: Some(time), ..Default::default() };
    nav.process(&mut raw, false, true, time, |p| p.x >= 100.0);
    nav.reset_view
}

fn tap_at(nav: &mut TouchNavigation, pos: Pos2, time: f64) -> bool {
    assert!(!timed(nav, events(0, TouchPhase::Start, pos, false), time));
    timed(nav, events(0, TouchPhase::End, pos, false), time + 0.04)
}

#[test]
fn double_tap_is_nearby_quick_and_consumes_each_pair_once() {
    let mut nav = TouchNavigation::default();
    let a = pos2(200.0, 200.0);
    assert!(!tap_at(&mut nav, a, 1.0));
    assert!(tap_at(&mut nav, a + vec2(12.0, 0.0), 1.15));
    assert!(!tap_at(&mut nav, a, 1.3), "third tap is not another reset");
    assert!(tap_at(&mut nav, a, 1.45));
    assert!(!tap_at(&mut nav, a, 2.0));
    assert!(!tap_at(&mut nav, a + vec2(80.0, 0.0), 2.15), "distant taps are separate");
    assert!(!tap_at(&mut nav, a + vec2(80.0, 0.0), 2.8), "slow taps are separate");
}

#[test]
fn double_tap_rejects_drags_long_presses_and_cancelled_contacts() {
    let a = pos2(200.0, 200.0);
    for interruption in [TouchPhase::Move, TouchPhase::Cancel, TouchPhase::End] {
        let mut nav = TouchNavigation::default();
        assert!(!tap_at(&mut nav, a, 1.0));
        timed(&mut nav, events(0, TouchPhase::Start, a, false), 1.1);
        let (pos, time) = if interruption == TouchPhase::Move { (a + vec2(50.0, 0.0), 1.15) } else { (a, 1.5) };
        assert!(!timed(&mut nav, events(0, interruption, pos, false), time));
        // Moving back to the original position doesn't turn a drag into a tap.
        assert!(!timed(&mut nav, events(0, TouchPhase::End, a, false), time + 0.01));
        assert!(!tap_at(&mut nav, a, time + 0.1));
    }
}

#[test]
fn double_tap_rejects_two_fingers_and_pen_or_ui_interruption() {
    let a = pos2(200.0, 200.0);
    for (id, pos, pen) in [(1, a + vec2(80.0, 0.0), false), (1, a, true), (1, pos2(20.0, 20.0), false)] {
        let mut nav = TouchNavigation::default();
        assert!(!tap_at(&mut nav, a, 1.0));
        timed(&mut nav, events(0, TouchPhase::Start, a, false), 1.1);
        timed(&mut nav, events(id, TouchPhase::Start, pos, pen), 1.12);
        assert!(!timed(&mut nav, events(id, TouchPhase::End, pos, pen), 1.14));
        assert!(!timed(&mut nav, events(0, TouchPhase::End, a, false), 1.16));
        assert!(!tap_at(&mut nav, a, 1.25));
    }
}

#[test]
fn double_tap_resets_on_focus_document_and_preference_changes() {
    let a = pos2(200.0, 200.0);
    for interruption in 0..4 {
        let mut nav = TouchNavigation::default();
        nav.document(Some(1));
        assert!(!tap_at(&mut nav, a, 1.0));
        match interruption {
            0 => {
                let mut raw = RawInput { focused: false, ..Default::default() };
                nav.process(&mut raw, false, true, 1.1, |_| true);
            }
            1 => nav.document(Some(2)),
            2 => {
                frame(&mut nav, vec![], true, true);
            }
            _ => {
                frame(&mut nav, vec![], false, false);
            }
        }
        assert!(!tap_at(&mut nav, a, 1.2));
    }
}

#[test]
fn double_tap_requires_same_device_and_valid_monotonic_time() {
    let a = pos2(200.0, 200.0);
    for time in [0.5, f64::NAN, f64::INFINITY] {
        let mut nav = TouchNavigation::default();
        assert!(!tap_at(&mut nav, a, 1.0));
        assert!(!tap_at(&mut nav, a, time));
    }
    let mut nav = TouchNavigation::default();
    assert!(!tap_at(&mut nav, a, 1.0));
    for phase in [TouchPhase::Start, TouchPhase::End] {
        let mut other = events(0, phase, a, false);
        for event in &mut other {
            if let Event::Touch { device_id, .. } = event {
                *device_id = egui::TouchDeviceId(2);
            }
        }
        assert!(!timed(&mut nav, other, 1.2));
    }
}

#[test]
fn double_tap_handles_coalesced_frames_without_pointer_events_or_marks() {
    let mut nav = TouchNavigation::default();
    let a = pos2(200.0, 200.0);
    let mut input = vec![];
    for _ in 0..2 {
        input.extend(events(0, TouchPhase::Start, a, false));
        input.extend(events(0, TouchPhase::End, a, false));
    }
    let raw = frame(&mut nav, input, false, true);
    assert!(nav.reset_view);
    assert!(raw.events.is_empty());
    assert!(nav.block_tools);
}
