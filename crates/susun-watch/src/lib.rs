//! Optional watch/develop support for Susun.
//!
//! This crate owns normalized file events, root confinement, ignore filtering,
//! debounce, and cancellation around native filesystem notifications.

use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use notify::{
    Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{ModifyKind, RenameMode},
};
use susun_build::Dockerignore;

/// Result type for watch operations.
pub type WatchResult<T> = Result<T, WatchError>;

/// Errors returned by watch operations.
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    /// Project root or watch path could not be resolved.
    #[error("watch path error for {path}: {source}")]
    Path {
        /// Path involved in the error.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// Path escapes the configured project root.
    #[error("watch path escapes project root: {path}")]
    RootEscape {
        /// Escaping path.
        path: PathBuf,
    },
    /// Native watcher failed.
    #[error("native watcher failed: {0}")]
    Native(#[from] notify::Error),
    /// Watch event channel closed.
    #[error("watch event channel closed")]
    Closed,
}

/// Cancellation token shared by watch sessions.
#[derive(Debug, Clone, Default)]
pub struct WatchCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl WatchCancellationToken {
    /// Creates a fresh non-cancelled token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns true when cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Watcher configuration.
#[derive(Debug, Clone)]
pub struct WatchOptions {
    /// Project root used for confinement and event-relative paths.
    pub project_root: PathBuf,
    /// Paths to watch. Empty means watch the project root recursively.
    pub paths: Vec<PathBuf>,
    /// Debounce window for normalized events.
    pub debounce: Duration,
    /// Dockerignore-style ignore rules.
    pub ignore: Dockerignore,
    /// Additional Dockerignore-style rules applied after `ignore`.
    pub extra_ignore: Dockerignore,
}

impl WatchOptions {
    /// Creates options for a project root with conservative defaults.
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            paths: Vec::new(),
            debounce: Duration::from_millis(150),
            ignore: Dockerignore::default(),
            extra_ignore: Dockerignore::default(),
        }
    }

    /// Replaces watched paths.
    pub fn with_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.paths = paths;
        self
    }

    /// Replaces the debounce window.
    pub fn with_debounce(mut self, debounce: Duration) -> Self {
        self.debounce = debounce;
        self
    }

    /// Replaces primary ignore rules.
    pub fn with_ignore(mut self, ignore: Dockerignore) -> Self {
        self.ignore = ignore;
        self
    }

    /// Replaces additional ignore rules.
    pub fn with_extra_ignore(mut self, ignore: Dockerignore) -> Self {
        self.extra_ignore = ignore;
        self
    }
}

/// Normalized watch event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WatchEventKind {
    /// File or directory was created.
    Created,
    /// File or directory contents or metadata changed.
    Modified,
    /// File or directory was removed.
    Removed,
    /// File or directory was renamed or moved.
    Renamed,
    /// Event kind could not be classified further.
    Other,
}

/// Normalized confined watch event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchEvent {
    /// Event kind.
    pub kind: WatchEventKind,
    /// Path relative to the project root.
    pub relative_path: PathBuf,
    /// Canonical absolute path when available, otherwise normalized absolute path.
    pub absolute_path: PathBuf,
    /// True when the path currently resolves to a directory.
    pub is_dir: bool,
}

/// Running watch session.
#[derive(Debug)]
pub struct WatchSession {
    receiver: mpsc::Receiver<WatchResult<WatchEvent>>,
    cancellation: WatchCancellationToken,
    worker: Option<thread::JoinHandle<()>>,
    _watcher: RecommendedWatcher,
}

impl WatchSession {
    /// Starts a native watcher and returns a normalized event session.
    pub fn start(options: WatchOptions) -> WatchResult<Self> {
        Self::start_with_token(options, WatchCancellationToken::new())
    }

    /// Starts a native watcher with an externally controlled cancellation token.
    pub fn start_with_token(
        options: WatchOptions,
        cancellation: WatchCancellationToken,
    ) -> WatchResult<Self> {
        let resolved = ResolvedWatchOptions::resolve(options)?;
        let (raw_tx, raw_rx) = mpsc::channel();
        let (event_tx, receiver) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |event| {
                let _ = raw_tx.send(event);
            },
            Config::default(),
        )?;

        for path in &resolved.paths {
            watcher.watch(path, RecursiveMode::Recursive)?;
        }

        let worker_token = cancellation.clone();
        let worker = thread::spawn(move || run_debouncer(resolved, worker_token, raw_rx, event_tx));

        Ok(Self {
            receiver,
            cancellation,
            worker: Some(worker),
            _watcher: watcher,
        })
    }

    /// Returns the session cancellation token.
    pub fn cancellation_token(&self) -> WatchCancellationToken {
        self.cancellation.clone()
    }

    /// Receives the next normalized event.
    pub fn recv(&self) -> WatchResult<WatchEvent> {
        self.receiver.recv().map_err(|_| WatchError::Closed)?
    }

    /// Receives the next normalized event up to a timeout.
    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<WatchResult<WatchEvent>, mpsc::RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }
}

impl Drop for WatchSession {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Debug)]
struct ResolvedWatchOptions {
    project_root: PathBuf,
    paths: Vec<PathBuf>,
    debounce: Duration,
    ignore: Dockerignore,
    extra_ignore: Dockerignore,
}

impl ResolvedWatchOptions {
    fn resolve(options: WatchOptions) -> WatchResult<Self> {
        let project_root = canonicalize_existing(&options.project_root)?;
        let paths = if options.paths.is_empty() {
            vec![project_root.clone()]
        } else {
            options
                .paths
                .iter()
                .map(|path| resolve_under_root(&project_root, path))
                .collect::<WatchResult<Vec<_>>>()?
        };
        Ok(Self {
            project_root,
            paths,
            debounce: options.debounce,
            ignore: options.ignore,
            extra_ignore: options.extra_ignore,
        })
    }
}

fn run_debouncer(
    options: ResolvedWatchOptions,
    cancellation: WatchCancellationToken,
    raw_rx: mpsc::Receiver<notify::Result<notify::Event>>,
    event_tx: mpsc::Sender<WatchResult<WatchEvent>>,
) {
    let mut pending = BTreeMap::<(PathBuf, WatchEventKind), (WatchEvent, Instant)>::new();
    let tick = options.debounce.max(Duration::from_millis(25));
    while !cancellation.is_cancelled() {
        match raw_rx.recv_timeout(tick) {
            Ok(Ok(event)) => {
                for event in normalize_event(&options, event) {
                    pending.insert(
                        (event.relative_path.clone(), event.kind),
                        (event, Instant::now()),
                    );
                }
            }
            Ok(Err(error)) => {
                let _ = event_tx.send(Err(WatchError::Native(error)));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        flush_ready(&mut pending, options.debounce, &event_tx);
    }
    flush_all(&mut pending, &event_tx);
}

fn normalize_event(options: &ResolvedWatchOptions, event: notify::Event) -> Vec<WatchEvent> {
    let kind = watch_event_kind(event.kind);
    event
        .paths
        .into_iter()
        .filter_map(|path| normalize_path(options, path, kind).ok().flatten())
        .collect()
}

fn normalize_path(
    options: &ResolvedWatchOptions,
    path: PathBuf,
    kind: WatchEventKind,
) -> WatchResult<Option<WatchEvent>> {
    let absolute_path = absolutize_path(&path)?;
    let relative_path = relative_to_root(&options.project_root, &absolute_path)?;
    let is_dir = absolute_path.is_dir();
    if options.ignore.is_ignored(&relative_path, is_dir)
        || options.extra_ignore.is_ignored(&relative_path, is_dir)
    {
        return Ok(None);
    }
    Ok(Some(WatchEvent {
        kind,
        relative_path,
        absolute_path,
        is_dir,
    }))
}

fn watch_event_kind(kind: EventKind) -> WatchEventKind {
    match kind {
        EventKind::Create(_) => WatchEventKind::Created,
        EventKind::Modify(ModifyKind::Name(
            RenameMode::Any | RenameMode::From | RenameMode::To | RenameMode::Both,
        )) => WatchEventKind::Renamed,
        EventKind::Modify(_) => WatchEventKind::Modified,
        EventKind::Remove(_) => WatchEventKind::Removed,
        EventKind::Any | EventKind::Other | EventKind::Access(_) => WatchEventKind::Other,
    }
}

fn flush_ready(
    pending: &mut BTreeMap<(PathBuf, WatchEventKind), (WatchEvent, Instant)>,
    debounce: Duration,
    event_tx: &mpsc::Sender<WatchResult<WatchEvent>>,
) {
    let now = Instant::now();
    let ready = pending
        .iter()
        .filter_map(|(key, (_, inserted))| {
            now.duration_since(*inserted)
                .ge(&debounce)
                .then_some(key.clone())
        })
        .collect::<Vec<_>>();
    for key in ready {
        if let Some((event, _)) = pending.remove(&key) {
            let _ = event_tx.send(Ok(event));
        }
    }
}

fn flush_all(
    pending: &mut BTreeMap<(PathBuf, WatchEventKind), (WatchEvent, Instant)>,
    event_tx: &mpsc::Sender<WatchResult<WatchEvent>>,
) {
    let events = std::mem::take(pending)
        .into_values()
        .map(|(event, _)| event)
        .collect::<Vec<_>>();
    for event in events {
        let _ = event_tx.send(Ok(event));
    }
}

fn canonicalize_existing(path: &Path) -> WatchResult<PathBuf> {
    fs::canonicalize(path).map_err(|source| WatchError::Path {
        path: path.to_path_buf(),
        source,
    })
}

fn resolve_under_root(root: &Path, path: &Path) -> WatchResult<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let resolved = canonicalize_existing(&candidate)?;
    if resolved.starts_with(root) {
        Ok(resolved)
    } else {
        Err(WatchError::RootEscape { path: resolved })
    }
}

fn absolutize_path(path: &Path) -> WatchResult<PathBuf> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(_) if path.is_absolute() => Ok(clean_path(path)),
        Err(source) => Err(WatchError::Path {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn relative_to_root(root: &Path, path: &Path) -> WatchResult<PathBuf> {
    if path.starts_with(root) {
        path.strip_prefix(root)
            .map(Path::to_path_buf)
            .map_err(|_| WatchError::RootEscape {
                path: path.to_path_buf(),
            })
    } else {
        Err(WatchError::RootEscape {
            path: path.to_path_buf(),
        })
    }
}

fn clean_path(path: &Path) -> PathBuf {
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                clean.pop();
            }
            _ => clean.push(component.as_os_str()),
        }
    }
    clean
}
