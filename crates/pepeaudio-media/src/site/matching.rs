use std::{cmp::Reverse, collections::HashSet};

use super::{SiteError, SiteReference, SiteSearch};

const MINIMUM_SCORE: u16 = 70;
const MINIMUM_WINNING_MARGIN: u16 = 8;
const MAXIMUM_CANDIDATES_TO_RESOLVE: usize = 3;
const PRESENTATION_MARKERS: &[&[&str]] = &[
    &["official", "music", "video"],
    &["official", "lyric", "video"],
    &["official", "visualizer"],
    &["official", "video"],
    &["official", "audio"],
    &["lyric", "video"],
    &["lyrics"],
    &["visualizer"],
];
const VISUAL_PRESENTATION_MARKERS: &[&[&str]] = &[
    &["official", "music", "video"],
    &["official", "lyric", "video"],
    &["official", "visualizer"],
    &["official", "video"],
    &["lyric", "video"],
    &["lyrics"],
    &["visualizer"],
];

#[cfg(test)]
pub(super) fn select_candidate<'a>(
    candidates: &'a [SiteReference],
    search: &SiteSearch,
) -> Result<&'a SiteReference, SiteError> {
    ranked_candidates(candidates, search).map(|ranked| ranked[0])
}

pub(super) fn ranked_candidates<'a>(
    candidates: &'a [SiteReference],
    search: &SiteSearch,
) -> Result<Vec<&'a SiteReference>, SiteError> {
    let mut scored = candidates
        .iter()
        .filter_map(|candidate| candidate_score(candidate, search).map(|score| (score, candidate)))
        .collect::<Vec<_>>();
    scored.sort_unstable_by_key(|candidate| Reverse(candidate.0));
    let Some((best, candidate)) = scored.first().copied() else {
        return Err(SiteError::NoSearchMatch);
    };
    let runner_up = scored.get(1).map_or(0, |(score, _)| *score);
    if best < MINIMUM_SCORE
        || (best.saturating_sub(runner_up) < MINIMUM_WINNING_MARGIN
            && !authoritative_tie(&scored, search, best))
    {
        return Err(SiteError::NoSearchMatch);
    }
    let _ = candidate;
    scored.retain(|(score, _)| *score >= MINIMUM_SCORE);
    scored.truncate(MAXIMUM_CANDIDATES_TO_RESOLVE);
    Ok(scored.into_iter().map(|(_, candidate)| candidate).collect())
}

fn authoritative_tie(scored: &[(u16, &SiteReference)], search: &SiteSearch, best: u16) -> bool {
    scored
        .iter()
        .take_while(|(score, _)| best.saturating_sub(*score) < MINIMUM_WINNING_MARGIN)
        .all(|(_, candidate)| authoritative_artist(candidate, &search.expected_artists))
}

fn authoritative_artist(candidate: &SiteReference, expected_artists: &[String]) -> bool {
    let uploader = normalize(candidate.artist.as_deref().unwrap_or_default());
    let channel = uploader.strip_suffix(" topic").unwrap_or(&uploader);
    let candidate_context = normalize(&format!(
        "{} {}",
        candidate.title.as_deref().unwrap_or_default(),
        candidate.artist.as_deref().unwrap_or_default()
    ));
    !channel.is_empty()
        && expected_artists
            .iter()
            .map(|artist| normalize(artist))
            .any(|artist| {
                channel == artist
                    || (phrase_present(&artist, channel)
                        && artist
                            .split_whitespace()
                            .all(|token| phrase_present(&candidate_context, token)))
            })
}

pub(super) fn duration_matches(preferred: Option<u64>, actual: u64) -> bool {
    preferred.is_none_or(|preferred| preferred.abs_diff(actual) <= duration_tolerance(preferred))
}

fn candidate_score(candidate: &SiteReference, search: &SiteSearch) -> Option<u16> {
    let title = normalize_title(candidate.title.as_deref()?, &search.expected_artists);
    let expected = normalize_title(&search.expected_title, &search.expected_artists);
    if qualifier_conflict(&expected, &title) {
        return None;
    }
    let title_score = if title == expected {
        65
    } else if phrase_present(&title, &expected) || phrase_present(&expected, &title) {
        55
    } else {
        token_overlap(&expected, &title) * 55 / 100
    };
    if title_score < 30 {
        return None;
    }

    let candidate_context = normalize(&format!(
        "{} {}",
        candidate.title.as_deref().unwrap_or_default(),
        candidate.artist.as_deref().unwrap_or_default()
    ));
    let expected_context = normalize(&format!(
        "{} {}",
        search.expected_title,
        search.expected_artists.join(" ")
    ));
    if qualifier_conflict(&expected_context, &candidate_context) {
        return None;
    }
    let candidate_artist = normalize(candidate.artist.as_deref().unwrap_or_default());
    let artist_score = artist_score(
        &candidate_artist,
        &candidate_context,
        &search.expected_artists,
    )?;
    let duration_score = match (search.preferred_duration_ms, candidate.duration_ms) {
        (Some(expected), Some(actual)) if !duration_matches(Some(expected), actual) => return None,
        (Some(expected), Some(actual))
            if expected.abs_diff(actual) <= (expected / 20).max(5_000) =>
        {
            15
        }
        (Some(_), Some(_)) => 5,
        _ => 0,
    };
    let presentation_penalty = u16::from(has_visual_presentation_marker(
        candidate.title.as_deref().unwrap_or_default(),
    )) * 10;
    let score = title_score + artist_score + duration_score;
    let ranked_score = score.saturating_sub(presentation_penalty);
    Some(if score >= MINIMUM_SCORE {
        ranked_score.max(MINIMUM_SCORE)
    } else {
        ranked_score
    })
}

fn artist_score(
    uploader: &str,
    candidate_context: &str,
    expected_artists: &[String],
) -> Option<u16> {
    if expected_artists.is_empty() {
        return Some(10);
    }
    let matched = expected_artists
        .iter()
        .map(|artist| normalize(artist))
        .filter(|artist| {
            phrase_present(uploader, artist)
                || token_overlap(artist, uploader) >= 70
                || (phrase_present(artist, uploader)
                    && artist
                        .split_whitespace()
                        .all(|token| phrase_present(candidate_context, token)))
        })
        .count()
        .min(2);
    (matched > 0).then(|| u16::try_from(matched).unwrap_or(2) * 15)
}

fn duration_tolerance(preferred: u64) -> u64 {
    (preferred / 20).clamp(5_000, 10_000)
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_title(value: &str, expected_artists: &[String]) -> String {
    let normalized = normalize(&strip_artist_credit_groups(value, expected_artists));
    let canonical = normalized
        .split_whitespace()
        .map(|token| match token {
            "ft" | "featuring" => "feat",
            token => token,
        })
        .collect::<Vec<_>>();
    let mut semantic = canonical.clone();
    for marker in PRESENTATION_MARKERS {
        while let Some(position) = semantic
            .windows(marker.len())
            .position(|window| window == *marker)
        {
            semantic.drain(position..position + marker.len());
        }
    }
    if semantic.is_empty() {
        canonical.join(" ")
    } else {
        semantic.join(" ")
    }
}

fn strip_artist_credit_groups(value: &str, expected_artists: &[String]) -> String {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    while let Some((opening, closing)) = next_group(value, cursor) {
        output.push_str(&value[cursor..opening]);
        let content = &value[opening + 1..closing];
        if is_artist_credit(content, expected_artists) {
            output.push(' ');
        } else {
            output.push_str(&value[opening..=closing]);
        }
        cursor = closing + 1;
    }
    output.push_str(&value[cursor..]);
    output
}

fn next_group(value: &str, cursor: usize) -> Option<(usize, usize)> {
    let (relative_opening, opening_character) = value[cursor..]
        .char_indices()
        .find(|(_, character)| matches!(character, '(' | '['))?;
    let opening = cursor + relative_opening;
    let closing_character = if opening_character == '(' { ')' } else { ']' };
    let relative_closing = value[opening + 1..].find(closing_character)?;
    Some((opening, opening + 1 + relative_closing))
}

fn is_artist_credit(content: &str, expected_artists: &[String]) -> bool {
    let normalized = normalize(content);
    let mut parts = normalized.split_whitespace();
    let Some(marker) = parts.next() else {
        return false;
    };
    if !matches!(marker, "with" | "feat" | "featuring" | "ft") {
        return false;
    }
    let credited = parts.collect::<Vec<_>>().join(" ");
    !credited.is_empty()
        && expected_artists
            .iter()
            .map(|artist| normalize(artist))
            .any(|artist| {
                phrase_present(&credited, &artist)
                    || phrase_present(&artist, &credited)
                    || token_overlap(&artist, &credited) >= 70
            })
}

fn has_visual_presentation_marker(value: &str) -> bool {
    let normalized = normalize(value);
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    VISUAL_PRESENTATION_MARKERS
        .iter()
        .any(|marker| tokens.windows(marker.len()).any(|window| window == *marker))
}

fn token_overlap(expected: &str, candidate: &str) -> u16 {
    let expected = expected.split_whitespace().collect::<HashSet<_>>();
    let candidate = candidate.split_whitespace().collect::<HashSet<_>>();
    if expected.is_empty() || candidate.is_empty() {
        return 0;
    }
    let common = expected.intersection(&candidate).count() * 2;
    u16::try_from(common * 100 / (expected.len() + candidate.len())).unwrap_or(100)
}

fn qualifier_conflict(expected: &str, candidate: &str) -> bool {
    const QUALIFIERS: &[&str] = &[
        "live",
        "cover",
        "remix",
        "karaoke",
        "instrumental",
        "nightcore",
        "slowed",
        "sped up",
        "radio edit",
        "tribute",
        "impersonator",
        "teaser",
        "acoustic",
        "demo",
        "re recorded",
        "remaster",
        "remastered",
        "extended",
        "歌ってみた",
        "カバー",
        "ライブ",
    ];
    QUALIFIERS.iter().any(|qualifier| {
        phrase_present(candidate, qualifier) != phrase_present(expected, qualifier)
    })
}

fn phrase_present(text: &str, phrase: &str) -> bool {
    if phrase.is_ascii() {
        let padded = format!(" {text} ");
        padded.contains(&format!(" {phrase} "))
    } else {
        text.contains(phrase)
    }
}

#[cfg(test)]
mod tests {
    use super::select_candidate;
    use crate::{SiteProvider, SiteReference, SiteSearch};

    fn candidate(title: &str, artist: &str, duration_ms: u64) -> SiteReference {
        SiteReference {
            provider: SiteProvider::YouTube,
            page_url: "https://www.youtube.com/watch?v=abcdefghijk".into(),
            title: Some(title.into()),
            artist: Some(artist.into()),
            duration_ms: Some(duration_ms),
        }
    }

    fn search(title: &str) -> SiteSearch {
        SiteSearch::new(
            format!("{title} Primary Artist"),
            title,
            vec!["Primary Artist".into()],
            Some(180_000),
            Some("JPABC1234567".into()),
        )
        .expect("search")
    }

    #[test]
    fn ranking_can_select_the_correct_result_after_rank_one() {
        let candidates = [
            candidate("Example Song cover", "Unrelated", 180_000),
            candidate("Example Song", "Primary Artist - Topic", 181_000),
        ];
        let selected =
            select_candidate(&candidates, &search("Example Song")).expect("strong match");
        assert_eq!(selected.artist.as_deref(), Some("Primary Artist - Topic"));
    }

    #[test]
    fn weak_duration_and_qualifier_mismatches_are_rejected() {
        let expected = search("Example Song");
        for rejected in [
            candidate("Unrelated", "Primary Artist", 180_000),
            candidate("Example Song", "Primary Artist", 400_000),
            candidate("Example Song live", "Primary Artist", 180_000),
            candidate("Example Song cover", "Primary Artist", 180_000),
            candidate("Example Song radio edit", "Primary Artist", 180_000),
            candidate("Example Song tribute", "Primary Artist", 180_000),
            candidate("Example Song", "Primary Artist Tribute Band", 180_000),
            candidate("Example Song teaser", "Primary Artist", 180_000),
            candidate("Example Song acoustic", "Primary Artist", 180_000),
            candidate("Example Song remastered", "Primary Artist", 180_000),
            candidate("Example Song extended", "Primary Artist", 180_000),
        ] {
            assert!(select_candidate(&[rejected], &expected).is_err());
        }
    }

    #[test]
    fn qualifier_comparison_is_symmetric() {
        for title in ["Example Song remix", "Example Song radio edit"] {
            assert!(
                select_candidate(
                    &[candidate("Example Song", "Primary Artist", 180_000)],
                    &search(title)
                )
                .is_err()
            );
        }
    }

    #[test]
    fn tied_strong_candidates_are_ambiguous() {
        let candidates = [
            candidate("Example Song", "Primary Artist - Topic", 180_000),
            candidate("Example Song", "Primary Artist Official", 180_000),
        ];
        assert!(select_candidate(&candidates, &search("Example Song")).is_err());
    }

    #[test]
    fn artist_mentions_in_titles_do_not_make_reuploads_authoritative() {
        let expected = SiteSearch::new(
            "Faded Alan Walker",
            "Faded",
            vec!["Alan Walker".into()],
            None,
            None,
        )
        .expect("search");
        let candidates = [
            candidate("Alan Walker - Faded", "Alan Walker", 213_000),
            candidate("Alan Walker - Faded (Lyrics)", "7clouds", 213_000),
        ];

        let selected = select_candidate(&candidates, &expected).expect("official artist match");

        assert_eq!(selected.artist.as_deref(), Some("Alan Walker"));
    }

    #[test]
    fn spotify_duration_and_audio_presentation_avoid_a_long_music_video() {
        let expected = SiteSearch::new(
            "Thriller Michael Jackson",
            "Thriller",
            vec!["Michael Jackson".into()],
            Some(359_000),
            None,
        )
        .expect("search");
        let candidates = [
            candidate(
                "Michael Jackson - Thriller (Official 4K Video)",
                "Michael Jackson",
                822_000,
            ),
            candidate(
                "Michael Jackson - Thriller (Official Audio)",
                "Michael Jackson",
                358_000,
            ),
        ];

        let selected = select_candidate(&candidates, &expected).expect("duration match");

        assert_eq!(
            selected.title.as_deref(),
            Some("Michael Jackson - Thriller (Official Audio)")
        );
    }

    #[test]
    fn official_audio_is_preferred_to_an_equally_strong_music_video() {
        let expected = SiteSearch::new(
            "Example Song Primary Artist",
            "Example Song",
            vec!["Primary Artist".into()],
            None,
            None,
        )
        .expect("search");
        let candidates = [
            candidate(
                "Example Song (Official Music Video)",
                "Primary Artist",
                180_000,
            ),
            candidate("Example Song (Official Audio)", "Primary Artist", 180_000),
        ];

        let selected = select_candidate(&candidates, &expected).expect("audio match");

        assert_eq!(
            selected.title.as_deref(),
            Some("Example Song (Official Audio)")
        );
    }

    #[test]
    fn artist_name_in_title_cannot_replace_an_artist_match() {
        let expected = SiteSearch::new(
            "Faded Alan Walker",
            "Faded",
            vec!["Alan Walker".into()],
            None,
            None,
        )
        .expect("search");
        let reupload = candidate("Alan Walker - Faded", "Unrelated Channel", 213_000);

        assert!(select_candidate(&[reupload], &expected).is_err());
    }

    #[test]
    fn feature_aliases_and_official_video_labels_do_not_hide_a_strong_match() {
        let expected = SiteSearch::new(
            "Hall of Fame feat will i am The Script",
            "Hall of Fame (feat. will.i.am)",
            vec!["The Script".into()],
            None,
            None,
        )
        .expect("search");
        let candidates = [
            candidate(
                "The Script - Hall of Fame (Official Video) ft. will.i.am",
                "The Script",
                204_000,
            ),
            candidate(
                "The Script - Hall of Fame ft. will.i.am (Lyrics)",
                "Unrelated Channel",
                204_000,
            ),
        ];

        let selected = select_candidate(&candidates, &expected).expect("official artist match");

        assert_eq!(selected.artist.as_deref(), Some("The Script"));
    }

    #[test]
    fn combined_spotify_artists_match_an_official_contributor_upload() {
        let expected = SiteSearch::new(
            "Good Life with G-Eazy Kehlani",
            "Good Life (with G-Eazy & Kehlani)",
            vec!["G-Eazy, Kehlani".into()],
            None,
            None,
        )
        .expect("search");
        let candidates = [
            candidate(
                "Kehlani & G-Eazy - Good Life (from The Fate of the Furious: The Album) [Official Music Video]",
                "Kehlani",
                225_000,
            ),
            candidate("G-Eazy & Kehlani - Good Life (Lyrics)", "Reupload", 225_000),
        ];

        let selected = select_candidate(&candidates, &expected).expect("official contributor");

        assert_eq!(selected.artist.as_deref(), Some("Kehlani"));
    }

    #[test]
    fn tied_official_uploads_accept_a_combined_spotify_artist_credit() {
        let expected = SiteSearch::new(
            "Monster Alan Walker Emyrson Flora",
            "Monster",
            vec!["Alan Walker, Emyrson Flora".into()],
            None,
            None,
        )
        .expect("search");
        let candidates = [
            candidate(
                "Alan Walker, Emyrson Flora - Monster (Official Music Video)",
                "Alan Walker",
                159_000,
            ),
            candidate(
                "Alan Walker, Emyrson Flora - Monster (Official Lyric Video)",
                "Alan Walker",
                159_000,
            ),
        ];

        let selected = select_candidate(&candidates, &expected).expect("official contributor");

        assert_eq!(
            selected.title.as_deref(),
            Some("Alan Walker, Emyrson Flora - Monster (Official Music Video)")
        );
    }

    #[test]
    fn combined_artist_credits_do_not_authorize_reupload_ties() {
        let expected = SiteSearch::new(
            "Monster Alan Walker Emyrson Flora",
            "Monster",
            vec!["Alan Walker, Emyrson Flora".into()],
            None,
            None,
        )
        .expect("search");
        let candidates = [
            candidate(
                "Alan Walker, Emyrson Flora - Monster",
                "Unofficial Uploads",
                159_000,
            ),
            candidate(
                "Alan Walker, Emyrson Flora - Monster (Lyrics)",
                "Another Reupload",
                159_000,
            ),
        ];

        assert!(select_candidate(&candidates, &expected).is_err());
    }
}
