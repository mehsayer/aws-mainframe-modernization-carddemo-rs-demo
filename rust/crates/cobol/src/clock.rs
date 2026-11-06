//! The clock behind FUNCTION CURRENT-DATE and EXEC CICS ASKTIME, batch and online alike.
//! `CARDDEMO_NOW` freezes it for reproducible runs.

use chrono::NaiveDateTime;

/// The current local time, or the frozen test clock (see [`fixed_now`]).
pub fn now() -> NaiveDateTime {
    fixed_now().unwrap_or_else(|| chrono::Local::now().naive_local())
}

/// The test clock: `CARDDEMO_NOW` when it is set and valid (`YYYY-MM-DDTHH:MM:SS[.ff]`), else None.
pub fn fixed_now() -> Option<NaiveDateTime> {
    let s = std::env::var("CARDDEMO_NOW").ok()?;
    ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S"].iter().find_map(|f| NaiveDateTime::parse_from_str(&s, f).ok())
}
