# Resurse de localizare pentru linia de comandă Netsuke.

cli.about = Netsuke compilează manifeste YAML + Jinja în planuri de construire Ninja.
cli.long_about = Netsuke transformă manifestele YAML + Jinja în grafuri Ninja reproductibile și rulează Ninja cu valori implicite sigure.
cli.usage = { $usage }

# Textul de ajutor pentru opțiunile globale.
cli.flag.file.help = Calea către fișierul de manifest Netsuke care va fi folosit.
cli.flag.directory.help = Rulează ca și cum pornirea ar fi avut loc în acest director.
cli.flag.config.help = Calea către un fișier de configurare, ocolind căutarea automată.
cli.flag.jobs.help = Stabilește numărul de sarcini de construire care rulează în paralel.
cli.flag.verbose.help = Activează jurnalizarea de diagnostic detaliată și rezumatele de timp la final.
cli.flag.locale.help = Eticheta de limbă pentru textele liniei de comandă (de exemplu: en-US, ro).
cli.flag.fetch_allow_scheme.help = Scheme URL suplimentare permise pentru ajutorul fetch.
cli.flag.fetch_allow_host.help = Numele de gazde permise atunci când refuzul implicit este activ.
cli.flag.fetch_block_host.help = Numele de gazde blocate întotdeauna, chiar dacă sunt permise în altă parte.
cli.flag.fetch_default_deny.help = Refuză implicit toate gazdele; permite doar lista declarată.
cli.flag.json.help = Produce ieșire JSON care poate fi prelucrată automat.
cli.flag.no_input.help = Nu citi niciodată date introduse interactiv.
cli.flag.color.help = Politica de ieșire colorată (auto, always, never).
cli.flag.emoji.help = Politica pentru emoji (auto, always, never).
cli.flag.progress.help = Politica de afișare a progresului (auto, always, never).
cli.flag.accessibility.help = Politica de ieșire accesibilă (auto, on, off).
cli.flag.default_targets.help = Țintele de construire implicite când nu este indicată niciuna.

# Descrierile subcomenzilor.
cli.subcommand.build.about = Construiește țintele definite în manifest (implicit).
cli.subcommand.build.long_about = Construiește țintele cerute; dacă nu este indicată niciuna, folosește țintele implicite din manifest.
cli.subcommand.clean.about = Elimină artefactele de construire prin Ninja.
cli.subcommand.clean.long_about = Generează un fișier Ninja temporar, apoi rulează `ninja -t clean`.
cli.subcommand.graph.about = Emite graful dependențelor de construire. Formatul implicit este DOT.
cli.subcommand.graph.long_about = Proiectează manifestul Netsuke analizat într-un graf de construire canonic și îl scrie ca Graphviz DOT sau, cu `--html`, ca pagină HTML de sine stătătoare. Folosiți `--output <FIȘIER>` pentru a scrie într-un fișier; `-` scrie la ieșirea standard.
cli.subcommand.generate.about = Generează manifestul Ninja fără a rula Ninja.
cli.subcommand.generate.long_about = Scrie manifestul Ninja generat la ieșirea standard sau într-un fișier ales cu `--output`.
cli.subcommand.help.about = Afișează ajutorul de nivel superior sau ajutorul pentru un subiect specificat.
cli.subcommand.help.long_about = Fără subiect, acest lucru corespunde cu `--help`. Folosiți `help targets` pentru a afișa catalogul de ținte și acțiuni pentru fișierul selectat.

# Help catalogue headings and markers.
cli.help.actions_heading = Acțiuni:
cli.help.targets_heading = Ținte:
cli.help.targets.about = Listează țintele și acțiunile din fișierul selectat.
cli.help.default_marker = implicit

# Textul de ajutor pentru opțiunile subcomenzii build.
cli.subcommand.build.flag.targets.help = Țintele de construit (dacă lipsesc, se folosesc cele implicite din manifest).

# Textul de ajutor pentru opțiunile subcomenzii graph.
cli.subcommand.graph.flag.html.help = Redă graful ca pagină HTML de sine stătătoare în loc de format DOT.
cli.subcommand.graph.flag.output.help = Scrie artefactul grafului în FIȘIER; folosiți `-` pentru ieșirea standard.

# Textul de ajutor pentru opțiunile subcomenzii generate.
cli.subcommand.generate.flag.output.help = Scrie manifestul Ninja generat în FIȘIER în loc de ieșirea standard.

# Erori de validare la linia de comandă.
cli.validation.jobs.invalid_number = { $value } nu este un număr valid.
cli.validation.jobs.out_of_range = Numărul de sarcini trebuie să fie între { $min } și { $max }.
cli.validation.scheme.empty = Schema nu trebuie să fie goală.
cli.validation.scheme.invalid_start = Schema „{ $scheme }” trebuie să înceapă cu o literă ASCII.
cli.validation.scheme.invalid = Schemă nevalidă „{ $scheme }”.
cli.validation.locale.empty = Eticheta de limbă nu trebuie să fie goală.
cli.validation.locale.invalid = Etichetă de limbă nevalidă „{ $locale }”.
cli.validation.color.invalid = Politică de culoare nevalidă „{ $value }”. Opțiuni valide: auto, always, never.
cli.validation.emoji.invalid = Politică pentru emoji nevalidă „{ $value }”. Opțiuni valide: auto, always, never.
cli.validation.progress.invalid = Politică de progres nevalidă „{ $value }”. Opțiuni valide: auto, always, never.
cli.validation.accessibility.invalid = Politică de accesibilitate nevalidă „{ $value }”. Opțiuni valide: auto, on, off.
cli.validation.config.expected_object = Valorile liniei de comandă trebuiau serializate într-un obiect; s-a primit { $value }.

# Mesajele de eroare din Clap.
clap-error-missing-argument = Lipsește un argument obligatoriu: { $argument }
clap-error-missing-subcommand = Lipsește subcomanda. Opțiuni disponibile: { $valid_subcommands }
clap-error-unknown-argument = Argument necunoscut: { $argument }
clap-error-invalid-value = Valoare nevalidă pentru { $argument }: { $value }
clap-error-invalid-subcommand = Subcomandă necunoscută: { $subcommand }
# Notă: value-validation este formulat diferit de invalid-value pentru a
# deosebi erorile validatoarelor proprii (ErrorKind::ValueValidation) de
# nepotrivirile de tip (ErrorKind::InvalidValue).
clap-error-value-validation = Validarea a eșuat pentru { $argument }: { $value }

# Erori și context la execuție.
runner.manifest.not_found = Manifestul „{ $manifest_name }” nu a fost găsit în { $directory }.
runner.manifest.not_found.help = Verificați că manifestul există sau indicați `--file` cu calea corectă.
runner.manifest.path_missing_name = Calea manifestului „{ $path }” nu conține un nume de fișier.
runner.manifest.path_utf8 = Calea manifestului „{ $path }” nu este UTF-8 valid.
runner.manifest.directory_utf8 = Calea directorului manifestului „{ $path }” nu este UTF-8 valid.
runner.manifest.directory_label = directorul `{ $directory }`
runner.manifest.current_directory_label = directorul curent
runner.context.network_policy = Politica de rețea nu a putut fi construită.
runner.context.load_manifest = Manifestul din { $path } nu a putut fi încărcat.
runner.context.serialise_manifest = Manifestul nu a putut fi serializat.
runner.context.build_graph = Graful nu a putut fi construit din manifest.
runner.context.generate_ninja = Manifestul Ninja nu a putut fi generat.
runner.context.render_graph = Artefactul grafului nu a putut fi redat.

runner.io.create_temp_file = Fișierul Ninja temporar nu a putut fi creat.
runner.io.write_temp_ninja = Fișierul Ninja temporar nu a putut fi scris.
runner.io.flush_temp_ninja = Memoria tampon a fișierului Ninja temporar nu a putut fi golită.
runner.io.sync_temp_ninja = Fișierul Ninja temporar nu a putut fi sincronizat.
runner.io.create_parent_dir = Directorul părinte { $path } nu a putut fi creat.
runner.io.create_ninja_file = Fișierul Ninja din { $path } nu a putut fi creat.
runner.io.write_ninja_file = Fișierul Ninja din { $path } nu a putut fi scris.
runner.io.flush_ninja_file = Memoria tampon a fișierului Ninja din { $path } nu a putut fi golită.
runner.io.sync_ninja_file = Fișierul Ninja din { $path } nu a putut fi sincronizat.
runner.io.open_ambient_dir = Directorul înconjurător nu a putut fi deschis.
runner.io.no_existing_ancestor = Pentru { $path } nu există niciun director părinte.
runner.io.derive_relative_path = Calea Ninja relativă nu a putut fi dedusă.
runner.io.non_utf8_path = Căile care nu sunt UTF-8 nu sunt acceptate (calea: { $path }).
runner.io.write_stdout = Manifestul Ninja nu a putut fi scris la ieșirea standard.
runner.io.flush_stdout = Memoria tampon a ieșirii standard nu a putut fi golită.

# Diagnostice ale manifestului.
manifest.parse = Analiza manifestului a eșuat.
manifest.structure_error = Eroare de structură a manifestului la { $name }: { $details }
manifest.yaml.parse = Eroare de analiză YAML la linia { $line }, coloana { $column }: { $details }
manifest.yaml.label = YAML nevalid
manifest.yaml.hint.tabs = YAML nu permite tabulatori; folosiți spații pentru indentare.
manifest.yaml.hint.list_item = Elementele de listă YAML trebuie să înceapă cu „-” și să fie indentate corect.
manifest.yaml.hint.expected_colon = Pare o intrare de mapare; lipsește „:” după cheie.
manifest.yaml.hint.mapping_values = Mapările YAML cer o valoare după „:” (sau un bloc imbricat).
manifest.yaml.hint.invalid_token = Simbolul YAML este nevalid sau neașteptat.
manifest.yaml.hint.escape = Aplicați escape barelor oblice inverse sau eliminați secvențele de escape nevalide.
manifest.env.missing = O variabilă de mediu obligatorie nu este setată.
manifest.env.invalid_utf8 = O variabilă de mediu conține UTF-8 nevalid.
manifest.vars.not_object = Câmpul `vars` al manifestului trebuie să fie o mapare sau un obiect.
manifest.vars.reserved_name = Cheia `vars` '{ $name }' din manifest este rezervată pentru o funcție ajutătoare de șabloane integrată; redenumiți variabila.
manifest.read_failed = Manifestul din { $path } nu a putut fi citit.
manifest.resolve_workspace_root = Rădăcina spațiului de lucru nu a putut fi determinată.
manifest.workspace_non_utf8 = Calea rădăcină a spațiului de lucru „{ $path }” nu este UTF-8 valid.
manifest.path_non_utf8 = Calea manifestului „{ $manifest }” nu este UTF-8 valid: { $path }.
manifest.path_missing_name = Calea manifestului „{ $path }” nu conține un nume de fișier.
manifest.open_workspace_failed = Spațiul de lucru { $workspace } nu a putut fi deschis pentru manifestul { $manifest }.
manifest.foreach.not_iterable = Expresia `foreach` nu poate fi parcursă.
manifest.foreach.serialise_item = Elementul din `foreach` nu a putut fi serializat.
manifest.when.empty = Expresia `when` nu trebuie să fie goală.
manifest.when.eval_error = Expresia `when` „{ $expr }” nu a putut fi evaluată.
manifest.when.template_error = Șablonul `when` „{ $expr }” nu a putut fi redat.
manifest.target.vars_not_object = Câmpul `vars` al țintei trebuie să fie un obiect; s-a primit { $value }.
manifest.vars.entry_not_object = O intrare `vars` a manifestului trebuie să fie un obiect.
manifest.field_not_string = Câmpul „{ $field }” trebuie să fie un șir de caractere.
manifest.expression.parse_error = Expresia { $name } nu a putut fi analizată.
manifest.expression.eval_error = Expresia { $name } nu a putut fi evaluată.

# Diagnostice ale macrourilor din manifest.
manifest.macro.signature_missing_identifier = Din semnătura macroului lipsește un identificator.
manifest.macro.signature_missing_params = Din semnătura macroului lipsesc parametrii.
manifest.macro.compile_failed = Macroul { $name } nu a putut fi compilat.
manifest.macro.sequence_invalid = Macrourile trebuie definite ca o mapare de la nume la șabloane.
manifest.macro.register_failed = Macrourile manifestului nu au putut fi înregistrate.
manifest.macro.not_initialised = Mediul de macrouri nu este inițializat.
manifest.macro.caller_invalid = Apelantul macroului trebuie să fie un șir de caractere.
manifest.macro.template_load_failed = Șablonul macroului nu a putut fi încărcat.
manifest.macro.init_failed = Mediul de macrouri nu a putut fi inițializat.
manifest.macro.missing = Macroul { $name } lipsește.

# Erori ale tiparelor glob din manifest.
manifest.glob.unmatched_brace = Tipar glob nevalid „{ $pattern }”: „{ $character }” fără pereche la poziția { $position }.
manifest.glob.invalid_pattern = Tipar glob nevalid „{ $pattern }”: { $detail }.
manifest.glob.unknown_pattern_error = eroare de tipar necunoscută.
manifest.glob.io_failed = Glob a eșuat pentru „{ $pattern }”: { $detail }.
manifest.glob.unknown_io_error = eroare de intrare/ieșire necunoscută.
manifest.command_list_empty = Câmpul „command” nu trebuie să fie gol: furnizați un șir de comandă sau o listă nevidă.

# Erori ale reprezentării intermediare.
ir.rule_not_found = Regula „{ $rule }” la care face referire ținta „{ $target }” nu a fost găsită.
ir.multiple_rules = Ținta „{ $target }” trebuie să facă referire la o singură regulă; s-a primit { $rules }.
ir.empty_rule = Ținta „{ $target }” trebuie să facă referire la o regulă.
ir.duplicate_outputs = Au fost detectate ieșiri duplicate: { $outputs }.
ir.circular_dependency = A fost detectată o dependență circulară: { $cycle }.
ir.action_serialisation = Acțiunea nu a putut fi serializată: { $details }.
ir.invalid_command = Interpolare nevalidă în comandă: { $snippet }.

# Erori la generarea fișierelor Ninja.
ninja_gen.missing_action = Lipsește acțiunea „{ $id }” la care face referire o muchie de construire.
ninja_gen.format = Ieșirea manifestului Ninja nu a putut fi formatată.

# Validarea tiparelor de gazdă.
host_pattern.empty = Tiparul de gazdă nu trebuie să fie gol.
host_pattern.contains_scheme = Tiparul de gazdă „{ $pattern }” nu trebuie să conțină o schemă URL.
host_pattern.contains_slash = Tiparul de gazdă „{ $pattern }” nu trebuie să conțină „/”.
host_pattern.missing_suffix = Tiparul de gazdă „{ $pattern }” trebuie să conțină un sufix după „*.”.
host_pattern.empty_label = Tiparul de gazdă „{ $pattern }” conține o etichetă goală.
host_pattern.invalid_chars = Tiparul de gazdă „{ $pattern }” conține caractere nevalide.
host_pattern.invalid_label_edge = Etichetele tiparului de gazdă „{ $pattern }” nu trebuie să înceapă sau să se termine cu „-”.
host_pattern.label_too_long = Tiparul de gazdă „{ $pattern }” conține o etichetă mai lungă de 63 de caractere.
host_pattern.too_long = Tiparul de gazdă „{ $pattern }” depășește limita de 255 de caractere.

# Politica de rețea.
network_policy.scheme.empty = Schema nu trebuie să fie goală.
network_policy.scheme.invalid = Schema „{ $scheme }” conține caractere nevalide.
network_policy.allowlist.empty = Lista gazdelor permise nu trebuie să fie goală.
network_policy.scheme.not_allowed = Schema „{ $scheme }” nu este permisă.
network_policy.missing_host = Din adresa URL lipsește gazda.
network_policy.host.blocked = Gazda „{ $host }” este blocată de politică.
network_policy.host.not_allowlisted = Gazda „{ $host }” nu se află pe lista celor permise.

# Configurarea bibliotecii standard.
stdlib.config.default_fetch_cache_invalid = Calea implicită a memoriei cache fetch trebuie să fie relativă.
stdlib.config.default_which_cache_invalid = Capacitatea implicită a memoriei cache which trebuie să fie pozitivă.
stdlib.config.workspace_root_absolute = Calea rădăcină a spațiului de lucru trebuie să fie absolută.
stdlib.config.fetch_response_limit_positive = Limita răspunsului fetch trebuie să fie pozitivă.
stdlib.config.command_output_limit_positive = Limita ieșirii capturate a comenzilor trebuie să fie pozitivă.
stdlib.config.command_stream_limit_positive = Limita fluxului comenzilor trebuie să fie pozitivă.
stdlib.config.which_cache_capacity_positive = Capacitatea memoriei cache which trebuie să fie pozitivă.
stdlib.config.skip_dir_empty = Intrările pentru directoarele omise nu trebuie să fie goale.
stdlib.config.skip_dir_navigation = Intrările pentru directoarele omise nu trebuie să conțină „..”.
stdlib.config.skip_dir_separator = Intrările pentru directoarele omise nu trebuie să conțină separatori de cale.
stdlib.config.fetch_cache_empty = Calea memoriei cache fetch nu trebuie să fie goală.
stdlib.config.fetch_cache_not_relative = Calea memoriei cache fetch trebuie să fie relativă; s-a primit { $path }.
stdlib.config.fetch_cache_escapes = Calea memoriei cache fetch nu trebuie să iasă din spațiul de lucru: { $path }.
stdlib.config.open_workspace_root = Directorul curent nu a putut fi deschis ca rădăcină a spațiului de lucru stdlib.
stdlib.config.resolve_cwd = Directorul curent nu a putut fi determinat ca rădăcină a spațiului de lucru stdlib.
stdlib.config.cwd_non_utf8 = Directorul curent conține componente care nu sunt UTF-8: { $path }.

# Diagnostice ale ajutorului fetch.
stdlib.fetch.url_invalid = Adresă URL nevalidă „{ $url }”: { $details }.
stdlib.fetch.disallowed = Adresa URL „{ $url }” nu este permisă: { $details }.
stdlib.fetch.failed = Descărcarea de la „{ $url }” a eșuat: { $details }.
stdlib.fetch.cache_read_failed = Intrarea din memoria cache „{ $name }” nu a putut fi citită: { $details }.
stdlib.fetch.cache_open_failed = Intrarea din memoria cache „{ $name }” nu a putut fi deschisă: { $details }.
stdlib.fetch.response_read_failed = Răspunsul de la „{ $url }” nu a putut fi citit: { $details }.
stdlib.fetch.response_buffer_overflow = Depășirea memoriei tampon la citirea „{ $url }”.
stdlib.fetch.cache_write_failed = Memoria cache pentru „{ $url }” nu a putut fi scrisă: { $details }.
stdlib.fetch.response_limit_exceeded = Răspunsul de la „{ $url }” a depășit limita de { $limit } octeți.
stdlib.fetch.cache_limit_exceeded = Răspunsul din memoria cache „{ $name }” a depășit limita de { $limit } octeți.
stdlib.fetch.io_failed = Acțiunea „{ $action }” a eșuat pentru { $path }: { $details }.
stdlib.fetch.action.sync_cache = sincronizarea memoriei cache fetch
stdlib.fetch.action.create_cache_dir = crearea directorului memoriei cache fetch
stdlib.fetch.action.open_cache_dir = deschiderea directorului memoriei cache fetch
stdlib.fetch.action.stat_cache = citirea informațiilor despre intrarea din memoria cache fetch
stdlib.fetch.action.open_cache_entry = deschiderea intrării din memoria cache fetch

# Diagnostice ale ajutorului pentru comenzi.
stdlib.command.location = comanda „{ $command }” din șablonul „{ $template }”
stdlib.command.spawn_failed = Pornirea { $location } a eșuat: { $details }.
stdlib.command.io_failed = { $location } a eșuat: { $details }.
stdlib.command.closed_input_early = Intrarea s-a închis înainte de terminarea scrierii către comandă.
stdlib.command.broken_pipe = Conductă întreruptă la rularea { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } a fost oprit de un semnal.
stdlib.command.exited_with_status = { $location } s-a încheiat cu starea { $status }.
stdlib.command.output_limit_exceeded = { $location } a depășit limita { $mode } de { $limit } octeți pentru { $stream }.
stdlib.command.timeout = { $location } a depășit limita de timp de { $seconds } secunde.
stdlib.command.exit_status_suffix = (stare de ieșire { $status })
stdlib.command.signal_suffix = (oprit de un semnal)
stdlib.command.shell.empty = Comanda de shell nu trebuie să fie goală.
stdlib.command.grep.empty_pattern = Tiparul grep nu trebuie să fie gol.
stdlib.command.grep.flags_not_string = Fanioanele grep trebuie să fie șiruri de caractere.
stdlib.command.quote.invalid = { $arg } nu a putut fi pus între ghilimele: { $details }.
stdlib.command.quote.line_break = Argumentele care conțin retur de car sau salt de linie nu pot fi puse în siguranță între ghilimele.
stdlib.command.input_undefined = Valoarea de intrare nu este definită.
stdlib.command.tempfile.root_required = Crearea fișierelor temporare de comandă necesită rădăcina spațiului de lucru.
stdlib.command.tempfile.create_failed = Fișierul temporar al comenzii nu a putut fi creat: { $details }.
stdlib.command.options.invalid_utf8 = Cheia unei opțiuni de comandă trebuie să fie UTF-8 valid.
stdlib.command.option.mode_not_string = Modul de ieșire trebuie să fie un șir de caractere.
stdlib.command.options.invalid_type = Opțiunile comenzii trebuie să fie un obiect.
stdlib.command.output.mode_unsupported = Mod de ieșire neacceptat „{ $mode }”.
stdlib.command.output.mode.capture = captură
stdlib.command.output.mode.streaming = flux continuu
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnostice ale ajutorului pentru căi.
stdlib.path.io.failed = Acțiunea „{ $action }” a eșuat pentru { $path } ({ $label }).
stdlib.path.io.failed_with_detail = Acțiunea „{ $action }” a eșuat pentru { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = Acțiunea „{ $action }” a eșuat pentru { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = negăsit
stdlib.path.io.permission_denied = acces refuzat
stdlib.path.io.already_exists = există deja
stdlib.path.io.invalid_input = intrare nevalidă
stdlib.path.io.invalid_data = date nevalide
stdlib.path.io.timed_out = timp expirat
stdlib.path.io.interrupted = întrerupt
stdlib.path.io.would_block = ar bloca execuția
stdlib.path.io.write_zero = s-au scris zero octeți
stdlib.path.io.unexpected_eof = sfârșit de fișier neașteptat
stdlib.path.io.broken_pipe = conductă întreruptă
stdlib.path.io.connection_refused = conexiune refuzată
stdlib.path.io.connection_reset = conexiune reinițializată
stdlib.path.io.connection_aborted = conexiune abandonată
stdlib.path.io.not_connected = neconectat
stdlib.path.io.addr_in_use = adresă deja folosită
stdlib.path.io.addr_not_available = adresă indisponibilă
stdlib.path.io.out_of_memory = memorie insuficientă
stdlib.path.io.unsupported = neacceptat
stdlib.path.io.file_too_large = fișier prea mare
stdlib.path.io.resource_busy = resursă ocupată
stdlib.path.io.executable_busy = executabil ocupat
stdlib.path.io.deadlock = blocaj reciproc
stdlib.path.io.crosses_devices = traversează dispozitive
stdlib.path.io.too_many_links = prea multe legături
stdlib.path.io.invalid_filename = nume de fișier nevalid
stdlib.path.io.arg_list_too_long = listă de argumente prea lungă
stdlib.path.io.stale_handle = referință de fișier de rețea învechită
stdlib.path.io.storage_full = spațiu de stocare plin
stdlib.path.io.not_seekable = nu permite poziționarea
stdlib.path.io.network_down = rețea nefuncțională
stdlib.path.io.network_unreachable = rețea inaccesibilă
stdlib.path.io.host_unreachable = gazdă inaccesibilă
stdlib.path.io.other = eroare de intrare/ieșire
stdlib.path.action.canonicalize = canonizarea
stdlib.path.action.open_directory = deschiderea directorului
stdlib.path.action.stat = citirea informațiilor
stdlib.path.action.read = citirea
stdlib.path.action.open_file = deschiderea fișierului
stdlib.path.with_suffix.empty_separator = with_suffix necesită un separator care nu este gol.
stdlib.path.relative_to.mismatch = { $path } nu este relativ la { $root }.
stdlib.path.expanduser.unsupported = Extinderea lui ~ pentru un anumit utilizator nu este acceptată.
stdlib.path.expanduser.no_home = Nu se poate extinde ~: nu este setată nicio variabilă de mediu pentru directorul personal.
stdlib.path.contents.unsupported_encoding = Codificare neacceptată „{ $encoding }”.
stdlib.path.hash.unsupported_algorithm = Algoritm de dispersie neacceptat „{ $algorithm }”.
stdlib.path.hash.unsupported_algorithm_legacy = Algoritm de dispersie neacceptat „{ $algorithm }” (activați funcționalitatea „{ $feature }”).

# Diagnostice ale ajutoarelor pentru colecții.
stdlib.collections.flatten.expected_sequence = flatten aștepta elemente dintr-o secvență, dar a găsit { $kind }.
stdlib.collections.group_by.empty_attribute = group_by necesită un atribut care nu este gol.
stdlib.collections.group_by.unresolved = group_by nu a putut găsi „{ $attr }” pe un element de tipul { $kind }.

# Diagnostice ale ajutoarelor pentru timp.
stdlib.time.offset.invalid = Decalajul now „{ $offset }” este nevalid: se aștepta „+HH:MM[:SS]” sau „Z”.
stdlib.time.timedelta.overflow = Depășire în timedelta la adunarea componentei { $component }.
stdlib.time.label.weeks = săptămâni
stdlib.time.label.days = zile
stdlib.time.label.hours = ore
stdlib.time.label.minutes = minute
stdlib.time.label.seconds = secunde
stdlib.time.label.milliseconds = milisecunde
stdlib.time.label.microseconds = microsecunde
stdlib.time.label.nanoseconds = nanosecunde

# Diagnostice ale ajutorului which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] comanda „{ $command }” nu a fost găsită după verificarea a { $count } intrări din PATH. Previzualizare: { $preview }
stdlib.which.not_found.hint.cwd_auto = Segmentele goale din PATH sunt ignorate; folosiți cwd_mode="auto" pentru a include directorul de lucru.
stdlib.which.not_found.hint.cwd_always = Stabiliți cwd_mode="always" pentru a include directorul curent.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] comanda „{ $command }” din „{ $path }” lipsește sau nu este executabilă.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <gol>
stdlib.which.path_entry.non_utf8 = Intrarea nr. { $index } din PATH conține caractere care nu sunt UTF-8; Netsuke necesită căi UTF-8.
stdlib.which.command.empty = which necesită un șir de caractere care nu este gol.
stdlib.which.cwd_mode.invalid = cwd_mode trebuie să fie „auto”, „always” sau „never”; s-a primit „{ $mode }”.
stdlib.which.cwd.resolve_failed = Directorul curent nu a putut fi determinat: { $details }.
stdlib.which.cwd.non_utf8 = Directorul curent conține componente care nu sunt UTF-8.
stdlib.which.canonicalize_failed = „{ $path }” nu a putut fi canonizat: { $details }.
stdlib.which.is_executable = Nu s-a putut stabili dacă „{ $path }” este executabil: { $details }.
stdlib.which.canonicalize_non_utf8 = Calea canonică conține componente care nu sunt UTF-8.
stdlib.which.workspace_non_utf8 = Calea spațiului de lucru conține componente care nu sunt UTF-8 la rezolvarea comenzii „{ $command }”: { $path }.
stdlib.which.walkdir_error = Eroare la parcurgerea spațiului de lucru în timpul rezolvării comenzii: { $details }.

# Înregistrarea bibliotecii standard.
stdlib.register.open_dir = Directorul curent nu a putut fi deschis pentru înregistrarea stdlib.
stdlib.register.resolve_dir = Directorul curent nu a putut fi determinat pentru înregistrarea stdlib.
stdlib.register.dir_non_utf8 = Directorul curent conține componente care nu sunt UTF-8: { $path }.

# Raportarea stării în modul de ieșire accesibil.
status.state.pending = în așteptare
status.state.running = în desfășurare
status.state.done = finalizată
status.state.failed = eșuată
status.stage.label = Etapa { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Sarcina { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Se citește fișierul de manifest
status.stage.initial_yaml_parsing = Se analizează documentul YAML
status.stage.template_expansion = Se extind directivele șabloanelor
status.stage.final_rendering = Se deserializează și se redau valorile manifestului
status.stage.ir_generation_validation = Se construiește și se verifică graful dependențelor
status.stage.ninja_synthesis = Se sintetizează planul de construire Ninja
status.stage.ninja_synthesis_execute = Se sintetizează planul Ninja și se rulează { $tool }
status.stage.graph_rendering = Se redă artefactul grafului
status.stage.graph_rendering_with_tool = Se redă { $tool }
status.complete = { $tool }: operațiune finalizată.
status.timing.summary_header = Rezumatul timpilor pe etape:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Timpul total al fluxului: { $duration }
status.tool.build = Construire
status.tool.clean = Curățare
status.tool.graph = Graf
status.tool.graph_html = Graf (HTML)
status.tool.generate = Generare
status.tool.help_targets = Catalogul țintelor

# Textele redării grafului în HTML.
graph.html.title = Graful de construire Netsuke
graph.html.heading = Graful de construire Netsuke
graph.html.description = Graf de construire redat de Netsuke
graph.html.outline.summary = Ținte și dependențe (schemă text)
graph.html.outline.no_inputs = Fără intrări
graph.html.noscript.notice = JavaScript este dezactivat. Schema text de mai sus conține întregul graf; mai jos urmează sursa DOT.

# Prefixe semantice pentru ieșirea accesibilă.
semantic.prefix.error = Eroare:
semantic.prefix.warning = Avertisment:
semantic.prefix.success = Reușit:
semantic.prefix.info = Informație:
semantic.prefix.timing = Timp:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Exemple de forme de plural pentru traducători.
# Româna are trei categorii CLDR: `one`, `few` și `other`. `few` acoperă 0,
# 2–19 și resturile 101–119, precum și valorile zecimale, de exemplu 1,5;
# `other` acoperă restul numerelor întregi și cere prepoziția „de”.
example.files_processed = { $count ->
    [one] S-a procesat { $count } fișier.
    [few] S-au procesat { $count } fișiere.
   *[other] S-au procesat { $count } de fișiere.
}

example.errors_found = { $count ->
    [0] Nu s-a găsit nicio eroare.
    [one] S-a găsit { $count } eroare.
    [few] S-au găsit { $count } erori.
   *[other] S-au găsit { $count } de erori.
}
