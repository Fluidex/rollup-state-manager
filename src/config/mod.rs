use once_cell::sync::OnceCell;
use serde::Deserialize;

#[doc(hidden)]
static SETTINGS: OnceCell<Settings> = OnceCell::new();

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Settings {
    pub brokers: String,
    pub prover_cluster_db: String,
    pub rollup_state_manager_db: String,
    pub persist_every_n_block: u64,
}

impl Settings {
    /// Sets the contents of this cell to the singleton `Settings`
    /// and returns the reference to it.
    ///
    /// # Panics
    /// if the underlying cell is full, it panics.
    pub fn set(settings: Self) -> &'static Self {
        SETTINGS.set(settings).unwrap();
        Self::get()
    }

    /// Gets the reference to the singleton `Settings`.
    ///
    /// # Panics
    /// if the underlying cell is empty, it panics.
    pub fn get() -> &'static Self {
        SETTINGS.get().unwrap()
    }
}
