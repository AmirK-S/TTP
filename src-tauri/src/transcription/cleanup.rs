// TTP - Talk To Paste
// Phase-1 deterministic cleanup — runs in Rust BEFORE the polish LLM.
//
// Handles ~80% of cases the LLM would otherwise spend tokens on:
//   - Explicit punctuation commands ("virgule" → ",")
//   - Tech dictionary normalization ("type script" → "TypeScript")
//   - Acronym joining ("A P I" → "API")
//   - Bigram repetition collapsing ("et et" → "et")
//   - Truncated prefix merging ("pro- programmer" → "programmer")
//   - Standalone filler removal ("euh", "um")
//
// What survives this pass goes to the polish LLM for semantic work
// (self-corrections, false starts, tone, ambiguity).
//
// Why deterministic first:
//   1. Faster than LLM (microseconds vs hundreds of ms)
//   2. Cheaper (fewer input tokens to Groq)
//   3. Safer (LLM can't hallucinate on text it doesn't see)
//   4. Testable (pure functions, no API)

use regex::Regex;
use std::sync::OnceLock;

/// Apply phase-1 deterministic cleanup. Idempotent on already-cleaned text.
pub fn cleanup(input: &str) -> String {
    let mut text = input.to_string();
    text = apply_punctuation_commands(&text);
    text = apply_tech_dictionary(&text);
    text = join_acronyms(&text);
    text = merge_truncated_prefixes(&text);
    text = collapse_repetitions(&text);
    text = remove_standalone_fillers(&text);
    text = normalize_whitespace(&text);
    text
}

/// Replace explicit punctuation/formatting commands. Word-boundary matched
/// and case-insensitive. Order matters: longer phrases first to avoid
/// partial matches.
fn apply_punctuation_commands(text: &str) -> String {
    let mut out = text.to_string();
    for (pattern, replacement) in PUNCTUATION_COMMANDS {
        let re = regex_or_panic(pattern);
        out = re.replace_all(&out, *replacement).into_owned();
    }
    out
}

/// Tech dictionary normalization. Bigrams like "type script" → "TypeScript".
/// Word-boundary matched, case-insensitive on the input side.
fn apply_tech_dictionary(text: &str) -> String {
    let mut out = text.to_string();
    for (pattern, replacement) in TECH_DICTIONARY {
        let re = regex_or_panic(pattern);
        out = re.replace_all(&out, *replacement).into_owned();
    }
    out
}

/// Join 2-5 consecutive single capital letters separated by spaces into an
/// acronym: "A P I" → "API", "U R L" → "URL".
fn join_acronyms(text: &str) -> String {
    static ACRONYM_RE: OnceLock<Regex> = OnceLock::new();
    let re = ACRONYM_RE.get_or_init(|| {
        // Match 2-5 single uppercase letters separated by spaces, surrounded
        // by word boundaries to avoid eating into surrounding text.
        Regex::new(r"\b([A-Z])(?: ([A-Z])){1,4}\b").unwrap()
    });
    re.replace_all(text, |caps: &regex::Captures| {
        caps[0].chars().filter(|c| !c.is_whitespace()).collect::<String>()
    })
    .into_owned()
}

/// Merge truncated prefix + completion: "pro- programmer" → "programmer".
/// If the prefix doesn't match the start of the next word, drop the prefix.
fn merge_truncated_prefixes(text: &str) -> String {
    static TRUNCATED_RE: OnceLock<Regex> = OnceLock::new();
    let re = TRUNCATED_RE.get_or_init(|| {
        Regex::new(r"\b([a-zA-ZàâäéèêëïîôöùûüçÀÂÄÉÈÊËÏÎÔÖÙÛÜÇ]{1,4})-\s+([a-zA-ZàâäéèêëïîôöùûüçÀÂÄÉÈÊËÏÎÔÖÙÛÜÇ]+)").unwrap()
    });
    re.replace_all(text, |caps: &regex::Captures| {
        let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let word = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        if word.to_lowercase().starts_with(&prefix.to_lowercase()) {
            word.to_string()
        } else {
            word.to_string()
        }
    })
    .into_owned()
}

/// Collapse immediately repeated tokens and bigrams: "et et" → "et",
/// "the the" → "the", "je vais je vais" → "je vais".
/// Preserves x3+ short affirmations ("non non non c'est faux") since those
/// are emphasis, not disfluency.
///
/// Implemented manually (token-level) instead of regex backreferences —
/// the `regex` crate is non-backtracking and doesn't support `\1`.
fn collapse_repetitions(text: &str) -> String {
    // Split into tokens preserving whitespace runs as separators. Keep both
    // tokens and separators so we can reassemble exactly.
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    for c in text.chars() {
        let is_word_char = c.is_alphanumeric() || c == '\'' || c == '-';
        if is_word_char {
            if !in_word && !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            current.push(c);
            in_word = true;
        } else {
            if in_word && !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            current.push(c);
            in_word = false;
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    // Extract word-only indices and their lowercased forms for comparison.
    let word_idx: Vec<usize> = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.chars().next().map_or(false, |c| c.is_alphanumeric()))
        .map(|(i, _)| i)
        .collect();

    // A repetition is two spoken words, so only words separated by nothing but
    // whitespace count. Without this "3.2.2" split into 3 / 2 / 2 and lost its
    // last ".2" (2026-09-16, trace 0076-6510, five times in one dictation);
    // "192.168.1.1", "1,1" and "ok/ok" went the same way.
    let spaced = |k: usize| {
        tokens[word_idx[k] + 1..word_idx[k + 1]]
            .iter()
            .all(|t| t.chars().all(char::is_whitespace))
    };

    // Mark indices to drop. We collapse exactly ONE pair of repetition at a
    // time so x3+ repetitions become x2 (preserves emphasis intent).
    let mut drop_word_positions: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut consumed_unigram: std::collections::HashSet<usize> = std::collections::HashSet::new();

    // Bigram pass first (longer match wins).
    let mut i = 0;
    while i + 3 < word_idx.len() {
        let a = &tokens[word_idx[i]].to_lowercase();
        let b = &tokens[word_idx[i + 1]].to_lowercase();
        let c = &tokens[word_idx[i + 2]].to_lowercase();
        let d = &tokens[word_idx[i + 3]].to_lowercase();
        if a == c && b == d && spaced(i) && spaced(i + 1) && spaced(i + 2) {
            // Drop the duplicate bigram (positions i+2 and i+3)
            drop_word_positions.insert(word_idx[i + 2]);
            drop_word_positions.insert(word_idx[i + 3]);
            consumed_unigram.insert(i);
            consumed_unigram.insert(i + 1);
            consumed_unigram.insert(i + 2);
            consumed_unigram.insert(i + 3);
            i += 4;
            continue;
        }
        i += 1;
    }

    // Unigram pass — collapse exactly one pair at a time so triples stay
    // as doubles (emphasis preserved).
    let mut j = 0;
    while j + 1 < word_idx.len() {
        if consumed_unigram.contains(&j) || consumed_unigram.contains(&(j + 1)) {
            j += 1;
            continue;
        }
        let a = &tokens[word_idx[j]].to_lowercase();
        let b = &tokens[word_idx[j + 1]].to_lowercase();
        if a == b && !a.is_empty() && spaced(j) {
            drop_word_positions.insert(word_idx[j + 1]);
            // Also drop the separator token immediately preceding word j+1
            // (typically a single space).
            if word_idx[j + 1] > 0 {
                drop_word_positions.insert(word_idx[j + 1] - 1);
            }
            consumed_unigram.insert(j);
            consumed_unigram.insert(j + 1);
            j += 2;
            continue;
        }
        j += 1;
    }

    // Reassemble, skipping dropped positions.
    let mut out = String::with_capacity(text.len());
    for (idx, tok) in tokens.iter().enumerate() {
        if drop_word_positions.contains(&idx) {
            continue;
        }
        out.push_str(tok);
    }
    out
}

/// Remove standalone filler words (surrounded by whitespace or sentence
/// boundaries). Does NOT touch "like" / "you know" / "enfin" mid-sentence
/// where they might carry meaning — that's the polish LLM's job.
///
/// "ah" is not on the list: it is an interjection with meaning ("Ah !",
/// "Ah d'accord"), and removing it left a bare "!" at the start of a sentence.
/// A trailing ellipsis goes with the filler, so "Euh... Perso." no longer
/// leaves "..".
fn remove_standalone_fillers(text: &str) -> String {
    static FILLER_RE: OnceLock<Regex> = OnceLock::new();
    let re = FILLER_RE.get_or_init(|| {
        Regex::new(r"(?i)\b(uh|um|euh|heu|hum|hmm|er)\b(?:\.{2,}|…|[,.])?\s*").unwrap()
    });
    re.replace_all(text, "").into_owned()
}

/// Collapse double spaces, normalize newlines. Idempotent.
fn normalize_whitespace(text: &str) -> String {
    static SPACE_RE: OnceLock<Regex> = OnceLock::new();
    let spaces = SPACE_RE.get_or_init(|| Regex::new(r"  +").unwrap());
    spaces.replace_all(text.trim(), " ").into_owned()
}

fn regex_or_panic(pattern: &str) -> Regex {
    Regex::new(pattern).expect("invalid cleanup regex")
}

/// Explicit punctuation commands (FR + EN). Longer patterns first to avoid
/// partial matches (e.g. "point d'interrogation" before "point").
const PUNCTUATION_COMMANDS: &[(&str, &str)] = &[
    // Multi-word commands first
    (r"(?i)\bpoint d'interrogation\b", "?"),
    (r"(?i)\bpoint d'exclamation\b", "!"),
    (r"(?i)\bpoint virgule\b", ";"),
    (r"(?i)\bdeux points\b", ":"),
    (r"(?i)\bnouvelle ligne\b", "\n"),
    (r"(?i)\bnouveau paragraphe\b", "\n\n"),
    (r"(?i)\bà la ligne\b", "\n"),
    (r"(?i)\bouvrez parenthèse\b", "("),
    (r"(?i)\bfermez parenthèse\b", ")"),
    (r"(?i)\bouvrez guillemets?\b", "\""),
    (r"(?i)\bfermez guillemets?\b", "\""),
    (r"(?i)\bouvrir parenthèse\b", "("),
    (r"(?i)\bfermer parenthèse\b", ")"),
    (r"(?i)\bquestion mark\b", "?"),
    (r"(?i)\bexclamation (mark|point)\b", "!"),
    (r"(?i)\bfull stop\b", "."),
    (r"(?i)\bnew line\b", "\n"),
    (r"(?i)\bnew paragraph\b", "\n\n"),
    (r"(?i)\bline break\b", "\n"),
    (r"(?i)\bopen paren(thesis)?\b", "("),
    (r"(?i)\bclose paren(thesis)?\b", ")"),
    (r"(?i)\bopen bracket\b", "["),
    (r"(?i)\bclose bracket\b", "]"),
    (r"(?i)\bopen quote\b", "\""),
    (r"(?i)\bclose quote\b", "\""),
    // Single-word commands. No bare "point" or "period": they are ordinary
    // words far more often than commands — "du point de vue", "le point 5",
    // "point important" each came out with a "." in the middle (four times in
    // the maintainer's trace by 2026-09-16), and "a period of time" would too.
    // The polish model still turns a spoken "point" into punctuation in context.
    (r"(?i)\bvirgule\b", ","),
    (r"(?i)\bcomma\b", ","),
    (r"(?i)\bcolon\b", ":"),
    (r"(?i)\bsemicolon\b", ";"),
];

/// Tech dictionary — normalize commonly mis-segmented technical terms.
/// Pattern → replacement. Word-boundary + case-insensitive on input.
const TECH_DICTIONARY: &[(&str, &str)] = &[
    // Languages / frameworks (segmentation errors)
    (r"(?i)\btype script\b", "TypeScript"),
    (r"(?i)\bjava script\b", "JavaScript"),
    (r"(?i)\bnext (point )?j s\b", "Next.js"),
    (r"(?i)\bnext jay esse\b", "Next.js"),
    (r"(?i)\breact (point )?j s\b", "React.js"),
    (r"(?i)\breact jay esse\b", "React.js"),
    (r"(?i)\bnode (point )?j s\b", "Node.js"),
    (r"(?i)\bnode jay esse\b", "Node.js"),
    (r"(?i)\bvue (point )?j s\b", "Vue.js"),
    (r"(?i)\btail wind( css)?\b", "Tailwind"),
    (r"(?i)\bsupa base\b", "Supabase"),
    (r"(?i)\bpostgres q l\b", "PostgreSQL"),
    (r"(?i)\bpostgre sql\b", "PostgreSQL"),
    (r"(?i)\bpost grès\b", "Postgres"),
    (r"(?i)\bmongo d b\b", "MongoDB"),
    (r"(?i)\bredis\b", "Redis"),
    (r"(?i)\bkubernetes\b", "Kubernetes"),
    (r"(?i)\bdocker\b", "Docker"),
    (r"(?i)\bgraph q l\b", "GraphQL"),
    (r"(?i)\bgraph que elle\b", "GraphQL"),
    // Tools / brands
    (r"(?i)\bv s code\b", "VS Code"),
    (r"(?i)\bvesque code\b", "VS Code"),
    (r"(?i)\bvee es code\b", "VS Code"),
    (r"(?i)\bgit hub\b", "GitHub"),
    (r"(?i)\bgit lab\b", "GitLab"),
    (r"(?i)\bbit bucket\b", "Bitbucket"),
    (r"(?i)\bopen a i\b", "OpenAI"),
    (r"(?i)\banthropique\b", "Anthropic"),
    (r"(?i)\bclaude code\b", "Claude Code"),
    (r"(?i)\bchat g p t\b", "ChatGPT"),
    (r"(?i)\bchat gpt\b", "ChatGPT"),
    (r"(?i)\bcursor\b", "Cursor"),
    (r"(?i)\bgroq\b", "Groq"),
    (r"(?i)\bwhisper\b", "Whisper"),
    // Common acronyms in tech vocabulary
    (r"(?i)\bes q l\b", "SQL"),
    (r"(?i)\besquel\b", "SQL"),
    (r"(?i)\bj son\b", "JSON"),
    (r"(?i)\bj s o n\b", "JSON"),
    (r"(?i)\bhttps\b", "HTTPS"),
    (r"(?i)\bhttp\b", "HTTP"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_strips_standalone_fillers() {
        let out = cleanup("euh donc je pense que oui");
        assert!(!out.to_lowercase().contains("euh"));
        assert!(out.contains("donc"));
    }

    #[test]
    fn cleanup_normalizes_typescript() {
        let out = cleanup("write a type script function");
        assert!(out.contains("TypeScript"));
    }

    #[test]
    fn cleanup_joins_api_acronym() {
        let out = cleanup("call the A P I now");
        assert!(out.contains("API"));
        assert!(!out.contains("A P I"));
    }

    #[test]
    fn cleanup_collapses_unigram_repetition() {
        let out = cleanup("et et envoie le mail");
        assert_eq!(out, "et envoie le mail");
    }

    #[test]
    fn cleanup_collapses_bigram_repetition() {
        let out = cleanup("je vais je vais le faire");
        // Bigram collapse leaves single "je vais"
        assert_eq!(out.to_lowercase(), "je vais le faire");
    }

    #[test]
    fn cleanup_keeps_repeated_digits_in_numbers() {
        // Read aloud on 2026-09-16: every "3.2.2" came out as "3.2".
        assert_eq!(
            cleanup("installe la 3.2.2 et le tag v3.2.2"),
            "installe la 3.2.2 et le tag v3.2.2"
        );
        assert_eq!(cleanup("le serveur 192.168.1.1"), "le serveur 192.168.1.1");
        assert_eq!(cleanup("1,1 million et 2.2.2.2"), "1,1 million et 2.2.2.2");
        assert_eq!(cleanup("ok/ok et 10:10"), "ok/ok et 10:10");
    }

    #[test]
    fn cleanup_keeps_point_as_a_word() {
        // All four from the maintainer's trace.
        for raw in [
            "du point de vue comptable",
            "je suis d'accord avec le point 5",
            "juste le point numéro 4",
            "Par contre, point important",
            "over a period of time",
        ] {
            assert_eq!(cleanup(raw), raw);
        }
    }

    #[test]
    fn cleanup_keeps_ah_and_takes_the_fillers_ellipsis() {
        assert_eq!(cleanup("Ah ! Excellent"), "Ah ! Excellent");
        assert_eq!(cleanup("pour les applications... Euh... Perso."), "pour les applications... Perso.");
        assert_eq!(cleanup("je sais pas Euh... Je sais pas"), "je sais pas Je sais pas");
    }

    #[test]
    fn cleanup_keeps_repetition_across_punctuation() {
        // "moi ? Moi," lost its question; "de % de" lost the percent sign.
        assert_eq!(cleanup("près de moi ? Moi, je pense"), "près de moi ? Moi, je pense");
        assert_eq!(cleanup("le moins de % de taxes"), "le moins de % de taxes");
    }

    #[test]
    fn cleanup_still_collapses_spoken_repetition_around_numbers() {
        assert_eq!(cleanup("la la 3.2.2"), "la 3.2.2");
        assert_eq!(cleanup("version 3 3 sort"), "version 3 sort");
    }

    #[test]
    fn cleanup_preserves_x3_emphasis() {
        // Triple repetition stays as double (collapses ONE pair only) — keeps
        // the emphasis intent visible to the polish LLM.
        let out = cleanup("non non non c'est faux");
        assert!(out.to_lowercase().contains("non non"));
    }

    #[test]
    fn cleanup_replaces_virgule_command() {
        let out = cleanup("salut Paul virgule comment vas-tu");
        assert!(out.contains(","));
        assert!(!out.to_lowercase().contains("virgule"));
    }

    #[test]
    fn cleanup_replaces_question_mark_command() {
        let out = cleanup("tu viens point d'interrogation");
        assert!(out.contains("?"));
        assert!(!out.to_lowercase().contains("point d'interrogation"));
    }

    #[test]
    fn cleanup_normalizes_whitespace() {
        let out = cleanup("hello    world  ");
        assert_eq!(out, "hello world");
    }

    #[test]
    fn cleanup_idempotent() {
        let once = cleanup("call the A P I and check supa base");
        let twice = cleanup(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn cleanup_keeps_meaningful_text_intact() {
        let raw = "Hello Marie, can you check the deploy please";
        let out = cleanup(raw);
        // Should be near-identical — no fillers, no commands, no repetitions
        assert!(out.contains("Marie"));
        assert!(out.contains("deploy"));
        assert!(out.contains("check"));
    }

    #[test]
    fn cleanup_handles_truncated_prefix() {
        let out = cleanup("pro- programmer the function");
        // The prefix "pro-" should be merged into "programmer"
        assert!(out.contains("programmer"));
        assert!(!out.contains("pro-"));
    }

    #[test]
    fn cleanup_chatgpt_normalization() {
        let out = cleanup("ask chat g p t about it");
        assert!(out.contains("ChatGPT"));
    }

    #[test]
    fn cleanup_vscode_normalization() {
        let out = cleanup("open v s code now");
        assert!(out.contains("VS Code"));
    }
}
