//! Linux-only preference. An explicit SLINT_BACKEND (including an empty value)
//! belongs to the user and is left entirely to Slint.
pub fn initialize() -> Result<(), slint::PlatformError> {
    if std::env::var_os("SLINT_BACKEND").is_some() {
        crate::diagnostics::mark("renderer: explicit SLINT_BACKEND respected");
        return Ok(());
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
}
