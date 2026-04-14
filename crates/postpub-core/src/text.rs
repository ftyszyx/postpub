use encoding_rs::{Encoding, WINDOWS_1252};

const REPAIR_SCORE_MARGIN: i32 = 6;

pub(crate) fn repair_mojibake(value: &str) -> Option<String> {
    if value.trim().is_empty() {
        return None;
    }

    let mut current = value.to_string();
    let mut changed = false;

    for _ in 0..2 {
        let original_score = text_quality_score(&current);
        let mut best: Option<(String, i32)> = None;

        for candidate in [
            decode_utf8_from_latin1_text(&current),
            decode_utf8_from_reencoded_text(&current, WINDOWS_1252),
        ]
        .into_iter()
        .flatten()
        {
            if candidate == current {
                continue;
            }

            let score = text_quality_score(&candidate);
            if score < original_score + REPAIR_SCORE_MARGIN {
                continue;
            }

            match &best {
                Some((_, best_score)) if score <= *best_score => {}
                _ => best = Some((candidate, score)),
            }
        }

        let Some((candidate, _)) = best else {
            break;
        };

        current = candidate;
        changed = true;
    }

    changed.then_some(current)
}

fn decode_utf8_from_reencoded_text(value: &str, encoding: &'static Encoding) -> Option<String> {
    let (bytes, _, had_errors) = encoding.encode(value);
    if had_errors {
        return None;
    }

    std::str::from_utf8(bytes.as_ref())
        .ok()
        .map(ToOwned::to_owned)
}

fn decode_utf8_from_latin1_text(value: &str) -> Option<String> {
    let bytes: Option<Vec<u8>> = value
        .chars()
        .map(|ch| u8::try_from(u32::from(ch)).ok())
        .collect();

    std::str::from_utf8(bytes?.as_slice())
        .ok()
        .map(ToOwned::to_owned)
}

fn text_quality_score(value: &str) -> i32 {
    value.chars().fold(0, |score, ch| {
        score
            + match ch {
                '\u{E000}'..='\u{F8FF}' => -8,
                '\u{FFFD}' => -8,
                c if c.is_control() && !matches!(c, '\n' | '\r' | '\t') => -8,
                '\u{4E00}'..='\u{9FFF}' => 4,
                'a'..='z' | 'A'..='Z' | '0'..='9' => 1,
                ' ' | '-' | '_' | '.' | ',' | ':' | ';' | '/' | '\'' | '"' | '(' | ')' => 1,
                'Ã' | 'Â' | 'Å' | 'Ä' | 'Æ' | 'Ç' | 'È' | 'É' | 'Î' | 'Ï' | 'Ð' | 'Ò' | 'Ó'
                | 'Ô' | 'Ö' | 'Ù' | 'Û' | 'Ü' | 'å' | 'ä' | 'æ' | 'ç' | 'è' | 'é' | 'ê' | 'î'
                | 'ï' | 'ð' | 'ò' | 'ó' | 'ô' | 'ö' | 'ù' | 'û' | 'ü' => -3,
                _ => 0,
            }
    })
}

#[cfg(test)]
mod tests {
    use super::repair_mojibake;

    #[test]
    fn repairs_latin1_mojibake_text() {
        let original = "微信公众号 1";
        let corrupted: String = original
            .as_bytes()
            .iter()
            .map(|byte| char::from(*byte))
            .collect();

        assert_eq!(repair_mojibake(&corrupted).as_deref(), Some(original));
    }

    #[test]
    fn repairs_latin1_mojibake_provider_name() {
        let original = "阿里万相";
        let corrupted: String = original
            .as_bytes()
            .iter()
            .map(|byte| char::from(*byte))
            .collect();

        assert_eq!(repair_mojibake(&corrupted).as_deref(), Some(original));
    }

    #[test]
    fn keeps_valid_utf8_text_unchanged() {
        assert_eq!(repair_mojibake("阿里万相"), None);
        assert_eq!(repair_mojibake("Picsum"), None);
    }
}
