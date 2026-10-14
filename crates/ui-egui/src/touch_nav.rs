//! Android canvas input. The pinned winit patch preserves pen/mouse force and
//! removes finger force; do not use this classifier on other platform backends.
//! Finger contacts belong to the surface where they began. A two-finger gesture
//! owns the whole contact sequence, including the final remaining finger.

use egui::{Event, Pos2, RawInput, TouchPhase};
use std::collections::BTreeMap;

type ContactId = (u64, u64);

#[path = "touch_tap.rs"]
mod tap;

#[derive(Clone, Copy)]
struct Contact {
    pos: Pos2,
    canvas: bool,
    finger: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Gesture {
    pub from: Pos2,
    pub to: Pos2,
    pub zoom: f32,
    pub rotation: f32,
}

type Pair = [(ContactId, Pos2); 2];

#[derive(Default)]
pub(crate) struct TouchNavigation {
    contacts: BTreeMap<ContactId, Contact>,
    previous: Option<Pair>,
    claimed: bool,
    forwarded_finger: Option<(ContactId, Pos2)>,
    forwarded_pen: Option<(ContactId, Pos2)>,
    taps: tap::TapSequence,
    pub reset_view: bool,
    pub block_tools: bool,
    pub gesture: Option<Gesture>,
}

impl TouchNavigation {
    pub fn document(&mut self, document: Option<u64>) {
        self.taps.document(document);
    }

    /// Return true when a finger's pending tool stroke must be cancelled.
    pub fn process(&mut self, raw: &mut RawInput, paint: bool, gestures: bool, time: f64, canvas_hit: impl Fn(Pos2) -> bool) -> bool {
        self.block_tools = false;
        self.gesture = None;
        self.reset_view = false;
        let taps_enabled = !paint && gestures && raw.focused;
        if !taps_enabled {
            self.taps.clear();
        }
        if !raw.focused {
            let release = self.forwarded_finger.take().or(self.forwarded_pen.take());
            let cancel = release.is_some();
            raw.events.retain(|e| !matches!(e, Event::PointerMoved(_) | Event::PointerButton { .. } | Event::PointerGone));
            if let Some((_, pos)) = release {
                release_pointer(raw, pos);
            }
            self.contacts.clear();
            self.previous = None;
            self.claimed = false;
            self.block_tools = cancel;
            return cancel;
        }
        let mut cancelled = None;
        let mut cancelled_ids = std::collections::BTreeSet::new();
        let mut pen_frame = self.contacts.values().any(|c| !c.finger);
        let mut sources = Vec::with_capacity(raw.events.len());
        let mut source = None;
        let mut claimed = self.claimed;
        let mut canvas_finger = self.contacts.values().any(|c| c.finger && c.canvas);
        for event in &raw.events {
            match event {
                Event::Touch { device_id, id, phase, pos, force } => {
                    let key = (device_id.0, id.0);
                    let contact = self.contacts.get(&key).copied().unwrap_or(Contact { pos: *pos, canvas: canvas_hit(*pos), finger: force.is_none() });
                    if taps_enabled {
                        self.reset_view |= self.taps.touch(
                            key,
                            *phase,
                            *pos,
                            time,
                            contact.finger && contact.canvas && canvas_hit(*pos),
                            self.contacts.is_empty() && !claimed,
                        );
                    }
                    source = Some((key, contact));
                    pen_frame |= !contact.finger;
                    if *phase == TouchPhase::Cancel {
                        if contact.canvas {
                            cancelled_ids.insert(key);
                        }
                        cancelled = self.forwarded_finger.or(self.forwarded_pen).filter(|(id, _)| *id == key).map(|(_, pos)| pos).or(cancelled);
                    }
                    canvas_finger |= contact.finger && contact.canvas;
                    match phase {
                        TouchPhase::Start | TouchPhase::Move if pos.is_finite() => {
                            // Bounded even for malformed synthetic input.
                            if self.contacts.len() < 32 || self.contacts.contains_key(&key) {
                                self.contacts.insert(key, Contact { pos: *pos, ..contact });
                            }
                        }
                        TouchPhase::End | TouchPhase::Cancel => {
                            self.contacts.remove(&key);
                        }
                        _ => {}
                    }
                    if gestures && self.contacts.values().filter(|c| c.finger && c.canvas).count() >= 2 {
                        claimed = true;
                    }
                }
                Event::PointerButton { pressed: true, .. } if source.is_none() => self.taps.clear(),
                Event::PointerMoved(_) | Event::PointerButton { .. } | Event::PointerGone => {}
                _ => {
                    source = None;
                    self.taps.clear();
                }
            }
            sources.push(source);
        }
        let blocked = !paint || claimed;
        let pen_down = self.contacts.values().any(|c| !c.finger);
        self.block_tools = canvas_finger && blocked && !pen_frame;
        let pair = self.pair().filter(|_| gestures && claimed && !pen_down);
        if let (Some(before), Some(after)) = (self.previous, pair) {
            self.gesture = delta(before, after);
        }
        self.previous = pair;
        self.claimed = claimed && self.contacts.values().any(|c| c.finger);
        let release = if blocked { self.forwarded_finger.take().map(|(_, pos)| pos) } else { None }.or(cancelled);
        let cancelled_press =
            raw.events.iter().zip(&sources).any(|(event, source)| {
                matches!(event, Event::PointerButton { pressed: true, .. }) && source.is_some_and(|(key, _)| cancelled_ids.contains(&key))
            });
        let cancel = release.is_some() || cancelled_press;
        if cancelled.is_some() {
            self.forwarded_finger = None;
            self.forwarded_pen = None;
            self.gesture = None;
            self.previous = None;
        }
        let mut index = 0;
        raw.events.retain(|event| {
            let contact = sources.get(index).copied().flatten();
            index += 1;
            if contact.is_some_and(|(key, _)| cancelled_ids.contains(&key))
                && matches!(event, Event::PointerMoved(_) | Event::PointerButton { .. } | Event::PointerGone)
            {
                return false;
            }
            let canvas_finger = contact.is_some_and(|(_, c)| c.finger && c.canvas);
            if canvas_finger {
                // Canvas gestures are handled here, not egui's trackpad pathway.
                if matches!(event, Event::Touch { .. }) {
                    return false;
                }
                if blocked && matches!(event, Event::PointerMoved(_) | Event::PointerButton { .. } | Event::PointerGone) {
                    return false;
                }
            }
            if let (Some((key, c)), Event::PointerButton { pos, pressed, .. }) = (contact, event) {
                let slot = if c.finger { &mut self.forwarded_finger } else { &mut self.forwarded_pen };
                *slot = (c.canvas && *pressed).then_some((key, *pos));
            }
            true
        });
        if let Some(pos) = release {
            // Release egui's drag ownership, but never commit it as a tool click.
            release_pointer(raw, pos);
            self.block_tools = true;
        }
        self.block_tools |= cancel;
        cancel
    }

    fn pair(&self) -> Option<Pair> {
        let mut contacts = self.contacts.iter().filter(|(_, c)| c.finger && c.canvas);
        let (&a, first) = contacts.next()?;
        let (&b, second) = contacts.next()?;
        if contacts.next().is_some() || a.0 != b.0 {
            return None;
        }
        Some([(a, first.pos), (b, second.pos)])
    }
}

fn release_pointer(raw: &mut RawInput, pos: Pos2) {
    raw.events.insert(0, Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    raw.events.insert(1, Event::PointerGone);
}

fn delta(before: Pair, after: Pair) -> Option<Gesture> {
    let [(a, p), (b, q)] = before;
    let [(c, r), (d, s)] = after;
    if a != c || b != d {
        return None;
    }
    let old = q - p;
    let new = s - r;
    let length = old.length();
    let next = new.length();
    if length < 2.0 || next < 2.0 {
        return None;
    }
    let zoom = next / length;
    let rotation = crate::rotate_view::wrap_deg((new.angle() - old.angle()).to_degrees());
    let from = p + old * 0.5;
    let to = r + new * 0.5;
    (zoom.is_finite() && rotation.is_finite() && from.is_finite() && to.is_finite()).then_some(Gesture { from, to, zoom, rotation })
}

/// Move the same document point from the old centroid to the new centroid while
/// scaling and rotating about it (also honors flipped and non-square-pixel views).
pub(crate) fn apply(view: &mut crate::state::View, xf: crate::canvas::ViewXform, ppp: f32, gesture: Gesture) {
    let anchor = xf.to_doc(gesture.from);
    let zoom = crate::zoom_levels::clamp(view.zoom * gesture.zoom, view.doc_size);
    let rotation = crate::rotate_view::wrap_deg(view.rotation + crate::rotate_view::screen_delta(gesture.rotation, xf.flip));
    let next = crate::canvas::ViewXform { zoom: zoom / ppp, rotation, ..xf };
    let offset = next.unmap_vec(gesture.to - xf.rect.center()) / next.zoom;
    let center = [anchor[0] as f32 - offset.x, anchor[1] as f32 - offset.y];
    if zoom.is_finite() && zoom > 0.0 && center.iter().all(|v| v.is_finite()) {
        view.zoom = zoom;
        view.rotation = rotation;
        view.center = center;
        view.fit_pending = false;
        view.fill_pending = false;
    }
}

#[cfg(test)]
#[path = "touch_nav_tests.rs"]
mod tests;
