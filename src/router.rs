use crate::profiles::ProfileName;

#[derive(Debug, Clone)]
pub struct RouteDecision {
    pub profile: ProfileName,
    pub reason: String,
}

pub fn choose_profile(
    input: &str,
    explicit_profile: Option<ProfileName>,
    default_profile: ProfileName,
) -> RouteDecision {
    if let Some(profile) = explicit_profile {
        return RouteDecision {
            profile,
            reason: "user override is active".to_string(),
        };
    }

    let lowered = input.to_ascii_lowercase();
    let reasoning_markers = [
        "step by step",
        "reason",
        "debug",
        "plan",
        "compare",
        "tradeoff",
        "design",
        "analyze",
        "investigate",
        "why",
    ];

    if reasoning_markers
        .iter()
        .any(|marker| lowered.contains(marker))
        || input.len() > 240
    {
        return RouteDecision {
            profile: ProfileName::Reasoning,
            reason: "prompt looks multi-step or analytical".to_string(),
        };
    }

    let direct_markers = ["quick", "brief", "short", "one line", "one-liner"];
    if direct_markers.iter().any(|marker| lowered.contains(marker)) || input.len() < 80 {
        return RouteDecision {
            profile: ProfileName::Direct,
            reason: "prompt looks short and direct".to_string(),
        };
    }

    RouteDecision {
        profile: default_profile.clone(),
        reason: format!(
            "fell back to configured default profile '{}'",
            default_profile
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::choose_profile;
    use crate::profiles::ProfileName;

    #[test]
    fn explicit_override_wins() {
        let route = choose_profile("hello", Some(ProfileName::Agent), ProfileName::Direct);
        assert_eq!(route.profile, ProfileName::Agent);
    }

    #[test]
    fn analytical_prompt_goes_to_reasoning() {
        let route = choose_profile(
            "Please compare these designs and explain the tradeoff.",
            None,
            ProfileName::Direct,
        );
        assert_eq!(route.profile, ProfileName::Reasoning);
    }

    #[test]
    fn short_prompt_goes_to_direct() {
        let route = choose_profile("Summarize this", None, ProfileName::Reasoning);
        assert_eq!(route.profile, ProfileName::Direct);
    }
}
