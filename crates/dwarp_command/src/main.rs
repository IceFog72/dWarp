use clap::Parser;

#[derive(Parser)]
#[command(about = "Run a command visibly in dWarp and return its output and exit status")]
struct Args {
    #[command(flatten)]
    run: dwarp_command::RunArgs,
}

fn main() {
    let args = Args::parse();
    let code = match dwarp_command::run(&args.run, &[]) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("dwarp-run: {error:#}");
            125
        }
    };
    std::process::exit(code);
}
