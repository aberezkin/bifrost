use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about)]
pub(crate) struct Args {
    #[arg(short, long)]
    pub(crate) config: String,
}
