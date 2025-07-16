use netsuke::{cli::Cli, runner};

fn main() {
    let cli = Cli::parse_with_default();
    runner::run(cli);
}
