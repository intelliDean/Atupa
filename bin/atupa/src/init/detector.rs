//! Project framework and DeFi protocol detection for scaffolding.

use std::fs;
use std::path::Path;

/// Detected smart contract development framework.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ProjectKind {
    /// Foundry / Forge project (`foundry.toml` or `forge.toml`).
    Foundry,
    /// Hardhat JavaScript/TypeScript project (`hardhat.config.*`).
    Hardhat,
    /// Arbitrum Stylus Rust project (`Cargo.toml` without JS/Solidity frameworks).
    StylusOnly,
    /// Unknown or unsupported project structure.
    Unknown,
}

impl ProjectKind {
    /// Returns a human-friendly string name of the project framework.
    pub fn label(self) -> &'static str {
        match self {
            Self::Foundry => "Foundry",
            Self::Hardhat => "Hardhat",
            Self::StylusOnly => "Arbitrum Stylus (Rust-only)",
            Self::Unknown => "Unknown",
        }
    }
}

/// Detects the project kind in the current working directory.
pub fn detect_project() -> ProjectKind {
    detect_project_at(Path::new("."))
}

/// Detects the project kind at a specified root path.
pub fn detect_project_at(root: &Path) -> ProjectKind {
    if root.join("foundry.toml").exists() || root.join("forge.toml").exists() {
        return ProjectKind::Foundry;
    }
    if root.join("hardhat.config.js").exists()
        || root.join("hardhat.config.ts").exists()
        || root.join("hardhat.config.mjs").exists()
    {
        return ProjectKind::Hardhat;
    }
    if root.join("Cargo.toml").exists() {
        return ProjectKind::StylusOnly;
    }
    ProjectKind::Unknown
}

/// Detects if the current directory is related to a supported protocol (e.g. Aave, Lido).
pub fn detect_protocol() -> Option<String> {
    detect_protocol_at(Path::new("."))
}

/// Detects protocol mentions at a specified root path.
pub fn detect_protocol_at(root: &Path) -> Option<String> {
    let keywords = [("lido", "lido"), ("aave", "aave"), ("gho", "aave")];
    let files = ["package.json", "foundry.toml", "Cargo.toml"];

    for file in files {
        if let Ok(content) = fs::read_to_string(root.join(file)) {
            let content_lower = content.to_lowercase();
            for (kw, proto) in keywords {
                if content_lower.contains(kw) {
                    return Some(proto.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_kind_labels() {
        assert_eq!(ProjectKind::Foundry.label(), "Foundry");
        assert_eq!(ProjectKind::Hardhat.label(), "Hardhat");
        assert_eq!(ProjectKind::StylusOnly.label(), "Arbitrum Stylus (Rust-only)");
        assert_eq!(ProjectKind::Unknown.label(), "Unknown");
    }

    #[test]
    fn detect_project_at_mock_paths() {
        let temp_dir = std::env::temp_dir().join(format!("atupa_test_detect_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        // Initially unknown
        assert_eq!(detect_project_at(&temp_dir), ProjectKind::Unknown);

        // Add foundry.toml -> Foundry
        fs::write(temp_dir.join("foundry.toml"), "[profile.default]").unwrap();
        assert_eq!(detect_project_at(&temp_dir), ProjectKind::Foundry);
        let _ = fs::remove_file(temp_dir.join("foundry.toml"));

        // Add hardhat.config.ts -> Hardhat
        fs::write(temp_dir.join("hardhat.config.ts"), "export default {};").unwrap();
        assert_eq!(detect_project_at(&temp_dir), ProjectKind::Hardhat);
        let _ = fs::remove_file(temp_dir.join("hardhat.config.ts"));

        // Add Cargo.toml -> StylusOnly
        fs::write(temp_dir.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        assert_eq!(detect_project_at(&temp_dir), ProjectKind::StylusOnly);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn detect_protocol_at_finds_aave_and_lido() {
        let temp_dir = std::env::temp_dir().join(format!("atupa_test_proto_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        fs::write(temp_dir.join("Cargo.toml"), "[dependencies]\naave-v3-core = \"1.0\"").unwrap();
        assert_eq!(detect_protocol_at(&temp_dir), Some("aave".to_string()));

        fs::write(temp_dir.join("Cargo.toml"), "[dependencies]\nlido-contracts = \"1.0\"").unwrap();
        assert_eq!(detect_protocol_at(&temp_dir), Some("lido".to_string()));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
