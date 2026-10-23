use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    pub provider: String,
    pub harness: String,
    pub model: String,
    pub endpoint: String,
    pub key_env: String,
    pub cwd: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub id: u64,
    pub setup: Setup,
}
impl Profile {
    pub fn label(&self) -> String {
        let folder = Path::new(&self.setup.cwd)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        format!(
            "Saved #{} · {} / {} · {} · {}",
            self.id,
            self.setup.harness,
            self.setup.provider,
            if self.setup.model.is_empty() {
                "default"
            } else {
                &self.setup.model
            },
            folder
        )
    }
}
pub struct Store {
    directory: PathBuf,
}
impl Store {
    pub fn user() -> io::Result<Self> {
        Ok(Self {
            directory: crate::platform::config_dir()?,
        })
    }
    pub fn load(&self) -> io::Result<Vec<Profile>> {
        match fs::read(self.directory.join("profiles.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| {
                io::Error::other("Saved profiles could not be read. Existing file was preserved.")
            }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }
    fn edit(&self, update: impl FnOnce(&mut Vec<Profile>) -> io::Result<u64>) -> io::Result<u64> {
        fs::create_dir_all(&self.directory)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.directory.join("profiles.lock"))?;
        // Serialize independent launcher processes; never overwrite another launch's profile.
        lock.lock()?;
        let mut profiles = self.load()?;
        let id = update(&mut profiles)?;
        let temp = self
            .directory
            .join(format!("profiles.{}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp)?;
        file.write_all(&serde_json::to_vec_pretty(&profiles)?)?;
        file.sync_all()?;
        drop(file);
        fs::rename(temp, self.directory.join("profiles.json"))?;
        Ok(id)
    }
    pub fn save(&self, setup: Setup) -> io::Result<u64> {
        self.edit(|profiles| {
            let id = if let Some(index) = profiles.iter().position(|p| p.setup == setup) {
                profiles.remove(index).id
            } else {
                profiles
                    .iter()
                    .map(|p| p.id)
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("Profile ID limit reached."))?
            };
            profiles.insert(0, Profile { id, setup });
            Ok(id)
        })
    }
    pub fn forget(&self, id: u64) -> io::Result<()> {
        self.edit(|profiles| {
            let index = profiles
                .iter()
                .position(|p| p.id == id)
                .ok_or_else(|| io::Error::other("Unknown saved profile."))?;
            profiles.remove(index);
            Ok(id)
        })
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_deduplication_and_removal() {
        let store = Store {
            directory: std::env::temp_dir()
                .join(format!("strator-profiles-test-{}", std::process::id())),
        };
        let setup = Setup {
            provider: "openai".into(),
            harness: "codex".into(),
            model: "model-a".into(),
            endpoint: "https://example.test/v1".into(),
            key_env: "TEST_KEY".into(),
            cwd: "/project".into(),
        };
        let first = store.save(setup.clone()).unwrap();
        assert_eq!(store.save(setup.clone()).unwrap(), first);
        let mut other = setup.clone();
        other.model = "model-b".into();
        let second = store.save(other).unwrap();
        assert_ne!(first, second);
        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[1].setup, setup);
        store.forget(second).unwrap();
        assert_eq!(store.load().unwrap().len(), 1);
        fs::write(store.directory.join("profiles.json"), "invalid").unwrap();
        assert!(store.save(setup).is_err());
        assert_eq!(
            fs::read_to_string(store.directory.join("profiles.json")).unwrap(),
            "invalid"
        );
        fs::remove_dir_all(store.directory).unwrap();
    }
}
