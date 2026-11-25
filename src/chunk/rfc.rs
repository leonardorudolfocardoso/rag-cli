use std::collections::HashMap;

use super::{Chunker, blocks, pack};

/// Splits IETF RFC plaintext into chunks of at most `max_chars` chars, following its sections.
///
/// Same packing as [`Markdown`](super::Markdown), but headings are found the way a reader
/// spots them in a laid-out text document, not by syntax: a short standalone block that
/// sticks out left of the body text, isn't a sentence, and isn't page furniture. Page
/// headers and footers are left in place, since chunks are slices of the input.
///
/// # Panics
/// When chunking, if `max_chars` is 0.
#[derive(Debug, Clone, Copy)]
pub struct Rfc {
    pub max_chars: usize,
}

impl Chunker for Rfc {
    fn chunk<'a>(&self, text: &'a str) -> Vec<&'a str> {
        let layout = Layout::of(text);
        let mut blocks = blocks(text, |_| false, false);
        for b in &mut blocks {
            if layout.is_heading(&text[b.range.clone()]) {
                b.heading = true;
                b.heading_only = true;
            }
        }
        pack(text, blocks, self.max_chars)
    }
}

/// What a reader takes in at a glance before spotting headings in a laid-out text document.
struct Layout {
    /// The indent carrying most of the text (furniture aside): where body text starts.
    body_indent: usize,
    /// How often each line occurs, once normalised by [`furniture_key`].
    occurrences: HashMap<String, usize>,
}

impl Layout {
    /// A line repeated this often (ignoring numbers and spacing) is a page header or footer.
    const FURNITURE_REPEATS: usize = 3;
    /// A heading is one line, or two when a long title wraps.
    const MAX_HEADING_LINES: usize = 2;

    fn of(text: &str) -> Self {
        let lines = || text.lines().filter(|l| !l.trim().is_empty());
        let mut occurrences: HashMap<String, usize> = HashMap::new();
        for line in lines() {
            *occurrences.entry(furniture_key(line)).or_default() += 1;
        }
        let mut indents: HashMap<usize, usize> = HashMap::new();
        for line in lines().filter(|l| occurrences[&furniture_key(l)] < Self::FURNITURE_REPEATS) {
            *indents.entry(indent(line)).or_default() += line.trim().chars().count();
        }
        let body_indent = indents
            .into_iter()
            .max_by_key(|&(indent, n)| (n, std::cmp::Reverse(indent)))
            .map_or(0, |(indent, _)| indent);
        Self {
            body_indent,
            occurrences,
        }
    }

    /// True when `block` (a run of non-blank lines) reads as a heading:
    /// - it sticks out: its first line is indented less than the body,
    /// - it stands alone: at most [`Self::MAX_HEADING_LINES`] lines, any wrap indented further,
    /// - it's a title, not a sentence: no closing `.`, `:` or `,`,
    /// - it isn't page furniture: it doesn't repeat across pages.
    fn is_heading(&self, block: &str) -> bool {
        let mut lines = block.lines();
        let Some(first) = lines.next() else {
            return false;
        };
        let rest: Vec<&str> = lines.collect();

        indent(first) < self.body_indent
            && rest.len() < Self::MAX_HEADING_LINES
            && rest.iter().all(|l| indent(l) > indent(first))
            && !first.trim_end().ends_with(['.', ':', ','])
            && self.occurrences[&furniture_key(first)] < Self::FURNITURE_REPEATS
    }
}

/// Leading whitespace, in chars.
fn indent(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

/// `line` with digit runs masked and whitespace collapsed, so the footers
/// `Jones & Hardt   Standards Track    [Page 9]` and `... [Page 10]` compare equal.
fn furniture_key(line: &str) -> String {
    let mut key = String::with_capacity(line.len());
    for c in line.trim().chars() {
        let c = match c {
            c if c.is_ascii_digit() => '#',
            c if c.is_whitespace() => ' ',
            c => c,
        };
        if !(matches!(c, '#' | ' ') && key.ends_with(c)) {
            key.push(c);
        }
    }
    key
}

#[cfg(test)]
mod test {
    use super::{Layout, Rfc};
    use crate::chunk::{Chunker, Plain, blocks};

    /// An RFC-like page: body indented by 3, two footers, flush-left headings.
    const PAGE: &str = "\
1.  Introduction

   Body text that explains things at some length, the way a
   specification does, so that the body carries most of the text.

Doe                     Standards Track                     [Page 1]
\u{c}
1.1.  A Long Subsection Title That Wraps
      Onto a Second Line

   Body of the subsection, again long enough to read as prose
   rather than as a title or a short label of some kind.

2.  Next

   Body two, which continues the specification with more words.

Doe                     Standards Track                     [Page 2]
\u{c}
Doe                     Standards Track                    [Page 10]
";

    fn rfc_headings(text: &str) -> Vec<&str> {
        let layout = Layout::of(text);
        blocks(text, |_| false, false)
            .into_iter()
            .map(|b| &text[b.range])
            .filter(|b| layout.is_heading(b))
            .map(|b| b.lines().next().unwrap())
            .collect()
    }

    #[test]
    fn rfc_headings_follow_layout_cues() {
        assert_eq!(
            rfc_headings(PAGE),
            [
                "1.  Introduction",
                "1.1.  A Long Subsection Title That Wraps",
                "2.  Next"
            ]
        );
    }

    #[test]
    fn rfc_heading_must_stick_out_of_the_body() {
        let text = "   1.  Not Outdented\n\n   body\n\n   more body\n";
        assert!(rfc_headings(text).is_empty());
    }

    #[test]
    fn rfc_flush_left_sentence_is_not_a_heading() {
        let text = "This line ends like a sentence.\n\n   body\n\n   more body\n";
        assert!(rfc_headings(text).is_empty());
    }

    #[test]
    fn rfc_flush_left_paragraph_is_not_a_heading() {
        let text =
            "Header block line one\nHeader block line two\nline three\n\n   body\n\n   more\n";
        assert!(rfc_headings(text).is_empty());
    }

    #[test]
    fn rfc_without_indented_body_falls_back_to_plain() {
        let text = "Title\n\npara one\n\npara two\n";
        assert_eq!(
            Rfc { max_chars: 100 }.chunk(text),
            Plain { max_chars: 100 }.chunk(text)
        );
    }

    #[test]
    fn rfc_sections_start_a_new_chunk() {
        let text = "1.  Intro\n\n   alpha body text, long enough to be prose\n\n\
                    2.  Next\n\n   beta body text, long enough to be prose\n";
        assert_eq!(
            Rfc { max_chars: 200 }.chunk(text),
            [
                "1.  Intro\n\n   alpha body text, long enough to be prose\n",
                "2.  Next\n\n   beta body text, long enough to be prose\n"
            ]
        );
        assert_eq!(Plain { max_chars: 200 }.chunk(text), [text]);
    }

    #[test]
    fn rfc_wrapped_heading_is_not_an_orphan_chunk() {
        let text = "4.  Protocol\n\n4.1.  A Title That\n      Wraps\n\n   body\n\n   more\n";
        assert_eq!(Rfc { max_chars: 100 }.chunk(text), [text]);
    }

    /// On the real RFCs, layout cues find exactly the numbered and standard unnumbered
    /// section titles (checked by pattern here, as an oracle only).
    #[test]
    fn rfc_headings_match_fixture_sections() {
        const UNNUMBERED: &[&str] = &[
            "Abstract",
            "Status of This Memo",
            "Copyright Notice",
            "Table of Contents",
            "Acknowledgements",
            "Authors' Addresses",
        ];
        let fixtures = [
            include_str!("../../tests/fixtures_plain/rfc6585.txt"),
            include_str!("../../tests/fixtures_plain/rfc6750.txt"),
            include_str!("../../tests/fixtures_plain/rfc7636.txt"),
        ];
        for text in fixtures {
            let expected: Vec<&str> = text
                .lines()
                .filter(|l| {
                    let number = l.split_whitespace().next().unwrap_or_default();
                    (l.starts_with(|c: char| c.is_ascii_digit())
                        && number.ends_with('.')
                        && number.bytes().all(|b| b.is_ascii_digit() || b == b'.'))
                        || l.starts_with("Appendix ")
                        || UNNUMBERED.contains(l)
                })
                .collect();
            assert!(expected.len() > 20);
            assert_eq!(rfc_headings(text), expected);
        }
    }
}
