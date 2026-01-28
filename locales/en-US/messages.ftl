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
cli.flag.fetch_allow_scheme.help = Additional URL schemes allowed for the fetch helper.
cli.flag.fetch_allow_host.help = Hostnames that are permitted when default deny is enabled.
cli.flag.fetch_block_host.help = Hostnames that are always blocked, even when allowed elsewhere.
cli.flag.fetch_default_deny.help = Deny all hosts by default; only allow the declared allowlist.

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

# CLI validation errors.
cli.validation.jobs.invalid_number = { $value } is not a valid number.
cli.validation.jobs.out_of_range = Jobs must be between { $min } and { $max }.
cli.validation.scheme.empty = Scheme must not be empty.
cli.validation.scheme.invalid_start = Scheme '{ $scheme }' must start with an ASCII letter.
cli.validation.scheme.invalid = Invalid scheme '{ $scheme }'.
cli.validation.locale.empty = Locale must not be empty.
cli.validation.locale.invalid = Invalid locale '{ $locale }'.
cli.validation.config.expected_object = Expected parsed CLI values to serialize to an object, got { $value }.

# Clap error messages.
clap-error-missing-argument = Missing required argument: { $argument }
clap-error-missing-subcommand = Missing subcommand. Available options: { $valid_subcommands }
clap-error-unknown-argument = Unknown argument: { $argument }
clap-error-invalid-value = Invalid value for { $argument }: { $value }
clap-error-invalid-subcommand = Unknown subcommand: { $subcommand }
# Note: value-validation uses distinct wording from invalid-value to differentiate
# custom validator failures (ErrorKind::ValueValidation) from type mismatches
# (ErrorKind::InvalidValue).
clap-error-value-validation = Validation failed for { $argument }: { $value }

# Runner errors and contexts.
runner.manifest.not_found = Manifest '{ $manifest_name }' not found in { $directory }.
runner.manifest.not_found.help = Ensure the manifest exists or pass `--file` with the correct path.
runner.manifest.path_missing_name = Manifest path '{ $path }' has no file name.
runner.manifest.path_utf8 = Manifest path '{ $path }' is not valid UTF-8.
runner.manifest.directory_utf8 = Manifest directory path '{ $path }' is not valid UTF-8.
runner.manifest.directory_label = directory `{ $directory }`
runner.manifest.current_directory_label = the current directory
runner.context.network_policy = Failed to build the network policy.
runner.context.load_manifest = Failed to load manifest at { $path }.
runner.context.serialise_manifest = Failed to serialise manifest.
runner.context.build_graph = Failed to build graph from the manifest.
runner.context.generate_ninja = Failed to generate the Ninja manifest.

runner.io.create_temp_file = Failed to create temporary Ninja file.
runner.io.write_temp_ninja = Failed to write temporary Ninja file.
runner.io.flush_temp_ninja = Failed to flush temporary Ninja file.
runner.io.sync_temp_ninja = Failed to sync temporary Ninja file.
runner.io.create_parent_dir = Failed to create parent directory { $path }.
runner.io.create_ninja_file = Failed to create Ninja file at { $path }.
runner.io.write_ninja_file = Failed to write Ninja file at { $path }.
runner.io.flush_ninja_file = Failed to flush Ninja file at { $path }.
runner.io.sync_ninja_file = Failed to sync Ninja file at { $path }.
runner.io.open_ambient_dir = Failed to open ambient directory.
runner.io.no_existing_ancestor = No existing ancestor directory for { $path }.
runner.io.derive_relative_path = Failed to derive relative Ninja path.
runner.io.non_utf8_path = Non-UTF-8 path is not supported (path: { $path }).
runner.io.write_stdout = Failed to write Ninja manifest to stdout.
runner.io.flush_stdout = Failed to flush stdout.

# Manifest diagnostics.
manifest.parse = Manifest parse failed.
manifest.structure_error = Manifest structure error in { $name }: { $details }
manifest.yaml.parse = YAML parse error at line { $line }, column { $column }: { $details }
manifest.yaml.label = invalid YAML
manifest.yaml.hint.tabs = YAML does not permit tabs; use spaces for indentation.
manifest.yaml.hint.list_item = YAML list items must start with a '-' and be properly indented.
manifest.yaml.hint.expected_colon = This looks like a mapping entry; missing a ':' after the key.
manifest.yaml.hint.mapping_values = YAML mappings require values after ':' (or a nested block).
manifest.yaml.hint.invalid_token = YAML token is invalid or unexpected.
manifest.yaml.hint.escape = Escape backslashes or remove invalid escape sequences.
manifest.env.missing = Required environment variable '{ $name }' is not set.
manifest.env.invalid_utf8 = Environment variable '{ $name }' contains invalid UTF-8.
manifest.vars.not_object = Manifest `vars` must be a map/object.
manifest.read_failed = Failed to read manifest at { $path }.
manifest.resolve_workspace_root = Failed to resolve workspace root.
manifest.workspace_non_utf8 = Workspace root path '{ $path }' is not valid UTF-8.
manifest.path_non_utf8 = Manifest '{ $manifest }' path is not valid UTF-8: { $path }.
manifest.open_workspace_failed = Failed to open workspace { $workspace } for manifest { $manifest }.
manifest.foreach.not_iterable = `foreach` expression is not iterable.
manifest.foreach.serialise_item = Failed to serialise foreach item.
manifest.when.empty = `when` expression must not be empty.
manifest.when.eval_error = Failed to evaluate `when` expression '{ $expr }'.
manifest.when.template_error = Failed to render `when` template '{ $expr }'.
manifest.target.vars_not_object = Target `vars` must be an object, got { $value }.
manifest.vars.entry_not_object = Manifest `vars` entry must be an object.
manifest.field_not_string = Field '{ $field }' must be a string.
manifest.expression.parse_error = Failed to parse { $name } expression.
manifest.expression.eval_error = Failed to evaluate { $name } expression.

# Manifest macro diagnostics.
manifest.macro.signature_missing_identifier = Macro signature is missing an identifier.
manifest.macro.signature_missing_params = Macro signature is missing parameters.
manifest.macro.compile_failed = Failed to compile macro { $name }.
manifest.macro.sequence_invalid = Macros must be defined as a mapping of names to templates.
manifest.macro.register_failed = Failed to register manifest macros.
manifest.macro.not_initialised = Macro environment is not initialised.
manifest.macro.caller_invalid = Macro caller must be a string.
manifest.macro.template_load_failed = Failed to load macro template.
manifest.macro.init_failed = Failed to initialise macro environment.
manifest.macro.missing = Macro { $name } is missing.

# Manifest glob errors.
manifest.glob.unmatched_brace = Invalid glob pattern '{ $pattern }': unmatched '{ $character }' at position { $position }.
manifest.glob.invalid_pattern = Invalid glob pattern '{ $pattern }': { $detail }.
manifest.glob.unknown_pattern_error = unknown pattern error.
manifest.glob.io_failed = Glob failed for '{ $pattern }': { $detail }.
manifest.glob.unknown_io_error = unknown IO error.

# IR errors.
ir.rule_not_found = Rule '{ $rule }' referenced by target '{ $target }' was not found.
ir.multiple_rules = Target '{ $target }' must reference a single rule, got { $rules }.
ir.empty_rule = Target '{ $target }' must reference a rule.
ir.duplicate_outputs = Duplicate outputs detected: { $outputs }.
ir.circular_dependency = Circular dependency detected: { $cycle }.
ir.action_serialisation = Failed to serialise action: { $details }.
ir.invalid_command = Invalid command interpolation: { $snippet }.

# Ninja generation errors.
ninja_gen.missing_action = Missing action '{ $id }' referenced by a build edge.
ninja_gen.format = Failed to format the Ninja manifest output.

# Host pattern validation.
host_pattern.empty = Host pattern must not be empty.
host_pattern.contains_scheme = Host pattern '{ $pattern }' must not include a URL scheme.
host_pattern.contains_slash = Host pattern '{ $pattern }' must not include '/'.
host_pattern.missing_suffix = Host pattern '{ $pattern }' must include a suffix after '*.'.
host_pattern.empty_label = Host pattern '{ $pattern }' contains an empty label.
host_pattern.invalid_chars = Host pattern '{ $pattern }' contains invalid characters.
host_pattern.invalid_label_edge = Host pattern '{ $pattern }' labels must not start or end with '-'.
host_pattern.label_too_long = Host pattern '{ $pattern }' contains a label longer than 63 characters.
host_pattern.too_long = Host pattern '{ $pattern }' exceeds the 255 character limit.

# Network policy.
network_policy.scheme.empty = Scheme must not be empty.
network_policy.scheme.invalid = Scheme '{ $scheme }' contains invalid characters.
network_policy.allowlist.empty = Host allowlist must not be empty.
network_policy.scheme.not_allowed = Scheme '{ $scheme }' is not allowed.
network_policy.missing_host = URL is missing a host.
network_policy.host.blocked = Host '{ $host }' is blocked by policy.
network_policy.host.not_allowlisted = Host '{ $host }' is not on the allowlist.

# Stdlib configuration.
stdlib.config.default_fetch_cache_invalid = Default fetch cache path must be relative.
stdlib.config.default_which_cache_invalid = Default which cache capacity must be positive.
stdlib.config.workspace_root_absolute = Workspace root path must be absolute.
stdlib.config.fetch_response_limit_positive = Fetch response limit must be positive.
stdlib.config.command_output_limit_positive = Command output capture limit must be positive.
stdlib.config.command_stream_limit_positive = Command stream limit must be positive.
stdlib.config.which_cache_capacity_positive = Which cache capacity must be positive.
stdlib.config.skip_dir_empty = Skip directory entries must not be empty.
stdlib.config.skip_dir_navigation = Skip directory entries must not contain '..'.
stdlib.config.skip_dir_separator = Skip directory entries must not contain path separators.
stdlib.config.fetch_cache_empty = Fetch cache path must not be empty.
stdlib.config.fetch_cache_not_relative = Fetch cache path must be relative, got { $path }.
stdlib.config.fetch_cache_escapes = Fetch cache path must not escape the workspace: { $path }.

# Fetch helper diagnostics.
stdlib.fetch.url_invalid = Invalid URL '{ $url }': { $details }.
stdlib.fetch.disallowed = URL '{ $url }' is disallowed: { $details }.
stdlib.fetch.failed = Failed to fetch '{ $url }': { $details }.
stdlib.fetch.cache_read_failed = Failed to read fetch cache entry '{ $name }': { $details }.
stdlib.fetch.cache_open_failed = Failed to open fetch cache entry '{ $name }': { $details }.
stdlib.fetch.response_read_failed = Failed to read response from '{ $url }': { $details }.
stdlib.fetch.response_buffer_overflow = Response buffer overflow while reading '{ $url }'.
stdlib.fetch.cache_write_failed = Failed to write cache for '{ $url }': { $details }.
stdlib.fetch.response_limit_exceeded = Response from '{ $url }' exceeded limit { $limit } bytes.
stdlib.fetch.cache_limit_exceeded = Cached response '{ $name }' exceeded limit { $limit } bytes.
stdlib.fetch.io_failed = { $action } failed for { $path }: { $details }.
stdlib.fetch.action.sync_cache = sync fetch cache
stdlib.fetch.action.create_cache_dir = create fetch cache directory
stdlib.fetch.action.open_cache_dir = open fetch cache directory
stdlib.fetch.action.stat_cache = stat fetch cache entry
stdlib.fetch.action.open_cache_entry = open fetch cache entry

# Command helper diagnostics.
stdlib.command.location = command '{ $command }' in template '{ $template }'
stdlib.command.spawn_failed = Failed to spawn { $location }: { $details }.
stdlib.command.io_failed = { $location } failed: { $details }.
stdlib.command.closed_input_early = Input closed early while writing to the command.
stdlib.command.broken_pipe = Broken pipe while running { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } terminated by signal.
stdlib.command.exited_with_status = { $location } exited with status { $status }.
stdlib.command.output_limit_exceeded = { $location } exceeded { $mode } { $stream } limit of { $limit } bytes.
stdlib.command.timeout = { $location } timed out after { $seconds } seconds.
stdlib.command.exit_status_suffix = (exit status { $status })
stdlib.command.signal_suffix = (terminated by signal)
stdlib.command.shell.empty = Shell command must not be empty.
stdlib.command.grep.empty_pattern = Grep pattern must not be empty.
stdlib.command.grep.flags_not_string = Grep flags must be strings.
stdlib.command.quote.invalid = Failed to quote { $arg }: { $details }.
stdlib.command.quote.line_break = Arguments containing carriage returns or line feeds cannot be safely quoted.
stdlib.command.input_undefined = Input value is undefined.
stdlib.command.tempfile.root_required = Workspace root is required to create command temp files.
stdlib.command.tempfile.create_failed = Failed to create command tempfile: { $details }.
stdlib.command.options.invalid_utf8 = Command option key must be valid UTF-8.
stdlib.command.option.mode_not_string = Output mode must be a string.
stdlib.command.options.invalid_type = Command options must be an object.
stdlib.command.output.mode_unsupported = Unsupported output mode '{ $mode }'.
stdlib.command.output.mode.capture = capture
stdlib.command.output.mode.streaming = streaming
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Path helper diagnostics.
stdlib.path.io.failed = { $action } failed for { $path } ({ $label }).
stdlib.path.io.failed_with_detail = { $action } failed for { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $action } failed for { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = not found
stdlib.path.io.permission_denied = permission denied
stdlib.path.io.already_exists = already exists
stdlib.path.io.invalid_input = invalid input
stdlib.path.io.invalid_data = invalid data
stdlib.path.io.timed_out = timed out
stdlib.path.io.interrupted = interrupted
stdlib.path.io.would_block = would block
stdlib.path.io.write_zero = write zero
stdlib.path.io.unexpected_eof = unexpected end of file
stdlib.path.io.broken_pipe = broken pipe
stdlib.path.io.connection_refused = connection refused
stdlib.path.io.connection_reset = connection reset
stdlib.path.io.connection_aborted = connection aborted
stdlib.path.io.not_connected = not connected
stdlib.path.io.addr_in_use = address in use
stdlib.path.io.addr_not_available = address not available
stdlib.path.io.out_of_memory = out of memory
stdlib.path.io.unsupported = unsupported
stdlib.path.io.file_too_large = file too large
stdlib.path.io.resource_busy = resource busy
stdlib.path.io.executable_busy = executable busy
stdlib.path.io.deadlock = deadlock
stdlib.path.io.crosses_devices = crosses devices
stdlib.path.io.too_many_links = too many links
stdlib.path.io.invalid_filename = invalid filename
stdlib.path.io.arg_list_too_long = argument list too long
stdlib.path.io.stale_handle = stale network file handle
stdlib.path.io.storage_full = storage full
stdlib.path.io.not_seekable = not seekable
stdlib.path.io.network_down = network down
stdlib.path.io.network_unreachable = network unreachable
stdlib.path.io.host_unreachable = host unreachable
stdlib.path.io.other = io error
stdlib.path.action.canonicalise = canonicalise
stdlib.path.action.open_directory = open directory
stdlib.path.action.stat = stat
stdlib.path.action.read = read
stdlib.path.action.open_file = open file
stdlib.path.with_suffix.empty_separator = with_suffix requires a non-empty separator.
stdlib.path.relative_to.mismatch = { $path } is not relative to { $root }.
stdlib.path.expanduser.unsupported = User-specific ~ expansion is unsupported.
stdlib.path.expanduser.no_home = Cannot expand ~: no home directory environment variables are set.
stdlib.path.contents.unsupported_encoding = Unsupported encoding '{ $encoding }'.
stdlib.path.hash.unsupported_algorithm = Unsupported hash algorithm '{ $algorithm }'.
stdlib.path.hash.unsupported_algorithm_legacy = Unsupported hash algorithm '{ $algorithm }' (enable feature '{ $feature }').

# Collection helper diagnostics.
stdlib.collections.flatten.expected_sequence = Flatten expected sequence items but found { $kind }.
stdlib.collections.group_by.empty_attribute = group_by requires a non-empty attribute.
stdlib.collections.group_by.unresolved = group_by could not resolve '{ $attr }' on item of kind { $kind }.

# Time helper diagnostics.
stdlib.time.offset.invalid = now offset '{ $offset }' is invalid: expected '+HH:MM[:SS]' or 'Z'.
stdlib.time.timedelta.overflow = timedelta overflow when adding { $component }.
stdlib.time.label.weeks = weeks
stdlib.time.label.days = days
stdlib.time.label.hours = hours
stdlib.time.label.minutes = minutes
stdlib.time.label.seconds = seconds
stdlib.time.label.milliseconds = milliseconds
stdlib.time.label.microseconds = microseconds
stdlib.time.label.nanoseconds = nanoseconds

# Which helper diagnostics.
stdlib.which.not_found = [netsuke::jinja::which::not_found] command '{ $command }' not found after checking { $count } PATH entries. Preview: { $preview }
stdlib.which.not_found.hint.cwd_auto = Empty PATH segments are ignored; use cwd_mode="auto" to include the working directory.
stdlib.which.not_found.hint.cwd_always = Set cwd_mode="always" to include the current directory.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] command '{ $command }' at '{ $path }' is missing or not executable.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <empty>
stdlib.which.path_entry.non_utf8 = PATH entry #{ $index } contains non-UTF-8 characters; Netsuke requires UTF-8 paths.
stdlib.which.command.empty = which requires a non-empty string.
stdlib.which.cwd_mode.invalid = cwd_mode must be 'auto', 'always', or 'never', got '{ $mode }'.
stdlib.which.cwd.resolve_failed = Failed to resolve current directory: { $details }.
stdlib.which.cwd.non_utf8 = Current directory contains non-UTF-8 components.
stdlib.which.canonicalise_failed = Failed to canonicalise '{ $path }': { $details }.
stdlib.which.canonicalise_non_utf8 = Canonical path contains non-UTF-8 components.
stdlib.which.workspace_non_utf8 = Workspace path contains non-UTF-8 components while resolving command '{ $command }': { $path }.

# Stdlib registration.
stdlib.register.open_dir = Failed to open current directory for stdlib registration.
stdlib.register.resolve_dir = Failed to resolve current directory for stdlib registration.
stdlib.register.dir_non_utf8 = Current directory contains non-UTF-8 components: { $path }.
