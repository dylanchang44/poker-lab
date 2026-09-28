//! The evaluator dependency is isolated here; the engine only sees HandValue.
use super::cards::{Card, Suit};
use rs_poker::core::{Card as EvalCard, Rank as EvalRank, Rankable, Suit as EvalSuit, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HandValue(EvalRank);

impl HandValue {
    pub fn category(self) -> &'static str {
        match self.category_index() {
            1 => "High card",
            2 => "One pair",
            3 => "Two pair",
            4 => "Three of a kind",
            5 => "Straight",
            6 => "Flush",
            7 => "Full house",
            8 => "Four of a kind",
            9 => "Straight flush",
            _ => unreachable!("validated evaluator score"),
        }
    }
    pub fn category_index(self) -> u16 {
        match self.0 {
            EvalRank::HighCard(_) => 1,
            EvalRank::OnePair(_) => 2,
            EvalRank::TwoPair(_) => 3,
            EvalRank::ThreeOfAKind(_) => 4,
            EvalRank::Straight(_) => 5,
            EvalRank::Flush(_) => 6,
            EvalRank::FullHouse(_) => 7,
            EvalRank::FourOfAKind(_) => 8,
            EvalRank::StraightFlush(_) => 9,
        }
    }
}

pub fn evaluate(cards: &[Card]) -> Result<HandValue, &'static str> {
    if !(5..=7).contains(&cards.len()) {
        return Err("Evaluation needs five to seven cards");
    }
    for (i, card) in cards.iter().enumerate() {
        if cards[..i].contains(card) {
            return Err("Duplicate card");
        }
    }
    let converted: Vec<_> = cards
        .iter()
        .map(|card| {
            EvalCard::new(
                Value::from_u8(card.rank as u8 - 2),
                match card.suit {
                    Suit::Clubs => EvalSuit::Club,
                    Suit::Diamonds => EvalSuit::Diamond,
                    Suit::Hearts => EvalSuit::Heart,
                    Suit::Spades => EvalSuit::Spade,
                },
            )
        })
        .collect();
    Ok(HandValue(converted.rank()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn value(s: &str) -> HandValue {
        evaluate(
            &s.split_whitespace()
                .map(|c| c.parse().unwrap())
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }

    #[test]
    fn all_nine_categories_are_ordered() {
        let hands = [
            "As Kd 8c 6h 2s",
            "As Ad 8c 6h 2s",
            "As Ad 8c 8h 2s",
            "As Ad Ac 6h 2s",
            "As 2d 3c 4h 5s",
            "As Js 8s 6s 2s",
            "As Ad Ac 6h 6s",
            "As Ad Ac Ah 2s",
            "9s Ts Js Qs Ks",
        ];
        let mut previous = None;
        for (i, hand) in hands.into_iter().enumerate() {
            let rank = value(hand);
            assert_eq!(rank.category_index(), i as u16 + 1);
            if let Some(previous) = previous {
                assert!(rank > previous);
            }
            previous = Some(rank);
        }
    }

    #[test]
    fn kickers_wheels_and_best_five() {
        for (winner, loser) in [
            ("As Ad Kc 6h 2s", "Ah Ac Qc 6d 2h"),
            ("As Ad Kc 6h 3s", "Ah Ac Kd 6d 2h"),
            ("As Ad 8c 8h Ks", "Ah Ac 8d 8s Qh"),
            ("As Ad Ac Kh 2s", "Ah Ac Ad Qd 2h"),
            ("2s 3d 4c 5h 6s", "As 2d 3c 4h 5s"),
            ("As Ks 9s 6s 2s", "Ah Qh 9h 6h 2h"),
            ("As Ad Ac Kh Ks", "Ah Ac Ad Qd Qh"),
            ("As Ad Ac Ah Ks", "As Ad Ac Ah Qs"),
            ("2s 3s 4s 5s 6s", "As 2s 3s 4s 5s"),
        ] {
            assert!(value(winner) > value(loser), "{winner} vs {loser}");
        }
        assert_eq!(value("As Ks Qs Js Ts 2d 3c"), value("As Ks Qs Js Ts 8d 9c"));
        assert_eq!(value("As Ad Ac Ks Kd Kh 2s"), value("As Ad Ac Ks Kd"));
        assert_eq!(value("As 2s 3s 4s 5s 9d Th").category(), "Straight flush");
    }

    #[test]
    fn rejects_duplicates_and_wrong_card_count() {
        assert!(evaluate(&[]).is_err());
        assert!(evaluate(&["As".parse().unwrap(); 5]).is_err());
    }
}
