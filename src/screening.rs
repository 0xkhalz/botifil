//! Applicant bio screening for join requests (Bot API 10.1+).

/// Reason why an applicant was declined during screening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreeningReason {
    ContainsUrl,
    ContainsTelegramInvite,
}

/// Verdict after evaluating an applicant's profile and bio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreeningVerdict {
    Allow { is_premium: bool },
    Decline { reason: ScreeningReason },
}

/// Evaluates applicant profile data, screening out spam URLs and invite links in bio.
#[must_use]
pub fn evaluate_applicant(bio: Option<&str>, is_premium: bool) -> ScreeningVerdict {
    let Some(bio) = bio else {
        return ScreeningVerdict::Allow { is_premium };
    };
    if bio.is_empty() {
        return ScreeningVerdict::Allow { is_premium };
    }
    let lower = bio.to_ascii_lowercase();
    if lower.contains("t.me/") || lower.contains("telegram.me/") || lower.contains("tg://") {
        return ScreeningVerdict::Decline {
            reason: ScreeningReason::ContainsTelegramInvite,
        };
    }
    if lower.contains("http://") || lower.contains("https://") {
        return ScreeningVerdict::Decline {
            reason: ScreeningReason::ContainsUrl,
        };
    }
    ScreeningVerdict::Allow { is_premium }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_applicant_none_or_empty_bio() {
        let (a_f, a_t) = (
            ScreeningVerdict::Allow { is_premium: false },
            ScreeningVerdict::Allow { is_premium: true },
        );
        assert_eq!(evaluate_applicant(None, false), a_f);
        assert_eq!(evaluate_applicant(None, true), a_t);
        assert_eq!(evaluate_applicant(Some(""), false), a_f);
        assert_eq!(evaluate_applicant(Some(""), true), a_t);
    }

    #[test]
    fn test_evaluate_applicant_clean_bio() {
        assert_eq!(
            evaluate_applicant(Some("Rust enthusiast and developer"), false),
            ScreeningVerdict::Allow { is_premium: false }
        );
        assert_eq!(
            evaluate_applicant(Some("Just a human being"), true),
            ScreeningVerdict::Allow { is_premium: true }
        );
    }

    #[test]
    fn test_evaluate_applicant_url_decline() {
        let dec = ScreeningVerdict::Decline {
            reason: ScreeningReason::ContainsUrl,
        };
        assert_eq!(
            evaluate_applicant(Some("Check my site http://example.com"), false),
            dec
        );
        assert_eq!(
            evaluate_applicant(Some("Visit HTTPS://SECURE.ORG/HOME"), true),
            dec
        );
    }

    #[test]
    fn test_evaluate_applicant_invite_decline() {
        let dec = ScreeningVerdict::Decline {
            reason: ScreeningReason::ContainsTelegramInvite,
        };
        for invite in [
            "join me at t.me/mygroup",
            "check T.ME/JOINCHAT/XYZ",
            "chat on telegram.me/channel",
            "chat on TELEGRAM.ME/TEST",
            "tg://join?invite=abc",
            "open TG://RESOLVE?domain=xyz",
            "https://t.me/spam_group",
        ] {
            assert_eq!(evaluate_applicant(Some(invite), false), dec);
        }
    }
}
