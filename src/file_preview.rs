//! File-type routing and tool settings come from npicker's `nfm-preview`.
//! Replace `Configuration.file_preview_resolver` with any closure that returns
//! a plan to change routing or commands.
pub use nfm_preview::{
    CommandOutput, CommandSpec, FilePreviewResolver, MediaPreviewSettings, PreviewPlan,
    ResolvedPreviewProvider, resolver,
};
