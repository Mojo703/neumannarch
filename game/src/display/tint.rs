use mirage_engine::Color;
use neumannarch_sim::Materials;

const MATERIAL_COLOURS: [Color; 3] = [
    Color::rgb(0.62, 0.64, 0.70),
    Color::rgb(0.35, 0.62, 0.55),
    Color::rgb(0.72, 0.60, 0.30),
];

pub fn toward(base: Color, caps: Materials, strength: f32) -> Color {
    let total = caps.total();
    if total <= 0.0 {
        return base;
    }
    let even = 1.0 / MATERIAL_COLOURS.len() as f32;
    let mut colour = base;
    for ((_, share), material) in caps.amounts().zip(MATERIAL_COLOURS) {
        let lead = ((share / total) as f32 - even).max(0.0) / (1.0 - even);
        let pull = strength * lead;
        colour = Color::rgb(
            colour.red + pull * (material.red - colour.red),
            colour.green + pull * (material.green - colour.green),
            colour.blue + pull * (material.blue - colour.blue),
        );
    }
    colour
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: Color = Color::rgb(0.55, 0.5, 0.45);

    fn apart(colour: Color, toward: Color) -> f32 {
        (colour.red - toward.red).abs()
            + (colour.green - toward.green).abs()
            + (colour.blue - toward.blue).abs()
    }

    #[test]
    fn a_asteroid_is_tinted_toward_the_material_it_is_rich_in_and_an_even_asteroid_is_not() {
        assert_eq!(toward(BASE, Materials::new(4.0, 4.0, 4.0), 1.0), BASE);
        assert_eq!(
            toward(BASE, Materials::ZERO, 1.0),
            BASE,
            "nor a bare asteroid"
        );

        let regions = [
            Materials::new(8.0, 1.0, 1.0),
            Materials::new(1.0, 8.0, 1.0),
            Materials::new(1.0, 1.0, 8.0),
        ];
        for (caps, material) in regions.into_iter().zip(MATERIAL_COLOURS) {
            let tinted = toward(BASE, caps, 1.0);
            assert!(
                apart(tinted, material) < apart(BASE, material),
                "{caps:?} draws no nearer its own material's colour"
            );
            for other in regions.into_iter().filter(|other| *other != caps) {
                assert_ne!(
                    tinted,
                    toward(BASE, other, 1.0),
                    "two regions share a colour"
                );
            }
        }
    }

    #[test]
    fn a_weaker_pull_leaves_a_colour_nearer_the_one_it_started_from() {
        let caps = Materials::new(8.0, 1.0, 1.0);
        let faint = toward(BASE, caps, 0.2);
        let full = toward(BASE, caps, 1.0);

        assert!(apart(faint, BASE) < apart(full, BASE));
        assert_ne!(faint, BASE, "a faint pull still moves the colour");
    }
}
