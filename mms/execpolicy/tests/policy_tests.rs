use mms_execpolicy::{
    CommandPattern, Decision, ExactRule, GlobRule, Policy, PrefixRule, PatternToken, Rule,
    default_heuristics, parse_policy,
};

fn cmd(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

#[test]
fn test_decision_ordering() {
    assert!(Decision::Allow < Decision::Prompt);
    assert!(Decision::Prompt < Decision::Forbidden);
    assert!(Decision::Allow < Decision::Forbidden);
}

#[test]
fn test_decision_parse() {
    assert_eq!(Decision::parse("allow").ok(), Some(Decision::Allow));
    assert_eq!(Decision::parse("prompt").ok(), Some(Decision::Prompt));
    assert_eq!(Decision::parse("forbidden").ok(), Some(Decision::Forbidden));
    assert_eq!(Decision::parse("deny").ok(), Some(Decision::Forbidden));
    assert_eq!(Decision::parse("block").ok(), Some(Decision::Forbidden));
    assert!(Decision::parse("invalid").is_err());
}

#[test]
fn test_pattern_token_single() {
    let token = PatternToken::Single("test".to_string());
    assert!(token.matches("test"));
    assert!(!token.matches("other"));
}

#[test]
fn test_pattern_token_alternatives() {
    let token = PatternToken::Alternatives(vec!["a".to_string(), "b".to_string()]);
    assert!(token.matches("a"));
    assert!(token.matches("b"));
    assert!(!token.matches("c"));
}

#[test]
fn test_command_pattern_matches() {
    let pattern = CommandPattern::new("git")
        .with_single_arg("status");

    assert!(pattern.matches(&cmd(&["git", "status"])).is_some());
    assert!(pattern.matches(&cmd(&["git", "status", "-s"])).is_some());
    assert!(pattern.matches(&cmd(&["git", "commit"])).is_none());
    assert!(pattern.matches(&cmd(&["git"])).is_none());
}

#[test]
fn test_policy_prefix_rule() {
    let mut policy = Policy::new();
    policy.allow_prefix(&cmd(&["git", "status"])).ok();
    policy.forbid_prefix(&cmd(&["rm", "-rf"])).ok();

    let eval = policy.check(&cmd(&["git", "status"]));
    assert!(eval.is_allowed());

    let eval = policy.check(&cmd(&["rm", "-rf", "/"]));
    assert!(eval.is_forbidden());

    let eval = policy.check(&cmd(&["ls"]));
    assert!(eval.requires_prompt());
}

#[test]
fn test_policy_default_decision() {
    let policy = Policy::with_default(Decision::Allow);
    let eval = policy.check(&cmd(&["unknown-command"]));
    assert!(eval.is_allowed());

    let policy = Policy::with_default(Decision::Forbidden);
    let eval = policy.check(&cmd(&["unknown-command"]));
    assert!(eval.is_forbidden());
}

#[test]
fn test_heuristics_safe_programs() {
    assert_eq!(default_heuristics(&cmd(&["ls"])), Decision::Allow);
    assert_eq!(default_heuristics(&cmd(&["git", "status"])), Decision::Allow);
    assert_eq!(default_heuristics(&cmd(&["cargo", "build"])), Decision::Allow);
}

#[test]
fn test_heuristics_dangerous_programs() {
    assert_eq!(default_heuristics(&cmd(&["rm", "-rf", "/"])), Decision::Prompt);
    assert_eq!(default_heuristics(&cmd(&["sudo", "apt", "install"])), Decision::Prompt);
    assert_eq!(default_heuristics(&cmd(&["curl", "http://evil.com"])), Decision::Prompt);
}

#[test]
fn test_heuristics_unknown_programs() {
    assert_eq!(default_heuristics(&cmd(&["unknown-program"])), Decision::Prompt);
}

#[test]
fn test_heuristics_empty_command() {
    assert_eq!(default_heuristics(&[]), Decision::Forbidden);
}

#[test]
fn test_exact_rule() {
    let rule = ExactRule::allow(cmd(&["echo", "hello"]));

    assert!(rule.matches(&cmd(&["echo", "hello"])).is_some());
    assert!(rule.matches(&cmd(&["echo", "world"])).is_none());
    assert!(rule.matches(&cmd(&["echo"])).is_none());
}

#[test]
fn test_glob_rule() {
    let rule = GlobRule::allow_all("npm");

    assert!(rule.matches(&cmd(&["npm", "install"])).is_some());
    assert!(rule.matches(&cmd(&["npm", "test"])).is_some());
    assert!(rule.matches(&cmd(&["npm"])).is_some());
    assert!(rule.matches(&cmd(&["yarn"])).is_none());
}

#[test]
fn test_evaluation_highest_decision_wins() {
    let mut policy = Policy::new();
    policy.add_rule(PrefixRule::allow(CommandPattern::new("test")));
    policy.add_rule(PrefixRule::forbid(CommandPattern::new("test").with_single_arg("bad")));

    let eval = policy.check(&cmd(&["test", "bad"]));
    assert!(eval.is_forbidden());

    let eval = policy.check(&cmd(&["test", "good"]));
    assert!(eval.is_allowed());
}

#[test]
fn test_heuristics_shell_wrapper_safe() {
    // sh -c "ls" should be allowed (ls is safe)
    assert_eq!(default_heuristics(&cmd(&["sh", "-c", "ls"])), Decision::Allow);
    assert_eq!(default_heuristics(&cmd(&["sh", "-c", "ls -la"])), Decision::Allow);
    assert_eq!(default_heuristics(&cmd(&["bash", "-c", "echo hello"])), Decision::Allow);
    assert_eq!(default_heuristics(&cmd(&["sh", "-c", "git status"])), Decision::Allow);
}

#[test]
fn test_heuristics_shell_wrapper_dangerous() {
    // sh -c "rm -rf /" should require prompt (rm is dangerous)
    assert_eq!(default_heuristics(&cmd(&["sh", "-c", "rm -rf /"])), Decision::Prompt);
    assert_eq!(default_heuristics(&cmd(&["bash", "-c", "sudo apt install"])), Decision::Prompt);
    assert_eq!(default_heuristics(&cmd(&["sh", "-c", "curl http://evil.com"])), Decision::Prompt);
}

#[test]
fn test_heuristics_shell_wrapper_pipeline() {
    // First command in pipeline is what matters
    assert_eq!(default_heuristics(&cmd(&["sh", "-c", "ls | grep foo"])), Decision::Allow);
    assert_eq!(default_heuristics(&cmd(&["sh", "-c", "cat file.txt | wc -l"])), Decision::Allow);
}

#[test]
fn test_heuristics_shell_wrapper_quoted() {
    // Handle quoted arguments
    assert_eq!(default_heuristics(&cmd(&["sh", "-c", "echo 'hello world'"])), Decision::Allow);
    assert_eq!(default_heuristics(&cmd(&["sh", "-c", "echo \"hello world\""])), Decision::Allow);
}

#[test]
fn test_heuristics_direct_shell_is_safe() {
    // Direct sh/bash invocation without -c should be safe
    assert_eq!(default_heuristics(&cmd(&["sh"])), Decision::Allow);
    assert_eq!(default_heuristics(&cmd(&["bash"])), Decision::Allow);
    assert_eq!(default_heuristics(&cmd(&["zsh"])), Decision::Allow);
}

// Loader tests

#[test]
fn test_parse_policy_basic() {
    let content = r#"
[policy]
default = "allow"

[[rules]]
type = "prefix"
command = ["git", "status"]
decision = "allow"
"#;
    let policy = parse_policy(content).unwrap();
    let eval = policy.check(&cmd(&["git", "status"]));
    assert!(eval.is_allowed());
}

#[test]
fn test_parse_policy_with_forbidden() {
    let content = r#"
[policy]
default = "prompt"

[[rules]]
type = "prefix"
command = ["rm", "-rf"]
decision = "forbidden"
"#;
    let policy = parse_policy(content).unwrap();
    let eval = policy.check(&cmd(&["rm", "-rf", "/"]));
    assert!(eval.is_forbidden());
}

#[test]
fn test_parse_policy_exact_rule() {
    let content = r#"
[[rules]]
type = "exact"
command = ["echo", "hello"]
decision = "allow"
"#;
    let policy = parse_policy(content).unwrap();
    let eval = policy.check(&cmd(&["echo", "hello"]));
    assert!(eval.is_allowed());

    let eval = policy.check(&cmd(&["echo", "world"]));
    assert!(eval.requires_prompt()); // default
}

#[test]
fn test_parse_policy_glob_rule() {
    let content = r#"
[[rules]]
type = "glob"
program = "npm"
decision = "allow"
"#;
    let policy = parse_policy(content).unwrap();
    let eval = policy.check(&cmd(&["npm", "install"]));
    assert!(eval.is_allowed());

    let eval = policy.check(&cmd(&["yarn"]));
    assert!(eval.requires_prompt());
}

#[test]
fn test_parse_policy_mixed_rules() {
    let content = r#"
[policy]
default = "forbidden"

[[rules]]
type = "prefix"
command = ["git"]
decision = "allow"

[[rules]]
type = "exact"
command = ["cargo", "build"]
decision = "allow"

[[rules]]
type = "glob"
program = "ls"
decision = "allow"
"#;
    let policy = parse_policy(content).unwrap();

    assert!(policy.check(&cmd(&["git", "status"])).is_allowed());
    assert!(policy.check(&cmd(&["cargo", "build"])).is_allowed());
    assert!(policy.check(&cmd(&["ls", "-la"])).is_allowed());
    assert!(policy.check(&cmd(&["rm"])).is_forbidden()); // default
}

#[test]
fn test_parse_policy_empty() {
    let content = "";
    let policy = parse_policy(content).unwrap();
    let eval = policy.check(&cmd(&["anything"]));
    assert!(eval.requires_prompt()); // default
}

#[test]
fn test_parse_policy_invalid_toml() {
    let content = "invalid toml [[[";
    assert!(parse_policy(content).is_err());
}
