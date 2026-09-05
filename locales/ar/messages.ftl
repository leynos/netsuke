# موارد التوطين لواجهة سطر الأوامر في Netsuke.

runner.io.dyndep.retention = ‏ تعذّر تطبيق الاحتفاظ بـ dyndep المُنشأ أسفل ‏{ $path }.
cli.about = يصرّف Netsuke بيانات YAML + Jinja إلى خطط بناء بصيغة Ninja.
cli.long_about = يحوّل Netsuke بيانات YAML + Jinja إلى رسوم Ninja قابلة لإعادة الإنتاج، ثم ينفّذ Ninja بإعدادات افتراضية آمنة.
cli.usage = { $usage }

# نص المساعدة للخيارات العامة.
cli.flag.file.help = مسار ملف بيانات Netsuke المطلوب استخدامه.
cli.flag.directory.help = التنفيذ كما لو كان البدء في هذا الدليل.
cli.flag.config.help = مسار ملف إعدادات، مع تجاوز البحث التلقائي.
cli.flag.jobs.help = تحديد عدد مهام البناء المتوازية.
cli.flag.verbose.help = تفعيل سجلات التشخيص المفصّلة وملخّصات الزمن عند الانتهاء.
cli.flag.locale.help = وسم اللغة لنصوص سطر الأوامر (مثل: en-US أو ar).
cli.flag.fetch_allow_scheme.help = مخطّطات URL إضافية مسموح بها لمساعد fetch.
cli.flag.fetch_allow_host.help = أسماء المضيفين المسموح بها عند تفعيل الرفض الافتراضي.
cli.flag.fetch_block_host.help = أسماء المضيفين المحجوبة دائمًا، حتى إن سُمح بها في موضع آخر.
cli.flag.fetch_default_deny.help = رفض جميع المضيفين افتراضيًا؛ والسماح بالقائمة المعلنة فقط.
cli.flag.json.help = إخراج JSON قابل للقراءة آليًا.
cli.flag.no_input.help = عدم قراءة أي مدخلات تفاعلية أبدًا.
cli.flag.color.help = سياسة الإخراج الملوّن (auto أو always أو never).
cli.flag.emoji.help = سياسة الرموز التعبيرية (auto أو always أو never).
cli.flag.progress.help = سياسة عرض التقدّم (auto أو always أو never).
cli.flag.accessibility.help = سياسة الإخراج الميسّر (auto أو on أو off).
cli.flag.default_targets.help = أهداف البناء الافتراضية عند عدم تحديد أي هدف.

# أوصاف الأوامر الفرعية.
cli.subcommand.build.about = بناء الأهداف المعرّفة في ملف البيانات (الافتراضي).
cli.subcommand.build.long_about = بناء الأهداف المطلوبة؛ وإن لم يُحدَّد أي هدف تُستخدم أهداف ملف البيانات الافتراضية.
cli.subcommand.clean.about = إزالة مخرجات البناء عبر Ninja.
cli.subcommand.clean.long_about = إنشاء ملف Ninja مؤقت ثم تنفيذ `ninja -t clean`.
cli.subcommand.graph.about = إخراج رسم اعتماديات البناء. الصيغة الافتراضية هي DOT.
cli.subcommand.graph.long_about = إسقاط ملف بيانات Netsuke بعد تحليله إلى رسم بناء قياسي وكتابته بصيغة Graphviz DOT، أو صفحة HTML مكتفية بذاتها عند استخدام `--html`. استخدم `--output <ملف>` للكتابة إلى ملف؛ و`-` يكتب إلى المخرج القياسي.
cli.subcommand.generate.about = توليد ملف بيانات Ninja دون تنفيذ Ninja.
cli.subcommand.generate.long_about = كتابة ملف بيانات Ninja المولَّد إلى المخرج القياسي أو إلى ملف يُختار بـ `--output`.
cli.subcommand.help.about = اطبع التعليمات العامة، أو التعليمات لموضوع محدد.
cli.subcommand.help.long_about = بدون موضوع، يطابق هذا `--help`. استخدم `help targets` لطباعة كتالوج الأهداف والإجراءات للملف المحدد.

# Help catalogue headings and markers.
cli.help.actions_heading = الإجراءات:
cli.help.targets_heading = الأهداف:
cli.help.targets.about = سرد الأهداف والإجراءات في الملف المحدد.
cli.help.default_marker = الافتراضي
cli.help.conditional_marker = مشروط

# نص المساعدة لخيارات الأمر الفرعي build.
cli.subcommand.build.flag.targets.help = الأهداف المطلوب بناؤها (تُستخدم افتراضيات ملف البيانات عند الإغفال).

# نص المساعدة لخيارات الأمر الفرعي graph.
cli.subcommand.graph.flag.html.help = عرض الرسم كصفحة HTML مكتفية بذاتها بدلًا من صيغة DOT.
cli.subcommand.graph.flag.output.help = كتابة مخرج الرسم إلى ملف؛ استخدم `-` للمخرج القياسي.

# نص المساعدة لخيارات الأمر الفرعي generate.
cli.subcommand.generate.flag.output.help = كتابة ملف بيانات Ninja المولَّد إلى ملف بدلًا من المخرج القياسي.

# أخطاء التحقق في سطر الأوامر.
cli.validation.jobs.invalid_number = ‏{ $value } ليس عددًا صالحًا.
cli.validation.jobs.out_of_range = يجب أن يقع عدد المهام بين { $min } و{ $max }.
cli.validation.scheme.empty = يجب ألّا يكون المخطّط فارغًا.
cli.validation.scheme.invalid_start = يجب أن يبدأ المخطّط «{ $scheme }» بحرف من ASCII.
cli.validation.scheme.invalid = مخطّط غير صالح: «{ $scheme }».
cli.validation.locale.empty = يجب ألّا يكون وسم اللغة فارغًا.
cli.validation.locale.invalid = وسم لغة غير صالح: «{ $locale }».
cli.validation.color.invalid = سياسة ألوان غير صالحة: «{ $value }». القيم الصالحة: auto وalways وnever.
cli.validation.emoji.invalid = سياسة رموز تعبيرية غير صالحة: «{ $value }». القيم الصالحة: auto وalways وnever.
cli.validation.progress.invalid = سياسة تقدّم غير صالحة: «{ $value }». القيم الصالحة: auto وalways وnever.
cli.validation.accessibility.invalid = سياسة تيسير غير صالحة: «{ $value }». القيم الصالحة: auto وon وoff.
cli.validation.config.expected_object = كان يُنتظر أن تُسلسَل قيم سطر الأوامر إلى كائن، لكن ورد { $value }.

# رسائل الخطأ من Clap.
clap-error-missing-argument = مُعامل مطلوب مفقود: { $argument }
clap-error-missing-subcommand = الأمر الفرعي مفقود. الخيارات المتاحة: { $valid_subcommands }
clap-error-unknown-argument = مُعامل غير معروف: { $argument }
clap-error-invalid-value = قيمة غير صالحة للمُعامل { $argument }: { $value }
clap-error-invalid-subcommand = أمر فرعي غير معروف: { $subcommand }
# ملاحظة: صيغة value-validation تختلف عن invalid-value لتمييز إخفاقات
# المدقّقات المخصّصة (ErrorKind::ValueValidation) عن عدم تطابق الأنواع
# (ErrorKind::InvalidValue).
clap-error-value-validation = فشل التحقق من { $argument }: { $value }

# أخطاء التنفيذ وسياقه.
runner.manifest.not_found = تعذّر العثور على ملف البيانات «{ $manifest_name }» في { $directory }.
runner.manifest.not_found.help = تأكّد من وجود ملف البيانات أو مرّر `--file` مع المسار الصحيح.
runner.manifest.path_missing_name = مسار ملف البيانات «{ $path }» لا يتضمّن اسم ملف.
cli.file.non_utf8 = مسار ملف البيانات «{ $path }» ليس UTF-8 صالحًا.
runner.manifest.directory_label = الدليل `{ $directory }`
runner.manifest.current_directory_label = الدليل الحالي
runner.manifest.default_not_declared = الافتراضي للبيان «{ $default }» لا يسمّي إجراءً أو هدفًا معلنًا.
runner.context.network_policy = تعذّر بناء سياسة الشبكة.
runner.context.load_manifest = تعذّر تحميل ملف البيانات من { $path }.
runner.context.serialise_manifest = تعذّرت سَلسَلة ملف البيانات.
runner.context.build_graph = تعذّر بناء الرسم من ملف البيانات.
runner.context.generate_ninja = تعذّر توليد ملف بيانات Ninja.
runner.context.render_graph = تعذّر عرض مخرج الرسم.

runner.io.create_temp_file = تعذّر إنشاء ملف Ninja المؤقت.
runner.io.write_temp_ninja = تعذّرت الكتابة إلى ملف Ninja المؤقت.
runner.io.flush_temp_ninja = تعذّر إفراغ ذاكرة ملف Ninja المؤقت.
runner.io.sync_temp_ninja = تعذّرت مزامنة ملف Ninja المؤقت.
runner.io.create_parent_dir = تعذّر إنشاء الدليل الأصل { $path }.
runner.io.create_ninja_file = تعذّر إنشاء ملف Ninja في { $path }.
runner.io.write_ninja_file = تعذّرت الكتابة إلى ملف Ninja في { $path }.
runner.io.flush_ninja_file = تعذّر إفراغ ذاكرة ملف Ninja في { $path }.
runner.io.sync_ninja_file = تعذّرت مزامنة ملف Ninja في { $path }.
runner.io.open_ambient_dir = تعذّر فتح الدليل المحيط.
cli.directory.non_utf8 = ‏ مسار دليل العمل غير صالح بترميز UTF-8. ({ $path })
runner.io.no_existing_ancestor = لا يوجد دليل أعلى قائم للمسار { $path }.
runner.io.derive_relative_path = تعذّر اشتقاق مسار Ninja النسبي.
runner.io.non_utf8_path = المسارات غير المرمّزة بـ UTF-8 غير مدعومة (المسار: { $path }).
runner.io.write_stdout = تعذّرت كتابة ملف بيانات Ninja إلى المخرج القياسي.
runner.io.flush_stdout = تعذّر إفراغ ذاكرة المخرج القياسي.
runner.io.dyndep.create_dir = ‏ تعذّر إنشاء دليل dyndep ‏{ $path }.
runner.io.dyndep.read = ‏ تعذّرت قراءة ملف dyndep المُنشأ في ‏{ $path }.
runner.io.dyndep.write = ‏ تعذّرت كتابة ملف dyndep المُنشأ في ‏{ $path }.
runner.io.dyndep.rename = ‏ تعذّر إنهاء ملف dyndep المُنشأ في ‏{ $path }.
runner.io.dyndep.corrupt = ‏ ملف dyndep المُنشأ في ‏{ $path } لا يطابق المحتوى المتوقع؛ أزل ذلك الملف وحده وأعد المحاولة.
runner.io.dyndep.temp_collisions = ‏ تعذّر إنشاء ملف dyndep مؤقت وفريد لـ ‏{ $path } بعد اصطدامات متكرّرة في الأسماء.
runner.io.dyndep.too_large = ‏ يتجاوز ملف dyndep المُنشأ في ‏{ $path } حدّ التحقّق البالغ ‏{ $limit } بايت.

# تشخيصات ملف البيانات.
manifest.parse = فشل تحليل ملف البيانات.
manifest.structure_error = خطأ في بنية ملف البيانات عند { $name }: { $details }
manifest.yaml.parse = خطأ في تحليل YAML في السطر { $line } والعمود { $column }: { $details }
manifest.yaml.label = ‏YAML غير صالح
manifest.yaml.hint.tabs = لا يسمح YAML بمحارف الجدولة؛ استخدم المسافات في الإزاحة.
manifest.yaml.hint.list_item = يجب أن تبدأ عناصر قوائم YAML بـ «-» وأن تكون مُزاحة بشكل صحيح.
manifest.yaml.hint.expected_colon = يبدو هذا مدخلًا في تخطيط؛ ينقص «:» بعد المفتاح.
manifest.yaml.hint.mapping_values = تتطلّب تخطيطات YAML قيمة بعد «:» (أو كتلة متداخلة).
manifest.yaml.hint.invalid_token = رمز YAML غير صالح أو غير متوقّع.
manifest.yaml.hint.escape = هرّب الشرطات المائلة العكسية أو احذف تسلسلات التهريب غير الصالحة.
manifest.env.missing = متغيّر بيئة مطلوب غير مضبوط.
manifest.env.invalid_utf8 = يتضمّن متغيّر بيئة ترميز UTF-8 غير صالح.
manifest.vars.not_object = يجب أن يكون `vars` في ملف البيانات تخطيطًا أو كائنًا.
manifest.vars.reserved_name = يُعدّ مفتاح `vars` المسمّى '{ $name }' في ملف البيانات محجوزًا لدالة قوالب مدمجة؛ أعد تسمية المتغيّر.
manifest.read_failed = تعذّرت قراءة ملف البيانات من { $path }.
manifest.resolve_workspace_root = تعذّر تحديد جذر مساحة العمل.
manifest.workspace_non_utf8 = مسار جذر مساحة العمل «{ $path }» ليس UTF-8 صالحًا.
manifest.path_non_utf8 = مسار ملف البيانات «{ $manifest }» ليس UTF-8 صالحًا: { $path }.
manifest.path_missing_name = مسار ملف البيانات «{ $path }» لا يتضمّن اسم ملف.
manifest.open_workspace_failed = تعذّر فتح مساحة العمل { $workspace } لأجل ملف البيانات { $manifest }.
manifest.foreach.not_iterable = تعبير `foreach` غير قابل للتكرار.
manifest.foreach.serialise_item = تعذّرت سَلسَلة عنصر `foreach`.
manifest.when.empty = يجب ألّا يكون تعبير `when` فارغًا.
manifest.when.eval_error = تعذّر تقييم تعبير `when` «{ $expr }».
manifest.when.template_error = تعذّر عرض قالب `when` «{ $expr }».
manifest.target.vars_not_object = يجب أن يكون `vars` الخاص بالهدف كائنًا، لكن ورد { $value }.
manifest.vars.entry_not_object = يجب أن يكون مدخل `vars` في ملف البيانات كائنًا.
manifest.field_not_string = يجب أن يكون الحقل «{ $field }» سلسلة نصية.
manifest.expression.parse_error = تعذّر تحليل تعبير { $name }.
manifest.expression.eval_error = تعذّر تقييم تعبير { $name }.

# تشخيصات ماكروهات ملف البيانات.
manifest.macro.signature_missing_identifier = ينقص توقيع الماكرو مُعرّفًا.
manifest.macro.signature_missing_params = تنقص توقيع الماكرو مُعاملات.
manifest.macro.compile_failed = تعذّر تصريف الماكرو { $name }.
manifest.macro.sequence_invalid = يجب تعريف الماكروهات كتخطيط من الأسماء إلى القوالب.
manifest.macro.register_failed = تعذّر تسجيل ماكروهات ملف البيانات.
manifest.macro.not_initialised = بيئة الماكروهات غير مهيّأة.
manifest.macro.caller_invalid = يجب أن يكون مُستدعي الماكرو سلسلة نصية.
manifest.macro.template_load_failed = تعذّر تحميل قالب الماكرو.
manifest.macro.init_failed = تعذّرت تهيئة بيئة الماكروهات.
manifest.macro.missing = الماكرو { $name } مفقود.

# أخطاء أنماط glob في ملف البيانات.
manifest.glob.unmatched_brace = نمط glob غير صالح «{ $pattern }»: المحرف «{ $character }» بلا مقابل في الموضع { $position }.
manifest.glob.invalid_pattern = نمط glob غير صالح «{ $pattern }»: { $detail }.
manifest.glob.unknown_pattern_error = خطأ نمط غير معروف.
manifest.glob.io_failed = فشل glob للنمط «{ $pattern }»: { $detail }.
manifest.glob.unknown_io_error = خطأ إدخال/إخراج غير معروف.
manifest.command_list_empty = يجب ألّا يكون الحقل «command» فارغًا: قدِّم سلسلة أمر أو قائمة غير فارغة.

# أخطاء التمثيل الوسيط.
ir.rule_not_found = تعذّر العثور على القاعدة «{ $rule }» التي يشير إليها الهدف «{ $target }».
ir.multiple_rules = يجب أن يشير الهدف «{ $target }» إلى قاعدة واحدة فقط، لكن ورد { $rules }.
ir.empty_rule = يجب أن يشير الهدف «{ $target }» إلى قاعدة.
ir.duplicate_outputs = رُصدت مخرجات مكرّرة: { $outputs }.
ir.circular_dependency = رُصد اعتماد دائري: { $cycle }.
ir.action_serialisation = تعذّرت سَلسَلة الإجراء: { $details }.
ir.invalid_command = إقحام غير صالح داخل الأمر: { $snippet }.

# أخطاء توليد ملفات Ninja.
ninja_gen.missing_action = الإجراء «{ $id }» الذي تشير إليه حافة بناء مفقود.
ninja_gen.format = تعذّر تنسيق مخرجات ملف بيانات Ninja.
ninja_gen.dyndep_files_required = ‏ يتطلّب هذا البناء حزمة Ninja مُنشأة؛ استخدم `netsuke build` أو `netsuke clean` أو `netsuke generate` لتجهيز ملفات dyndep.
ninja_gen.reserved_output_path = ‏ المسار '{ $path }' محجوز لحالة تبعيات Netsuke التسلسلية.
ninja_gen.unsupported_path_character = ‏ يحتوي المسار '{ $path }' على محرف مسار Ninja غير مدعوم هو '{ $character }'.

# التحقق من أنماط المضيفين.
host_pattern.empty = يجب ألّا يكون نمط المضيف فارغًا.
host_pattern.contains_scheme = يجب ألّا يتضمّن نمط المضيف «{ $pattern }» مخطّط URL.
host_pattern.contains_slash = يجب ألّا يتضمّن نمط المضيف «{ $pattern }» المحرف «/».
host_pattern.missing_suffix = يجب أن يتضمّن نمط المضيف «{ $pattern }» لاحقة بعد «*.».
host_pattern.empty_label = يتضمّن نمط المضيف «{ $pattern }» تسمية فارغة.
host_pattern.invalid_chars = يتضمّن نمط المضيف «{ $pattern }» محارف غير صالحة.
host_pattern.invalid_label_edge = يجب ألّا تبدأ تسميات نمط المضيف «{ $pattern }» بـ «-» أو تنتهي به.
host_pattern.label_too_long = يتضمّن نمط المضيف «{ $pattern }» تسمية تتجاوز 63 محرفًا.
host_pattern.too_long = يتجاوز نمط المضيف «{ $pattern }» حدّ 255 محرفًا.

# سياسة الشبكة.
network_policy.scheme.empty = يجب ألّا يكون المخطّط فارغًا.
network_policy.scheme.invalid = يتضمّن المخطّط «{ $scheme }» محارف غير صالحة.
network_policy.allowlist.empty = يجب ألّا تكون قائمة المضيفين المسموح بهم فارغة.
network_policy.scheme.not_allowed = المخطّط «{ $scheme }» غير مسموح به.
network_policy.missing_host = لا يتضمّن العنوان URL مضيفًا.
network_policy.host.blocked = المضيف «{ $host }» محجوب بموجب السياسة.
network_policy.host.not_allowlisted = المضيف «{ $host }» ليس ضمن قائمة المسموح بهم.

# إعدادات المكتبة القياسية.
stdlib.config.default_fetch_cache_invalid = يجب أن يكون المسار الافتراضي لذاكرة fetch المخبّأة نسبيًا.
stdlib.config.default_which_cache_invalid = يجب أن تكون السعة الافتراضية لذاكرة which المخبّأة موجبة.
stdlib.config.workspace_root_absolute = يجب أن يكون مسار جذر مساحة العمل مطلقًا.
stdlib.config.fetch_response_limit_positive = يجب أن يكون حدّ استجابة fetch موجبًا.
stdlib.config.command_output_limit_positive = يجب أن يكون حدّ التقاط مخرجات الأوامر موجبًا.
stdlib.config.command_stream_limit_positive = يجب أن يكون حدّ تدفّق الأوامر موجبًا.
stdlib.config.which_cache_capacity_positive = يجب أن تكون سعة ذاكرة which المخبّأة موجبة.
stdlib.config.skip_dir_empty = يجب ألّا تكون مداخل الأدلة المتجاوَزة فارغة.
stdlib.config.skip_dir_navigation = يجب ألّا تتضمّن مداخل الأدلة المتجاوَزة «..».
stdlib.config.skip_dir_separator = يجب ألّا تتضمّن مداخل الأدلة المتجاوَزة فواصل مسار.
stdlib.config.fetch_cache_empty = يجب ألّا يكون مسار ذاكرة fetch المخبّأة فارغًا.
stdlib.config.fetch_cache_not_relative = يجب أن يكون مسار ذاكرة fetch المخبّأة نسبيًا، لكن ورد { $path }.
stdlib.config.fetch_cache_escapes = يجب ألّا يخرج مسار ذاكرة fetch المخبّأة عن مساحة العمل: { $path }.
stdlib.config.open_workspace_root = تعذّر فتح الدليل الحالي بوصفه جذر مساحة عمل stdlib.
stdlib.config.resolve_cwd = تعذّر تحديد الدليل الحالي بوصفه جذر مساحة عمل stdlib.
stdlib.config.cwd_non_utf8 = يتضمّن الدليل الحالي أجزاءً ليست UTF-8: { $path }.

# تشخيصات مساعد fetch.
stdlib.fetch.url_invalid = عنوان URL غير صالح «{ $url }»: { $details }.
stdlib.fetch.disallowed = العنوان URL «{ $url }» غير مسموح به: { $details }.
stdlib.fetch.failed = تعذّر جلب «{ $url }»: { $details }.
stdlib.fetch.redirect_loop = ‏Redirect loop detected at '{ $url }'.
stdlib.fetch.redirect_limit_exceeded = ‏Redirect limit of { $limit } exceeded while fetching '{ $url }'.
stdlib.fetch.redirect_location_invalid = ‏Invalid redirect location '{ $location }' from '{ $url }': { $details }.
stdlib.fetch.redirect_disallowed = ‏Redirect URL '{ $url }' is disallowed: { $details }.
stdlib.fetch.redirect_location_missing = ‏Redirect response from '{ $url }' did not include a Location header.
stdlib.fetch.cache_read_failed = تعذّرت قراءة مدخل الذاكرة المخبّأة «{ $name }»: { $details }.
stdlib.fetch.cache_open_failed = تعذّر فتح مدخل الذاكرة المخبّأة «{ $name }»: { $details }.
stdlib.fetch.response_read_failed = تعذّرت قراءة الاستجابة من «{ $url }»: { $details }.
stdlib.fetch.response_buffer_overflow = فاض المخزن المؤقت أثناء قراءة «{ $url }».
stdlib.fetch.cache_write_failed = تعذّرت كتابة الذاكرة المخبّأة لـ «{ $url }»: { $details }.
stdlib.fetch.response_limit_exceeded = تجاوزت الاستجابة من «{ $url }» حدّ { $limit } بايت.
stdlib.fetch.cache_limit_exceeded = تجاوزت الاستجابة المخبّأة «{ $name }» حدّ { $limit } بايت.
stdlib.fetch.io_failed = فشل الإجراء «{ $action }» على { $path }: { $details }.
stdlib.fetch.action.sync_cache = مزامنة ذاكرة fetch المخبّأة
stdlib.fetch.action.create_cache_dir = إنشاء دليل ذاكرة fetch المخبّأة
stdlib.fetch.action.open_cache_dir = فتح دليل ذاكرة fetch المخبّأة
stdlib.fetch.action.stat_cache = قراءة بيانات مدخل ذاكرة fetch المخبّأة
stdlib.fetch.action.open_cache_entry = فتح مدخل ذاكرة fetch المخبّأة

# تشخيصات مساعد الأوامر.
stdlib.command.location = الأمر «{ $command }» في القالب «{ $template }»
stdlib.command.spawn_failed = تعذّر تشغيل { $location }: { $details }.
stdlib.command.io_failed = فشل { $location }: { $details }.
stdlib.command.closed_input_early = أُغلق المدخل قبل اكتمال الكتابة إلى الأمر.
stdlib.command.broken_pipe = انقطعت الأنبوبة أثناء تنفيذ { $location }: { $details }.
stdlib.command.terminated_by_signal = أُنهي { $location } بإشارة.
stdlib.command.exited_with_status = انتهى { $location } بالحالة { $status }.
stdlib.command.output_limit_exceeded = تجاوز { $location } حدّ { $mode } البالغ { $limit } بايت للتدفّق { $stream }.
stdlib.command.timeout = تجاوز { $location } المهلة البالغة { $seconds } ثانية.
stdlib.command.exit_status_suffix = ‏(حالة الخروج { $status })
stdlib.command.signal_suffix = ‏(أُنهي بإشارة)
stdlib.command.shell.empty = يجب ألّا يكون أمر الصدفة فارغًا.
stdlib.command.grep.empty_pattern = يجب ألّا يكون نمط grep فارغًا.
stdlib.command.grep.flags_not_string = يجب أن تكون رايات grep سلاسل نصية.
stdlib.command.quote.invalid = تعذّر وضع { $arg } بين علامتي اقتباس: { $details }.
stdlib.command.quote.line_break = لا يمكن وضع المعاملات التي تتضمّن إرجاع أوّل السطر أو تغذية السطر بين علامتي اقتباس بأمان.
stdlib.command.input_undefined = قيمة المدخل غير معرّفة.
stdlib.command.tempfile.root_required = يلزم جذر مساحة العمل لإنشاء ملفات الأوامر المؤقتة.
stdlib.command.tempfile.create_failed = تعذّر إنشاء الملف المؤقت للأمر: { $details }.
stdlib.command.options.invalid_utf8 = يجب أن يكون مفتاح خيار الأمر بترميز UTF-8 صالح.
stdlib.command.option.mode_not_string = يجب أن يكون وضع الإخراج سلسلة نصية.
stdlib.command.options.invalid_type = يجب أن تكون خيارات الأمر كائنًا.
stdlib.command.output.mode_unsupported = وضع إخراج غير مدعوم: «{ $mode }».
stdlib.command.output.mode.capture = الالتقاط
stdlib.command.output.mode.streaming = التدفّق
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# تشخيصات مساعد المسارات.
stdlib.path.io.failed = فشل الإجراء «{ $action }» على { $path } ({ $label }).
stdlib.path.io.failed_with_detail = فشل الإجراء «{ $action }» على { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = فشل الإجراء «{ $action }» على { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = غير موجود
stdlib.path.io.permission_denied = رُفض الإذن
stdlib.path.io.already_exists = موجود سلفًا
stdlib.path.io.invalid_input = مدخل غير صالح
stdlib.path.io.invalid_data = بيانات غير صالحة
stdlib.path.io.timed_out = انتهت المهلة
stdlib.path.io.interrupted = قوطع
stdlib.path.io.would_block = سيؤدي إلى التعليق
stdlib.path.io.write_zero = كُتبت صفر بايت
stdlib.path.io.unexpected_eof = نهاية ملف غير متوقّعة
stdlib.path.io.broken_pipe = انقطاع الأنبوبة
stdlib.path.io.connection_refused = رُفض الاتصال
stdlib.path.io.connection_reset = أُعيد ضبط الاتصال
stdlib.path.io.connection_aborted = أُجهض الاتصال
stdlib.path.io.not_connected = غير متّصل
stdlib.path.io.addr_in_use = العنوان قيد الاستخدام
stdlib.path.io.addr_not_available = العنوان غير متاح
stdlib.path.io.out_of_memory = نفدت الذاكرة
stdlib.path.io.unsupported = غير مدعوم
stdlib.path.io.file_too_large = الملف أكبر من اللازم
stdlib.path.io.resource_busy = المورد مشغول
stdlib.path.io.executable_busy = الملف التنفيذي مشغول
stdlib.path.io.deadlock = تعطّل متبادل
stdlib.path.io.crosses_devices = يعبر حدود الأجهزة
stdlib.path.io.too_many_links = روابط أكثر من اللازم
stdlib.path.io.invalid_filename = اسم ملف غير صالح
stdlib.path.io.arg_list_too_long = قائمة المعاملات أطول من اللازم
stdlib.path.io.stale_handle = مقبض ملف شبكي قديم
stdlib.path.io.storage_full = مساحة التخزين ممتلئة
stdlib.path.io.not_seekable = لا يقبل تحديد الموضع
stdlib.path.io.network_down = الشبكة متوقّفة
stdlib.path.io.network_unreachable = تعذّر الوصول إلى الشبكة
stdlib.path.io.host_unreachable = تعذّر الوصول إلى المضيف
stdlib.path.io.other = خطأ إدخال/إخراج
stdlib.path.action.canonicalize = التقييس
stdlib.path.action.open_directory = فتح الدليل
stdlib.path.action.stat = قراءة البيانات
stdlib.path.action.read = القراءة
stdlib.path.action.open_file = فتح الملف
stdlib.path.with_suffix.empty_separator = يتطلّب with_suffix فاصلًا غير فارغ.
stdlib.path.relative_to.mismatch = المسار { $path } ليس نسبيًا إلى { $root }.
stdlib.path.expanduser.unsupported = توسيع ~ لمستخدم بعينه غير مدعوم.
stdlib.path.expanduser.no_home = تعذّر توسيع ~: لم يُضبط أي متغيّر بيئة لدليل المنزل.
stdlib.path.contents.unsupported_encoding = ترميز غير مدعوم: «{ $encoding }».
stdlib.path.hash.unsupported_algorithm = خوارزمية تلبيد غير مدعومة: «{ $algorithm }».
stdlib.path.hash.unsupported_algorithm_legacy = خوارزمية تلبيد غير مدعومة: «{ $algorithm }» (فعّل الميزة «{ $feature }»).

# تشخيصات مساعدات المجموعات.
stdlib.collections.flatten.expected_sequence = توقّع flatten عناصر متتالية لكنه وجد { $kind }.
stdlib.collections.group_by.empty_attribute = يتطلّب group_by سمة غير فارغة.
stdlib.collections.group_by.unresolved = تعذّر على group_by إيجاد «{ $attr }» في عنصر من النوع { $kind }.

# تشخيصات مساعدات الزمن.
stdlib.time.offset.invalid = إزاحة now «{ $offset }» غير صالحة: المتوقّع «+HH:MM[:SS]» أو «Z».
stdlib.time.timedelta.overflow = فاض timedelta عند إضافة { $component }.
stdlib.time.label.weeks = أسابيع
stdlib.time.label.days = أيام
stdlib.time.label.hours = ساعات
stdlib.time.label.minutes = دقائق
stdlib.time.label.seconds = ثوانٍ
stdlib.time.label.milliseconds = أجزاء من الألف من الثانية
stdlib.time.label.microseconds = أجزاء من المليون من الثانية
stdlib.time.label.nanoseconds = أجزاء من المليار من الثانية

# تشخيصات مساعد which.
stdlib.which.not_found = ‏[netsuke::jinja::which::not_found] تعذّر العثور على الأمر «{ $command }» بعد فحص { $count } من مداخل PATH. معاينة: { $preview }
stdlib.which.not_found.hint.cwd_auto = تُتجاهل الأجزاء الفارغة من PATH؛ استخدم cwd_mode="auto" لتضمين دليل العمل.
stdlib.which.not_found.hint.cwd_always = اضبط cwd_mode="always" لتضمين الدليل الحالي.
stdlib.which.direct_not_found = ‏[netsuke::jinja::which::not_found] الأمر «{ $command }» في «{ $path }» غير موجود أو غير قابل للتنفيذ.
stdlib.which.args_error = ‏[netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = ‏<فارغ>
stdlib.which.path_entry.non_utf8 = يتضمّن المدخل رقم { $index } في PATH محارف ليست UTF-8؛ ويتطلّب Netsuke مسارات بترميز UTF-8.
stdlib.which.command.empty = يتطلّب which سلسلة نصية غير فارغة.
stdlib.which.cwd_mode.invalid = يجب أن تكون قيمة cwd_mode إحدى «auto» أو «always» أو «never»، لكن ورد «{ $mode }».
stdlib.which.cwd.resolve_failed = تعذّر تحديد الدليل الحالي: { $details }.
stdlib.which.cwd.non_utf8 = يتضمّن الدليل الحالي أجزاءً ليست UTF-8.
stdlib.which.canonicalize_failed = تعذّر تقييس «{ $path }»: { $details }.
stdlib.which.is_executable = تعذّر التحقق ممّا إذا كان «{ $path }» قابلًا للتنفيذ: { $details }.
stdlib.which.canonicalize_non_utf8 = يتضمّن المسار المقيَّس أجزاءً ليست UTF-8.
stdlib.which.workspace_non_utf8 = يتضمّن مسار مساحة العمل أجزاءً ليست UTF-8 أثناء تحديد الأمر «{ $command }»: { $path }.
stdlib.which.walkdir_error = خطأ أثناء اجتياز مساحة العمل بحثًا عن الأمر: { $details }.

# تسجيل المكتبة القياسية.
stdlib.register.open_dir = تعذّر فتح الدليل الحالي لتسجيل stdlib.
stdlib.register.resolve_dir = تعذّر تحديد الدليل الحالي لتسجيل stdlib.
stdlib.register.dir_non_utf8 = يتضمّن الدليل الحالي أجزاءً ليست UTF-8: { $path }.

# تقارير الحالة لوضع الإخراج الميسّر.
status.state.pending = في الانتظار
status.state.running = قيد التنفيذ
status.state.done = مكتملة
status.state.failed = فاشلة
status.stage.label = المرحلة { $current }/{ $total }: { $description }
status.stage.summary = ‏[{ $state }] { $label }
status.stage.summary_with_task = ‏[{ $state }] { $label } ({ $task_progress })
status.task.progress_label = المهمة { $current }/{ $total }
status.task.progress_update = ‏{ $task }: { $description }
status.stage.manifest_ingestion = قراءة ملف البيانات
status.stage.initial_yaml_parsing = تحليل مستند YAML
status.stage.template_expansion = توسيع توجيهات القوالب
status.stage.final_rendering = فكّ سَلسَلة قيم ملف البيانات وعرضها
status.stage.ir_generation_validation = بناء رسم الاعتماديات والتحقق منه
status.stage.ninja_synthesis = تركيب خطة بناء Ninja
status.stage.ninja_synthesis_execute = تركيب خطة Ninja وتنفيذ { $tool }
status.stage.graph_rendering = عرض مخرج الرسم
status.stage.graph_rendering_with_tool = عرض { $tool }
status.complete = اكتمل { $tool }.
status.timing.summary_header = ملخّص الزمن حسب المرحلة:
status.timing.stage_line = ‏- { $label }: { $duration }
status.timing.total_line = الزمن الكلي لسلسلة المعالجة: { $duration }
status.tool.build = البناء
status.tool.clean = التنظيف
status.tool.graph = الرسم
status.tool.graph_html = الرسم (HTML)
status.tool.generate = التوليد
status.tool.help_targets = فهرس الأهداف

# نصوص عرض الرسم بصيغة HTML.
graph.html.title = رسم بناء Netsuke
graph.html.heading = رسم بناء Netsuke
graph.html.description = رسم بناء عرضه Netsuke
graph.html.outline.summary = الأهداف والاعتماديات (مخطّط نصي)
graph.html.outline.no_inputs = لا توجد مدخلات
graph.html.noscript.notice = ‏JavaScript معطّلة. المخطّط النصي أعلاه هو الرسم كاملًا، ويليه مصدر DOT.

# البادئات الدلالية للإخراج الميسّر.
semantic.prefix.error = خطأ:
semantic.prefix.warning = تحذير:
semantic.prefix.success = نجاح:
semantic.prefix.info = معلومة:
semantic.prefix.timing = الزمن:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# أمثلة على صيغ الجمع للمترجمين.
# تستخدم العربية فئات CLDR الست: `zero` و`one` و`two` و`few` (3–10)
# و`many` (11–99) و`other`، ويتغيّر تمييز العدد بينها.
example.files_processed = { $count ->
    [zero] لم تُعالَج أي ملفات.
    [one] عولج ملف واحد.
    [two] عولج ملفان.
    [few] عولجت { $count } ملفات.
    [many] عولج { $count } ملفًا.
   *[other] عولج { $count } ملف.
}

example.errors_found = { $count ->
    [0] لم يُعثر على أي أخطاء.
    [one] عُثر على خطأ واحد.
    [two] عُثر على خطأين.
    [few] عُثر على { $count } أخطاء.
    [many] عُثر على { $count } خطأً.
   *[other] عُثر على { $count } خطأ.
}
