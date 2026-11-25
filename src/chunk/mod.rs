use std::{ops::Range, path::Path, sync::Arc};

mod markdown;
mod plain;

pub use markdown::Markdown;
pub use plain::Plain;

/// A piece of a source file, kept with where it came from so answers can cite it.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub source: Arc<Path>,
    pub text: Arc<str>,
}

/// Chunks split from one or more documents, in document order.
#[derive(Debug, Clone, Default)]
pub struct Chunks(Vec<Chunk>);

impl std::ops::Deref for Chunks {
    type Target = [Chunk];

    fn deref(&self) -> &[Chunk] {
        &self.0
    }
}

impl FromIterator<Chunk> for Chunks {
    fn from_iter<I: IntoIterator<Item = Chunk>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoIterator for Chunks {
    type Item = Chunk;
    type IntoIter = std::vec::IntoIter<Chunk>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Splits a document into chunks.
pub trait Chunker {
    fn chunk<'a>(&self, text: &'a str) -> Vec<&'a str>;
}

/// Packs `blocks` of `markdown` into chunks of at most `max_chars` chars.
/// A heading block starts a new chunk unless the chunk so far is only headings.
fn pack(markdown: &str, blocks: Vec<Block>, max_chars: usize) -> Vec<&str> {
    assert!(max_chars > 0, "max_chars must be at least 1");
    let len = |r: &Range<usize>| markdown[r.clone()].chars().count();

    let mut out = Vec::new();
    let mut current: Option<(Range<usize>, bool)> = None;

    for block in blocks {
        let Block {
            range,
            heading,
            heading_only,
        } = block;

        if len(&range) > max_chars {
            out.extend(current.take().map(|(c, _)| &markdown[c]));
            out.extend(split_chars(&markdown[range], max_chars));
            continue;
        }

        current = Some(match current {
            None => (range, heading_only),
            // Don't flush a heading-only prefix (e.g. `# Doc\n\n## Section`) into an orphan chunk.
            Some((c, false)) if heading => {
                out.push(&markdown[c]);
                (range, heading_only)
            }
            Some((c, cur_heading_only)) if len(&(c.start..range.end)) <= max_chars => {
                (c.start..range.end, cur_heading_only && heading_only)
            }
            Some((c, _)) => {
                out.push(&markdown[c]);
                (range, heading_only)
            }
        });
    }

    out.extend(current.map(|(c, _)| &markdown[c]));
    out
}

struct Block {
    range: Range<usize>,
    heading: bool,
    /// The block is only a heading, with no body attached, so it shouldn't become a chunk alone.
    heading_only: bool,
}

impl Block {
    /// A block over `range`; a heading counts as heading-only when it is a single line.
    fn new(text: &str, range: Range<usize>, heading: bool) -> Self {
        let heading_only = heading && text[range.clone()].lines().take(2).count() == 1;
        Self {
            range,
            heading,
            heading_only,
        }
    }
}

/// Byte ranges of the blocks in `text`: paragraphs separated by blank lines, where a
/// line accepted by `is_heading` also starts a new block and, if `fences`, fenced code stays whole.
fn blocks(text: &str, is_heading: impl Fn(&str) -> bool, fences: bool) -> Vec<Block> {
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
            blocks.push(Block::new(text, s..pos, heading));
        }
        if !blank && start.is_none() {
            start = Some((pos, heading));
        }
        if fences && (trimmed.starts_with("```") || trimmed.starts_with("~~~")) {
            in_fence = !in_fence;
        }
        pos += line.len();
    }

    if let Some((s, heading)) = start {
        blocks.push(Block::new(text, s..text.len(), heading));
    }
    blocks
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
