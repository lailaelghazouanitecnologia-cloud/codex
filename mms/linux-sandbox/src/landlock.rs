use std::path::Path;

use enumflags2::BitFlags;
use landlock::{
    ABI, Access, AccessFs, CompatLevel, Compatible, PathBeneath, PathFd,
    Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus,
};

use crate::error::{SandboxError, SandboxResult};
use crate::policy::WritableRoot;

const LANDLOCK_ABI: ABI = ABI::V5;

pub fn install_filesystem_rules(
    writable_roots: Vec<WritableRoot>,
    no_new_privs: bool,
) -> SandboxResult<()> {
    let access_rw = AccessFs::from_all(LANDLOCK_ABI);
    let access_ro = AccessFs::from_read(LANDLOCK_ABI);

    let ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(access_rw)
        .map_err(|_| SandboxError::LandlockRulesetCreate)?
        .create()
        .map_err(|_| SandboxError::LandlockRulesetCreate)?;

    let mut ruleset = add_path_rule(ruleset, "/", access_ro)?;

    ruleset = add_path_rule(ruleset, "/dev/null", access_rw)?;
    ruleset = add_path_rule(ruleset, "/dev/zero", access_ro)?;
    ruleset = add_path_rule(ruleset, "/dev/urandom", access_ro)?;
    ruleset = add_path_rule(ruleset, "/dev/random", access_ro)?;

    for writable in &writable_roots {
        if writable.root.exists() {
            ruleset = add_path_rule(
                ruleset,
                &writable.root,
                access_rw,
            )?;
        }
    }

    let ruleset = if no_new_privs {
        ruleset.set_no_new_privs(true)
    } else {
        ruleset
    };

    let status = ruleset
        .restrict_self()
        .map_err(|_| SandboxError::LandlockRestrict)?;

    if status.ruleset == RulesetStatus::NotEnforced {
        return Err(SandboxError::LandlockNotEnforced);
    }

    Ok(())
}

fn add_path_rule<R: RulesetCreatedAttr>(
    ruleset: R,
    path: impl AsRef<Path>,
    access: BitFlags<AccessFs>,
) -> SandboxResult<R> {
    let path = path.as_ref();

    if !path.exists() {
        return Ok(ruleset);
    }

    let fd = PathFd::new(path)
        .map_err(|e| SandboxError::LandlockRuleAdd(format!("PathFd: {}", e)))?;

    let rule = PathBeneath::new(fd, access);

    let ruleset = ruleset
        .add_rule(rule)
        .map_err(|e| SandboxError::LandlockRuleAdd(e.to_string()))?;

    Ok(ruleset)
}

pub fn is_landlock_supported() -> bool {
    Ruleset::default()
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(AccessFs::from_all(LANDLOCK_ABI))
        .is_ok()
}
