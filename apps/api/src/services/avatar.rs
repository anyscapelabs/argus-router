use urlencoding::encode;

pub fn initials_avatar_url(seed: &str) -> String {
    format!(
        "https://api.dicebear.com/9.x/initials/svg?seed={}&backgroundColor={}&radius=50&fontScale=0.5",
        encode(seed),
        color_for(seed)
    )
}

fn color_for(seed: &str) -> &'static str {
    const PALETTE: [&str; 5] = ["F97316", "0EA5E9", "22C55E", "8B5CF6", "EC4899"];
    let hash: u64 = seed.bytes().fold(5381u64, |acc, b| {
        acc.wrapping_mul(33).wrapping_add(u64::from(b))
    });
    PALETTE[(hash as usize) % PALETTE.len()]
}

pub fn letter_of(name: &str) -> String {
    name.split_whitespace()
        .next()
        .and_then(|word| word.chars().next())
        .map(|letter| letter.to_uppercase().collect())
        .unwrap_or_default()
}

pub fn letter_avatar_url(name: &str) -> String {
    format!(
        "https://api.dicebear.com/9.x/initials/svg?seed={}&backgroundColor=transparent",
        encode(&letter_of(name))
    )
}
