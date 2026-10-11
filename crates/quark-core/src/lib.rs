//! 夸克网盘协议层：只懂夸克，不懂下载（ADR-0001）。

pub mod cookie;
pub mod protocol;
pub mod login;

pub use cookie::{account_nickname, account_state_is_logged_in, is_auth_failure, CookieJar};
pub use reqwest::Method;
