use super::cards::{Card, Rank, Suit};
use rand::{SeedableRng, seq::SliceRandom};
use rand_chacha::ChaCha8Rng;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deck {
    cards: Vec<Card>,
}

impl Deck {
    pub fn shuffled(seed: u64) -> Self {
        let mut cards: Vec<_> = Suit::ALL
            .into_iter()
            .flat_map(|suit| Rank::ALL.into_iter().map(move |rank| Card { rank, suit }))
            .collect();
        cards.shuffle(&mut ChaCha8Rng::seed_from_u64(seed));
        Self { cards }
    }

    pub fn deal(&mut self) -> Option<Card> {
        self.cards.pop()
    }
    pub fn remaining(&self) -> usize {
        self.cards.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn shuffled_decks_have_52_unique_cards_and_repeat_with_seed() {
        for seed in 0..100 {
            let mut deck = Deck::shuffled(seed);
            assert_eq!(deck, Deck::shuffled(seed));
            assert_eq!(deck.remaining(), 52);
            let cards: HashSet<_> = std::iter::from_fn(|| deck.deal()).collect();
            assert_eq!(cards.len(), 52);
            assert_eq!(deck.deal(), None);
        }
        assert_ne!(Deck::shuffled(1), Deck::shuffled(2));
    }
}
