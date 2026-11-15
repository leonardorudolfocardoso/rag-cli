use std::{ops::Range, path::Path, sync::Arc};

/// A piece of a source file, kept with where it came from so answers can cite it.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub source: Arc<Path>,
    pub text: String,
}

/// Splits Markdown into chunks of at most `size` chars, following its structure.
///
/// A heading always starts a new chunk, and paragraphs are packed together until
/// the next one would overflow `size`. Fenced code is never split at its blank lines.
/// Only a single block longer than `size` gets cut mid-text.
///
/// When a chunk is closed because it's full, its last paragraph is repeated at the
/// start of the next one if it's at most `overlap` chars, so text that straddles the
/// cut still lands whole in one chunk.
///
/// # Panics
/// If `size` is 0.
pub fn chunks(text: &str, size: usize, overlap: usize) -> Vec<&str> {
    assert!(size > 0, "chunk size must be at least 1");
    let len = |r: &Range<usize>| text[r.clone()].chars().count();

    let mut out = Vec::new();
    let mut current: Option<Range<usize>> = None;
    let mut last: Option<Range<usize>> = None; // last block in `current`, the overlap candidate

    for Block { range, heading } in blocks(text) {
        if len(&range) > size {
            out.extend(current.take().map(|c| &text[c]));
            out.extend(split_chars(&text[range], size));
            last = None;
            continue;
        }

        current = Some(match current {
            None => range.clone(),
            Some(c) if heading => {
                out.push(&text[c]);
                range.clone()
            }
            Some(c) if len(&(c.start..range.end)) <= size => c.start..range.end,
            Some(c) => {
                out.push(&text[c]);
                match last {
                    Some(l) if len(&l) <= overlap && len(&(l.start..range.end)) <= size => {
                        l.start..range.end
                    }
                    _ => range.clone(),
                }
            }
        });
        last = Some(range);
    }

    out.extend(current.map(|c| &text[c]));
    out
}

struct Block {
    range: Range<usize>,
    heading: bool,
}

/// Byte ranges of the Markdown blocks in `text`: paragraphs separated by blank
/// lines, where a heading line also starts a new block and fenced code stays whole.
fn blocks(text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut start: Option<(usize, bool)> = None;
    let mut in_fence = false;
    let mut pos = 0;

    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        let blank = !in_fence && trimmed.is_empty();
        let heading = !in_fence && is_heading(trimmed);

        if (blank || heading)
            && let Some((s, heading)) = start.take()
        {
            blocks.push(Block {
                range: s..pos,
                heading,
            });
        }
        if !blank && start.is_none() {
            start = Some((pos, heading));
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        }
        pos += line.len();
    }

    if let Some((s, heading)) = start {
        blocks.push(Block {
            range: s..text.len(),
            heading,
        });
    }
    blocks
}

/// `# Title` through `###### Title`, but not `#hashtag`.
fn is_heading(line: &str) -> bool {
    let hashes = line.len() - line.trim_start_matches('#').len();
    (1..=6).contains(&hashes) && (line.len() == hashes || line[hashes..].starts_with([' ', '\t']))
}

/// Cuts `text` every `size` chars, never inside a multi-byte char.
fn split_chars(text: &str, size: usize) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }

        let cut = rest
            .char_indices()
            .nth(size)
            .map(|(i, _)| i)
            .unwrap_or_else(|| rest.len());

        let (chunk, tail) = rest.split_at(cut);
        rest = tail;
        Some(chunk)
    })
}

#[cfg(test)]
mod test {
    use super::{blocks, chunks};

    #[test]
    fn long_paragraph_is_cut_on_char_boundaries() {
        let v = chunks("ação é ótima", 4, 0);
        assert_eq!(v, ["ação", " é ó", "tima"]); // 4 chars each, never cut inside "ç"
        assert_eq!(v.concat(), "ação é ótima"); // nothing lost
        assert!(chunks("", 4, 0).is_empty());
    }

    #[test]
    fn headings_start_a_new_chunk() {
        let v = chunks("# A\nalpha\n\n# B\nbeta\n", 100, 0);
        assert_eq!(v, ["# A\nalpha\n", "# B\nbeta\n"]);
    }

    #[test]
    fn hashtag_is_not_a_heading() {
        let v = chunks("intro\n#tag here\n", 100, 0);
        assert_eq!(v, ["intro\n#tag here\n"]);
    }

    #[test]
    fn packs_paragraphs_up_to_size() {
        let v = chunks("aa\n\nbb\n\ncc", 7, 0);
        assert_eq!(v, ["aa\n\nbb\n", "cc"]);
    }

    #[test]
    fn short_last_paragraph_overlaps_into_next_chunk() {
        let v = chunks("aa\n\nbb\n\ncc", 7, 3);
        assert_eq!(v, ["aa\n\nbb\n", "bb\n\ncc"]);
    }

    #[test]
    fn overlap_never_exceeds_size() {
        // carrying "bb\n" over would make "bb\n\ncccc" (9 chars) > 7, so it's dropped
        let v = chunks("aa\n\nbb\n\ncccc", 7, 3);
        assert_eq!(v, ["aa\n\nbb\n", "cccc"]);
    }

    #[test]
    fn fenced_code_is_one_block() {
        let text = "```\n# not a heading\n\nx\n```\n\nafter";
        let b: Vec<&str> = blocks(text).into_iter().map(|b| &text[b.range]).collect();
        assert_eq!(b, ["```\n# not a heading\n\nx\n```\n", "after"]);
    }

    #[test]
    fn no_chunk_exceeds_size() {
        let text = "# Title\nintro line\n\npara one is here\n\n## Sub\n\
                    a much longer paragraph that will not fit in one go\n\nend";
        for size in 1..40 {
            for c in chunks(text, size, size / 3) {
                assert!(c.chars().count() <= size, "size {size}: {c:?}");
            }
        }
    }
}
