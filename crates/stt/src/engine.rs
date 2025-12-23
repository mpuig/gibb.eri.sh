use std::any::Any;

#[derive(Debug, Clone)]
pub struct Word {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct Segment {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub words: Vec<Word>,
    pub speaker: Option<i32>,
}

pub trait SttEngine: Send + Sync {
    fn transcribe(&self, audio: &[f32]) -> crate::Result<Vec<Segment>>;

    fn is_streaming_capable(&self) -> bool {
        false
    }

    fn model_name(&self) -> &str;

    fn supported_languages(&self) -> Vec<&'static str> {
        vec!["en"]
    }

    fn as_any(&self) -> &dyn Any;
}
