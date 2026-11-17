//! Linux-only preference. An explicit SLINT_BACKEND (including an empty value)
//! belongs to the user and is left entirely to Slint.
const RETRY_FLAG: &str = "VOLTIX_RENDERER_RETRY";

pub fn initialize() -> Result<(), slint::PlatformError> {
    if std::env::var_os("SLINT_BACKEND").is_some() {
        crate::diagnostics::mark("renderer: explicit SLINT_BACKEND respected");
        return Ok(());
    }
    if std::env::var_os(RETRY_FLAG).is_some() {
        crate::diagnostics::mark("renderer: fresh-process automatic fallback selected");
        return slint::BackendSelector::new().select();
    }
    let preferred = with_fallback(
        || {
            slint::BackendSelector::new()
                .backend_name("winit".into())
                .renderer_name("software".into())
                .select()
        },
        || slint::BackendSelector::new().select(),
    )?;
    crate::diagnostics::mark(if preferred {
        "renderer: Linux software preference selected"
    } else {
        "renderer: Slint automatic fallback selected"
    });
    Ok(())
}

fn may_retry(explicit_backend: bool, already_retried: bool) -> bool {
    !explicit_backend && !already_retried
}

fn retry_command(
    executable: std::path::PathBuf,
    args: impl Iterator<Item = std::ffi::OsString>,
) -> std::process::Command {
    let mut command = std::process::Command::new(executable);
    command.args(args).env(RETRY_FLAG, "1");
    command
}

/// Backend selection is not the last fallible initialization step: the surface
/// is created on resume/show. Winit only permits one event loop per process, so
/// retrying there requires exec, not another BackendSelector in the same process.
/// app::run has already stopped/joined its monitor thread before returning an error.
pub fn retry_after_error(error: &impl std::fmt::Display) {
    if !may_retry(
        std::env::var_os("SLINT_BACKEND").is_some(),
        std::env::var_os(RETRY_FLAG).is_some(),
    ) {
        return;
    }
    log::warn!("Default renderer failed: {error}; retrying Slint automatic selection once in a fresh process");
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            log::error!("Cannot locate executable for renderer retry: {error}");
            return;
        }
    };
    use std::os::unix::process::CommandExt;
    let error = retry_command(executable, std::env::args_os().skip(1)).exec();
    log::error!("Cannot restart for renderer fallback: {error}");
}

fn with_fallback<E: std::fmt::Display>(
    preferred: impl FnOnce() -> Result<(), E>,
    fallback: impl FnOnce() -> Result<(), E>,
) -> Result<bool, String> {
    if let Err(error) = preferred() {
        log::warn!(
            "Software backend initialization failed: {error}; trying Slint automatic selection"
        );
        fallback()
            .map_err(|next| format!("Software backend: {error}; automatic fallback: {next}"))?;
        return Ok(false);
    }
    // Winit also retries its other compiled renderers if creating the window's
    // preferred renderer fails. Do not initialize a GPU just to probe it here.
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn success_does_not_probe_gpu_and_failure_tries_fallback() {
        assert!(with_fallback(|| Ok::<_, &str>(()), || panic!("unnecessary GPU probe")).unwrap());
        assert!(!with_fallback(|| Err("software unavailable"), || Ok(())).unwrap());
        let error =
            with_fallback(|| Err("software failed"), || Err("display unavailable")).unwrap_err();
        assert!(error.contains("software failed") && error.contains("display unavailable"));
    }
    #[test]
    fn retry_is_bounded_and_never_replaces_an_explicit_backend() {
        assert!(may_retry(false, false));
        assert!(!may_retry(true, false));
        assert!(!may_retry(false, true));
        assert!(!may_retry(true, true));
        let command = retry_command(
            "/path with spaces/voltix".into(),
            ["--argument with spaces".into()].into_iter(),
        );
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![std::ffi::OsStr::new("--argument with spaces")]
        );
        let changes = command.get_envs().collect::<Vec<_>>();
        assert_eq!(
            changes,
            vec![(
                std::ffi::OsStr::new(RETRY_FLAG),
                Some(std::ffi::OsStr::new("1"))
            )]
        );
    }
}
