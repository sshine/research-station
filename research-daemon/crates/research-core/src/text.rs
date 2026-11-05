/// Turn a question into a stable, filesystem-safe slug.
///
/// Lowercase ASCII letters and digits survive; every other run of characters
/// becomes a single `-`. The result is trimmed to `max_len` characters without
/// ending in a separator, and falls back to `research` when nothing remains.
pub fn slugify(input: &str, max_len: usize) -> String {
    let mut slug = String::with_capacity(input.len().min(max_len));
    let mut pending_dash = false;

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            let separator = pending_dash && !slug.is_empty();
            let needed = slug.len() + usize::from(separator) + 1;
            if needed > max_len {
                break;
            }
            if separator {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(ch.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "research".to_owned()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_questions() {
        assert_eq!(
            slugify("Tavily vs. Kagi: which is better?", 48),
            "tavily-vs-kagi-which-is-better"
        );
        assert_eq!(slugify("  Ångström  measures  ", 48), "ngstr-m-measures");
        assert_eq!(slugify("!!!", 10), "research");
    }

    #[test]
    fn respects_the_length_limit() {
        let slug = slugify("a very long research question that keeps going", 12);
        assert!(slug.len() <= 12);
        assert!(!slug.ends_with('-'));
    }
}
