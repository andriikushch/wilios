//! "Did you mean" suggestion ranking (spec §7). Hand-rolled, dependency-free
//! Damerau-Levenshtein (optimal-string-alignment variant — transpositions of
//! adjacent characters count as one edit, standard for typo detection on
//! short identifiers) plus the exact ranking/tie-break/cap rules from the
//! spec, built directly against `wilios_core::stdlib::all_symbols()` for
//! stdlib candidates and the resolver's own scope for in-file/imported ones
//! — never a second symbol table.

use std::collections::{BTreeMap, HashSet};

use super::{Confidence, Suggestion};

/// One name that could be suggested as a replacement for an unresolved
/// identifier. `priority` orders *candidate sources*, lowest first: 0 = a
/// binding in scope at the error site, 1 = a name imported into the file,
/// 2 = a builtin/preset from the stdlib symbol table — matching spec §7's
/// candidate-set priority order, used only to break ties within an
/// otherwise-equal confidence/distance group.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub text: String,
    pub priority: u8,
}

fn source_label(priority: u8) -> &'static str {
    match priority {
        0 => "a binding in scope",
        1 => "an imported name",
        _ => "a stdlib symbol",
    }
}

fn edit_distance_threshold(len: usize) -> usize {
    if len <= 4 {
        1
    } else if len <= 9 {
        2
    } else {
        3
    }
}

/// Optimal-string-alignment distance: Levenshtein plus adjacent-character
/// transposition as a single edit. Case-sensitive — callers lowercase both
/// sides first when a case-insensitive comparison is wanted.
fn damerau_levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (la, lb) = (a.len(), b.len());

    let mut d = vec![vec![0usize; lb + 1]; la + 1];
    for (i, row) in d.iter_mut().enumerate().take(la + 1) {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }

    for i in 1..=la {
        for j in 1..=lb {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let mut val = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                val = val.min(d[i - 2][j - 2] + 1);
            }
            d[i][j] = val;
        }
    }
    d[la][lb]
}

fn sort_group(group: &mut [&Candidate]) {
    group.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.text.cmp(&b.text))
    });
}

/// Ranks `candidates` against `target` per spec §7 and returns at most 3
/// suggestions, ordered best-first, deterministically for a given input
/// (never dependent on hashmap iteration order).
///
/// Tiers, in priority order, each filling remaining slots (up to 3 total):
/// 1. Exact match ignoring case → `Confidence::High`.
/// 2. Damerau-Levenshtein distance ≤ threshold (1 for ≤4 chars, 2 for 5-9,
///    3 for 10+), grouped by exact distance value, ascending distance first;
///    distance 1 → `High`, otherwise → `Medium`.
/// 3. Substring/prefix match either direction (case-insensitive) → `Low`.
///
/// Within every tier/group, ties are broken by `priority` then
/// alphabetically. If a single tier/distance-group has *more than 3*
/// equally-ranked candidates, it contributes nothing at all — showing 3 of
/// (say) 8 equally-likely candidates would imply a precision the ranking
/// doesn't have, and usually means the target wasn't a typo of anything in
/// particular.
pub fn suggest(target: &str, candidates: &[Candidate]) -> Vec<Suggestion> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let target_lower = target.to_lowercase();
    let mut result: Vec<Suggestion> = Vec::new();
    // Candidates already accounted for by a stronger tier (whether emitted
    // or suppressed for ambiguity) never fall through to a weaker tier.
    let mut considered: HashSet<&str> = HashSet::new();

    // Tier 1: exact match ignoring case.
    let mut exact: Vec<&Candidate> = candidates
        .iter()
        .filter(|c| c.text.to_lowercase() == target_lower)
        .collect();
    for c in &exact {
        considered.insert(c.text.as_str());
    }
    if !exact.is_empty() && exact.len() <= 3 {
        sort_group(&mut exact);
        for c in exact {
            result.push(Suggestion {
                replacement: c.text.clone(),
                confidence: Confidence::High,
                reason: "exact match ignoring case".to_string(),
            });
        }
    }

    // Tier 2: edit-distance matches, grouped by exact distance, ascending.
    if result.len() < 3 {
        let mut by_distance: BTreeMap<usize, Vec<&Candidate>> = BTreeMap::new();
        for c in candidates {
            if considered.contains(c.text.as_str()) {
                continue;
            }
            let threshold = edit_distance_threshold(c.text.chars().count());
            let dist = damerau_levenshtein(&target_lower, &c.text.to_lowercase());
            if dist > 0 && dist <= threshold {
                by_distance.entry(dist).or_default().push(c);
            }
        }

        for (dist, mut group) in by_distance {
            for c in &group {
                considered.insert(c.text.as_str());
            }
            if result.len() >= 3 || group.len() > 3 {
                continue;
            }
            sort_group(&mut group);
            let confidence = if dist == 1 {
                Confidence::High
            } else {
                Confidence::Medium
            };
            for c in group {
                if result.len() >= 3 {
                    break;
                }
                result.push(Suggestion {
                    replacement: c.text.clone(),
                    confidence,
                    reason: format!("edit distance {dist} from {}", source_label(c.priority)),
                });
            }
        }
    }

    // Tier 3: substring/prefix match either direction.
    if result.len() < 3 {
        let mut substr: Vec<&Candidate> = candidates
            .iter()
            .filter(|c| {
                if considered.contains(c.text.as_str()) {
                    return false;
                }
                let cl = c.text.to_lowercase();
                cl.contains(&target_lower) || target_lower.contains(&cl)
            })
            .collect();
        if !substr.is_empty() && substr.len() <= 3 {
            sort_group(&mut substr);
            for c in substr {
                if result.len() >= 3 {
                    break;
                }
                result.push(Suggestion {
                    replacement: c.text.clone(),
                    confidence: Confidence::Low,
                    reason: format!("shares a substring with {}", source_label(c.priority)),
                });
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::all_symbols;

    fn stdlib_candidates() -> Vec<Candidate> {
        all_symbols()
            .map(|s| Candidate {
                text: s.name.to_string(),
                priority: 2,
            })
            .collect()
    }

    /// A single-character-substitution typo of `name`: the middle character
    /// is replaced with `q` — a letter that appears in no stdlib symbol
    /// name — so the typo is always edit distance 1 from `name` itself and
    /// (unlike a deletion, which can coincidentally land on a different
    /// real symbol — e.g. deleting the middle of "brass" produces "brss",
    /// which is *also* distance 1 from the unrelated symbol "bass")
    /// guaranteed not to be close to any other symbol.
    fn typo_of(name: &str) -> String {
        assert!(
            !name.contains('q'),
            "typo_of assumes no symbol contains 'q': {name}"
        );
        let mut chars: Vec<char> = name.chars().collect();
        let mid = chars.len() / 2;
        chars[mid] = 'q';
        chars.into_iter().collect()
    }

    #[test]
    fn every_stdlib_symbol_typo_ranks_the_original_first() {
        let candidates = stdlib_candidates();
        for symbol in all_symbols() {
            let typo = typo_of(symbol.name);
            let suggestions = suggest(&typo, &candidates);
            assert!(
                !suggestions.is_empty(),
                "no suggestions for typo {:?} of {:?}",
                typo,
                symbol.name
            );
            assert_eq!(
                suggestions[0].replacement, symbol.name,
                "typo {:?} of {:?} did not rank the original first (got {:?})",
                typo, symbol.name, suggestions
            );
            assert_eq!(suggestions[0].confidence, Confidence::High);
        }
    }

    #[test]
    fn exact_case_insensitive_match_is_high_confidence() {
        let candidates = vec![Candidate {
            text: "Transpose".to_string(),
            priority: 2,
        }];
        let s = suggest("transpose", &candidates);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].replacement, "Transpose");
        assert_eq!(s[0].confidence, Confidence::High);
    }

    #[test]
    fn no_candidates_qualify_returns_empty() {
        let candidates = vec![Candidate {
            text: "print".to_string(),
            priority: 2,
        }];
        assert!(suggest("xyzzyplugh", &candidates).is_empty());
    }

    #[test]
    fn is_deterministic_across_repeated_calls() {
        let candidates = stdlib_candidates();
        let a = suggest("trnaspose", &candidates);
        let b = suggest("trnaspose", &candidates);
        assert_eq!(a, b);
    }

    #[test]
    fn exactly_three_ties_at_same_distance_are_all_emitted() {
        // "aaa", "aab", "aac" are each edit distance 1 from "aax" — a group
        // of exactly 3, at the cap boundary, so none should be suppressed.
        let candidates: Vec<Candidate> = vec!["aaa", "aab", "aac"]
            .into_iter()
            .map(|t| Candidate {
                text: t.to_string(),
                priority: 0,
            })
            .collect();
        let s = suggest("aax", &candidates);
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn more_than_three_ties_at_same_distance_contribute_nothing() {
        // Five candidates all at edit distance 1 from "aax" ("aaa","aab",...
        // each differ by one substitution) — the group has 5 > 3 members,
        // so this tier must contribute nothing, and there's nothing else to
        // fall back to, so the result is empty.
        let candidates: Vec<Candidate> = vec!["aaa", "aab", "aac", "aad", "aae"]
            .into_iter()
            .map(|t| Candidate {
                text: t.to_string(),
                priority: 0,
            })
            .collect();
        let s = suggest("aax", &candidates);
        assert!(s.is_empty(), "expected no suggestions, got {:?}", s);
    }
}
