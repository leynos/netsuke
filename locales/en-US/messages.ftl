# Netsuke CLI localisation resources.

cli.about = Netsuke compiles YAML + Jinja manifests into Ninja build plans.
cli.long_about = Netsuke transforms YAML + Jinja manifests into reproducible Ninja graphs and runs Ninja with safe defaults.
cli.usage = { $usage }

cli.subcommand.build.about = Build targets defined in the manifest (default).
cli.subcommand.build.long_about = Build the requested targets; when none are provided, use the manifest defaults.
cli.subcommand.clean.about = Remove build artefacts via Ninja.
cli.subcommand.clean.long_about = Generate a temporary Ninja file, then run `ninja -t clean`.
cli.subcommand.graph.about = Emit the dependency graph in DOT format.
cli.subcommand.graph.long_about = Generate a temporary Ninja file, then run `ninja -t graph` to emit DOT output.
cli.subcommand.manifest.about = Write the generated Ninja manifest without running Ninja.
cli.subcommand.manifest.long_about = Generate the Ninja file and write it to the specified path or '-' for stdout.

clap-error-missing-argument = Missing required argument: { $argument }
clap-error-missing-subcommand = Missing subcommand. Available options: { $valid_subcommands }
clap-error-unknown-argument = Unknown argument: { $argument }
clap-error-invalid-value = Invalid value for { $argument }: { $value }
clap-error-invalid-subcommand = Unknown subcommand: { $subcommand }
clap-error-value-validation = Invalid value for { $argument }: { $value }
