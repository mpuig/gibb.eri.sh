use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let model_path = args
        .next()
        .ok_or("usage: inspect_functiongemma <model.onnx>")?;

    let session = Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .commit_from_file(model_path)?;

    println!("Inputs:");
    for i in &session.inputs {
        println!("- {}: {:?}", i.name, i.input_type);
    }

    println!("\nOutputs:");
    for o in &session.outputs {
        println!("- {}: {:?}", o.name, o.output_type);
    }

    Ok(())
}
