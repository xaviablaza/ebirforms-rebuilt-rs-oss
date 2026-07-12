use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("profile store failed: {0}")]
    Store(String),
    #[error("profile `{0}` not found")]
    NotFound(String),
    #[error("profile field `{0}` is required")]
    MissingField(&'static str),
    #[error("theme must be one of light, dark, or system")]
    InvalidTheme,
    #[error("master PIN must contain at least 4 characters")]
    InvalidPin,
    #[error("master PIN is not initialized")]
    PinNotInitialized,
    #[error("submission mode must be dry_run or live")]
    InvalidSubmissionMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxpayerProfile {
    pub profile_id: String,
    pub tin: String,
    pub email: String,
    pub taxpayer_name: String,
    #[serde(default)]
    pub rdo_code: Option<String>,
    #[serde(default)]
    pub registered_address: Option<String>,
    #[serde(default)]
    pub zip_code: Option<String>,
    pub created_unix_seconds: u64,
    pub updated_unix_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettings {
    pub theme: Theme,
    #[serde(default)]
    pub master_pin: Option<PinVerifier>,
    #[serde(default)]
    pub submission_mode: SubmissionModePreference,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubmissionModePreference {
    DryRun,
    Live,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PinVerifier {
    pub algorithm: String,
    pub salt_hex: String,
    pub hash_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppState {
    pub profiles: Vec<TaxpayerProfile>,
    pub settings: AppSettings,
}

#[derive(Debug, Clone)]
pub struct AppStateStore {
    path: PathBuf,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            master_pin: None,
            submission_mode: SubmissionModePreference::DryRun,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            settings: AppSettings::default(),
        }
    }
}

impl Theme {
    pub fn parse(value: &str) -> Result<Self, ProfileError> {
        match value.to_ascii_lowercase().as_str() {
            "light" => Ok(Theme::Light),
            "dark" => Ok(Theme::Dark),
            "system" => Ok(Theme::System),
            _ => Err(ProfileError::InvalidTheme),
        }
    }
}

impl Default for SubmissionModePreference {
    fn default() -> Self {
        Self::DryRun
    }
}

impl SubmissionModePreference {
    pub fn parse(value: &str) -> Result<Self, ProfileError> {
        match value.to_ascii_lowercase().replace('-', "_").as_str() {
            "dry_run" | "dryrun" | "dry" => Ok(Self::DryRun),
            "live" => Ok(Self::Live),
            _ => Err(ProfileError::InvalidSubmissionMode),
        }
    }
}

impl AppStateStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<AppState, ProfileError> {
        if !self.path.exists() {
            return Ok(AppState::default());
        }
        let bytes = fs::read(&self.path).map_err(|err| ProfileError::Store(err.to_string()))?;
        if bytes.is_empty() {
            return Ok(AppState::default());
        }
        serde_json::from_slice(&bytes).map_err(|err| ProfileError::Store(err.to_string()))
    }

    pub fn save(&self, state: &AppState) -> Result<(), ProfileError> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| ProfileError::Store(err.to_string()))?;
            }
        }
        let bytes =
            serde_json::to_vec_pretty(state).map_err(|err| ProfileError::Store(err.to_string()))?;
        fs::write(&self.path, bytes).map_err(|err| ProfileError::Store(err.to_string()))
    }

    pub fn list_profiles(&self) -> Result<Vec<TaxpayerProfile>, ProfileError> {
        Ok(self.load()?.profiles)
    }

    pub fn settings(&self) -> Result<AppSettings, ProfileError> {
        Ok(self.load()?.settings)
    }

    pub fn upsert_profile(
        &self,
        mut profile: TaxpayerProfile,
    ) -> Result<TaxpayerProfile, ProfileError> {
        profile.validate()?;
        let mut state = self.load()?;
        let now = unix_now();
        match state
            .profiles
            .iter_mut()
            .find(|existing| existing.profile_id == profile.profile_id)
        {
            Some(existing) => {
                profile.created_unix_seconds = existing.created_unix_seconds;
                profile.updated_unix_seconds = now;
                *existing = profile.clone();
            }
            None => {
                profile.created_unix_seconds = now;
                profile.updated_unix_seconds = now;
                state.profiles.push(profile.clone());
            }
        }
        self.save(&state)?;
        Ok(profile)
    }

    pub fn set_theme(&self, theme: Theme) -> Result<AppSettings, ProfileError> {
        let mut state = self.load()?;
        state.settings.theme = theme;
        let settings = state.settings.clone();
        self.save(&state)?;
        Ok(settings)
    }

    pub fn set_submission_mode(
        &self,
        mode: SubmissionModePreference,
    ) -> Result<AppSettings, ProfileError> {
        let mut state = self.load()?;
        state.settings.submission_mode = mode;
        let settings = state.settings.clone();
        self.save(&state)?;
        Ok(settings)
    }

    pub fn set_master_pin(&self, pin: &str) -> Result<AppSettings, ProfileError> {
        if pin.len() != 4 || !pin.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(ProfileError::InvalidPin);
        }
        let mut state = self.load()?;
        state.settings.master_pin = Some(hash_pin(pin));
        let settings = state.settings.clone();
        self.save(&state)?;
        Ok(settings)
    }

    pub fn verify_master_pin(&self, pin: &str) -> Result<bool, ProfileError> {
        let state = self.load()?;
        let verifier = state
            .settings
            .master_pin
            .ok_or(ProfileError::PinNotInitialized)?;
        Ok(verify_pin(pin, &verifier))
    }
}

impl TaxpayerProfile {
    pub fn new(profile_id: String, tin: String, email: String, taxpayer_name: String) -> Self {
        Self {
            profile_id,
            tin,
            email,
            taxpayer_name,
            rdo_code: None,
            registered_address: None,
            zip_code: None,
            created_unix_seconds: 0,
            updated_unix_seconds: 0,
        }
    }

    fn validate(&self) -> Result<(), ProfileError> {
        if self.profile_id.trim().is_empty() {
            return Err(ProfileError::MissingField("profile_id"));
        }
        if self.tin.trim().is_empty() {
            return Err(ProfileError::MissingField("tin"));
        }
        if self.email.trim().is_empty() {
            return Err(ProfileError::MissingField("email"));
        }
        if self.taxpayer_name.trim().is_empty() {
            return Err(ProfileError::MissingField("taxpayer_name"));
        }
        Ok(())
    }
}

fn hash_pin(pin: &str) -> PinVerifier {
    let salt = format!("{}:{}", unix_now(), std::process::id());
    let salt_hex = hex::encode(salt.as_bytes());
    let hash_hex = pin_hash_hex(pin, &salt_hex);
    PinVerifier {
        algorithm: "sha256-salted-basic-v1".to_string(),
        salt_hex,
        hash_hex,
    }
}

fn verify_pin(pin: &str, verifier: &PinVerifier) -> bool {
    verifier.algorithm == "sha256-salted-basic-v1"
        && pin_hash_hex(pin, &verifier.salt_hex) == verifier.hash_hex
}

fn pin_hash_hex(pin: &str, salt_hex: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt_hex.as_bytes());
    hasher.update(b":");
    hasher.update(pin.as_bytes());
    hex::encode(hasher.finalize())
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("{name}-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        path
    }

    #[test]
    fn creates_and_updates_taxpayer_profile() {
        let path = temp_path("ebirforms-profiles");
        let store = AppStateStore::new(&path);
        let mut profile = TaxpayerProfile::new(
            "profile-1".to_string(),
            "123-456-789-00000".to_string(),
            "authorized@example.test".to_string(),
            "AUTHORIZED TEST TAXPAYER".to_string(),
        );
        profile.rdo_code = Some("044".to_string());

        let created = store.upsert_profile(profile).unwrap();
        assert_eq!(created.profile_id, "profile-1");
        assert_eq!(store.list_profiles().unwrap().len(), 1);

        let mut updated = created.clone();
        updated.taxpayer_name = "UPDATED TAXPAYER".to_string();
        store.upsert_profile(updated).unwrap();
        let profiles = store.list_profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].taxpayer_name, "UPDATED TAXPAYER");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn settings_store_theme_and_verify_pin() {
        let path = temp_path("ebirforms-settings");
        let store = AppStateStore::new(&path);
        let settings = store.set_theme(Theme::Dark).unwrap();
        assert_eq!(settings.theme, Theme::Dark);
        store.set_master_pin("1234").unwrap();
        assert!(store.verify_master_pin("1234").unwrap());
        assert!(!store.verify_master_pin("9999").unwrap());
        let _ = fs::remove_file(&path);
    }
}
