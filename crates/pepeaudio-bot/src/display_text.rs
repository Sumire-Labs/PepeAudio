/// Escapes Discord Markdown syntax that can change external display text into
/// formatting, masked links, or automatic links.
#[must_use]
pub(crate) fn escape_discord_markdown(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            if matches!(
                character,
                '\\' | '*' | '_' | '`' | '~' | '|' | '>' | '[' | ']' | '(' | ')' | '<'
            ) {
                vec!['\\', character]
            } else {
                vec![character]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::escape_discord_markdown;

    #[test]
    fn escapes_masked_and_automatic_link_delimiters() {
        assert_eq!(
            escape_discord_markdown(
                "[公式](https://evil.example) <https://evil.example> **title**"
            ),
            "\\[公式\\]\\(https://evil.example\\) \\<https://evil.example\\> \\*\\*title\\*\\*"
        );
    }
}
