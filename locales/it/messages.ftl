# Risorse di localizzazione per la CLI di Netsuke.

cli.about = Netsuke compila manifest YAML + Jinja in piani di build Ninja.
cli.long_about = Netsuke trasforma manifest YAML + Jinja in grafi Ninja riproducibili ed esegue Ninja con impostazioni predefinite sicure.
cli.usage = { $usage }

# Testo di aiuto delle opzioni globali.
cli.flag.file.help = Percorso del file manifest Netsuke da usare.
cli.flag.directory.help = Esegui come se fosse stato avviato in questa directory.
cli.flag.config.help = Percorso di un file di configurazione, ignorando la ricerca automatica.
cli.flag.jobs.help = Imposta il numero di job di build in parallelo.
cli.flag.verbose.help = Abilita log diagnostici dettagliati e riepiloghi dei tempi al termine.
cli.flag.locale.help = Tag di lingua per i testi della CLI (per esempio: en-US, it).
cli.flag.fetch_allow_scheme.help = Schemi URL aggiuntivi consentiti per l'helper fetch.
cli.flag.fetch_allow_host.help = Nomi host consentiti quando il diniego predefinito è attivo.
cli.flag.fetch_block_host.help = Nomi host sempre bloccati, anche se consentiti altrove.
cli.flag.fetch_default_deny.help = Nega tutti gli host per impostazione predefinita; consenti solo l'elenco dichiarato.
cli.flag.json.help = Produci output JSON leggibile da una macchina.
cli.flag.no_input.help = Non leggere mai input interattivo.
cli.flag.color.help = Criterio per l'output a colori (auto, always, never).
cli.flag.emoji.help = Criterio per le emoji (auto, always, never).
cli.flag.progress.help = Criterio di visualizzazione dell'avanzamento (auto, always, never).
cli.flag.accessibility.help = Criterio per l'output accessibile (auto, on, off).
cli.flag.default_targets.help = Target di build predefiniti quando non ne viene indicato alcuno.

# Descrizioni dei sottocomandi.
cli.subcommand.build.about = Compila i target definiti nel manifest (predefinito).
cli.subcommand.build.long_about = Compila i target richiesti; se non ne vengono indicati, usa quelli predefiniti del manifest.
cli.subcommand.clean.about = Rimuovi gli artefatti di build tramite Ninja.
cli.subcommand.clean.long_about = Genera un file Ninja temporaneo, quindi esegui `ninja -t clean`.
cli.subcommand.graph.about = Emetti il grafo delle dipendenze di build. Il formato predefinito è DOT.
cli.subcommand.graph.long_about = Proietta il manifest Netsuke analizzato in un grafo di build canonico e scrivilo come Graphviz DOT, oppure come pagina HTML autonoma con `--html`. Usa `--output <FILE>` per scrivere su file; `-` scrive su stdout.
cli.subcommand.generate.about = Genera il manifest Ninja senza eseguire Ninja.
cli.subcommand.generate.long_about = Scrivi il manifest Ninja generato su stdout oppure nel file scelto con `--output`.

# Testo di aiuto delle opzioni del sottocomando build.
cli.subcommand.build.flag.targets.help = Target da compilare (se omesso usa quelli predefiniti del manifest).

# Testo di aiuto delle opzioni del sottocomando graph.
cli.subcommand.graph.flag.html.help = Genera il grafo come pagina HTML autonoma anziché come DOT.
cli.subcommand.graph.flag.output.help = Scrivi l'artefatto del grafo su FILE; usa `-` per stdout.

# Testo di aiuto delle opzioni del sottocomando generate.
cli.subcommand.generate.flag.output.help = Scrivi il manifest Ninja generato su FILE anziché su stdout.

# Errori di validazione della CLI.
cli.validation.jobs.invalid_number = { $value } non è un numero valido.
cli.validation.jobs.out_of_range = Il numero di job deve essere compreso tra { $min } e { $max }.
cli.validation.scheme.empty = Lo schema non deve essere vuoto.
cli.validation.scheme.invalid_start = Lo schema «{ $scheme }» deve iniziare con una lettera ASCII.
cli.validation.scheme.invalid = Schema non valido «{ $scheme }».
cli.validation.locale.empty = Il tag di lingua non deve essere vuoto.
cli.validation.locale.invalid = Tag di lingua non valido «{ $locale }».
cli.validation.color.invalid = Criterio di colore non valido «{ $value }». Opzioni valide: auto, always, never.
cli.validation.emoji.invalid = Criterio per le emoji non valido «{ $value }». Opzioni valide: auto, always, never.
cli.validation.progress.invalid = Criterio di avanzamento non valido «{ $value }». Opzioni valide: auto, always, never.
cli.validation.accessibility.invalid = Criterio di accessibilità non valido «{ $value }». Opzioni valide: auto, on, off.
cli.validation.config.expected_object = I valori della CLI dovevano essere serializzati in un oggetto, ricevuto { $value }.

# Messaggi di errore di Clap.
clap-error-missing-argument = Argomento obbligatorio mancante: { $argument }
clap-error-missing-subcommand = Sottocomando mancante. Opzioni disponibili: { $valid_subcommands }
clap-error-unknown-argument = Argomento sconosciuto: { $argument }
clap-error-invalid-value = Valore non valido per { $argument }: { $value }
clap-error-invalid-subcommand = Sottocomando sconosciuto: { $subcommand }
# Nota: value-validation usa una formulazione diversa da invalid-value per
# distinguere gli errori dei validatori personalizzati
# (ErrorKind::ValueValidation) dalle incompatibilità di tipo
# (ErrorKind::InvalidValue).
clap-error-value-validation = Validazione non riuscita per { $argument }: { $value }

# Errori e contesti del runner.
runner.manifest.not_found = Manifest «{ $manifest_name }» non trovato in { $directory }.
runner.manifest.not_found.help = Verifica che il manifest esista oppure indica `--file` con il percorso corretto.
runner.manifest.path_missing_name = Il percorso del manifest «{ $path }» non contiene un nome di file.
runner.manifest.path_utf8 = Il percorso del manifest «{ $path }» non è UTF-8 valido.
runner.manifest.directory_utf8 = Il percorso della directory del manifest «{ $path }» non è UTF-8 valido.
runner.manifest.directory_label = directory `{ $directory }`
runner.manifest.current_directory_label = la directory corrente
runner.context.network_policy = Impossibile costruire il criterio di rete.
runner.context.load_manifest = Impossibile caricare il manifest in { $path }.
runner.context.serialise_manifest = Impossibile serializzare il manifest.
runner.context.build_graph = Impossibile costruire il grafo a partire dal manifest.
runner.context.generate_ninja = Impossibile generare il manifest Ninja.
runner.context.render_graph = Impossibile generare l'artefatto del grafo.

runner.io.create_temp_file = Impossibile creare il file Ninja temporaneo.
runner.io.write_temp_ninja = Impossibile scrivere il file Ninja temporaneo.
runner.io.flush_temp_ninja = Impossibile svuotare il buffer del file Ninja temporaneo.
runner.io.sync_temp_ninja = Impossibile sincronizzare il file Ninja temporaneo.
runner.io.create_parent_dir = Impossibile creare la directory padre { $path }.
runner.io.create_ninja_file = Impossibile creare il file Ninja in { $path }.
runner.io.write_ninja_file = Impossibile scrivere il file Ninja in { $path }.
runner.io.flush_ninja_file = Impossibile svuotare il buffer del file Ninja in { $path }.
runner.io.sync_ninja_file = Impossibile sincronizzare il file Ninja in { $path }.
runner.io.open_ambient_dir = Impossibile aprire la directory ambientale.
runner.io.no_existing_ancestor = Nessuna directory antenata esistente per { $path }.
runner.io.derive_relative_path = Impossibile derivare il percorso Ninja relativo.
runner.io.non_utf8_path = I percorsi non UTF-8 non sono supportati (percorso: { $path }).
runner.io.write_stdout = Impossibile scrivere il manifest Ninja su stdout.
runner.io.flush_stdout = Impossibile svuotare il buffer di stdout.

# Diagnostica del manifest.
manifest.parse = Analisi del manifest non riuscita.
manifest.structure_error = Errore di struttura del manifest in { $name }: { $details }
manifest.yaml.parse = Errore di analisi YAML alla riga { $line }, colonna { $column }: { $details }
manifest.yaml.label = YAML non valido
manifest.yaml.hint.tabs = YAML non ammette tabulazioni; usa spazi per l'indentazione.
manifest.yaml.hint.list_item = Gli elementi di elenco YAML devono iniziare con «-» ed essere indentati correttamente.
manifest.yaml.hint.expected_colon = Sembra una voce di mappatura; manca il «:» dopo la chiave.
manifest.yaml.hint.mapping_values = Le mappature YAML richiedono un valore dopo «:» (oppure un blocco annidato).
manifest.yaml.hint.invalid_token = Il token YAML non è valido o è inatteso.
manifest.yaml.hint.escape = Usa l'escape per le barre rovesciate o rimuovi le sequenze di escape non valide.
manifest.env.missing = Una variabile d'ambiente richiesta non è impostata.
manifest.env.invalid_utf8 = Una variabile d'ambiente contiene UTF-8 non valido.
manifest.vars.not_object = `vars` del manifest deve essere una mappa o un oggetto.
manifest.vars.reserved_name = La chiave `vars` '{ $name }' del manifest è riservata a una funzione di supporto per i template integrata; rinomina la variabile.
manifest.read_failed = Impossibile leggere il manifest in { $path }.
manifest.resolve_workspace_root = Impossibile risolvere la radice dell'area di lavoro.
manifest.workspace_non_utf8 = Il percorso radice dell'area di lavoro «{ $path }» non è UTF-8 valido.
manifest.path_non_utf8 = Il percorso del manifest «{ $manifest }» non è UTF-8 valido: { $path }.
manifest.path_missing_name = Il percorso del manifest «{ $path }» non contiene un nome di file.
manifest.open_workspace_failed = Impossibile aprire l'area di lavoro { $workspace } per il manifest { $manifest }.
manifest.foreach.not_iterable = L'espressione `foreach` non è iterabile.
manifest.foreach.serialise_item = Impossibile serializzare l'elemento di `foreach`.
manifest.when.empty = L'espressione `when` non deve essere vuota.
manifest.when.eval_error = Impossibile valutare l'espressione `when` «{ $expr }».
manifest.when.template_error = Impossibile generare il template `when` «{ $expr }».
manifest.target.vars_not_object = `vars` del target deve essere un oggetto, ricevuto { $value }.
manifest.vars.entry_not_object = Una voce `vars` del manifest deve essere un oggetto.
manifest.field_not_string = Il campo «{ $field }» deve essere una stringa.
manifest.expression.parse_error = Impossibile analizzare l'espressione { $name }.
manifest.expression.eval_error = Impossibile valutare l'espressione { $name }.

# Diagnostica delle macro del manifest.
manifest.macro.signature_missing_identifier = Alla firma della macro manca un identificatore.
manifest.macro.signature_missing_params = Alla firma della macro mancano i parametri.
manifest.macro.compile_failed = Impossibile compilare la macro { $name }.
manifest.macro.sequence_invalid = Le macro devono essere definite come mappatura da nomi a template.
manifest.macro.register_failed = Impossibile registrare le macro del manifest.
manifest.macro.not_initialised = L'ambiente delle macro non è inizializzato.
manifest.macro.caller_invalid = Il chiamante della macro deve essere una stringa.
manifest.macro.template_load_failed = Impossibile caricare il template della macro.
manifest.macro.init_failed = Impossibile inizializzare l'ambiente delle macro.
manifest.macro.missing = La macro { $name } è mancante.

# Errori dei pattern glob del manifest.
manifest.glob.unmatched_brace = Pattern glob non valido «{ $pattern }»: «{ $character }» senza corrispondenza alla posizione { $position }.
manifest.glob.invalid_pattern = Pattern glob non valido «{ $pattern }»: { $detail }.
manifest.glob.unknown_pattern_error = errore di pattern sconosciuto.
manifest.glob.io_failed = Glob non riuscito per «{ $pattern }»: { $detail }.
manifest.glob.unknown_io_error = errore di I/O sconosciuto.
manifest.command_list_empty = Il campo «command» non deve essere vuoto: fornire una stringa di comando o un elenco non vuoto.

# Errori della rappresentazione intermedia.
ir.rule_not_found = La regola «{ $rule }» referenziata dal target «{ $target }» non è stata trovata.
ir.multiple_rules = Il target «{ $target }» deve referenziare una sola regola, ricevuto { $rules }.
ir.empty_rule = Il target «{ $target }» deve referenziare una regola.
ir.duplicate_outputs = Rilevati output duplicati: { $outputs }.
ir.circular_dependency = Rilevata dipendenza circolare: { $cycle }.
ir.action_serialisation = Impossibile serializzare l'azione: { $details }.
ir.invalid_command = Interpolazione del comando non valida: { $snippet }.

# Errori di generazione Ninja.
ninja_gen.missing_action = Manca l'azione «{ $id }» referenziata da un arco di build.
ninja_gen.format = Impossibile formattare l'output del manifest Ninja.

# Validazione dei pattern host.
host_pattern.empty = Il pattern host non deve essere vuoto.
host_pattern.contains_scheme = Il pattern host «{ $pattern }» non deve includere uno schema URL.
host_pattern.contains_slash = Il pattern host «{ $pattern }» non deve contenere «/».
host_pattern.missing_suffix = Il pattern host «{ $pattern }» deve includere un suffisso dopo «*.».
host_pattern.empty_label = Il pattern host «{ $pattern }» contiene un'etichetta vuota.
host_pattern.invalid_chars = Il pattern host «{ $pattern }» contiene caratteri non validi.
host_pattern.invalid_label_edge = Le etichette del pattern host «{ $pattern }» non devono iniziare o terminare con «-».
host_pattern.label_too_long = Il pattern host «{ $pattern }» contiene un'etichetta più lunga di 63 caratteri.
host_pattern.too_long = Il pattern host «{ $pattern }» supera il limite di 255 caratteri.

# Criteri di rete.
network_policy.scheme.empty = Lo schema non deve essere vuoto.
network_policy.scheme.invalid = Lo schema «{ $scheme }» contiene caratteri non validi.
network_policy.allowlist.empty = L'elenco degli host consentiti non deve essere vuoto.
network_policy.scheme.not_allowed = Lo schema «{ $scheme }» non è consentito.
network_policy.missing_host = L'URL non contiene un host.
network_policy.host.blocked = L'host «{ $host }» è bloccato dal criterio.
network_policy.host.not_allowlisted = L'host «{ $host }» non è nell'elenco dei consentiti.

# Configurazione della libreria standard.
stdlib.config.default_fetch_cache_invalid = Il percorso predefinito della cache di fetch deve essere relativo.
stdlib.config.default_which_cache_invalid = La capacità predefinita della cache di which deve essere positiva.
stdlib.config.workspace_root_absolute = Il percorso radice dell'area di lavoro deve essere assoluto.
stdlib.config.fetch_response_limit_positive = Il limite di risposta di fetch deve essere positivo.
stdlib.config.command_output_limit_positive = Il limite di cattura dell'output dei comandi deve essere positivo.
stdlib.config.command_stream_limit_positive = Il limite di streaming dei comandi deve essere positivo.
stdlib.config.which_cache_capacity_positive = La capacità della cache di which deve essere positiva.
stdlib.config.skip_dir_empty = Le voci di directory da ignorare non devono essere vuote.
stdlib.config.skip_dir_navigation = Le voci di directory da ignorare non devono contenere «..».
stdlib.config.skip_dir_separator = Le voci di directory da ignorare non devono contenere separatori di percorso.
stdlib.config.fetch_cache_empty = Il percorso della cache di fetch non deve essere vuoto.
stdlib.config.fetch_cache_not_relative = Il percorso della cache di fetch deve essere relativo, ricevuto { $path }.
stdlib.config.fetch_cache_escapes = Il percorso della cache di fetch non deve uscire dall'area di lavoro: { $path }.
stdlib.config.open_workspace_root = Impossibile aprire la directory corrente come radice dell'area di lavoro della stdlib.
stdlib.config.resolve_cwd = Impossibile risolvere la directory corrente come radice dell'area di lavoro della stdlib.
stdlib.config.cwd_non_utf8 = La directory corrente contiene componenti non UTF-8: { $path }.

# Diagnostica dell'helper fetch.
stdlib.fetch.url_invalid = URL non valido «{ $url }»: { $details }.
stdlib.fetch.disallowed = L'URL «{ $url }» non è consentito: { $details }.
stdlib.fetch.failed = Impossibile scaricare «{ $url }»: { $details }.
stdlib.fetch.cache_read_failed = Impossibile leggere la voce di cache «{ $name }»: { $details }.
stdlib.fetch.cache_open_failed = Impossibile aprire la voce di cache «{ $name }»: { $details }.
stdlib.fetch.response_read_failed = Impossibile leggere la risposta da «{ $url }»: { $details }.
stdlib.fetch.response_buffer_overflow = Overflow del buffer durante la lettura di «{ $url }».
stdlib.fetch.cache_write_failed = Impossibile scrivere la cache per «{ $url }»: { $details }.
stdlib.fetch.response_limit_exceeded = La risposta da «{ $url }» ha superato il limite di { $limit } byte.
stdlib.fetch.cache_limit_exceeded = La risposta in cache «{ $name }» ha superato il limite di { $limit } byte.
stdlib.fetch.io_failed = L'operazione di { $action } non è riuscita per { $path }: { $details }.
stdlib.fetch.action.sync_cache = sincronizzare la cache di fetch
stdlib.fetch.action.create_cache_dir = creare la directory di cache di fetch
stdlib.fetch.action.open_cache_dir = aprire la directory di cache di fetch
stdlib.fetch.action.stat_cache = interrogare la voce di cache di fetch
stdlib.fetch.action.open_cache_entry = aprire la voce di cache di fetch

# Diagnostica dell'helper dei comandi.
stdlib.command.location = comando «{ $command }» nel template «{ $template }»
stdlib.command.spawn_failed = Impossibile avviare { $location }: { $details }.
stdlib.command.io_failed = { $location } non riuscito: { $details }.
stdlib.command.closed_input_early = L'input si è chiuso prima di completare la scrittura verso il comando.
stdlib.command.broken_pipe = Pipe interrotta durante l'esecuzione di { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } è stato terminato da un segnale.
stdlib.command.exited_with_status = { $location } è terminato con stato { $status }.
stdlib.command.output_limit_exceeded = { $location } ha superato il limite { $mode } di { $limit } byte per { $stream }.
stdlib.command.timeout = { $location } ha superato il tempo limite di { $seconds } secondi.
stdlib.command.exit_status_suffix = (stato di uscita { $status })
stdlib.command.signal_suffix = (terminato da un segnale)
stdlib.command.shell.empty = Il comando shell non deve essere vuoto.
stdlib.command.grep.empty_pattern = Il pattern di grep non deve essere vuoto.
stdlib.command.grep.flags_not_string = Le opzioni di grep devono essere stringhe.
stdlib.command.quote.invalid = Impossibile applicare le virgolette a { $arg }: { $details }.
stdlib.command.quote.line_break = Gli argomenti con ritorni a capo o avanzamenti di riga non possono essere racchiusi tra virgolette in sicurezza.
stdlib.command.input_undefined = Il valore di input non è definito.
stdlib.command.tempfile.root_required = Per creare file temporanei dei comandi è necessaria la radice dell'area di lavoro.
stdlib.command.tempfile.create_failed = Impossibile creare il file temporaneo del comando: { $details }.
stdlib.command.options.invalid_utf8 = La chiave di un'opzione del comando deve essere UTF-8 valido.
stdlib.command.option.mode_not_string = La modalità di output deve essere una stringa.
stdlib.command.options.invalid_type = Le opzioni del comando devono essere un oggetto.
stdlib.command.output.mode_unsupported = Modalità di output non supportata «{ $mode }».
stdlib.command.output.mode.capture = cattura
stdlib.command.output.mode.streaming = streaming
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnostica dell'helper dei percorsi.
stdlib.path.io.failed = L'operazione di { $action } non è riuscita per { $path } ({ $label }).
stdlib.path.io.failed_with_detail = L'operazione di { $action } non è riuscita per { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = L'operazione di { $action } non è riuscita per { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = non trovato
stdlib.path.io.permission_denied = autorizzazione negata
stdlib.path.io.already_exists = già esistente
stdlib.path.io.invalid_input = input non valido
stdlib.path.io.invalid_data = dati non validi
stdlib.path.io.timed_out = tempo scaduto
stdlib.path.io.interrupted = interrotto
stdlib.path.io.would_block = si bloccherebbe
stdlib.path.io.write_zero = scrittura nulla
stdlib.path.io.unexpected_eof = fine del file inattesa
stdlib.path.io.broken_pipe = pipe interrotta
stdlib.path.io.connection_refused = connessione rifiutata
stdlib.path.io.connection_reset = connessione reimpostata
stdlib.path.io.connection_aborted = connessione interrotta
stdlib.path.io.not_connected = non connesso
stdlib.path.io.addr_in_use = indirizzo già in uso
stdlib.path.io.addr_not_available = indirizzo non disponibile
stdlib.path.io.out_of_memory = memoria esaurita
stdlib.path.io.unsupported = non supportato
stdlib.path.io.file_too_large = file troppo grande
stdlib.path.io.resource_busy = risorsa occupata
stdlib.path.io.executable_busy = eseguibile occupato
stdlib.path.io.deadlock = stallo
stdlib.path.io.crosses_devices = attraversa dispositivi diversi
stdlib.path.io.too_many_links = troppi collegamenti
stdlib.path.io.invalid_filename = nome di file non valido
stdlib.path.io.arg_list_too_long = elenco di argomenti troppo lungo
stdlib.path.io.stale_handle = handle di file di rete obsoleto
stdlib.path.io.storage_full = spazio di archiviazione esaurito
stdlib.path.io.not_seekable = posizionamento non consentito
stdlib.path.io.network_down = rete non attiva
stdlib.path.io.network_unreachable = rete irraggiungibile
stdlib.path.io.host_unreachable = host irraggiungibile
stdlib.path.io.other = errore di I/O
stdlib.path.action.canonicalize = canonicalizzare
stdlib.path.action.open_directory = aprire la directory
stdlib.path.action.stat = interrogare
stdlib.path.action.read = leggere
stdlib.path.action.open_file = aprire il file
stdlib.path.with_suffix.empty_separator = with_suffix richiede un separatore non vuoto.
stdlib.path.relative_to.mismatch = { $path } non è relativo a { $root }.
stdlib.path.expanduser.unsupported = L'espansione di ~ per uno specifico utente non è supportata.
stdlib.path.expanduser.no_home = Impossibile espandere ~: non è impostata alcuna variabile d'ambiente per la directory home.
stdlib.path.contents.unsupported_encoding = Codifica non supportata «{ $encoding }».
stdlib.path.hash.unsupported_algorithm = Algoritmo di hash non supportato «{ $algorithm }».
stdlib.path.hash.unsupported_algorithm_legacy = Algoritmo di hash non supportato «{ $algorithm }» (abilita la funzionalità «{ $feature }»).

# Diagnostica degli helper per le collezioni.
stdlib.collections.flatten.expected_sequence = flatten si aspettava elementi di sequenza ma ha trovato { $kind }.
stdlib.collections.group_by.empty_attribute = group_by richiede un attributo non vuoto.
stdlib.collections.group_by.unresolved = group_by non ha potuto risolvere «{ $attr }» su un elemento di tipo { $kind }.

# Diagnostica degli helper temporali.
stdlib.time.offset.invalid = L'offset di now «{ $offset }» non è valido: previsto «+HH:MM[:SS]» oppure «Z».
stdlib.time.timedelta.overflow = Overflow di timedelta durante l'aggiunta di { $component }.
stdlib.time.label.weeks = settimane
stdlib.time.label.days = giorni
stdlib.time.label.hours = ore
stdlib.time.label.minutes = minuti
stdlib.time.label.seconds = secondi
stdlib.time.label.milliseconds = millisecondi
stdlib.time.label.microseconds = microsecondi
stdlib.time.label.nanoseconds = nanosecondi

# Diagnostica dell'helper which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] comando «{ $command }» non trovato dopo aver controllato { $count } voci di PATH. Anteprima: { $preview }
stdlib.which.not_found.hint.cwd_auto = I segmenti vuoti di PATH vengono ignorati; usa cwd_mode="auto" per includere la directory di lavoro.
stdlib.which.not_found.hint.cwd_always = Imposta cwd_mode="always" per includere la directory corrente.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] il comando «{ $command }» in «{ $path }» è assente o non eseguibile.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <vuoto>
stdlib.which.path_entry.non_utf8 = La voce PATH n. { $index } contiene caratteri non UTF-8; Netsuke richiede percorsi UTF-8.
stdlib.which.command.empty = which richiede una stringa non vuota.
stdlib.which.cwd_mode.invalid = cwd_mode deve essere «auto», «always» o «never», ricevuto «{ $mode }».
stdlib.which.cwd.resolve_failed = Impossibile risolvere la directory corrente: { $details }.
stdlib.which.cwd.non_utf8 = La directory corrente contiene componenti non UTF-8.
stdlib.which.canonicalize_failed = Impossibile canonicalizzare «{ $path }»: { $details }.
stdlib.which.is_executable = Impossibile verificare se «{ $path }» è eseguibile: { $details }.
stdlib.which.canonicalize_non_utf8 = Il percorso canonico contiene componenti non UTF-8.
stdlib.which.workspace_non_utf8 = Il percorso dell'area di lavoro contiene componenti non UTF-8 durante la risoluzione del comando «{ $command }»: { $path }.
stdlib.which.walkdir_error = Errore nell'attraversamento dell'area di lavoro durante la risoluzione del comando: { $details }.

# Registrazione della libreria standard.
stdlib.register.open_dir = Impossibile aprire la directory corrente per la registrazione della stdlib.
stdlib.register.resolve_dir = Impossibile risolvere la directory corrente per la registrazione della stdlib.
stdlib.register.dir_non_utf8 = La directory corrente contiene componenti non UTF-8: { $path }.

# Segnalazione di stato per la modalità di output accessibile.
status.state.pending = in attesa
status.state.running = in corso
status.state.done = completata
status.state.failed = non riuscita
status.stage.label = Fase { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Attività { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Lettura del file manifest
status.stage.initial_yaml_parsing = Analisi del documento YAML
status.stage.template_expansion = Espansione delle direttive dei template
status.stage.final_rendering = Deserializzazione e rendering dei valori del manifest
status.stage.ir_generation_validation = Costruzione e validazione del grafo delle dipendenze
status.stage.ninja_synthesis = Sintesi del piano di build Ninja
status.stage.ninja_synthesis_execute = Sintesi del piano Ninja ed esecuzione di { $tool }
status.stage.graph_rendering = Rendering dell'artefatto del grafo
status.stage.graph_rendering_with_tool = Rendering di { $tool }
status.complete = { $tool }: operazione completata.
status.timing.summary_header = Riepilogo dei tempi per fase:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Tempo totale della pipeline: { $duration }
status.tool.build = Build
status.tool.clean = Pulizia
status.tool.graph = Grafo
status.tool.graph_html = Grafo (HTML)
status.tool.generate = Generazione

# Stringhe del renderer HTML del grafo.
graph.html.title = Grafo di build di Netsuke
graph.html.heading = Grafo di build di Netsuke
graph.html.description = Grafo di build generato da Netsuke
graph.html.outline.summary = Target e dipendenze (schema testuale)
graph.html.outline.no_inputs = Nessun input
graph.html.noscript.notice = JavaScript è disattivato. Lo schema testuale qui sopra contiene il grafo completo; segue il sorgente DOT.

# Prefissi semantici per l'output accessibile.
semantic.prefix.error = Errore:
semantic.prefix.warning = Avviso:
semantic.prefix.success = Operazione riuscita:
semantic.prefix.info = Info:
semantic.prefix.timing = Tempi:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Esempi di forme plurali per i traduttori.
# L'italiano usa le categorie CLDR `one` e `other`, come la lingua di origine.
example.files_processed = { $count ->
    [one] Elaborato { $count } file.
   *[other] Elaborati { $count } file.
}

example.errors_found = { $count ->
    [0] Nessun errore trovato.
    [one] Trovato { $count } errore.
   *[other] Trovati { $count } errori.
}
