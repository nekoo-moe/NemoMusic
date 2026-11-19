//! The JavaScript side: the `yt-dlp` EJS solver scripts in an embedded QuickJS.
//!
//! The scripts are `yt.solver.lib.min.js` (meriyah and astring, a JavaScript parser and printer)
//! and `yt.solver.core.min.js` (version 0.8.0, Unlicense, from
//! `github.com/yt-dlp/ejs`). The way to run them is the one `yt-dlp` uses with the QuickJS
//! command (`yt_dlp/extractor/youtube/jsc/_builtin/ejs.py`): evaluate `lib`, copy its exports
//! to the global object, evaluate `core`, call `jsc(input)`.
//!
//! One worker thread (big stack) owns the engine. Every job has a time limit (an interrupt
//! handler stops the script) and the engine has a memory limit; an engine that hit a limit is
//! thrown away and rebuilt for the next job. The engine has no file, network or module access:
//! nothing but the scripts and the data of the job is in it. Errors carry fixed texts, never
//! the script or a challenge.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use rquickjs::{Context, Runtime};
use serde_json::{Value, json};

/// The EJS core script.
const CORE: &str = include_str!("../../assets/ejs/yt.solver.core.min.js");
/// The EJS library script (parser and printer).
const LIB: &str = include_str!("../../assets/ejs/yt.solver.lib.min.js");

/// Longest one solve may run (a whole player script is parsed the first time).
pub const SOLVE_TIMEOUT: Duration = Duration::from_secs(40);
/// Most memory the engine may use, in bytes.
pub const MEMORY_LIMIT: usize = 512 * 1024 * 1024;
/// Stack the script may use, in bytes (the parser recurses deeply on a large script).
const JS_STACK: usize = 16 * 1024 * 1024;
/// Stack of the worker thread: more than the script's, for the engine's own frames.
const THREAD_STACK: usize = 32 * 1024 * 1024;

/// What failed. The texts are fixed: no script, challenge or answer is in them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SolveError {
    /// The worker thread or the engine could not be started.
    #[error("the JavaScript engine could not start")]
    Start,
    /// The solve took longer than its time limit.
    #[error("the JavaScript solver took too long")]
    Timeout,
    /// The script failed or ran out of memory.
    #[error("the JavaScript solver failed")]
    Script,
    /// The solver found no function of this kind in the player script, or it failed on the
    /// challenges.
    #[error("the solver could not solve the {0} challenges")]
    NoFunction(&'static str),
    /// The worker is gone.
    #[error("the JavaScript worker has stopped")]
    Stopped,
}

/// The challenges of one answer: `n` values and signatures, each once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Challenges {
    /// Values of the `n` parameter of stream addresses.
    pub n: Vec<String>,
    /// Encrypted signatures (`s` of a `signatureCipher`).
    pub sig: Vec<String>,
}

/// The solved challenges, and what the solver made of the player script.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Solved {
    /// `n` challenge to solved value.
    pub n: HashMap<String, String>,
    /// Encrypted signature to signature.
    pub sig: HashMap<String, String>,
    /// The player script after the solver's preprocessing, when it was given the whole script.
    /// Much smaller, and the next solve with it skips the parse.
    pub preprocessed: Option<String>,
}

/// What the solver is given of the player. The `id` names the player: the engine keeps the
/// functions it built from the last one, so a solve for the same id reads no script at all.
#[derive(Debug, Clone, Copy)]
pub enum Script<'a> {
    /// The player script as YouTube serves it (parsed once, about 3 s to 20 s).
    Whole {
        /// The player's id.
        id: &'a str,
        /// The script.
        text: &'a str,
    },
    /// What an earlier solve returned in [`Solved::preprocessed`] (compiled once, about 1 s).
    Preprocessed {
        /// The player's id.
        id: &'a str,
        /// The preprocessed script.
        text: &'a str,
    },
}

struct Job {
    kind: &'static str,
    id: String,
    script: String,
    requests: String,
    limit: Duration,
    reply: Sender<Result<Solved, SolveError>>,
}

/// Solves challenges. [`JsSolver`] is the real one; tests give a fake.
pub trait ChallengeSolver: Send + Sync {
    /// Solve `challenges` with `script`, within `limit`. Blocks: call it from a blocking thread.
    ///
    /// # Errors
    /// See [`SolveError`].
    fn solve(
        &self,
        script: Script<'_>,
        challenges: &Challenges,
        limit: Duration,
    ) -> Result<Solved, SolveError>;
}

impl ChallengeSolver for JsSolver {
    fn solve(
        &self,
        script: Script<'_>,
        challenges: &Challenges,
        limit: Duration,
    ) -> Result<Solved, SolveError> {
        Self::solve(self, script, challenges, limit)
    }
}

/// Handle to the worker thread. Cheap to clone; the thread ends when the last clone is dropped.
#[derive(Debug, Clone)]
pub struct JsSolver {
    jobs: Sender<Job>,
}

impl JsSolver {
    /// Start the worker thread. The engine itself is built on the first job.
    ///
    /// # Errors
    /// [`SolveError::Start`] when the operating system refuses the thread.
    pub fn start() -> Result<Self, SolveError> {
        let (jobs, inbox) = channel::<Job>();
        std::thread::Builder::new()
            .name("nemo-js".into())
            .stack_size(THREAD_STACK)
            .spawn(move || worker(&inbox))
            .map_err(|_| SolveError::Start)?;
        Ok(Self { jobs })
    }

    /// Solve `challenges` with `script`. Blocks until done: call it from a blocking thread. A
    /// caller that gives up does not stop the job; its work is not wasted, because a second
    /// call for the same player is cheap once its preprocessed script is kept.
    ///
    /// # Errors
    /// See [`SolveError`].
    pub fn solve(
        &self,
        script: Script<'_>,
        challenges: &Challenges,
        limit: Duration,
    ) -> Result<Solved, SolveError> {
        let (kind, id, text) = match script {
            Script::Whole { id, text } => ("player", id, text),
            Script::Preprocessed { id, text } => ("preprocessed", id, text),
        };
        let requests = json!({ "n": challenges.n, "sig": challenges.sig }).to_string();
        let (reply, answer) = channel();
        self.jobs
            .send(Job {
                kind,
                id: id.to_owned(),
                script: text.to_owned(),
                requests,
                limit,
                reply,
            })
            .map_err(|_| SolveError::Stopped)?;
        answer.recv().map_err(|_| SolveError::Stopped)?
    }
}

/// The engine: a runtime with the two scripts loaded, and the deadline its interrupt handler
/// reads.
pub(super) struct Engine {
    context: Context,
    deadline: Arc<Mutex<Instant>>,
    /// The player whose solver functions are in the engine now.
    loaded: Option<String>,
}

/// Builds the solver functions of a preprocessed player (what `core` does for every call) and
/// keeps them, so the script is compiled once per player.
const LOAD: &str = "globalThis.__solvers = (function () { \
    const result = { n: null, sig: null }; Function('_result', __script)(result); return result; \
})(); globalThis.__script = undefined;";

/// Runs the kept functions on the challenges; a kind with no function, or one that throws,
/// answers `null`.
const RUN: &str = "(function (requests) { const out = {}; \
    for (const kind of ['n', 'sig']) { \
        const solve = globalThis.__solvers && globalThis.__solvers[kind]; \
        if (!solve) { out[kind] = null; continue; } \
        try { const map = {}; for (const c of requests[kind]) { map[c] = solve(c); } out[kind] = map; } \
        catch (e) { out[kind] = null; } \
    } \
    return JSON.stringify(out); \
})(JSON.parse(__requests))";

impl Engine {
    fn new() -> Result<Self, SolveError> {
        Self::with_memory(MEMORY_LIMIT)
    }

    /// An engine that may use `memory` bytes.
    pub(super) fn with_memory(memory: usize) -> Result<Self, SolveError> {
        let runtime = Runtime::new().map_err(|_| SolveError::Start)?;
        runtime.set_memory_limit(memory);
        runtime.set_max_stack_size(JS_STACK);
        let deadline = Arc::new(Mutex::new(Instant::now() + SOLVE_TIMEOUT));
        let watched = Arc::clone(&deadline);
        runtime.set_interrupt_handler(Some(Box::new(move || {
            Instant::now() > *watched.lock().unwrap_or_else(PoisonError::into_inner)
        })));
        let context = Context::full(&runtime).map_err(|_| SolveError::Start)?;
        context
            .with(|ctx| {
                ctx.eval::<(), _>(LIB)?;
                ctx.eval::<(), _>("Object.assign(globalThis, lib);")?;
                ctx.eval::<(), _>(CORE)
            })
            .map_err(|_| SolveError::Start)?;
        Ok(Self {
            context,
            deadline,
            loaded: None,
        })
    }

    /// Run `code` with the strings `globals` set, under the job's time limit.
    pub(super) fn eval(
        &self,
        limit: Duration,
        globals: &[(&str, &str)],
        code: &str,
    ) -> Result<String, SolveError> {
        *self.deadline.lock().unwrap_or_else(PoisonError::into_inner) = Instant::now() + limit;
        let text: Result<String, rquickjs::Error> = self.context.with(|ctx| {
            let object = ctx.globals();
            for (name, value) in globals {
                object.set(*name, *value)?;
            }
            ctx.eval::<String, _>(code)
        });
        let late = Instant::now() > *self.deadline.lock().unwrap_or_else(PoisonError::into_inner);
        text.map_err(|_| {
            if late {
                SolveError::Timeout
            } else {
                SolveError::Script
            }
        })
    }

    fn run(&mut self, job: &Job) -> Result<Solved, SolveError> {
        let mut preprocessed = None;
        if self.loaded.as_deref() != Some(job.id.as_str()) {
            self.loaded = None;
            let script = if job.kind == "player" {
                // The whole script: the solver parses it and gives back its preprocessed form.
                let empty = r#"[{"type":"n","challenges":[]},{"type":"sig","challenges":[]}]"#;
                let text = self.eval(
                    job.limit,
                    &[("__script", &job.script), ("__requests", empty)],
                    "JSON.stringify(jsc({type: 'player', player: __script, requests: JSON.parse(__requests), output_preprocessed: true}))",
                )?;
                let given = read_output(&text)?;
                preprocessed = given.preprocessed;
                preprocessed.clone().ok_or(SolveError::Script)?
            } else {
                job.script.clone()
            };
            self.eval(job.limit, &[("__script", &script)], &format!("{LOAD} ''"))?;
            self.loaded = Some(job.id.clone());
        }
        let text = self.eval(job.limit, &[("__requests", &job.requests)], RUN)?;
        let mut solved = read_solved(&text, &job.requests)?;
        solved.preprocessed = preprocessed;
        Ok(solved)
    }
}

/// The answers of [`RUN`]: `{n: {...}|null, sig: {...}|null}`. A kind with nothing asked is
/// fine; a kind asked for and answered `null` is an error.
fn read_solved(text: &str, requests: &str) -> Result<Solved, SolveError> {
    let answers: Value = serde_json::from_str(text).map_err(|_| SolveError::Script)?;
    let asked: Value = serde_json::from_str(requests).map_err(|_| SolveError::Script)?;
    let table =
        |kind: &'static str, label: &'static str| -> Result<HashMap<String, String>, SolveError> {
            let wanted = asked
                .get(kind)
                .and_then(Value::as_array)
                .is_some_and(|list| !list.is_empty());
            match answers.get(kind).and_then(Value::as_object) {
                Some(map) => Ok(map
                    .iter()
                    .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
                    .collect()),
                None if wanted => Err(SolveError::NoFunction(label)),
                None => Ok(HashMap::new()),
            }
        };
    Ok(Solved {
        n: table("n", "n")?,
        sig: table("sig", "signature")?,
        preprocessed: None,
    })
}

/// Read the answer of `jsc` for a whole script: what matters is the preprocessed script.
fn read_output(text: &str) -> Result<Solved, SolveError> {
    let root: Value = serde_json::from_str(text).map_err(|_| SolveError::Script)?;
    let responses = root
        .get("responses")
        .and_then(Value::as_array)
        .ok_or(SolveError::Script)?;
    let table = |index: usize, kind: &'static str| -> Result<HashMap<String, String>, SolveError> {
        let response = responses.get(index).ok_or(SolveError::Script)?;
        if response.get("type").and_then(Value::as_str) != Some("result") {
            return Err(SolveError::NoFunction(kind));
        }
        let data = response
            .get("data")
            .and_then(Value::as_object)
            .ok_or(SolveError::Script)?;
        Ok(data
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
            .collect())
    };
    Ok(Solved {
        n: table(0, "n")?,
        sig: table(1, "signature")?,
        preprocessed: root
            .get("preprocessed_player")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

fn worker(inbox: &Receiver<Job>) {
    let mut engine: Option<Engine> = None;
    for job in inbox {
        let result = match engine.take().map_or_else(Engine::new, Ok) {
            Ok(mut ready) => {
                let result = ready.run(&job);
                // An engine that hit a limit may hold a half-built state: build a new one.
                if !matches!(result, Err(SolveError::Timeout | SolveError::Script)) {
                    engine = Some(ready);
                }
                result
            }
            Err(e) => Err(e),
        };
        // The caller is gone only when it gave up; the result is then not wanted.
        let _ = job.reply.send(result);
    }
}
