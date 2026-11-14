use teloxide::{ApiError, RequestError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotErrorAction {
    IdempotentNoOp,
    IdempotentSuccess,
    FatalAdminRequired,
    BugInvalidUserId,
    UserBlockedBot,
    DmWindowExpired,
    RateLimited { retry_after_secs: u64 },
    TransientOrOther,
}

use BotErrorAction::{
    BugInvalidUserId, DmWindowExpired, FatalAdminRequired, IdempotentNoOp, IdempotentSuccess,
    RateLimited, TransientOrOther, UserBlockedBot,
};

#[must_use]
pub fn classify_error_message(msg: &str) -> BotErrorAction {
    let patterns = [
        ("HIDE_REQUESTER_MISSING", IdempotentNoOp),
        ("USER_ALREADY_PARTICIPANT", IdempotentSuccess),
        ("CHAT_ADMIN_REQUIRED", FatalAdminRequired),
        ("RIGHTS_INSUFFICIENT", FatalAdminRequired),
        ("USER_ID_INVALID", BugInvalidUserId),
        ("bot was blocked by the user", UserBlockedBot),
        ("USER_IS_BLOCKED", UserBlockedBot),
        ("bot can't initiate conversation", DmWindowExpired),
    ];
    for (pat, action) in patterns {
        if msg.contains(pat) {
            return action;
        }
    }
    if let Some(pos) = msg.to_ascii_lowercase().find("retry after") {
        let digits: String = msg[pos + 11..]
            .chars()
            .filter(char::is_ascii_digit)
            .collect();
        return RateLimited {
            retry_after_secs: digits.parse().unwrap_or(1),
        };
    }
    TransientOrOther
}

#[must_use]
pub fn classify_request_error(err: &RequestError) -> BotErrorAction {
    match err {
        RequestError::RetryAfter(s) => RateLimited {
            retry_after_secs: u64::from(s.seconds()),
        },
        RequestError::Api(ApiError::BotBlocked) => UserBlockedBot,
        RequestError::Api(ApiError::CantInitiateConversation) => DmWindowExpired,
        RequestError::Api(ApiError::Unknown(s)) => classify_error_message(s),
        RequestError::Api(other) => classify_error_message(&other.to_string()),
        _ => classify_error_message(&err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::types::Seconds;

    #[test]
    fn test_classify_error_message() {
        let (dm, r42) = (
            "bot can't initiate conversation with a user",
            RateLimited {
                retry_after_secs: 42,
            },
        );
        for (msg, exp) in [
            ("HIDE_REQUESTER_MISSING", IdempotentNoOp),
            ("USER_ALREADY_PARTICIPANT", IdempotentSuccess),
            ("CHAT_ADMIN_REQUIRED", FatalAdminRequired),
            ("RIGHTS_INSUFFICIENT", FatalAdminRequired),
            ("USER_ID_INVALID", BugInvalidUserId),
            ("bot was blocked by the user", UserBlockedBot),
            ("USER_IS_BLOCKED", UserBlockedBot),
            (dm, DmWindowExpired),
            ("retry after 42", r42),
            ("other", TransientOrOther),
        ] {
            assert_eq!(classify_error_message(msg), exp);
        }
    }

    #[test]
    fn test_classify_request_error() {
        let s9 = RequestError::RetryAfter(Seconds::from_seconds(9));
        let blk = RequestError::Api(ApiError::BotBlocked);
        let cant = RequestError::Api(ApiError::CantInitiateConversation);
        let unk = RequestError::Api(ApiError::Unknown("HIDE_REQUESTER_MISSING".into()));
        let r9 = RateLimited {
            retry_after_secs: 9,
        };
        assert_eq!(classify_request_error(&s9), r9);
        assert_eq!(classify_request_error(&blk), UserBlockedBot);
        assert_eq!(classify_request_error(&cant), DmWindowExpired);
        assert_eq!(classify_request_error(&unk), IdempotentNoOp);
    }
}
