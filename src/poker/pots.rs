use super::state::*;

/// Layers include folded money, but only live contributors can win a layer.
/// During betting these are provisional; unmatched top chips are refunded at close.
pub fn construct(contributions: &[Chips], in_hand: &[bool]) -> Vec<Pot> {
    let mut levels: Vec<_> = contributions.iter().copied().filter(|n| *n > 0).collect();
    levels.sort_unstable();
    levels.dedup();
    let mut previous = 0;
    let mut pots: Vec<Pot> = Vec::new();
    for level in levels {
        let contributors = contributions.iter().filter(|n| **n >= level).count() as Chips;
        let amount = (level - previous) * contributors;
        let eligible = contributions
            .iter()
            .enumerate()
            .filter(|(i, n)| **n >= level && in_hand[*i])
            .map(|(i, _)| Seat::ALL[i])
            .collect::<Vec<_>>();
        if let Some(last) = pots.last_mut()
            && last.eligible == eligible
        {
            last.amount += amount;
        } else {
            pots.push(Pot { amount, eligible });
        }
        previous = level;
    }
    pots
}

pub fn split(amount: Chips, winners: &[Seat], dealer: Seat, seats: usize) -> Vec<Chips> {
    assert!(!winners.is_empty());
    let mut awards = vec![0; seats];
    for seat in winners {
        awards[seat.index()] = amount / winners.len() as Chips;
    }
    let mut odd = amount % winners.len() as Chips;
    for offset in 1..=seats {
        let seat = Seat::ALL[(dealer.index() + offset) % seats];
        if odd > 0 && winners.contains(&seat) {
            awards[seat.index()] += 1;
            odd -= 1;
        }
    }
    awards
}
