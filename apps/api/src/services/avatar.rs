use urlencoding::encode;

/// Initials avatar service (DiceBear) for email/password signups that come
/// without a profile picture.
///
/// `https://api.dicebear.com/9.x/initials/svg?seed=<email>&backgroundColor=<hex>&radius=50`
pub fn initials_avatar_url(seed: &str) -> String {
    format!(
        "https://api.dicebear.com/9.x/initials/svg?seed={}&backgroundColor={}&radius=50&fontScale=0.5",
        encode(seed),
        color_for(seed)
    )
}

fn color_for(seed: &str) -> &'static str {
    const PALETTE: [&str; 5] = ["F97316", "0EA5E9", "22C55E", "8B5CF6", "EC4899"];
    // `wrapping_mul`/`wrapping_add` are load-bearing, not style: this runs
    // while building signup and avatar responses, and a debug build panics on
    // `u64` overflow (`attempt to multiply with overflow`), which kills the
    // connection mid-response instead of returning an error.
    let hash: u64 = seed.bytes().fold(5381u64, |acc, b| {
        acc.wrapping_mul(33).wrapping_add(u64::from(b))
    });
    PALETTE[(hash as usize) % PALETTE.len()]
}

/// Text initials for a name — the fallback the dashboard renders when the
/// remote avatar cannot load, so the two never disagree about the letters.
/// The workspace chip is one glyph wide, so only the first word contributes:
/// "Default" → "D", "Argus Production" → "A".
pub fn letter_of(name: &str) -> String {
    name.split_whitespace()
        .next()
        .and_then(|word| word.chars().next())
        .map(|letter| letter.to_uppercase().collect())
        .unwrap_or_default()
}

/// Background-free, single-letter avatar for the workspace chip.
///
/// Two deliberate differences from [`initials_avatar_url`]. `backgroundColor`
/// is sent as the literal `transparent`, which is the only way the hosted
/// service omits its filled `<rect>` — the verified output is a lone white
/// `<text>` element, so the chip's own Tailwind background stays visible. And
/// the seed is the letter itself rather than the name: DiceBear derives up to
/// two glyphs from a seed, so `seed=Anyscapelabs` renders "AN" while
/// `seed=A` renders "A".
pub fn letter_avatar_url(name: &str) -> String {
    format!(
        "https://api.dicebear.com/9.x/initials/svg?seed={}&backgroundColor=transparent",
        encode(&letter_of(name))
    )
}
