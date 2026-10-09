use super::models::*;
use crate::poker::Seat;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::{path::Path, time::Duration};

pub const SCHEMA_VERSION: i64 = 4;
pub const MEMORY_LIMIT: usize = 96;
pub struct Repository {
    connection: Connection,
}
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl Repository {
    pub fn open(path: Option<&Path>) -> Result<Self> {
        let connection = if let Some(path) = path {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent)?;
            }
            Connection::open(path)?
        } else {
            Connection::open_in_memory()?
        };
        connection.busy_timeout(Duration::from_millis(150))?;
        connection.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )?;
        let mut repository = Self { connection };
        repository.migrate()?;
        Ok(repository)
    }
    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .connection
            .query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }
    fn migrate(&mut self) -> Result<()> {
        let version = self.schema_version()?;
        if version > SCHEMA_VERSION {
            return Err("memory database belongs to a newer Poker Lab version".into());
        }
        let tx = self.connection.transaction()?;
        if version < 1 {
            tx.execute_batch("CREATE TABLE npc_profiles(npc_id TEXT PRIMARY KEY, relationship TEXT NOT NULL, sessions INTEGER NOT NULL DEFAULT 0, hands INTEGER NOT NULL DEFAULT 0, last_interaction INTEGER);
                CREATE TABLE sessions(id INTEGER PRIMARY KEY, session_key TEXT NOT NULL UNIQUE, started_at INTEGER NOT NULL, ended_at INTEGER, participants TEXT NOT NULL, hands INTEGER NOT NULL DEFAULT 0, summary TEXT NOT NULL DEFAULT '', initial_relationships TEXT NOT NULL);
                CREATE TABLE events(id INTEGER PRIMARY KEY, session_id INTEGER NOT NULL REFERENCES sessions(id), event_key TEXT NOT NULL, observed TEXT NOT NULL, changes TEXT NOT NULL, created_at INTEGER NOT NULL, UNIQUE(session_id,event_key));
                PRAGMA user_version=1;")?;
        }
        if version < 2 {
            tx.execute_batch("CREATE TABLE memories(id INTEGER PRIMARY KEY, npc_id TEXT NOT NULL REFERENCES npc_profiles(npc_id), memory_key TEXT NOT NULL, kind TEXT NOT NULL, summary TEXT NOT NULL, importance INTEGER NOT NULL CHECK(importance BETWEEN 0 AND 100), occurrences INTEGER NOT NULL DEFAULT 1, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, last_recalled_at INTEGER, recall_count INTEGER NOT NULL DEFAULT 0, source_event_id INTEGER NOT NULL REFERENCES events(id), last_source_event_id INTEGER NOT NULL REFERENCES events(id), UNIQUE(npc_id,memory_key));
                CREATE INDEX memory_owner ON memories(npc_id,importance DESC,updated_at DESC);
                CREATE INDEX events_session ON events(session_id);
                PRAGMA user_version=2;")?;
        }
        if version < 3 {
            tx.execute_batch(
                "ALTER TABLE memories ADD COLUMN audience TEXT NOT NULL DEFAULT '\"Public\"';
                ALTER TABLE sessions ADD COLUMN statistics TEXT NOT NULL DEFAULT '{}';
                PRAGMA user_version=3;",
            )?;
        }
        if version < 4 {
            tx.execute_batch("CREATE TABLE opponent_model(id INTEGER PRIMARY KEY CHECK(id=1), model TEXT NOT NULL, updated_at INTEGER NOT NULL);
                CREATE TABLE opponent_commits(id INTEGER PRIMARY KEY, event_key TEXT NOT NULL UNIQUE);
                PRAGMA user_version=4;")?;
        }
        for npc in NpcId::ALL {
            tx.execute(
                "INSERT OR IGNORE INTO npc_profiles(npc_id,relationship) VALUES(?1,?2)",
                params![npc.id(), serde_json::to_string(&Relationship::default())?],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn begin(&mut self, key: &str, participants: &[NpcId], at: i64) -> Result<()> {
        let initial: Vec<_> = self
            .snapshots()?
            .into_iter()
            .map(|s| (s.npc, s.relationship))
            .collect();
        let tx = self.connection.transaction()?;
        let inserted = tx.execute("INSERT OR IGNORE INTO sessions(session_key,started_at,participants,initial_relationships) VALUES(?1,?2,?3,?4)", params![key,at,serde_json::to_string(participants)?,serde_json::to_string(&initial)?])?;
        if inserted == 0 {
            return Ok(());
        }
        let session = tx.last_insert_rowid();
        let observed = ObservedEvent {
            witnesses: participants.to_vec(),
            fact: Fact::SessionStarted,
        };
        tx.execute("INSERT INTO events(session_id,event_key,observed,changes,created_at) VALUES(?1,'session-start',?2,'{}',?3)", params![session,serde_json::to_string(&observed)?,at])?;
        let event = tx.last_insert_rowid();
        let mut changes = Vec::new();
        for npc in participants {
            let mut relation = relationship(&tx, *npc)?;
            let delta = Relationship {
                familiarity: 5,
                ..Default::default()
            };
            relation.apply(&delta);
            changes.push((*npc, delta));
            tx.execute("UPDATE npc_profiles SET sessions=sessions+1,relationship=?2,last_interaction=?3 WHERE npc_id=?1", params![npc.id(),serde_json::to_string(&relation)?,at])?;
            let sessions: i64 = tx.query_row(
                "SELECT sessions FROM npc_profiles WHERE npc_id=?1",
                [npc.id()],
                |r| r.get(0),
            )?;
            let candidate = MemoryCandidate {
                key: "milestone:sessions".into(),
                kind: MemoryType::RelationshipMilestone,
                summary: format!("The player and I have shared {sessions} table sessions."),
                importance: if sessions == 1 { 80 } else { 55 },
                delta: Relationship::default(),
                audience: Audience::Public,
            };
            save_memory(&tx, *npc, &candidate, event, at)?;
        }
        tx.execute(
            "UPDATE events SET changes=?2 WHERE id=?1",
            params![event, serde_json::to_string(&changes)?],
        )?;
        update_summary(&tx, session)?;
        tx.commit()?;
        Ok(())
    }
    pub fn record(
        &mut self,
        session_key: &str,
        key: &str,
        observed: &ObservedEvent,
        at: i64,
    ) -> Result<()> {
        let tx = self.connection.transaction()?;
        let session: i64 = tx.query_row(
            "SELECT id FROM sessions WHERE session_key=?1",
            [session_key],
            |r| r.get(0),
        )?;
        let inserted = tx.execute("INSERT OR IGNORE INTO events(session_id,event_key,observed,changes,created_at) VALUES(?1,?2,?3,'{}',?4)", params![session,key,serde_json::to_string(observed)?,at])?;
        if inserted == 0 {
            return Ok(());
        }
        let event = tx.last_insert_rowid();
        let mut changes = Vec::new();
        let raw_stats: String = tx.query_row(
            "SELECT statistics FROM sessions WHERE id=?1",
            [session],
            |r| r.get(0),
        )?;
        let mut stats: SessionStatistics = serde_json::from_str(&raw_stats)?;
        match &observed.fact {
            Fact::HandFinished {
                pot,
                awards,
                shown_river_aggression,
                ..
            } => {
                stats.largest_pot = stats.largest_pot.max(*pot);
                stats.shown_aggression += u64::from(*shown_river_aggression);
                for (i, n) in awards.iter().take(4).enumerate() {
                    stats.awards[i] += u64::from(*n);
                }
            }
            Fact::Eliminated { seat, .. } => {
                if !stats.eliminations.contains(seat) {
                    stats.eliminations.push(*seat);
                }
            }
            Fact::Conversation { .. } => stats.conversations += 1,
            _ => {}
        }
        tx.execute(
            "UPDATE sessions SET statistics=?2 WHERE id=?1",
            params![session, serde_json::to_string(&stats)?],
        )?;
        if let Fact::HandStarted { participants, .. } = &observed.fact {
            tx.execute("UPDATE sessions SET hands=hands+1 WHERE id=?1", [session])?;
            for npc in participants
                .iter()
                .filter(|n| observed.witnesses.contains(n))
            {
                let mut relation = relationship(&tx, *npc)?;
                let delta = Relationship {
                    familiarity: 1,
                    ..Default::default()
                };
                relation.apply(&delta);
                tx.execute(
                    "UPDATE npc_profiles SET hands=hands+1,relationship=?2 WHERE npc_id=?1",
                    params![npc.id(), serde_json::to_string(&relation)?],
                )?;
                changes.push((*npc, delta));
                let hands: i64 = tx.query_row(
                    "SELECT hands FROM npc_profiles WHERE npc_id=?1",
                    [npc.id()],
                    |r| r.get(0),
                )?;
                if hands % 100 == 0 {
                    save_memory(
                        &tx,
                        *npc,
                        &MemoryCandidate {
                            key: "milestone:hands".into(),
                            kind: MemoryType::RelationshipMilestone,
                            summary: format!("The player and I have shared {hands} hands."),
                            importance: 80,
                            delta: Relationship::default(),
                            audience: Audience::Public,
                        },
                        event,
                        at,
                    )?;
                }
            }
        }
        for npc in NpcId::ALL {
            if let Some(mut candidate) = derive(npc, observed) {
                if candidate.kind == MemoryType::PokerEvent {
                    candidate.key = format!("{session}:{}", candidate.key);
                }
                // Near-identical repeated quotes consolidate without farming relationship points.
                let exists: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM memories WHERE npc_id=?1 AND memory_key=?2)",
                    params![npc.id(), candidate.key],
                    |r| r.get(0),
                )?;
                if !exists
                    || candidate.kind == MemoryType::BehavioralObservation
                    || candidate.delta.warmth < 0
                    || candidate.delta.trust < 0
                {
                    let mut relation = relationship(&tx, npc)?;
                    relation.apply(&candidate.delta);
                    tx.execute("UPDATE npc_profiles SET relationship=?2,last_interaction=?3 WHERE npc_id=?1", params![npc.id(),serde_json::to_string(&relation)?,at])?;
                    changes.push((npc, candidate.delta.clone()));
                }
                save_memory(&tx, npc, &candidate, event, at)?;
            }
        }
        tx.execute(
            "UPDATE events SET changes=?2 WHERE id=?1",
            params![event, serde_json::to_string(&changes)?],
        )?;
        trim_memories(&tx)?;
        update_summary(&tx, session)?;
        prune_sources(&tx)?;
        tx.commit()?;
        Ok(())
    }
    pub fn end(&mut self, key: &str, at: i64) -> Result<()> {
        let tx = self.connection.transaction()?;
        let session: Option<i64> = tx
            .query_row("SELECT id FROM sessions WHERE session_key=?1", [key], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(session) = session {
            tx.execute(
                "UPDATE sessions SET ended_at=COALESCE(ended_at,?2) WHERE id=?1",
                params![session, at],
            )?;
            update_summary(&tx, session)?;
        }
        trim_memories(&tx)?;
        // Keep last 512 factual records plus the first/latest sources of retained memories.
        prune_sources(&tx)?;
        tx.commit()?;
        Ok(())
    }
    pub fn snapshots(&self) -> Result<Vec<CharacterSnapshot>> {
        let mut result = Vec::new();
        for npc in NpcId::ALL {
            let (relation, sessions, hands): (String, i64, i64) = self.connection.query_row(
                "SELECT relationship,sessions,hands FROM npc_profiles WHERE npc_id=?1",
                [npc.id()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            result.push(CharacterSnapshot {
                npc,
                relationship: serde_json::from_str(&relation)?,
                sessions,
                hands,
                memories: self.memories(npc)?,
            });
        }
        Ok(result)
    }
    pub fn memories(&self, npc: NpcId) -> Result<Vec<Memory>> {
        let mut query = self.connection.prepare("SELECT id,kind,summary,importance,occurrences,created_at,last_recalled_at,recall_count,source_event_id,audience FROM memories WHERE npc_id=?1 ORDER BY importance DESC,updated_at DESC,id DESC LIMIT 96")?;
        let raw = query.query_map([npc.id()], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i32>(3)?,
                r.get::<_, i32>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, i32>(7)?,
                r.get::<_, i64>(8)?,
                r.get::<_, String>(9)?,
            ))
        })?;
        let mut memories = Vec::new();
        for row in raw {
            let (
                id,
                kind,
                summary,
                importance,
                occurrences,
                created_at,
                last_recalled_at,
                recall_count,
                source_event_id,
                audience,
            ) = row?;
            memories.push(Memory {
                id,
                npc,
                kind: serde_json::from_str(&kind)?,
                summary,
                importance,
                occurrences,
                created_at,
                last_recalled_at,
                recall_count,
                source_event_id,
                audience: serde_json::from_str(&audience)?,
            });
        }
        Ok(memories)
    }
    pub fn retrieve(
        &mut self,
        npc: NpcId,
        topic: &str,
        audience: Audience,
        at: i64,
    ) -> Result<SocialContext> {
        let snapshot = self
            .snapshots()?
            .into_iter()
            .find(|s| s.npc == npc)
            .ok_or("unknown character")?;
        let words: Vec<_> = topic
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.chars().count() >= 4)
            .take(12)
            .map(str::to_lowercase)
            .collect();
        let mut candidates: Vec<_> = snapshot
            .memories
            .iter()
            .filter_map(|m| {
                if m.audience != Audience::Public
                    && (audience == Audience::Public || !audience.permits(npc))
                {
                    return None;
                }
                let lower = m.summary.to_lowercase();
                let relevance = words.iter().filter(|w| lower.contains(w.as_str())).count() as i64;
                let recent = m.last_recalled_at.is_some_and(|last| at - last < 120);
                if recent {
                    return None;
                }
                let score = i64::from(m.importance)
                    + relevance * 40
                    + (10 - (at - m.created_at).max(0) / 86400).max(0)
                    - i64::from(m.recall_count.min(10)) * 3
                    + if m.kind == MemoryType::SocialInteraction {
                        i64::from(snapshot.relationship.familiarity.min(100)) / 20
                    } else {
                        0
                    };
                Some((score, m))
            })
            .collect();
        candidates.sort_by_key(|(score, m)| (std::cmp::Reverse(*score), std::cmp::Reverse(m.id)));
        let selected: Vec<_> = candidates.into_iter().take(3).map(|(_, m)| m).collect();
        let tx = self.connection.transaction()?;
        for m in &selected {
            tx.execute(
                "UPDATE memories SET last_recalled_at=?2,recall_count=recall_count+1 WHERE id=?1",
                params![m.id, at],
            )?;
        }
        tx.commit()?;
        Ok(SocialContext {
            tier: snapshot.relationship.tier(npc).into(),
            relationship: Some(snapshot.relationship),
            memories: selected
                .iter()
                .map(|m| m.summary.chars().take(240).collect())
                .collect(),
        })
    }
    pub fn reset(&mut self, npc: Option<NpcId>, relationships_only: bool) -> Result<()> {
        let tx = self.connection.transaction()?;
        for owner in NpcId::ALL
            .into_iter()
            .filter(|n| npc.is_none_or(|selected| selected == *n))
        {
            tx.execute(
                "UPDATE npc_profiles SET relationship=?2 WHERE npc_id=?1",
                params![owner.id(), serde_json::to_string(&Relationship::default())?],
            )?;
            if !relationships_only {
                tx.execute("DELETE FROM memories WHERE npc_id=?1", [owner.id()])?;
                tx.execute("UPDATE npc_profiles SET sessions=0,hands=0,last_interaction=NULL WHERE npc_id=?1", [owner.id()])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn opponent_model(&self) -> Result<crate::npc::opponent::OpponentModel> {
        let text: Option<String> = self
            .connection
            .query_row("SELECT model FROM opponent_model WHERE id=1", [], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(match text {
            Some(text) => serde_json::from_str(&text)?,
            None => Default::default(),
        })
    }
    pub fn record_opponent(
        &mut self,
        key: &str,
        sample: &crate::npc::opponent::HandSample,
        at: i64,
    ) -> Result<()> {
        let tx = self.connection.transaction()?;
        if tx.execute(
            "INSERT OR IGNORE INTO opponent_commits(event_key) VALUES(?1)",
            [key],
        )? == 0
        {
            return Ok(());
        }
        // Read after acquiring the write transaction: two application instances
        // must not overwrite each other's completed-hand totals.
        let text: Option<String> = tx
            .query_row("SELECT model FROM opponent_model WHERE id=1", [], |r| {
                r.get(0)
            })
            .optional()?;
        let mut model: crate::npc::opponent::OpponentModel = match text {
            Some(text) => serde_json::from_str(&text)?,
            None => Default::default(),
        };
        model.record(sample.clone());
        tx.execute("INSERT INTO opponent_model(id,model,updated_at) VALUES(1,?1,?2) ON CONFLICT(id) DO UPDATE SET model=excluded.model,updated_at=excluded.updated_at",params![serde_json::to_string(&model)?,at])?;
        // More than the bounded 256-write recovery journal; retry replay is idempotent.
        tx.execute("DELETE FROM opponent_commits WHERE id NOT IN (SELECT id FROM opponent_commits ORDER BY id DESC LIMIT 1024)",[])?;
        tx.commit()?;
        Ok(())
    }
    pub fn reset_opponent(&mut self) -> Result<()> {
        let tx = self.connection.transaction()?;
        tx.execute("DELETE FROM opponent_model", [])?;
        tx.execute("DELETE FROM opponent_commits", [])?;
        tx.commit()?;
        Ok(())
    }
    pub fn reset_memories_only(&mut self, npc: Option<NpcId>) -> Result<()> {
        let tx = self.connection.transaction()?;
        for owner in NpcId::ALL
            .into_iter()
            .filter(|n| npc.is_none_or(|selected| selected == *n))
        {
            tx.execute("DELETE FROM memories WHERE npc_id=?1", [owner.id()])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn summary(&self, key: &str) -> Result<String> {
        Ok(self.connection.query_row(
            "SELECT summary FROM sessions WHERE session_key=?1",
            [key],
            |r| r.get(0),
        )?)
    }
}
fn relationship(tx: &Transaction<'_>, npc: NpcId) -> Result<Relationship> {
    let json: String = tx.query_row(
        "SELECT relationship FROM npc_profiles WHERE npc_id=?1",
        [npc.id()],
        |r| r.get(0),
    )?;
    Ok(serde_json::from_str(&json)?)
}
fn save_memory(
    tx: &Transaction<'_>,
    npc: NpcId,
    candidate: &MemoryCandidate,
    event: i64,
    at: i64,
) -> Result<()> {
    tx.execute("INSERT INTO memories(npc_id,memory_key,kind,summary,importance,created_at,updated_at,source_event_id,last_source_event_id,audience) VALUES(?1,?2,?3,?4,?5,?6,?6,?7,?7,?8) ON CONFLICT(npc_id,memory_key) DO UPDATE SET summary=excluded.summary,importance=MAX(importance,excluded.importance),occurrences=occurrences+1,updated_at=excluded.updated_at,last_source_event_id=excluded.last_source_event_id", params![npc.id(),candidate.key,serde_json::to_string(&candidate.kind)?,candidate.summary,candidate.importance.clamp(0,100),at,event,serde_json::to_string(&candidate.audience)?])?;
    Ok(())
}
fn trim_memories(tx: &Transaction<'_>) -> Result<()> {
    for npc in NpcId::ALL {
        tx.execute("DELETE FROM memories WHERE npc_id=?1 AND id NOT IN(SELECT id FROM memories WHERE npc_id=?1 ORDER BY importance DESC,updated_at DESC,id DESC LIMIT ?2)", params![npc.id(),MEMORY_LIMIT as i64])?;
    }
    Ok(())
}
fn prune_sources(tx: &Transaction<'_>) -> Result<()> {
    tx.execute("DELETE FROM events WHERE id NOT IN (SELECT id FROM events ORDER BY id DESC LIMIT 512) AND id NOT IN(SELECT source_event_id FROM memories UNION SELECT last_source_event_id FROM memories)", [])?;
    tx.execute("DELETE FROM sessions WHERE id NOT IN(SELECT id FROM sessions ORDER BY id DESC LIMIT 128) AND id NOT IN(SELECT session_id FROM events)", [])?;
    Ok(())
}
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct SessionStatistics {
    largest_pot: u32,
    awards: [u64; 4],
    eliminations: Vec<usize>,
    conversations: u64,
    shown_aggression: u64,
}
fn update_summary(tx: &Transaction<'_>, session: i64) -> Result<()> {
    let (hands, initial, raw_stats): (i64, String, String) = tx.query_row(
        "SELECT hands,initial_relationships,statistics FROM sessions WHERE id=?1",
        [session],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let stats: SessionStatistics = serde_json::from_str(&raw_stats)?;
    let memories: i64 = tx.query_row("SELECT COUNT(*) FROM memories WHERE source_event_id IN(SELECT id FROM events WHERE session_id=?1)", [session], |r| r.get(0))?;
    let initial: Vec<(NpcId, Relationship)> = serde_json::from_str(&initial)?;
    let mut changes = Vec::new();
    for (npc, before) in initial {
        let after = relationship(tx, npc)?;
        changes.push(format!(
            "{} familiarity {:+}, warmth {:+}, respect {:+}, tension {:+}, trust {:+}",
            npc.name(),
            after.familiarity - before.familiarity,
            after.warmth - before.warmth,
            after.respect - before.respect,
            after.tension - before.tension,
            after.trust - before.trust
        ));
    }
    let eliminated: Vec<_> = stats
        .eliminations
        .iter()
        .filter_map(|i| Seat::ALL.get(*i))
        .map(|s| crate::npc::profiles::display_name(*s))
        .collect();
    let summary = format!(
        "{hands} hands; largest pot {}; awards by seat {:?}; eliminations {}; {} notable conversation records; {} public high-card river aggression observations (intent unknown); {memories} retained new memories. {}",
        stats.largest_pot,
        stats.awards,
        eliminated.join(", "),
        stats.conversations,
        stats.shown_aggression,
        changes.join("; ")
    );
    tx.execute(
        "UPDATE sessions SET summary=?2 WHERE id=?1",
        params![session, summary],
    )?;
    Ok(())
}
