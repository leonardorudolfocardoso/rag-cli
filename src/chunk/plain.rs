use super::{Chunker, blocks, pack};

/// Splits unstructured text into chunks of at most `max_chars` chars.
///
/// Paragraphs (separated by blank lines) are packed together until the next one would
/// overflow `max_chars`. There is no notion of headings or code, so it is the baseline
/// for structure-aware chunkers. Only a single paragraph longer than `max_chars` gets cut mid-text.
///
/// # Panics
/// When chunking, if `max_chars` is 0.
#[derive(Debug, Clone, Copy)]
pub struct Plain {
    pub max_chars: usize,
}

impl Chunker for Plain {
    fn chunk<'a>(&self, text: &'a str) -> Vec<&'a str> {
        pack(text, blocks(text, |_| false, false), self.max_chars)
    }
}

#[cfg(test)]
mod test {
    use super::Plain;
    use crate::chunk::{Chunker, Markdown};

    fn chunk_plain(text: &str, max_chars: usize) -> Vec<&str> {
        Plain { max_chars }.chunk(text)
    }

    #[test]
    fn plain_ignores_markdown_structure() {
        let text = "# A\nalpha\n\n# B\nbeta\n";
        assert_eq!(chunk_plain(text, 100), [text]);
        assert_eq!(Markdown { max_chars: 100 }.chunk(text).len(), 2);

        let fenced = "```\nx\n\ny\n```";
        assert_eq!(chunk_plain(fenced, 6), ["```\nx\n", "y\n```"]);
    }

    #[test]
    fn plain_packs_paragraphs_up_to_size() {
        assert_eq!(chunk_plain("aa\n\nbb\n\ncc", 7), ["aa\n\nbb\n", "cc"]);
    }
}
