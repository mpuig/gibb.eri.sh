#[derive(Debug, Clone)]
pub struct FunctionGemmaSpec {
    pub variant: &'static str,
    pub size_bytes: u64,
}

pub const FUNCTIONGEMMA_SPECS: &[FunctionGemmaSpec] = &[FunctionGemmaSpec {
    variant: "model_fp16",
    size_bytes: 570_000_000,
}];

pub fn is_supported_variant(variant: &str) -> bool {
    FUNCTIONGEMMA_SPECS.iter().any(|s| s.variant == variant)
}
