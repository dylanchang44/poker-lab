//! Inspect/reset local social data with the game closed. No renderer or LLM.
use poker_lab::memory::{NpcId, Repository, database_path};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let path = database_path()?;
    let mut repository = Repository::open(Some(&path))?;
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] | ["inspect"] => {
            println!("Database: {}", path.display());
            for character in repository.snapshots()? {
                println!("\n{} — {} ({} sessions, {} hands)\n{:?}", character.npc.name(), character.relationship.tier(character.npc), character.sessions, character.hands, character.relationship);
                for memory in character.memories.iter().take(8) {
                    println!("  [{:?}; importance {}] {}", memory.audience, memory.importance, memory.summary);
                }
            }
        }
        [command @ ("reset" | "reset-relationships"), target, "--confirm"] => {
            let npc = if *target == "all" { None } else { Some(NpcId::parse(target).ok_or("unknown NPC; use Ananya, Freya, Yuna or all")?) };
            repository.reset(npc, *command == "reset-relationships")?;
            println!("Reset {command} for {target}. Historical source events/session summaries remain; use a fresh profile for a completely separate history.");
        }
        _ => return Err("Usage: cargo run --example social -- [inspect | reset NPC|all --confirm | reset-relationships NPC|all --confirm]. Close the game first.".into()),
    }
    Ok(())
}
