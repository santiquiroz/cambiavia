use std::collections::BTreeMap;

pub trait PrefStore {
    fn list(&self) -> Vec<(String, String)>;
    fn get(&self, name: &str) -> Option<String>;
    fn set(&mut self, name: &str, value: &str);
    fn delete(&mut self, name: &str);
}

fn normalize(name: &str) -> String {
    name.to_lowercase()
}

#[derive(Debug, Default)]
pub struct MemoryStore {
    entries: BTreeMap<String, (String, String)>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PrefStore for MemoryStore {
    fn list(&self) -> Vec<(String, String)> {
        self.entries.values().cloned().collect()
    }

    fn get(&self, name: &str) -> Option<String> {
        self.entries
            .get(&normalize(name))
            .map(|(_, value)| value.clone())
    }

    fn set(&mut self, name: &str, value: &str) {
        self.entries
            .insert(normalize(name), (name.to_string(), value.to_string()));
    }

    fn delete(&mut self, name: &str) {
        self.entries.remove(&normalize(name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_round_trip() {
        let mut store = MemoryStore::new();
        assert_eq!(store.get("DirectXUserGlobalSettings"), None);
        store.set(
            "DirectXUserGlobalSettings",
            "SwapEffectUpgradeEnable=1;AutoHDREnable=1;",
        );
        assert_eq!(
            store.get("DirectXUserGlobalSettings"),
            Some("SwapEffectUpgradeEnable=1;AutoHDREnable=1;".to_string())
        );
    }

    #[test]
    fn names_are_case_insensitive() {
        let mut store = MemoryStore::new();
        store.set("C:\\Game\\PEAK.exe", "AppStatus=0;");
        assert_eq!(
            store.get("c:\\game\\peak.exe"),
            Some("AppStatus=0;".to_string())
        );
        store.delete("C:\\GAME\\PEAK.EXE");
        assert_eq!(store.get("C:\\Game\\PEAK.exe"), None);
    }

    #[test]
    fn list_returns_all_entries() {
        let mut store = MemoryStore::new();
        store.set("B.exe", "GpuPreference=1;");
        store.set("A.exe", "GpuPreference=2;");
        let mut listed = store.list();
        listed.sort();
        assert_eq!(
            listed,
            vec![
                ("A.exe".to_string(), "GpuPreference=2;".to_string()),
                ("B.exe".to_string(), "GpuPreference=1;".to_string()),
            ]
        );
    }
}
