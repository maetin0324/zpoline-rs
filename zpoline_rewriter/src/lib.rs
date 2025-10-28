mod maps;
mod rewriter;

pub use maps::{MemoryRegion, parse_proc_maps};
pub use rewriter::{RewriteConfig, Rewriter, RewriteError, RewriteStats};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_maps() {
        // Test will be implemented
    }
}
