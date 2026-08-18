# Lokaliseringsressourcer til Netsukes kommandolinje.

runner.io.dyndep.retention = Kunne ikke anvende opbevaring af den genererede dyndep under { $path }.
cli.about = Netsuke oversætter YAML- + Jinja-manifester til Ninja-byggeplaner.
cli.long_about = Netsuke omdanner YAML- + Jinja-manifester til reproducerbare Ninja-grafer og kører Ninja med sikre standardindstillinger.
cli.usage = { $usage }

# Hjælpetekst til globale tilvalg.
cli.flag.file.help = Sti til den Netsuke-manifestfil, der skal bruges.
cli.flag.directory.help = Kør, som om der var startet i denne mappe.
cli.flag.config.help = Sti til en konfigurationsfil, som springer den automatiske søgning over.
cli.flag.jobs.help = Angiv antallet af parallelle byggejob.
cli.flag.verbose.help = Aktivér udførlig diagnostisk logning og tidsopsummeringer ved afslutning.
cli.flag.locale.help = Sprogmærke til kommandolinjens tekster (for eksempel: en-US, da).
cli.flag.fetch_allow_scheme.help = Yderligere URL-skemaer, som fetch-hjælperen må bruge.
cli.flag.fetch_allow_host.help = Værtsnavne, der tillades, når standardafvisning er slået til.
cli.flag.fetch_block_host.help = Værtsnavne, der altid blokeres, også hvis de er tilladt andetsteds.
cli.flag.fetch_default_deny.help = Afvis alle værter som standard; tillad kun den erklærede liste.
cli.flag.json.help = Udskriv maskinlæsbart JSON.
cli.flag.no_input.help = Læs aldrig interaktivt input.
cli.flag.color.help = Politik for farvet output (auto, always, never).
cli.flag.emoji.help = Politik for emoji (auto, always, never).
cli.flag.progress.help = Politik for visning af fremdrift (auto, always, never).
cli.flag.accessibility.help = Politik for tilgængeligt output (auto, on, off).
cli.flag.default_targets.help = Standardmål for bygning, når ingen er angivet.

# Beskrivelser af underkommandoer.
cli.subcommand.build.about = Byg de mål, der er defineret i manifestet (standard).
cli.subcommand.build.long_about = Byg de ønskede mål; er ingen angivet, bruges manifestets standardmål.
cli.subcommand.clean.about = Fjern byggeartefakter via Ninja.
cli.subcommand.clean.long_about = Generér en midlertidig Ninja-fil, og kør derefter `ninja -t clean`.
cli.subcommand.graph.about = Udskriv byggegrafen over afhængigheder. Standardformatet er DOT.
cli.subcommand.graph.long_about = Omsæt det indlæste Netsuke-manifest til en kanonisk byggegraf, og skriv den som Graphviz DOT eller som en selvstændig HTML-side med `--html`. Brug `--output <FIL>` for at skrive til en fil; `-` skriver til stdout.
cli.subcommand.generate.about = Generér Ninja-manifestet uden at køre Ninja.
cli.subcommand.generate.long_about = Skriv det genererede Ninja-manifest til stdout eller til en fil valgt med `--output`.
cli.subcommand.help.about = Udskriv hjælpen på øverste niveau eller hjælp til et navngivet emne.
cli.subcommand.help.long_about = Uden emne svarer dette til `--help`. Brug `help targets` til at udskrive kataloget over mål og handlinger for den valgte fil.

# Help catalogue headings and markers.
cli.help.actions_heading = Handlinger:
cli.help.targets_heading = Mål:
cli.help.targets.about = Vis mål og handlinger i det valgte manifest.
cli.help.default_marker = standard

# Hjælpetekst til tilvalg for underkommandoen build.
cli.subcommand.build.flag.targets.help = Mål, der skal bygges (bruger manifestets standardmål, hvis udeladt).

# Hjælpetekst til tilvalg for underkommandoen graph.
cli.subcommand.graph.flag.html.help = Gengiv grafen som en selvstændig HTML-side i stedet for DOT.
cli.subcommand.graph.flag.output.help = Skriv grafartefaktet til FIL; brug `-` for stdout.

# Hjælpetekst til tilvalg for underkommandoen generate.
cli.subcommand.generate.flag.output.help = Skriv det genererede Ninja-manifest til FIL i stedet for stdout.

# Valideringsfejl på kommandolinjen.
cli.validation.jobs.invalid_number = { $value } er ikke et gyldigt tal.
cli.validation.jobs.out_of_range = Antallet af job skal ligge mellem { $min } og { $max }.
cli.validation.scheme.empty = Skemaet må ikke være tomt.
cli.validation.scheme.invalid_start = Skemaet "{ $scheme }" skal begynde med et ASCII-bogstav.
cli.validation.scheme.invalid = Ugyldigt skema "{ $scheme }".
cli.validation.locale.empty = Sprogmærket må ikke være tomt.
cli.validation.locale.invalid = Ugyldigt sprogmærke "{ $locale }".
cli.validation.color.invalid = Ugyldig farvepolitik "{ $value }". Gyldige valg: auto, always, never.
cli.validation.emoji.invalid = Ugyldig emojipolitik "{ $value }". Gyldige valg: auto, always, never.
cli.validation.progress.invalid = Ugyldig fremdriftspolitik "{ $value }". Gyldige valg: auto, always, never.
cli.validation.accessibility.invalid = Ugyldig tilgængelighedspolitik "{ $value }". Gyldige valg: auto, on, off.
cli.validation.config.expected_object = Kommandolinjens værdier skulle serialiseres til et objekt, men gav { $value }.

# Fejlmeddelelser fra Clap.
clap-error-missing-argument = Manglende påkrævet argument: { $argument }
clap-error-missing-subcommand = Manglende underkommando. Tilgængelige valg: { $valid_subcommands }
clap-error-unknown-argument = Ukendt argument: { $argument }
clap-error-invalid-value = Ugyldig værdi til { $argument }: { $value }
clap-error-invalid-subcommand = Ukendt underkommando: { $subcommand }
# Bemærk: value-validation er formuleret anderledes end invalid-value for at
# skelne fejl fra egne validatorer (ErrorKind::ValueValidation) fra
# typekonflikter (ErrorKind::InvalidValue).
clap-error-value-validation = Validering mislykkedes for { $argument }: { $value }

# Fejl og kontekst fra kørslen.
runner.manifest.not_found = Manifestet "{ $manifest_name }" blev ikke fundet i { $directory }.
runner.manifest.not_found.help = Kontrollér, at manifestet findes, eller angiv `--file` med den rigtige sti.
runner.manifest.path_missing_name = Manifeststien "{ $path }" har intet filnavn.
runner.manifest.path_utf8 = Manifeststien "{ $path }" er ikke gyldig UTF-8.
runner.manifest.directory_utf8 = Stien til manifestmappen "{ $path }" er ikke gyldig UTF-8.
runner.manifest.directory_label = mappen `{ $directory }`
runner.manifest.current_directory_label = den aktuelle mappe
runner.manifest.default_not_declared = Manifestets standardværdi '{ $default }' angiver ikke en erklæret handling eller et mål.
runner.context.network_policy = Netværkspolitikken kunne ikke opbygges.
runner.context.load_manifest = Manifestet i { $path } kunne ikke indlæses.
runner.context.serialise_manifest = Manifestet kunne ikke serialiseres.
runner.context.build_graph = Grafen kunne ikke bygges ud fra manifestet.
runner.context.generate_ninja = Ninja-manifestet kunne ikke genereres.
runner.context.render_graph = Grafartefaktet kunne ikke gengives.

runner.io.create_temp_file = Den midlertidige Ninja-fil kunne ikke oprettes.
runner.io.write_temp_ninja = Den midlertidige Ninja-fil kunne ikke skrives.
runner.io.flush_temp_ninja = Bufferen for den midlertidige Ninja-fil kunne ikke tømmes.
runner.io.sync_temp_ninja = Den midlertidige Ninja-fil kunne ikke synkroniseres.
runner.io.create_parent_dir = Overmappen { $path } kunne ikke oprettes.
runner.io.create_ninja_file = Ninja-filen i { $path } kunne ikke oprettes.
runner.io.write_ninja_file = Ninja-filen i { $path } kunne ikke skrives.
runner.io.flush_ninja_file = Bufferen for Ninja-filen i { $path } kunne ikke tømmes.
runner.io.sync_ninja_file = Ninja-filen i { $path } kunne ikke synkroniseres.
runner.io.open_ambient_dir = Den omgivende mappe kunne ikke åbnes.
runner.io.non_utf8_working_directory = Stien til arbejdsmappen er ikke gyldig UTF-8.
runner.io.no_existing_ancestor = Der findes ingen overordnet mappe for { $path }.
runner.io.derive_relative_path = Den relative Ninja-sti kunne ikke udledes.
runner.io.non_utf8_path = Stier, der ikke er UTF-8, understøttes ikke (sti: { $path }).
runner.io.write_stdout = Ninja-manifestet kunne ikke skrives til stdout.
runner.io.flush_stdout = Bufferen for stdout kunne ikke tømmes.
runner.io.dyndep.create_dir = Kunne ikke oprette dyndep-mappen { $path }.
runner.io.dyndep.read = Kunne ikke læse den genererede dyndep-fil på { $path }.
runner.io.dyndep.write = Kunne ikke skrive den genererede dyndep-fil til { $path }.
runner.io.dyndep.rename = Kunne ikke færdiggøre den genererede dyndep-fil på { $path }.
runner.io.dyndep.corrupt = Den genererede dyndep-fil på { $path } matcher ikke det forventede indhold; fjern kun denne fil, og prøv igen.
runner.io.dyndep.temp_collisions = Kunne ikke oprette en entydig midlertidig dyndep-fil for { $path } efter gentagne navnekollisioner.
runner.io.dyndep.too_large = Den genererede dyndep-fil på { $path } overskrider verificeringsgrænsen på { $limit } byte.

# Manifestdiagnostik.
manifest.parse = Parsingen af manifestet mislykkedes.
manifest.structure_error = Strukturfejl i manifestet ved { $name }: { $details }
manifest.yaml.parse = YAML-fejl i linje { $line }, kolonne { $column }: { $details }
manifest.yaml.label = ugyldig YAML
manifest.yaml.hint.tabs = YAML tillader ikke tabulatorer; brug mellemrum til indrykning.
manifest.yaml.hint.list_item = YAML-listeelementer skal begynde med "-" og være korrekt indrykket.
manifest.yaml.hint.expected_colon = Dette ligner et opslag i en tilknytning; der mangler et ":" efter nøglen.
manifest.yaml.hint.mapping_values = YAML-tilknytninger kræver en værdi efter ":" (eller en indrykket blok).
manifest.yaml.hint.invalid_token = YAML-symbolet er ugyldigt eller uventet.
manifest.yaml.hint.escape = Escape omvendte skråstreger, eller fjern ugyldige escape-sekvenser.
manifest.env.missing = En påkrævet miljøvariabel er ikke sat.
manifest.env.invalid_utf8 = En miljøvariabel indeholder ugyldig UTF-8.
manifest.vars.not_object = Manifestets `vars` skal være en tilknytning eller et objekt.
manifest.vars.reserved_name = Manifestets `vars`-nøgle '{ $name }' er reserveret til en indbygget skabelonhjælper; omdøb variablen.
manifest.read_failed = Manifestet i { $path } kunne ikke læses.
manifest.resolve_workspace_root = Roden af arbejdsområdet kunne ikke bestemmes.
manifest.workspace_non_utf8 = Rodstien for arbejdsområdet "{ $path }" er ikke gyldig UTF-8.
manifest.path_non_utf8 = Stien til manifestet "{ $manifest }" er ikke gyldig UTF-8: { $path }.
manifest.path_missing_name = Manifeststien "{ $path }" har intet filnavn.
manifest.open_workspace_failed = Arbejdsområdet { $workspace } kunne ikke åbnes for manifestet { $manifest }.
manifest.foreach.not_iterable = Udtrykket `foreach` kan ikke gennemløbes.
manifest.foreach.serialise_item = Elementet i `foreach` kunne ikke serialiseres.
manifest.when.empty = Udtrykket `when` må ikke være tomt.
manifest.when.eval_error = Udtrykket `when` "{ $expr }" kunne ikke evalueres.
manifest.when.template_error = Skabelonen `when` "{ $expr }" kunne ikke gengives.
manifest.target.vars_not_object = Målets `vars` skal være et objekt, men gav { $value }.
manifest.vars.entry_not_object = Et `vars`-opslag i manifestet skal være et objekt.
manifest.field_not_string = Feltet "{ $field }" skal være en streng.
manifest.expression.parse_error = Udtrykket { $name } kunne ikke indlæses.
manifest.expression.eval_error = Udtrykket { $name } kunne ikke evalueres.

# Diagnostik for manifestmakroer.
manifest.macro.signature_missing_identifier = Makrosignaturen mangler et navn.
manifest.macro.signature_missing_params = Makrosignaturen mangler parametre.
manifest.macro.compile_failed = Makroen { $name } kunne ikke oversættes.
manifest.macro.sequence_invalid = Makroer skal defineres som en tilknytning fra navne til skabeloner.
manifest.macro.register_failed = Manifestets makroer kunne ikke registreres.
manifest.macro.not_initialised = Makromiljøet er ikke klargjort.
manifest.macro.caller_invalid = Makroens kalder skal være en streng.
manifest.macro.template_load_failed = Makroskabelonen kunne ikke indlæses.
manifest.macro.init_failed = Makromiljøet kunne ikke klargøres.
manifest.macro.missing = Makroen { $name } mangler.

# Glob-fejl i manifestet.
manifest.glob.unmatched_brace = Ugyldigt glob-mønster "{ $pattern }": "{ $character }" uden modstykke på position { $position }.
manifest.glob.invalid_pattern = Ugyldigt glob-mønster "{ $pattern }": { $detail }.
manifest.glob.unknown_pattern_error = ukendt mønsterfejl.
manifest.glob.io_failed = Glob mislykkedes for "{ $pattern }": { $detail }.
manifest.glob.unknown_io_error = ukendt I/O-fejl.
manifest.command_list_empty = Feltet "command" må ikke være tomt: angiv en kommandostreng eller en ikke-tom liste.

# Fejl i den interne repræsentation.
ir.rule_not_found = Reglen "{ $rule }", som målet "{ $target }" henviser til, blev ikke fundet.
ir.multiple_rules = Målet "{ $target }" skal henvise til præcis én regel, men gav { $rules }.
ir.empty_rule = Målet "{ $target }" skal henvise til en regel.
ir.duplicate_outputs = Der blev fundet dublerede output: { $outputs }.
ir.circular_dependency = Der blev fundet en cirkulær afhængighed: { $cycle }.
ir.action_serialisation = Handlingen kunne ikke serialiseres: { $details }.
ir.invalid_command = Ugyldig indsættelse i kommandoen: { $snippet }.

# Fejl under generering af Ninja.
ninja_gen.missing_action = Handlingen "{ $id }", som en byggekant henviser til, mangler.
ninja_gen.format = Ninja-manifestets output kunne ikke formateres.
ninja_gen.dyndep_files_required = Dette build kræver en genereret Ninja-pakke; brug `netsuke build`, `netsuke clean` eller `netsuke generate`, så dyndep-filerne materialiseres.
ninja_gen.reserved_output_path = Stien '{ $path }' er reserveret til Netsukes serielle afhængighedstilstand.
ninja_gen.unsupported_path_character = Stien '{ $path }' indeholder det ikke-understøttede Ninja-stitegn '{ $character }'.

# Validering af værtsmønstre.
host_pattern.empty = Værtsmønsteret må ikke være tomt.
host_pattern.contains_scheme = Værtsmønsteret "{ $pattern }" må ikke indeholde et URL-skema.
host_pattern.contains_slash = Værtsmønsteret "{ $pattern }" må ikke indeholde "/".
host_pattern.missing_suffix = Værtsmønsteret "{ $pattern }" skal have et suffiks efter "*.".
host_pattern.empty_label = Værtsmønsteret "{ $pattern }" indeholder en tom etiket.
host_pattern.invalid_chars = Værtsmønsteret "{ $pattern }" indeholder ugyldige tegn.
host_pattern.invalid_label_edge = Etiketter i værtsmønsteret "{ $pattern }" må ikke begynde eller slutte med "-".
host_pattern.label_too_long = Værtsmønsteret "{ $pattern }" indeholder en etiket på over 63 tegn.
host_pattern.too_long = Værtsmønsteret "{ $pattern }" overskrider grænsen på 255 tegn.

# Netværkspolitik.
network_policy.scheme.empty = Skemaet må ikke være tomt.
network_policy.scheme.invalid = Skemaet "{ $scheme }" indeholder ugyldige tegn.
network_policy.allowlist.empty = Listen over tilladte værter må ikke være tom.
network_policy.scheme.not_allowed = Skemaet "{ $scheme }" er ikke tilladt.
network_policy.missing_host = URL-adressen mangler en vært.
network_policy.host.blocked = Værten "{ $host }" er blokeret af politikken.
network_policy.host.not_allowlisted = Værten "{ $host }" står ikke på listen over tilladte.

# Konfiguration af standardbiblioteket.
stdlib.config.default_fetch_cache_invalid = Standardstien til fetch-mellemlageret skal være relativ.
stdlib.config.default_which_cache_invalid = Standardkapaciteten for which-mellemlageret skal være positiv.
stdlib.config.workspace_root_absolute = Rodstien for arbejdsområdet skal være absolut.
stdlib.config.fetch_response_limit_positive = Svargrænsen for fetch skal være positiv.
stdlib.config.command_output_limit_positive = Grænsen for opsamlet kommandooutput skal være positiv.
stdlib.config.command_stream_limit_positive = Strømgrænsen for kommandoer skal være positiv.
stdlib.config.which_cache_capacity_positive = Kapaciteten for which-mellemlageret skal være positiv.
stdlib.config.skip_dir_empty = Opslag over oversprungne mapper må ikke være tomme.
stdlib.config.skip_dir_navigation = Opslag over oversprungne mapper må ikke indeholde "..".
stdlib.config.skip_dir_separator = Opslag over oversprungne mapper må ikke indeholde stiadskillere.
stdlib.config.fetch_cache_empty = Stien til fetch-mellemlageret må ikke være tom.
stdlib.config.fetch_cache_not_relative = Stien til fetch-mellemlageret skal være relativ, men gav { $path }.
stdlib.config.fetch_cache_escapes = Stien til fetch-mellemlageret må ikke forlade arbejdsområdet: { $path }.
stdlib.config.open_workspace_root = Den aktuelle mappe kunne ikke åbnes som rod for stdlib-arbejdsområdet.
stdlib.config.resolve_cwd = Den aktuelle mappe kunne ikke bestemmes som rod for stdlib-arbejdsområdet.
stdlib.config.cwd_non_utf8 = Den aktuelle mappe indeholder dele, der ikke er UTF-8: { $path }.

# Diagnostik for fetch-hjælperen.
stdlib.fetch.url_invalid = Ugyldig URL-adresse "{ $url }": { $details }.
stdlib.fetch.disallowed = URL-adressen "{ $url }" er ikke tilladt: { $details }.
stdlib.fetch.failed = "{ $url }" kunne ikke hentes: { $details }.
stdlib.fetch.cache_read_failed = Opslaget "{ $name }" i mellemlageret kunne ikke læses: { $details }.
stdlib.fetch.cache_open_failed = Opslaget "{ $name }" i mellemlageret kunne ikke åbnes: { $details }.
stdlib.fetch.response_read_failed = Svaret fra "{ $url }" kunne ikke læses: { $details }.
stdlib.fetch.response_buffer_overflow = Bufferoverløb under læsning af "{ $url }".
stdlib.fetch.cache_write_failed = Mellemlageret for "{ $url }" kunne ikke skrives: { $details }.
stdlib.fetch.response_limit_exceeded = Svaret fra "{ $url }" oversteg grænsen på { $limit } byte.
stdlib.fetch.cache_limit_exceeded = Det mellemlagrede svar "{ $name }" oversteg grænsen på { $limit } byte.
stdlib.fetch.io_failed = { $action } mislykkedes for { $path }: { $details }.
stdlib.fetch.action.sync_cache = synkronisering af fetch-mellemlageret
stdlib.fetch.action.create_cache_dir = oprettelse af mappen til fetch-mellemlageret
stdlib.fetch.action.open_cache_dir = åbning af mappen til fetch-mellemlageret
stdlib.fetch.action.stat_cache = opslag på posten i fetch-mellemlageret
stdlib.fetch.action.open_cache_entry = åbning af posten i fetch-mellemlageret

# Diagnostik for kommandohjælperen.
stdlib.command.location = kommandoen "{ $command }" i skabelonen "{ $template }"
stdlib.command.spawn_failed = { $location } kunne ikke startes: { $details }.
stdlib.command.io_failed = { $location } mislykkedes: { $details }.
stdlib.command.closed_input_early = Inputtet blev lukket, før skrivningen til kommandoen var færdig.
stdlib.command.broken_pipe = Brudt datakanal under kørsel af { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } blev afbrudt af et signal.
stdlib.command.exited_with_status = { $location } afsluttede med status { $status }.
stdlib.command.output_limit_exceeded = { $location } oversteg { $mode }-grænsen på { $limit } byte for { $stream }.
stdlib.command.timeout = { $location } overskred tidsgrænsen på { $seconds } sekunder.
stdlib.command.exit_status_suffix = (afslutningsstatus { $status })
stdlib.command.signal_suffix = (afbrudt af et signal)
stdlib.command.shell.empty = Skalkommandoen må ikke være tom.
stdlib.command.grep.empty_pattern = Mønsteret til grep må ikke være tomt.
stdlib.command.grep.flags_not_string = Flag til grep skal være strenge.
stdlib.command.quote.invalid = { $arg } kunne ikke sættes i anførselstegn: { $details }.
stdlib.command.quote.line_break = Argumenter med vognretur eller linjeskift kan ikke sættes sikkert i anførselstegn.
stdlib.command.input_undefined = Inputværdien er udefineret.
stdlib.command.tempfile.root_required = Der kræves en rod for arbejdsområdet for at oprette midlertidige kommandofiler.
stdlib.command.tempfile.create_failed = Den midlertidige kommandofil kunne ikke oprettes: { $details }.
stdlib.command.options.invalid_utf8 = Nøglen til et kommandotilvalg skal være gyldig UTF-8.
stdlib.command.option.mode_not_string = Outputtilstanden skal være en streng.
stdlib.command.options.invalid_type = Kommandotilvalg skal være et objekt.
stdlib.command.output.mode_unsupported = Outputtilstanden "{ $mode }" understøttes ikke.
stdlib.command.output.mode.capture = opsamling
stdlib.command.output.mode.streaming = strømning
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnostik for stihjælperen.
stdlib.path.io.failed = { $action } mislykkedes for { $path } ({ $label }).
stdlib.path.io.failed_with_detail = { $action } mislykkedes for { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $action } mislykkedes for { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = ikke fundet
stdlib.path.io.permission_denied = adgang nægtet
stdlib.path.io.already_exists = findes allerede
stdlib.path.io.invalid_input = ugyldigt input
stdlib.path.io.invalid_data = ugyldige data
stdlib.path.io.timed_out = tidsgrænsen udløb
stdlib.path.io.interrupted = afbrudt
stdlib.path.io.would_block = ville blokere
stdlib.path.io.write_zero = nul byte skrevet
stdlib.path.io.unexpected_eof = uventet filslutning
stdlib.path.io.broken_pipe = brudt datakanal
stdlib.path.io.connection_refused = forbindelse afvist
stdlib.path.io.connection_reset = forbindelse nulstillet
stdlib.path.io.connection_aborted = forbindelse afbrudt
stdlib.path.io.not_connected = ikke forbundet
stdlib.path.io.addr_in_use = adressen er i brug
stdlib.path.io.addr_not_available = adressen er ikke tilgængelig
stdlib.path.io.out_of_memory = ikke mere hukommelse
stdlib.path.io.unsupported = understøttes ikke
stdlib.path.io.file_too_large = filen er for stor
stdlib.path.io.resource_busy = ressourcen er optaget
stdlib.path.io.executable_busy = programfilen er optaget
stdlib.path.io.deadlock = baglås
stdlib.path.io.crosses_devices = krydser enheder
stdlib.path.io.too_many_links = for mange kæder
stdlib.path.io.invalid_filename = ugyldigt filnavn
stdlib.path.io.arg_list_too_long = argumentlisten er for lang
stdlib.path.io.stale_handle = forældet netværksfilreference
stdlib.path.io.storage_full = lageret er fuldt
stdlib.path.io.not_seekable = kan ikke søges i
stdlib.path.io.network_down = netværket er nede
stdlib.path.io.network_unreachable = netværket kan ikke nås
stdlib.path.io.host_unreachable = værten kan ikke nås
stdlib.path.io.other = I/O-fejl
stdlib.path.action.canonicalize = kanonisering
stdlib.path.action.open_directory = åbning af mappe
stdlib.path.action.stat = opslag
stdlib.path.action.read = læsning
stdlib.path.action.open_file = åbning af fil
stdlib.path.with_suffix.empty_separator = with_suffix kræver en adskiller, der ikke er tom.
stdlib.path.relative_to.mismatch = { $path } er ikke relativ til { $root }.
stdlib.path.expanduser.unsupported = Brugerspecifik udvidelse af ~ understøttes ikke.
stdlib.path.expanduser.no_home = ~ kan ikke udvides: der er ingen miljøvariabler for hjemmemappen.
stdlib.path.contents.unsupported_encoding = Tegnkodningen "{ $encoding }" understøttes ikke.
stdlib.path.hash.unsupported_algorithm = Hash-algoritmen "{ $algorithm }" understøttes ikke.
stdlib.path.hash.unsupported_algorithm_legacy = Hash-algoritmen "{ $algorithm }" understøttes ikke (slå funktionen "{ $feature }" til).

# Diagnostik for samlingshjælpere.
stdlib.collections.flatten.expected_sequence = flatten forventede elementer fra en følge, men fandt { $kind }.
stdlib.collections.group_by.empty_attribute = group_by kræver en attribut, der ikke er tom.
stdlib.collections.group_by.unresolved = group_by kunne ikke slå "{ $attr }" op på et element af typen { $kind }.

# Diagnostik for tidshjælpere.
stdlib.time.offset.invalid = Forskydningen for now "{ $offset }" er ugyldig: forventede "+HH:MM[:SS]" eller "Z".
stdlib.time.timedelta.overflow = Overløb i timedelta ved tilføjelse af { $component }.
stdlib.time.label.weeks = uger
stdlib.time.label.days = dage
stdlib.time.label.hours = timer
stdlib.time.label.minutes = minutter
stdlib.time.label.seconds = sekunder
stdlib.time.label.milliseconds = millisekunder
stdlib.time.label.microseconds = mikrosekunder
stdlib.time.label.nanoseconds = nanosekunder

# Diagnostik for which-hjælperen.
stdlib.which.not_found = [netsuke::jinja::which::not_found] kommandoen "{ $command }" blev ikke fundet efter gennemgang af { $count } PATH-opslag. Uddrag: { $preview }
stdlib.which.not_found.hint.cwd_auto = Tomme dele af PATH ignoreres; brug cwd_mode="auto" for at medtage arbejdsmappen.
stdlib.which.not_found.hint.cwd_always = Sæt cwd_mode="always" for at medtage den aktuelle mappe.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] kommandoen "{ $command }" i "{ $path }" mangler eller kan ikke køres.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <tom>
stdlib.which.path_entry.non_utf8 = PATH-opslag nr. { $index } indeholder tegn, der ikke er UTF-8; Netsuke kræver UTF-8-stier.
stdlib.which.command.empty = which kræver en streng, der ikke er tom.
stdlib.which.cwd_mode.invalid = cwd_mode skal være "auto", "always" eller "never", men gav "{ $mode }".
stdlib.which.cwd.resolve_failed = Den aktuelle mappe kunne ikke bestemmes: { $details }.
stdlib.which.cwd.non_utf8 = Den aktuelle mappe indeholder dele, der ikke er UTF-8.
stdlib.which.canonicalize_failed = "{ $path }" kunne ikke kanoniseres: { $details }.
stdlib.which.is_executable = Det kunne ikke afgøres, om "{ $path }" kan køres: { $details }.
stdlib.which.canonicalize_non_utf8 = Den kanoniske sti indeholder dele, der ikke er UTF-8.
stdlib.which.workspace_non_utf8 = Stien til arbejdsområdet indeholder dele, der ikke er UTF-8, under opslag af kommandoen "{ $command }": { $path }.
stdlib.which.walkdir_error = Fejl under gennemgang af arbejdsområdet ved opslag af kommandoen: { $details }.

# Registrering af standardbiblioteket.
stdlib.register.open_dir = Den aktuelle mappe kunne ikke åbnes til registrering af stdlib.
stdlib.register.resolve_dir = Den aktuelle mappe kunne ikke bestemmes til registrering af stdlib.
stdlib.register.dir_non_utf8 = Den aktuelle mappe indeholder dele, der ikke er UTF-8: { $path }.

# Statusrapportering i tilgængelig outputtilstand.
status.state.pending = afventer
status.state.running = i gang
status.state.done = færdig
status.state.failed = mislykkedes
status.stage.label = Trin { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Opgave { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Læser manifestfilen
status.stage.initial_yaml_parsing = Indlæser YAML-dokumentet
status.stage.template_expansion = Udfolder skabelondirektiver
status.stage.final_rendering = Deserialiserer og gengiver manifestets værdier
status.stage.ir_generation_validation = Bygger og validerer afhængighedsgrafen
status.stage.ninja_synthesis = Danner Ninja-byggeplanen
status.stage.ninja_synthesis_execute = Danner Ninja-planen og kører { $tool }
status.stage.graph_rendering = Gengiver grafartefaktet
status.stage.graph_rendering_with_tool = Gengiver { $tool }
status.complete = { $tool } fuldført.
status.timing.summary_header = Tidsopsummering pr. trin:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Samlet tid for kæden: { $duration }
status.tool.build = Bygning
status.tool.clean = Oprydning
status.tool.graph = Graf
status.tool.graph_html = Graf (HTML)
status.tool.generate = Generering
status.tool.help_targets = Hjælp til mål

# Tekster til HTML-gengivelsen af grafen.
graph.html.title = Netsuke-byggegraf
graph.html.heading = Netsuke-byggegraf
graph.html.description = Byggegraf gengivet af Netsuke
graph.html.outline.summary = Mål og afhængigheder (tekstoversigt)
graph.html.outline.no_inputs = Ingen input
graph.html.noscript.notice = JavaScript er slået fra. Tekstoversigten ovenfor er hele grafen; DOT-kildeteksten følger nedenfor.

# Semantiske præfikser til tilgængeligt output.
semantic.prefix.error = Fejl:
semantic.prefix.warning = Advarsel:
semantic.prefix.success = Succes:
semantic.prefix.info = Info:
semantic.prefix.timing = Tid:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Eksempler på flertalsformer til oversættere.
# Dansk bruger CLDR-kategorierne `one` og `other` som kildesproget.
example.files_processed = { $count ->
    [one] Behandlede { $count } fil.
   *[other] Behandlede { $count } filer.
}

example.errors_found = { $count ->
    [0] Ingen fejl fundet.
    [one] { $count } fejl fundet.
   *[other] { $count } fejl fundet.
}
