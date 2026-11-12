use cauldron_chrome::avatar::{svg, Mood};
use cauldron_chrome::tokens::{avatar, AGENT_DOTS};

fn colours(s: &str) -> Vec<String> {
    s.match_indices('#').map(|(i, _)| s[i..i + 7].to_lowercase()).filter(|h| h[1..].chars().all(|c| c.is_ascii_hexdigit())).collect()
}

#[test]
fn an_agent_always_gets_the_same_avatar() {
    assert_eq!(svg(7, 2, Mood::Idle), svg(7, 2, Mood::Idle));
}

#[test]
fn agents_get_different_textures() {
    assert_ne!(svg(1, 2, Mood::Idle), svg(2, 2, Mood::Idle));
}

#[test]
fn avatars_paint_only_with_tokens() {
    let mut ok: Vec<String> = AGENT_DOTS.iter().map(|c| c.to_hex()).collect();
    ok.extend([avatar::PAINT, avatar::FACE, avatar::EYE, avatar::SHADE, avatar::LIGHT].map(|c| c.to_hex()));
    for seed in 0..20 {
        let s = svg(seed, seed as usize, Mood::Idle);
        assert!(s.starts_with("<svg"), "{s}");
        for c in colours(&s) {
            assert!(ok.contains(&c), "{c} is not a token");
        }
    }
}

#[test]
fn the_body_takes_the_agents_dot_colour() {
    assert!(svg(3, 4, Mood::Idle).contains(&AGENT_DOTS[4].to_hex()));
}
