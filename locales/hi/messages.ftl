# Netsuke की कमांड लाइन के लिए स्थानीयकरण संसाधन।

runner.io.dyndep.retention = बनाई गई dyndep को { $path } के नीचे बनाए रखने की प्रक्रिया लागू नहीं की जा सकी।
cli.about = Netsuke YAML + Jinja मैनिफ़ेस्ट को Ninja की बिल्ड योजनाओं में संकलित करता है।
cli.long_about = Netsuke YAML + Jinja मैनिफ़ेस्ट को पुनरुत्पादनीय Ninja ग्राफ़ में बदलता है और सुरक्षित डिफ़ॉल्ट मानों के साथ Ninja चलाता है।
cli.usage = { $usage }

# सामान्य विकल्पों का सहायता पाठ।
cli.flag.file.help = उपयोग की जाने वाली Netsuke मैनिफ़ेस्ट फ़ाइल का पथ।
cli.flag.directory.help = इस तरह चलाएँ मानो आरंभ इसी निर्देशिका में हुआ हो।
cli.flag.config.help = किसी विन्यास फ़ाइल का पथ, स्वतः खोज को छोड़ते हुए।
cli.flag.jobs.help = समांतर बिल्ड कार्यों की संख्या तय करें।
cli.flag.verbose.help = विस्तृत निदान लॉग और समाप्ति पर समय का सारांश सक्षम करें।
cli.flag.locale.help = कमांड लाइन के पाठ के लिए भाषा टैग (उदाहरण: en-US, hi)।
cli.flag.fetch_allow_scheme.help = fetch सहायक के लिए अतिरिक्त अनुमत URL स्कीम।
cli.flag.fetch_allow_host.help = डिफ़ॉल्ट अस्वीकृति सक्रिय होने पर अनुमत होस्ट नाम।
cli.flag.fetch_block_host.help = वे होस्ट नाम जो सदैव अवरुद्ध रहते हैं, भले ही अन्यत्र अनुमत हों।
cli.flag.fetch_default_deny.help = डिफ़ॉल्ट रूप से सभी होस्ट अस्वीकार करें; केवल घोषित सूची को अनुमति दें।
cli.flag.json.help = मशीन-पठनीय JSON निर्गत करें।
cli.flag.no_input.help = संवादात्मक इनपुट कभी न पढ़ें।
cli.flag.color.help = रंगीन निर्गम की नीति (auto, always, never)।
cli.flag.emoji.help = इमोजी की नीति (auto, always, never)।
cli.flag.progress.help = प्रगति दिखाने की नीति (auto, always, never)।
cli.flag.accessibility.help = सुगम्य निर्गम की नीति (auto, on, off)।
cli.flag.default_targets.help = कोई लक्ष्य न बताए जाने पर उपयोग होने वाले डिफ़ॉल्ट बिल्ड लक्ष्य।

# उपआदेशों का विवरण।
cli.subcommand.build.about = मैनिफ़ेस्ट में परिभाषित लक्ष्यों का निर्माण करें (डिफ़ॉल्ट)।
cli.subcommand.build.long_about = माँगे गए लक्ष्यों का निर्माण करें; कोई न बताया जाए तो मैनिफ़ेस्ट के डिफ़ॉल्ट लक्ष्य लें।
cli.subcommand.clean.about = Ninja के माध्यम से बिल्ड के उत्पाद हटाएँ।
cli.subcommand.clean.long_about = एक अस्थायी Ninja फ़ाइल बनाएँ, फिर `ninja -t clean` चलाएँ।
cli.subcommand.graph.about = बिल्ड की निर्भरता ग्राफ़ निर्गत करें। डिफ़ॉल्ट प्रारूप DOT है।
cli.subcommand.graph.long_about = विश्लेषित Netsuke मैनिफ़ेस्ट को मानक बिल्ड ग्राफ़ में प्रक्षिप्त करें और उसे Graphviz DOT के रूप में लिखें, अथवा `--html` के साथ स्वतः पूर्ण HTML पृष्ठ के रूप में। फ़ाइल में लिखने हेतु `--output <फ़ाइल>` का प्रयोग करें; `-` मानक निर्गम पर लिखता है।
cli.subcommand.generate.about = Ninja चलाए बिना Ninja मैनिफ़ेस्ट बनाएँ।
cli.subcommand.generate.long_about = बनाया गया Ninja मैनिफ़ेस्ट मानक निर्गम पर लिखें, अथवा `--output` से चुनी गई फ़ाइल में।
cli.subcommand.help.about = शीर्ष-स्तरीय सहायता, या किसी नामित विषय की सहायता प्रिंट करें।
cli.subcommand.help.long_about = बिना विषय के यह `--help` से मेल खाता है। चयनित मैनिफेस्ट के लिए लक्ष्य और क्रिया सूची प्रिंट करने हेतु `help targets` का उपयोग करें।

# Help catalogue headings and markers.
cli.help.actions_heading = क्रियाएँ:
cli.help.targets_heading = लक्ष्य:
cli.help.targets.about = चयनित मैनिफेस्ट में लक्ष्यों और क्रियाओं की सूची बनाएँ।
cli.help.default_marker = डिफ़ॉल्ट

# build उपआदेश के विकल्पों का सहायता पाठ।
cli.subcommand.build.flag.targets.help = बनाए जाने वाले लक्ष्य (न बताए जाने पर मैनिफ़ेस्ट के डिफ़ॉल्ट लिए जाते हैं)।

# graph उपआदेश के विकल्पों का सहायता पाठ।
cli.subcommand.graph.flag.html.help = ग्राफ़ को DOT के बजाय स्वतः पूर्ण HTML पृष्ठ के रूप में प्रस्तुत करें।
cli.subcommand.graph.flag.output.help = ग्राफ़ का उत्पाद फ़ाइल में लिखें; मानक निर्गम के लिए `-` का प्रयोग करें।

# generate उपआदेश के विकल्पों का सहायता पाठ।
cli.subcommand.generate.flag.output.help = बनाया गया Ninja मैनिफ़ेस्ट मानक निर्गम के बजाय फ़ाइल में लिखें।

# कमांड लाइन की सत्यापन त्रुटियाँ।
cli.validation.jobs.invalid_number = { $value } एक मान्य संख्या नहीं है।
cli.validation.jobs.out_of_range = कार्यों की संख्या { $min } और { $max } के बीच होनी चाहिए।
cli.validation.scheme.empty = स्कीम रिक्त नहीं होनी चाहिए।
cli.validation.scheme.invalid_start = स्कीम “{ $scheme }” का आरंभ ASCII अक्षर से होना चाहिए।
cli.validation.scheme.invalid = अमान्य स्कीम: “{ $scheme }”।
cli.validation.locale.empty = भाषा टैग रिक्त नहीं होना चाहिए।
cli.validation.locale.invalid = अमान्य भाषा टैग: “{ $locale }”।
cli.validation.color.invalid = अमान्य रंग नीति: “{ $value }”। मान्य विकल्प: auto, always, never।
cli.validation.emoji.invalid = अमान्य इमोजी नीति: “{ $value }”। मान्य विकल्प: auto, always, never।
cli.validation.progress.invalid = अमान्य प्रगति नीति: “{ $value }”। मान्य विकल्प: auto, always, never।
cli.validation.accessibility.invalid = अमान्य सुगम्यता नीति: “{ $value }”। मान्य विकल्प: auto, on, off।
cli.validation.config.expected_object = कमांड लाइन के मानों का क्रमांकन किसी वस्तु में होना चाहिए था, किंतु { $value } मिला।

# Clap की त्रुटि सूचनाएँ।
clap-error-missing-argument = आवश्यक तर्क अनुपस्थित है: { $argument }
clap-error-missing-subcommand = उपआदेश अनुपस्थित है। उपलब्ध विकल्प: { $valid_subcommands }
clap-error-unknown-argument = अज्ञात तर्क: { $argument }
clap-error-invalid-value = { $argument } के लिए अमान्य मान: { $value }
clap-error-invalid-subcommand = अज्ञात उपआदेश: { $subcommand }
# ध्यान दें: value-validation का शब्दन invalid-value से भिन्न रखा गया है ताकि
# अपने सत्यापकों की विफलता (ErrorKind::ValueValidation) और प्रकार की असंगति
# (ErrorKind::InvalidValue) में अंतर बना रहे।
clap-error-value-validation = { $argument } का सत्यापन विफल रहा: { $value }

# चलाने के दौरान की त्रुटियाँ और संदर्भ।
runner.manifest.not_found = मैनिफ़ेस्ट “{ $manifest_name }” { $directory } में नहीं मिला।
runner.manifest.not_found.help = सुनिश्चित करें कि मैनिफ़ेस्ट मौजूद है, अथवा सही पथ के साथ `--file` दें।
runner.manifest.path_missing_name = मैनिफ़ेस्ट पथ “{ $path }” में फ़ाइल नाम नहीं है।
runner.manifest.path_utf8 = मैनिफ़ेस्ट पथ “{ $path }” मान्य UTF-8 नहीं है।
runner.manifest.directory_utf8 = मैनिफ़ेस्ट की निर्देशिका का पथ “{ $path }” मान्य UTF-8 नहीं है।
runner.manifest.directory_label = निर्देशिका `{ $directory }`
runner.manifest.current_directory_label = वर्तमान निर्देशिका
runner.manifest.default_not_declared = मैनिफ़ेस्ट डिफ़ॉल्ट '{ $default }' किसी घोषित क्रिया या लक्ष्य का नाम नहीं है।
runner.context.network_policy = नेटवर्क नीति नहीं बनाई जा सकी।
runner.context.load_manifest = { $path } से मैनिफ़ेस्ट नहीं लादा जा सका।
runner.context.serialise_manifest = मैनिफ़ेस्ट का क्रमांकन नहीं हो सका।
runner.context.build_graph = मैनिफ़ेस्ट से ग्राफ़ नहीं बनाया जा सका।
runner.context.generate_ninja = Ninja मैनिफ़ेस्ट नहीं बनाया जा सका।
runner.context.render_graph = ग्राफ़ का उत्पाद प्रस्तुत नहीं किया जा सका।

runner.io.create_temp_file = अस्थायी Ninja फ़ाइल नहीं बनाई जा सकी।
runner.io.write_temp_ninja = अस्थायी Ninja फ़ाइल में नहीं लिखा जा सका।
runner.io.flush_temp_ninja = अस्थायी Ninja फ़ाइल का बफ़र खाली नहीं किया जा सका।
runner.io.sync_temp_ninja = अस्थायी Ninja फ़ाइल समकालिक नहीं की जा सकी।
runner.io.create_parent_dir = मूल निर्देशिका { $path } नहीं बनाई जा सकी।
runner.io.create_ninja_file = { $path } पर Ninja फ़ाइल नहीं बनाई जा सकी।
runner.io.write_ninja_file = { $path } की Ninja फ़ाइल में नहीं लिखा जा सका।
runner.io.flush_ninja_file = { $path } की Ninja फ़ाइल का बफ़र खाली नहीं किया जा सका।
runner.io.sync_ninja_file = { $path } की Ninja फ़ाइल समकालिक नहीं की जा सकी।
runner.io.open_ambient_dir = आसपास की निर्देशिका नहीं खोली जा सकी।
runner.io.non_utf8_working_directory = कार्य निर्देशिका का पथ मान्य UTF-8 नहीं है।
runner.io.no_existing_ancestor = { $path } के लिए कोई विद्यमान पूर्वज निर्देशिका नहीं है।
runner.io.derive_relative_path = सापेक्ष Ninja पथ नहीं निकाला जा सका।
runner.io.non_utf8_path = UTF-8 से भिन्न पथ समर्थित नहीं हैं (पथ: { $path })।
runner.io.write_stdout = Ninja मैनिफ़ेस्ट मानक निर्गम पर नहीं लिखा जा सका।
runner.io.flush_stdout = मानक निर्गम का बफ़र खाली नहीं किया जा सका।
runner.io.dyndep.create_dir = dyndep निर्देशिका { $path } नहीं बनाई जा सकी।
runner.io.dyndep.read = { $path } पर बनाई गई dyndep फ़ाइल पढ़ी नहीं जा सकी।
runner.io.dyndep.write = { $path } पर बनाई गई dyndep फ़ाइल लिखी नहीं जा सकी।
runner.io.dyndep.rename = { $path } पर बनाई गई dyndep फ़ाइल का नाम नहीं बदला जा सका।
runner.io.dyndep.corrupt = { $path } पर बनाई गई dyndep फ़ाइल अपेक्षित सामग्री से मेल नहीं खाती; केवल उसी फ़ाइल को हटाकर फिर प्रयास करें।
runner.io.dyndep.temp_collisions = बार-बार नाम टकराने के बाद { $path } के लिए एक अद्वितीय अस्थायी dyndep फ़ाइल नहीं बनाई जा सकी।
runner.io.dyndep.too_large = { $path } पर बनाई गई dyndep फ़ाइल { $limit } बाइट की सत्यापन सीमा से बड़ी है।

# मैनिफ़ेस्ट के निदान।
manifest.parse = मैनिफ़ेस्ट का विश्लेषण विफल रहा।
manifest.structure_error = { $name } पर मैनिफ़ेस्ट की संरचना में त्रुटि: { $details }
manifest.yaml.parse = पंक्ति { $line }, स्तंभ { $column } पर YAML विश्लेषण त्रुटि: { $details }
manifest.yaml.label = अमान्य YAML
manifest.yaml.hint.tabs = YAML टैब की अनुमति नहीं देता; अंतर्वेशन के लिए रिक्त स्थान लें।
manifest.yaml.hint.list_item = YAML सूची की मदें “-” से आरंभ होनी चाहिए और सही ढंग से अंतर्वेशित होनी चाहिए।
manifest.yaml.hint.expected_colon = यह प्रतिचित्रण की प्रविष्टि जान पड़ती है; कुंजी के बाद “:” नहीं है।
manifest.yaml.hint.mapping_values = YAML प्रतिचित्रणों में “:” के बाद कोई मान (अथवा नेस्टेड खंड) चाहिए।
manifest.yaml.hint.invalid_token = YAML टोकन अमान्य अथवा अप्रत्याशित है।
manifest.yaml.hint.escape = बैकस्लैश को एस्केप करें अथवा अमान्य एस्केप अनुक्रम हटाएँ।
manifest.env.missing = एक आवश्यक परिवेश चर निर्धारित नहीं है।
manifest.env.invalid_utf8 = एक परिवेश चर में अमान्य UTF-8 है।
manifest.vars.not_object = मैनिफ़ेस्ट का `vars` प्रतिचित्रण अथवा वस्तु होना चाहिए।
manifest.vars.reserved_name = मैनिफ़ेस्ट की `vars` कुंजी '{ $name }' अंतर्निहित टेम्पलेट सहायक के लिए आरक्षित है; चर का नाम बदलें।
manifest.read_failed = { $path } से मैनिफ़ेस्ट नहीं पढ़ा जा सका।
manifest.resolve_workspace_root = कार्यक्षेत्र की जड़ निर्धारित नहीं की जा सकी।
manifest.workspace_non_utf8 = कार्यक्षेत्र का मूल पथ “{ $path }” मान्य UTF-8 नहीं है।
manifest.path_non_utf8 = मैनिफ़ेस्ट “{ $manifest }” का पथ मान्य UTF-8 नहीं है: { $path }।
manifest.path_missing_name = मैनिफ़ेस्ट पथ “{ $path }” में फ़ाइल नाम नहीं है।
manifest.open_workspace_failed = मैनिफ़ेस्ट { $manifest } के लिए कार्यक्षेत्र { $workspace } नहीं खोला जा सका।
manifest.foreach.not_iterable = `foreach` व्यंजक पुनरावृत्त नहीं किया जा सकता।
manifest.foreach.serialise_item = `foreach` की मद का क्रमांकन नहीं हो सका।
manifest.when.empty = `when` व्यंजक रिक्त नहीं होना चाहिए।
manifest.when.eval_error = `when` व्यंजक “{ $expr }” का मूल्यांकन नहीं हो सका।
manifest.when.template_error = `when` टेम्पलेट “{ $expr }” प्रस्तुत नहीं किया जा सका।
manifest.target.vars_not_object = लक्ष्य का `vars` वस्तु होना चाहिए, किंतु { $value } मिला।
manifest.vars.entry_not_object = मैनिफ़ेस्ट की `vars` प्रविष्टि वस्तु होनी चाहिए।
manifest.field_not_string = क्षेत्र “{ $field }” स्ट्रिंग होना चाहिए।
manifest.expression.parse_error = { $name } व्यंजक का विश्लेषण नहीं हो सका।
manifest.expression.eval_error = { $name } व्यंजक का मूल्यांकन नहीं हो सका।

# मैनिफ़ेस्ट के मैक्रो संबंधी निदान।
manifest.macro.signature_missing_identifier = मैक्रो के हस्ताक्षर में पहचानकर्ता नहीं है।
manifest.macro.signature_missing_params = मैक्रो के हस्ताक्षर में प्राचल नहीं हैं।
manifest.macro.compile_failed = मैक्रो { $name } संकलित नहीं हो सका।
manifest.macro.sequence_invalid = मैक्रो नामों से टेम्पलेट तक के प्रतिचित्रण के रूप में परिभाषित होने चाहिए।
manifest.macro.register_failed = मैनिफ़ेस्ट के मैक्रो पंजीकृत नहीं हो सके।
manifest.macro.not_initialised = मैक्रो का परिवेश आरंभीकृत नहीं है।
manifest.macro.caller_invalid = मैक्रो का आह्वानकर्ता स्ट्रिंग होना चाहिए।
manifest.macro.template_load_failed = मैक्रो का टेम्पलेट नहीं लादा जा सका।
manifest.macro.init_failed = मैक्रो का परिवेश आरंभीकृत नहीं हो सका।
manifest.macro.missing = मैक्रो { $name } अनुपस्थित है।

# मैनिफ़ेस्ट की glob त्रुटियाँ।
manifest.glob.unmatched_brace = अमान्य glob प्रतिरूप “{ $pattern }”: स्थान { $position } पर “{ $character }” का युग्म नहीं है।
manifest.glob.invalid_pattern = अमान्य glob प्रतिरूप “{ $pattern }”: { $detail }।
manifest.glob.unknown_pattern_error = अज्ञात प्रतिरूप त्रुटि।
manifest.glob.io_failed = “{ $pattern }” के लिए glob विफल रहा: { $detail }।
manifest.glob.unknown_io_error = अज्ञात इनपुट/आउटपुट त्रुटि।
manifest.command_list_empty = “command” फ़ील्ड रिक्त नहीं होना चाहिए: कोई कमांड स्ट्रिंग या ग़ैर-रिक्त सूची दें।

# मध्यवर्ती निरूपण की त्रुटियाँ।
ir.rule_not_found = लक्ष्य “{ $target }” जिस नियम “{ $rule }” का संदर्भ देता है वह नहीं मिला।
ir.multiple_rules = लक्ष्य “{ $target }” को ठीक एक नियम का संदर्भ देना चाहिए, किंतु { $rules } मिला।
ir.empty_rule = लक्ष्य “{ $target }” को किसी नियम का संदर्भ देना चाहिए।
ir.duplicate_outputs = दोहरे निर्गम मिले: { $outputs }।
ir.circular_dependency = चक्रीय निर्भरता मिली: { $cycle }।
ir.action_serialisation = क्रिया का क्रमांकन नहीं हो सका: { $details }।
ir.invalid_command = आदेश में अमान्य प्रक्षेपण: { $snippet }।

# Ninja निर्माण की त्रुटियाँ।
ninja_gen.missing_action = किसी बिल्ड कोर द्वारा संदर्भित क्रिया “{ $id }” अनुपस्थित है।
ninja_gen.format = Ninja मैनिफ़ेस्ट का निर्गम स्वरूपित नहीं किया जा सका।
ninja_gen.dyndep_files_required = इस ऑपरेशन के लिए जनरेट किया गया Ninja बंडल आवश्यक है; dyndep फ़ाइलों को मूर्त रूप देने के लिए `netsuke build`, `netsuke clean` या `netsuke generate` का उपयोग करें।
ninja_gen.reserved_output_path = पथ '{ $path }' Netsuke की क्रमिक निर्भरता स्थिति के लिए आरक्षित है।
ninja_gen.unsupported_path_character = पथ '{ $path }' में असमर्थित Ninja पथ वर्ण '{ $character }' है।

# होस्ट प्रतिरूपों का सत्यापन।
host_pattern.empty = होस्ट प्रतिरूप रिक्त नहीं होना चाहिए।
host_pattern.contains_scheme = होस्ट प्रतिरूप “{ $pattern }” में URL स्कीम नहीं होनी चाहिए।
host_pattern.contains_slash = होस्ट प्रतिरूप “{ $pattern }” में “/” नहीं होना चाहिए।
host_pattern.missing_suffix = होस्ट प्रतिरूप “{ $pattern }” में “*.” के बाद प्रत्यय होना चाहिए।
host_pattern.empty_label = होस्ट प्रतिरूप “{ $pattern }” में रिक्त लेबल है।
host_pattern.invalid_chars = होस्ट प्रतिरूप “{ $pattern }” में अमान्य वर्ण हैं।
host_pattern.invalid_label_edge = होस्ट प्रतिरूप “{ $pattern }” के लेबल “-” से आरंभ अथवा समाप्त नहीं होने चाहिए।
host_pattern.label_too_long = होस्ट प्रतिरूप “{ $pattern }” में 63 वर्णों से लंबा लेबल है।
host_pattern.too_long = होस्ट प्रतिरूप “{ $pattern }” 255 वर्णों की सीमा से अधिक है।

# नेटवर्क नीति।
network_policy.scheme.empty = स्कीम रिक्त नहीं होनी चाहिए।
network_policy.scheme.invalid = स्कीम “{ $scheme }” में अमान्य वर्ण हैं।
network_policy.allowlist.empty = अनुमत होस्ट की सूची रिक्त नहीं होनी चाहिए।
network_policy.scheme.not_allowed = स्कीम “{ $scheme }” अनुमत नहीं है।
network_policy.missing_host = URL में होस्ट नहीं है।
network_policy.host.blocked = होस्ट “{ $host }” नीति द्वारा अवरुद्ध है।
network_policy.host.not_allowlisted = होस्ट “{ $host }” अनुमत सूची में नहीं है।

# मानक पुस्तकालय का विन्यास।
stdlib.config.default_fetch_cache_invalid = fetch कैश का डिफ़ॉल्ट पथ सापेक्ष होना चाहिए।
stdlib.config.default_which_cache_invalid = which कैश की डिफ़ॉल्ट क्षमता धनात्मक होनी चाहिए।
stdlib.config.workspace_root_absolute = कार्यक्षेत्र का मूल पथ निरपेक्ष होना चाहिए।
stdlib.config.fetch_response_limit_positive = fetch की अनुक्रिया सीमा धनात्मक होनी चाहिए।
stdlib.config.command_output_limit_positive = आदेश के निर्गम को संचित करने की सीमा धनात्मक होनी चाहिए।
stdlib.config.command_stream_limit_positive = आदेशों की धारा सीमा धनात्मक होनी चाहिए।
stdlib.config.which_cache_capacity_positive = which कैश की क्षमता धनात्मक होनी चाहिए।
stdlib.config.skip_dir_empty = छोड़ी जाने वाली निर्देशिकाओं की प्रविष्टियाँ रिक्त नहीं होनी चाहिए।
stdlib.config.skip_dir_navigation = छोड़ी जाने वाली निर्देशिकाओं की प्रविष्टियों में “..” नहीं होना चाहिए।
stdlib.config.skip_dir_separator = छोड़ी जाने वाली निर्देशिकाओं की प्रविष्टियों में पथ विभाजक नहीं होने चाहिए।
stdlib.config.fetch_cache_empty = fetch कैश का पथ रिक्त नहीं होना चाहिए।
stdlib.config.fetch_cache_not_relative = fetch कैश का पथ सापेक्ष होना चाहिए, किंतु { $path } मिला।
stdlib.config.fetch_cache_escapes = fetch कैश का पथ कार्यक्षेत्र से बाहर नहीं जाना चाहिए: { $path }।
stdlib.config.open_workspace_root = वर्तमान निर्देशिका को stdlib कार्यक्षेत्र की जड़ के रूप में नहीं खोला जा सका।
stdlib.config.resolve_cwd = वर्तमान निर्देशिका को stdlib कार्यक्षेत्र की जड़ के रूप में निर्धारित नहीं किया जा सका।
stdlib.config.cwd_non_utf8 = वर्तमान निर्देशिका में ऐसे अंश हैं जो UTF-8 नहीं हैं: { $path }।

# fetch सहायक के निदान।
stdlib.fetch.url_invalid = अमान्य URL “{ $url }”: { $details }।
stdlib.fetch.disallowed = URL “{ $url }” अनुमत नहीं है: { $details }।
stdlib.fetch.failed = “{ $url }” प्राप्त नहीं किया जा सका: { $details }।
stdlib.fetch.cache_read_failed = कैश प्रविष्टि “{ $name }” नहीं पढ़ी जा सकी: { $details }।
stdlib.fetch.cache_open_failed = कैश प्रविष्टि “{ $name }” नहीं खोली जा सकी: { $details }।
stdlib.fetch.response_read_failed = “{ $url }” से अनुक्रिया नहीं पढ़ी जा सकी: { $details }।
stdlib.fetch.response_buffer_overflow = “{ $url }” पढ़ते समय बफ़र भर गया।
stdlib.fetch.cache_write_failed = “{ $url }” के लिए कैश नहीं लिखा जा सका: { $details }।
stdlib.fetch.response_limit_exceeded = “{ $url }” से आई अनुक्रिया { $limit } बाइट की सीमा से अधिक थी।
stdlib.fetch.cache_limit_exceeded = कैश में रखी अनुक्रिया “{ $name }” { $limit } बाइट की सीमा से अधिक थी।
stdlib.fetch.io_failed = { $path } पर क्रिया “{ $action }” विफल रही: { $details }।
stdlib.fetch.action.sync_cache = fetch कैश का समकालन
stdlib.fetch.action.create_cache_dir = fetch कैश निर्देशिका का निर्माण
stdlib.fetch.action.open_cache_dir = fetch कैश निर्देशिका को खोलना
stdlib.fetch.action.stat_cache = fetch कैश प्रविष्टि का विवरण पढ़ना
stdlib.fetch.action.open_cache_entry = fetch कैश प्रविष्टि को खोलना

# आदेश सहायक के निदान।
stdlib.command.location = टेम्पलेट “{ $template }” में आदेश “{ $command }”
stdlib.command.spawn_failed = { $location } आरंभ नहीं किया जा सका: { $details }।
stdlib.command.io_failed = { $location } विफल रहा: { $details }।
stdlib.command.closed_input_early = आदेश को लिखना पूरा होने से पहले ही इनपुट बंद हो गया।
stdlib.command.broken_pipe = { $location } चलाते समय पाइप टूट गया: { $details }।
stdlib.command.terminated_by_signal = { $location } संकेत द्वारा समाप्त हुआ।
stdlib.command.exited_with_status = { $location } स्थिति { $status } के साथ समाप्त हुआ।
stdlib.command.output_limit_exceeded = { $location } ने { $stream } के लिए { $mode } की { $limit } बाइट सीमा पार कर दी।
stdlib.command.timeout = { $location } { $seconds } सेकंड की समय सीमा से आगे चला गया।
stdlib.command.exit_status_suffix = (निकास स्थिति { $status })
stdlib.command.signal_suffix = (संकेत द्वारा समाप्त)
stdlib.command.shell.empty = शेल आदेश रिक्त नहीं होना चाहिए।
stdlib.command.grep.empty_pattern = grep का प्रतिरूप रिक्त नहीं होना चाहिए।
stdlib.command.grep.flags_not_string = grep के फ़्लैग स्ट्रिंग होने चाहिए।
stdlib.command.quote.invalid = { $arg } को उद्धरण चिह्नों में नहीं रखा जा सका: { $details }।
stdlib.command.quote.line_break = गाड़ी वापसी अथवा पंक्ति परिवर्तन वाले तर्कों को सुरक्षित रूप से उद्धृत नहीं किया जा सकता।
stdlib.command.input_undefined = इनपुट का मान अपरिभाषित है।
stdlib.command.tempfile.root_required = आदेश की अस्थायी फ़ाइलें बनाने के लिए कार्यक्षेत्र की जड़ चाहिए।
stdlib.command.tempfile.create_failed = आदेश की अस्थायी फ़ाइल नहीं बनाई जा सकी: { $details }।
stdlib.command.options.invalid_utf8 = आदेश के विकल्प की कुंजी मान्य UTF-8 होनी चाहिए।
stdlib.command.option.mode_not_string = निर्गम का ढंग स्ट्रिंग होना चाहिए।
stdlib.command.options.invalid_type = आदेश के विकल्प वस्तु होने चाहिए।
stdlib.command.output.mode_unsupported = असमर्थित निर्गम ढंग: “{ $mode }”।
stdlib.command.output.mode.capture = संचयन
stdlib.command.output.mode.streaming = धारा
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# पथ सहायक के निदान।
stdlib.path.io.failed = { $path } पर क्रिया “{ $action }” विफल रही ({ $label })।
stdlib.path.io.failed_with_detail = { $path } पर क्रिया “{ $action }” विफल रही: { $detail }।
stdlib.path.io.failed_with_label_and_detail = { $path } पर क्रिया “{ $action }” विफल रही ({ $label }): { $detail }।
stdlib.path.io.not_found = नहीं मिला
stdlib.path.io.permission_denied = अनुमति अस्वीकृत
stdlib.path.io.already_exists = पहले से विद्यमान
stdlib.path.io.invalid_input = अमान्य इनपुट
stdlib.path.io.invalid_data = अमान्य आँकड़े
stdlib.path.io.timed_out = समय समाप्त
stdlib.path.io.interrupted = बाधित
stdlib.path.io.would_block = अवरोध उत्पन्न करता
stdlib.path.io.write_zero = शून्य बाइट लिखे गए
stdlib.path.io.unexpected_eof = फ़ाइल का अप्रत्याशित अंत
stdlib.path.io.broken_pipe = टूटा हुआ पाइप
stdlib.path.io.connection_refused = संबंध अस्वीकृत
stdlib.path.io.connection_reset = संबंध पुनःस्थापित
stdlib.path.io.connection_aborted = संबंध विफल
stdlib.path.io.not_connected = संबंध नहीं है
stdlib.path.io.addr_in_use = पता पहले से प्रयोग में है
stdlib.path.io.addr_not_available = पता उपलब्ध नहीं है
stdlib.path.io.out_of_memory = स्मृति समाप्त
stdlib.path.io.unsupported = असमर्थित
stdlib.path.io.file_too_large = फ़ाइल बहुत बड़ी है
stdlib.path.io.resource_busy = संसाधन व्यस्त है
stdlib.path.io.executable_busy = निष्पादनीय फ़ाइल व्यस्त है
stdlib.path.io.deadlock = गतिरोध
stdlib.path.io.crosses_devices = उपकरणों की सीमा पार करता है
stdlib.path.io.too_many_links = बहुत अधिक कड़ियाँ
stdlib.path.io.invalid_filename = अमान्य फ़ाइल नाम
stdlib.path.io.arg_list_too_long = तर्कों की सूची बहुत लंबी है
stdlib.path.io.stale_handle = बासी नेटवर्क फ़ाइल हैंडल
stdlib.path.io.storage_full = भंडारण भर गया
stdlib.path.io.not_seekable = स्थान निर्धारण संभव नहीं
stdlib.path.io.network_down = नेटवर्क बंद है
stdlib.path.io.network_unreachable = नेटवर्क तक पहुँच नहीं
stdlib.path.io.host_unreachable = होस्ट तक पहुँच नहीं
stdlib.path.io.other = इनपुट/आउटपुट त्रुटि
stdlib.path.action.canonicalize = मानकीकरण
stdlib.path.action.open_directory = निर्देशिका खोलना
stdlib.path.action.stat = विवरण पढ़ना
stdlib.path.action.read = पढ़ना
stdlib.path.action.open_file = फ़ाइल खोलना
stdlib.path.with_suffix.empty_separator = with_suffix को अरिक्त विभाजक चाहिए।
stdlib.path.relative_to.mismatch = { $path } { $root } के सापेक्ष नहीं है।
stdlib.path.expanduser.unsupported = किसी विशेष उपयोक्ता के लिए ~ का विस्तार समर्थित नहीं है।
stdlib.path.expanduser.no_home = ~ का विस्तार नहीं हो सकता: गृह निर्देशिका का कोई परिवेश चर निर्धारित नहीं है।
stdlib.path.contents.unsupported_encoding = असमर्थित कूटलेखन: “{ $encoding }”।
stdlib.path.hash.unsupported_algorithm = असमर्थित हैश कलनविधि: “{ $algorithm }”।
stdlib.path.hash.unsupported_algorithm_legacy = असमर्थित हैश कलनविधि: “{ $algorithm }” (“{ $feature }” सुविधा सक्षम करें)।

# संग्रह सहायकों के निदान।
stdlib.collections.flatten.expected_sequence = flatten को अनुक्रम की मदें अपेक्षित थीं, किंतु { $kind } मिला।
stdlib.collections.group_by.empty_attribute = group_by को अरिक्त गुण चाहिए।
stdlib.collections.group_by.unresolved = group_by { $kind } प्रकार की मद पर “{ $attr }” नहीं खोज सका।

# समय सहायकों के निदान।
stdlib.time.offset.invalid = now का विचलन “{ $offset }” अमान्य है: “+HH:MM[:SS]” अथवा “Z” अपेक्षित था।
stdlib.time.timedelta.overflow = { $component } जोड़ते समय timedelta भर गया।
stdlib.time.label.weeks = सप्ताह
stdlib.time.label.days = दिन
stdlib.time.label.hours = घंटे
stdlib.time.label.minutes = मिनट
stdlib.time.label.seconds = सेकंड
stdlib.time.label.milliseconds = मिलीसेकंड
stdlib.time.label.microseconds = माइक्रोसेकंड
stdlib.time.label.nanoseconds = नैनोसेकंड

# which सहायक के निदान।
stdlib.which.not_found = [netsuke::jinja::which::not_found] PATH की { $count } प्रविष्टियाँ जाँचने पर भी आदेश “{ $command }” नहीं मिला। झलक: { $preview }
stdlib.which.not_found.hint.cwd_auto = PATH के रिक्त खंड अनदेखे रहते हैं; कार्य निर्देशिका सम्मिलित करने हेतु cwd_mode="auto" लें।
stdlib.which.not_found.hint.cwd_always = वर्तमान निर्देशिका सम्मिलित करने हेतु cwd_mode="always" निर्धारित करें।
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] “{ $path }” पर आदेश “{ $command }” अनुपस्थित है अथवा निष्पादनीय नहीं है।
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <रिक्त>
stdlib.which.path_entry.non_utf8 = PATH की { $index }वीं प्रविष्टि में ऐसे वर्ण हैं जो UTF-8 नहीं हैं; Netsuke को UTF-8 पथ चाहिए।
stdlib.which.command.empty = which को अरिक्त स्ट्रिंग चाहिए।
stdlib.which.cwd_mode.invalid = cwd_mode “auto”, “always” अथवा “never” होना चाहिए, किंतु “{ $mode }” मिला।
stdlib.which.cwd.resolve_failed = वर्तमान निर्देशिका निर्धारित नहीं की जा सकी: { $details }।
stdlib.which.cwd.non_utf8 = वर्तमान निर्देशिका में ऐसे अंश हैं जो UTF-8 नहीं हैं।
stdlib.which.canonicalize_failed = “{ $path }” का मानकीकरण नहीं हो सका: { $details }।
stdlib.which.is_executable = यह जाँचा नहीं जा सका कि “{ $path }” निष्पादनीय है या नहीं: { $details }।
stdlib.which.canonicalize_non_utf8 = मानक पथ में ऐसे अंश हैं जो UTF-8 नहीं हैं।
stdlib.which.workspace_non_utf8 = आदेश “{ $command }” को हल करते समय कार्यक्षेत्र के पथ में ऐसे अंश हैं जो UTF-8 नहीं हैं: { $path }।
stdlib.which.walkdir_error = आदेश हल करते समय कार्यक्षेत्र में भ्रमण के दौरान त्रुटि: { $details }।

# मानक पुस्तकालय का पंजीकरण।
stdlib.register.open_dir = stdlib के पंजीकरण हेतु वर्तमान निर्देशिका नहीं खोली जा सकी।
stdlib.register.resolve_dir = stdlib के पंजीकरण हेतु वर्तमान निर्देशिका निर्धारित नहीं की जा सकी।
stdlib.register.dir_non_utf8 = वर्तमान निर्देशिका में ऐसे अंश हैं जो UTF-8 नहीं हैं: { $path }।

# सुगम्य निर्गम ढंग के लिए स्थिति सूचना।
status.state.pending = प्रतीक्षारत
status.state.running = प्रगति पर
status.state.done = पूर्ण
status.state.failed = विफल
status.stage.label = चरण { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = कार्य { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = मैनिफ़ेस्ट फ़ाइल पढ़ी जा रही है
status.stage.initial_yaml_parsing = YAML दस्तावेज़ का विश्लेषण हो रहा है
status.stage.template_expansion = टेम्पलेट निर्देशों का विस्तार हो रहा है
status.stage.final_rendering = मैनिफ़ेस्ट के मानों का विक्रमांकन और प्रस्तुतीकरण हो रहा है
status.stage.ir_generation_validation = निर्भरता ग्राफ़ बनाया और जाँचा जा रहा है
status.stage.ninja_synthesis = Ninja की बिल्ड योजना बनाई जा रही है
status.stage.ninja_synthesis_execute = Ninja की योजना बनाकर { $tool } चलाया जा रहा है
status.stage.graph_rendering = ग्राफ़ का उत्पाद प्रस्तुत किया जा रहा है
status.stage.graph_rendering_with_tool = { $tool } प्रस्तुत किया जा रहा है
status.complete = { $tool }: पूर्ण।
status.timing.summary_header = चरणवार समय का सारांश:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = संपूर्ण शृंखला का कुल समय: { $duration }
status.tool.build = निर्माण
status.tool.clean = सफ़ाई
status.tool.graph = ग्राफ़
status.tool.graph_html = ग्राफ़ (HTML)
status.tool.generate = उत्पादन
status.tool.help_targets = लक्ष्य सहायता

# ग्राफ़ के HTML प्रस्तुतीकरण के पाठ।
graph.html.title = Netsuke का बिल्ड ग्राफ़
graph.html.heading = Netsuke का बिल्ड ग्राफ़
graph.html.description = Netsuke द्वारा प्रस्तुत बिल्ड ग्राफ़
graph.html.outline.summary = लक्ष्य और निर्भरताएँ (पाठ रूपरेखा)
graph.html.outline.no_inputs = कोई इनपुट नहीं
graph.html.noscript.notice = JavaScript निष्क्रिय है। ऊपर की पाठ रूपरेखा ही पूरा ग्राफ़ है; उसके बाद DOT स्रोत है।

# सुगम्य निर्गम के अर्थपूर्ण उपसर्ग।
semantic.prefix.error = त्रुटि:
semantic.prefix.warning = चेतावनी:
semantic.prefix.success = सफल:
semantic.prefix.info = सूचना:
semantic.prefix.timing = समय:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# अनुवादकों के लिए बहुवचन रूपों के उदाहरण।
# हिंदी में CLDR की दो श्रेणियाँ हैं: `one` (0 और 1 दोनों) तथा `other`।
# शून्य के लिए स्पष्ट `[0]` रूप CLDR की `one` श्रेणी से पहले चुना जाता है,
# ताकि "0 फ़ाइल" के बजाय स्वाभाविक वाक्य दिखे।
example.files_processed = { $count ->
    [0] कोई फ़ाइल संसाधित नहीं हुई।
    [one] { $count } फ़ाइल संसाधित हुई।
   *[other] { $count } फ़ाइलें संसाधित हुईं।
}

example.errors_found = { $count ->
    [0] कोई त्रुटि नहीं मिली।
    [one] { $count } त्रुटि मिली।
   *[other] { $count } त्रुटियाँ मिलीं।
}
