//! Single source of "now" for sync cursors, fetch throttles and local-write
//! stamps. Everything compares these values to each other, so they must all
//! come from the same clock.

pub(crate) fn now_epoch() -> i64 {
    chrono::Utc::now().timestamp()
}
