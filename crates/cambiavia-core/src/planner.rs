use std::fmt;

use crate::identity::{AdapterIdStyle, GpuKey};
use crate::prefs::choice::{self, GpuChoice};
use crate::prefs::store::PrefStore;

pub const GLOBAL_SETTINGS_VALUE: &str = "DirectXUserGlobalSettings";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub value_name: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadBackMismatch {
    pub value_name: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

impl fmt::Display for ReadBackMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "read-back mismatch for {}", self.value_name)
    }
}

impl std::error::Error for ReadBackMismatch {}

fn plan_value(value_name: &str, before: Option<String>, after: Option<String>) -> Option<Change> {
    if before == after {
        return None;
    }
    Some(Change {
        value_name: value_name.to_string(),
        before,
        after,
    })
}

pub fn plan_app_choice(
    store: &dyn PrefStore,
    value_name: &str,
    choice: &GpuChoice,
    style: AdapterIdStyle,
) -> Option<Change> {
    let before = store.get(value_name);
    let after = choice::value_after(before.as_deref(), choice, style);
    plan_value(value_name, before, after)
}

pub fn plan_high_perf(
    store: &dyn PrefStore,
    adapter: Option<GpuKey>,
    style: AdapterIdStyle,
) -> Option<Change> {
    let before = store.get(GLOBAL_SETTINGS_VALUE);
    let after = choice::value_after_high_perf(before.as_deref(), adapter, style);
    plan_value(GLOBAL_SETTINGS_VALUE, before, after)
}

pub fn apply_changes(
    store: &mut dyn PrefStore,
    changes: &[Change],
) -> Result<(), ReadBackMismatch> {
    for change in changes {
        match &change.after {
            Some(value) => store.set(&change.value_name, value),
            None => store.delete(&change.value_name),
        }
        let actual = store.get(&change.value_name);
        if actual != change.after {
            return Err(ReadBackMismatch {
                value_name: change.value_name.clone(),
                expected: change.after.clone(),
                actual,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::store::MemoryStore;

    const PEAK_EXE: &str = "C:\\Games\\PEAK\\PEAK.exe";
    const PEAK_BEFORE: &str = "AppStatus=0;AutoHDREnable=2097;";
    const GLOBAL_BEFORE: &str = "SwapEffectUpgradeEnable=1;AutoHDREnable=1;";

    fn dgpu_key() -> GpuKey {
        GpuKey::parse("1002&747E&2427148C").unwrap()
    }

    #[test]
    fn plan_returns_none_when_nothing_changes() {
        let mut store = MemoryStore::new();
        store.set("C:\\ASUS\\app.exe", "GpuPreference=1; ");
        assert_eq!(
            plan_app_choice(
                &store,
                "C:\\ASUS\\app.exe",
                &GpuChoice::PowerSaving,
                AdapterIdStyle::Padded
            ),
            None
        );
        assert_eq!(
            plan_app_choice(
                &store,
                "C:\\Missing\\app.exe",
                &GpuChoice::WindowsDecides,
                AdapterIdStyle::Padded
            ),
            None
        );
    }

    #[test]
    fn plan_specific_records_before_and_after() {
        let mut store = MemoryStore::new();
        store.set(PEAK_EXE, PEAK_BEFORE);
        let change = plan_app_choice(
            &store,
            PEAK_EXE,
            &GpuChoice::Specific(dgpu_key()),
            AdapterIdStyle::Padded,
        )
        .unwrap();
        assert_eq!(change.value_name, PEAK_EXE);
        assert_eq!(change.before.as_deref(), Some(PEAK_BEFORE));
        assert_eq!(
            change.after.as_deref(),
            Some("SpecificAdapter=1002&747E&2427148C;GpuPreference=1073741824;AppStatus=0;AutoHDREnable=2097;")
        );
    }

    #[test]
    fn plan_high_perf_targets_global_settings() {
        let mut store = MemoryStore::new();
        store.set(GLOBAL_SETTINGS_VALUE, GLOBAL_BEFORE);
        let change = plan_high_perf(&store, Some(dgpu_key()), AdapterIdStyle::Padded).unwrap();
        assert_eq!(change.value_name, GLOBAL_SETTINGS_VALUE);
        assert_eq!(
            change.after.as_deref(),
            Some("SwapEffectUpgradeEnable=1;AutoHDREnable=1;HighPerfAdapter=1002&747E&2427148C;")
        );
        let mut applied = MemoryStore::new();
        applied.set(GLOBAL_SETTINGS_VALUE, GLOBAL_BEFORE);
        let changes = [change];
        apply_changes(&mut applied, &changes).unwrap();
        assert_eq!(
            plan_high_perf(&applied, Some(dgpu_key()), AdapterIdStyle::Padded),
            None
        );
    }

    #[test]
    fn apply_writes_and_second_plan_is_empty() {
        let mut store = MemoryStore::new();
        store.set(PEAK_EXE, PEAK_BEFORE);
        let change = plan_app_choice(
            &store,
            PEAK_EXE,
            &GpuChoice::Specific(dgpu_key()),
            AdapterIdStyle::Padded,
        )
        .unwrap();
        apply_changes(&mut store, std::slice::from_ref(&change)).unwrap();
        assert_eq!(store.get(PEAK_EXE), change.after);
        assert_eq!(
            plan_app_choice(
                &store,
                PEAK_EXE,
                &GpuChoice::Specific(dgpu_key()),
                AdapterIdStyle::Padded
            ),
            None
        );
    }

    struct NoWriteStore;

    impl PrefStore for NoWriteStore {
        fn list(&self) -> Vec<(String, String)> {
            Vec::new()
        }

        fn get(&self, _name: &str) -> Option<String> {
            None
        }

        fn set(&mut self, _name: &str, _value: &str) {}

        fn delete(&mut self, _name: &str) {}
    }

    #[test]
    fn apply_reports_first_mismatch() {
        let mut store = NoWriteStore;
        let changes = [
            Change {
                value_name: "gone.exe".to_string(),
                before: None,
                after: None,
            },
            Change {
                value_name: "kept.exe".to_string(),
                before: None,
                after: Some("GpuPreference=2;".to_string()),
            },
        ];
        let err = apply_changes(&mut store, &changes).unwrap_err();
        assert_eq!(err.value_name, "kept.exe");
        assert_eq!(err.expected.as_deref(), Some("GpuPreference=2;"));
        assert_eq!(err.actual, None);
    }
}
