use neumannarch_sim::Material;

pub fn material(material: Material) -> &'static str {
    match material {
        Material::Metals => "Metals",
        Material::Volatiles => "Volatiles",
        Material::Energy => "Energy",
    }
}

pub fn titled(word: &str) -> String {
    let mut letters = word.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => String::new(),
    }
}

pub fn is_a_phrase(text: &str) -> bool {
    let dashed = text
        .char_indices()
        .filter(|(_, letter)| *letter == '-')
        .any(|(at, _)| {
            !text[at + 1..]
                .chars()
                .next()
                .is_some_and(|next| next.is_ascii_digit())
        });
    !text.is_empty() && !text.contains(['.', ';', ',']) && !dashed
}

#[cfg(test)]
mod tests {
    use mirage_engine::egui::{Pos2, Rect};
    use neumannarch_protocol::{Bot, Holder, Lobby, PlayerId, Refused};
    use neumannarch_sim::roster::{FRIGATE, Roster};
    use neumannarch_sim::state::view::Building;
    use neumannarch_sim::{Materials, RockId, Stockpile, TeamId, Tick};

    use super::*;
    use crate::display::scene::{Entry, StripView, WheelBand, rock_name};
    use crate::display::strip::Strip;
    use crate::display::wheels::{NOT_YET, ROCK_TAKEN, Spoken};
    use crate::net::listener::NoListener;
    use crate::screens::control::HOST_ONLY;
    use crate::screens::lobby::{
        ALREADY_READY, CLOCKS, LobbyScreen, NO_SEAT, clock_name, player_name, refusal_phrase,
        seat_names, team_name,
    };
    use crate::screens::pause::NO_SURRENDER;
    use crate::screens::title::{NO_QUIT, NO_SETTINGS, Outcome, TAGLINE, TITLE};
    use crate::screens::{held, loading};

    const IMPERATIVES: [&str; 10] = [
        "Click", "Press", "Drag", "Choose", "Select", "Pick", "Tap", "Hold", "Try", "Use",
    ];

    const WORDS: [&str; 24] = [
        "Skirmish",
        "Host",
        "Join",
        "Settings",
        "Quit",
        "Start",
        "Ready",
        "Waiting",
        "Leave",
        "Kick",
        "Rematch",
        "Resume",
        "Surrender",
        "Random",
        "Seat",
        "Holder",
        "Team",
        "Clock",
        "Seed",
        "Results",
        "Winner",
        "Paused",
        "Desynced",
        "Connecting",
    ];

    fn every_string() -> Vec<String> {
        let roster = Roster::shipped();
        let name = titled(roster[FRIGATE].name);
        let screen = LobbyScreen::of(Lobby::skirmish(PlayerId::HOST), PlayerId::HOST);
        let entries = [
            Entry::Present(2),
            Entry::Surplus(1),
            Entry::Leaving {
                count: 1,
                to: RockId(4),
            },
            Entry::Building(Building {
                progress: 0.5,
                starved_of: None,
            }),
            Entry::Building(Building {
                progress: 0.5,
                starved_of: Some(Material::Metals),
            }),
            Entry::Building(Building {
                progress: 0.5,
                starved_of: Some(Material::Volatiles),
            }),
            Entry::Building(Building {
                progress: 0.5,
                starved_of: Some(Material::Energy),
            }),
            Entry::Arriving {
                count: 3,
                from: RockId(2),
            },
            Entry::Wanted {
                count: 1,
                dashed: false,
            },
            Entry::Wanted {
                count: 1,
                dashed: true,
            },
        ];
        let refusals = [
            Refused::NotHost,
            Refused::NotYours,
            Refused::NotSeated,
            Refused::NoSuchSlot,
            Refused::BadTeam,
            Refused::BadClock,
            Refused::AlreadySeated,
            Refused::NotAGuest,
            Refused::HeldByAGuest,
        ];
        let unready = [
            neumannarch_protocol::NotReady::NoSeats,
            neumannarch_protocol::NotReady::OpenSeat { slot: 1 },
            neumannarch_protocol::NotReady::Unready { slot: 1 },
            neumannarch_protocol::NotReady::HostUnseated,
        ];
        let strip = Strip::across(
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1280.0, 720.0)),
            StripView {
                stockpile: Stockpile::new(
                    Materials::new(120.0, 0.0, 300.0),
                    Materials::new(300.0, 300.0, 300.0),
                ),
                income: Materials::new(12.0, 0.0, 9.0),
                spend: Materials::new(9.0, 4.0, 0.0),
                elapsed: neumannarch_sim::Time(4120),
                clock: neumannarch_sim::Time(9000),
            },
        );
        let bars = Material::EVERY.map(|material| Spoken::Bar {
            material,
            pull: 12.0,
            cap: 20.0,
        });
        let refused_bands = [NOT_YET, ROCK_TAKEN].map(|why| Spoken::Refused {
            why: why.to_string(),
        });
        let seated = Lobby::skirmish(PlayerId::HOST)
            .freeze()
            .expect("a skirmish starts");
        let holders = [
            Holder::Open,
            Holder::Closed,
            Holder::Bot(Bot::Turtle),
            Holder::Bot(Bot::Expand),
            Holder::Player {
                player: PlayerId::HOST,
                ready: false,
            },
            Holder::Player {
                player: PlayerId(3),
                ready: true,
            },
        ];

        WORDS
            .iter()
            .map(|word| word.to_string())
            .chain([
                TITLE.to_string(),
                TAGLINE.to_string(),
                NO_SETTINGS.to_string(),
                NO_QUIT.to_string(),
                NO_SURRENDER.to_string(),
                NO_SEAT.to_string(),
                ALREADY_READY.to_string(),
                HOST_ONLY.to_string(),
                held::WAITING.to_string(),
                held::parted(Tick(4120)),
                loading::BUILDING.to_string(),
            ])
            .chain(Outcome::every().map(|outcome| outcome.phrase().to_string()))
            .chain([NoListener::NotBuilt, NoListener::PortHeld].map(|why| why.reason().to_string()))
            .chain(entries.map(|entry| entry.phrase(&name)))
            .chain(refusals.map(refusal_phrase))
            .chain(unready.map(|why| screen.start_reason(why)))
            .chain(holders.map(|holder| screen.holder_name(holder)))
            .chain([TeamId(0), TeamId(3)].map(team_name))
            .chain([PlayerId::HOST, PlayerId(3)].map(player_name))
            .chain(CLOCKS.map(clock_name))
            .chain(Material::EVERY.into_iter().flat_map(|material| {
                strip
                    .phrases(material)
                    .into_iter()
                    .chain([strip.net(material)])
            }))
            .chain([strip.elapsed()])
            .chain(bars.iter().map(|bar| bar.phrase(&roster)))
            .chain(refused_bands.iter().map(|band| band.phrase(&roster)))
            .chain(seat_names(seated.seating(), PlayerId::HOST))
            .chain([rock_name(RockId(11))])
            .collect()
    }

    #[test]
    fn every_string_the_player_reads_is_a_short_phrase() {
        for text in every_string() {
            assert!(
                is_a_phrase(&text),
                "the player is shown {text:?}, which is a sentence"
            );
        }
    }

    #[test]
    fn no_string_the_player_reads_tells_them_what_to_do() {
        for text in every_string() {
            let first = text.split_whitespace().next().unwrap_or_default();
            assert!(
                !IMPERATIVES.contains(&first),
                "the player is shown {text:?}, which is an instruction"
            );
        }
    }

    #[test]
    fn a_band_bears_the_sign_of_the_step_it_edits_by() {
        assert_eq!(WheelBand::Plus(1).label(), "+");
        assert_eq!(WheelBand::Minus(1).label(), "-");
        assert_eq!(WheelBand::Plus(5).label(), "+5");
        assert_eq!(WheelBand::Minus(5).label(), "-5");
    }

    #[test]
    fn a_name_is_shown_with_its_first_letter_upper_case() {
        assert_eq!(titled("shipyard"), "Shipyard");
        assert_eq!(titled("Turtle"), "Turtle");
        assert_eq!(titled(""), "");
    }

    #[test]
    fn a_sentence_is_not_a_phrase() {
        assert!(!is_a_phrase("Only the host starts a rematch."));
        assert!(!is_a_phrase("Waiting for Team 2, then Team 3"));
        assert!(!is_a_phrase("A two-to-four-player space RTS"));
        assert!(!is_a_phrase(""));
        assert!(is_a_phrase("Waiting for Team 2"));
    }

    #[test]
    fn a_minus_before_a_numeral_is_a_sign_and_not_a_dash() {
        assert!(is_a_phrase("-5"));
        assert!(is_a_phrase("+12 in -9 out"));
        assert!(!is_a_phrase("Team 2 - Team 3"));
        assert!(!is_a_phrase("Rock 5-"));
    }

    #[test]
    fn a_material_is_named_in_title_case() {
        assert_eq!(material(Material::Metals), "Metals");
        assert_eq!(material(Material::Volatiles), "Volatiles");
        assert_eq!(material(Material::Energy), "Energy");
    }
}
