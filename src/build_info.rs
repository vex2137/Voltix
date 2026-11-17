//! Build provenance displayed in the actual root and About screen.
pub const SOURCE_COMMIT: &str = env!("VOLTIX_SOURCE_COMMIT");
pub const UI_REVISION: &str = "Mobile UI R2.1";
pub const BUILD_IDENTIFIER: &str = concat!("Mobile UI R2.1 / ", env!("VOLTIX_SOURCE_COMMIT"));
