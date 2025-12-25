use tauri_plugin_gibberish_tools::FunctionGemmaRunner;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let model_path = args
        .next()
        .ok_or("usage: run_functiongemma_once <model.onnx>")?;
    let tokenizer_path = args
        .next()
        .ok_or("usage: run_functiongemma_once <model.onnx> <tokenizer.json>")?;
    let text = args
        .next()
        .unwrap_or_else(|| "Are you in Madrid today?".to_string());

    let runner = FunctionGemmaRunner::load(model_path, tokenizer_path)?;

    let function_declarations = r#"<start_function_declaration>declaration:wikipedia_city_lookup{description:<escape>Lookup a city on Wikipedia and return a short summary and URL.<escape>,parameters:{properties:{city:{type:<escape>STRING<escape>},lang:{type:<escape>STRING<escape>},sentences:{type:<escape>INTEGER<escape>}},required:[<escape>city<escape>],type:<escape>OBJECT<escape>}}<end_function_declaration>
"#;
    let developer_context = format!(
    "You are a model that can do function calling with the following functions\n\
You are an action router that reads live transcript commits.\n\
You do not chat. You never write natural language. You only emit function calls.\n\
\n\
Policy:\n\
- If the text mentions a city (or asks about a city), call wikipedia_city_lookup.\n\
- The city argument must be ONLY the city name (no extra words).\n\
\n\
Examples:\n\
Text: Are you in Madrid today?\n\
Output: <start_function_call>call:wikipedia_city_lookup{{city:<escape>Madrid<escape>}}<end_function_call>\n\
Text: I'm from Barcelona.\n\
Output: <start_function_call>call:wikipedia_city_lookup{{city:<escape>Barcelona<escape>}}<end_function_call>\n\
{function_declarations}"
  );

    let out = runner.infer_once(&developer_context, &text)?;
    println!("raw: {}", out.raw_text);
    println!("proposals: {}", out.proposals.len());
    for p in out.proposals {
        println!("tool={} args={}", p.tool, p.args);
    }

    let args = runner.infer_args_object(&developer_context, "wikipedia_city_lookup", &text)?;
    println!("args_only: {args}");

    Ok(())
}
