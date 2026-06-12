//! Case-insensitive subsequence fuzzy matching with simple contiguity scoring.

/// Match `needle` as a subsequence of `haystack`, case-insensitively (ASCII).
/// Returns `(score, matched char positions)` or `None` if it doesn't match.
/// Higher score = better match. Contiguous runs and early matches score higher.
#[must_use]
pub fn fuzzy_match(needle: &str, haystack: &str) -> Option<(i32, Vec<usize>)> {
    if needle.is_empty() {
        return Some((0, Vec::new()));
    }
    let hay: Vec<char> = haystack.chars().collect();
    let needle_chars: Vec<char> = needle.chars().collect();
    let mut positions = Vec::with_capacity(needle_chars.len());
    let mut hi = 0;
    for nc in &needle_chars {
        let mut found = false;
        while hi < hay.len() {
            if hay[hi].eq_ignore_ascii_case(nc) {
                positions.push(hi);
                hi += 1;
                found = true;
                break;
            }
            hi += 1;
        }
        if !found {
            return None;
        }
    }
    let mut score = 0i32;
    for pair in positions.windows(2) {
        if pair[1] == pair[0] + 1 {
            score += 5;
        } else {
            score -= i32::try_from(pair[1] - pair[0]).unwrap_or(i32::MAX);
        }
    }
    score -= i32::try_from(positions[0]).unwrap_or(i32::MAX);
    Some((score, positions))
}

#[cfg(test)]
mod tests {
    use super::fuzzy_match;

    #[test]
    fn empty_needle_matches_everything() {
        let (score, positions) = fuzzy_match("", "feature-x").unwrap();
        assert_eq!(score, 0);
        assert!(positions.is_empty());
    }

    #[test]
    fn contiguous_substring_matches() {
        let (_, positions) = fuzzy_match("feat", "feature-x").unwrap();
        assert_eq!(positions, vec![0, 1, 2, 3]);
    }

    #[test]
    fn subsequence_matches_with_gaps() {
        let (_, positions) = fuzzy_match("fx", "feature-x").unwrap();
        assert_eq!(positions, vec![0, 8]);
    }

    #[test]
    fn non_matching_needle_returns_none() {
        assert!(fuzzy_match("z", "feature").is_none());
        assert!(fuzzy_match("featurex-", "feature-x").is_none());
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(fuzzy_match("FEAT", "feature").is_some());
        assert!(fuzzy_match("feat", "FEATURE").is_some());
    }

    #[test]
    fn contiguous_match_scores_higher_than_scattered() {
        let (contiguous, _) = fuzzy_match("fix", "fix-login").unwrap();
        let (scattered, _) = fuzzy_match("fix", "feature-import-x").unwrap();
        assert!(contiguous > scattered);
    }

    #[test]
    fn earlier_match_scores_higher() {
        let (early, _) = fuzzy_match("api", "api-v2").unwrap();
        let (late, _) = fuzzy_match("api", "legacy-api").unwrap();
        assert!(early > late);
    }
}
