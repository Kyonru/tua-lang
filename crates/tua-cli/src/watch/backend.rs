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
        additional_roots: &[PathBuf],
        poll_interval: Duration,
        force_poll: bool,
    ) -> Result<(Self, Option<Error>), Error> {
        if !force_poll {
            match Self::native(workspace_root, source_root, additional_roots) {
                Ok(backend) => return Ok((backend, None)),
                Err(native_error) => {
                    return Self::poll(
                        workspace_root,
                        source_root,
                        additional_roots,
                        poll_interval,
                    )
                    .map(|backend| (backend, Some(native_error)));
                }
            }
        }

        Self::poll(workspace_root, source_root, additional_roots, poll_interval)
            .map(|backend| (backend, None))
    }

    pub(super) fn poll(
        workspace_root: &Path,
        source_root: &Path,
        additional_roots: &[PathBuf],
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

        backend.watch_roots(workspace_root, source_root, additional_roots)?;
        Ok(backend)
    }

    fn native(
        workspace_root: &Path,
        source_root: &Path,
        additional_roots: &[PathBuf],
    ) -> Result<Self, Error> {
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

        backend.watch_roots(workspace_root, source_root, additional_roots)?;
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
        let Some(first) =
            receive_first(&self.receiver, &mut self.last_reconcile, reconcile_interval)?
        else {
            return Ok(WatchBatch::rescan_all());
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

        if batch.rescan_all || self.last_reconcile.elapsed() >= reconcile_interval {
            batch.rescan_all = true;
            self.last_reconcile = Instant::now();
        }

        Ok(batch)
    }

    fn watch_roots(
        &mut self,
        workspace_root: &Path,
        source_root: &Path,
        additional_roots: &[PathBuf],
    ) -> Result<(), Error> {
        if paths_same_location(workspace_root, source_root) {
            self.watcher_mut()
                .watch(workspace_root, RecursiveMode::Recursive)?;
        } else {
            self.watcher_mut()
                .watch(workspace_root, RecursiveMode::NonRecursive)?;
            self.watcher_mut()
                .watch(source_root, RecursiveMode::Recursive)?;
        }

        for root in additional_roots {
            if root.starts_with(source_root)
                || (paths_same_location(workspace_root, source_root)
                    && root.starts_with(workspace_root))
            {
                continue;
            }

            self.watcher_mut().watch(root, RecursiveMode::Recursive)?;
        }

        Ok(())
    }

    fn watcher_mut(&mut self) -> &mut dyn Watcher {
        match &mut self._handle {
            WatcherHandle::Native(watcher) => watcher,
            WatcherHandle::Poll(watcher) => watcher,
        }
    }
}

fn receive_first(
    receiver: &Receiver<notify::Result<Event>>,
    last_reconcile: &mut Instant,
    reconcile_interval: Duration,
) -> Result<Option<notify::Result<Event>>, ReceiveError> {
    // Polling can miss a change made before its first baseline, so every
    // backend needs a bounded wait followed by snapshot reconciliation.
    let timeout = reconcile_interval.saturating_sub(last_reconcile.elapsed());
    match receiver.recv_timeout(timeout) {
        Ok(event) => Ok(Some(event)),
        Err(RecvTimeoutError::Timeout) => {
            *last_reconcile = Instant::now();
            Ok(None)
        }
        Err(RecvTimeoutError::Disconnected) => Err(ReceiveError::Disconnected(RecvError)),
    }
}

fn append_event(batch: &mut WatchBatch, event: Event) {
    if event.paths.is_empty() || event.need_rescan() {
        batch.rescan_all = true;
    }

    batch.paths.extend(event.paths);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receive_first_times_out_for_reconciliation_without_backend_events() {
        let (_sender, receiver) = mpsc::channel();
        let mut last_reconcile = Instant::now();

        let event = receive_first(&receiver, &mut last_reconcile, Duration::ZERO)
            .expect("reconciliation should not depend on a polling event");

        assert!(event.is_none());
    }
}
