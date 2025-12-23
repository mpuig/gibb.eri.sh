use gibberish_parakeet::ParakeetEngine;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <model_dir> <audio_file.wav>", args[0]);
        eprintln!("\nExample:");
        eprintln!(
            "  {} ~/Library/Application\\ Support/gibberish/models/parakeet-ctc-1.1b recording.wav",
            args[0]
        );
        std::process::exit(1);
    }

    let model_dir = &args[1];
    let audio_file = &args[2];

    println!("Loading Parakeet model from: {}", model_dir);
    let engine = match ParakeetEngine::new(model_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to load model: {}", e);
            std::process::exit(1);
        }
    };

    println!("Transcribing: {}", audio_file);
    let result = match engine.transcribe_file(audio_file) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Transcription failed: {}", e);
            std::process::exit(1);
        }
    };

    println!("\n=== Transcription ===");
    println!("{}", result.text);

    if !result.tokens.is_empty() {
        println!("\n=== Word Timestamps ===");
        for token in &result.tokens {
            println!(
                "[{:.2}s - {:.2}s] {}",
                token.start, token.end, token.text
            );
        }
    }
}
