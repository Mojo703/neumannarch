use mirage_engine::math::UVec2;
use mirage_engine::mesh::Sphere;
use mirage_engine::prelude::*;
use probe_game::controls::Controls;
use probe_game::display::glyph_quad::GlyphQuad;
use probe_game::screens::flow::Flow;

meshes! { enum Shape { Sphere, GlyphQuad } }

const WINDOW: UVec2 = UVec2::new(1280, 720);

fn main() {
    run(
        Config::new("Probe Game")
            .with_size(WINDOW.x, WINDOW.y)
            .with_tick_interval(probe_sim::TICK),
        |_| Ok(Probe::new()),
    );
}

struct Probe {
    flow: Flow,
}

impl Game for Probe {
    type Actions = Controls;
    type Meshes = Shape;
    type Sound = NoSound;
    type Sources = NoSources;
    type Styles = ();

    fn tick(&mut self, _ctx: &mut TickCtx<'_, Self>) {
        self.flow.tick();
    }

    fn frame(&mut self, ctx: &mut FrameCtx<'_, Self>) {
        self.flow.frame(ctx);
    }
}

impl Probe {
    fn new() -> Probe {
        Probe {
            flow: Flow::opening(),
        }
    }
}

#[cfg(all(test, feature = "look"))]
mod tests {
    use std::path::PathBuf;

    use mirage_engine::egui;
    use mirage_engine::headless::Session as Offscreen;
    use mirage_engine::math::Vec2;
    use probe_game::display::ease;
    use probe_game::display::scene::WheelBand;
    use probe_game::display::viewport::Viewport;
    use probe_game::display::wheels::Still;
    use probe_game::screens::flow::Screen;
    use probe_game::screens::play::Play;
    use probe_game::screens::{control, lobby, title};
    use probe_protocol::Notice;
    use probe_sim::roster::SHIPYARD;
    use probe_sim::{RockId, RowId};

    use super::*;

    const TARGET: UVec2 = UVec2::new(1280, 720);

    const PATIENCE: usize = 600;

    static ONE_DRIVE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        ONE_DRIVE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    const ROCK: RockId = RockId(10);

    fn game() -> Offscreen<Probe> {
        Offscreen::new(
            Config::new("probe-play").with_tick_interval(probe_sim::TICK),
            TARGET,
            |_| Ok(Probe::new()),
        )
        .expect("the offscreen session starts")
    }

    fn window() -> egui::Rect {
        egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(TARGET.x as f32, TARGET.y as f32),
        )
    }

    fn play(session: &Offscreen<Probe>) -> &Play {
        match session.game().flow.stage() {
            Screen::Play { play, .. } => play,
            _ => panic!("the match is on screen"),
        }
    }

    fn lobby(session: &Offscreen<Probe>) -> Option<&probe_protocol::Lobby> {
        match session.game().flow.stage() {
            Screen::Lobby { screen, .. } => Some(screen.lobby()),
            _ => None,
        }
    }

    fn viewport(session: &Offscreen<Probe>) -> Viewport {
        Viewport::of(play(session).camera(), TARGET, 1.0)
    }

    fn click(session: &mut Offscreen<Probe>) {
        session.press(MouseButton::Left);
        session.step();
        session.release(MouseButton::Left);
        session.step();
    }

    fn click_at(session: &mut Offscreen<Probe>, at: egui::Pos2) {
        session.set_pointer(Vec2::new(at.x, at.y));
        session.step();
        click(session);
    }

    fn tap(session: &mut Offscreen<Probe>, key: Key) {
        session.press(key);
        session.step();
        session.release(key);
        session.step();
    }

    fn advance(session: &mut Offscreen<Probe>, ticks: u64) {
        for _ in 0..ticks {
            session.tick();
        }
    }

    fn settled(session: &mut Offscreen<Probe>) {
        let ticks = (ease::SPAN_SECONDS / probe_sim::TICK.as_secs_f64()).ceil() as u64;
        advance(session, ticks);
        session.step();
    }

    fn save(session: &Offscreen<Probe>, name: &str) -> PathBuf {
        let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("look");
        std::fs::create_dir_all(&out).expect("game/look is writable");
        let path = out.join(format!("{name}.png"));
        let pixels = session.pixels().expect("the target reads back");
        image::RgbaImage::from_raw(TARGET.x, TARGET.y, pixels)
            .expect("the pixels match the target's size")
            .save(&path)
            .expect("the PNG writes");
        path
    }

    fn lobby_places(session: &Offscreen<Probe>) -> lobby::Places {
        lobby(session).expect("the lobby is on screen");
        lobby::Places::over(window())
    }

    fn stepped_until(session: &mut Offscreen<Probe>, ready: impl Fn(&Offscreen<Probe>) -> bool) {
        for _ in 0..PATIENCE {
            if ready(session) {
                return;
            }
            session.step();
        }
        panic!("the room never answered");
    }

    #[test]
    fn a_skirmish_starts_the_moment_its_lobby_opens() {
        let _one = one_at_a_time();
        let mut session = game();
        session.step();
        click_at(
            &mut session,
            title::Places::over(window()).skirmish.center(),
        );

        let places = lobby_places(&session);
        assert_eq!(places.rows.len(), 4, "one row per seat the match can hold");
        let act = places.act;
        click_at(&mut session, act.center());
        session.tick();
        session.step();

        assert!(
            matches!(session.game().flow.stage(), Screen::Play { .. }),
            "Start was not enabled on the lobby the game opens"
        );
    }

    #[test]
    fn a_seats_team_is_chosen_from_the_list_its_choice_opens() {
        let _one = one_at_a_time();
        let mut session = game();
        session.step();
        click_at(
            &mut session,
            title::Places::over(window()).skirmish.center(),
        );

        let team = lobby_places(&session).rows[1].team;
        click_at(&mut session, team.center());
        save(&session, "lobby_choice");
        click_at(&mut session, control::list_row(team, 2).center());

        assert_eq!(
            lobby(&session).expect("still the lobby").slots()[1].team,
            probe_sim::TeamId(2)
        );
    }

    #[test]
    fn quit_is_disabled_and_shows_why_while_the_pointer_is_over_it() {
        let _one = one_at_a_time();
        let mut session = game();
        session.step();

        let quit = title::Places::over(window()).quit;
        session.set_pointer(Vec2::new(quit.center().x, quit.center().y));
        session.step();
        click(&mut session);

        assert!(
            matches!(session.game().flow.stage(), Screen::Title { .. }),
            "a disabled Quit opens nothing"
        );
        save(&session, "title_quit_reason");
    }

    #[test]
    fn the_host_removes_a_guest_and_the_seat_it_held_opens() {
        let _one = one_at_a_time();
        let mut session = game();
        session.step();
        click_at(&mut session, title::Places::over(window()).host.center());
        stepped_until(&mut session, |session| lobby(session).is_some());

        let mut guest = probe_game::net::connection::Connection::joining(&format!(
            "127.0.0.1:{}",
            probe_protocol::DEFAULT_PORT
        ));
        stepped_until(&mut session, |session| {
            lobby(session)
                .is_some_and(|lobby| lobby.slot_of(probe_protocol::PlayerId(1)) == Some(1))
        });

        let kick = lobby_places(&session).rows[1].kick;
        click_at(&mut session, kick.center());
        stepped_until(&mut session, |session| {
            lobby(session)
                .is_some_and(|lobby| lobby.slots()[1].holder == probe_protocol::Holder::Open)
        });

        let mut notices = Vec::new();
        for _ in 0..PATIENCE {
            notices.extend(guest.notices());
            if notices.contains(&Notice::Removed) {
                return;
            }
            session.step();
        }
        panic!("the kicked machine was never told: {notices:?}");
    }

    #[test]
    fn a_skirmish_runs_from_the_title_to_the_results() {
        let _one = one_at_a_time();
        let mut session = game();
        session.step();
        save(&session, "title");

        click_at(
            &mut session,
            title::Places::over(window()).skirmish.center(),
        );
        let opened = lobby(&session).expect("Skirmish opens a lobby").clone();
        assert!(
            matches!(opened.slots()[1].holder, probe_protocol::Holder::Bot(_)),
            "a skirmish seats a bot in seat one"
        );
        assert_eq!(opened.seat_of(1), Some(probe_sim::SeatId(1)));

        let clock = lobby_places(&session).clock;
        click_at(&mut session, clock.center());
        save(&session, "lobby");
        click_at(&mut session, control::list_row(clock, 0).center());
        let shortest = lobby(&session).expect("still the lobby").clock();
        assert_eq!(shortest, *probe_protocol::CLOCK_RANGE.start());

        let act = lobby_places(&session).act;
        click_at(&mut session, act.center());
        session.tick();
        session.step();
        let started = play(&session);
        assert_eq!(started.session().state().clock(), shortest);
        assert_eq!(started.seat(), probe_sim::SeatId(0));

        placed(&mut session);
        paused(&mut session);

        while matches!(session.game().flow.stage(), Screen::Play { play, .. } if !play.over()) {
            advance(&mut session, 1);
        }
        session.step();
        assert!(
            matches!(session.game().flow.stage(), Screen::Results { .. }),
            "the clock runs out into the results"
        );

        session.step();
        save(&session, "results");
    }

    fn placed(session: &mut Offscreen<Probe>) {
        let centre = rock_at(session);
        click_at(session, centre);
        assert_eq!(
            play(session).selection(),
            Some(ROCK),
            "clicking a rock selects it"
        );
        settled(session);

        let band = band_of(session, centre, SHIPYARD);
        click_at(session, band);
        advance(session, 2);
        session.step();

        let mine = play(session).seat();
        let shipyard = play(session)
            .view()
            .present
            .iter()
            .find(|present| present.row == SHIPYARD && present.seat == mine)
            .expect("the reserve placed the shipyard");
        assert_eq!(shipyard.home, ROCK);

        session.set_pointer(Vec2::new(centre.x, centre.y));
        session.step();
        save(session, "wheel");
    }

    fn band_of(session: &Offscreen<Probe>, pointer: egui::Pos2, row: RowId) -> egui::Pos2 {
        let wheels = play(session).wheels(&viewport(session), Some(pointer), false, &mut Still);
        wheels
            .iter()
            .find(|wheel| wheel.rock() == ROCK)
            .expect("the selected rock carries a wheel")
            .band(row, WheelBand::Plus(1))
            .expect("the row's band is on the wheel")
    }

    fn paused(session: &mut Offscreen<Probe>) {
        let before = play(session).session().state().tick();
        tap(session, Key::Escape);
        assert!(play(session).paused(), "Escape opens the pause screen");
        advance(session, 4);
        assert_eq!(
            play(session).session().state().tick(),
            before,
            "a skirmish stops under the pause screen"
        );

        tap(session, Key::Escape);
        assert!(!play(session).paused());
        advance(session, 4);
        assert!(play(session).session().state().tick() > before);
    }

    #[test]
    fn the_opening_view_frames_the_belt_with_the_focus_at_its_centre() {
        let _one = one_at_a_time();
        let mut session = game();
        session.step();
        click_at(
            &mut session,
            title::Places::over(window()).skirmish.center(),
        );
        let act = lobby_places(&session).act;
        click_at(&mut session, act.center());
        session.tick();
        session.step();

        let play = play(&session);
        let viewport = viewport(&session);
        let focus = viewport
            .point_of(play.camera().focus())
            .expect("the focus is in front of the eye");
        assert!(
            focus.distance(egui::pos2(TARGET.x as f32 / 2.0, TARGET.y as f32 / 2.0)) <= 1.0,
            "the focus draws at {focus}, not the centre pixel"
        );

        let framed = play
            .view()
            .terrain
            .iter()
            .filter_map(|terrain| viewport.point_of(play.rock_pos(terrain.rock)?))
            .filter(|at| {
                (0.0..TARGET.x as f32).contains(&at.x) && (0.0..TARGET.y as f32).contains(&at.y)
            })
            .count();

        assert!(
            framed > 1,
            "the opening zoom shows {framed} rocks, so there is nothing to choose between"
        );
    }

    #[test]
    fn a_right_drag_moves_the_belt_under_the_pointer() {
        let _one = one_at_a_time();
        let mut session = game();
        session.step();
        click_at(
            &mut session,
            title::Places::over(window()).skirmish.center(),
        );
        let act = lobby_places(&session).act;
        click_at(&mut session, act.center());
        session.tick();
        session.step();

        let from = egui::pos2(400.0, 300.0);
        let dragged = egui::vec2(-120.0, 60.0);
        session.set_pointer(Vec2::new(from.x, from.y));
        session.step();
        let was = rock_at(&session);

        session.press(MouseButton::Right);
        session.set_pointer(Vec2::new(from.x + dragged.x, from.y + dragged.y));
        session.step();
        settled(&mut session);

        let now = rock_at(&session);
        assert!(
            (now - was - dragged).length() <= 0.06 * dragged.length(),
            "the rock moved from {was} to {now}, not by {dragged}"
        );
    }

    fn rock_at(session: &Offscreen<Probe>) -> egui::Pos2 {
        viewport(session)
            .point_of(
                play(session)
                    .rock_pos(ROCK)
                    .expect("the rock is on the map"),
            )
            .expect("the rock is in front of the eye")
    }
}
