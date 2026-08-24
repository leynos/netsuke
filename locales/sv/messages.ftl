# Lokaliseringsresurser för Netsukes kommandoradsgränssnitt.

runner.io.dyndep.retention = Det gick inte att behålla den genererade dyndep-filen (sökväg: { $path }).
cli.about = Netsuke kompilerar YAML- + Jinja-manifest till Ninja-byggplaner.
cli.long_about = Netsuke omvandlar YAML- + Jinja-manifest till reproducerbara Ninja-grafer och kör Ninja med säkra standardvärden.
cli.usage = { $usage }

# Hjälptext för globala flaggor.
cli.flag.file.help = Sökväg till den Netsuke-manifestfil som ska användas.
cli.flag.directory.help = Kör som om starten hade skett i den här katalogen.
cli.flag.config.help = Sökväg till en konfigurationsfil, förbi den automatiska sökningen.
cli.flag.jobs.help = Ange antalet parallella byggjobb.
cli.flag.verbose.help = Aktivera utförlig diagnostikloggning och tidssammanfattningar vid avslut.
cli.flag.locale.help = Språktagg för kommandoradens texter (till exempel: en-US, sv).
cli.flag.fetch_allow_scheme.help = Ytterligare URL-scheman som hjälpfunktionen fetch får använda.
cli.flag.fetch_allow_host.help = Värdnamn som tillåts när standardnekandet är aktivt.
cli.flag.fetch_block_host.help = Värdnamn som alltid blockeras, även om de tillåts på annat håll.
cli.flag.fetch_default_deny.help = Neka alla värdar som standard; tillåt endast den angivna listan.
cli.flag.json.help = Skriv ut maskinläsbar JSON.
cli.flag.no_input.help = Läs aldrig interaktiv indata.
cli.flag.color.help = Policy för färgad utdata (auto, always, never).
cli.flag.emoji.help = Policy för emoji (auto, always, never).
cli.flag.progress.help = Policy för förloppsvisning (auto, always, never).
cli.flag.accessibility.help = Policy för tillgänglig utdata (auto, on, off).
cli.flag.default_targets.help = Standardmål för bygget när inga anges.

# Beskrivningar av underkommandon.
cli.subcommand.build.about = Bygg de mål som definierats i manifestet (standard).
cli.subcommand.build.long_about = Bygg de begärda målen; om inga anges används manifestets standardmål.
cli.subcommand.clean.about = Ta bort byggartefakter via Ninja.
cli.subcommand.clean.long_about = Skapa en tillfällig Ninja-fil och kör sedan `ninja -t clean`.
cli.subcommand.graph.about = Skriv ut byggets beroendegraf. Standardformatet är DOT.
cli.subcommand.graph.long_about = Projicera det tolkade Netsuke-manifestet till en kanonisk bygggraf och skriv den som Graphviz DOT, eller som en fristående HTML-sida med `--html`. Använd `--output <FIL>` för att skriva till en fil; `-` skriver till stdout.
cli.subcommand.generate.about = Skapa Ninja-manifestet utan att köra Ninja.
cli.subcommand.generate.long_about = Skriv det skapade Ninja-manifestet till stdout eller till en fil som väljs med `--output`.
cli.subcommand.help.about = Skriv ut hjälpen på den översta nivån eller hjälpen för ett namngivet ämne.
cli.subcommand.help.long_about = Utan ämne motsvarar detta `--help`. Använd `help targets` för att skriva ut katalogen över mål och åtgärder för den valda filen.

# Help catalogue headings and markers.
cli.help.actions_heading = Åtgärder:
cli.help.targets_heading = Mål:
cli.help.targets.about = Lista mål och åtgärder i det valda manifestet.
cli.help.default_marker = standard
cli.help.conditional_marker = villkorlig

# Hjälptext för flaggor till underkommandot build.
cli.subcommand.build.flag.targets.help = Mål som ska byggas (använder manifestets standardmål om det utelämnas).

# Hjälptext för flaggor till underkommandot graph.
cli.subcommand.graph.flag.html.help = Rendera grafen som en fristående HTML-sida i stället för DOT.
cli.subcommand.graph.flag.output.help = Skriv grafartefakten till FIL; använd `-` för stdout.

# Hjälptext för flaggor till underkommandot generate.
cli.subcommand.generate.flag.output.help = Skriv det skapade Ninja-manifestet till FIL i stället för stdout.

# Valideringsfel i kommandoradsgränssnittet.
cli.validation.jobs.invalid_number = { $value } är inte ett giltigt tal.
cli.validation.jobs.out_of_range = Antalet jobb måste ligga mellan { $min } och { $max }.
cli.validation.scheme.empty = Schemat får inte vara tomt.
cli.validation.scheme.invalid_start = Schemat ”{ $scheme }” måste börja med en ASCII-bokstav.
cli.validation.scheme.invalid = Ogiltigt schema ”{ $scheme }”.
cli.validation.locale.empty = Språktaggen får inte vara tom.
cli.validation.locale.invalid = Ogiltig språktagg ”{ $locale }”.
cli.validation.color.invalid = Ogiltig färgpolicy ”{ $value }”. Giltiga val: auto, always, never.
cli.validation.emoji.invalid = Ogiltig emojipolicy ”{ $value }”. Giltiga val: auto, always, never.
cli.validation.progress.invalid = Ogiltig förloppspolicy ”{ $value }”. Giltiga val: auto, always, never.
cli.validation.accessibility.invalid = Ogiltig tillgänglighetspolicy ”{ $value }”. Giltiga val: auto, on, off.
cli.validation.config.expected_object = Kommandoradens värden skulle serialiseras till ett objekt, men gav { $value }.

# Felmeddelanden från Clap.
clap-error-missing-argument = Obligatoriskt argument saknas: { $argument }
clap-error-missing-subcommand = Underkommando saknas. Tillgängliga val: { $valid_subcommands }
clap-error-unknown-argument = Okänt argument: { $argument }
clap-error-invalid-value = Ogiltigt värde för { $argument }: { $value }
clap-error-invalid-subcommand = Okänt underkommando: { $subcommand }
# Obs: value-validation är formulerat annorlunda än invalid-value för att
# skilja fel från egna validerare (ErrorKind::ValueValidation) från
# typkonflikter (ErrorKind::InvalidValue).
clap-error-value-validation = Valideringen misslyckades för { $argument }: { $value }

# Fel och sammanhang från körningen.
runner.manifest.not_found = Manifestet ”{ $manifest_name }” hittades inte i { $directory }.
runner.manifest.not_found.help = Kontrollera att manifestet finns, eller ange `--file` med rätt sökväg.
runner.manifest.path_missing_name = Manifestsökvägen ”{ $path }” saknar filnamn.
runner.manifest.path_utf8 = Manifestsökvägen ”{ $path }” är inte giltig UTF-8.
runner.manifest.directory_utf8 = Sökvägen till manifestkatalogen ”{ $path }” är inte giltig UTF-8.
runner.manifest.directory_label = katalogen `{ $directory }`
runner.manifest.current_directory_label = den aktuella katalogen
runner.manifest.default_not_declared = Manifestets standardvärde '{ $default }' anger ingen deklarerad åtgärd eller något mål.
runner.context.network_policy = Nätverkspolicyn kunde inte byggas.
runner.context.load_manifest = Manifestet i { $path } kunde inte läsas in.
runner.context.serialise_manifest = Manifestet kunde inte serialiseras.
runner.context.build_graph = Grafen kunde inte byggas utifrån manifestet.
runner.context.generate_ninja = Ninja-manifestet kunde inte skapas.
runner.context.render_graph = Grafartefakten kunde inte renderas.

runner.io.create_temp_file = Den tillfälliga Ninja-filen kunde inte skapas.
runner.io.write_temp_ninja = Den tillfälliga Ninja-filen kunde inte skrivas.
runner.io.flush_temp_ninja = Bufferten för den tillfälliga Ninja-filen kunde inte tömmas.
runner.io.sync_temp_ninja = Den tillfälliga Ninja-filen kunde inte synkroniseras.
runner.io.create_parent_dir = Överkatalogen { $path } kunde inte skapas.
runner.io.create_ninja_file = Ninja-filen i { $path } kunde inte skapas.
runner.io.write_ninja_file = Ninja-filen i { $path } kunde inte skrivas.
runner.io.flush_ninja_file = Bufferten för Ninja-filen i { $path } kunde inte tömmas.
runner.io.sync_ninja_file = Ninja-filen i { $path } kunde inte synkroniseras.
runner.io.open_ambient_dir = Den omgivande katalogen kunde inte öppnas.
runner.io.non_utf8_working_directory = Sökvägen till arbetskatalogen är inte giltig UTF-8.
runner.io.no_existing_ancestor = Det finns ingen överordnad katalog för { $path }.
runner.io.derive_relative_path = Den relativa Ninja-sökvägen kunde inte härledas.
runner.io.non_utf8_path = Sökvägen är inte kodad i UTF-8 och stöds inte (sökväg: { $path }).
runner.io.write_stdout = Ninja-manifestet kunde inte skrivas till stdout.
runner.io.flush_stdout = Bufferten för stdout kunde inte tömmas.
runner.io.dyndep.create_dir = Det gick inte att skapa dyndep-katalogen { $path }.
runner.io.dyndep.read = Det gick inte att läsa den genererade dyndep-filen på { $path }.
runner.io.dyndep.write = Det gick inte att skriva den genererade dyndep-filen på { $path }.
runner.io.dyndep.rename = Det gick inte att byta namn på den genererade dyndep-filen på { $path }.
runner.io.dyndep.corrupt = Den genererade dyndep-filen på { $path } stämmer inte med det förväntade innehållet; ta endast bort denna fil och försök igen.
runner.io.dyndep.temp_collisions = Det gick inte att skapa en unik tillfällig dyndep-fil för { $path } efter upprepade namnkonflikter.
runner.io.dyndep.too_large = Den genererade dyndep-filen på { $path } överskrider verifieringsgränsen på { $limit } byte.

# Manifestdiagnostik.
manifest.parse = Tolkningen av manifestet misslyckades.
manifest.structure_error = Strukturfel i manifestet vid { $name }: { $details }
manifest.yaml.parse = YAML-fel på rad { $line }, kolumn { $column }: { $details }
manifest.yaml.label = ogiltig YAML
manifest.yaml.hint.tabs = YAML tillåter inte tabbtecken; använd blanksteg för indrag.
manifest.yaml.hint.list_item = YAML-listposter måste börja med ”-” och vara korrekt indragna.
manifest.yaml.hint.expected_colon = Det här ser ut som en post i en mappning; det saknas ett ”:” efter nyckeln.
manifest.yaml.hint.mapping_values = YAML-mappningar kräver ett värde efter ”:” (eller ett indraget block).
manifest.yaml.hint.invalid_token = YAML-symbolen är ogiltig eller oväntad.
manifest.yaml.hint.escape = Escapa omvända snedstreck eller ta bort ogiltiga escapesekvenser.
manifest.env.missing = En obligatorisk miljövariabel är inte satt.
manifest.env.invalid_utf8 = En miljövariabel innehåller ogiltig UTF-8.
manifest.vars.not_object = Manifestets `vars` måste vara en mappning eller ett objekt.
manifest.vars.reserved_name = Manifestets `vars`-nyckel '{ $name }' är reserverad för en inbyggd mallhjälpare; byt namn på variabeln.
manifest.read_failed = Manifestet i { $path } kunde inte läsas.
manifest.resolve_workspace_root = Arbetsytans rot kunde inte fastställas.
manifest.workspace_non_utf8 = Arbetsytans rotsökväg ”{ $path }” är inte giltig UTF-8.
manifest.path_non_utf8 = Sökvägen till manifestet ”{ $manifest }” är inte giltig UTF-8: { $path }.
manifest.path_missing_name = Manifestsökvägen ”{ $path }” saknar filnamn.
manifest.open_workspace_failed = Arbetsytan { $workspace } kunde inte öppnas för manifestet { $manifest }.
manifest.foreach.not_iterable = Uttrycket `foreach` går inte att iterera över.
manifest.foreach.serialise_item = Posten i `foreach` kunde inte serialiseras.
manifest.when.empty = Uttrycket `when` får inte vara tomt.
manifest.when.eval_error = Uttrycket `when` ”{ $expr }” kunde inte utvärderas.
manifest.when.template_error = Mallen `when` ”{ $expr }” kunde inte renderas.
manifest.target.vars_not_object = Målets `vars` måste vara ett objekt, men gav { $value }.
manifest.vars.entry_not_object = En `vars`-post i manifestet måste vara ett objekt.
manifest.field_not_string = Fältet ”{ $field }” måste vara en sträng.
manifest.expression.parse_error = Uttrycket { $name } kunde inte tolkas.
manifest.expression.eval_error = Uttrycket { $name } kunde inte utvärderas.

# Diagnostik för manifestmakron.
manifest.macro.signature_missing_identifier = Makrosignaturen saknar en identifierare.
manifest.macro.signature_missing_params = Makrosignaturen saknar parametrar.
manifest.macro.compile_failed = Makrot { $name } kunde inte kompileras.
manifest.macro.sequence_invalid = Makron måste definieras som en mappning från namn till mallar.
manifest.macro.register_failed = Manifestets makron kunde inte registreras.
manifest.macro.not_initialised = Makromiljön är inte initierad.
manifest.macro.caller_invalid = Makrots anropare måste vara en sträng.
manifest.macro.template_load_failed = Makromallen kunde inte läsas in.
manifest.macro.init_failed = Makromiljön kunde inte initieras.
manifest.macro.missing = Makrot { $name } saknas.

# Glob-fel i manifestet.
manifest.glob.unmatched_brace = Ogiltigt glob-mönster ”{ $pattern }”: ”{ $character }” saknar motsvarighet på position { $position }.
manifest.glob.invalid_pattern = Ogiltigt glob-mönster ”{ $pattern }”: { $detail }.
manifest.glob.unknown_pattern_error = okänt mönsterfel.
manifest.glob.io_failed = Glob misslyckades för ”{ $pattern }”: { $detail }.
manifest.glob.unknown_io_error = okänt I/O-fel.
manifest.command_list_empty = Fältet ”command” får inte vara tomt: ange en kommandosträng eller en icke-tom lista.

# Fel i den interna representationen.
ir.rule_not_found = Regeln ”{ $rule }” som målet ”{ $target }” hänvisar till hittades inte.
ir.multiple_rules = Målet ”{ $target }” måste hänvisa till exakt en regel, men gav { $rules }.
ir.empty_rule = Målet ”{ $target }” måste hänvisa till en regel.
ir.duplicate_outputs = Dubblerade utdata upptäcktes: { $outputs }.
ir.circular_dependency = Ett cirkulärt beroende upptäcktes: { $cycle }.
ir.action_serialisation = Åtgärden kunde inte serialiseras: { $details }.
ir.invalid_command = Ogiltig interpolering i kommandot: { $snippet }.

# Fel vid generering av Ninja.
ninja_gen.missing_action = Åtgärden ”{ $id }” som en byggbåge hänvisar till saknas.
ninja_gen.format = Ninja-manifestets utdata kunde inte formateras.
ninja_gen.dyndep_files_required = Den här åtgärden kräver ett genererat Ninja-paket; använd `netsuke build`, `netsuke clean` eller `netsuke generate` för att materialisera dyndep-filerna.
ninja_gen.reserved_output_path = Sökvägen '{ $path }' är reserverad för Netsukes seriella beroendetillstånd.
ninja_gen.unsupported_path_character = Sökvägen '{ $path }' innehåller ett tecken som inte stöds i Ninja-sökvägar: '{ $character }'.

# Validering av värdmönster.
host_pattern.empty = Värdmönstret får inte vara tomt.
host_pattern.contains_scheme = Värdmönstret ”{ $pattern }” får inte innehålla ett URL-schema.
host_pattern.contains_slash = Värdmönstret ”{ $pattern }” får inte innehålla ”/”.
host_pattern.missing_suffix = Värdmönstret ”{ $pattern }” måste ha ett suffix efter ”*.”.
host_pattern.empty_label = Värdmönstret ”{ $pattern }” innehåller en tom etikett.
host_pattern.invalid_chars = Värdmönstret ”{ $pattern }” innehåller ogiltiga tecken.
host_pattern.invalid_label_edge = Etiketter i värdmönstret ”{ $pattern }” får inte börja eller sluta med ”-”.
host_pattern.label_too_long = Värdmönstret ”{ $pattern }” innehåller en etikett längre än 63 tecken.
host_pattern.too_long = Värdmönstret ”{ $pattern }” överskrider gränsen på 255 tecken.

# Nätverkspolicy.
network_policy.scheme.empty = Schemat får inte vara tomt.
network_policy.scheme.invalid = Schemat ”{ $scheme }” innehåller ogiltiga tecken.
network_policy.allowlist.empty = Listan över tillåtna värdar får inte vara tom.
network_policy.scheme.not_allowed = Schemat ”{ $scheme }” är inte tillåtet.
network_policy.missing_host = URL-adressen saknar värd.
network_policy.host.blocked = Värden ”{ $host }” blockeras av policyn.
network_policy.host.not_allowlisted = Värden ”{ $host }” finns inte på listan över tillåtna.

# Konfiguration av standardbiblioteket.
stdlib.config.default_fetch_cache_invalid = Standardsökvägen till fetch-cachen måste vara relativ.
stdlib.config.default_which_cache_invalid = Standardkapaciteten för which-cachen måste vara positiv.
stdlib.config.workspace_root_absolute = Arbetsytans rotsökväg måste vara absolut.
stdlib.config.fetch_response_limit_positive = Svarsgränsen för fetch måste vara positiv.
stdlib.config.command_output_limit_positive = Gränsen för fångad kommandoutdata måste vara positiv.
stdlib.config.command_stream_limit_positive = Strömgränsen för kommandon måste vara positiv.
stdlib.config.which_cache_capacity_positive = Kapaciteten för which-cachen måste vara positiv.
stdlib.config.skip_dir_empty = Poster över överhoppade kataloger får inte vara tomma.
stdlib.config.skip_dir_navigation = Poster över överhoppade kataloger får inte innehålla ”..”.
stdlib.config.skip_dir_separator = Poster över överhoppade kataloger får inte innehålla sökvägsavgränsare.
stdlib.config.fetch_cache_empty = Sökvägen till fetch-cachen får inte vara tom.
stdlib.config.fetch_cache_not_relative = Sökvägen till fetch-cachen måste vara relativ, men gav { $path }.
stdlib.config.fetch_cache_escapes = Sökvägen till fetch-cachen får inte lämna arbetsytan: { $path }.
stdlib.config.open_workspace_root = Den aktuella katalogen kunde inte öppnas som rot för stdlib-arbetsytan.
stdlib.config.resolve_cwd = Den aktuella katalogen kunde inte fastställas som rot för stdlib-arbetsytan.
stdlib.config.cwd_non_utf8 = Den aktuella katalogen innehåller delar som inte är UTF-8: { $path }.

# Diagnostik för hjälpfunktionen fetch.
stdlib.fetch.url_invalid = Ogiltig URL ”{ $url }”: { $details }.
stdlib.fetch.disallowed = URL-adressen ”{ $url }” är inte tillåten: { $details }.
stdlib.fetch.failed = ”{ $url }” kunde inte hämtas: { $details }.
stdlib.fetch.cache_read_failed = Cacheposten ”{ $name }” kunde inte läsas: { $details }.
stdlib.fetch.cache_open_failed = Cacheposten ”{ $name }” kunde inte öppnas: { $details }.
stdlib.fetch.response_read_failed = Svaret från ”{ $url }” kunde inte läsas: { $details }.
stdlib.fetch.response_buffer_overflow = Buffertspill vid läsning av ”{ $url }”.
stdlib.fetch.cache_write_failed = Cachen för ”{ $url }” kunde inte skrivas: { $details }.
stdlib.fetch.response_limit_exceeded = Svaret från ”{ $url }” överskred gränsen på { $limit } byte.
stdlib.fetch.cache_limit_exceeded = Det cachade svaret ”{ $name }” överskred gränsen på { $limit } byte.
stdlib.fetch.io_failed = { $action } misslyckades för { $path }: { $details }.
stdlib.fetch.action.sync_cache = synkronisera fetch-cachen
stdlib.fetch.action.create_cache_dir = skapa katalogen för fetch-cachen
stdlib.fetch.action.open_cache_dir = öppna katalogen för fetch-cachen
stdlib.fetch.action.stat_cache = slå upp posten i fetch-cachen
stdlib.fetch.action.open_cache_entry = öppna posten i fetch-cachen

# Diagnostik för kommandohjälparen.
stdlib.command.location = kommandot ”{ $command }” i mallen ”{ $template }”
stdlib.command.spawn_failed = { $location } kunde inte startas: { $details }.
stdlib.command.io_failed = { $location } misslyckades: { $details }.
stdlib.command.closed_input_early = Indata stängdes innan skrivningen till kommandot var klar.
stdlib.command.broken_pipe = Bruten rörledning vid körning av { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } avbröts av en signal.
stdlib.command.exited_with_status = { $location } avslutades med status { $status }.
stdlib.command.output_limit_exceeded = { $location } överskred { $mode }-gränsen på { $limit } byte för { $stream }.
stdlib.command.timeout = { $location } överskred tidsgränsen på { $seconds } sekunder.
stdlib.command.exit_status_suffix = (slutstatus { $status })
stdlib.command.signal_suffix = (avbrutet av en signal)
stdlib.command.shell.empty = Skalkommandot får inte vara tomt.
stdlib.command.grep.empty_pattern = Mönstret till grep får inte vara tomt.
stdlib.command.grep.flags_not_string = Flaggor till grep måste vara strängar.
stdlib.command.quote.invalid = { $arg } kunde inte citeras: { $details }.
stdlib.command.quote.line_break = Argument med vagnretur eller radmatning kan inte citeras säkert.
stdlib.command.input_undefined = Indatavärdet är odefinierat.
stdlib.command.tempfile.root_required = Arbetsytans rot krävs för att skapa tillfälliga kommandofiler.
stdlib.command.tempfile.create_failed = Den tillfälliga kommandofilen kunde inte skapas: { $details }.
stdlib.command.options.invalid_utf8 = Nyckeln till en kommandoinställning måste vara giltig UTF-8.
stdlib.command.option.mode_not_string = Utdataläget måste vara en sträng.
stdlib.command.options.invalid_type = Kommandoinställningar måste vara ett objekt.
stdlib.command.output.mode_unsupported = Utdataläget ”{ $mode }” stöds inte.
stdlib.command.output.mode.capture = infångning
stdlib.command.output.mode.streaming = strömning
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnostik för sökvägshjälparen.
stdlib.path.io.failed = { $action } misslyckades för { $path } ({ $label }).
stdlib.path.io.failed_with_detail = { $action } misslyckades för { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $action } misslyckades för { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = hittades inte
stdlib.path.io.permission_denied = åtkomst nekad
stdlib.path.io.already_exists = finns redan
stdlib.path.io.invalid_input = ogiltig indata
stdlib.path.io.invalid_data = ogiltiga data
stdlib.path.io.timed_out = tidsgränsen löpte ut
stdlib.path.io.interrupted = avbruten
stdlib.path.io.would_block = skulle blockera
stdlib.path.io.write_zero = noll byte skrevs
stdlib.path.io.unexpected_eof = oväntat filslut
stdlib.path.io.broken_pipe = bruten rörledning
stdlib.path.io.connection_refused = anslutningen nekades
stdlib.path.io.connection_reset = anslutningen återställdes
stdlib.path.io.connection_aborted = anslutningen avbröts
stdlib.path.io.not_connected = inte ansluten
stdlib.path.io.addr_in_use = adressen används redan
stdlib.path.io.addr_not_available = adressen är inte tillgänglig
stdlib.path.io.out_of_memory = minnet är slut
stdlib.path.io.unsupported = stöds inte
stdlib.path.io.file_too_large = filen är för stor
stdlib.path.io.resource_busy = resursen är upptagen
stdlib.path.io.executable_busy = den körbara filen är upptagen
stdlib.path.io.deadlock = dödläge
stdlib.path.io.crosses_devices = korsar enheter
stdlib.path.io.too_many_links = för många länkar
stdlib.path.io.invalid_filename = ogiltigt filnamn
stdlib.path.io.arg_list_too_long = argumentlistan är för lång
stdlib.path.io.stale_handle = föråldrat filhandtag i nätverket
stdlib.path.io.storage_full = lagringen är full
stdlib.path.io.not_seekable = går inte att söka i
stdlib.path.io.network_down = nätverket ligger nere
stdlib.path.io.network_unreachable = nätverket kan inte nås
stdlib.path.io.host_unreachable = värden kan inte nås
stdlib.path.io.other = I/O-fel
stdlib.path.action.canonicalize = kanonisera
stdlib.path.action.open_directory = öppna katalog
stdlib.path.action.stat = slå upp
stdlib.path.action.read = läsa
stdlib.path.action.open_file = öppna fil
stdlib.path.with_suffix.empty_separator = with_suffix kräver en avgränsare som inte är tom.
stdlib.path.relative_to.mismatch = { $path } är inte relativ till { $root }.
stdlib.path.expanduser.unsupported = Användarspecifik expansion av ~ stöds inte.
stdlib.path.expanduser.no_home = ~ kan inte expanderas: inga miljövariabler för hemkatalogen är satta.
stdlib.path.contents.unsupported_encoding = Teckenkodningen ”{ $encoding }” stöds inte.
stdlib.path.hash.unsupported_algorithm = Hashalgoritmen ”{ $algorithm }” stöds inte.
stdlib.path.hash.unsupported_algorithm_legacy = Hashalgoritmen ”{ $algorithm }” stöds inte (aktivera funktionen ”{ $feature }”).

# Diagnostik för samlingshjälpare.
stdlib.collections.flatten.expected_sequence = flatten väntade poster från en sekvens men fann { $kind }.
stdlib.collections.group_by.empty_attribute = group_by kräver ett attribut som inte är tomt.
stdlib.collections.group_by.unresolved = group_by kunde inte slå upp ”{ $attr }” på en post av typen { $kind }.

# Diagnostik för tidshjälpare.
stdlib.time.offset.invalid = Förskjutningen för now ”{ $offset }” är ogiltig: väntade ”+HH:MM[:SS]” eller ”Z”.
stdlib.time.timedelta.overflow = Spill i timedelta vid addition av { $component }.
stdlib.time.label.weeks = veckor
stdlib.time.label.days = dagar
stdlib.time.label.hours = timmar
stdlib.time.label.minutes = minuter
stdlib.time.label.seconds = sekunder
stdlib.time.label.milliseconds = millisekunder
stdlib.time.label.microseconds = mikrosekunder
stdlib.time.label.nanoseconds = nanosekunder

# Diagnostik för hjälpfunktionen which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] kommandot ”{ $command }” hittades inte efter genomgång av { $count } PATH-poster. Utdrag: { $preview }
stdlib.which.not_found.hint.cwd_auto = Tomma delar av PATH ignoreras; använd cwd_mode="auto" för att ta med arbetskatalogen.
stdlib.which.not_found.hint.cwd_always = Sätt cwd_mode="always" för att ta med den aktuella katalogen.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] kommandot ”{ $command }” i ”{ $path }” saknas eller är inte körbart.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <tom>
stdlib.which.path_entry.non_utf8 = PATH-post nr { $index } innehåller tecken som inte är UTF-8; Netsuke kräver UTF-8-sökvägar.
stdlib.which.command.empty = which kräver en sträng som inte är tom.
stdlib.which.cwd_mode.invalid = cwd_mode måste vara ”auto”, ”always” eller ”never”, men gav ”{ $mode }”.
stdlib.which.cwd.resolve_failed = Den aktuella katalogen kunde inte fastställas: { $details }.
stdlib.which.cwd.non_utf8 = Den aktuella katalogen innehåller delar som inte är UTF-8.
stdlib.which.canonicalize_failed = ”{ $path }” kunde inte kanoniseras: { $details }.
stdlib.which.is_executable = Det gick inte att avgöra om ”{ $path }” är körbar: { $details }.
stdlib.which.canonicalize_non_utf8 = Den kanoniska sökvägen innehåller delar som inte är UTF-8.
stdlib.which.workspace_non_utf8 = Arbetsytans sökväg innehåller delar som inte är UTF-8 vid uppslag av kommandot ”{ $command }”: { $path }.
stdlib.which.walkdir_error = Fel vid genomgång av arbetsytan under uppslag av kommandot: { $details }.

# Registrering av standardbiblioteket.
stdlib.register.open_dir = Den aktuella katalogen kunde inte öppnas för registrering av stdlib.
stdlib.register.resolve_dir = Den aktuella katalogen kunde inte fastställas för registrering av stdlib.
stdlib.register.dir_non_utf8 = Den aktuella katalogen innehåller delar som inte är UTF-8: { $path }.

# Statusrapportering för tillgängligt utdataläge.
status.state.pending = väntar
status.state.running = pågår
status.state.done = klar
status.state.failed = misslyckades
status.stage.label = Steg { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Uppgift { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Läser manifestfilen
status.stage.initial_yaml_parsing = Tolkar YAML-dokumentet
status.stage.template_expansion = Expanderar malldirektiv
status.stage.final_rendering = Deserialiserar och renderar manifestets värden
status.stage.ir_generation_validation = Bygger och validerar beroendegrafen
status.stage.ninja_synthesis = Skapar Ninja-byggplanen
status.stage.ninja_synthesis_execute = Skapar Ninja-planen och kör { $tool }
status.stage.graph_rendering = Renderar grafartefakten
status.stage.graph_rendering_with_tool = Renderar { $tool }
status.complete = { $tool }: operationen slutfördes.
status.timing.summary_header = Tidssammanfattning per steg:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Total tid för kedjan: { $duration }
status.tool.build = Bygge
status.tool.clean = Rensning
status.tool.graph = Graf
status.tool.graph_html = Graf (HTML)
status.tool.generate = Generering
status.tool.help_targets = Målhjälp

# Texter för HTML-renderingen av grafen.
graph.html.title = Netsuke-bygggraf
graph.html.heading = Netsuke-bygggraf
graph.html.description = Bygggraf renderad av Netsuke
graph.html.outline.summary = Mål och beroenden (textöversikt)
graph.html.outline.no_inputs = Inga indata
graph.html.noscript.notice = JavaScript är avstängt. Textöversikten ovan är hela grafen; DOT-källan följer nedan.

# Semantiska prefix för tillgänglig utdata.
semantic.prefix.error = Fel:
semantic.prefix.warning = Varning:
semantic.prefix.success = Lyckades:
semantic.prefix.info = Info:
semantic.prefix.timing = Tid:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Exempel på pluralformer för översättare.
# Svenskan använder CLDR-kategorierna `one` och `other` precis som källspråket.
example.files_processed = { $count ->
    [one] Behandlade { $count } fil.
   *[other] Behandlade { $count } filer.
}

example.errors_found = { $count ->
    [0] Inga fel hittades.
    [one] { $count } fel hittades.
   *[other] { $count } fel hittades.
}
