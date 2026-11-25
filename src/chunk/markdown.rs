use super::{Chunker, blocks, pack};

/// Splits Markdown (and Logseq) into chunks of at most `max_chars` chars, following its structure.
///
/// A heading always starts a new chunk, and paragraphs are packed together until
/// the next one would overflow `max_chars`. Fenced code is never split at its blank lines.
/// Only a single block longer than `max_chars` gets cut mid-text.
///
/// # Panics
/// When chunking, if `max_chars` is 0.
#[derive(Debug, Clone, Copy)]
pub struct Markdown {
    pub max_chars: usize,
}

impl Chunker for Markdown {
    fn chunk<'a>(&self, text: &'a str) -> Vec<&'a str> {
        pack(text, blocks(text, is_heading, true), self.max_chars)
    }
}

/// `# Title` through `###### Title`, including bulleted Logseq headings (`- ## Title`),
/// but not `#hashtag`.
fn is_heading(line: &str) -> bool {
    let line = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .map_or(line, str::trim_start);
    let hashes = line.len() - line.trim_start_matches('#').len();
    (1..=6).contains(&hashes) && (line.len() == hashes || line[hashes..].starts_with([' ', '\t']))
}

#[cfg(test)]
mod test {
    use super::{Markdown, is_heading};
    use crate::chunk::{Chunker, blocks};

    fn chunk_markdown(text: &str, max_chars: usize) -> Vec<&str> {
        Markdown { max_chars }.chunk(text)
    }

    #[test]
    fn long_paragraph_is_cut_on_char_boundaries() {
        let v = chunk_markdown("ação é ótima", 4);
        assert_eq!(v, ["ação", " é ó", "tima"]); // 4 chars each, never cut inside "ç"
        assert_eq!(v.concat(), "ação é ótima"); // nothing lost
        assert!(chunk_markdown("", 4).is_empty());
    }

    #[test]
    fn headings_start_a_new_chunk() {
        let v = chunk_markdown("# A\nalpha\n\n# B\nbeta\n", 100);
        assert_eq!(v, ["# A\nalpha\n", "# B\nbeta\n"]);
    }

    #[test]
    fn logseq_bullet_headings_start_a_new_chunk() {
        let v = chunk_markdown("- ## A\n\t- alpha\n- ## B\n\t- beta\n", 100);
        assert_eq!(v, ["- ## A\n\t- alpha\n", "- ## B\n\t- beta\n"]);
    }

    #[test]
    fn consecutive_headings_do_not_create_orphan_chunk() {
        let v = chunk_markdown("# Doc\n\n## First\n\nalpha\n\n## Second\n\nbeta\n", 100);
        assert_eq!(v, ["# Doc\n\n## First\n\nalpha\n", "## Second\n\nbeta\n"]);
    }

    #[test]
    fn hashtag_is_not_a_heading() {
        let v = chunk_markdown("intro\n#tag here\n", 100);
        assert_eq!(v, ["intro\n#tag here\n"]);
    }

    #[test]
    fn packs_paragraphs_up_to_size() {
        let v = chunk_markdown("aa\n\nbb\n\ncc", 7);
        assert_eq!(v, ["aa\n\nbb\n", "cc"]);
    }

    #[test]
    fn fenced_code_is_one_block() {
        let text = "```\n# not a heading\n\nx\n```\n\nafter";
        let b: Vec<&str> = blocks(text, is_heading, true)
            .into_iter()
            .map(|b| &text[b.range])
            .collect();
        assert_eq!(b, ["```\n# not a heading\n\nx\n```\n", "after"]);
    }

    #[test]
    fn no_chunk_exceeds_size() {
        let text = "# Title\nintro line\n\npara one is here\n\n## Sub\n\
                    a much longer paragraph that will not fit in one go\n\nend";
        for size in 1..40 {
            for c in chunk_markdown(text, size) {
                assert!(c.chars().count() <= size, "size {size}: {c:?}");
            }
        }
    }
}
