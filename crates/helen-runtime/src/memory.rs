//! Memory providers (Task 8.4) — port of `helen/runtime/memory.py`.
//!
//! `InMemoryProvider` (ephemeral) and `FileMemoryProvider` (JSON-file backed,
//! persists on every write, parent dirs auto-created).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

/// Abstract memory provider trait (Python `MemoryProvider` ABC).
pub trait MemoryProvider {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&mut self, key: &str, value: &str);
    fn delete(&mut self, key: &str);
    fn list_keys(&self) -> Vec<String>;
    fn clear(&mut self);
}

/// In-memory provider (ephemeral).
#[derive(Debug, Default, Clone)]
pub struct InMemoryProvider {
    data: BTreeMap<String, String>,
}

impl InMemoryProvider {
    pub fn new() -> Self {
        InMemoryProvider { data: BTreeMap::new() }
    }
}

impl MemoryProvider for InMemoryProvider {
    fn get(&self, key: &str) -> Option<String> {
        self.data.get(key).cloned()
    }

    fn set(&mut self, key: &str, value: &str) {
        self.data.insert(key.to_string(), value.to_string());
    }

    fn delete(&mut self, key: &str) {
        self.data.remove(key);
    }

    fn list_keys(&self) -> Vec<String> {
        self.data.keys().cloned().collect()
    }

    fn clear(&mut self) {
        self.data.clear();
    }
}

/// JSON file-backed memory provider (HLD §3.8.2).
///
/// Persists all key-value pairs to a JSON file on every write.
/// Loads existing data on construction; returns empty dict on corruption.
#[derive(Debug, Clone)]
pub struct FileMemoryProvider {
    path: PathBuf,
    data: BTreeMap<String, String>,
}

impl FileMemoryProvider {
    pub fn new(path: &str) -> Self {
        let p = PathBuf::from(path);
        let data = Self::load(&p);
        FileMemoryProvider { path: p, data }
    }

    fn load(path: &PathBuf) -> BTreeMap<String, String> {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(obj) = v.as_object() {
                        return obj
                            .iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect();
                    }
                }
            }
            return BTreeMap::new();
        }
        BTreeMap::new()
    }

    fn save(&self) {
        if let Some(parent) = self.path.parent() {
            if !parent.exists() {
                let _ = fs::create_dir_all(parent);
            }
        }
        let data: serde_json::Map<String, serde_json::Value> = self
            .data
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        if let Ok(json) = serde_json::to_string_pretty(&serde_json::Value::Object(data)) {
            let _ = fs::write(&self.path, json);
        }
    }

    pub fn path(&self) -> &str {
        self.path.to_str().unwrap_or("")
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }
}

impl MemoryProvider for FileMemoryProvider {
    fn get(&self, key: &str) -> Option<String> {
        self.data.get(key).cloned()
    }

    fn set(&mut self, key: &str, value: &str) {
        self.data.insert(key.to_string(), value.to_string());
        self.save();
    }

    fn delete(&mut self, key: &str) {
        if self.data.remove(key).is_some() {
            self.save();
        }
    }

    fn list_keys(&self) -> Vec<String> {
        self.data.keys().cloned().collect()
    }

    fn clear(&mut self) {
        self.data.clear();
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_basic_ops() {
        let mut m = InMemoryProvider::new();
        assert_eq!(m.get("k"), None);
        m.set("k", "v");
        assert_eq!(m.get("k").as_deref(), Some("v"));
        m.set("k2", "v2");
        assert_eq!(m.list_keys().len(), 2);
        m.delete("k");
        assert_eq!(m.list_keys().len(), 1);
        m.clear();
        assert_eq!(m.list_keys().len(), 0);
    }

    #[test]
    fn file_provider_persists() {
        let path = std::env::temp_dir()
            .join(format!("helen_mem_{}.json", crate::transcript::generate_uuid()));
        let path_s = path.to_string_lossy().to_string();

        {
            let mut m = FileMemoryProvider::new(&path_s);
            m.set("hello", "世界");
            m.set("key", "value");
            assert_eq!(m.size(), 2);
        }
        // Reload from disk
        let m2 = FileMemoryProvider::new(&path_s);
        assert_eq!(m2.get("hello").as_deref(), Some("世界"));
        assert_eq!(m2.get("key").as_deref(), Some("value"));
        assert_eq!(m2.path(), path_s);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn file_provider_corruption_returns_empty() {
        let path = std::env::temp_dir()
            .join(format!("helen_mem_bad_{}.json", crate::transcript::generate_uuid()));
        fs::write(&path, "{not valid json").unwrap();
        let m = FileMemoryProvider::new(&path.to_string_lossy());
        assert_eq!(m.size(), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn file_provider_creates_parent_dirs() {
        let dir = std::env::temp_dir()
            .join(format!("helen_mem_dir_{}", crate::transcript::generate_uuid()));
        let path = dir.join("sub").join("mem.json");
        let path_s = path.to_string_lossy().to_string();
        {
            let mut m = FileMemoryProvider::new(&path_s);
            m.set("a", "b");
        }
        assert!(path.exists());
        let m2 = FileMemoryProvider::new(&path_s);
        assert_eq!(m2.get("a").as_deref(), Some("b"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
