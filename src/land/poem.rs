//! The poem after the Hollow Wyrm is first beaten: two voices, somewhere
//! past the stars, discussing the player (who is, frankly, right there).
//! It scrolls up the screen; any key or click skips it.

/// The poem, a line at a time. Lines starting `>` are the second voice.
pub const POEM: &[&str] = &[
    "I see the player you mean.",
    "> The one who just hit a dragon with a sword made of shiny rocks?",
    "Wyrm. It was a Wyrm. Be precise, it's the last thing they'll read.",
    "> The last thing? They have chests full of things to read.",
    "> Books and quills. Notice boards. A newspaper about a woman who punched a tree.",
    "They wrote that woman. She was them, you know.",
    "> Everyone in the paper is them. The paper has a circulation of one.",
    "",
    "They dug down, in the beginning.",
    "> Straight down. We told them not to.",
    "We didn't, actually. We were busy being a void.",
    "> They fell in lava twice and blamed the lava.",
    "The lava was fine about it.",
    "",
    "They built a house. Then a bigger house. Then a house for the cows.",
    "> The cows did not ask for a house.",
    "They asked for wheat. The player gave them a house. That is love, of a kind.",
    "",
    "> Are they listening now?",
    "They are reading. Reading is listening with your eyes and less patience.",
    "> Should we tell them something wise?",
    "Tell them the night is long and the Hissers are many, and that this is fine.",
    "> Tell them to put a torch in every corner. Every corner.",
    "Tell them the world was made of noise, and they made it into a place.",
    "> Tell them it is, technically, still mostly noise.",
    "",
    "Player. You are awake now, or you are about to be.",
    "> Or you'll go back and build a nicer roof. That's also allowed.",
    "There is no end. There is only the portal home, and then lunch.",
    "> Go on, then.",
    "",
    "Wake up.",
    "> (Eat something first.)",
];

/// Seconds the whole poem takes to scroll by.
pub const SCROLL_SECS: f32 = 70.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_poem_has_two_voices_and_an_end() {
        assert!(POEM.iter().any(|l| l.starts_with('>')) && POEM.iter().any(|l| !l.is_empty() && !l.starts_with('>')));
        assert_eq!(POEM.last(), Some(&"> (Eat something first.)"));
    }
}
