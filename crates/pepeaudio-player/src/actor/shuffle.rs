use std::collections::VecDeque;
use uuid::Uuid;

const NONZERO_SEED: u64 = 0x9e37_79b9_7f4a_7c15;

pub(super) fn shuffle_queue<T>(queue: &mut VecDeque<T>, seed: u64) {
    let values = queue.make_contiguous();
    let mut random = SplitMix64::new(seed);
    for upper in (2..=values.len()).rev() {
        let index = random.index(upper);
        values.swap(upper - 1, index);
    }
}

pub(super) fn insert_shuffled<T>(queue: &mut VecDeque<T>, value: T, seed: u64) {
    let mut random = SplitMix64::new(seed);
    let index = random.index(queue.len().saturating_add(1));
    queue.insert(index, value);
}

pub(super) fn seed_from_uuid(id: Uuid) -> u64 {
    let bytes = *id.as_bytes();
    let low = u64::from_le_bytes(bytes[..8].try_into().expect("UUID half has eight bytes"));
    let high = u64::from_le_bytes(bytes[8..].try_into().expect("UUID half has eight bytes"));
    low ^ high
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self {
            state: seed ^ NONZERO_SEED,
        }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(NONZERO_SEED);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn index(&mut self, upper: usize) -> usize {
        debug_assert!(upper > 0);
        let index = u128::from(self.next()) % (upper as u128);
        usize::try_from(index).expect("index is strictly smaller than usize upper bound")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::{insert_shuffled, shuffle_queue};

    #[test]
    fn seeded_shuffle_is_deterministic_and_preserves_every_item() {
        let original: VecDeque<_> = (0..8).collect();
        let mut first = original.clone();
        let mut second = original.clone();
        shuffle_queue(&mut first, 42);
        shuffle_queue(&mut second, 42);

        assert_eq!(first, second);
        assert_eq!(first, VecDeque::from([7, 4, 1, 2, 5, 6, 0, 3]));
        assert_ne!(first, original);
        let mut actual: Vec<_> = first.into_iter().collect();
        actual.sort_unstable();
        assert_eq!(actual, (0..8).collect::<Vec<_>>());
    }

    #[test]
    fn empty_and_single_item_queues_are_stable() {
        let mut empty = VecDeque::<u8>::new();
        shuffle_queue(&mut empty, 7);
        assert!(empty.is_empty());

        let mut one = VecDeque::from([1]);
        shuffle_queue(&mut one, 7);
        assert_eq!(one, VecDeque::from([1]));
    }

    #[test]
    fn shuffled_insertion_preserves_existing_items_and_adds_one() {
        let mut queue = VecDeque::from([1, 2, 3]);
        insert_shuffled(&mut queue, 4, 91);
        let mut actual: Vec<_> = queue.into_iter().collect();
        actual.sort_unstable();
        assert_eq!(actual, vec![1, 2, 3, 4]);
    }
}
