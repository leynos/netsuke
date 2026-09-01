# Goireasan sgeadachaidh airson loidhne-àithne Netsuke.

runner.io.dyndep.retention = Dh’fhàillig cur an gnìomh gleidheadh an dyndep a chaidh a chruthachadh fo { $path }.
cli.about = Bidh Netsuke a' cur ri chèile foirm-liostaichean YAML + Jinja gu planaichean togail Ninja.
cli.long_about = Bidh Netsuke ag atharrachadh fhoirm-liostaichean YAML + Jinja gu graf Ninja a ghabhas ath-dhèanamh, agus a' ruith Ninja le bun-roghainnean sàbhailte.
cli.usage = { $usage }

# Teacsa taice nan roghainnean coitcheann.
cli.flag.file.help = An t-slighe gu faidhle foirm-liosta Netsuke ri chleachdadh.
cli.flag.directory.help = Ruith mar gun deach tòiseachadh sa phasgan seo.
cli.flag.config.help = Slighe gu faidhle rèiteachaidh, a' seachnadh an luirg fhèin-obrachail.
cli.flag.jobs.help = Suidhich an àireamh de dh'obraichean togail co-shìnte.
cli.flag.verbose.help = Cuir an comas logadh mionaideach agus geàrr-chunntasan ùine aig deireadh na h-obrach.
cli.flag.locale.help = Taga cànain airson teacsa na loidhne-àithne (mar eisimpleir: en-US, gd).
cli.flag.fetch_allow_scheme.help = Sgeamaichean URL a bharrachd a tha ceadaichte don chuidiche fetch.
cli.flag.fetch_allow_host.help = Ainmean òstairean a tha ceadaichte nuair a tha an diùltadh bunaiteach an gnìomh.
cli.flag.fetch_block_host.help = Ainmean òstairean a thèid a bhacadh an-còmhnaidh, ged a bhiodh iad ceadaichte an àite eile.
cli.flag.fetch_default_deny.help = Diùlt a h-uile òstair mar bhun-roghainn; na ceadaich ach an liosta a chaidh ainmeachadh.
cli.flag.json.help = Cuir a-mach JSON a leughas inneal.
cli.flag.no_input.help = Na leugh cur-a-steach eadar-ghnìomhach idir.
cli.flag.color.help = Poileasaidh an às-chuir dhathte (auto, always, never).
cli.flag.emoji.help = Poileasaidh nan emoji (auto, always, never).
cli.flag.progress.help = Poileasaidh sealltainn an adhartais (auto, always, never).
cli.flag.accessibility.help = Poileasaidh an às-chuir so-ruigsinneach (auto, on, off).
cli.flag.default_targets.help = Targaidean togail bunaiteach nuair nach eil gin air an sònrachadh.

# Tuairisgeulan nan fo-àitheantan.
cli.subcommand.build.about = Tog na targaidean a tha air am mìneachadh san fhoirm-liosta (bun-roghainn).
cli.subcommand.build.long_about = Tog na targaidean a chaidh iarraidh; mura h-eil gin ann, cleachd targaidean bunaiteach an fhoirm-liosta.
cli.subcommand.clean.about = Thoir air falbh toraidhean an togail tro Ninja.
cli.subcommand.clean.long_about = Dèan faidhle Ninja sealach, agus an uair sin ruith `ninja -t clean`.
cli.subcommand.graph.about = Cuir a-mach graf eisimeileachd an togail. Is e DOT am fòrmat bunaiteach.
cli.subcommand.graph.long_about = Tilg am foirm-liosta Netsuke a chaidh a pharsadh gu graf togail bun-riaghailteach agus sgrìobh e mar Graphviz DOT, no mar dhuilleag HTML fhèin-chuimseach le `--html`. Cleachd `--output <FAIDHLE>` gus sgrìobhadh gu faidhle; sgrìobhaidh `-` don às-chur àbhaisteach.
cli.subcommand.generate.about = Dèan am foirm-liosta Ninja gun a bhith a' ruith Ninja.
cli.subcommand.generate.long_about = Sgrìobh am foirm-liosta Ninja a chaidh a dhèanamh don às-chur àbhaisteach, no gu faidhle a thaghar le `--output`.
cli.subcommand.help.about = Clò-bhuail an cuideachadh aig an ìre as àirde, no an cuideachadh airson cuspair ainmichte.
cli.subcommand.help.long_about = Às aonais cuspair, tha seo a' freagairt ri `--help`. Cleachd `help targets` gus catalog nan targaidean agus nan gnìomhan airson an fhaidhle a thaghadh a chlò-bhualadh.

# Help catalogue headings and markers.
cli.help.actions_heading = Gnìomhan:
cli.help.targets_heading = Targaidean:
cli.help.targets.about = Dèan liosta de na targaidean agus na gnìomhan anns an fhoirm-liosta a chaidh a thaghadh.
cli.help.default_marker = bunaiteach
cli.help.conditional_marker = cumhach

# Teacsa taice roghainnean an fho-àithne build.
cli.subcommand.build.flag.targets.help = Na targaidean ri thogail (thèid bun-roghainnean an fhoirm-liosta a chleachdadh mura h-eil gin ann).

# Teacsa taice roghainnean an fho-àithne graph.
cli.subcommand.graph.flag.html.help = Reandaraich an graf mar dhuilleag HTML fhèin-chuimseach seach mar DOT.
cli.subcommand.graph.flag.output.help = Sgrìobh toradh a' ghraf gu FAIDHLE; cleachd `-` airson an às-chuir àbhaistich.

# Teacsa taice roghainnean an fho-àithne generate.
cli.subcommand.generate.flag.output.help = Sgrìobh am foirm-liosta Ninja a chaidh a dhèanamh gu FAIDHLE seach don às-chur àbhaisteach.

# Mearachdan dearbhaidh na loidhne-àithne.
cli.validation.jobs.invalid_number = Chan e àireamh dhligheach a th' ann an { $value }.
cli.validation.jobs.out_of_range = Feumaidh àireamh nan obraichean a bhith eadar { $min } agus { $max }.
cli.validation.scheme.empty = Chan fhaod an sgeama a bhith falamh.
cli.validation.scheme.invalid_start = Feumaidh an sgeama “{ $scheme }” tòiseachadh le litir ASCII.
cli.validation.scheme.invalid = Sgeama mì-dhligheach: “{ $scheme }”.
cli.validation.locale.empty = Chan fhaod an taga cànain a bhith falamh.
cli.validation.locale.invalid = Taga cànain mì-dhligheach: “{ $locale }”.
cli.validation.color.invalid = Poileasaidh dhathan mì-dhligheach: “{ $value }”. Roghainnean dligheach: auto, always, never.
cli.validation.emoji.invalid = Poileasaidh emoji mì-dhligheach: “{ $value }”. Roghainnean dligheach: auto, always, never.
cli.validation.progress.invalid = Poileasaidh adhartais mì-dhligheach: “{ $value }”. Roghainnean dligheach: auto, always, never.
cli.validation.accessibility.invalid = Poileasaidh so-ruigsinneachd mì-dhligheach: “{ $value }”. Roghainnean dligheach: auto, on, off.
cli.validation.config.expected_object = Bha dùil gun deigheadh luachan na loidhne-àithne a shreathachadh gu oibseact, ach fhuaireadh { $value }.

# Teachdaireachdan mearachd Clap.
clap-error-missing-argument = Tha argamaid riatanach a dhìth: { $argument }
clap-error-missing-subcommand = Tha fo-àithne a dhìth. Roghainnean rim faighinn: { $valid_subcommands }
clap-error-unknown-argument = Argamaid neo-aithnichte: { $argument }
clap-error-invalid-value = Luach mì-dhligheach airson { $argument }: { $value }
clap-error-invalid-subcommand = Fo-àithne neo-aithnichte: { $subcommand }
# Nòta: tha faclan value-validation eadar-dhealaichte o invalid-value gus
# fàiligidhean dhearbhairean gnàthaichte (ErrorKind::ValueValidation) a
# sgaradh o mhì-fhreagarrachd sheòrsan (ErrorKind::InvalidValue).
clap-error-value-validation = Dh'fhàillig an dearbhadh airson { $argument }: { $value }

# Mearachdan agus co-theacsa aig àm ruith.
runner.manifest.not_found = Cha deach am foirm-liosta “{ $manifest_name }” a lorg ann an { $directory }.
runner.manifest.not_found.help = Dèan cinnteach gu bheil am foirm-liosta ann, no thoir seachad `--file` leis an t-slighe cheart.
runner.manifest.path_missing_name = Chan eil ainm faidhle ann an slighe an fhoirm-liosta “{ $path }”.
cli.file.non_utf8 = Chan eil slighe an fhoirm-liosta “{ $path }” na UTF-8 dhligheach.
runner.manifest.directory_label = pasgan `{ $directory }`
runner.manifest.current_directory_label = am pasgan làithreach
runner.manifest.default_not_declared = Chan eil bun-roghainn a’ mhanifest '{ $default }' ag ainmeachadh gnìomh no targaid dhearbhaichte.
runner.context.network_policy = Cha b' urrainnear poileasaidh an lìonraidh a thogail.
runner.context.load_manifest = Cha b' urrainnear am foirm-liosta a luchdachadh o { $path }.
runner.context.serialise_manifest = Cha b' urrainnear am foirm-liosta a shreathachadh.
runner.context.build_graph = Cha b' urrainnear graf a thogail on fhoirm-liosta.
runner.context.generate_ninja = Cha b' urrainnear am foirm-liosta Ninja a dhèanamh.
runner.context.render_graph = Cha b' urrainnear toradh a' ghraf a reandarachadh.

runner.io.create_temp_file = Cha b' urrainnear am faidhle Ninja sealach a chruthachadh.
runner.io.write_temp_ninja = Cha b' urrainnear sgrìobhadh don fhaidhle Ninja shealach.
runner.io.flush_temp_ninja = Cha b' urrainnear bufair an fhaidhle Ninja shealaich fhalmhachadh.
runner.io.sync_temp_ninja = Cha b' urrainnear am faidhle Ninja sealach a cho-thìmeachadh.
runner.io.create_parent_dir = Cha b' urrainnear am pasgan pàrant { $path } a chruthachadh.
runner.io.create_ninja_file = Cha b' urrainnear faidhle Ninja a chruthachadh aig { $path }.
runner.io.write_ninja_file = Cha b' urrainnear sgrìobhadh don fhaidhle Ninja aig { $path }.
runner.io.flush_ninja_file = Cha b' urrainnear bufair an fhaidhle Ninja aig { $path } fhalmhachadh.
runner.io.sync_ninja_file = Cha b' urrainnear am faidhle Ninja aig { $path } a cho-thìmeachadh.
runner.io.open_ambient_dir = Cha b' urrainnear am pasgan mun cuairt fhosgladh.
cli.directory.non_utf8 = Chan eil slighe a’ phasgain obrach na UTF-8 dhligheach. ({ $path })
runner.io.no_existing_ancestor = Chan eil pasgan sinnsireil ann airson { $path }.
runner.io.derive_relative_path = Cha b' urrainnear slighe Ninja choimeasach a thoirt a-mach.
runner.io.non_utf8_path = Chan eil taic ann do shlighean nach eil nan UTF-8 (slighe: { $path }).
runner.io.write_stdout = Cha b' urrainnear am foirm-liosta Ninja a sgrìobhadh don às-chur àbhaisteach.
runner.io.flush_stdout = Cha b' urrainnear bufair an às-chuir àbhaistich fhalmhachadh.
runner.io.dyndep.create_dir = Dh’fhàillig cruthachadh an eòlaire dyndep { $path }.
runner.io.dyndep.read = Dh’fhàillig leughadh an fhaidhle dyndep a chaidh a chruthachadh aig { $path }.
runner.io.dyndep.write = Dh’fhàillig sgrìobhadh an fhaidhle dyndep a chaidh a chruthachadh aig { $path }.
runner.io.dyndep.rename = Dh’fhàillig crìochnachadh an fhaidhle dyndep a chaidh a chruthachadh aig { $path }.
runner.io.dyndep.corrupt = Chan eil an fhaidhle dyndep a chaidh a chruthachadh aig { $path } a’ freagairt ris an t-susbaint ris an robh dùil; thoir air falbh an fhaidhle sin a-mhàin agus feuch ris a-rithist.
runner.io.dyndep.temp_collisions = Dh’fhàillig cruthachadh faidhle dyndep sealach àraidh airson { $path } às dèidh còmhstrithean ainmeachaidh tric.
runner.io.dyndep.too_large = Tha am faidhle dyndep a chaidh a chruthachadh aig { $path } nas motha na crìoch dearbhaidh { $limit } baidht.

# Breithneachadh an fhoirm-liosta.
manifest.parse = Dh'fhàillig parsadh an fhoirm-liosta.
manifest.structure_error = Mearachd structair san fhoirm-liosta aig { $name }: { $details }
manifest.yaml.parse = Mearachd parsaidh YAML air loidhne { $line }, colbh { $column }: { $details }
manifest.yaml.label = YAML mì-dhligheach
manifest.yaml.hint.tabs = Chan eil YAML a' ceadachadh thabaichean; cleachd beàrnan airson eag-thabaidh.
manifest.yaml.hint.list_item = Feumaidh nithean liosta YAML tòiseachadh le “-” agus a bhith air an eagachadh mar bu chòir.
manifest.yaml.hint.expected_colon = Tha coltas mapaidh air seo; tha “:” a dhìth às dèidh na h-iuchrach.
manifest.yaml.hint.mapping_values = Tha mapaidhean YAML ag iarraidh luach às dèidh “:” (no bloca neadaichte).
manifest.yaml.hint.invalid_token = Tha an t-samhla YAML mì-dhligheach no gun dùil ris.
manifest.yaml.hint.escape = Teich na slaisean-cùil no thoir air falbh na sreathan teichidh mì-dhligheach.
manifest.env.missing = Chan eil caochladair àrainneachd riatanach air a shuidheachadh.
manifest.env.invalid_utf8 = Tha UTF-8 mì-dhligheach ann an caochladair àrainneachd.
manifest.vars.not_object = Feumaidh `vars` an fhoirm-liosta a bhith na mhapadh no na oibseact.
manifest.vars.reserved_name = Tha an iuchair `vars` '{ $name }' sa mhanifest glèidhte do chuidiche teamplaid na broinn; thoir ainm ùr air a' chaochladair.
manifest.read_failed = Cha b' urrainnear am foirm-liosta a leughadh o { $path }.
manifest.resolve_workspace_root = Cha b' urrainnear freumh an raoin-obrach a dhearbhadh.
manifest.workspace_non_utf8 = Chan eil slighe freumh an raoin-obrach “{ $path }” na UTF-8 dhligheach.
manifest.path_non_utf8 = Chan eil slighe an fhoirm-liosta “{ $manifest }” na UTF-8 dhligheach: { $path }.
manifest.path_missing_name = Chan eil ainm faidhle ann an slighe an fhoirm-liosta “{ $path }”.
manifest.open_workspace_failed = Cha b' urrainnear an raon-obrach { $workspace } fhosgladh airson an fhoirm-liosta { $manifest }.
manifest.foreach.not_iterable = Chan urrainnear cuairteachadh thairis air an eas-preisean `foreach`.
manifest.foreach.serialise_item = Cha b' urrainnear nì `foreach` a shreathachadh.
manifest.when.empty = Chan fhaod an eas-preisean `when` a bhith falamh.
manifest.when.eval_error = Cha b' urrainnear an eas-preisean `when` “{ $expr }” a mheasadh.
manifest.when.template_error = Cha b' urrainnear an teamplaid `when` “{ $expr }” a reandarachadh.
manifest.target.vars_not_object = Feumaidh `vars` na targaid a bhith na oibseact, ach fhuaireadh { $value }.
manifest.vars.entry_not_object = Feumaidh innteart `vars` an fhoirm-liosta a bhith na oibseact.
manifest.field_not_string = Feumaidh an raon “{ $field }” a bhith na shreang.
manifest.expression.parse_error = Cha b' urrainnear an eas-preisean { $name } a pharsadh.
manifest.expression.eval_error = Cha b' urrainnear an eas-preisean { $name } a mheasadh.

# Breithneachadh macros an fhoirm-liosta.
manifest.macro.signature_missing_identifier = Tha aithnichear a dhìth o shoidhneadh a' mhacro.
manifest.macro.signature_missing_params = Tha paramadairean a dhìth o shoidhneadh a' mhacro.
manifest.macro.compile_failed = Cha b' urrainnear am macro { $name } a chur ri chèile.
manifest.macro.sequence_invalid = Feumar macros a mhìneachadh mar mhapadh o ainmean gu teamplaidean.
manifest.macro.register_failed = Cha b' urrainnear macros an fhoirm-liosta a chlàradh.
manifest.macro.not_initialised = Chan eil àrainneachd nam macros air a tòiseachadh.
manifest.macro.caller_invalid = Feumaidh gairmear a' mhacro a bhith na shreang.
manifest.macro.template_load_failed = Cha b' urrainnear teamplaid a' mhacro a luchdachadh.
manifest.macro.init_failed = Cha b' urrainnear àrainneachd nam macros a thòiseachadh.
manifest.macro.missing = Tha am macro { $name } a dhìth.

# Mearachdan phàtranan glob san fhoirm-liosta.
manifest.glob.unmatched_brace = Pàtran glob mì-dhligheach “{ $pattern }”: chan eil paidhir aig “{ $character }” aig ionad { $position }.
manifest.glob.invalid_pattern = Pàtran glob mì-dhligheach “{ $pattern }”: { $detail }.
manifest.glob.unknown_pattern_error = mearachd phàtrain neo-aithnichte.
manifest.glob.io_failed = Dh'fhàillig glob airson “{ $pattern }”: { $detail }.
manifest.glob.unknown_io_error = mearachd ion-chuir/às-chuir neo-aithnichte.
manifest.command_list_empty = Chan fhaod an raon “command” a bhith falamh: thoir seachad sreang àithne no liosta nach eil falamh.

# Mearachdan an riochdachaidh mheadhanaich.
ir.rule_not_found = Cha deach an riaghailt “{ $rule }” air a bheil an targaid “{ $target }” a' toirt iomradh a lorg.
ir.multiple_rules = Feumaidh an targaid “{ $target }” iomradh a thoirt air aon riaghailt a-mhàin, ach fhuaireadh { $rules }.
ir.empty_rule = Feumaidh an targaid “{ $target }” iomradh a thoirt air riaghailt.
ir.duplicate_outputs = Chaidh às-chuir dhùblaichte a lorg: { $outputs }.
ir.circular_dependency = Chaidh eisimeileachd chuairteach a lorg: { $cycle }.
ir.action_serialisation = Cha b' urrainnear an gnìomh a shreathachadh: { $details }.
ir.invalid_command = Cur a-steach mì-dhligheach san àithne: { $snippet }.

# Mearachdan dèanamh Ninja.
ninja_gen.missing_action = Tha an gnìomh “{ $id }” air a bheil oir togail a' toirt iomradh a dhìth.
ninja_gen.format = Cha b' urrainnear às-chur an fhoirm-liosta Ninja fhòrmatadh.
ninja_gen.dyndep_files_required = Feumaidh an togail seo pasgan Ninja a chaidh a chruthachadh; cleachd `netsuke build`, `netsuke clean` no `netsuke generate` gus na faidhlichean dyndep a chruthachadh.
ninja_gen.reserved_output_path = Tha an t-slighe '{ $path }' glèidhte airson staid eisimeileachdan shreathach Netsuke.
ninja_gen.unsupported_path_character = Tha an t-slighe '{ $path }' a’ gabhail a-steach caractar slighe Ninja nach eil a’ faighinn taic, '{ $character }'.

# Dearbhadh phàtranan òstair.
host_pattern.empty = Chan fhaod pàtran an òstair a bhith falamh.
host_pattern.contains_scheme = Chan fhaod pàtran an òstair “{ $pattern }” sgeama URL a ghabhail a-steach.
host_pattern.contains_slash = Chan fhaod pàtran an òstair “{ $pattern }” “/” a ghabhail a-steach.
host_pattern.missing_suffix = Feumaidh pàtran an òstair “{ $pattern }” iar-leasachan a bhith aige às dèidh “*.”.
host_pattern.empty_label = Tha leubail fhalamh ann am pàtran an òstair “{ $pattern }”.
host_pattern.invalid_chars = Tha caractaran mì-dhligheach ann am pàtran an òstair “{ $pattern }”.
host_pattern.invalid_label_edge = Chan fhaod leubailean pàtran an òstair “{ $pattern }” tòiseachadh no crìochnachadh le “-”.
host_pattern.label_too_long = Tha leubail nas fhaide na 63 caractaran ann am pàtran an òstair “{ $pattern }”.
host_pattern.too_long = Tha pàtran an òstair “{ $pattern }” nas fhaide na crìoch nan 255 caractaran.

# Poileasaidh an lìonraidh.
network_policy.scheme.empty = Chan fhaod an sgeama a bhith falamh.
network_policy.scheme.invalid = Tha caractaran mì-dhligheach san sgeama “{ $scheme }”.
network_policy.allowlist.empty = Chan fhaod liosta nan òstairean ceadaichte a bhith falamh.
network_policy.scheme.not_allowed = Chan eil an sgeama “{ $scheme }” ceadaichte.
network_policy.missing_host = Tha òstair a dhìth on URL.
network_policy.host.blocked = Tha an t-òstair “{ $host }” air a bhacadh leis a' phoileasaidh.
network_policy.host.not_allowlisted = Chan eil an t-òstair “{ $host }” air liosta nan ceadaichte.

# Rèiteachadh an leabharlainn àbhaistich.
stdlib.config.default_fetch_cache_invalid = Feumaidh slighe bhunaiteach tasgadan fetch a bhith coimeasach.
stdlib.config.default_which_cache_invalid = Feumaidh tomhas bunaiteach tasgadan which a bhith dearbh.
stdlib.config.workspace_root_absolute = Feumaidh slighe freumh an raoin-obrach a bhith absaloideach.
stdlib.config.fetch_response_limit_positive = Feumaidh crìoch freagairt fetch a bhith dearbh.
stdlib.config.command_output_limit_positive = Feumaidh crìoch glacadh às-chur nan àitheantan a bhith dearbh.
stdlib.config.command_stream_limit_positive = Feumaidh crìoch sruth nan àitheantan a bhith dearbh.
stdlib.config.which_cache_capacity_positive = Feumaidh tomhas tasgadan which a bhith dearbh.
stdlib.config.skip_dir_empty = Chan fhaod innteartan nam pasganan a thèid a leigeil seachad a bhith falamh.
stdlib.config.skip_dir_navigation = Chan fhaod “..” a bhith ann an innteartan nam pasganan a thèid a leigeil seachad.
stdlib.config.skip_dir_separator = Chan fhaod sgaradairean slighe a bhith ann an innteartan nam pasganan a thèid a leigeil seachad.
stdlib.config.fetch_cache_empty = Chan fhaod slighe tasgadan fetch a bhith falamh.
stdlib.config.fetch_cache_not_relative = Feumaidh slighe tasgadan fetch a bhith coimeasach, ach fhuaireadh { $path }.
stdlib.config.fetch_cache_escapes = Chan fhaod slighe tasgadan fetch a dhol a-mach às an raon-obrach: { $path }.
stdlib.config.open_workspace_root = Cha b' urrainnear am pasgan làithreach fhosgladh mar fhreumh raon-obrach stdlib.
stdlib.config.resolve_cwd = Cha b' urrainnear am pasgan làithreach a dhearbhadh mar fhreumh raon-obrach stdlib.
stdlib.config.cwd_non_utf8 = Tha pàirtean anns a' phasgan làithreach nach eil nan UTF-8: { $path }.

# Breithneachadh a' chuidiche fetch.
stdlib.fetch.url_invalid = URL mì-dhligheach “{ $url }”: { $details }.
stdlib.fetch.disallowed = Chan eil an URL “{ $url }” ceadaichte: { $details }.
stdlib.fetch.failed = Cha b' urrainnear “{ $url }” fhaighinn: { $details }.
stdlib.fetch.cache_read_failed = Cha b' urrainnear innteart an tasgadain “{ $name }” a leughadh: { $details }.
stdlib.fetch.cache_open_failed = Cha b' urrainnear innteart an tasgadain “{ $name }” fhosgladh: { $details }.
stdlib.fetch.response_read_failed = Cha b' urrainnear an fhreagairt o “{ $url }” a leughadh: { $details }.
stdlib.fetch.response_buffer_overflow = Chuir am bufair thairis fhad 's a bhathar a' leughadh “{ $url }”.
stdlib.fetch.cache_write_failed = Cha b' urrainnear an tasgadan airson “{ $url }” a sgrìobhadh: { $details }.
stdlib.fetch.response_limit_exceeded = Chaidh an fhreagairt o “{ $url }” thairis air crìoch { $limit } baidht.
stdlib.fetch.cache_limit_exceeded = Chaidh an fhreagairt thasgte “{ $name }” thairis air crìoch { $limit } baidht.
stdlib.fetch.io_failed = Dh'fhàillig an gnìomh “{ $action }” airson { $path }: { $details }.
stdlib.fetch.action.sync_cache = co-thìmeachadh tasgadan fetch
stdlib.fetch.action.create_cache_dir = cruthachadh pasgan tasgadan fetch
stdlib.fetch.action.open_cache_dir = fosgladh pasgan tasgadan fetch
stdlib.fetch.action.stat_cache = leughadh fiosrachadh innteart tasgadan fetch
stdlib.fetch.action.open_cache_entry = fosgladh innteart tasgadan fetch

# Breithneachadh cuidiche nan àitheantan.
stdlib.command.location = an àithne “{ $command }” san teamplaid “{ $template }”
stdlib.command.spawn_failed = Cha b' urrainnear { $location } a thòiseachadh: { $details }.
stdlib.command.io_failed = Dh'fhàillig { $location }: { $details }.
stdlib.command.closed_input_early = Dhùin an cur-a-steach mus deach an sgrìobhadh don àithne a chrìochnachadh.
stdlib.command.broken_pipe = Bhris a' phìob fhad 's a bhathar a' ruith { $location }: { $details }.
stdlib.command.terminated_by_signal = Chaidh { $location } a chrìochnachadh le comharra.
stdlib.command.exited_with_status = Thàinig { $location } gu crìch le staid { $status }.
stdlib.command.output_limit_exceeded = Chaidh { $location } thairis air crìoch { $mode } de { $limit } baidht airson { $stream }.
stdlib.command.timeout = Chaidh { $location } thairis air crìoch-ùine de { $seconds } diog.
stdlib.command.exit_status_suffix = (staid fàgail { $status })
stdlib.command.signal_suffix = (air a chrìochnachadh le comharra)
stdlib.command.shell.empty = Chan fhaod àithne na slige a bhith falamh.
stdlib.command.grep.empty_pattern = Chan fhaod pàtran grep a bhith falamh.
stdlib.command.grep.flags_not_string = Feumaidh brataichean grep a bhith nan sreangan.
stdlib.command.quote.invalid = Cha b' urrainnear { $arg } a chur ann an comharran-labhairt: { $details }.
stdlib.command.quote.line_break = Chan urrainnear argamaidean le tilleadh-carbaid no briseadh-loidhne a chur ann an comharran-labhairt gu sàbhailte.
stdlib.command.input_undefined = Chan eil luach a' chuir-a-steach air a mhìneachadh.
stdlib.command.tempfile.root_required = Tha feum air freumh an raoin-obrach gus faidhlichean àithne sealach a chruthachadh.
stdlib.command.tempfile.create_failed = Cha b' urrainnear faidhle sealach na h-àithne a chruthachadh: { $details }.
stdlib.command.options.invalid_utf8 = Feumaidh iuchair roghainn na h-àithne a bhith na UTF-8 dhligheach.
stdlib.command.option.mode_not_string = Feumaidh am modh às-chuir a bhith na shreang.
stdlib.command.options.invalid_type = Feumaidh roghainnean na h-àithne a bhith nan oibseact.
stdlib.command.output.mode_unsupported = Modh às-chuir gun taic: “{ $mode }”.
stdlib.command.output.mode.capture = glacadh
stdlib.command.output.mode.streaming = sruthadh
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Breithneachadh cuidiche nan slighean.
stdlib.path.io.failed = Dh'fhàillig an gnìomh “{ $action }” airson { $path } ({ $label }).
stdlib.path.io.failed_with_detail = Dh'fhàillig an gnìomh “{ $action }” airson { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = Dh'fhàillig an gnìomh “{ $action }” airson { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = cha deach a lorg
stdlib.path.io.permission_denied = chaidh cead a dhiùltadh
stdlib.path.io.already_exists = ann mu thràth
stdlib.path.io.invalid_input = cur-a-steach mì-dhligheach
stdlib.path.io.invalid_data = dàta mì-dhligheach
stdlib.path.io.timed_out = dh'fhalbh an ùine
stdlib.path.io.interrupted = air a bhriseadh a-steach
stdlib.path.io.would_block = bhacadh e an obair
stdlib.path.io.write_zero = chaidh neoni baidht a sgrìobhadh
stdlib.path.io.unexpected_eof = deireadh faidhle gun dùil ris
stdlib.path.io.broken_pipe = pìob bhriste
stdlib.path.io.connection_refused = chaidh an ceangal a dhiùltadh
stdlib.path.io.connection_reset = chaidh an ceangal ath-shuidheachadh
stdlib.path.io.connection_aborted = chaidh an ceangal a sgur
stdlib.path.io.not_connected = gun cheangal
stdlib.path.io.addr_in_use = tha an seòladh ga chleachdadh
stdlib.path.io.addr_not_available = chan eil an seòladh ri fhaighinn
stdlib.path.io.out_of_memory = dh'fhalbh an cuimhne
stdlib.path.io.unsupported = gun taic
stdlib.path.io.file_too_large = tha am faidhle ro mhòr
stdlib.path.io.resource_busy = tha an goireas trang
stdlib.path.io.executable_busy = tha am faidhle so-ruithe trang
stdlib.path.io.deadlock = glasadh marbh
stdlib.path.io.crosses_devices = a' dol tarsainn air innealan
stdlib.path.io.too_many_links = cus cheanglaichean
stdlib.path.io.invalid_filename = ainm faidhle mì-dhligheach
stdlib.path.io.arg_list_too_long = tha liosta nan argamaidean ro fhada
stdlib.path.io.stale_handle = làmhrachan faidhle lìonraidh sean
stdlib.path.io.storage_full = tha an stòras làn
stdlib.path.io.not_seekable = cha ghabh ionad a shuidheachadh
stdlib.path.io.network_down = tha an lìonra sìos
stdlib.path.io.network_unreachable = cha ruigear an lìonra
stdlib.path.io.host_unreachable = cha ruigear an t-òstair
stdlib.path.io.other = mearachd ion-chuir/às-chuir
stdlib.path.action.canonicalize = bun-riaghailteachadh
stdlib.path.action.open_directory = fosgladh pasgain
stdlib.path.action.stat = leughadh fiosrachaidh
stdlib.path.action.read = leughadh
stdlib.path.action.open_file = fosgladh faidhle
stdlib.path.with_suffix.empty_separator = Tha with_suffix ag iarraidh sgaradair nach eil falamh.
stdlib.path.relative_to.mismatch = Chan eil { $path } coimeasach ri { $root }.
stdlib.path.expanduser.unsupported = Chan eil taic ann do leudachadh ~ airson cleachdaiche sònraichte.
stdlib.path.expanduser.no_home = Chan urrainnear ~ a leudachadh: chan eil caochladair àrainneachd sam bith ann airson a' phasgain dhachaigh.
stdlib.path.contents.unsupported_encoding = Còdachadh gun taic: “{ $encoding }”.
stdlib.path.hash.unsupported_algorithm = Algairim hais gun taic: “{ $algorithm }”.
stdlib.path.hash.unsupported_algorithm_legacy = Algairim hais gun taic: “{ $algorithm }” (cuir an comas am feart “{ $feature }”).

# Breithneachadh chuidichean nan cruinneachaidhean.
stdlib.collections.flatten.expected_sequence = Bha dùil aig flatten ri nithean sreath ach fhuair e { $kind }.
stdlib.collections.group_by.empty_attribute = Tha group_by ag iarraidh buadh nach eil falamh.
stdlib.collections.group_by.unresolved = Cha b' urrainn do group_by “{ $attr }” a lorg air nì den t-seòrsa { $kind }.

# Breithneachadh chuidichean na h-ùine.
stdlib.time.offset.invalid = Tha frith-ùine now “{ $offset }” mì-dhligheach: bha dùil ri “+HH:MM[:SS]” no “Z”.
stdlib.time.timedelta.overflow = Chuir timedelta thairis nuair a chaidh { $component } a chur ris.
stdlib.time.label.weeks = seachdainean
stdlib.time.label.days = làithean
stdlib.time.label.hours = uairean
stdlib.time.label.minutes = mionaidean
stdlib.time.label.seconds = diogan
stdlib.time.label.milliseconds = mille-dhiogan
stdlib.time.label.microseconds = meanbh-dhiogan
stdlib.time.label.nanoseconds = nano-dhiogan

# Breithneachadh a' chuidiche which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] cha deach an àithne “{ $command }” a lorg às dèidh { $count } innteart PATH a sgrùdadh. Ro-shealladh: { $preview }
stdlib.which.not_found.hint.cwd_auto = Thèid earrannan falamh de PATH a leigeil seachad; cleachd cwd_mode="auto" gus am pasgan obrach a ghabhail a-steach.
stdlib.which.not_found.hint.cwd_always = Suidhich cwd_mode="always" gus am pasgan làithreach a ghabhail a-steach.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] tha an àithne “{ $command }” aig “{ $path }” a dhìth no chan eil i so-ruithe.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <falamh>
stdlib.which.path_entry.non_utf8 = Tha caractaran nach eil nan UTF-8 ann an innteart àireamh { $index } de PATH; tha Netsuke ag iarraidh slighean UTF-8.
stdlib.which.command.empty = Tha which ag iarraidh sreang nach eil falamh.
stdlib.which.cwd_mode.invalid = Feumaidh cwd_mode a bhith na “auto”, “always” no “never”, ach fhuaireadh “{ $mode }”.
stdlib.which.cwd.resolve_failed = Cha b' urrainnear am pasgan làithreach a dhearbhadh: { $details }.
stdlib.which.cwd.non_utf8 = Tha pàirtean anns a' phasgan làithreach nach eil nan UTF-8.
stdlib.which.canonicalize_failed = Cha b' urrainnear “{ $path }” a bhun-riaghailteachadh: { $details }.
stdlib.which.is_executable = Cha b' urrainnear dearbhadh a bheil “{ $path }” so-ruithe: { $details }.
stdlib.which.canonicalize_non_utf8 = Tha pàirtean san t-slighe bhun-riaghailtich nach eil nan UTF-8.
stdlib.which.workspace_non_utf8 = Tha pàirtean ann an slighe an raoin-obrach nach eil nan UTF-8 nuair a bhathar a' fuasgladh na h-àithne “{ $command }”: { $path }.
stdlib.which.walkdir_error = Mearachd a' siubhal an raoin-obrach nuair a bhathar a' fuasgladh na h-àithne: { $details }.

# Clàradh an leabharlainn àbhaistich.
stdlib.register.open_dir = Cha b' urrainnear am pasgan làithreach fhosgladh airson clàradh stdlib.
stdlib.register.resolve_dir = Cha b' urrainnear am pasgan làithreach a dhearbhadh airson clàradh stdlib.
stdlib.register.dir_non_utf8 = Tha pàirtean anns a' phasgan làithreach nach eil nan UTF-8: { $path }.

# Aithris staide airson a' mhodh às-chuir so-ruigsinnich.
status.state.pending = a' feitheamh
status.state.running = a' dol air adhart
status.state.done = deiseil
status.state.failed = dh'fhàillig
status.stage.label = Ceum { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Obair { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = A' leughadh faidhle an fhoirm-liosta
status.stage.initial_yaml_parsing = A' parsadh na sgrìobhainn YAML
status.stage.template_expansion = A' leudachadh stiùiridhean nan teamplaidean
status.stage.final_rendering = A' dì-shreathachadh 's a' reandarachadh luachan an fhoirm-liosta
status.stage.ir_generation_validation = A' togail 's a' dearbhadh graf nan eisimeileachdan
status.stage.ninja_synthesis = A' cur ri chèile plana togail Ninja
status.stage.ninja_synthesis_execute = A' cur ri chèile plana Ninja 's a' ruith { $tool }
status.stage.graph_rendering = A' reandarachadh toradh a' ghraf
status.stage.graph_rendering_with_tool = A' reandarachadh { $tool }
status.complete = Chaidh { $tool } a chrìochnachadh.
status.timing.summary_header = Geàrr-chunntas ùine a rèir ceum:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Ùine iomlan na loidhne-obrach: { $duration }
status.tool.build = Togail
status.tool.clean = Glanadh
status.tool.graph = Graf
status.tool.graph_html = Graf (HTML)
status.tool.generate = Dèanamh
status.tool.help_targets = Cuideachadh thargaidean

# Sreangan reandaraiche HTML a' ghraf.
graph.html.title = Graf togail Netsuke
graph.html.heading = Graf togail Netsuke
graph.html.description = Graf togail a chaidh a reandarachadh le Netsuke
graph.html.outline.summary = Targaidean agus eisimeileachdan (dealbh teacsa)
graph.html.outline.no_inputs = Gun chur-a-steach
graph.html.noscript.notice = Tha JavaScript à comas. Is e an dealbh teacsa gu h-àrd an graf gu lèir; leanaidh tùs DOT air a shàilean.

# Ro-leasachain bhrìgheil airson an às-chuir so-ruigsinnich.
semantic.prefix.error = Mearachd:
semantic.prefix.warning = Rabhadh:
semantic.prefix.success = Soirbheas:
semantic.prefix.info = Fiosrachadh:
semantic.prefix.timing = Ùine:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Eisimpleirean de na foirmean iolra do dh'eadar-theangairean.
# Tha ceithir roinnean CLDR aig a' Ghàidhlig: `one` (1, 11), `two` (2, 12),
# `few` (3–10, 13–19) agus `other`, agus tha an t-ainmear ag atharrachadh.
example.files_processed = { $count ->
    [one] Chaidh { $count } fhaidhle a phròiseasadh.
    [two] Chaidh { $count } fhaidhle a phròiseasadh.
    [few] Chaidh { $count } faidhlichean a phròiseasadh.
   *[other] Chaidh { $count } faidhle a phròiseasadh.
}

example.errors_found = { $count ->
    [0] Cha deach mearachd sam bith a lorg.
    [one] Chaidh { $count } mhearachd a lorg.
    [two] Chaidh { $count } mhearachd a lorg.
    [few] Chaidh { $count } mearachdan a lorg.
   *[other] Chaidh { $count } mearachd a lorg.
}
