use crate::poker::{
    Observation,
    cards::{Card, Rank, Suit},
    hand::evaluate,
};
use rand::{Rng, seq::SliceRandom};

/// Monte Carlo share of the showdown against uniformly sampled unknown hands.
/// No match reference or private deal stream can be supplied to this function.
pub fn estimate(view: &Observation, samples: u32, rng: &mut impl Rng) -> f64 {
    let Some(own) = view.hole_cards else {
        return 0.0;
    };
    let opponents: Vec<_> = view
        .in_hand
        .iter()
        .enumerate()
        .filter(|(i, live)| **live && *i != view.seat.index())
        .map(|(i, _)| i)
        .collect();
    if opponents.is_empty() {
        return 1.0;
    }
    let mut known = view.board.clone();
    known.extend(own);
    if let Some(revealed) = &view.revealed_cards {
        for i in &opponents {
            if let Some(hand) = revealed[*i] {
                known.extend(hand);
            }
        }
    }
    let mut unknown: Vec<Card> = Suit::ALL
        .into_iter()
        .flat_map(|suit| Rank::ALL.into_iter().map(move |rank| Card { rank, suit }))
        .filter(|c| !known.contains(c))
        .collect();
    let mut equity = 0.0;
    for _ in 0..samples.max(1) {
        unknown.shuffle(rng);
        let missing = 5 - view.board.len();
        let mut board = view.board.clone();
        board.extend_from_slice(&unknown[..missing]);
        let mut index = missing;
        let mut cards = board.clone();
        cards.extend(own);
        let own_rank = evaluate(&cards).expect("valid observation cards");
        let mut ties = 1;
        let mut beaten = false;
        for i in &opponents {
            let hole = view
                .revealed_cards
                .as_ref()
                .and_then(|h| h[*i])
                .unwrap_or_else(|| {
                    let hand = [unknown[index], unknown[index + 1]];
                    index += 2;
                    hand
                });
            let mut cards = board.clone();
            cards.extend(hole);
            let rank = evaluate(&cards).expect("sampled without replacement");
            if rank > own_rank {
                beaten = true;
            }
            if rank == own_rank {
                ties += 1;
            }
        }
        if !beaten {
            equity += 1.0 / f64::from(ties);
        }
    }
    equity / f64::from(samples.max(1))
}

pub fn starting_strength(hole: [Card; 2]) -> f64 {
    let high = hole[0].rank.max(hole[1].rank) as u8;
    let low = hole[0].rank.min(hole[1].rank) as u8;
    if high == low {
        return 55.0 + f64::from(high) * 2.8;
    }
    let mut score = f64::from(high + low) * 2.0;
    if hole[0].suit == hole[1].suit {
        score += 8.0;
    }
    if high - low <= 1 {
        score += 6.0;
    } else if high - low >= 5 {
        score -= 8.0;
    }
    score
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Draws {
    pub flush: bool,
    pub straight: bool,
}
pub fn draws(hole: [Card; 2], board: &[Card]) -> Draws {
    if !(3..5).contains(&board.len()) {
        return Draws::default();
    }
    let all: Vec<_> = board.iter().copied().chain(hole).collect();
    let flush = Suit::ALL.iter().any(|s| {
        all.iter().filter(|c| c.suit == *s).count() == 4 && hole.iter().any(|c| c.suit == *s)
    });
    let mut ranks = [false; 15];
    for card in &all {
        ranks[card.rank as usize] = true;
        if card.rank == Rank::Ace {
            ranks[1] = true;
        }
    }
    let straight = (1..=10).any(|start| {
        ranks[start..start + 5].iter().filter(|x| **x).count() == 4
            && hole.iter().any(|c| {
                (start..start + 5).contains(&(c.rank as usize))
                    || (start == 1 && c.rank == Rank::Ace)
            })
    });
    Draws { flush, straight }
}
