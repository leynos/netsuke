# Netsuke CLI localisation resources.

cli.about = Netsuke compiles YAML + Jinja manifests into Ninja build plans.
cli.long_about = Netsuke transforms YAML + Jinja manifests into reproducible Ninja graphs and runs Ninja with safe defaults.
cli.usage = { $usage }

# Root-level flag help text.
cli.flag.file.help = Path to the Netsuke manifest file to use.
cli.flag.directory.help = Run as if started in this directory.
cli.flag.jobs.help = Set the number of parallel build jobs.
cli.flag.verbose.help = Enable verbose diagnostic logging.
cli.flag.locale.help = Locale tag for CLI copy (for example: en-US, es-ES).
cli.flag.fetch-allow-scheme.help = Additional URL schemes allowed for the fetch helper.
cli.flag.fetch-allow-host.help = Hostnames that are permitted when default deny is enabled.
cli.flag.fetch-block-host.help = Hostnames that are always blocked, even when allowed elsewhere.
cli.flag.fetch-default-deny.help = Deny all hosts by default; only allow the declared allowlist.

# Subcommand descriptions.
cli.subcommand.build.about = Build targets defined in the manifest (default).
cli.subcommand.build.long_about = Build the requested targets; when none are provided, use the manifest defaults.
cli.subcommand.clean.about = Remove build artefacts via Ninja.
cli.subcommand.clean.long_about = Generate a temporary Ninja file, then run `ninja -t clean`.
cli.subcommand.graph.about = Emit the dependency graph in DOT format.
cli.subcommand.graph.long_about = Generate a temporary Ninja file, then run `ninja -t graph` to emit DOT output.
cli.subcommand.manifest.about = Write the generated Ninja manifest without running Ninja.
cli.subcommand.manifest.long_about = Generate the Ninja file and write it to the specified path or '-' for stdout.

# Build subcommand flag help text.
cli.subcommand.build.flag.emit.help = Write the generated Ninja file to this path and keep it.
cli.subcommand.build.flag.targets.help = Targets to build (uses manifest defaults if omitted).

# Manifest subcommand argument help text.
cli.subcommand.manifest.flag.file.help = Output path for the Ninja file (use '-' for stdout).

# Clap error messages.
clap-error-missing-argument = Missing required argument: { $argument }
clap-error-missing-subcommand = Missing subcommand. Available options: { $valid_subcommands }
clap-error-unknown-argument = Unknown argument: { $argument }
clap-error-invalid-value = Invalid value for { $argument }: { $value }
clap-error-invalid-subcommand = Unknown subcommand: { $subcommand }
clap-error-value-validation = Invalid value for { $argument }: { $value }
