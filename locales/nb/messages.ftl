# Lokaliseringsressurser for kommandolinjen til Netsuke (bokmål).

cli.about = Netsuke kompilerer YAML- + Jinja-manifester til Ninja-byggeplaner.
cli.long_about = Netsuke gjør YAML- + Jinja-manifester om til reproduserbare Ninja-grafer og kjører Ninja med trygge standardverdier.
cli.usage = { $usage }

# Hjelpetekst for globale valg.
cli.flag.file.help = Sti til Netsuke-manifestfilen som skal brukes.
cli.flag.directory.help = Kjør som om starten skjedde i denne katalogen.
cli.flag.config.help = Sti til en konfigurasjonsfil, utenom det automatiske søket.
cli.flag.jobs.help = Angi antallet parallelle byggejobber.
cli.flag.verbose.help = Slå på utførlig diagnoselogging og tidsoppsummeringer ved avslutning.
cli.flag.locale.help = Språkmerke for tekstene på kommandolinjen (for eksempel: en-US, nb).
cli.flag.fetch_allow_scheme.help = Flere URL-skjemaer som fetch-hjelperen kan bruke.
cli.flag.fetch_allow_host.help = Vertsnavn som tillates når standardavslaget er slått på.
cli.flag.fetch_block_host.help = Vertsnavn som alltid blokkeres, selv om de tillates andre steder.
cli.flag.fetch_default_deny.help = Avvis alle verter som standard; tillat bare den oppgitte listen.
cli.flag.json.help = Skriv ut maskinlesbar JSON.
cli.flag.no_input.help = Les aldri interaktive inndata.
cli.flag.color.help = Regel for farget utdata (auto, always, never).
cli.flag.emoji.help = Regel for emoji (auto, always, never).
cli.flag.progress.help = Regel for visning av framdrift (auto, always, never).
cli.flag.accessibility.help = Regel for tilgjengelig utdata (auto, on, off).
cli.flag.default_targets.help = Standardmål for byggingen når ingen er oppgitt.

# Beskrivelser av underkommandoer.
cli.subcommand.build.about = Bygg målene som er definert i manifestet (standard).
cli.subcommand.build.long_about = Bygg de forespurte målene; er ingen oppgitt, brukes standardmålene fra manifestet.
cli.subcommand.clean.about = Fjern byggeartefakter via Ninja.
cli.subcommand.clean.long_about = Lag en midlertidig Ninja-fil og kjør deretter `ninja -t clean`.
cli.subcommand.graph.about = Skriv ut avhengighetsgrafen for byggingen. Standardformatet er DOT.
cli.subcommand.graph.long_about = Overfør det innleste Netsuke-manifestet til en kanonisk byggegraf og skriv den som Graphviz DOT, eller som en frittstående HTML-side med `--html`. Bruk `--output <FIL>` for å skrive til en fil; `-` skriver til stdout.
cli.subcommand.generate.about = Lag Ninja-manifestet uten å kjøre Ninja.
cli.subcommand.generate.long_about = Skriv det genererte Ninja-manifestet til stdout eller til en fil valgt med `--output`.

# Hjelpetekst for valg til underkommandoen build.
cli.subcommand.build.flag.targets.help = Mål som skal bygges (bruker standardmålene fra manifestet hvis utelatt).

# Hjelpetekst for valg til underkommandoen graph.
cli.subcommand.graph.flag.html.help = Gjengi grafen som en frittstående HTML-side i stedet for DOT.
cli.subcommand.graph.flag.output.help = Skriv grafartefaktet til FIL; bruk `-` for stdout.

# Hjelpetekst for valg til underkommandoen generate.
cli.subcommand.generate.flag.output.help = Skriv det genererte Ninja-manifestet til FIL i stedet for stdout.

# Valideringsfeil på kommandolinjen.
cli.validation.jobs.invalid_number = { $value } er ikke et gyldig tall.
cli.validation.jobs.out_of_range = Antallet jobber må ligge mellom { $min } og { $max }.
cli.validation.scheme.empty = Skjemaet kan ikke være tomt.
cli.validation.scheme.invalid_start = Skjemaet «{ $scheme }» må begynne med en ASCII-bokstav.
cli.validation.scheme.invalid = Ugyldig skjema «{ $scheme }».
cli.validation.locale.empty = Språkmerket kan ikke være tomt.
cli.validation.locale.invalid = Ugyldig språkmerke «{ $locale }».
cli.validation.color.invalid = Ugyldig fargeregel «{ $value }». Gyldige valg: auto, always, never.
cli.validation.emoji.invalid = Ugyldig emojiregel «{ $value }». Gyldige valg: auto, always, never.
cli.validation.progress.invalid = Ugyldig framdriftsregel «{ $value }». Gyldige valg: auto, always, never.
cli.validation.accessibility.invalid = Ugyldig tilgjengelighetsregel «{ $value }». Gyldige valg: auto, on, off.
cli.validation.config.expected_object = Verdiene fra kommandolinjen skulle serialiseres til et objekt, men ga { $value }.

# Feilmeldinger fra Clap.
clap-error-missing-argument = Mangler påkrevd argument: { $argument }
clap-error-missing-subcommand = Mangler underkommando. Tilgjengelige valg: { $valid_subcommands }
clap-error-unknown-argument = Ukjent argument: { $argument }
clap-error-invalid-value = Ugyldig verdi for { $argument }: { $value }
clap-error-invalid-subcommand = Ukjent underkommando: { $subcommand }
# Merk: value-validation er formulert annerledes enn invalid-value for å skille
# feil fra egne validatorer (ErrorKind::ValueValidation) fra typekonflikter
# (ErrorKind::InvalidValue).
clap-error-value-validation = Valideringen mislyktes for { $argument }: { $value }

# Feil og sammenheng fra kjøringen.
runner.manifest.not_found = Manifestet «{ $manifest_name }» ble ikke funnet i { $directory }.
runner.manifest.not_found.help = Kontroller at manifestet finnes, eller oppgi `--file` med riktig sti.
runner.manifest.path_missing_name = Manifeststien «{ $path }» mangler filnavn.
runner.manifest.path_utf8 = Manifeststien «{ $path }» er ikke gyldig UTF-8.
runner.manifest.directory_utf8 = Stien til manifestkatalogen «{ $path }» er ikke gyldig UTF-8.
runner.manifest.directory_label = katalogen `{ $directory }`
runner.manifest.current_directory_label = gjeldende katalog
runner.context.network_policy = Nettverksregelen kunne ikke bygges.
runner.context.load_manifest = Manifestet i { $path } kunne ikke lastes inn.
runner.context.serialise_manifest = Manifestet kunne ikke serialiseres.
runner.context.build_graph = Grafen kunne ikke bygges ut fra manifestet.
runner.context.generate_ninja = Ninja-manifestet kunne ikke lages.
runner.context.render_graph = Grafartefaktet kunne ikke gjengis.

runner.io.create_temp_file = Den midlertidige Ninja-filen kunne ikke opprettes.
runner.io.write_temp_ninja = Den midlertidige Ninja-filen kunne ikke skrives.
runner.io.flush_temp_ninja = Bufferen for den midlertidige Ninja-filen kunne ikke tømmes.
runner.io.sync_temp_ninja = Den midlertidige Ninja-filen kunne ikke synkroniseres.
runner.io.create_parent_dir = Overkatalogen { $path } kunne ikke opprettes.
runner.io.create_ninja_file = Ninja-filen i { $path } kunne ikke opprettes.
runner.io.write_ninja_file = Ninja-filen i { $path } kunne ikke skrives.
runner.io.flush_ninja_file = Bufferen for Ninja-filen i { $path } kunne ikke tømmes.
runner.io.sync_ninja_file = Ninja-filen i { $path } kunne ikke synkroniseres.
runner.io.open_ambient_dir = Den omgivende katalogen kunne ikke åpnes.
runner.io.no_existing_ancestor = Det finnes ingen overordnet katalog for { $path }.
runner.io.derive_relative_path = Den relative Ninja-stien kunne ikke utledes.
runner.io.non_utf8_path = Stier som ikke er UTF-8, støttes ikke (sti: { $path }).
runner.io.write_stdout = Ninja-manifestet kunne ikke skrives til stdout.
runner.io.flush_stdout = Bufferen for stdout kunne ikke tømmes.

# Manifestdiagnostikk.
manifest.parse = Innlesingen av manifestet mislyktes.
manifest.structure_error = Strukturfeil i manifestet ved { $name }: { $details }
manifest.yaml.parse = YAML-feil på linje { $line }, kolonne { $column }: { $details }
manifest.yaml.label = ugyldig YAML
manifest.yaml.hint.tabs = YAML tillater ikke tabulatorer; bruk mellomrom til innrykk.
manifest.yaml.hint.list_item = YAML-listeelementer må begynne med «-» og ha riktig innrykk.
manifest.yaml.hint.expected_colon = Dette ser ut som en oppføring i en tilordning; det mangler et «:» etter nøkkelen.
manifest.yaml.hint.mapping_values = YAML-tilordninger krever en verdi etter «:» (eller en blokk med innrykk).
manifest.yaml.hint.invalid_token = YAML-symbolet er ugyldig eller uventet.
manifest.yaml.hint.escape = Escape omvendte skråstreker, eller fjern ugyldige escape-sekvenser.
manifest.env.missing = En påkrevd miljøvariabel er ikke satt.
manifest.env.invalid_utf8 = En miljøvariabel inneholder ugyldig UTF-8.
manifest.vars.not_object = `vars` i manifestet må være en tilordning eller et objekt.
manifest.vars.reserved_name = Manifestets `vars`-nøkkel '{ $name }' er reservert for en innebygd malhjelper; gi variabelen et nytt navn.
manifest.read_failed = Manifestet i { $path } kunne ikke leses.
manifest.resolve_workspace_root = Roten til arbeidsområdet kunne ikke bestemmes.
manifest.workspace_non_utf8 = Rotstien til arbeidsområdet «{ $path }» er ikke gyldig UTF-8.
manifest.path_non_utf8 = Stien til manifestet «{ $manifest }» er ikke gyldig UTF-8: { $path }.
manifest.path_missing_name = Manifeststien «{ $path }» mangler filnavn.
manifest.open_workspace_failed = Arbeidsområdet { $workspace } kunne ikke åpnes for manifestet { $manifest }.
manifest.foreach.not_iterable = Uttrykket `foreach` kan ikke itereres over.
manifest.foreach.serialise_item = Elementet i `foreach` kunne ikke serialiseres.
manifest.when.empty = Uttrykket `when` kan ikke være tomt.
manifest.when.eval_error = Uttrykket `when` «{ $expr }» kunne ikke evalueres.
manifest.when.template_error = Malen `when` «{ $expr }» kunne ikke gjengis.
manifest.target.vars_not_object = `vars` for målet må være et objekt, men ga { $value }.
manifest.vars.entry_not_object = En `vars`-oppføring i manifestet må være et objekt.
manifest.field_not_string = Feltet «{ $field }» må være en streng.
manifest.expression.parse_error = Uttrykket { $name } kunne ikke leses inn.
manifest.expression.eval_error = Uttrykket { $name } kunne ikke evalueres.

# Diagnostikk for manifestmakroer.
manifest.macro.signature_missing_identifier = Makrosignaturen mangler en identifikator.
manifest.macro.signature_missing_params = Makrosignaturen mangler parametere.
manifest.macro.compile_failed = Makroen { $name } kunne ikke kompileres.
manifest.macro.sequence_invalid = Makroer må defineres som en tilordning fra navn til maler.
manifest.macro.register_failed = Makroene i manifestet kunne ikke registreres.
manifest.macro.not_initialised = Makromiljøet er ikke klargjort.
manifest.macro.caller_invalid = Kalleren til makroen må være en streng.
manifest.macro.template_load_failed = Makromalen kunne ikke lastes inn.
manifest.macro.init_failed = Makromiljøet kunne ikke klargjøres.
manifest.macro.missing = Makroen { $name } mangler.

# Glob-feil i manifestet.
manifest.glob.unmatched_brace = Ugyldig glob-mønster «{ $pattern }»: «{ $character }» uten motpart på posisjon { $position }.
manifest.glob.invalid_pattern = Ugyldig glob-mønster «{ $pattern }»: { $detail }.
manifest.glob.unknown_pattern_error = ukjent mønsterfeil.
manifest.glob.io_failed = Glob mislyktes for «{ $pattern }»: { $detail }.
manifest.glob.unknown_io_error = ukjent I/U-feil.
manifest.command_list_empty = Feltet «command» kan ikke være tomt: oppgi en kommandostreng eller en ikke-tom liste.

# Feil i den interne representasjonen.
ir.rule_not_found = Regelen «{ $rule }» som målet «{ $target }» viser til, ble ikke funnet.
ir.multiple_rules = Målet «{ $target }» må vise til nøyaktig én regel, men ga { $rules }.
ir.empty_rule = Målet «{ $target }» må vise til en regel.
ir.duplicate_outputs = Det ble funnet dupliserte utdata: { $outputs }.
ir.circular_dependency = Det ble funnet en sirkulær avhengighet: { $cycle }.
ir.action_serialisation = Handlingen kunne ikke serialiseres: { $details }.
ir.invalid_command = Ugyldig interpolasjon i kommandoen: { $snippet }.

# Feil ved generering av Ninja.
ninja_gen.missing_action = Handlingen «{ $id }» som en byggekant viser til, mangler.
ninja_gen.format = Utdataene fra Ninja-manifestet kunne ikke formateres.

# Validering av vertsmønstre.
host_pattern.empty = Vertsmønsteret kan ikke være tomt.
host_pattern.contains_scheme = Vertsmønsteret «{ $pattern }» kan ikke inneholde et URL-skjema.
host_pattern.contains_slash = Vertsmønsteret «{ $pattern }» kan ikke inneholde «/».
host_pattern.missing_suffix = Vertsmønsteret «{ $pattern }» må ha et suffiks etter «*.».
host_pattern.empty_label = Vertsmønsteret «{ $pattern }» inneholder en tom etikett.
host_pattern.invalid_chars = Vertsmønsteret «{ $pattern }» inneholder ugyldige tegn.
host_pattern.invalid_label_edge = Etiketter i vertsmønsteret «{ $pattern }» kan ikke begynne eller slutte med «-».
host_pattern.label_too_long = Vertsmønsteret «{ $pattern }» inneholder en etikett på over 63 tegn.
host_pattern.too_long = Vertsmønsteret «{ $pattern }» overskrider grensen på 255 tegn.

# Nettverksregler.
network_policy.scheme.empty = Skjemaet kan ikke være tomt.
network_policy.scheme.invalid = Skjemaet «{ $scheme }» inneholder ugyldige tegn.
network_policy.allowlist.empty = Listen over tillatte verter kan ikke være tom.
network_policy.scheme.not_allowed = Skjemaet «{ $scheme }» er ikke tillatt.
network_policy.missing_host = URL-adressen mangler vert.
network_policy.host.blocked = Verten «{ $host }» er blokkert av reglene.
network_policy.host.not_allowlisted = Verten «{ $host }» står ikke på listen over tillatte.

# Konfigurasjon av standardbiblioteket.
stdlib.config.default_fetch_cache_invalid = Standardstien til fetch-hurtiglageret må være relativ.
stdlib.config.default_which_cache_invalid = Standardkapasiteten for which-hurtiglageret må være positiv.
stdlib.config.workspace_root_absolute = Rotstien til arbeidsområdet må være absolutt.
stdlib.config.fetch_response_limit_positive = Svargrensen for fetch må være positiv.
stdlib.config.command_output_limit_positive = Grensen for fanget kommandoutdata må være positiv.
stdlib.config.command_stream_limit_positive = Strømgrensen for kommandoer må være positiv.
stdlib.config.which_cache_capacity_positive = Kapasiteten for which-hurtiglageret må være positiv.
stdlib.config.skip_dir_empty = Oppføringer over katalogene som hoppes over, kan ikke være tomme.
stdlib.config.skip_dir_navigation = Oppføringer over katalogene som hoppes over, kan ikke inneholde «..».
stdlib.config.skip_dir_separator = Oppføringer over katalogene som hoppes over, kan ikke inneholde stiskilletegn.
stdlib.config.fetch_cache_empty = Stien til fetch-hurtiglageret kan ikke være tom.
stdlib.config.fetch_cache_not_relative = Stien til fetch-hurtiglageret må være relativ, men ga { $path }.
stdlib.config.fetch_cache_escapes = Stien til fetch-hurtiglageret kan ikke gå utenfor arbeidsområdet: { $path }.
stdlib.config.open_workspace_root = Gjeldende katalog kunne ikke åpnes som rot for stdlib-arbeidsområdet.
stdlib.config.resolve_cwd = Gjeldende katalog kunne ikke bestemmes som rot for stdlib-arbeidsområdet.
stdlib.config.cwd_non_utf8 = Gjeldende katalog inneholder deler som ikke er UTF-8: { $path }.

# Diagnostikk for fetch-hjelperen.
stdlib.fetch.url_invalid = Ugyldig URL-adresse «{ $url }»: { $details }.
stdlib.fetch.disallowed = URL-adressen «{ $url }» er ikke tillatt: { $details }.
stdlib.fetch.failed = «{ $url }» kunne ikke hentes: { $details }.
stdlib.fetch.cache_read_failed = Oppføringen «{ $name }» i hurtiglageret kunne ikke leses: { $details }.
stdlib.fetch.cache_open_failed = Oppføringen «{ $name }» i hurtiglageret kunne ikke åpnes: { $details }.
stdlib.fetch.response_read_failed = Svaret fra «{ $url }» kunne ikke leses: { $details }.
stdlib.fetch.response_buffer_overflow = Bufferoverflyt under lesing av «{ $url }».
stdlib.fetch.cache_write_failed = Hurtiglageret for «{ $url }» kunne ikke skrives: { $details }.
stdlib.fetch.response_limit_exceeded = Svaret fra «{ $url }» oversteg grensen på { $limit } byte.
stdlib.fetch.cache_limit_exceeded = Det hurtiglagrede svaret «{ $name }» oversteg grensen på { $limit } byte.
stdlib.fetch.io_failed = { $action } mislyktes for { $path }: { $details }.
stdlib.fetch.action.sync_cache = synkronisering av fetch-hurtiglageret
stdlib.fetch.action.create_cache_dir = oppretting av katalogen for fetch-hurtiglageret
stdlib.fetch.action.open_cache_dir = åpning av katalogen for fetch-hurtiglageret
stdlib.fetch.action.stat_cache = oppslag på oppføringen i fetch-hurtiglageret
stdlib.fetch.action.open_cache_entry = åpning av oppføringen i fetch-hurtiglageret

# Diagnostikk for kommandohjelperen.
stdlib.command.location = kommandoen «{ $command }» i malen «{ $template }»
stdlib.command.spawn_failed = { $location } kunne ikke startes: { $details }.
stdlib.command.io_failed = { $location } mislyktes: { $details }.
stdlib.command.closed_input_early = Inndataene ble lukket før skrivingen til kommandoen var ferdig.
stdlib.command.broken_pipe = Brutt datakanal under kjøring av { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } ble avbrutt av et signal.
stdlib.command.exited_with_status = { $location } avsluttet med status { $status }.
stdlib.command.output_limit_exceeded = { $location } oversteg { $mode }-grensen på { $limit } byte for { $stream }.
stdlib.command.timeout = { $location } overskred tidsgrensen på { $seconds } sekunder.
stdlib.command.exit_status_suffix = (avslutningsstatus { $status })
stdlib.command.signal_suffix = (avbrutt av et signal)
stdlib.command.shell.empty = Skallkommandoen kan ikke være tom.
stdlib.command.grep.empty_pattern = Mønsteret til grep kan ikke være tomt.
stdlib.command.grep.flags_not_string = Flagg til grep må være strenger.
stdlib.command.quote.invalid = { $arg } kunne ikke settes i anførselstegn: { $details }.
stdlib.command.quote.line_break = Argumenter med vognretur eller linjeskift kan ikke settes trygt i anførselstegn.
stdlib.command.input_undefined = Inndataverdien er udefinert.
stdlib.command.tempfile.root_required = Roten til arbeidsområdet kreves for å opprette midlertidige kommandofiler.
stdlib.command.tempfile.create_failed = Den midlertidige kommandofilen kunne ikke opprettes: { $details }.
stdlib.command.options.invalid_utf8 = Nøkkelen til et kommandovalg må være gyldig UTF-8.
stdlib.command.option.mode_not_string = Utdatamodusen må være en streng.
stdlib.command.options.invalid_type = Kommandovalg må være et objekt.
stdlib.command.output.mode_unsupported = Utdatamodusen «{ $mode }» støttes ikke.
stdlib.command.output.mode.capture = fangst
stdlib.command.output.mode.streaming = strømming
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnostikk for stihjelperen.
stdlib.path.io.failed = { $action } mislyktes for { $path } ({ $label }).
stdlib.path.io.failed_with_detail = { $action } mislyktes for { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $action } mislyktes for { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = ikke funnet
stdlib.path.io.permission_denied = tilgang nektet
stdlib.path.io.already_exists = finnes allerede
stdlib.path.io.invalid_input = ugyldige inndata
stdlib.path.io.invalid_data = ugyldige data
stdlib.path.io.timed_out = tidsgrensen løp ut
stdlib.path.io.interrupted = avbrutt
stdlib.path.io.would_block = ville blokkere
stdlib.path.io.write_zero = ingen byte skrevet
stdlib.path.io.unexpected_eof = uventet filslutt
stdlib.path.io.broken_pipe = brutt datakanal
stdlib.path.io.connection_refused = tilkoblingen ble avvist
stdlib.path.io.connection_reset = tilkoblingen ble nullstilt
stdlib.path.io.connection_aborted = tilkoblingen ble avbrutt
stdlib.path.io.not_connected = ikke tilkoblet
stdlib.path.io.addr_in_use = adressen er i bruk
stdlib.path.io.addr_not_available = adressen er ikke tilgjengelig
stdlib.path.io.out_of_memory = tomt for minne
stdlib.path.io.unsupported = støttes ikke
stdlib.path.io.file_too_large = filen er for stor
stdlib.path.io.resource_busy = ressursen er opptatt
stdlib.path.io.executable_busy = programfilen er opptatt
stdlib.path.io.deadlock = vranglås
stdlib.path.io.crosses_devices = krysser enheter
stdlib.path.io.too_many_links = for mange lenker
stdlib.path.io.invalid_filename = ugyldig filnavn
stdlib.path.io.arg_list_too_long = argumentlisten er for lang
stdlib.path.io.stale_handle = utdatert filhåndtak i nettverket
stdlib.path.io.storage_full = lageret er fullt
stdlib.path.io.not_seekable = kan ikke søkes i
stdlib.path.io.network_down = nettverket er nede
stdlib.path.io.network_unreachable = nettverket kan ikke nås
stdlib.path.io.host_unreachable = verten kan ikke nås
stdlib.path.io.other = I/U-feil
stdlib.path.action.canonicalize = kanonisering
stdlib.path.action.open_directory = åpning av katalog
stdlib.path.action.stat = oppslag
stdlib.path.action.read = lesing
stdlib.path.action.open_file = åpning av fil
stdlib.path.with_suffix.empty_separator = with_suffix krever et skilletegn som ikke er tomt.
stdlib.path.relative_to.mismatch = { $path } er ikke relativ til { $root }.
stdlib.path.expanduser.unsupported = Brukerspesifikk utvidelse av ~ støttes ikke.
stdlib.path.expanduser.no_home = ~ kan ikke utvides: ingen miljøvariabler for hjemmekatalogen er satt.
stdlib.path.contents.unsupported_encoding = Tegnkodingen «{ $encoding }» støttes ikke.
stdlib.path.hash.unsupported_algorithm = Hash-algoritmen «{ $algorithm }» støttes ikke.
stdlib.path.hash.unsupported_algorithm_legacy = Hash-algoritmen «{ $algorithm }» støttes ikke (slå på funksjonen «{ $feature }»).

# Diagnostikk for samlingshjelpere.
stdlib.collections.flatten.expected_sequence = flatten ventet elementer fra en sekvens, men fant { $kind }.
stdlib.collections.group_by.empty_attribute = group_by krever et attributt som ikke er tomt.
stdlib.collections.group_by.unresolved = group_by kunne ikke slå opp «{ $attr }» på et element av typen { $kind }.

# Diagnostikk for tidshjelpere.
stdlib.time.offset.invalid = Forskyvningen for now «{ $offset }» er ugyldig: ventet «+HH:MM[:SS]» eller «Z».
stdlib.time.timedelta.overflow = Overflyt i timedelta ved tillegg av { $component }.
stdlib.time.label.weeks = uker
stdlib.time.label.days = dager
stdlib.time.label.hours = timer
stdlib.time.label.minutes = minutter
stdlib.time.label.seconds = sekunder
stdlib.time.label.milliseconds = millisekunder
stdlib.time.label.microseconds = mikrosekunder
stdlib.time.label.nanoseconds = nanosekunder

# Diagnostikk for which-hjelperen.
stdlib.which.not_found = [netsuke::jinja::which::not_found] kommandoen «{ $command }» ble ikke funnet etter gjennomgang av { $count } PATH-oppføringer. Utdrag: { $preview }
stdlib.which.not_found.hint.cwd_auto = Tomme deler av PATH ignoreres; bruk cwd_mode="auto" for å ta med arbeidskatalogen.
stdlib.which.not_found.hint.cwd_always = Sett cwd_mode="always" for å ta med gjeldende katalog.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] kommandoen «{ $command }» i «{ $path }» mangler eller kan ikke kjøres.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <tom>
stdlib.which.path_entry.non_utf8 = PATH-oppføring nr. { $index } inneholder tegn som ikke er UTF-8; Netsuke krever UTF-8-stier.
stdlib.which.command.empty = which krever en streng som ikke er tom.
stdlib.which.cwd_mode.invalid = cwd_mode må være «auto», «always» eller «never», men ga «{ $mode }».
stdlib.which.cwd.resolve_failed = Gjeldende katalog kunne ikke bestemmes: { $details }.
stdlib.which.cwd.non_utf8 = Gjeldende katalog inneholder deler som ikke er UTF-8.
stdlib.which.canonicalize_failed = «{ $path }» kunne ikke kanoniseres: { $details }.
stdlib.which.is_executable = Det kunne ikke avgjøres om «{ $path }» kan kjøres: { $details }.
stdlib.which.canonicalize_non_utf8 = Den kanoniske stien inneholder deler som ikke er UTF-8.
stdlib.which.workspace_non_utf8 = Stien til arbeidsområdet inneholder deler som ikke er UTF-8 under oppslag av kommandoen «{ $command }»: { $path }.
stdlib.which.walkdir_error = Feil under gjennomgang av arbeidsområdet ved oppslag av kommandoen: { $details }.

# Registrering av standardbiblioteket.
stdlib.register.open_dir = Gjeldende katalog kunne ikke åpnes for registrering av stdlib.
stdlib.register.resolve_dir = Gjeldende katalog kunne ikke bestemmes for registrering av stdlib.
stdlib.register.dir_non_utf8 = Gjeldende katalog inneholder deler som ikke er UTF-8: { $path }.

# Statusrapportering for tilgjengelig utdatamodus.
status.state.pending = venter
status.state.running = pågår
status.state.done = ferdig
status.state.failed = mislyktes
status.stage.label = Trinn { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Oppgave { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Leser manifestfilen
status.stage.initial_yaml_parsing = Leser inn YAML-dokumentet
status.stage.template_expansion = Utvider maldirektiver
status.stage.final_rendering = Deserialiserer og gjengir verdiene i manifestet
status.stage.ir_generation_validation = Bygger og validerer avhengighetsgrafen
status.stage.ninja_synthesis = Lager Ninja-byggeplanen
status.stage.ninja_synthesis_execute = Lager Ninja-planen og kjører { $tool }
status.stage.graph_rendering = Gjengir grafartefaktet
status.stage.graph_rendering_with_tool = Gjengir { $tool }
status.complete = { $tool } fullført.
status.timing.summary_header = Tidsoppsummering per trinn:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Samlet tid for kjeden: { $duration }
status.tool.build = Bygging
status.tool.clean = Opprydding
status.tool.graph = Graf
status.tool.graph_html = Graf (HTML)
status.tool.generate = Generering

# Tekster for HTML-gjengivelsen av grafen.
graph.html.title = Netsuke-byggegraf
graph.html.heading = Netsuke-byggegraf
graph.html.description = Byggegraf gjengitt av Netsuke
graph.html.outline.summary = Mål og avhengigheter (tekstoversikt)
graph.html.outline.no_inputs = Ingen inndata
graph.html.noscript.notice = JavaScript er slått av. Tekstoversikten over er hele grafen; DOT-kilden følger under.

# Semantiske prefikser for tilgjengelig utdata.
semantic.prefix.error = Feil:
semantic.prefix.warning = Advarsel:
semantic.prefix.success = Vellykket:
semantic.prefix.info = Info:
semantic.prefix.timing = Tid:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Eksempler på flertallsformer for oversettere.
# Bokmål bruker CLDR-kategoriene `one` og `other`, som kildespråket.
example.files_processed = { $count ->
    [one] Behandlet { $count } fil.
   *[other] Behandlet { $count } filer.
}

example.errors_found = { $count ->
    [0] Ingen feil funnet.
    [one] { $count } feil funnet.
   *[other] { $count } feil funnet.
}
