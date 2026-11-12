fn chunks(doc: &str, size: usize) -> impl Iterator<Item = &str> {
    let mut rest = doc;
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

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod test {
    use super::chunks;

    #[test]
    fn test_chunks() {
        let v: Vec<&str> = chunks("ação é ótima", 4).collect();
        assert_eq!(v, ["ação", " é ó", "tima"]); // 4 chars each, never cut inside "ç"
        assert_eq!(v.concat(), "ação é ótima"); // nothing lost
        assert_eq!(chunks("", 4).count(), 0);
    }
}
