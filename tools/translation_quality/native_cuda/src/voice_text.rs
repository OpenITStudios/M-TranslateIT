//! Bounded English speech normalization, scoped to the selected native TTS.
//! No translation context, model prompt, network, regex engine or new dependency.
use super::Error;
use std::borrow::Cow;

const MAX_TEXT_BYTES: usize = 4000;
const ONES: [&str; 20] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen"];
const TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];
const SCALES: [&str; 5] = ["", " thousand", " million", " billion", " trillion"];

fn under_thousand(n: u64) -> String {
    let mut result = String::new();
    if n >= 100 {
        result.push_str(ONES[(n / 100) as usize]);
        result.push_str(" hundred");
        if n % 100 != 0 { result.push(' '); }
    }
    let rem = n % 100;
    if rem >= 20 {
        result.push_str(TENS[(rem / 10) as usize]);
        if rem % 10 != 0 {
            result.push('-');
            result.push_str(ONES[(rem % 10) as usize]);
        }
    } else if rem != 0 || n == 0 {
        result.push_str(ONES[rem as usize]);
    }
    result
}

fn cardinal(mut n: u64) -> String {
    if n == 0 { return "zero".into(); }
    let mut groups = Vec::new();
    for scale in SCALES {
        let part = n % 1000;
        if part != 0 { groups.push(format!("{}{scale}", under_thousand(part))); }
        n /= 1000;
        if n == 0 { break; }
    }
    groups.reverse();
    groups.join(" ")
}

fn digits(s: &str) -> String {
    s.bytes().map(|b| ONES[(b - b'0') as usize]).collect::<Vec<_>>().join(" ")
}

fn ascii_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

// A comma is a thousands separator only for complete, well-formed groups.
fn integer(s: &str) -> Option<String> {
    if ascii_digits(s) { return Some(s.to_owned()); }
    let mut groups = s.split(',');
    let first = groups.next()?;
    if !ascii_digits(first) || first.len() > 3 { return None; }
    let mut result = first.to_owned();
    for group in groups {
        if group.len() != 3 || !ascii_digits(group) { return None; }
        result.push_str(group);
    }
    Some(result)
}

fn year(n: u64) -> String {
    let hi = n / 100;
    let lo = n % 100;
    if n == 2000 || (hi == 20 && lo < 10) || (hi % 10 == 0 && lo == 0) {
        cardinal(n)
    } else if lo == 0 {
        format!("{} hundred", cardinal(hi))
    } else if lo < 10 {
        format!("{} oh {}", cardinal(hi), cardinal(lo))
    } else {
        format!("{} {}", cardinal(hi), cardinal(lo))
    }
}

fn integer_words(s: &str, year_ok: bool) -> String {
    if s.len() > 15 || (s.len() > 1 && s.starts_with('0')) { return digits(s); }
    let n = s.parse::<u64>().expect("bounded ASCII integer");
    if year_ok && s.len() == 4 && (1100..=2099).contains(&n) { year(n) } else { cardinal(n) }
}

fn number(s: &str, year_ok: bool) -> Option<String> {
    let (sign, s) = if let Some(rest) = s.strip_prefix('-').or_else(|| s.strip_prefix('−')) {
        ("minus ", rest)
    } else if let Some(rest) = s.strip_prefix('+') { ("plus ", rest) } else { ("", s) };
    let (whole, frac) = s.split_once('.').map_or((s, None), |(a,b)| (a, Some(b)));
    let whole = integer(whole)?;
    if let Some(frac) = frac {
        if !ascii_digits(frac) { return None; }
        Some(format!("{sign}{} point {}", integer_words(&whole, false), digits(frac)))
    } else {
        Some(format!("{sign}{}", integer_words(&whole, year_ok && sign.is_empty() && !s.contains(','))))
    }
}

fn ordinal(s: &str) -> Option<String> {
    let split = s.len().checked_sub(2)?;
    if !s.is_char_boundary(split) { return None; }
    let (n, suffix) = s.split_at(split);
    if !ascii_digits(n) || n.len() > 15 || (n.len() > 1 && n.starts_with('0')) { return None; }
    let n = n.parse::<u64>().ok()?;
    let expected = if (11..=13).contains(&(n % 100)) { "th" } else {
        match n % 10 { 1 => "st", 2 => "nd", 3 => "rd", _ => "th" }
    };
    if !suffix.eq_ignore_ascii_case(expected) { return None; }
    let words = cardinal(n);
    let split = words.rfind([' ', '-']).map_or(0, |i| i + 1);
    let (prefix, last) = words.split_at(split);
    let last = match last {
        "one" => "first".into(), "two" => "second".into(), "three" => "third".into(),
        "five" => "fifth".into(), "eight" => "eighth".into(), "nine" => "ninth".into(),
        "twelve" => "twelfth".into(),
        _ => if let Some(base) = last.strip_suffix('y') { format!("{base}ieth") } else { format!("{last}th") },
    };
    Some(format!("{prefix}{last}"))
}

fn currency(s: &str) -> Option<String> {
    let (sign, s) = if let Some(s) = s.strip_prefix('-').or_else(|| s.strip_prefix('−')) {
        ("minus ", s)
    } else if let Some(s) = s.strip_prefix('+') { ("plus ", s) } else { ("", s) };
    let (cur, amount) = if let Some(c) = s.chars().next().filter(|c| matches!(*c, ' {
        (c, &s[c.len_utf8()..])
    } else {
        let c = s.chars().last().filter(|c| matches!(c, '$'|'£'|'€'|'¥'))?;
        (c, &s[..s.len() - c.len_utf8()])
    };
    let (unit, plural, minor, minors) = match cur {
        '$' => ("dollar", "dollars", "cent", "cents"),
        '£' => ("pound", "pounds", "penny", "pence"),
        '€' => ("euro", "euros", "cent", "cents"),
        '¥' => ("yen", "yen", "", ""),
        _ => return None,
    };
    let (whole, frac) = amount.split_once('.').map_or((amount, None), |(a,b)| (a, Some(b)));
    let whole = integer(whole)?;
    if whole.len() > 15 { return None; }
    let n = whole.parse::<u64>().ok()?;
    if let Some(frac) = frac {
        if !ascii_digits(frac) { return None; }
        if frac.len() <= 2 && !minor.is_empty() {
            let cents = frac.parse::<u64>().ok()? * if frac.len() == 1 { 10 } else { 1 };
            let major = format!("{} {}", cardinal(n), if n == 1 { unit } else { plural });
            let small = format!("{} {}", cardinal(cents), if cents == 1 { minor } else { minors });
            return Some(format!("{sign}{}", match (n, cents) {
                (_, 0) => major, (0, _) => small, _ => format!("{major} and {small}"),
            }));
        }
        return Some(format!("{sign}{} {plural}", number(amount, false)?));
    }
    Some(format!("{sign}{} {}", cardinal(n), if n == 1 { unit } else { plural }))
}

fn clock(s: &str) -> Option<String> {
    let (h, m) = s.split_once(':')?;
    if !(1..=2).contains(&h.len()) || m.len() != 2 || !ascii_digits(h) || !ascii_digits(m) { return None; }
    let h = h.parse::<u64>().ok()?;
    let m = m.parse::<u64>().ok()?;
    if h > 23 || m > 59 { return None; }
    // Keep the written hour, including 24-hour input; don't invent AM/PM.
    Some(if m == 0 { format!("{} o'clock", cardinal(h)) }
        else if m < 10 { format!("{} oh {}", cardinal(h), cardinal(m)) }
        else { format!("{} {}", cardinal(h), cardinal(m)) })
}

fn version(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.split('.').collect();
    if !(2..=4).contains(&parts.len()) || parts.iter().any(|p| !ascii_digits(p) || p.len() > 4) { return None; }
    Some(parts.iter().map(|p| integer_words(p, false)).collect::<Vec<_>>().join(" point "))
}

fn year_cue(s: &str) -> bool {
    matches!(s.to_ascii_lowercase().as_str(), "in"|"since"|"by"|"from"|"until"|"till"|"to"|"through"|"between"|"of"|
        "around"|"circa"|"before"|"after"|"during"|"year"|"early"|"late"|"mid"|"january"|"february"|"march"|
        "april"|"may"|"june"|"july"|"august"|"september"|"october"|"november"|"december")
}

fn render_token(s: &str, previous: &str) -> Option<String> {
    match s {
        "C++" => return Some("C plus plus".into()),
        "C#" => return Some("C sharp".into()),
        _ => {}
    }
    if let Some(v) = s.strip_prefix("HTTP/") {
        if matches!(v, "1.0"|"1.1"|"2"|"3") { return Some(format!("HTTP version {}", number(v, false)?)); }
    }
    if let Some(v) = s.strip_prefix('v').or_else(|| s.strip_prefix('V')) {
        if let Some(v) = version(v) { return Some(format!("version {v}")); }
    }
    if previous.eq_ignore_ascii_case("version") {
        if let Some(v) = version(s) { return Some(v); }
    }
    if let Some((owner, member)) = s.split_once('.') {
        if owner.starts_with(|c: char| c.is_ascii_uppercase()) &&
            owner.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_') &&
            member.starts_with(|c: char| c.is_ascii_lowercase()) &&
            member.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_') {
            return Some(format!("{owner} dot {member}"));
        }
    }
    if let Some(n) = s.strip_suffix('%').or_else(|| s.strip_suffix('％')) {
        return number(n, false).map(|n| format!("{n} percent"));
    }
    if let Some(n) = currency(s) { return Some(n); }
    if let Some(n) = ordinal(s) { return Some(n); }
    if let Some(n) = clock(s) { return Some(n); }
    number(s, year_cue(previous))
}

fn token_core(token: &str) -> (&str, &str, &str) {
    let leading = token.trim_start_matches(['(', '[', '"', '\'', '“', '‘']);
    let core = leading.trim_end_matches(['.', ',', '!', '?', ';', ':', ')', ']', '"', '\'', '”', '’']);
    (&token[..token.len() - leading.len()], core, &leading[core.len()..])
}

/// Normalize only structurally recognized English speech tokens. Unknown
/// identifiers, dates, URLs and compound forms remain literal; no guessing.
/// Preserve original whitespace/punctuation and borrow unchanged text.
pub(super) fn normalize(text: &str) -> Result<Cow<'_, str>, Error> {
    if text.len() > MAX_TEXT_BYTES || text.trim().is_empty() || text.contains("<|") ||
        text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err(Error::InvalidInput);
    }
    // Do not allocate an output buffer for an ordinary, unchanged sentence.
    let mut output: Option<String> = None;
    let mut copied = 0;
    let mut previous = "";
    let mut start = 0;
    for (i, ch) in text.char_indices().chain(std::iter::once((text.len(), ' '))) {
        if !ch.is_whitespace() { continue; }
        if start < i {
            let token = &text[start..i];
            let (prefix, core, suffix) = token_core(token);
            if let Some(rendered) = render_token(core, previous) {
                let begin = start + prefix.len();
                let end = begin + core.len();
                let out = output.get_or_insert_with(|| String::with_capacity(text.len()));
                out.push_str(&text[copied..begin]);
                out.push_str(&rendered);
                copied = end;
                if out.len() > MAX_TEXT_BYTES { return Err(Error::InvalidInput); }
            }
            previous = if suffix.is_empty() { core } else { "" };
        }
        start = i + ch.len_utf8();
    }
    match output {
        None => Ok(Cow::Borrowed(text)),
        Some(mut output) => {
            if output.len() + text.len() - copied > MAX_TEXT_BYTES { return Err(Error::InvalidInput); }
            output.push_str(&text[copied..]);
            Ok(Cow::Owned(output))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_numeric_and_technical_cases() {
        for (input, expected) in [
            ("The limit is 250 MB.", "The limit is two hundred fifty MB."),
            ("It costs $3.50, 12.5% off.", "It costs three dollars and fifty cents, twelve point five percent off."),
            ("£0.01 and €1.00.", "one penny and one euro."),
            ("At 10:05, 19:30 and 8:00.", "At ten oh five, nineteen thirty and eight o'clock."),
            ("In 2026, 1,234 people joined on the 21st.", "In twenty twenty-six, one thousand two hundred thirty-four people joined on the twenty-first."),
            ("Please use version 1.21.4 on port 19132.", "Please use version one point twenty-one point four on port nineteen thousand one hundred thirty-two."),
            ("C++ and C# use API.fetch and HTTP/1.1.", "C plus plus and C sharp use API dot fetch and HTTP version one point one."),
            ("v1.02.3, 007, -5 and 0.05.", "version one point zero two point three, zero zero seven, minus five and zero point zero five."),
            ("(42), 12th and 103rd.", "(forty-two), twelfth and one hundred third."),
            ("Do not change https://example.com/v1.2.3 or 192.168.1.1.", "Do not change https://example.com/v1.2.3 or 192.168.1.1."),
            ("ID-001 2026-11-28 1/2 12:99 1,23", "ID-001 2026-11-28 1/2 12:99 1,23"),
            ("  café\t42\n", "  café\tforty-two\n"),
            ("42, then 7; keep this text.", "forty-two, then seven; keep this text."),
            ("The difference is -2026.", "The difference is minus two thousand twenty-six."),
        ] {
            assert_eq!(normalize(input).unwrap().as_ref(), expected, "{input}");
        }
    }

    #[test]
    fn normalization_is_bounded_and_does_not_rewrite_plain_text() {
        let text = "Please confirm the final schedule.";
        assert!(matches!(normalize(text), Ok(Cow::Borrowed(_))));
        for input in ["", " \t", "hello <|control|>", "a\0b"] {
            assert_eq!(normalize(input), Err(Error::InvalidInput));
        }
        assert_eq!(normalize(&"x".repeat(4001)), Err(Error::InvalidInput));
        assert_eq!(normalize(&"9 ".repeat(2000)), Err(Error::InvalidInput));
    }
}
|'£'|'€'|'¥')) {
        (c, &s[c.len_utf8()..])
    } else {
        let c = s.chars().last().filter(|c| matches!(*c, '?;
        (c, &s[..s.len() - c.len_utf8()])
    };
    let (unit, plural, minor, minors) = match cur {
        '$' => ("dollar", "dollars", "cent", "cents"),
        '£' => ("pound", "pounds", "penny", "pence"),
        '€' => ("euro", "euros", "cent", "cents"),
        '¥' => ("yen", "yen", "", ""),
        _ => return None,
    };
    let (whole, frac) = amount.split_once('.').map_or((amount, None), |(a,b)| (a, Some(b)));
    let whole = integer(whole)?;
    if whole.len() > 15 { return None; }
    let n = whole.parse::<u64>().ok()?;
    if let Some(frac) = frac {
        if !ascii_digits(frac) { return None; }
        if frac.len() <= 2 && !minor.is_empty() {
            let cents = frac.parse::<u64>().ok()? * if frac.len() == 1 { 10 } else { 1 };
            let major = format!("{} {}", cardinal(n), if n == 1 { unit } else { plural });
            let small = format!("{} {}", cardinal(cents), if cents == 1 { minor } else { minors });
            return Some(format!("{sign}{}", match (n, cents) {
                (_, 0) => major, (0, _) => small, _ => format!("{major} and {small}"),
            }));
        }
        return Some(format!("{sign}{} {plural}", number(amount, false)?));
    }
    Some(format!("{sign}{} {}", cardinal(n), if n == 1 { unit } else { plural }))
}

fn clock(s: &str) -> Option<String> {
    let (h, m) = s.split_once(':')?;
    if !(1..=2).contains(&h.len()) || m.len() != 2 || !ascii_digits(h) || !ascii_digits(m) { return None; }
    let h = h.parse::<u64>().ok()?;
    let m = m.parse::<u64>().ok()?;
    if h > 23 || m > 59 { return None; }
    // Keep the written hour, including 24-hour input; don't invent AM/PM.
    Some(if m == 0 { format!("{} o'clock", cardinal(h)) }
        else if m < 10 { format!("{} oh {}", cardinal(h), cardinal(m)) }
        else { format!("{} {}", cardinal(h), cardinal(m)) })
}

fn version(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.split('.').collect();
    if !(2..=4).contains(&parts.len()) || parts.iter().any(|p| !ascii_digits(p) || p.len() > 4) { return None; }
    Some(parts.iter().map(|p| integer_words(p, false)).collect::<Vec<_>>().join(" point "))
}

fn year_cue(s: &str) -> bool {
    matches!(s.to_ascii_lowercase().as_str(), "in"|"since"|"by"|"from"|"until"|"till"|"to"|"through"|"between"|"of"|
        "around"|"circa"|"before"|"after"|"during"|"year"|"early"|"late"|"mid"|"january"|"february"|"march"|
        "april"|"may"|"june"|"july"|"august"|"september"|"october"|"november"|"december")
}

fn render_token(s: &str, previous: &str) -> Option<String> {
    match s {
        "C++" => return Some("C plus plus".into()),
        "C#" => return Some("C sharp".into()),
        _ => {}
    }
    if let Some(v) = s.strip_prefix("HTTP/") {
        if matches!(v, "1.0"|"1.1"|"2"|"3") { return Some(format!("HTTP version {}", number(v, false)?)); }
    }
    if let Some(v) = s.strip_prefix('v').or_else(|| s.strip_prefix('V')) {
        if let Some(v) = version(v) { return Some(format!("version {v}")); }
    }
    if previous.eq_ignore_ascii_case("version") {
        if let Some(v) = version(s) { return Some(v); }
    }
    if let Some((owner, member)) = s.split_once('.') {
        if owner.starts_with(|c: char| c.is_ascii_uppercase()) &&
            owner.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_') &&
            member.starts_with(|c: char| c.is_ascii_lowercase()) &&
            member.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_') {
            return Some(format!("{owner} dot {member}"));
        }
    }
    if let Some(n) = s.strip_suffix('%').or_else(|| s.strip_suffix('％')) {
        return number(n, false).map(|n| format!("{n} percent"));
    }
    if let Some(n) = currency(s) { return Some(n); }
    if let Some(n) = ordinal(s) { return Some(n); }
    if let Some(n) = clock(s) { return Some(n); }
    number(s, year_cue(previous))
}

fn token_core(token: &str) -> (&str, &str, &str) {
    let leading = token.trim_start_matches(['(', '[', '"', '\'', '“', '‘']);
    let core = leading.trim_end_matches(['.', ',', '!', '?', ';', ':', ')', ']', '"', '\'', '”', '’']);
    (&token[..token.len() - leading.len()], core, &leading[core.len()..])
}

/// Normalize only structurally recognized English speech tokens. Unknown
/// identifiers, dates, URLs and compound forms remain literal; no guessing.
/// Preserve original whitespace/punctuation and borrow unchanged text.
pub(super) fn normalize(text: &str) -> Result<Cow<'_, str>, Error> {
    if text.len() > MAX_TEXT_BYTES || text.trim().is_empty() || text.contains("<|") ||
        text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err(Error::InvalidInput);
    }
    let mut output = String::with_capacity(text.len());
    let mut previous = "";
    let mut start = 0;
    for (i, ch) in text.char_indices().chain(std::iter::once((text.len(), ' '))) {
        if !ch.is_whitespace() { continue; }
        if start < i {
            let token = &text[start..i];
            let (prefix, core, suffix) = token_core(token);
            output.push_str(prefix);
            if let Some(rendered) = render_token(core, previous) { output.push_str(&rendered); }
            else { output.push_str(core); }
            output.push_str(suffix);
            previous = if suffix.is_empty() { core } else { "" };
        }
        if i < text.len() { output.push(ch); }
        start = i + ch.len_utf8();
        if output.len() > MAX_TEXT_BYTES { return Err(Error::InvalidInput); }
    }
    if output == text { Ok(Cow::Borrowed(text)) } else { Ok(Cow::Owned(output)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_numeric_and_technical_cases() {
        for (input, expected) in [
            ("The limit is 250 MB.", "The limit is two hundred fifty MB."),
            ("It costs $3.50, 12.5% off.", "It costs three dollars and fifty cents, twelve point five percent off."),
            ("£0.01 and €1.00.", "one penny and one euro."),
            ("At 10:05, 19:30 and 8:00.", "At ten oh five, nineteen thirty and eight o'clock."),
            ("In 2026, 1,234 people joined on the 21st.", "In twenty twenty-six, one thousand two hundred thirty-four people joined on the twenty-first."),
            ("Please use version 1.21.4 on port 19132.", "Please use version one point twenty-one point four on port nineteen thousand one hundred thirty-two."),
            ("C++ and C# use API.fetch and HTTP/1.1.", "C plus plus and C sharp use API dot fetch and HTTP version one point one."),
            ("v1.02.3, 007, -5 and 0.05.", "version one point zero two point three, zero zero seven, minus five and zero point zero five."),
            ("(42), 12th and 103rd.", "(forty-two), twelfth and one hundred third."),
            ("Do not change https://example.com/v1.2.3 or 192.168.1.1.", "Do not change https://example.com/v1.2.3 or 192.168.1.1."),
            ("ID-001 2026-11-28 1/2 12:99 1,23", "ID-001 2026-11-28 1/2 12:99 1,23"),
            ("  café\t42\n", "  café\tforty-two\n"),
        ] {
            assert_eq!(normalize(input).unwrap().as_ref(), expected, "{input}");
        }
    }

    #[test]
    fn normalization_is_bounded_and_does_not_rewrite_plain_text() {
        let text = "Please confirm the final schedule.";
        assert!(matches!(normalize(text), Ok(Cow::Borrowed(_))));
        for input in ["", " \t", "hello <|control|>", "a\0b"] {
            assert_eq!(normalize(input), Err(Error::InvalidInput));
        }
        assert_eq!(normalize(&"x".repeat(4001)), Err(Error::InvalidInput));
        assert_eq!(normalize(&"9 ".repeat(2000)), Err(Error::InvalidInput));
    }
}
|'£'|'€'|'¥'))?;
        (c, &s[..s.len() - c.len_utf8()])
    };
    let (unit, plural, minor, minors) = match cur {
        '$' => ("dollar", "dollars", "cent", "cents"),
        '£' => ("pound", "pounds", "penny", "pence"),
        '€' => ("euro", "euros", "cent", "cents"),
        '¥' => ("yen", "yen", "", ""),
        _ => return None,
    };
    let (whole, frac) = amount.split_once('.').map_or((amount, None), |(a,b)| (a, Some(b)));
    let whole = integer(whole)?;
    if whole.len() > 15 { return None; }
    let n = whole.parse::<u64>().ok()?;
    if let Some(frac) = frac {
        if !ascii_digits(frac) { return None; }
        if frac.len() <= 2 && !minor.is_empty() {
            let cents = frac.parse::<u64>().ok()? * if frac.len() == 1 { 10 } else { 1 };
            let major = format!("{} {}", cardinal(n), if n == 1 { unit } else { plural });
            let small = format!("{} {}", cardinal(cents), if cents == 1 { minor } else { minors });
            return Some(format!("{sign}{}", match (n, cents) {
                (_, 0) => major, (0, _) => small, _ => format!("{major} and {small}"),
            }));
        }
        return Some(format!("{sign}{} {plural}", number(amount, false)?));
    }
    Some(format!("{sign}{} {}", cardinal(n), if n == 1 { unit } else { plural }))
}

fn clock(s: &str) -> Option<String> {
    let (h, m) = s.split_once(':')?;
    if !(1..=2).contains(&h.len()) || m.len() != 2 || !ascii_digits(h) || !ascii_digits(m) { return None; }
    let h = h.parse::<u64>().ok()?;
    let m = m.parse::<u64>().ok()?;
    if h > 23 || m > 59 { return None; }
    // Keep the written hour, including 24-hour input; don't invent AM/PM.
    Some(if m == 0 { format!("{} o'clock", cardinal(h)) }
        else if m < 10 { format!("{} oh {}", cardinal(h), cardinal(m)) }
        else { format!("{} {}", cardinal(h), cardinal(m)) })
}

fn version(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.split('.').collect();
    if !(2..=4).contains(&parts.len()) || parts.iter().any(|p| !ascii_digits(p) || p.len() > 4) { return None; }
    Some(parts.iter().map(|p| integer_words(p, false)).collect::<Vec<_>>().join(" point "))
}

fn year_cue(s: &str) -> bool {
    matches!(s.to_ascii_lowercase().as_str(), "in"|"since"|"by"|"from"|"until"|"till"|"to"|"through"|"between"|"of"|
        "around"|"circa"|"before"|"after"|"during"|"year"|"early"|"late"|"mid"|"january"|"february"|"march"|
        "april"|"may"|"june"|"july"|"august"|"september"|"october"|"november"|"december")
}

fn render_token(s: &str, previous: &str) -> Option<String> {
    match s {
        "C++" => return Some("C plus plus".into()),
        "C#" => return Some("C sharp".into()),
        _ => {}
    }
    if let Some(v) = s.strip_prefix("HTTP/") {
        if matches!(v, "1.0"|"1.1"|"2"|"3") { return Some(format!("HTTP version {}", number(v, false)?)); }
    }
    if let Some(v) = s.strip_prefix('v').or_else(|| s.strip_prefix('V')) {
        if let Some(v) = version(v) { return Some(format!("version {v}")); }
    }
    if previous.eq_ignore_ascii_case("version") {
        if let Some(v) = version(s) { return Some(v); }
    }
    if let Some((owner, member)) = s.split_once('.') {
        if owner.starts_with(|c: char| c.is_ascii_uppercase()) &&
            owner.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_') &&
            member.starts_with(|c: char| c.is_ascii_lowercase()) &&
            member.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_') {
            return Some(format!("{owner} dot {member}"));
        }
    }
    if let Some(n) = s.strip_suffix('%').or_else(|| s.strip_suffix('％')) {
        return number(n, false).map(|n| format!("{n} percent"));
    }
    if let Some(n) = currency(s) { return Some(n); }
    if let Some(n) = ordinal(s) { return Some(n); }
    if let Some(n) = clock(s) { return Some(n); }
    number(s, year_cue(previous))
}

fn token_core(token: &str) -> (&str, &str, &str) {
    let leading = token.trim_start_matches(['(', '[', '"', '\'', '“', '‘']);
    let core = leading.trim_end_matches(['.', ',', '!', '?', ';', ':', ')', ']', '"', '\'', '”', '’']);
    (&token[..token.len() - leading.len()], core, &leading[core.len()..])
}

/// Normalize only structurally recognized English speech tokens. Unknown
/// identifiers, dates, URLs and compound forms remain literal; no guessing.
/// Preserve original whitespace/punctuation and borrow unchanged text.
pub(super) fn normalize(text: &str) -> Result<Cow<'_, str>, Error> {
    if text.len() > MAX_TEXT_BYTES || text.trim().is_empty() || text.contains("<|") ||
        text.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err(Error::InvalidInput);
    }
    let mut output = String::with_capacity(text.len());
    let mut previous = "";
    let mut start = 0;
    for (i, ch) in text.char_indices().chain(std::iter::once((text.len(), ' '))) {
        if !ch.is_whitespace() { continue; }
        if start < i {
            let token = &text[start..i];
            let (prefix, core, suffix) = token_core(token);
            output.push_str(prefix);
            if let Some(rendered) = render_token(core, previous) { output.push_str(&rendered); }
            else { output.push_str(core); }
            output.push_str(suffix);
            previous = if suffix.is_empty() { core } else { "" };
        }
        if i < text.len() { output.push(ch); }
        start = i + ch.len_utf8();
        if output.len() > MAX_TEXT_BYTES { return Err(Error::InvalidInput); }
    }
    if output == text { Ok(Cow::Borrowed(text)) } else { Ok(Cow::Owned(output)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_numeric_and_technical_cases() {
        for (input, expected) in [
            ("The limit is 250 MB.", "The limit is two hundred fifty MB."),
            ("It costs $3.50, 12.5% off.", "It costs three dollars and fifty cents, twelve point five percent off."),
            ("£0.01 and €1.00.", "one penny and one euro."),
            ("At 10:05, 19:30 and 8:00.", "At ten oh five, nineteen thirty and eight o'clock."),
            ("In 2026, 1,234 people joined on the 21st.", "In twenty twenty-six, one thousand two hundred thirty-four people joined on the twenty-first."),
            ("Please use version 1.21.4 on port 19132.", "Please use version one point twenty-one point four on port nineteen thousand one hundred thirty-two."),
            ("C++ and C# use API.fetch and HTTP/1.1.", "C plus plus and C sharp use API dot fetch and HTTP version one point one."),
            ("v1.02.3, 007, -5 and 0.05.", "version one point zero two point three, zero zero seven, minus five and zero point zero five."),
            ("(42), 12th and 103rd.", "(forty-two), twelfth and one hundred third."),
            ("Do not change https://example.com/v1.2.3 or 192.168.1.1.", "Do not change https://example.com/v1.2.3 or 192.168.1.1."),
            ("ID-001 2026-11-28 1/2 12:99 1,23", "ID-001 2026-11-28 1/2 12:99 1,23"),
            ("  café\t42\n", "  café\tforty-two\n"),
        ] {
            assert_eq!(normalize(input).unwrap().as_ref(), expected, "{input}");
        }
    }

    #[test]
    fn normalization_is_bounded_and_does_not_rewrite_plain_text() {
        let text = "Please confirm the final schedule.";
        assert!(matches!(normalize(text), Ok(Cow::Borrowed(_))));
        for input in ["", " \t", "hello <|control|>", "a\0b"] {
            assert_eq!(normalize(input), Err(Error::InvalidInput));
        }
        assert_eq!(normalize(&"x".repeat(4001)), Err(Error::InvalidInput));
        assert_eq!(normalize(&"9 ".repeat(2000)), Err(Error::InvalidInput));
    }
}
