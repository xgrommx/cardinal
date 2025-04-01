#[derive(Default)]
pub struct NamePool {
    pool: Vec<u8>,
}

impl NamePool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.pool.len()
    }

    pub fn push(&mut self, name: &str) -> usize {
        let start = self.pool.len();
        self.pool.extend_from_slice(name.as_bytes());
        self.pool.push(0);
        start
    }

    fn get(&self, offset: usize) -> &str {
        let begin = self.pool[..offset]
            .iter()
            .rposition(|&x| x == 0)
            .map(|x| x + 1)
            .unwrap_or(0);
        let end = self.pool[offset..]
            .iter()
            .position(|&x| x == 0)
            .unwrap_or(self.pool.len() - offset);
        unsafe { std::str::from_utf8_unchecked(&self.pool[begin..offset + end]) }
    }

    pub fn search_substr<'a>(&'a self, substr: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        memchr::memmem::find_iter(&self.pool, substr.as_bytes()).map(|x| self.get(x))
    }
}
