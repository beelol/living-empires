//! Single host-owned event clock. No worker, model call or client can advance it.
use anyhow::Result;
use argon_dev::events::{EventSnapshot, EventWorld};
use std::{
    path::Path,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

pub struct EventHost {
    pub world: EventWorld,
    last_tick: Instant,
    public: Arc<RwLock<EventSnapshot>>,
}

impl EventHost {
    pub fn load(path: impl AsRef<Path>, seed: u64, center: [f64; 2]) -> Result<Self> {
        let mut world = EventWorld::load(path, seed)?;
        let snapshot = world.snapshot();
        if snapshot.now == 0 && snapshot.events.is_empty() {
            world.set_center(center)?;
        }
        let public = Arc::new(RwLock::new(world.snapshot()));
        Ok(Self {
            world,
            last_tick: Instant::now(),
            public,
        })
    }

    /// An inactive deployment candidate cannot create events. Downtime is paused.
    /// Long host stalls advance at most five seconds per frame, without losing
    /// the remaining elapsed duration or blocking a frame on unbounded replay.
    pub fn update(&mut self, active: bool) -> Result<()> {
        if !active {
            self.last_tick = Instant::now();
            return Ok(());
        }
        let seconds = self.last_tick.elapsed().as_secs().min(5);
        if seconds > 0 {
            self.world.advance(seconds)?;
            self.last_tick += Duration::from_secs(seconds);
            if let Ok(mut public) = self.public.write() {
                *public = self.world.snapshot();
            }
        }
        Ok(())
    }

    pub fn snapshot(&self) -> EventSnapshot {
        self.world.snapshot()
    }

    /// Read-only public broadcast input for the existing sole diplomacy/GM
    /// scheduler. Contains no operator prompts or private observations.
    pub fn public_feed(&self) -> Arc<RwLock<EventSnapshot>> {
        self.public.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inactive_deployment_does_not_advance_events() {
        let dir = tempfile::tempdir().unwrap();
        let mut host = EventHost::load(dir.path().join("events.json"), 8, [520.0, 300.0]).unwrap();
        host.last_tick = Instant::now() - Duration::from_secs(40);
        host.update(false).unwrap();
        assert_eq!(host.snapshot().now, 0);
        let feed = host.public_feed();
        assert_eq!(feed.read().unwrap().now, 0);
        host.last_tick = Instant::now() - Duration::from_secs(3);
        host.update(true).unwrap();
        assert_eq!(host.snapshot().now, 3);
        assert_eq!(feed.read().unwrap().now, 3);
    }
}
