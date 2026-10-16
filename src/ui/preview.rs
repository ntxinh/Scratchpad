use adw::prelude::*;
use gtk4::glib::translate::IntoGlib;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

fn ensure_tags(buf: &gtk4::TextBuffer) {
    let table = buf.tag_table();
    let add = |name: &str, f: fn(&gtk4::TextTag)| {
        if table.lookup(name).is_none() {
            let tag = gtk4::TextTag::new(Some(name));
            f(&tag);
            table.add(&tag);
        }
    };
    add("h1", |t| {
        t.set_scale(1.6);
        t.set_weight(gtk4::pango::Weight::Bold.into_glib());
    });
    add("h2", |t| {
        t.set_scale(1.3);
        t.set_weight(gtk4::pango::Weight::Bold.into_glib());
    });
    add("code", |t| t.set_family(Some("monospace")));
    add("bold", |t| {
        t.set_weight(gtk4::pango::Weight::Bold.into_glib())
    });
    add("italic", |t| t.set_style(gtk4::pango::Style::Italic));
}

fn ensure_trailing_newline(buf: &gtk4::TextBuffer) {
    let mut end = buf.end_iter();
    if end.offset() == 0 {
        return;
    }
    if end.backward_char() && end.char() != '\n' {
        buf.insert(&mut buf.end_iter(), "\n");
    }
}

pub fn render_markdown(md: &str, buf: &gtk4::TextBuffer) {
    ensure_tags(buf);
    buf.set_text("");
    // Tags currently open around the insertion point; End pops one.
    let mut active: Vec<String> = Vec::new();
    for event in Parser::new_ext(md, Options::all()) {
        match event {
            Event::Start(tag) => {
                let name = match tag {
                    Tag::Heading { level, .. } => Some(match level {
                        HeadingLevel::H1 => "h1",
                        _ => "h2",
                    }),
                    Tag::CodeBlock(_) => Some("code"),
                    Tag::Strong => Some("bold"),
                    Tag::Emphasis => Some("italic"),
                    _ => None,
                };
                if let Some(n) = name {
                    active.push(n.to_string());
                }
            }
            Event::End(end) => {
                match end {
                    TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Strong | TagEnd::Emphasis => {
                        active.pop();
                    }
                    _ => {}
                }
                // Block ends get a newline so paragraphs, headings and
                // lists don't collapse onto one line in the preview.
                if matches!(
                    end,
                    TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item
                ) {
                    ensure_trailing_newline(buf);
                }
            }
            Event::Text(t) | Event::InlineMath(t) | Event::DisplayMath(t) => {
                let names: Vec<&str> = active.iter().map(String::as_str).collect();
                buf.insert_with_tags_by_name(&mut buf.end_iter(), &t, &names);
            }
            Event::Code(t) => {
                let mut names: Vec<&str> = active.iter().map(String::as_str).collect();
                names.push("code");
                buf.insert_with_tags_by_name(&mut buf.end_iter(), &t, &names);
            }
            Event::SoftBreak | Event::HardBreak => {
                buf.insert(&mut buf.end_iter(), "\n");
            }
            Event::Rule => {
                buf.insert(&mut buf.end_iter(), "\u{2500}\u{2500}\u{2500}\n");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offset_of(text: &str, needle: &str) -> i32 {
        let byte = text.find(needle).expect("needle in rendered text") as i32;
        text[..byte as usize].chars().count() as i32
    }

    #[test]
    fn renders_heading_bold_and_code() {
        if gtk4::init().is_err() {
            return; // headless/CI: no display, nothing to assert
        }
        let buf = gtk4::TextBuffer::new(None::<&gtk4::TextTagTable>);
        render_markdown(
            "# Hi\n\nsome **bold** text\n\n```\nfn main() {}\n```\n",
            &buf,
        );
        let text = buf
            .text(&buf.start_iter(), &buf.end_iter(), false)
            .to_string();
        assert!(text.contains("Hi"));
        assert!(text.contains("bold"));
        assert!(text.contains("fn main() {}"));
        for (needle, tag) in [("Hi", "h1"), ("bold", "bold"), ("fn main", "code")] {
            let tag = buf.tag_table().lookup(tag).unwrap();
            let at = offset_of(&text, needle);
            assert!(buf.iter_at_offset(at).has_tag(&tag), "{needle} has {tag:?}");
        }
    }
}
