use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvError, RecvTimeoutError};
use std::time::{Duration, Instant};

use notify::{
    Config, Error, Event, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher, WatcherKind,
};

use crate::source::paths_same_location;

#[derive(Debug)]
pub(super) struct WatchBatch {
    pub(super) paths: BTreeSet<PathBuf>,
    pub(super) rescan_all: bool,
}

impl WatchBatch {
    pub(super) fn rescan_all() -> Self {
        Self {
            paths: BTreeSet::new(),
            rescan_all: true,
        }
    }
}

#[derive(Debug)]
pub(super) enum ReceiveError {
    Backend(Error),
    Disconnected(RecvError),
}

pub(super) struct WatchBackend {
    _handle: WatcherHandle,
    receiver: Receiver<notify::Result<Event>>,
    kind: WatcherKind,
    last_reconcile: Instant,
}

enum WatcherHandle {
    Native(RecommendedWatcher),
    Poll(PollWatcher),
}

impl WatchBackend {
    pub(super) fn start(
        workspace_root: &Path,
        source_root: &Path,
        poll_interval: Duration,
        force_poll: bool,
    ) -> Result<(Self, Option<Error>), Error> {
        if !force_poll {
            match Self::native(workspace_root, source_root) {
                Ok(backend) => return Ok((backend, None)),
                Err(native_error) => {
                    return Self::poll(workspace_root, source_root, poll_interval)
                        .map(|backend| (backend, Some(native_error)));
                }
            }
        }

        Self::poll(workspace_root, source_root, poll_interval).map(|backend| (backend, None))
    }

    pub(super) fn poll(
        workspace_root: &Path,
        source_root: &Path,
        poll_interval: Duration,
    ) -> Result<Self, Error> {
        let (sender, receiver) = mpsc::channel();
        let config = Config::default()
            .with_poll_interval(poll_interval)
            .with_compare_contents(true);
        let watcher = PollWatcher::new(
            move |event| {
                let _ = sender.send(event);
            },
            config,
        )?;
        let mut backend = Self {
            _handle: WatcherHandle::Poll(watcher),
            receiver,
            kind: WatcherKind::PollWatcher,
            last_reconcile: Instant::now(),
        };

        backend.watch_roots(workspace_root, source_root)?;
        Ok(backend)
    }

    fn native(workspace_root: &Path, source_root: &Path) -> Result<Self, Error> {
        let (sender, receiver) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })?;
        let kind = RecommendedWatcher::kind();
        let mut backend = Self {
            _handle: WatcherHandle::Native(watcher),
            receiver,
            kind,
            last_reconcile: Instant::now(),
        };

        backend.watch_roots(workspace_root, source_root)?;
        Ok(backend)
    }

    pub(super) fn is_polling(&self) -> bool {
        self.kind == WatcherKind::PollWatcher
    }

    pub(super) fn receive_batch(
        &mut self,
        debounce: Duration,
        reconcile_interval: Duration,
    ) -> Result<WatchBatch, ReceiveError> {
        let polling = self.is_polling();
        let first = if polling {
            self.receiver.recv().map_err(ReceiveError::Disconnected)?
        } else {
            let timeout = reconcile_interval.saturating_sub(self.last_reconcile.elapsed());

            match self.receiver.recv_timeout(timeout) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => {
                    self.last_reconcile = Instant::now();
                    return Ok(WatchBatch::rescan_all());
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(ReceiveError::Disconnected(RecvError));
                }
            }
        };
        let mut batch = WatchBatch {
            paths: BTreeSet::new(),
            rescan_all: false,
        };

        append_event(&mut batch, first.map_err(ReceiveError::Backend)?);

        loop {
            match self.receiver.recv_timeout(debounce) {
                Ok(Ok(event)) => append_event(&mut batch, event),
                Ok(Err(error)) => return Err(ReceiveError::Backend(error)),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(ReceiveError::Disconnected(RecvError));
                }
            }
        }

        if !polling && (batch.rescan_all || self.last_reconcile.elapsed() >= reconcile_interval) {
            batch.rescan_all = true;
            self.last_reconcile = Instant::now();
        }

        Ok(batch)
    }

    fn watch_roots(&mut self, workspace_root: &Path, source_root: &Path) -> Result<(), Error> {
        if paths_same_location(workspace_root, source_root) {
            return self
                .watcher_mut()
                .watch(workspace_root, RecursiveMode::Recursive);
        }

        self.watcher_mut()
            .watch(workspace_root, RecursiveMode::NonRecursive)?;
        self.watcher_mut()
            .watch(source_root, RecursiveMode::Recursive)
    }

    fn watcher_mut(&mut self) -> &mut dyn Watcher {
        match &mut self._handle {
            WatcherHandle::Native(watcher) => watcher,
            WatcherHandle::Poll(watcher) => watcher,
        }
    }
}

fn append_event(batch: &mut WatchBatch, event: Event) {
    if event.paths.is_empty() || event.need_rescan() {
        batch.rescan_all = true;
    }

    batch.paths.extend(event.paths);
}
