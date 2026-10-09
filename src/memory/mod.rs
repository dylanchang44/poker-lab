//! Local, bounded social memory. Poker strategies do not depend on this module.
pub mod models;
pub mod observation;
pub mod repository;
use crate::conversation::{DialogueProvider, TurnRequest};
pub use models::*;
pub use observation::PokerObserver;
pub use repository::Repository;
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub fn database_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("POKER_LAB_DB") {
        return Ok(PathBuf::from(path));
    }
    let root = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .ok_or("no local data directory")?;
    let profile = std::env::var("POKER_LAB_PROFILE").unwrap_or_else(|_| "default".into());
    if profile.is_empty()
        || profile.len() > 40
        || !profile
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("invalid player profile name".into());
    }
    Ok(if profile == "default" {
        root.join("poker-lab/poker_lab.db")
    } else {
        root.join("poker-lab/profiles")
            .join(profile)
            .join("poker_lab.db")
    })
}

#[derive(Clone)]
enum Write {
    Opponent {
        key: String,
        sample: Box<crate::npc::opponent::HandSample>,
        at: i64,
    },
    Begin {
        key: String,
        participants: Vec<NpcId>,
        at: i64,
    },
    Record {
        session: String,
        key: String,
        event: ObservedEvent,
        at: i64,
    },
    End {
        key: String,
        at: i64,
    },
}
enum Command {
    Write(Write),
    Retrieve {
        npc: NpcId,
        topic: String,
        audience: Audience,
        reply: mpsc::Sender<SocialContext>,
    },
    Flush(mpsc::Sender<()>),
}
#[derive(Default, Clone)]
pub struct MemorySnapshot {
    pub opponent: Option<crate::npc::opponent::OpponentModel>,
    pub characters: Vec<CharacterSnapshot>,
    pub persistent: bool,
    pub error: Option<String>,
}
struct ServiceInner {
    sender: mpsc::SyncSender<Command>,
    snapshot: Arc<Mutex<MemorySnapshot>>,
    dropped: AtomicBool,
}
#[derive(Clone)]
pub struct MemoryService(Arc<ServiceInner>);
impl MemoryService {
    pub fn record_opponent(&self, key: String, sample: crate::npc::opponent::HandSample) {
        self.submit(Command::Write(Write::Opponent {
            key,
            sample: Box::new(sample),
            at: now(),
        }));
    }
    /// None means an isolated in-memory store, used by all automated Bevy tests.
    pub fn start(path: Option<PathBuf>) -> Self {
        let (sender, receiver) = mpsc::sync_channel(128);
        let snapshot = Arc::new(Mutex::new(MemorySnapshot::default()));
        let state = Arc::clone(&snapshot);
        if let Err(error) = std::thread::Builder::new()
            .name("poker-memory".into())
            .spawn(move || worker(path, receiver, state))
        {
            snapshot.lock().unwrap().error = Some(format!("memory worker unavailable: {error}"));
        }
        Self(Arc::new(ServiceInner {
            sender,
            snapshot,
            dropped: AtomicBool::new(false),
        }))
    }
    fn submit(&self, command: Command) {
        if self.0.sender.try_send(command).is_err() {
            self.0.dropped.store(true, Ordering::Relaxed);
        }
    }
    pub fn begin(&self, key: String, participants: Vec<NpcId>) {
        self.submit(Command::Write(Write::Begin {
            key,
            participants,
            at: now(),
        }));
    }
    pub fn record(&self, session: String, key: String, event: ObservedEvent) {
        self.submit(Command::Write(Write::Record {
            session,
            key,
            event,
            at: now(),
        }));
    }
    pub fn end(&self, key: String) {
        self.submit(Command::Write(Write::End { key, at: now() }));
    }
    pub fn snapshot(&self) -> MemorySnapshot {
        let mut snapshot = self.0.snapshot.lock().unwrap().clone();
        if self.0.dropped.load(Ordering::Relaxed) {
            snapshot.error = Some("Memory queue overflow/unavailable: some social observations were not saved; poker continues.".into());
        }
        snapshot
    }
    pub fn retrieve(&self, npc: NpcId, topic: String, audience: Audience) -> SocialContext {
        let (reply, result) = mpsc::channel();
        if self
            .0
            .sender
            .try_send(Command::Retrieve {
                npc,
                topic,
                audience,
                reply,
            })
            .is_err()
        {
            return SocialContext::default();
        }
        result
            .recv_timeout(Duration::from_secs(2))
            .unwrap_or_default()
    }
    /// Only used at shutdown or in verification tools; the frame loop never waits.
    pub fn flush(&self) -> bool {
        let (reply, result) = mpsc::channel();
        if self.0.sender.try_send(Command::Flush(reply)).is_err() {
            return false;
        }
        result.recv_timeout(Duration::from_secs(3)).is_ok()
    }
}
fn apply(
    repository: &mut Repository,
    write: &Write,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match write {
        Write::Opponent { key, sample, at } => repository.record_opponent(key, sample, *at),
        Write::Begin {
            key,
            participants,
            at,
        } => repository.begin(key, participants, *at),
        Write::Record {
            session,
            key,
            event,
            at,
        } => repository.record(session, key, event, *at),
        Write::End { key, at } => repository.end(key, *at),
    }
}
fn worker(
    path: Option<PathBuf>,
    receiver: mpsc::Receiver<Command>,
    snapshot: Arc<Mutex<MemorySnapshot>>,
) {
    let mut persistent = path.is_some();
    let mut error = None;
    let mut repository = match Repository::open(path.as_deref()) {
        Ok(r) => r,
        Err(e) => {
            error = Some(format!("Storage unavailable: {e}"));
            persistent = false;
            match Repository::open(None) {
                Ok(r) => r,
                Err(_) => return,
            }
        }
    };
    let mut retry = VecDeque::<Write>::new();
    let mut last_retry = Instant::now();
    loop {
        if persistent && !retry.is_empty() {
            while let Some(write) = retry.front() {
                if apply(&mut repository, write).is_err() {
                    break;
                }
                retry.pop_front();
            }
            if retry.is_empty() {
                error = None;
            }
        }
        if path.is_some() && !persistent && last_retry.elapsed() >= Duration::from_secs(5) {
            last_retry = Instant::now();
            if let Ok(mut restored) = Repository::open(path.as_deref()) {
                let replayed = retry.iter().try_for_each(|w| apply(&mut restored, w));
                if replayed.is_ok() {
                    repository = restored;
                    persistent = true;
                    error = None;
                    retry.clear();
                }
            }
        }
        *snapshot.lock().unwrap() = MemorySnapshot {
            opponent: repository.opponent_model().ok(),
            characters: repository.snapshots().unwrap_or_default(),
            persistent,
            error: error.clone(),
        };
        let command = match receiver.recv_timeout(Duration::from_millis(500)) {
            Ok(c) => c,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        match command {
            Command::Write(write) => {
                if let Err(e) = apply(&mut repository, &write) {
                    error = Some(format!("Save failed: {e}"));
                    // Retain failed writes for retry. A lock/error never reaches poker logic.
                    retry.push_back(write.clone());
                } else if !persistent && path.is_some() {
                    retry.push_back(write);
                }
                if persistent && !retry.is_empty() {
                    while let Some(write) = retry.front() {
                        if apply(&mut repository, write).is_err() {
                            break;
                        }
                        retry.pop_front();
                    }
                    if retry.is_empty() {
                        error = None;
                    }
                }
                // Bounded recovery journal. Keep session beginnings so later writes remain valid.
                if retry.len() > 256 {
                    error = Some(
                        "Recovery journal full; some recent social data could not be saved.".into(),
                    );
                    if let Some(index) =
                        retry.iter().position(|w| !matches!(w, Write::Begin { .. }))
                    {
                        retry.remove(index);
                    } else {
                        retry.pop_front();
                    }
                }
            }
            Command::Retrieve {
                npc,
                topic,
                audience,
                reply,
            } => {
                let _ = reply.send(
                    repository
                        .retrieve(npc, &topic, audience, now())
                        .unwrap_or_default(),
                );
            }
            Command::Flush(reply) => {
                *snapshot.lock().unwrap() = MemorySnapshot {
                    opponent: repository.opponent_model().ok(),
                    characters: repository.snapshots().unwrap_or_default(),
                    persistent,
                    error: error.clone(),
                };
                let _ = reply.send(());
            }
        }
    }
}

pub struct RememberingProvider {
    pub inner: Arc<dyn DialogueProvider>,
    pub memory: MemoryService,
}
impl DialogueProvider for RememberingProvider {
    fn respond(&self, request: &TurnRequest) -> Result<String, String> {
        let mut enriched = request.clone();
        if let Some(npc) = request.speaker.seat().and_then(NpcId::from_seat) {
            let topic = request
                .player_text
                .clone()
                .unwrap_or_else(|| request.context.recent_actions.join(" "));
            enriched.social = self.memory.retrieve(npc, topic, request.audience);
        }
        self.inner.respond(&enriched)
    }
}

#[cfg(test)]
mod tests;
