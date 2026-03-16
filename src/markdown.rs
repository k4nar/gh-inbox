use comrak::{Options, markdown_to_html};

#[allow(dead_code)]
pub fn render_markdown(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.render.r#unsafe = true;

    let html = markdown_to_html(input, &options);

    ammonia::Builder::new()
        .add_tags(&["details", "summary", "kbd", "input"])
        .add_tag_attributes("input", &["type", "checked", "disabled"])
        .clean(&html)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_returns_empty() {
        assert_eq!(render_markdown(""), "");
    }

    #[test]
    fn script_tag_is_stripped() {
        let output = render_markdown("<script>alert(1)</script>xss");
        assert!(!output.contains("<script>"), "output: {output}");
        assert!(
            output.contains("xss"),
            "text content should survive: {output}"
        );
    }

    #[test]
    fn details_summary_pass_through() {
        let input = "<details><summary>click me</summary>hidden content</details>";
        let output = render_markdown(input);
        assert!(output.contains("<details>"), "output: {output}");
        assert!(output.contains("<summary>"), "output: {output}");
        assert!(output.contains("hidden content"), "output: {output}");
    }

    #[test]
    fn kbd_passes_through() {
        let output = render_markdown("Press <kbd>Enter</kbd> to continue.");
        assert!(output.contains("<kbd>"), "output: {output}");
    }

    #[test]
    fn gfm_strikethrough() {
        let output = render_markdown("~~deleted~~");
        assert!(output.contains("<del>"), "output: {output}");
    }

    #[test]
    fn gfm_table() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let output = render_markdown(input);
        assert!(output.contains("<table>"), "output: {output}");
        assert!(output.contains("<th>"), "output: {output}");
    }

    #[test]
    fn gfm_task_list() {
        let input = "- [x] Done\n- [ ] Todo";
        let output = render_markdown(input);
        assert!(output.contains(r#"type="checkbox""#), "output: {output}");
        assert!(output.contains("checked"), "output: {output}");
    }

    #[test]
    fn fenced_code_block() {
        let input = "```rust\nfn main() {}\n```";
        let output = render_markdown(input);
        assert!(output.contains("<pre>"), "output: {output}");
        assert!(output.contains("<code>"), "output: {output}");
    }

    #[test]
    fn paragraph_wraps_text() {
        let output = render_markdown("Hello world");
        assert!(output.contains("<p>"), "output: {output}");
        assert!(output.contains("Hello world"), "output: {output}");
    }
}
