use std::{cmp::Reverse, collections::HashSet};

use super::{SiteError, SiteReference, SiteSearch};

const MINIMUM_SCORE: u16 = 70;
const MINIMUM_WINNING_MARGIN: u16 = 8;
const MAXIMUM_CANDIDATES_TO_RESOLVE: usize = 3;

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
    if best < MINIMUM_SCORE || best.saturating_sub(runner_up) < MINIMUM_WINNING_MARGIN {
        return Err(SiteError::NoSearchMatch);
    }
    let _ = candidate;
    scored.retain(|(score, _)| *score >= MINIMUM_SCORE);
    scored.truncate(MAXIMUM_CANDIDATES_TO_RESOLVE);
    Ok(scored.into_iter().map(|(_, candidate)| candidate).collect())
}

pub(super) fn duration_matches(preferred: Option<u64>, actual: u64) -> bool {
    preferred.is_none_or(|preferred| preferred.abs_diff(actual) <= duration_tolerance(preferred))
}

fn candidate_score(candidate: &SiteReference, search: &SiteSearch) -> Option<u16> {
    let title = normalize(candidate.title.as_deref()?);
    let expected = normalize(&search.expected_title);
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

    let haystack = normalize(&format!(
        "{} {}",
        candidate.title.as_deref().unwrap_or_default(),
        candidate.artist.as_deref().unwrap_or_default()
    ));
    let expected_context = normalize(&format!(
        "{} {}",
        search.expected_title,
        search.expected_artists.join(" ")
    ));
    if qualifier_conflict(&expected_context, &haystack) {
        return None;
    }
    let artist_score = artist_score(&haystack, &search.expected_artists)?;
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
    Some(title_score + artist_score + duration_score)
}

fn artist_score(haystack: &str, expected_artists: &[String]) -> Option<u16> {
    if expected_artists.is_empty() {
        return Some(10);
    }
    let matched = expected_artists
        .iter()
        .map(|artist| normalize(artist))
        .filter(|artist| phrase_present(haystack, artist) || token_overlap(artist, haystack) >= 70)
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
}
