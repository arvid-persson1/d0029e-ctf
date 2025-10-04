// TODO:
// `BitVec` would have a smaller memory footprint.
// `BTreeSet` or similar would have better performance for longer sequential skips.

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SkipSeq {
    passed: usize,
    offset: usize,
    skip: Vec<bool>,
}

impl SkipSeq {
    pub const fn new(start: usize) -> Self {
        Self::init(start, Vec::new())
    }

    pub fn with_capacity(start: usize, capacity: usize) -> Self {
        Self::init(start, Vec::with_capacity(capacity))
    }

    const fn init(offset: usize, skip: Vec<bool>) -> Self {
        Self {
            passed: 0,
            offset,
            skip,
        }
    }

    pub const fn peek(&self) -> usize {
        self.passed + self.offset
    }

    pub fn next(&mut self) -> usize {
        while self.skip.get(self.offset).copied().unwrap_or_default() {
            self.offset += 1;
        }

        let res = self.passed + self.offset;
        self.offset += 1;
        res
    }

    pub fn skip(&mut self, n: usize) -> bool {
        if n >= self.peek() {
            let i = n - self.passed;
            self.skip.reserve(i - self.skip.capacity());
            self.skip[i] = true;
            true
        } else {
            false
        }
    }

    // TODO:
    // Rename constructors? 4 options instead of 2?
    // `trim_start` as `passed` is redundant without it.
    // `trim_end`, possibly with option to ignore existent skips.
    // `skip_unchecked(n)`
    // `skip(n)` with exact reservation.
    // `is_skipped(n)`, possibly as `impl Index` and `skip` as `impl IndexMut`.
    // `impl Iterator`
    // `union(Self)`
    // `unskip(n)`, possibly under a different name.
    // `advance(n)`.
    // Fine-grained control over leading/trailing/total capacity.
}
