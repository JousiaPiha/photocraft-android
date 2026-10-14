//! A finger tap is navigation only while finger painting is disabled. No paint
//! events are delayed, replayed or undone to recognize this gesture.

use super::ContactId;
use egui::{Pos2, TouchPhase};

const TAP_SECONDS: f64 = 0.3;
const TAP_SLOP: f32 = 18.0; // egui points, independent of screen density
const DOUBLE_TAP_SLOP: f32 = 24.0;

#[derive(Clone, Copy)]
struct Tap {
    id: ContactId,
    pos: Pos2,
    time: f64,
}

#[derive(Default)]
pub(super) struct TapSequence {
    document: Option<u64>,
    down: Option<(Tap, bool)>,
    last: Option<Tap>,
}

impl TapSequence {
    pub fn document(&mut self, document: Option<u64>) {
        if self.document != document {
            self.clear();
            self.document = document;
        }
    }

    pub fn clear(&mut self) {
        self.down = None;
        self.last = None;
    }

    /// `eligible` requires a finger inside the canvas; `alone` is measured
    /// before adding a new contact, so a pen/palm/UI contact breaks the sequence.
    pub fn touch(&mut self, id: ContactId, phase: TouchPhase, pos: Pos2, time: f64, eligible: bool, alone: bool) -> bool {
        if !eligible || !pos.is_finite() || !time.is_finite() {
            self.clear();
            return false;
        }
        match phase {
            TouchPhase::Start if alone => {
                let second = self.last.take().is_some_and(|tap| tap.id.0 == id.0 && elapsed(tap.time, time) && tap.pos.distance(pos) <= DOUBLE_TAP_SLOP);
                self.down = Some((Tap { id, pos, time }, second));
            }
            TouchPhase::Move => {
                if !self.down.is_some_and(|(tap, _)| tap.id == id && elapsed(tap.time, time) && tap.pos.distance(pos) <= TAP_SLOP) {
                    self.clear();
                }
            }
            TouchPhase::End => {
                if let Some((tap, second)) = self.down.take()
                    && tap.id == id
                    && elapsed(tap.time, time)
                    && tap.pos.distance(pos) <= TAP_SLOP
                {
                    if second {
                        self.last = None;
                        return true;
                    }
                    self.last = Some(Tap { time, ..tap });
                } else {
                    self.clear();
                }
            }
            TouchPhase::Start | TouchPhase::Cancel => self.clear(),
        }
        false
    }
}

fn elapsed(start: f64, end: f64) -> bool {
    (0.0..=TAP_SECONDS).contains(&(end - start))
}
