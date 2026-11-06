//! Sentry crash reporting for the desktop app.
//!
//! DSN resolution order:
//! 1. `SENTRY_DSN` environment variable at runtime (e.g. in CI or on a dev machine).
//! 2. `SENTRY_DSN` environment variable at compile time (`option_env!`), baked into the binary.
//!
//! If no DSN is found the SDK initializes disabled and the app works as before —
//! local `app.log` / `sennheiser-control-error.log` are still written.
//!
//! Privacy: enabled by default (opt-out in Settings → Privacy), captures panics and
//! `log::error!` messages only, no PII (`send_default_pii(false)`), no tracing.

use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(true);

/// DSN baked in at compile time via `SENTRY_DSN=... cargo build`, if present.
pub const COMPILE_TIME_DSN: Option<&str> = option_env!("SENTRY_DSN");

/// Prefer the runtime env var so a shipped binary can be pointed at a
/// different project without rebuilding; fall back to the baked-in value.
pub fn resolve_dsn() -> Option<String> {
    std::env::var("SENTRY_DSN")
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .or_else(|| COMPILE_TIME_DSN.map(|s| s.to_owned()))
        .filter(|s| !s.is_empty())
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Live toggle backing Settings → Privacy. Events already queued may still
/// flush; future events (including panics via `before_send`) are dropped.
pub fn set_enabled(v: bool) {
    ENABLED.store(v, Ordering::Relaxed);
}

/// Initialize Sentry. Must be called before `init_logging` so the panic hook
/// installed there chains onto Sentry's panic handler (via `take_hook`).
/// Returns the client guard — keep it alive for the lifetime of the process.
pub fn init(enabled: bool, debug: bool, sim: bool) -> Option<sentry::ClientInitGuard> {
    set_enabled(enabled);
    let dsn = resolve_dsn();

    let environment: &str = if sim {
        "sim"
    } else if cfg!(debug_assertions) || debug {
        "development"
    } else {
        "production"
    };

    let mut opts = sentry::ClientOptions::new();
    opts = opts
        .maybe_release(sentry::release_name!())
        .environment(environment)
        .max_breadcrumbs(50)
        .attach_stacktrace(true)
        .send_default_pii(false)
        .sample_rate(1.0)
        .debug(debug)
        .before_send(|mut event| {
            if !ENABLED.load(Ordering::Relaxed) {
                return None;
            }
            // Opt-out promise: no hostnames, no user data.
            event.server_name = None;
            event.user = None;
            Some(event)
        });

    // Only call the parsing `.dsn()` builder when we have a value: it panics
    // on parse failure, and we prefer to run disabled over crashing here.
    if let Some(dsn) = dsn.as_deref() {
        // Validate without panicking; a bad DSN means disabled client.
        if dsn.parse::<sentry::types::Dsn>().is_ok() {
            opts = opts.dsn(dsn);
        } else {
            log::warn!("invalid SENTRY_DSN, crash reporting disabled");
        }
    }

    // `sentry::init` with no DSN yields a disabled (no-op) client; the guard
    // still needs to be kept alive.
    let guard = sentry::init(opts);

    if guard.is_enabled() {
        sentry::configure_scope(|scope| {
            scope.set_tag("app", crate::APP_NAME);
            scope.set_tag("sim", if sim { "true" } else { "false" });
        });
    }
    Some(guard)
}

/// Record a breadcrumb for warnings (no event on its own).
pub fn breadcrumb(message: &str, level: sentry::Level) {
    if !is_enabled() {
        return;
    }
    sentry::add_breadcrumb(sentry::Breadcrumb {
        message: Some(message.to_owned()),
        level,
        ..Default::default()
    });
}

/// Send an error-level log line as a Sentry message event.
/// Skips the panic-hook's own log line — the panic integration already
/// reports the panic itself, so capturing both would double-report.
pub fn capture_error_log(line: &str) {
    if !is_enabled() || line.contains("crash in thread") {
        return;
    }
    sentry::capture_message(line, sentry::Level::Error);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsn_empty_env_is_ignored() {
        let prev = std::env::var("SENTRY_DSN").ok();
        unsafe {
            std::env::set_var("SENTRY_DSN", "   ");
        }
        // Empty/whitespace resolves to compile-time value (usually None locally).
        let got = resolve_dsn();
        assert!(got.is_none() || got == COMPILE_TIME_DSN.map(|s| s.to_owned()));
        unsafe {
            if let Some(v) = prev {
                std::env::set_var("SENTRY_DSN", v);
            } else {
                std::env::remove_var("SENTRY_DSN");
            }
        }
    }

    #[test]
    fn settings_default_enables_crash_reporting() {
        // Old settings.json files predate the field; they must opt-out, not opt-in.
        let s: crate::settings::Settings = serde_json::from_str("{}").unwrap();
        assert!(s.crash_reporting);
        assert!(crate::settings::Settings::default().crash_reporting);
    }
}
