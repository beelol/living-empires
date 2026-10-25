//! One bounded model worker beside the native simulation. Never blocks a game tick.
use crate::{
    diplomacy::{Diplomacy, FACTION_IDS},
    gemini::{self, GeminiConfig},
};
use anyhow::Result;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub type DiplomacyStatus = Arc<RwLock<Value>>;

pub struct DiplomacyLoop {
    status: DiplomacyStatus,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl DiplomacyLoop {
    pub fn start(
        state: PathBuf,
        env_file: PathBuf,
        activation_file: Option<PathBuf>,
    ) -> Result<Self> {
        Self::start_with(state, env_file, activation_file, gemini::decide)
    }

    pub fn status(&self) -> DiplomacyStatus {
        self.status.clone()
    }

    fn start_with<F>(
        state: PathBuf,
        env_file: PathBuf,
        activation_file: Option<PathBuf>,
        decide: F,
    ) -> Result<Self>
    where
        F: Fn(&GeminiConfig, Value) -> Result<Value> + Send + 'static,
    {
        let status = Arc::new(RwLock::new(
            json!({"phase":"starting","message":"Starting diplomacy"}),
        ));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_status = status.clone();
        let worker_stop = stop.clone();
        let worker = thread::Builder::new().name("empire-diplomacy".into()).spawn(move || {
            if let Err(error) = run_worker(&state, &env_file, activation_file.as_deref(), &worker_status, &worker_stop, decide) {
                if let Ok(mut value) = worker_status.write() {
                    value["phase"] = json!("error");
                    value["message"] = json!(format!("Diplomacy stopped: {error}. Fix the saved state or configuration, then restart the host."));
                }
            }
        })?;
        Ok(Self {
            status,
            stop,
            worker: Some(worker),
        })
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn publish(
    status: &DiplomacyStatus,
    core: &Diplomacy,
    config: Option<&GeminiConfig>,
    phase: &str,
    message: &str,
) {
    let mut world = core.snapshot(now());
    // Keep full evidence on disk; the local page only needs recent turns.
    for field in ["attempts", "decisions"] {
        if let Some(values) = world[field].as_array_mut() {
            let remove = values.len().saturating_sub(24);
            values.drain(..remove);
        }
    }
    if let Ok(mut value) = status.write() {
        *value = json!({"phase":phase,"message":message,"model":config.map(|c| c.model.as_str()),
            "interval_seconds":config.map(|c| c.interval.as_secs()),"max_calls":config.map(|c| c.max_calls),"world":world});
    }
}

fn pause(stop: &AtomicBool, duration: Duration) {
    let until = Instant::now() + duration;
    while !stop.load(Ordering::Relaxed) && Instant::now() < until {
        thread::sleep(
            Duration::from_millis(50).min(until.saturating_duration_since(Instant::now())),
        );
    }
}

fn retry_delay(interval: Duration, failures: u32) -> Duration {
    interval.max(Duration::from_secs(((1u64 << failures.min(5)) * 2).min(60)))
}

fn run_worker<F>(
    state: &Path,
    env_file: &Path,
    activation_file: Option<&Path>,
    status: &DiplomacyStatus,
    stop: &AtomicBool,
    decide: F,
) -> Result<()>
where
    F: Fn(&GeminiConfig, Value) -> Result<Value>,
{
    let mut core = Diplomacy::load(state)?;
    // A saved request without an outcome was interrupted. Resolve it explicitly;
    // it still consumed budget, and a late result must never apply after recovery.
    if let Some(attempts) = core.snapshot(now())["attempts"].as_array() {
        for attempt in attempts.iter().filter(|a| a["finished_at"].is_null()) {
            core.record_failure(
                attempt["actor"].as_str().unwrap_or(""),
                attempt["request_id"].as_str().unwrap_or(""),
                attempt["context"].clone(),
                "Request interrupted by host restart; no proposal was applied",
                now(),
            )?;
        }
    }
    let mut next = Instant::now();
    let mut failures: u32 = 0;
    let mut phase = "running";
    let mut message = "Factions are ready to make decisions.".to_owned();
    while !stop.load(Ordering::Relaxed) {
        let config = match gemini::load_config(env_file) {
            Ok(config) => config,
            Err(error) => {
                publish(
                    status,
                    &core,
                    None,
                    "error",
                    &format!("{error}. Fix .env.local; it will be read again automatically."),
                );
                pause(stop, Duration::from_secs(1));
                continue;
            }
        };
        let waiting = if activation_file.is_some_and(|path| !path.is_file()) {
            Some((
                "waiting_activation",
                "This host is awaiting activation. No model requests are being made.",
            ))
        } else if config.key.is_none() {
            Some((
                "waiting_key",
                "Add GEMINI_API_KEY to .env.local. The Rust host will pick it up automatically.",
            ))
        } else if core.attempts() >= config.max_calls {
            Some(("paused", "The saved API call limit has been reached. Raise DIPLOMACY_MAX_CALLS in .env.local to continue."))
        } else {
            None
        };
        if let Some((phase, message)) = waiting {
            publish(status, &core, Some(&config), phase, message);
            pause(stop, Duration::from_secs(1));
            continue;
        }
        if Instant::now() < next {
            publish(status, &core, Some(&config), phase, &message);
            pause(stop, Duration::from_millis(250));
            continue;
        }
        let actor = FACTION_IDS[core.attempts() as usize % FACTION_IDS.len()];
        let request_id = format!("diplomacy-{}", core.attempts() + 1);
        let observed = now();
        let context = core.context(actor, observed)?;
        let revision = core.revision();
        core.record_attempt(actor, &request_id, context.clone(), &config.model, observed)?;
        publish(
            status,
            &core,
            Some(&config),
            "thinking",
            &format!(
                "{actor} is considering its known contacts. The game continues while it thinks."
            ),
        );
        let started = Instant::now();
        let result = decide(&config, context.clone());
        if stop.load(Ordering::Relaxed) || activation_file.is_some_and(|path| !path.is_file()) {
            core.record_failure(
                actor,
                &request_id,
                context,
                "Host stopped before proposal application",
                now(),
            )?;
            publish(
                status,
                &core,
                Some(&config),
                "stopped",
                "The pending proposal was discarded.",
            );
            break;
        }
        let result = if started.elapsed() > Duration::from_secs(25) {
            Err(anyhow::anyhow!("Decision expired before application"))
        } else {
            result
        };
        match result {
            Ok(proposal) => {
                failures = 0;
                phase = "running";
                message = match core.apply(actor, revision, &request_id, proposal, now()) {
                    Ok(_) => format!("{actor}'s decision was recorded. The next faction will decide shortly."),
                    Err(error) => format!("{actor}'s proposal was rejected: {error}. The next turn uses current observations."),
                };
                next = Instant::now() + config.interval;
            }
            Err(error) => {
                let error = error.to_string();
                core.record_failure(actor, &request_id, context, &error, now())?;
                failures = failures.saturating_add(1);
                let delay = retry_delay(config.interval, failures);
                phase = "retrying";
                message = format!(
                    "{error}. Retrying with fresh observations in {} seconds.",
                    delay.as_secs()
                );
                next = Instant::now() + delay;
            }
        }
        publish(status, &core, Some(&config), phase, &message);
    }
    Ok(())
}

impl Drop for DiplomacyLoop {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::{atomic::AtomicUsize, mpsc},
    };

    fn wait_until(check: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !check() {
            assert!(
                Instant::now() < deadline,
                "diplomacy did not reach expected state"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn config(path: &Path, key: &str, max: u64) {
        fs::write(
            path,
            format!(
                "GEMINI_API_KEY={key}\nDIPLOMACY_INTERVAL_SECONDS=1\nDIPLOMACY_MAX_CALLS={max}\n"
            ),
        )
        .unwrap();
    }
    fn proposal() -> Value {
        json!({"action":"wait","target":"","message":"","summary":"Reviewing known contacts","offer_id":"","offer_version":0,"stance":"neutral"})
    }

    #[test]
    fn late_key_starts_real_worker_and_budget_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let env = dir.path().join(".env.local");
        let state = dir.path().join("diplomacy.json");
        config(&env, "", 1);
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let worker =
            DiplomacyLoop::start_with(state.clone(), env.clone(), None, move |_, context| {
                assert_eq!(context["self"]["id"], "helios");
                count.fetch_add(1, Ordering::Relaxed);
                Ok(proposal())
            })
            .unwrap();
        wait_until(|| worker.status.read().unwrap()["phase"] == "waiting_key");
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        config(&env, "test-only-key", 1);
        wait_until(|| worker.status.read().unwrap()["phase"] == "paused");
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            worker.status.read().unwrap()["world"]["decisions"][0]["accepted"],
            true
        );
        drop(worker);
        let worker = DiplomacyLoop::start_with(state, env, None, |_, _| {
            panic!("budget cannot reset on restart")
        })
        .unwrap();
        wait_until(|| worker.status.read().unwrap()["phase"] == "paused");
    }

    #[test]
    fn pending_model_keeps_status_readable_and_stop_discards_late_proposal() {
        let dir = tempfile::tempdir().unwrap();
        let env = dir.path().join(".env.local");
        let state = dir.path().join("diplomacy.json");
        config(&env, "test-only-key", 2);
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let worker = DiplomacyLoop::start_with(state.clone(), env, None, move |_, _| {
            started_tx.send(()).unwrap();
            finish_rx.recv().unwrap();
            Ok(proposal())
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(worker.status.read().unwrap()["phase"], "thinking");
        worker.stop.store(true, Ordering::Relaxed);
        finish_tx.send(()).unwrap();
        drop(worker);
        let saved = Diplomacy::load(state).unwrap().snapshot(0);
        assert!(saved["decisions"].as_array().unwrap().is_empty());
        assert_eq!(saved["attempt_count"], 1);
    }

    #[test]
    fn inactive_candidate_never_calls_model() {
        let dir = tempfile::tempdir().unwrap();
        let env = dir.path().join(".env.local");
        config(&env, "test-only-key", 2);
        let worker = DiplomacyLoop::start_with(
            dir.path().join("world.json"),
            env,
            Some(dir.path().join("activate")),
            |_, _| panic!("candidate cannot spend"),
        )
        .unwrap();
        wait_until(|| worker.status.read().unwrap()["phase"] == "waiting_activation");
    }

    #[test]
    fn recovery_resolves_interrupted_attempt_without_spending_again() {
        let dir = tempfile::tempdir().unwrap();
        let env = dir.path().join(".env.local");
        let state = dir.path().join("world.json");
        config(&env, "", 2);
        let mut core = Diplomacy::load(&state).unwrap();
        let at = now();
        let context = core.context("helios", at).unwrap();
        core.record_attempt("helios", "diplomacy-1", context, "gemini-4-argon", at)
            .unwrap();
        drop(core);
        let worker = DiplomacyLoop::start_with(state, env, None, |_, _| panic!("no key")).unwrap();
        wait_until(|| worker.status.read().unwrap()["phase"] == "waiting_key");
        let value = worker.status.read().unwrap();
        assert_eq!(value["world"]["attempt_count"], 1);
        assert!(value["world"]["attempts"][0]["error"]
            .as_str()
            .is_some_and(|s| s.contains("interrupted")));
        assert!(value["world"]["decisions"].as_array().unwrap().is_empty());
    }

    #[test]
    fn retry_never_shortens_the_configured_interval() {
        assert_eq!(
            retry_delay(Duration::from_secs(300), 1),
            Duration::from_secs(300)
        );
        assert_eq!(
            retry_delay(Duration::from_secs(10), 5),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn provider_outage_is_recorded_and_next_faction_retries_without_fake_action() {
        let dir = tempfile::tempdir().unwrap();
        let env = dir.path().join(".env.local");
        config(&env, "test-only-key", 2);
        let worker =
            DiplomacyLoop::start_with(dir.path().join("world.json"), env, None, |_, context| {
                if context["self"]["id"] == "helios" {
                    anyhow::bail!("provider unavailable");
                }
                assert_eq!(context["self"]["id"], "tide");
                Ok(proposal())
            })
            .unwrap();
        wait_until(|| worker.status.read().unwrap()["phase"] == "paused");
        let value = worker.status.read().unwrap();
        assert_eq!(
            value["world"]["attempts"][0]["error"],
            "provider unavailable"
        );
        assert_eq!(value["world"]["decisions"].as_array().unwrap().len(), 1);
        assert_eq!(value["world"]["decisions"][0]["actor"], "tide");
    }
}
