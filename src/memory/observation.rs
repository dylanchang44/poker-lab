use super::models::*;
use crate::poker::{Action, Phase, Seat, cards::Card, events::GameEvent, hand::evaluate};

/// Trusted adapter. Only public dealt cards at showdown are examined; private
/// deal/burn/seed events are ignored and cannot become persistent facts.
#[derive(Default)]
pub struct PokerObserver {
    hand: u64,
    witnesses: Vec<NpcId>,
    board: Vec<Card>,
    river_aggression: bool,
    shown_high_card: bool,
    showdown: bool,
    counted_preflop: bool,
    opportunities: u32,
    raises: u32,
    street_high: u32,
}
impl PokerObserver {
    pub fn observe(&mut self, event: &GameEvent) -> Vec<ObservedEvent> {
        let mut facts = Vec::new();
        match event {
            GameEvent::HandStarted { number, stacks, .. } => {
                self.hand = *number;
                self.street_high = 0;
                let witnesses = NpcId::ALL
                    .into_iter()
                    .filter(|n| stacks.get(n.seat().index()).is_some_and(|s| *s > 0))
                    .collect();
                if self.witnesses != witnesses {
                    self.opportunities = 0;
                    self.raises = 0;
                }
                self.witnesses = witnesses;
                self.board.clear();
                self.river_aggression = false;
                self.shown_high_card = false;
                self.showdown = false;
                self.counted_preflop = false;
                facts.push(Fact::HandStarted {
                    hand: *number,
                    participants: self.witnesses.clone(),
                });
            }
            GameEvent::BlindPosted { amount, .. } => {
                self.street_high = self.street_high.max(*amount)
            }
            GameEvent::CommunityCardsDealt { cards, .. } => {
                self.board.extend(cards);
                self.street_high = 0;
            }
            GameEvent::PlayerActed {
                seat,
                phase,
                action,
                street_total,
                ..
            } => {
                // The public contribution distinguishes an all-in raise from a call.
                let raise = matches!(action, Action::BetTo(_) | Action::RaiseTo(_))
                    || (*action == Action::AllIn && *street_total > self.street_high);
                self.street_high = self.street_high.max(*street_total);
                if *seat != Seat::Human {
                    return Vec::new();
                }
                if *phase == Phase::River && raise {
                    self.river_aggression = true;
                }
                if *phase == Phase::PreFlop && !self.counted_preflop {
                    self.counted_preflop = true;
                    self.opportunities += 1;
                    self.raises += u32::from(raise);
                    if self.opportunities == 20 {
                        facts.push(Fact::Behavior {
                            hand: self.hand,
                            opportunities: self.opportunities,
                            raises: self.raises,
                        });
                        self.opportunities = 0;
                        self.raises = 0;
                    }
                }
            }
            GameEvent::ShowdownStarted { cards } => {
                self.showdown = true;
                if self.river_aggression
                    && let Some(Some(hole)) = cards.first()
                {
                    let mut visible = self.board.clone();
                    visible.extend(hole);
                    self.shown_high_card =
                        evaluate(&visible).is_ok_and(|h| h.category_index() == 1);
                }
            }
            GameEvent::HandCompleted { outcome, .. } => facts.push(Fact::HandFinished {
                hand: self.hand,
                pot: outcome.pot,
                awards: outcome.awards.clone(),
                public_showdown: self.showdown,
                shown_river_aggression: self.shown_high_card,
            }),
            GameEvent::PlayerEliminated { seat } => facts.push(Fact::Eliminated {
                hand: self.hand,
                seat: seat.index(),
            }),
            _ => {}
        }
        facts
            .into_iter()
            .map(|fact| ObservedEvent {
                witnesses: self.witnesses.clone(),
                fact,
            })
            .collect()
    }
}
