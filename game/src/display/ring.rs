use core::f32::consts::TAU;

use crate::display::scene::Run;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub radius: f32,
    pub glyph: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub start: f32,
    pub end: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    placements: Vec<Placement>,
    runs: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub run: usize,
    pub mark: usize,
    pub angle: f32,
    pub depth: u8,
    pub scale: f32,
}

#[derive(Clone, Copy)]
enum Fit {
    Loose,
    Stacked { scale: f32 },
}

#[derive(Clone, Copy)]
struct Spacing {
    start: f32,
    pitch: f32,
    fit: Fit,
}

impl Fit {
    fn depth(self, mark: usize) -> u8 {
        match self {
            Fit::Loose => 0,
            Fit::Stacked { .. } => u8::try_from(mark).unwrap_or(u8::MAX),
        }
    }

    fn scale(self) -> f32 {
        match self {
            Fit::Loose => 1.0,
            Fit::Stacked { scale } => scale,
        }
    }
}

impl Spacing {
    fn of(start: f32, share: f32, glyph: f32, marks: usize) -> Spacing {
        let natural = glyph * marks as f32;
        if natural <= share {
            Spacing {
                start,
                pitch: glyph,
                fit: Fit::Loose,
            }
        } else {
            let pitch = share / marks as f32;
            Spacing {
                start,
                pitch,
                fit: Fit::Stacked {
                    scale: pitch / glyph,
                },
            }
        }
    }

    fn place(self, run: usize, mark: usize) -> Placement {
        Placement {
            run,
            mark,
            angle: self.start + mark as f32 * self.pitch,
            depth: self.fit.depth(mark),
            scale: self.fit.scale(),
        }
    }
}

impl Layout {
    pub fn of(runs: &[Run], geometry: Geometry) -> Layout {
        let share = TAU / runs.len() as f32;
        let glyph = geometry.glyph / geometry.radius;
        Layout {
            placements: runs
                .iter()
                .enumerate()
                .flat_map(|(run, Run { marks, .. })| {
                    let spacing = Spacing::of(run as f32 * share, share, glyph, marks.len());
                    (0..marks.len()).map(move |mark| spacing.place(run, mark))
                })
                .collect(),
            runs: runs.len(),
        }
    }

    pub fn placements(&self) -> &[Placement] {
        &self.placements
    }

    pub fn span(&self, index: usize) -> Span {
        let share = TAU / self.runs as f32;
        Span {
            start: index as f32 * share,
            end: (index + 1) as f32 * share,
        }
    }
}

impl Span {
    pub fn at(self, fraction: f32) -> f32 {
        self.start + (self.end - self.start) * fraction.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use core::f32::consts::{PI, TAU};

    use probe_sim::{RowId, SeatId};

    use super::{Geometry, Layout};
    use crate::display::glyph::{Frame, Glyph, Size};
    use crate::display::scene::{Fill, Mark, Reason, Run};

    const RING: Geometry = Geometry {
        radius: 100.0,
        glyph: 16.0,
    };

    fn any_glyph() -> Glyph {
        Glyph {
            frame: Frame::Triangle,
            marks: Vec::new(),
            size: Size::Small,
        }
    }

    fn laid(runs: &[Run], geometry: Geometry) -> Vec<super::Placement> {
        Layout::of(runs, geometry).placements().to_vec()
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    fn run(marks: usize) -> Run {
        Run {
            seat: SeatId(0),
            marks: (0..marks)
                .map(|_| Mark {
                    glyph: any_glyph(),
                    fill: Fill::Solid,
                    dim: false,
                    reason: Reason::Here(RowId(0)),
                })
                .collect(),
        }
    }

    #[test]
    fn three_marks_of_one_run_each_have_their_own_arc() {
        let placed = laid(&[run(3)], RING);
        assert_eq!(placed.len(), 3);
        assert!(placed.iter().all(|p| p.depth == 0 && p.scale == 1.0));
        assert_eq!(placed[0].angle, 0.0);
        assert!(placed.windows(2).all(|w| w[0].angle < w[1].angle));
    }

    #[test]
    fn forty_marks_of_one_run_stack_within_the_turn() {
        let placed = laid(&[run(40)], RING);
        let depths: Vec<u8> = placed.iter().map(|p| p.depth).collect();
        assert_eq!(depths, (0..40).collect::<Vec<u8>>());
        assert!(placed.iter().all(|p| p.scale < 1.0));
        assert!(placed[39].angle < TAU);
    }

    #[test]
    fn a_compressed_run_fills_its_share_uniformly() {
        let placed = laid(&[run(40)], RING);
        let pitch = placed[1].angle - placed[0].angle;
        assert!(
            placed
                .windows(2)
                .all(|w| close(w[1].angle - w[0].angle, pitch))
        );
        assert!(close(placed[39].angle + pitch, TAU));
        assert!(close(pitch, placed[0].scale * RING.glyph / RING.radius));
    }

    #[test]
    fn two_runs_start_half_a_turn_apart() {
        let placed = laid(&[run(1), run(1)], RING);
        assert!(close(placed[1].angle - placed[0].angle, PI));
    }

    #[test]
    fn a_short_run_keeps_its_own_arcs_beside_a_stacked_one() {
        let placed = laid(&[run(40), run(3)], RING);
        assert!(placed.iter().any(|p| p.run == 0 && p.depth > 0));
        let short: Vec<_> = placed.iter().filter(|p| p.run == 1).collect();
        assert_eq!(short.len(), 3);
        assert!(short.iter().all(|p| p.depth == 0 && p.scale == 1.0));
    }

    #[test]
    fn placements_come_in_run_then_mark_order() {
        let placed = laid(&[run(2), run(0), run(3)], RING);
        let order: Vec<_> = placed.iter().map(|p| (p.run, p.mark)).collect();
        assert_eq!(order, [(0, 0), (0, 1), (2, 0), (2, 1), (2, 2)]);
    }

    #[test]
    fn an_empty_run_takes_its_share_and_places_nothing() {
        let placed = laid(&[run(0), run(1)], RING);
        assert_eq!(placed.len(), 1);
        assert!(close(placed[0].angle, PI));
    }

    #[test]
    fn no_runs_place_nothing() {
        assert!(laid(&[], RING).is_empty());
    }

    #[test]
    fn each_run_owns_an_equal_share_starting_at_twelve_oclock() {
        let laid = Layout::of(&[run(2), run(3)], RING);

        let first = laid.span(0);
        let second = laid.span(1);
        assert!(close(first.start, 0.0));
        assert!(close(first.end, PI));
        assert!(close(second.start, PI));
        assert!(close(second.end, TAU));
        assert!(close(first.at(0.5), PI / 2.0));
        assert_eq!(first.at(2.0), first.end, "a fraction past one is the end");
    }

    #[test]
    fn a_lone_run_owns_the_whole_turn() {
        let laid = Layout::of(&[run(0)], RING);
        assert_eq!(
            laid.span(0),
            super::Span {
                start: 0.0,
                end: TAU
            }
        );
    }
}
