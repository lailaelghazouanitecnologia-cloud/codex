mod decision;
mod error;
mod pattern;
mod policy;
mod rule;

pub use decision::Decision;
pub use error::{PolicyError, PolicyResult};
pub use pattern::{PatternToken, CommandPattern};
pub use policy::{Policy, Evaluation, default_heuristics};
pub use rule::{Rule, RuleRef, PrefixRule, ExactRule, GlobRule, RuleMatch};

#[cfg(test)]
mod tests {
    use super::*;

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
}
