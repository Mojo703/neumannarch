pub fn titled(word: &str) -> String {
    let mut letters = word.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => String::new(),
    }
}

pub fn is_a_phrase(text: &str) -> bool {
    !text.is_empty() && !text.contains(['.', ';', ',', '-'])
}

#[cfg(test)]
mod tests {
    use probe_protocol::{Bot, Holder, Lobby, PlayerId, Refused};
    use probe_sim::roster::{FRIGATE, Roster};
    use probe_sim::state::view::Building;
    use probe_sim::{Material, RockId, TeamId, Tick};

    use super::*;
    use crate::display::scene::{Entry, WheelBand};
    use crate::net::listener::NoListener;
    use crate::screens::control::HOST_ONLY;
    use crate::screens::lobby::{
        ALREADY_READY, CLOCKS, LobbyScreen, NO_SEAT, clock_name, player_name, refusal_phrase,
        team_name,
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
            probe_protocol::NotReady::NoSeats,
            probe_protocol::NotReady::OpenSeat { slot: 1 },
            probe_protocol::NotReady::Unready { slot: 1 },
            probe_protocol::NotReady::HostUnseated,
        ];
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
}
