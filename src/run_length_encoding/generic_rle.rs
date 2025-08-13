//! Provides a generic implementation of Run Length Encoding for use with arbitrary types.

use rle::{AppendRle, MergableSpan};

/// Represents an individual run of a specific token.
///
/// Individual runs are of indefinite length by themselves. The end of the run (and thus the length) is
/// dictated by the start value of the next run in your sequence.
///
/// Type `T` is the token value that you're encoding.
/// Type `S` is the sequence index type. The default of `usize` should work for most cases, but you
/// can save space if you know that your token sequences have a length that can be specified with a smaller
/// sized type.
#[derive(Clone)]
pub struct IndexedRLE<T: Clone + Eq, S = usize> {
    pub token: T,
    pub start: S,
}

impl<T: Clone + Eq, S: Copy + PartialEq + PartialOrd> MergableSpan for IndexedRLE<T, S> {
    fn can_append(&self, other: &Self) -> bool {
        // Since this is an indefinite-length run, we only need to check for start value orderings
        (self.token == other.token) & (self.start <= other.start)
    }

    fn append(&mut self, other: Self) {
        // Appending the same token does nothing, since this just measures the start of the run
        ()
    }

    fn prepend(&mut self, other: Self) {
        // Unlike when appending, when prepending we do need to keep track of which run starts
        // sooner, since if the other run starts sooner, we need to extend this run back to that
        // one.
        if other.start < self.start {
            self.start = other.start;
        }
    }
}

impl<T: Clone + Eq, S> IndexedRLE<T, S> {
    pub fn new(token: T, start: S) -> Self {
        Self { token, start }
    }

    /// The amount of memory it takes to store this data.
    pub fn memory_size(&self) -> usize {
        size_of::<T>() + size_of::<S>()
    }
}

/// An ordered sequence of runs, searchable in O(lg(n)) time via binary search.
pub struct BinarySearchRLE<T: Clone + Eq, S = usize> {
    vec: Vec<IndexedRLE<T, S>>,
}

impl<T: Clone + Eq, S: Copy + PartialEq + PartialOrd> BinarySearchRLE<T, S> {
    /// Create a new, empty BinarySearchRLE.
    pub fn new() -> Self {
        Self {
            vec: Vec::new(),
        }
    }

    /// Appends the provided run to the sequence.
    ///
    /// Returns true if the run was added to the end of the internal list, and returns false if it
    /// was instead merged into the last run.
    ///
    /// Runs are expected to be inserted in ascending sorted order; this does no sorting.
    pub fn append_run(&mut self, run: IndexedRLE<T, S>) -> bool {
        self.vec.push_rle(run)
    }

    /// Appends an individual token to the sequence as its own run.
    ///
    /// Returns true if the token was added to the end of the internal list as its own run, and
    /// returns false if it was instead merged into the last run.
    ///
    /// Tokens are expected to be inserted in ascending sorted order; this does no sorting.
    pub fn append_token(&mut self, token: T, start: S) -> bool {
        let run = IndexedRLE::new(token, start);
        self.append_run(run)
    }

    /// Search for the token value at a particular index in the sequence.
    ///
    /// Returns None if:
    /// - The sequence is empty (there are no runs)
    /// - The sequence index requested is before the start of the first run
    ///
    /// Otherwise, this returns the token value for the run that contains the requested index.
    pub fn find_token_at_index(&self, index: S) -> Option<T> {
        if self.vec.len() == 0 {
            None
        } else {
            // Edge case: If the token index requested is before the first run, return None and
            // avoid a fruitless search
            if index < self.vec[0].start {
                None
            } else {
                // Slices already implement binary search, so we can avoid all the manual implementation
                let idx = (&self.vec).partition_point(|item| item.start < index);

                let run_idx = if idx == self.vec.len() {
                    // If the token index requested is after the start of the last run, the partition point can
                    // return self.vec.len() as the run index
                    idx - 1
                } else {
                    // Two cases:
                    // - The token index is at the start of a run; this means we want the current
                    // run that partition point gave us
                    // - The token index is in the middle of a run; this means we want the previous
                    // run from what `partition_point` gave us
                    let current_run = &self.vec[idx];
                    if current_run.start == index {
                        idx
                    } else {
                        idx - 1
                    }
                };

                Some(self.vec[run_idx].token.clone())
            }
        }
    }

    /// Returns the total number of runs contained in the sequence.
    pub fn num_runs(&self) -> usize {
        self.vec.len()
    }

    /// The amount of memory it takes to store this data.
    pub fn memory_size(&self) -> usize {
        self.vec.len() * size_of::<IndexedRLE<T, S>>() + size_of::<Vec<IndexedRLE<T, S>>>()
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use itertools::Itertools;

    #[test]
    pub fn indexed_rle_can_append_accepts_valid_runs() {
        let max_start: usize = 1000;
        for start in 0..=max_start {
            for after in (start + 1)..=(max_start + 1) {
                let lower_rle: IndexedRLE<bool> = IndexedRLE::new(true, start);
                let higher_matching_rle: IndexedRLE<bool> = IndexedRLE::new(true, after);
                let higher_nonmatching_rle: IndexedRLE<bool> = IndexedRLE::new(false, after);

                assert!(lower_rle.can_append(&lower_rle), "Valid self append failed: {start} = {after}");
                assert!(lower_rle.can_append(&higher_matching_rle), "Valid append failed: {start} < {after}");
                assert!(!lower_rle.can_append(&higher_nonmatching_rle), "Non-matching token append succeeded");
                assert!(!higher_matching_rle.can_append(&lower_rle), "Out-of-order matching token append succeeded: {start} < {after}");
                assert!(!higher_nonmatching_rle.can_append(&lower_rle), "Out-of-order non-matching token append succeeded");
            }
        }
    }

    #[test]
    pub fn binary_search_rle_append_run_merges_properly() {
        let mut rle_data = BinarySearchRLE::<bool>::new();

        let start = 10;
        let after = 20;

        let lower_rle: IndexedRLE<bool> = IndexedRLE::new(true, start);
        let higher_matching_rle: IndexedRLE<bool> = IndexedRLE::new(true, after);
        let higher_nonmatching_rle: IndexedRLE<bool> = IndexedRLE::new(false, after);

        assert_eq!(rle_data.num_runs(), 0); // There should be no runs before we've started anything

        // Add the initial run
        rle_data.append_run(lower_rle);
        assert_eq!(rle_data.num_runs(), 1);

        // Test that appending a matching token run doesn't increase the length of the internal
        // data vector
        rle_data.append_run(higher_matching_rle);
        assert_eq!(rle_data.num_runs(), 1); // We appended a matching run, so there should not be an increase in the total number of runs

        // Test that appending a non-matching token run increases the length of the internal data
        // vector
        rle_data.append_run(higher_nonmatching_rle);
        assert_eq!(rle_data.num_runs(), 2); // We appended a non-matching run, so there should be an increase in the total number of runs
    }

    #[test]
    pub fn binary_search_rle_append_token_merges_properly() {
        let mut rle_data = BinarySearchRLE::<bool>::new();

        let start = 10;
        let after = 20;

        let matching_token = true;
        let nonmatching_token = false;

        let lower_rle: IndexedRLE<bool> = IndexedRLE::new(true, start);

        assert_eq!(rle_data.num_runs(), 0); // There should be no runs before we've started anything

        // Add the initial run
        rle_data.append_run(lower_rle);
        assert_eq!(rle_data.num_runs(), 1);

        // Test that appending a matching token doesn't increase the length of the internal
        // data vector
        rle_data.append_token(matching_token, after);
        assert_eq!(rle_data.num_runs(), 1); // We appended a matching token, so there should not be an increase in the total number of runs

        // Test that appending a non-matching token increases the length of the internal data
        // vector
        rle_data.append_token(nonmatching_token, after);
        assert_eq!(rle_data.num_runs(), 2); // We appended a non-matching token, so there should be an increase in the total number of runs
    }

    #[test]
    pub fn binary_search_rle_find_token_at_index_returns_none_when_empty() {
        let mut rle_data = BinarySearchRLE::<bool>::new();
        assert_eq!(rle_data.find_token_at_index(0), None);
    }

    #[test]
    pub fn binary_search_rle_find_token_at_index_returns_none_for_index_before_first_run() {
        let mut rle_data = BinarySearchRLE::<bool>::new();
        let rle: IndexedRLE<bool> = IndexedRLE::new(true, 10);
        rle_data.append_run(rle);
        assert_eq!(rle_data.num_runs(), 1);
        assert_eq!(rle_data.find_token_at_index(5), None);
    }

    #[test]
    pub fn binary_search_rle_find_token_at_index_works() {
        let mut rle_data = BinarySearchRLE::<bool>::new();

        let (first_start, first_len) = (10, 4);
        let (second_start, second_len) = ((first_start + first_len), 13);
        let (third_start, third_len) = ((second_start + second_len), 2);
        let (fourth_start, fourth_len) = ((third_start + third_len), 1);
        let (fifth_start, fifth_len) = ((fourth_start + fourth_len), 21);

        let first_end_inclusive = second_start - 1;
        let second_end_inclusive = third_start - 1;
        let third_end_inclusive = fourth_start - 1;
        let fourth_end_inclusive = fifth_start - 1;
        let fifth_end_inclusive = fifth_start + fifth_len;

        let first_rle: IndexedRLE<bool> = IndexedRLE::new(true, first_start);
        let second_rle: IndexedRLE<bool> = IndexedRLE::new(false, second_start);
        let third_rle: IndexedRLE<bool> = IndexedRLE::new(true, third_start);
        let fourth_rle: IndexedRLE<bool> = IndexedRLE::new(false, fourth_start);
        let fifth_rle: IndexedRLE<bool> = IndexedRLE::new(true, fifth_start);

        let true_token_ranges: Vec<(usize, usize)> = vec!((first_start, second_start), (third_start, fourth_start), (fifth_start, fifth_end_inclusive+1));
        let false_token_ranges: Vec<(usize, usize)> = vec!((second_start, third_start), (fourth_start, fifth_start));

        println!("First range inclusive: {first_start} - {first_end_inclusive}");
        println!("Second range inclusive: {second_start} - {second_end_inclusive}");
        println!("Third range inclusive: {third_start} - {third_end_inclusive}");
        println!("Fourth range inclusive: {fourth_start} - {fourth_end_inclusive}");
        println!("Fifth range inclusive: {fifth_start} - {fifth_end_inclusive}");

        assert_eq!(rle_data.num_runs(), 0); // There should be no runs before we've started anything

        // Add the runs
        rle_data.append_run(first_rle);
        rle_data.append_run(second_rle);
        rle_data.append_run(third_rle);
        rle_data.append_run(fourth_rle);
        rle_data.append_run(fifth_rle);
        assert_eq!(rle_data.num_runs(), 5);

        // Validate that the known token ranges match
        for (range_start, range_end) in true_token_ranges {
            for token_index in range_start..range_end {
                assert_eq!(rle_data.find_token_at_index(token_index), Some(true), "Token index {token_index} expected to be true");
            }
        }

        for (range_start, range_end) in false_token_ranges {
            for token_index in range_start..range_end {
                assert_eq!(rle_data.find_token_at_index(token_index), Some(false), "Token index {token_index} expected to be false");
            }
        }
    }
}
