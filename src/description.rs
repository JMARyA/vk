use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

// ── Task-item helpers (used by the check TUI) ────────────────────────────────

fn strip_html_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Extract `(checked, plain_text)` for every task-list item in TipTap HTML.
pub fn parse_task_items(html: &str) -> Vec<(bool, String)> {
    let mut items = Vec::new();
    let mut rest = html;
    let needle = r#"<li data-checked=""#;

    while let Some(pos) = rest.find(needle) {
        rest = &rest[pos..];
        let checked = rest.starts_with(r#"<li data-checked="true""#);

        let text = rest
            .find("<div><p>")
            .map(|s| {
                let cs = s + "<div><p>".len();
                rest[cs..]
                    .find("</p></div>")
                    .map(|e| strip_html_tags(&rest[cs..cs + e]))
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        items.push((checked, text));
        rest = &rest[needle.len()..]; // advance past this item's opening
    }
    items
}

/// Rewrite every `data-checked` value in TipTap HTML according to `states`
/// (one bool per task-list item, in order).
pub fn apply_task_item_states(html: &str, states: &[bool]) -> String {
    let prefix = r#"<li data-checked=""#;
    let mut result = html.to_string();
    let mut idx = 0;
    let mut from = 0;

    while let Some(rel) = result[from..].find(prefix) {
        if idx >= states.len() {
            break;
        }
        let val_start = from + rel + prefix.len();
        if let Some(val_len) = result[val_start..].find('"') {
            let val_end = val_start + val_len;
            let new_val = if states[idx] { "true" } else { "false" };
            result.replace_range(val_start..val_end, new_val);
            from = val_start + new_val.len();
        } else {
            break;
        }
        idx += 1;
    }
    result
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn heading_num(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Extract the value of an HTML attribute from a tag string.
fn extract_attr<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let needle = format!(r#"{}=""#, attr);
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

/// Convert TipTap attachment images to standard `<img src=ALT alt=ID>` so
/// htmd produces `![ID](URL)` which roundtrips cleanly back to TipTap format.
fn convert_tiptap_images(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("<img") {
        result.push_str(&rest[..start]);
        if let Some(rel_end) = rest[start..].find('>') {
            let tag = &rest[start..start + rel_end + 1];
            let data_src = extract_attr(tag, "data-src");
            let id = extract_attr(tag, "id");
            match (data_src, id) {
                (Some(url), Some(img_id)) if img_id.starts_with("tiptap-image-") => {
                    // Render as standard img so htmd → ![ID](url)
                    result.push_str(&format!(r#"<img src="{url}" alt="{img_id}">"#));
                }
                _ => {
                    // Keep unknown img tags as-is
                    result.push_str(tag);
                }
            }
            rest = &rest[start + rel_end + 1..];
        } else {
            rest = "";
            break;
        }
    }
    result.push_str(rest);
    result
}

/// Preprocess TipTap-specific HTML into standard HTML that htmd understands.
fn preprocess_tiptap(html: &str) -> String {
    let mut s = convert_tiptap_images(html);

    // Task list container
    s = s.replace(r#"<ul data-type="taskList">"#, "<ul>");

    // Unchecked task items — include the opening <p> in the replacement so htmd
    // sees `<li><p>[ ] text</p></li>` on a single list-item paragraph.
    s = s.replace(
        r#"<li data-checked="false" data-type="taskItem"><label><input type="checkbox"><span></span></label><div><p>"#,
        "<li><p>[ ] ",
    );
    // Checked task items
    s = s.replace(
        r#"<li data-checked="true" data-type="taskItem"><label><input type="checkbox"><span></span></label><div><p>"#,
        "<li><p>[x] ",
    );
    // Variant with checked attribute on the input element
    s = s.replace(
        r#"<li data-checked="true" data-type="taskItem"><label><input type="checkbox" checked><span></span></label><div><p>"#,
        "<li><p>[x] ",
    );

    // Close the injected <p> and the div/li wrapper
    s = s.replace("</p></div></li>", "</p></li>");
    // Fallback for multi-paragraph task items
    s = s.replace("</div></li>", "</li>");

    s
}

/// htmd escapes `[ ]` to `\[ \]` and `[x]` to `\[x\]` inside list items.
/// Undo that so pulldown-cmark recognises them as GFM task-list markers.
fn fix_task_list_markers(md: &str) -> String {
    md.lines()
        .map(|line| {
            let stripped = line.trim_start();
            let is_list_item = stripped.starts_with("* ")
                || stripped.starts_with("- ")
                || stripped.starts_with("+ ");
            if is_list_item
                && (line.contains(r"\[ \]")
                    || line.contains(r"\[x\]")
                    || line.contains(r"\[X\]"))
            {
                line.replace(r"\[ \]", "[ ]")
                    .replace(r"\[x\]", "[x]")
                    .replace(r"\[X\]", "[x]")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert Vikunja TipTap HTML to Markdown for editing.
pub fn html_to_markdown(html: &str) -> String {
    if html.is_empty() || html == "<p></p>" {
        return String::new();
    }
    let preprocessed = preprocess_tiptap(html);
    let md = htmd::convert(&preprocessed).unwrap_or(preprocessed);
    fix_task_list_markers(&md)
}

// ── Markdown → TipTap HTML ────────────────────────────────────────────────────

/// Convert Markdown to Vikunja TipTap HTML for the API.
pub fn markdown_to_html(md: &str) -> String {
    if md.trim().is_empty() {
        return "<p></p>".to_string();
    }

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_TABLES);

    let events: Vec<Event> = Parser::new_ext(md, options).collect();
    render_tiptap(&events)
}

fn render_tiptap(events: &[Event]) -> String {
    let mut out = String::new();

    // List tracking: (byte-pos of list open tag, is_task_list_upgraded, is_ordered)
    let mut list_stack: Vec<(usize, bool, bool)> = vec![];
    let mut item_start_pos: usize = 0;
    let mut in_task_item = false;

    // Paragraph tracking inside list items for tight lists
    let mut in_list_item = false;
    let mut item_needs_paragraph = false; // true until first content inside tight item
    let mut item_has_tight_para = false;  // we opened a <p> for tight content
    let mut in_paragraph = false;

    let mut in_table_head = false;

    // Image accumulation (alt text comes as Text events between Start/End Image)
    let mut in_image = false;
    let mut image_url = String::new();
    let mut image_alt = String::new();

    // Emit `<p>` if we're in a tight list item and haven't opened one yet.
    macro_rules! maybe_open_tight_para {
        () => {
            if in_list_item && item_needs_paragraph && !in_paragraph {
                out.push_str("<p>");
                item_needs_paragraph = false;
                item_has_tight_para = true;
            }
        };
    }

    for event in events {
        match event {
            Event::Start(Tag::Paragraph) => {
                item_needs_paragraph = false;
                in_paragraph = true;
                out.push_str("<p>");
            }
            Event::End(TagEnd::Paragraph) => {
                in_paragraph = false;
                out.push_str("</p>");
            }

            Event::Start(Tag::Heading { level, .. }) => {
                let n = heading_num(*level);
                out.push_str(&format!("<h{n}>"));
            }
            Event::End(TagEnd::Heading(level)) => {
                let n = heading_num(*level);
                out.push_str(&format!("</h{n}>"));
            }

            Event::Start(Tag::Strong) => {
                maybe_open_tight_para!();
                out.push_str("<strong>");
            }
            Event::End(TagEnd::Strong) => out.push_str("</strong>"),

            Event::Start(Tag::Emphasis) => {
                maybe_open_tight_para!();
                out.push_str("<em>");
            }
            Event::End(TagEnd::Emphasis) => out.push_str("</em>"),

            Event::Start(Tag::Strikethrough) => {
                maybe_open_tight_para!();
                out.push_str("<s>");
            }
            Event::End(TagEnd::Strikethrough) => out.push_str("</s>"),

            Event::Code(text) => {
                maybe_open_tight_para!();
                out.push_str(&format!("<code>{}</code>", escape_html(text)));
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => {
                        out.push_str(&format!(
                            r#"<pre><code class="language-{}">"#,
                            escape_html(lang)
                        ));
                    }
                    _ => out.push_str("<pre><code>"),
                }
            }
            Event::End(TagEnd::CodeBlock) => out.push_str("</code></pre>"),

            Event::Start(Tag::List(ordered)) => {
                let pos = out.len();
                let is_ordered = ordered.is_some();
                if is_ordered {
                    out.push_str("<ol>");
                } else {
                    out.push_str("<ul>");
                }
                list_stack.push((pos, false, is_ordered));
            }
            Event::End(TagEnd::List(ordered)) => {
                list_stack.pop();
                if *ordered {
                    out.push_str("</ol>");
                } else {
                    out.push_str("</ul>");
                }
            }

            Event::Start(Tag::Item) => {
                item_start_pos = out.len();
                in_task_item = false;
                in_list_item = true;
                item_needs_paragraph = true;
                item_has_tight_para = false;
                out.push_str("<li>");
            }

            Event::TaskListMarker(checked) => {
                // Retroactively upgrade the parent <ul> to a TipTap task list.
                if let Some(last) = list_stack.last_mut() {
                    if !last.1 {
                        last.1 = true;
                        let ul_pos = last.0;
                        let from = "<ul>";
                        let to = r#"<ul data-type="taskList">"#;
                        let delta = to.len() - from.len();
                        out.replace_range(ul_pos..ul_pos + from.len(), to);
                        item_start_pos += delta;
                    }
                }

                // Retroactively replace <li> with the TipTap task item format.
                // The content div replaces the bare <li>; <p> comes later from
                // maybe_open_tight_para! when the first text arrives.
                let checked_str = if *checked { "true" } else { "false" };
                let task_open = format!(
                    r#"<li data-checked="{checked_str}" data-type="taskItem"><label><input type="checkbox"><span></span></label><div>"#
                );
                out.truncate(item_start_pos);
                out.push_str(&task_open);
                in_task_item = true;
                // For loose lists, Start(Paragraph) fires *before* TaskListMarker,
                // so the <p> was truncated away — re-emit it.
                if in_paragraph {
                    out.push_str("<p>");
                    item_needs_paragraph = false;
                }
                // For tight lists, item_needs_paragraph stays true — the <p>
                // will be injected by maybe_open_tight_para! when text arrives.
            }

            Event::End(TagEnd::Item) => {
                if item_has_tight_para {
                    out.push_str("</p>");
                    item_has_tight_para = false;
                }
                item_needs_paragraph = false;
                in_list_item = false;
                if in_task_item {
                    out.push_str("</div></li>");
                    in_task_item = false;
                } else {
                    out.push_str("</li>");
                }
            }

            Event::Start(Tag::BlockQuote(_)) => out.push_str("<blockquote>"),
            Event::End(TagEnd::BlockQuote(_)) => out.push_str("</blockquote>"),

            Event::Rule => out.push_str("<hr>"),

            Event::Start(Tag::Table(alignments)) => {
                let cols = alignments.len();
                out.push_str(r#"<table style="min-width: 50px"><colgroup>"#);
                for _ in 0..cols {
                    out.push_str("<col>");
                }
                out.push_str("</colgroup>");
            }
            Event::End(TagEnd::Table) => out.push_str("</tbody></table>"),

            Event::Start(Tag::TableHead) => {
                out.push_str("<tbody><tr>");
                in_table_head = true;
            }
            Event::End(TagEnd::TableHead) => {
                out.push_str("</tr>");
                in_table_head = false;
            }

            Event::Start(Tag::TableRow) => out.push_str("<tr>"),
            Event::End(TagEnd::TableRow) => out.push_str("</tr>"),

            Event::Start(Tag::TableCell) => {
                if in_table_head {
                    out.push_str(r#"<th colspan="1" rowspan="1"><p>"#);
                } else {
                    out.push_str(
                        r#"<td colspan="1" rowspan="1" style="background-color: null"><p>"#,
                    );
                }
            }
            Event::End(TagEnd::TableCell) => {
                if in_table_head {
                    out.push_str("</p></th>");
                } else {
                    out.push_str("</p></td>");
                }
            }

            Event::Start(Tag::Link { dest_url, .. }) => {
                maybe_open_tight_para!();
                let url = escape_html(dest_url);
                out.push_str(&format!(
                    r#"<a target="_blank" rel="noopener noreferrer nofollow" href="{url}">"#
                ));
            }
            Event::End(TagEnd::Link) => out.push_str("</a>"),

            Event::Start(Tag::Image { dest_url, .. }) => {
                in_image = true;
                image_url = dest_url.to_string();
                image_alt.clear();
            }
            Event::End(TagEnd::Image) => {
                in_image = false;
                if image_alt.starts_with("tiptap-image-") {
                    // Restore TipTap attachment image format
                    let url = escape_html(&image_url);
                    out.push_str(&format!(
                        "<img data-src=\"{url}\" src=\"#\" id=\"{image_alt}\">"
                    ));
                } else if !image_url.is_empty() {
                    let url = escape_html(&image_url);
                    let alt = escape_html(&image_alt);
                    out.push_str(&format!(r#"<img src="{url}" alt="{alt}">"#));
                }
                image_url.clear();
                image_alt.clear();
            }

            Event::Text(text) => {
                if in_image {
                    image_alt.push_str(text);
                } else {
                    maybe_open_tight_para!();
                    out.push_str(&escape_html(text));
                }
            }
            Event::SoftBreak => {}
            Event::HardBreak => out.push_str("<br>"),
            Event::Html(html) | Event::InlineHtml(html) => out.push_str(html),

            _ => {}
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headings_and_paragraph() {
        let html = "<h1>h1 text</h1><p>simple text</p><h2>h2 text</h2>";
        let md = html_to_markdown(html);
        assert!(md.contains("# h1 text"), "got: {md}");
        assert!(md.contains("## h2 text"), "got: {md}");
        assert!(md.contains("simple text"), "got: {md}");
    }

    #[test]
    fn test_html_to_md_task_list() {
        let html = r#"<ul data-type="taskList"><li data-checked="false" data-type="taskItem"><label><input type="checkbox"><span></span></label><div><p>task 1</p></div></li><li data-checked="true" data-type="taskItem"><label><input type="checkbox"><span></span></label><div><p>task 2</p></div></li></ul>"#;
        let md = html_to_markdown(html);
        println!("task list md:\n{md}");
        assert!(md.contains("[ ]"), "unchecked marker missing, got: {md}");
        assert!(md.contains("[x]") || md.contains("[X]"), "checked marker missing, got: {md}");
    }

    #[test]
    fn test_md_to_html_task_list() {
        let md = "- [ ] task 1\n- [x] task 2\n";
        let html = markdown_to_html(md);
        println!("task list html:\n{html}");
        assert!(html.contains(r#"data-type="taskList""#), "got: {html}");
        assert!(html.contains(r#"data-checked="false""#), "got: {html}");
        assert!(html.contains(r#"data-checked="true""#), "got: {html}");
        assert!(html.contains("<p>task 1</p>"), "got: {html}");
        assert!(html.contains("<p>task 2</p>"), "got: {html}");
    }

    #[test]
    fn test_md_to_html_formatting() {
        let html = markdown_to_html("**bold** *italic* ~~strike~~ `code`\n");
        assert!(html.contains("<strong>bold</strong>"), "got: {html}");
        assert!(html.contains("<em>italic</em>"), "got: {html}");
        assert!(html.contains("<s>strike</s>"), "got: {html}");
        assert!(html.contains("<code>code</code>"), "got: {html}");
    }

    #[test]
    fn test_md_to_html_link() {
        let html = markdown_to_html("[link](https://example.com)\n");
        assert!(html.contains(r#"href="https://example.com""#), "got: {html}");
        assert!(html.contains(r#"target="_blank""#), "got: {html}");
    }

    #[test]
    fn test_md_to_html_tight_list() {
        let md = "- item1\n- item2\n";
        let html = markdown_to_html(md);
        println!("tight list html:\n{html}");
        assert!(html.contains("<ul>"), "got: {html}");
        assert!(html.contains("<li><p>item1</p></li>"), "got: {html}");
    }

    #[test]
    fn test_round_trip_task_list() {
        let original = r#"<ul data-type="taskList"><li data-checked="false" data-type="taskItem"><label><input type="checkbox"><span></span></label><div><p>task 1</p></div></li><li data-checked="false" data-type="taskItem"><label><input type="checkbox"><span></span></label><div><p>task 2</p></div></li></ul>"#;
        let md = html_to_markdown(original);
        println!("round-trip md:\n{md}");
        let back = markdown_to_html(&md);
        println!("round-trip html:\n{back}");
        assert!(back.contains(r#"data-type="taskList""#), "got: {back}");
        assert!(back.contains("<p>task 1</p>"), "got: {back}");
        assert!(back.contains("<p>task 2</p>"), "got: {back}");
    }
}
