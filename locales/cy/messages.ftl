# Adnoddau lleoleiddio ar gyfer llinell orchymyn Netsuke.

runner.io.dyndep.retention = Methu gymhwyso cadw dyndep a gynhyrchwyd o dan { $path }.
cli.about = Mae Netsuke yn trosi maniffestau YAML + Jinja yn gynlluniau adeiladu Ninja.
cli.long_about = Mae Netsuke yn trawsnewid maniffestau YAML + Jinja yn graffiau Ninja atgynhyrchadwy ac yn rhedeg Ninja gyda rhagosodiadau diogel.
cli.usage = { $usage }

# Testun cymorth y dewisiadau cyffredinol.
cli.flag.file.help = Llwybr y ffeil faniffest Netsuke i'w defnyddio.
cli.flag.directory.help = Rhedeg fel petai wedi cychwyn yn y cyfeiriadur hwn.
cli.flag.config.help = Llwybr ffeil ffurfweddu, gan osgoi'r chwilio awtomatig.
cli.flag.jobs.help = Gosod nifer y tasgau adeiladu cyfochrog.
cli.flag.verbose.help = Galluogi cofnodi diagnostig manwl a chrynodebau amser wrth orffen.
cli.flag.locale.help = Tag iaith ar gyfer testun y llinell orchymyn (er enghraifft: en-US, cy).
cli.flag.fetch_allow_scheme.help = Cynlluniau URL ychwanegol a ganiateir i'r cynorthwyydd fetch.
cli.flag.fetch_allow_host.help = Enwau gwesteiwyr a ganiateir pan fo'r gwrthod rhagosodedig ymlaen.
cli.flag.fetch_block_host.help = Enwau gwesteiwyr a rwystrir bob amser, hyd yn oed os caniateir hwy mewn man arall.
cli.flag.fetch_default_deny.help = Gwrthod pob gwesteiwr yn rhagosodedig; caniatáu'r rhestr a ddatganwyd yn unig.
cli.flag.trust_project_fetch_policy.help = Allow project configuration to widen fetch-policy grants.
cli.flag.json.help = Allbynnu JSON y gall pheiriant ei ddarllen.
cli.flag.no_input.help = Peidio byth â darllen mewnbwn rhyngweithiol.
cli.flag.color.help = Polisi allbwn lliw (auto, always, never).
cli.flag.emoji.help = Polisi emoji (auto, always, never).
cli.flag.progress.help = Polisi dangos cynnydd (auto, always, never).
cli.flag.accessibility.help = Polisi allbwn hygyrch (auto, on, off).
cli.flag.default_targets.help = Targedau adeiladu rhagosodedig pan na nodir yr un.

# Disgrifiadau'r is-orchmynion.
cli.subcommand.build.about = Adeiladu'r targedau a ddiffinnir yn y maniffest (rhagosodedig).
cli.subcommand.build.long_about = Adeiladu'r targedau y gofynnwyd amdanynt; os na nodir yr un, defnyddir rhagosodiadau'r maniffest.
cli.subcommand.clean.about = Tynnu arteffactau'r adeiladu drwy Ninja.
cli.subcommand.clean.long_about = Creu ffeil Ninja dros dro, yna rhedeg `ninja -t clean`.
cli.subcommand.graph.about = Allbynnu graff dibyniaethau'r adeiladu. DOT yw'r fformat rhagosodedig.
cli.subcommand.graph.long_about = Taflunio'r maniffest Netsuke a ddadansoddwyd yn graff adeiladu canonaidd a'i ysgrifennu fel Graphviz DOT, neu fel tudalen HTML hunangynhwysol gyda `--html`. Defnyddiwch `--output <FFEIL>` i ysgrifennu i ffeil; mae `-` yn ysgrifennu i'r allbwn safonol.
cli.subcommand.generate.about = Creu'r maniffest Ninja heb redeg Ninja.
cli.subcommand.generate.long_about = Ysgrifennu'r maniffest Ninja a gynhyrchwyd i'r allbwn safonol, neu i ffeil a ddewisir gyda `--output`.
cli.subcommand.help.about = Argraffwch cymorth lefel uchaf, neu cymorth ar gyfer pwnc a enwir.
cli.subcommand.help.long_about = Heb bwnc, mae hyn yn cyfateb i `--help`. Defnyddiwch `help targets` i argraffu catalog targedau a gweithredoedd ar gyfer y ffeil a ddewiswyd.

# Help catalogue headings and markers.
cli.help.actions_heading = Gweithredoedd:
cli.help.targets_heading = Targedau:
cli.help.targets.about = Rhestru targedau a gweithredoedd yn y maniffest a ddewiswyd.
cli.help.default_marker = diofyn
cli.help.conditional_marker = amodol

# Testun cymorth dewisiadau'r is-orchymyn build.
cli.subcommand.build.flag.targets.help = Y targedau i'w hadeiladu (defnyddir rhagosodiadau'r maniffest os hepgorir hwy).

# Testun cymorth dewisiadau'r is-orchymyn graph.
cli.subcommand.graph.flag.html.help = Rendro'r graff fel tudalen HTML hunangynhwysol yn lle DOT.
cli.subcommand.graph.flag.output.help = Ysgrifennu arteffact y graff i FFEIL; defnyddiwch `-` ar gyfer yr allbwn safonol.

# Testun cymorth dewisiadau'r is-orchymyn generate.
cli.subcommand.generate.flag.output.help = Ysgrifennu'r maniffest Ninja a gynhyrchwyd i FFEIL yn lle'r allbwn safonol.

# Gwallau dilysu'r llinell orchymyn.
cli.validation.jobs.invalid_number = Nid yw { $value } yn rhif dilys.
cli.validation.jobs.out_of_range = Rhaid i nifer y tasgau fod rhwng { $min } a { $max }.
cli.validation.scheme.empty = Rhaid i'r cynllun beidio â bod yn wag.
cli.validation.scheme.invalid_start = Rhaid i'r cynllun ‘{ $scheme }’ ddechrau â llythyren ASCII.
cli.validation.scheme.invalid = Cynllun annilys: ‘{ $scheme }’.
cli.validation.locale.empty = Rhaid i'r tag iaith beidio â bod yn wag.
cli.validation.locale.invalid = Tag iaith annilys: ‘{ $locale }’.
cli.validation.color.invalid = Polisi lliw annilys: ‘{ $value }’. Dewisiadau dilys: auto, always, never.
cli.validation.emoji.invalid = Polisi emoji annilys: ‘{ $value }’. Dewisiadau dilys: auto, always, never.
cli.validation.progress.invalid = Polisi cynnydd annilys: ‘{ $value }’. Dewisiadau dilys: auto, always, never.
cli.validation.accessibility.invalid = Polisi hygyrchedd annilys: ‘{ $value }’. Dewisiadau dilys: auto, on, off.
cli.validation.config.expected_object = Disgwylid i werthoedd y llinell orchymyn gyfresoli'n wrthrych, ond cafwyd { $value }.

# Negeseuon gwall Clap.
clap-error-missing-argument = Ymresymiad gofynnol ar goll: { $argument }
clap-error-missing-subcommand = Is-orchymyn ar goll. Dewisiadau sydd ar gael: { $valid_subcommands }
clap-error-unknown-argument = Ymresymiad anhysbys: { $argument }
clap-error-invalid-value = Gwerth annilys ar gyfer { $argument }: { $value }
clap-error-invalid-subcommand = Is-orchymyn anhysbys: { $subcommand }
# Sylwer: mae geiriad value-validation yn wahanol i invalid-value er mwyn
# gwahaniaethu rhwng methiannau dilyswyr pwrpasol (ErrorKind::ValueValidation)
# a gwrthdaro mathau (ErrorKind::InvalidValue).
clap-error-value-validation = Methodd y dilysu ar gyfer { $argument }: { $value }

# Gwallau a chyd-destun wrth redeg.
runner.manifest.not_found = Ni chafwyd hyd i'r maniffest ‘{ $manifest_name }’ yn { $directory }.
runner.manifest.not_found.help = Sicrhewch fod y maniffest yn bodoli, neu rhowch `--file` gyda'r llwybr cywir.
runner.manifest.path_missing_name = Nid oes enw ffeil yn llwybr y maniffest ‘{ $path }’.
cli.file.non_utf8 = Nid yw llwybr y maniffest ‘{ $path }’ yn UTF-8 dilys.
runner.manifest.directory_label = cyfeiriadur `{ $directory }`
runner.manifest.current_directory_label = y cyfeiriadur cyfredol
runner.manifest.default_not_declared = Nid yw rhagosodiad y maniffest '{ $default }' yn enwi gweithred neu darged datganedig.
runner.context.network_policy = Methwyd â llunio'r polisi rhwydwaith.
runner.context.load_manifest = Methwyd â llwytho'r maniffest o { $path }.
runner.context.serialise_manifest = Methwyd â chyfresoli'r maniffest.
runner.context.build_graph = Methwyd â llunio graff o'r maniffest.
runner.context.generate_ninja = Methwyd â chreu'r maniffest Ninja.
runner.context.render_graph = Methwyd â rendro arteffact y graff.

runner.io.create_temp_file = Methwyd â chreu'r ffeil Ninja dros dro.
runner.io.write_temp_ninja = Methwyd ag ysgrifennu i'r ffeil Ninja dros dro.
runner.io.flush_temp_ninja = Methwyd â gwagio byffer y ffeil Ninja dros dro.
runner.io.sync_temp_ninja = Methwyd â chydamseru'r ffeil Ninja dros dro.
runner.io.create_parent_dir = Methwyd â chreu'r cyfeiriadur rhiant { $path }.
runner.io.create_ninja_file = Methwyd â chreu'r ffeil Ninja yn { $path }.
runner.io.write_ninja_file = Methwyd ag ysgrifennu i'r ffeil Ninja yn { $path }.
runner.io.flush_ninja_file = Methwyd â gwagio byffer y ffeil Ninja yn { $path }.
runner.io.sync_ninja_file = Methwyd â chydamseru'r ffeil Ninja yn { $path }.
runner.io.open_ambient_dir = Methwyd ag agor y cyfeiriadur amgylchynol.
cli.directory.non_utf8 = Nid yw llwybr y cyfeiriadur gweithio yn UTF-8 dilys. ({ $path })
runner.io.no_existing_ancestor = Nid oes cyfeiriadur uwch yn bodoli ar gyfer { $path }.
runner.io.derive_relative_path = Methwyd â deillio llwybr Ninja cymharol.
runner.io.non_utf8_path = Ni chefnogir llwybrau nad ydynt yn UTF-8 (llwybr: { $path }).
runner.io.write_stdout = Methwyd ag ysgrifennu'r maniffest Ninja i'r allbwn safonol.
runner.io.flush_stdout = Methwyd â gwagio byffer yr allbwn safonol.
runner.io.dyndep.create_dir = Methu creu cyfeiriadur dyndep { $path }.
runner.io.dyndep.read = Methu darllen ffeil dyndep a gynhyrchwyd yn { $path }.
runner.io.dyndep.write = Methu ysgrifennu ffeil dyndep a gynhyrchwyd at { $path }.
runner.io.dyndep.rename = Methu cwblhau ffeil dyndep a gynhyrchwyd yn { $path }.
runner.io.dyndep.corrupt = Nid yw'r ffeil dyndep a gynhyrchwyd yn { $path } yn cyfateb i'w chynnwys disgwyliedig; tynnwch y ffeil honno yn unig a cheisiwch eto.
runner.io.dyndep.temp_collisions = Methwyd creu ffeil dyndep dros dro unigryw ar gyfer { $path } ar ôl gwrthdrawiadau enwau mynych.
runner.io.dyndep.too_large = Mae'r ffeil dyndep a gynhyrchwyd yn { $path } yn fwy na'r terfyn dilysu o { $limit } beit.

# Diagnosteg y maniffest.
manifest.parse = Methodd dadansoddiad y maniffest.
manifest.structure_error = Gwall strwythur yn y maniffest yn { $name }: { $details }
manifest.yaml.parse = Gwall dadansoddi YAML ar linell { $line }, colofn { $column }: { $details }
manifest.yaml.label = YAML annilys
manifest.yaml.hint.tabs = Nid yw YAML yn caniatáu tabiau; defnyddiwch fylchau i fewnoli.
manifest.yaml.hint.list_item = Rhaid i eitemau rhestr YAML ddechrau â ‘-’ a chael eu mewnoli'n gywir.
manifest.yaml.hint.expected_colon = Mae hyn yn edrych fel cofnod mapio; mae ‘:’ ar goll ar ôl yr allwedd.
manifest.yaml.hint.mapping_values = Mae mapiau YAML angen gwerth ar ôl ‘:’ (neu floc nythog).
manifest.yaml.hint.invalid_token = Mae'r tocyn YAML yn annilys neu'n annisgwyl.
manifest.yaml.hint.escape = Diangwch y slaesau ôl neu dynnwch y dilyniannau dianc annilys.
manifest.env.missing = Nid yw newidyn amgylchedd gofynnol wedi'i osod.
manifest.env.invalid_utf8 = Mae newidyn amgylchedd yn cynnwys UTF-8 annilys.
manifest.vars.not_object = Rhaid i `vars` y maniffest fod yn fap neu'n wrthrych.
manifest.vars.reserved_name = Mae'r allwedd `vars` '{ $name }' yn y maniffest wedi'i chadw ar gyfer cynorthwyydd templed mewnol; ailenwch y newidyn.
manifest.read_failed = Methwyd â darllen y maniffest o { $path }.
manifest.resolve_workspace_root = Methwyd â phennu gwraidd y gweithle.
manifest.workspace_non_utf8 = Nid yw llwybr gwraidd y gweithle ‘{ $path }’ yn UTF-8 dilys.
manifest.path_non_utf8 = Nid yw llwybr y maniffest ‘{ $manifest }’ yn UTF-8 dilys: { $path }.
manifest.path_missing_name = Nid oes enw ffeil yn llwybr y maniffest ‘{ $path }’.
manifest.open_workspace_failed = Methwyd ag agor y gweithle { $workspace } ar gyfer y maniffest { $manifest }.
manifest.foreach.not_iterable = Ni ellir iteru dros y mynegiad `foreach`.
manifest.foreach.serialise_item = Methwyd â chyfresoli eitem `foreach`.
manifest.when.empty = Rhaid i'r mynegiad `when` beidio â bod yn wag.
manifest.when.eval_error = Methwyd â gwerthuso'r mynegiad `when` ‘{ $expr }’.
manifest.when.template_error = Methwyd â rendro'r templed `when` ‘{ $expr }’.
manifest.target.vars_not_object = Rhaid i `vars` y targed fod yn wrthrych, ond cafwyd { $value }.
manifest.vars.entry_not_object = Rhaid i gofnod `vars` y maniffest fod yn wrthrych.
manifest.field_not_string = Rhaid i'r maes ‘{ $field }’ fod yn llinyn.
manifest.expression.parse_error = Methwyd â dadansoddi'r mynegiad { $name }.
manifest.expression.eval_error = Methwyd â gwerthuso'r mynegiad { $name }.

# Diagnosteg macros y maniffest.
manifest.macro.signature_missing_identifier = Mae dynodydd ar goll o lofnod y macro.
manifest.macro.signature_missing_params = Mae paramedrau ar goll o lofnod y macro.
manifest.macro.compile_failed = Methwyd â throsi'r macro { $name }.
manifest.macro.sequence_invalid = Rhaid diffinio macros fel map o enwau i dempledi.
manifest.macro.register_failed = Methwyd â chofrestru macros y maniffest.
manifest.macro.not_initialised = Nid yw amgylchedd y macros wedi'i baratoi.
manifest.macro.caller_invalid = Rhaid i alwr y macro fod yn llinyn.
manifest.macro.template_load_failed = Methwyd â llwytho templed y macro.
manifest.macro.init_failed = Methwyd â pharatoi amgylchedd y macros.
manifest.macro.missing = Mae'r macro { $name } ar goll.

# Gwallau patrymau glob y maniffest.
manifest.glob.unmatched_brace = Patrwm glob annilys ‘{ $pattern }’: nid oes pâr i ‘{ $character }’ yn safle { $position }.
manifest.glob.invalid_pattern = Patrwm glob annilys ‘{ $pattern }’: { $detail }.
manifest.glob.unknown_pattern_error = gwall patrwm anhysbys.
manifest.glob.io_failed = Methodd glob ar gyfer ‘{ $pattern }’: { $detail }.
manifest.glob.unknown_io_error = gwall mewnbwn/allbwn anhysbys.
manifest.command_list_empty = Rhaid i’r maes ‘command’ beidio â bod yn wag: rhowch linyn gorchymyn neu restr nad yw’n wag.

# Gwallau'r cynrychioliad canolradd.
ir.rule_not_found = Ni chafwyd hyd i'r rheol ‘{ $rule }’ y cyfeirir ati gan y targed ‘{ $target }’.
ir.multiple_rules = Rhaid i'r targed ‘{ $target }’ gyfeirio at un rheol yn unig, ond cafwyd { $rules }.
ir.empty_rule = Rhaid i'r targed ‘{ $target }’ gyfeirio at reol.
ir.duplicate_outputs = Canfuwyd allbynnau dyblyg: { $outputs }.
ir.circular_dependency = Canfuwyd dibyniaeth gylchol: { $cycle }.
ir.action_serialisation = Methwyd â chyfresoli'r weithred: { $details }.
ir.invalid_command = Mewnosodiad annilys yn y gorchymyn: { $snippet }.

# Gwallau cynhyrchu Ninja.
ninja_gen.missing_action = Mae'r weithred ‘{ $id }’ y cyfeirir ati gan ymyl adeiladu ar goll.
ninja_gen.format = Methwyd â fformatio allbwn y maniffest Ninja.
ninja_gen.dyndep_files_required = Mae'r adeilad hwn yn gofyn am fwndel Ninja a gynhyrchwyd; defnyddiwch `netsuke build`, `netsuke clean` neu `netsuke generate` er mwyn deunyddoli ffeiliau dyndep.
ninja_gen.reserved_output_path = Mae'r llwybr '{ $path }' wedi'i gadw ar gyfer cyflwr dibyniaethau cyfresol Netsuke.
ninja_gen.unsupported_path_character = Mae'r llwybr '{ $path }' yn cynnwys nod llwybr Ninja nas cynhelir, sef '{ $character }'.

# Dilysu patrymau gwesteiwyr.
host_pattern.empty = Rhaid i'r patrwm gwesteiwr beidio â bod yn wag.
host_pattern.contains_scheme = Rhaid i'r patrwm gwesteiwr ‘{ $pattern }’ beidio â chynnwys cynllun URL.
host_pattern.contains_slash = Rhaid i'r patrwm gwesteiwr ‘{ $pattern }’ beidio â chynnwys ‘/’.
host_pattern.missing_suffix = Rhaid i'r patrwm gwesteiwr ‘{ $pattern }’ gynnwys ôl-ddodiad ar ôl ‘*.’.
host_pattern.empty_label = Mae'r patrwm gwesteiwr ‘{ $pattern }’ yn cynnwys label gwag.
host_pattern.invalid_chars = Mae'r patrwm gwesteiwr ‘{ $pattern }’ yn cynnwys nodau annilys.
host_pattern.invalid_label_edge = Rhaid i labeli'r patrwm gwesteiwr ‘{ $pattern }’ beidio â dechrau na gorffen â ‘-’.
host_pattern.label_too_long = Mae'r patrwm gwesteiwr ‘{ $pattern }’ yn cynnwys label hwy na 63 nod.
host_pattern.too_long = Mae'r patrwm gwesteiwr ‘{ $pattern }’ yn fwy na'r terfyn o 255 nod.

# Polisi'r rhwydwaith.
network_policy.scheme.empty = Rhaid i'r cynllun beidio â bod yn wag.
network_policy.scheme.invalid = Mae'r cynllun ‘{ $scheme }’ yn cynnwys nodau annilys.
network_policy.allowlist.empty = Rhaid i'r rhestr gwesteiwyr a ganiateir beidio â bod yn wag.
network_policy.scheme.not_allowed = Ni chaniateir y cynllun ‘{ $scheme }’.
network_policy.missing_host = Mae gwesteiwr ar goll o'r URL.
network_policy.host.blocked = Mae'r gwesteiwr ‘{ $host }’ wedi'i rwystro gan y polisi.
network_policy.host.not_allowlisted = Nid yw'r gwesteiwr ‘{ $host }’ ar y rhestr a ganiateir.

# Ffurfweddu'r llyfrgell safonol.
stdlib.config.default_fetch_cache_invalid = Rhaid i lwybr rhagosodedig storfa fetch fod yn gymharol.
stdlib.config.default_which_cache_invalid = Rhaid i gynhwysedd rhagosodedig storfa which fod yn bositif.
stdlib.config.workspace_root_absolute = Rhaid i lwybr gwraidd y gweithle fod yn absoliwt.
stdlib.config.fetch_response_limit_positive = Rhaid i derfyn ymateb fetch fod yn bositif.
stdlib.config.command_output_limit_positive = Rhaid i derfyn dal allbwn gorchmynion fod yn bositif.
stdlib.config.command_stream_limit_positive = Rhaid i derfyn ffrwd y gorchmynion fod yn bositif.
stdlib.config.which_cache_capacity_positive = Rhaid i gynhwysedd storfa which fod yn bositif.
stdlib.config.skip_dir_empty = Rhaid i gofnodion y cyfeiriaduron a hepgorir beidio â bod yn wag.
stdlib.config.skip_dir_navigation = Rhaid i gofnodion y cyfeiriaduron a hepgorir beidio â chynnwys ‘..’.
stdlib.config.skip_dir_separator = Rhaid i gofnodion y cyfeiriaduron a hepgorir beidio â chynnwys gwahanyddion llwybr.
stdlib.config.fetch_cache_empty = Rhaid i lwybr storfa fetch beidio â bod yn wag.
stdlib.config.fetch_cache_not_relative = Rhaid i lwybr storfa fetch fod yn gymharol, ond cafwyd { $path }.
stdlib.config.fetch_cache_escapes = Rhaid i lwybr storfa fetch beidio â gadael y gweithle: { $path }.
stdlib.config.open_workspace_root = Methwyd ag agor y cyfeiriadur cyfredol fel gwraidd gweithle stdlib.
stdlib.config.resolve_cwd = Methwyd â phennu'r cyfeiriadur cyfredol fel gwraidd gweithle stdlib.
stdlib.config.cwd_non_utf8 = Mae'r cyfeiriadur cyfredol yn cynnwys rhannau nad ydynt yn UTF-8: { $path }.

# Diagnosteg y cynorthwyydd fetch.
stdlib.fetch.url_invalid = URL annilys ‘{ $url }’: { $details }.
stdlib.fetch.disallowed = Ni chaniateir yr URL ‘{ $url }’: { $details }.
stdlib.fetch.failed = Methwyd â nôl ‘{ $url }’: { $details }.
stdlib.fetch.cache_read_failed = Methwyd â darllen cofnod y storfa ‘{ $name }’: { $details }.
stdlib.fetch.cache_open_failed = Methwyd ag agor cofnod y storfa ‘{ $name }’: { $details }.
stdlib.fetch.response_read_failed = Methwyd â darllen yr ymateb o ‘{ $url }’: { $details }.
stdlib.fetch.response_buffer_overflow = Gorlifodd y byffer wrth ddarllen ‘{ $url }’.
stdlib.fetch.cache_write_failed = Methwyd ag ysgrifennu'r storfa ar gyfer ‘{ $url }’: { $details }.
stdlib.fetch.response_limit_exceeded = Aeth yr ymateb o ‘{ $url }’ dros y terfyn o { $limit } beit.
stdlib.fetch.cache_limit_exceeded = Aeth yr ymateb a storiwyd ‘{ $name }’ dros y terfyn o { $limit } beit.
stdlib.fetch.io_failed = Methodd y weithred ‘{ $action }’ ar gyfer { $path }: { $details }.
stdlib.fetch.action.sync_cache = cydamseru storfa fetch
stdlib.fetch.action.create_cache_dir = creu cyfeiriadur storfa fetch
stdlib.fetch.action.open_cache_dir = agor cyfeiriadur storfa fetch
stdlib.fetch.action.stat_cache = darllen manylion cofnod storfa fetch
stdlib.fetch.action.open_cache_entry = agor cofnod storfa fetch

# Diagnosteg y cynorthwyydd gorchmynion.
stdlib.command.location = y gorchymyn ‘{ $command }’ yn y templed ‘{ $template }’
stdlib.command.spawn_failed = Methwyd â chychwyn { $location }: { $details }.
stdlib.command.io_failed = Methodd { $location }: { $details }.
stdlib.command.closed_input_early = Caeodd y mewnbwn cyn gorffen ysgrifennu i'r gorchymyn.
stdlib.command.broken_pipe = Torrodd y bibell wrth redeg { $location }: { $details }.
stdlib.command.terminated_by_signal = Terfynwyd { $location } gan signal.
stdlib.command.exited_with_status = Gorffennodd { $location } gyda'r statws { $status }.
stdlib.command.output_limit_exceeded = Aeth { $location } dros derfyn { $mode } o { $limit } beit ar gyfer { $stream }.
stdlib.command.timeout = Aeth { $location } dros y terfyn amser o { $seconds } eiliad.
stdlib.command.exit_status_suffix = (statws gadael { $status })
stdlib.command.signal_suffix = (terfynwyd gan signal)
stdlib.command.shell.empty = Rhaid i orchymyn y gragen beidio â bod yn wag.
stdlib.command.grep.empty_pattern = Rhaid i batrwm grep beidio â bod yn wag.
stdlib.command.grep.flags_not_string = Rhaid i faneri grep fod yn llinynnau.
stdlib.command.quote.invalid = Methwyd â rhoi { $arg } mewn dyfynodau: { $details }.
stdlib.command.quote.line_break = Ni ellir rhoi ymresymiadau sy'n cynnwys dychweliad cerbyd neu doriad llinell mewn dyfynodau'n ddiogel.
stdlib.command.input_undefined = Nid yw gwerth y mewnbwn wedi'i ddiffinio.
stdlib.command.tempfile.root_required = Mae angen gwraidd y gweithle i greu ffeiliau gorchymyn dros dro.
stdlib.command.tempfile.create_failed = Methwyd â chreu ffeil dros dro'r gorchymyn: { $details }.
stdlib.command.options.invalid_utf8 = Rhaid i allwedd dewisiad gorchymyn fod yn UTF-8 dilys.
stdlib.command.option.mode_not_string = Rhaid i'r modd allbwn fod yn llinyn.
stdlib.command.options.invalid_type = Rhaid i ddewisiadau'r gorchymyn fod yn wrthrych.
stdlib.command.output.mode_unsupported = Modd allbwn nas cefnogir: ‘{ $mode }’.
stdlib.command.output.mode.capture = dal
stdlib.command.output.mode.streaming = ffrydio
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnosteg y cynorthwyydd llwybrau.
stdlib.path.io.failed = Methodd y weithred ‘{ $action }’ ar gyfer { $path } ({ $label }).
stdlib.path.io.failed_with_detail = Methodd y weithred ‘{ $action }’ ar gyfer { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = Methodd y weithred ‘{ $action }’ ar gyfer { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = heb ei ganfod
stdlib.path.io.permission_denied = caniatâd wedi'i wrthod
stdlib.path.io.already_exists = yn bodoli eisoes
stdlib.path.io.invalid_input = mewnbwn annilys
stdlib.path.io.invalid_data = data annilys
stdlib.path.io.timed_out = amser wedi dod i ben
stdlib.path.io.interrupted = torrwyd ar draws
stdlib.path.io.would_block = byddai'n rhwystro
stdlib.path.io.write_zero = ysgrifennwyd dim beit
stdlib.path.io.unexpected_eof = diwedd ffeil annisgwyl
stdlib.path.io.broken_pipe = pibell wedi torri
stdlib.path.io.connection_refused = gwrthodwyd y cysylltiad
stdlib.path.io.connection_reset = ailosodwyd y cysylltiad
stdlib.path.io.connection_aborted = terfynwyd y cysylltiad
stdlib.path.io.not_connected = heb gysylltu
stdlib.path.io.addr_in_use = cyfeiriad ar waith eisoes
stdlib.path.io.addr_not_available = cyfeiriad ddim ar gael
stdlib.path.io.out_of_memory = cof wedi dod i ben
stdlib.path.io.unsupported = nas cefnogir
stdlib.path.io.file_too_large = ffeil yn rhy fawr
stdlib.path.io.resource_busy = adnodd yn brysur
stdlib.path.io.executable_busy = ffeil weithredadwy yn brysur
stdlib.path.io.deadlock = cloi marw
stdlib.path.io.crosses_devices = yn croesi dyfeisiau
stdlib.path.io.too_many_links = gormod o gysylltiadau
stdlib.path.io.invalid_filename = enw ffeil annilys
stdlib.path.io.arg_list_too_long = rhestr ymresymiadau'n rhy hir
stdlib.path.io.stale_handle = dolen ffeil rwydwaith hen
stdlib.path.io.storage_full = storfa'n llawn
stdlib.path.io.not_seekable = methu gosod safle
stdlib.path.io.network_down = rhwydwaith i lawr
stdlib.path.io.network_unreachable = methu cyrraedd y rhwydwaith
stdlib.path.io.host_unreachable = methu cyrraedd y gwesteiwr
stdlib.path.io.other = gwall mewnbwn/allbwn
stdlib.path.action.canonicalize = canoneiddio
stdlib.path.action.open_directory = agor cyfeiriadur
stdlib.path.action.stat = darllen manylion
stdlib.path.action.read = darllen
stdlib.path.action.open_file = agor ffeil
stdlib.path.with_suffix.empty_separator = Mae with_suffix angen gwahanydd nad yw'n wag.
stdlib.path.relative_to.mismatch = Nid yw { $path } yn gymharol i { $root }.
stdlib.path.expanduser.unsupported = Ni chefnogir ehangu ~ ar gyfer defnyddiwr penodol.
stdlib.path.expanduser.no_home = Ni ellir ehangu ~: nid oes newidyn amgylchedd cyfeiriadur cartref wedi'i osod.
stdlib.path.contents.unsupported_encoding = Amgodiad nas cefnogir: ‘{ $encoding }’.
stdlib.path.hash.unsupported_algorithm = Algorithm stwnsio nas cefnogir: ‘{ $algorithm }’.
stdlib.path.hash.unsupported_algorithm_legacy = Algorithm stwnsio nas cefnogir: ‘{ $algorithm }’ (galluogwch y nodwedd ‘{ $feature }’).

# Diagnosteg cynorthwywyr y casgliadau.
stdlib.collections.flatten.expected_sequence = Roedd flatten yn disgwyl eitemau dilyniant ond cafodd { $kind }.
stdlib.collections.group_by.empty_attribute = Mae group_by angen priodoledd nad yw'n wag.
stdlib.collections.group_by.unresolved = Methodd group_by â chanfod ‘{ $attr }’ ar eitem o'r math { $kind }.

# Diagnosteg cynorthwywyr amser.
stdlib.time.offset.invalid = Mae gwrthbwyso now ‘{ $offset }’ yn annilys: disgwylid ‘+HH:MM[:SS]’ neu ‘Z’.
stdlib.time.timedelta.overflow = Gorlifodd timedelta wrth ychwanegu { $component }.
stdlib.time.label.weeks = wythnosau
stdlib.time.label.days = dyddiau
stdlib.time.label.hours = oriau
stdlib.time.label.minutes = munudau
stdlib.time.label.seconds = eiliadau
stdlib.time.label.milliseconds = milieiliadau
stdlib.time.label.microseconds = microeiliadau
stdlib.time.label.nanoseconds = nanoeiliadau

# Diagnosteg y cynorthwyydd which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] ni chafwyd hyd i'r gorchymyn ‘{ $command }’ ar ôl gwirio { $count } cofnod PATH. Rhagolwg: { $preview }
stdlib.which.not_found.hint.cwd_auto = Anwybyddir segmentau gwag PATH; defnyddiwch cwd_mode="auto" i gynnwys y cyfeiriadur gwaith.
stdlib.which.not_found.hint.cwd_always = Gosodwch cwd_mode="always" i gynnwys y cyfeiriadur cyfredol.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] mae'r gorchymyn ‘{ $command }’ yn ‘{ $path }’ ar goll neu nid yw'n weithredadwy.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <gwag>
stdlib.which.path_entry.non_utf8 = Mae cofnod PATH rhif { $index } yn cynnwys nodau nad ydynt yn UTF-8; mae Netsuke angen llwybrau UTF-8.
stdlib.which.command.empty = Mae which angen llinyn nad yw'n wag.
stdlib.which.cwd_mode.invalid = Rhaid i cwd_mode fod yn ‘auto’, ‘always’ neu ‘never’, ond cafwyd ‘{ $mode }’.
stdlib.which.cwd.resolve_failed = Methwyd â phennu'r cyfeiriadur cyfredol: { $details }.
stdlib.which.cwd.non_utf8 = Mae'r cyfeiriadur cyfredol yn cynnwys rhannau nad ydynt yn UTF-8.
stdlib.which.canonicalize_failed = Methwyd â chanoneiddio ‘{ $path }’: { $details }.
stdlib.which.is_executable = Methwyd â gwirio a yw ‘{ $path }’ yn weithredadwy: { $details }.
stdlib.which.canonicalize_non_utf8 = Mae'r llwybr canonaidd yn cynnwys rhannau nad ydynt yn UTF-8.
stdlib.which.workspace_non_utf8 = Mae llwybr y gweithle'n cynnwys rhannau nad ydynt yn UTF-8 wrth ddatrys y gorchymyn ‘{ $command }’: { $path }.
stdlib.which.walkdir_error = Gwall wrth dramwyo'r gweithle wrth ddatrys y gorchymyn: { $details }.

# Cofrestru'r llyfrgell safonol.
stdlib.register.open_dir = Methwyd ag agor y cyfeiriadur cyfredol ar gyfer cofrestru stdlib.
stdlib.register.resolve_dir = Methwyd â phennu'r cyfeiriadur cyfredol ar gyfer cofrestru stdlib.
stdlib.register.dir_non_utf8 = Mae'r cyfeiriadur cyfredol yn cynnwys rhannau nad ydynt yn UTF-8: { $path }.

# Adrodd statws ar gyfer y modd allbwn hygyrch.
status.state.pending = yn aros
status.state.running = ar y gweill
status.state.done = wedi'i gwblhau
status.state.failed = wedi methu
status.stage.label = Cam { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Tasg { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Yn darllen ffeil y maniffest
status.stage.initial_yaml_parsing = Yn dadansoddi'r ddogfen YAML
status.stage.template_expansion = Yn ehangu cyfarwyddiadau'r templedi
status.stage.final_rendering = Yn dadgyfresoli ac yn rendro gwerthoedd y maniffest
status.stage.ir_generation_validation = Yn llunio ac yn dilysu'r graff dibyniaethau
status.stage.ninja_synthesis = Yn saernïo cynllun adeiladu Ninja
status.stage.ninja_synthesis_execute = Yn saernïo cynllun Ninja ac yn rhedeg { $tool }
status.stage.graph_rendering = Yn rendro arteffact y graff
status.stage.graph_rendering_with_tool = Yn rendro { $tool }
status.complete = Cwblhawyd { $tool }.
status.timing.summary_header = Crynodeb amser fesul cam:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Cyfanswm amser y llinell brosesu: { $duration }
status.tool.build = Adeiladu
status.tool.clean = Glanhau
status.tool.graph = Graff
status.tool.graph_html = Graff (HTML)
status.tool.generate = Cynhyrchu
status.tool.help_targets = Cymorth targedau

# Testunau rendrwr HTML y graff.
graph.html.title = Graff adeiladu Netsuke
graph.html.heading = Graff adeiladu Netsuke
graph.html.description = Graff adeiladu a rendrwyd gan Netsuke
graph.html.outline.summary = Targedau a dibyniaethau (amlinelliad testun)
graph.html.outline.no_inputs = Dim mewnbynnau
graph.html.noscript.notice = Mae JavaScript wedi'i analluogi. Yr amlinelliad testun uchod yw'r graff cyfan; daw ffynhonnell DOT ar ei ôl.

# Rhagddodiaid semantig ar gyfer yr allbwn hygyrch.
semantic.prefix.error = Gwall:
semantic.prefix.warning = Rhybudd:
semantic.prefix.success = Llwyddiant:
semantic.prefix.info = Gwybodaeth:
semantic.prefix.timing = Amser:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Enghreifftiau o ffurfiau lluosog i gyfieithwyr.
# Mae'r Gymraeg yn defnyddio pob un o chwe chategori CLDR: `zero`, `one`,
# `two`, `few` (3), `many` (6) ac `other`, ac mae'r treiglad yn newid rhyngddynt.
example.files_processed = { $count ->
    [zero] Ni phroseswyd { $count } ffeil.
    [one] Proseswyd { $count } ffeil.
    [two] Proseswyd { $count } ffeil.
    [few] Proseswyd { $count } ffeil.
    [many] Proseswyd { $count } ffeil.
   *[other] Proseswyd { $count } ffeil.
}

example.errors_found = { $count ->
    [0] Ni chafwyd hyd i unrhyw wallau.
    [one] Cafwyd hyd i { $count } gwall.
    [two] Cafwyd hyd i { $count } wall.
    [few] Cafwyd hyd i { $count } gwall.
    [many] Cafwyd hyd i { $count } gwall.
   *[other] Cafwyd hyd i { $count } gwall.
}
