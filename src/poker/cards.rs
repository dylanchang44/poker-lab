use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Rank {
    Two = 2,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

impl Rank {
    pub const ALL: [Self; 13] = [
        Self::Two,
        Self::Three,
        Self::Four,
        Self::Five,
        Self::Six,
        Self::Seven,
        Self::Eight,
        Self::Nine,
        Self::Ten,
        Self::Jack,
        Self::Queen,
        Self::King,
        Self::Ace,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    pub const ALL: [Self; 4] = [Self::Clubs, Self::Diamonds, Self::Hearts, Self::Spades];
    pub fn is_red(self) -> bool {
        matches!(self, Self::Diamonds | Self::Hearts)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rank = match self.rank {
            Rank::Ten => "T".into(),
            Rank::Jack => "J".into(),
            Rank::Queen => "Q".into(),
            Rank::King => "K".into(),
            Rank::Ace => "A".into(),
            other => (other as u8).to_string(),
        };
        let suit = match self.suit {
            Suit::Clubs => "c",
            Suit::Diamonds => "d",
            Suit::Hearts => "h",
            Suit::Spades => "s",
        };
        write!(f, "{rank}{suit}")
    }
}

impl std::str::FromStr for Card {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes = value.as_bytes();
        if bytes.len() != 2 {
            return Err("Use a rank and suit, e.g. As or Td");
        }
        let rank = "23456789TJQKA"
            .bytes()
            .position(|b| b == bytes[0].to_ascii_uppercase())
            .map(|i| Rank::ALL[i])
            .ok_or("Invalid rank")?;
        let suit = match bytes[1].to_ascii_lowercase() {
            b'c' => Suit::Clubs,
            b'd' => Suit::Diamonds,
            b'h' => Suit::Hearts,
            b's' => Suit::Spades,
            _ => return Err("Invalid suit"),
        };
        Ok(Self { rank, suit })
    }
}
