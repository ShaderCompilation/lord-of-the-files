//! Pure string transforms — one per pipeline step. All operate on `char` boundaries so
//! they are Unicode-safe, and all are deterministic and unit-tested below.

use regex::Regex;

use crate::types::{AffixPosition, CaseMode, InsertPosition, RemoveFrom};

/// Literal find & replace, optionally case-insensitive / first-only.
pub fn find_replace(
    s: &str,
    find: &str,
    replace: &str,
    case_sensitive: bool,
    all_occurrences: bool,
) -> String {
    if find.is_empty() {
        return s.to_string();
    }
    if case_sensitive {
        if all_occurrences {
            s.replace(find, replace)
        } else {
            s.replacen(find, replace, 1)
        }
    } else {
        replace_ci(s, find, replace, all_occurrences)
    }
}

/// Case-insensitive literal replace (byte-safe via lowercase scanning of `char`s).
fn replace_ci(s: &str, find: &str, replace: &str, all: bool) -> String {
    let hay: Vec<char> = s.chars().collect();
    let needle: Vec<char> = find.chars().collect();
    let hay_lower: Vec<char> = s.to_lowercase().chars().collect();
    let needle_lower: Vec<char> = find.to_lowercase().chars().collect();
    // Lowercasing can change length for some scripts; fall back to a simple path then.
    if hay_lower.len() != hay.len() || needle_lower.len() != needle.len() {
        return s.to_string();
    }

    let mut out = String::new();
    let mut i = 0;
    let mut replaced = false;
    while i < hay.len() {
        let matches = !needle_lower.is_empty()
            && i + needle_lower.len() <= hay.len()
            && hay_lower[i..i + needle_lower.len()] == needle_lower[..];
        if matches && (all || !replaced) {
            out.push_str(replace);
            i += needle_lower.len();
            replaced = true;
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    out
}

/// Regex replace-all with `$1` / `${name}` capture substitution.
pub fn regex_replace(s: &str, re: &Regex, replacement: &str) -> String {
    re.replace_all(s, replacement).into_owned()
}

pub fn change_case(s: &str, mode: CaseMode) -> String {
    match mode {
        CaseMode::Lower => s.to_lowercase(),
        CaseMode::Upper => s.to_uppercase(),
        CaseMode::Title => title_case(s),
        CaseMode::Sentence => sentence_case(s),
        CaseMode::Camel => join_words(s, "", true),
        CaseMode::Snake => words(s).join("_").to_lowercase(),
        CaseMode::Kebab => words(s).join("-").to_lowercase(),
    }
}

/// Capitalize the first letter of each word, preserving separators.
fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut at_boundary = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if at_boundary {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            at_boundary = false;
        } else {
            out.push(c);
            at_boundary = true;
        }
    }
    out
}

/// Lowercase everything, then capitalize the first alphabetic char.
fn sentence_case(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut done = false;
    for c in lower.chars() {
        if !done && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            done = true;
        } else {
            out.push(c);
        }
    }
    out
}

/// Split into words, breaking on separators and camelCase boundaries.
fn words(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut cur = String::new();
    let mut prev: Option<char> = None;
    for c in s.chars() {
        if c.is_whitespace() || c == '_' || c == '-' || c == '.' {
            if !cur.is_empty() {
                result.push(std::mem::take(&mut cur));
            }
            prev = None;
            continue;
        }
        if let Some(p) = prev {
            if (p.is_lowercase() || p.is_ascii_digit()) && c.is_uppercase() && !cur.is_empty() {
                result.push(std::mem::take(&mut cur));
            }
        }
        cur.push(c);
        prev = Some(c);
    }
    if !cur.is_empty() {
        result.push(cur);
    }
    result
}

/// Join words with a separator, capitalizing each (camelCase keeps first word lower).
fn join_words(s: &str, sep: &str, lower_first: bool) -> String {
    let ws = words(s);
    let mut out = String::new();
    for (i, w) in ws.iter().enumerate() {
        if i > 0 {
            out.push_str(sep);
        }
        if i == 0 && lower_first {
            out.push_str(&w.to_lowercase());
        } else {
            out.push_str(&capitalize(w));
        }
    }
    out
}

fn capitalize(w: &str) -> String {
    let mut chars = w.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        None => String::new(),
    }
}

pub fn insert(s: &str, text: &str, position: InsertPosition, index: i64) -> String {
    match position {
        InsertPosition::Prefix => format!("{text}{s}"),
        InsertPosition::Suffix => format!("{s}{text}"),
        InsertPosition::AtIndex => {
            let chars: Vec<char> = s.chars().collect();
            let i = index.clamp(0, chars.len() as i64) as usize;
            let mut out: String = chars[..i].iter().collect();
            out.push_str(text);
            out.extend(&chars[i..]);
            out
        }
    }
}

pub fn remove(s: &str, from: RemoveFrom, count: usize, index: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let (start, end) = match from {
        RemoveFrom::Start => (0, count.min(len)),
        RemoveFrom::End => (len.saturating_sub(count), len),
        RemoveFrom::Index => {
            let start = index.min(len);
            (start, (start + count).min(len))
        }
    };
    let mut out: String = chars[..start].iter().collect();
    out.extend(&chars[end..]);
    out
}

pub fn clean_up(
    s: &str,
    trim: bool,
    collapse_whitespace: bool,
    spaces_to: Option<&str>,
    strip_diacritics: bool,
) -> String {
    let mut out = s.to_string();
    if strip_diacritics {
        out = out.chars().map(fold_diacritic).collect();
    }
    if collapse_whitespace {
        let mut collapsed = String::with_capacity(out.len());
        let mut in_ws = false;
        for c in out.chars() {
            if c.is_whitespace() {
                if !in_ws {
                    collapsed.push(' ');
                }
                in_ws = true;
            } else {
                collapsed.push(c);
                in_ws = false;
            }
        }
        out = collapsed;
    }
    if trim {
        out = out.trim().to_string();
    }
    if let Some(rep) = spaces_to {
        out = out.replace(' ', rep);
    }
    out
}

/// Fold common Latin diacritics to ASCII. Intentionally small (v1 scope); unmapped chars
/// pass through unchanged.
fn fold_diacritic(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => 'A',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => 'E',
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ĭ' | 'į' => 'i',
        'Ì' | 'Í' | 'Î' | 'Ï' | 'Ī' | 'Ĭ' | 'Į' => 'I',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => 'O',
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => 'U',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'Ñ' | 'Ń' | 'Ņ' | 'Ň' => 'N',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => 'C',
        'ý' | 'ÿ' => 'y',
        'Ý' | 'Ÿ' => 'Y',
        'š' | 'ś' | 'ŝ' | 'ş' => 's',
        'Š' | 'Ś' | 'Ŝ' | 'Ş' => 'S',
        'ž' | 'ź' | 'ż' => 'z',
        'Ž' | 'Ź' | 'Ż' => 'Z',
        _ => c,
    }
}

/// Zero-pad an integer to a width (negatives are left unpadded).
fn format_counter(num: i64, padding: usize) -> String {
    if num >= 0 {
        format!("{num:0>padding$}")
    } else {
        num.to_string()
    }
}

pub fn counter_affix(
    s: &str,
    value: i64,
    padding: usize,
    separator: &str,
    position: AffixPosition,
) -> String {
    let num = format_counter(value, padding);
    match position {
        AffixPosition::Prefix => format!("{num}{separator}{s}"),
        AffixPosition::Suffix => format!("{s}{separator}{num}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_replace_basic() {
        assert_eq!(find_replace("foo_bar_foo", "foo", "x", true, true), "x_bar_x");
        assert_eq!(find_replace("foo_bar_foo", "foo", "x", true, false), "x_bar_foo");
    }

    #[test]
    fn find_replace_case_insensitive() {
        assert_eq!(find_replace("FooBar", "foo", "x", false, true), "xBar");
        assert_eq!(find_replace("FOO foo", "foo", "x", false, true), "x x");
    }

    #[test]
    fn regex_groups() {
        let re = Regex::new(r"(\d+)-(\d+)").unwrap();
        // Braces are required when a group number is followed by a word char, otherwise
        // `$2_` is parsed as a capture group *named* "2_".
        assert_eq!(regex_replace("12-34", &re, "${2}_${1}"), "34_12");
        assert_eq!(regex_replace("a-b", &re, "$2$1"), "a-b");
    }

    #[test]
    fn case_modes() {
        assert_eq!(change_case("hello world", CaseMode::Title), "Hello World");
        assert_eq!(change_case("HELLO world", CaseMode::Sentence), "Hello world");
        assert_eq!(change_case("my_file name", CaseMode::Camel), "myFileName");
        assert_eq!(change_case("MyFileName", CaseMode::Snake), "my_file_name");
        assert_eq!(change_case("My File Name", CaseMode::Kebab), "my-file-name");
    }

    #[test]
    fn insert_and_remove() {
        assert_eq!(insert("name", "pre_", InsertPosition::Prefix, 0), "pre_name");
        assert_eq!(insert("name", "X", InsertPosition::AtIndex, 2), "naXme");
        assert_eq!(remove("0123456789", RemoveFrom::Start, 3, 0), "3456789");
        assert_eq!(remove("0123456789", RemoveFrom::End, 3, 0), "0123456");
        assert_eq!(remove("0123456789", RemoveFrom::Index, 2, 3), "0125678 9".replace(' ', ""));
    }

    #[test]
    fn cleanup_collapse_trim_spaces() {
        assert_eq!(
            clean_up("  a   b  ", true, true, Some("_"), false),
            "a_b"
        );
        assert_eq!(clean_up("café", false, false, None, true), "cafe");
    }

    #[test]
    fn counter_padding() {
        assert_eq!(
            counter_affix("img", 7, 3, "_", AffixPosition::Suffix),
            "img_007"
        );
        assert_eq!(
            counter_affix("img", 12, 2, "-", AffixPosition::Prefix),
            "12-img"
        );
    }
}
