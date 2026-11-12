use cauldron_chrome::avatar::{cog_angle, hop, size, svg, Look, Mood, MOODS};
use cauldron_chrome::tokens::{avatar, Rgb, AGENT_DOTS};

fn look(px: f32) -> Look {
    Look { px, outline: false, ms: Some(1000), bust: false, ink: None }
}

fn colours(s: &str) -> Vec<String> {
    s.match_indices('#')
        .filter_map(|(i, _)| s.get(i..i + 7))
        .filter(|h| h[1..].chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_lowercase)
        .collect()
}

#[test]
fn there_are_eleven_moods_and_each_draws_deterministically() {
    assert_eq!(MOODS.len(), 11);
    for m in MOODS {
        assert_eq!(svg(2, m, look(64.)), svg(2, m, look(64.)));
        assert!(svg(2, m, look(64.)).starts_with("<svg"));
    }
}

#[test]
fn avatars_paint_only_with_tokens_and_take_the_agents_colour() {
    let mut ok: Vec<String> = AGENT_DOTS.iter().map(|c| c.to_hex()).collect();
    ok.push(avatar::EYE.to_hex());
    for (d, m) in MOODS.iter().enumerate() {
        let s = svg(d, *m, look(64.));
        assert!(s.contains(&AGENT_DOTS[d % AGENT_DOTS.len()].to_hex()));
        for c in colours(&s) {
            assert!(ok.contains(&c), "{c} is not a token");
        }
    }
}

#[test]
fn working_turns_the_head_into_a_cog() {
    let s = svg(0, Mood::Working, look(64.));
    assert!(s.contains("class=\"cog\""));
    assert!(!s.contains("class=\"head\""));
    assert!(svg(0, Mood::Idle, look(64.)).contains("class=\"head\""));
}

#[test]
fn typing_turns_the_head_into_three_small_heads() {
    let s = svg(0, Mood::Typing, look(64.));
    assert_eq!(s.matches("class=\"mini\"").count(), 3);
    assert!(!s.contains("class=\"head\""));
}

#[test]
fn the_outline_agent_is_stroked_not_filled() {
    let s = svg(0, Mood::Idle, Look { outline: true, ..look(64.) });
    assert!(s.contains("fill=\"none\""));
    assert!(s.contains("stroke=\""));
}

#[test]
fn marks_show_from_28px_up() {
    for m in [Mood::Asking, Mood::Surprised, Mood::Sleepy] {
        assert!(!svg(0, m, look(27.)).contains("class=\"mark\""), "{m:?} at 27");
        assert!(svg(0, m, look(28.)).contains("class=\"mark\""), "{m:?} at 28");
    }
    assert!(!svg(0, Mood::Idle, look(64.)).contains("class=\"mark\""));
}

#[test]
fn reduced_motion_still_shows_the_mark() {
    for m in [Mood::Asking, Mood::Surprised, Mood::Sleepy] {
        assert!(svg(0, m, Look { ms: None, ..look(64.) }).contains("class=\"mark\""), "{m:?}");
    }
}

#[test]
fn the_cog_turns_once_every_3_8_seconds() {
    assert_eq!(cog_angle(0), 0.0);
    assert!((cog_angle(1900) - 180.0).abs() < 0.01);
    assert!(cog_angle(3800).abs() < 0.01);
}

#[test]
fn the_three_heads_hop_6_units_one_after_another() {
    for ms in (0..1200).step_by(10) {
        for i in 0..3 {
            assert!((-6.0..=0.0).contains(&hop(i, ms)));
        }
    }
    assert!((hop(0, 360) + 6.0).abs() < 0.01);
    assert!((hop(1, 510) + 6.0).abs() < 0.01);
    assert!((hop(2, 660) + 6.0).abs() < 0.01);
}

#[test]
fn px_is_the_sheets_figure_height_with_headroom_only_for_marks() {
    assert!(svg(0, Mood::Idle, look(16.)).contains(r#"viewBox="10 4 80 96""#));
    assert_eq!(size(16.), (16. * 80. / 96., 16.));
    // From 28px up the box grows upward; the figure keeps the sheet's scale (96 units = px).
    let (w, h) = size(64.);
    assert!((h - 64. * 112. / 96.).abs() < 1e-3 && (w - 64. * 80. / 96.).abs() < 1e-3);
}

#[test]
fn the_bust_is_the_head_and_shoulders_in_a_square_slot_with_marks_overhanging() {
    let b = Look { bust: true, outline: true, ..look(16.) };
    let s = svg(0, Mood::Asking, b);
    assert!(s.contains(r#"viewBox="22 -18 64 83""#), "{s}");
    assert!(s.contains("class=\"mark\""), "the bust keeps its marks at 16px");
    assert!(!svg(0, Mood::Asking, Look { px: 13., ..b }).contains("class=\"mark\""));
    // The slot is the sheet's 22 9 56 56: 16 units of image per 56 units, the image 27 units above.
    let (w, h, top) = cauldron_chrome::avatar::bust_frame(16.);
    let k = 16. / 56.;
    assert!((w - 64. * k).abs() < 1e-3 && (h - 83. * k).abs() < 1e-3 && (top - 27. * k).abs() < 1e-3);
}

#[test]
fn ink_draws_the_figure_in_another_token_colour() {
    let grey = Rgb::hex(0x8e8896);
    let s = svg(3, Mood::Typing, Look { outline: true, ink: Some(grey), ..look(17.) });
    assert!(s.contains("#8e8896"));
    assert!(!s.contains(&AGENT_DOTS[3].to_hex()));
}
