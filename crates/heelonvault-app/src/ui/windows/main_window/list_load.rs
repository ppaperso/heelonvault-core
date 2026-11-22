use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub(super) struct LoadGeneration {
    current: Cell<u64>,
    cancellation: RefCell<Option<Arc<AtomicBool>>>,
}

pub(super) struct LoadTicket {
    generation: u64,
    cancelled: Arc<AtomicBool>,
}

impl LoadGeneration {
    pub(super) fn invalidate(&self) {
        self.current.set(self.current.get().wrapping_add(1));
        if let Some(cancelled) = self.cancellation.borrow_mut().take() {
            cancelled.store(true, Ordering::Relaxed);
        }
    }

    pub(super) fn begin(&self) -> LoadTicket {
        self.invalidate();
        let cancelled = Arc::new(AtomicBool::new(false));
        *self.cancellation.borrow_mut() = Some(Arc::clone(&cancelled));
        LoadTicket {
            generation: self.current.get(),
            cancelled,
        }
    }

    pub(super) fn accepts(&self, ticket: &LoadTicket, session_open: bool) -> bool {
        session_open && self.current.get() == ticket.generation && !ticket.cancelled()
    }
}

impl LoadTicket {
    pub(super) fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub(super) fn cancellation(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Coverage {
    pub vaults: usize,
    pub loaded_vaults: usize,
    pub failed_vaults: usize,
    pub failed_items: usize,
}

impl Coverage {
    pub(super) fn partial(self) -> bool {
        self.failed_vaults > 0 || self.failed_items > 0
    }

    pub(super) fn failed(self) -> bool {
        self.vaults > 0 && self.loaded_vaults == 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ListLoadState {
    #[default]
    Idle,
    Loading,
    Locked,
    Failed,
    Ready {
        coverage: Coverage,
        global: bool,
    },
}

impl ListLoadState {
    pub(super) fn message(self) -> String {
        use heelonvault_core::i18n::{I18nArg, tr_args};
        match self {
            Self::Idle => String::new(),
            Self::Loading => heelonvault_core::tr!("main-secrets-loading-title"),
            Self::Locked => heelonvault_core::tr!("main-session-locked-title"),
            Self::Failed => heelonvault_core::tr!("main-list-unavailable-description"),
            Self::Ready { coverage, global } => {
                let key = if coverage.partial() {
                    "main-list-partial"
                } else if global {
                    "main-list-complete-global"
                } else {
                    "main-list-complete"
                };
                tr_args(
                    key,
                    &[
                        ("loaded", I18nArg::Num(coverage.loaded_vaults as i64)),
                        ("total", I18nArg::Num(coverage.vaults as i64)),
                        ("failed", I18nArg::Num(coverage.failed_vaults as i64)),
                        ("items", I18nArg::Num(coverage.failed_items as i64)),
                    ],
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_latest_request_can_complete() {
        let generation = LoadGeneration::default();
        let first = generation.begin();
        let second = generation.begin();
        assert!(first.cancelled());
        assert!(generation.accepts(&second, true));
        assert!(!generation.accepts(&first, true));
        assert!(!generation.accepts(&second, false));
    }

    #[test]
    fn locking_invalidates_even_after_unlock() {
        let generation = LoadGeneration::default();
        let before_lock = generation.begin();
        generation.invalidate();
        assert!(!generation.accepts(&before_lock, true));
        let after_unlock = generation.begin();
        assert!(!generation.accepts(&before_lock, true));
        assert!(generation.accepts(&after_unlock, true));
    }

    #[test]
    fn empty_partial_and_failed_are_distinct() {
        assert!(!Coverage::default().partial());
        assert!(!Coverage::default().failed());
        let coverage = Coverage {
            vaults: 2,
            loaded_vaults: 1,
            failed_vaults: 1,
            failed_items: 0,
        };
        assert!(coverage.partial());
        assert!(!coverage.failed());
        assert!(
            Coverage {
                loaded_vaults: 0,
                ..coverage
            }
            .failed()
        );
        assert!(
            Coverage {
                vaults: 1,
                loaded_vaults: 1,
                failed_items: 1,
                ..Coverage::default()
            }
            .partial()
        );
    }
}
