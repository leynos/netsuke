# منابع بومی‌سازی خط فرمان Netsuke.

cli.about = ‏Netsuke مانیفست‌های YAML + Jinja را به طرح‌های ساخت Ninja ترجمه می‌کند.
cli.long_about = ‏Netsuke مانیفست‌های YAML + Jinja را به گراف‌های تکرارپذیر Ninja تبدیل می‌کند و Ninja را با پیش‌فرض‌های ایمن اجرا می‌کند.
cli.usage = { $usage }

# متن راهنمای گزینه‌های عمومی.
cli.flag.file.help = مسیر پرونده مانیفست Netsuke که باید به کار رود.
cli.flag.directory.help = چنان اجرا کن که گویی در این شاخه آغاز شده است.
cli.flag.config.help = مسیر یک پرونده پیکربندی، با نادیده‌گرفتن جست‌وجوی خودکار.
cli.flag.jobs.help = تعیین شمار کارهای موازی ساخت.
cli.flag.verbose.help = فعال‌کردن گزارش تشخیصی مفصل و خلاصهٔ زمان در پایان کار.
cli.flag.locale.help = برچسب زبان برای متن‌های خط فرمان (برای نمونه: en-US یا fa).
cli.flag.fetch_allow_scheme.help = طرح‌های URL افزوده که برای یاور fetch مجازند.
cli.flag.fetch_allow_host.help = نام میزبان‌هایی که هنگام فعال‌بودن ردّ پیش‌فرض مجازند.
cli.flag.fetch_block_host.help = نام میزبان‌هایی که همیشه مسدودند، حتی اگر جای دیگری مجاز باشند.
cli.flag.fetch_default_deny.help = ردّ همهٔ میزبان‌ها به‌صورت پیش‌فرض؛ تنها فهرست اعلام‌شده مجاز است.
cli.flag.json.help = خروجی JSON خوانا برای ماشین تولید کن.
cli.flag.no_input.help = هرگز ورودی تعاملی نخوان.
cli.flag.color.help = سیاست خروجی رنگی (auto، always، never).
cli.flag.emoji.help = سیاست ایموجی (auto، always، never).
cli.flag.progress.help = سیاست نمایش پیشرفت (auto، always، never).
cli.flag.accessibility.help = سیاست خروجی دسترس‌پذیر (auto، on، off).
cli.flag.default_targets.help = هدف‌های پیش‌فرض ساخت هنگامی که هدفی مشخص نشده باشد.

# شرح زیرفرمان‌ها.
cli.subcommand.build.about = ساخت هدف‌های تعریف‌شده در مانیفست (پیش‌فرض).
cli.subcommand.build.long_about = ساخت هدف‌های خواسته‌شده؛ اگر هدفی داده نشود، هدف‌های پیش‌فرض مانیفست به کار می‌روند.
cli.subcommand.clean.about = حذف فرآورده‌های ساخت از راه Ninja.
cli.subcommand.clean.long_about = ساختن یک پروندهٔ موقت Ninja و سپس اجرای `ninja -t clean`.
cli.subcommand.graph.about = چاپ گراف وابستگی‌های ساخت. قالب پیش‌فرض DOT است.
cli.subcommand.graph.long_about = تصویرکردن مانیفست تجزیه‌شدهٔ Netsuke به یک گراف ساخت متعارف و نوشتن آن به شکل Graphviz DOT، یا با `--html` به شکل یک صفحهٔ HTML خودبسنده. برای نوشتن در پرونده از `--output <پرونده>` استفاده کنید؛ `-` در خروجی استاندارد می‌نویسد.
cli.subcommand.generate.about = تولید مانیفست Ninja بدون اجرای Ninja.
cli.subcommand.generate.long_about = نوشتن مانیفست Ninja تولیدشده در خروجی استاندارد یا در پرونده‌ای که با `--output` برگزیده می‌شود.
cli.subcommand.help.about = راهنمای سطح بالا یا راهنمای یک موضوع مشخص را چاپ کنید.
cli.subcommand.help.long_about = بدون موضوع، این با `--help` یکسان است. از `help targets` برای چاپ فهرست اهداف و اقدامات پرونده انتخاب‌شده استفاده کنید.

# Help catalogue headings and markers.
cli.help.actions_heading = اقدامات:
cli.help.targets_heading = اهداف:
cli.help.default_marker = پیش‌فرض

# متن راهنمای گزینه‌های زیرفرمان build.
cli.subcommand.build.flag.targets.help = هدف‌هایی که باید ساخته شوند (در صورت نیامدن، پیش‌فرض‌های مانیفست به کار می‌روند).

# متن راهنمای گزینه‌های زیرفرمان graph.
cli.subcommand.graph.flag.html.help = نمایش گراف به شکل صفحهٔ HTML خودبسنده به‌جای قالب DOT.
cli.subcommand.graph.flag.output.help = نوشتن فرآوردهٔ گراف در پرونده؛ برای خروجی استاندارد از `-` استفاده کنید.

# متن راهنمای گزینه‌های زیرفرمان generate.
cli.subcommand.generate.flag.output.help = نوشتن مانیفست Ninja تولیدشده در پرونده به‌جای خروجی استاندارد.

# خطاهای اعتبارسنجی خط فرمان.
cli.validation.jobs.invalid_number = ‏{ $value } عدد معتبری نیست.
cli.validation.jobs.out_of_range = شمار کارها باید میان { $min } و { $max } باشد.
cli.validation.scheme.empty = طرح نباید تهی باشد.
cli.validation.scheme.invalid_start = طرح «{ $scheme }» باید با یک حرف ASCII آغاز شود.
cli.validation.scheme.invalid = طرح نامعتبر: «{ $scheme }».
cli.validation.locale.empty = برچسب زبان نباید تهی باشد.
cli.validation.locale.invalid = برچسب زبان نامعتبر: «{ $locale }».
cli.validation.color.invalid = سیاست رنگ نامعتبر: «{ $value }». مقادیر معتبر: auto، always، never.
cli.validation.emoji.invalid = سیاست ایموجی نامعتبر: «{ $value }». مقادیر معتبر: auto، always، never.
cli.validation.progress.invalid = سیاست پیشرفت نامعتبر: «{ $value }». مقادیر معتبر: auto، always، never.
cli.validation.accessibility.invalid = سیاست دسترس‌پذیری نامعتبر: «{ $value }». مقادیر معتبر: auto، on، off.
cli.validation.config.expected_object = انتظار می‌رفت مقادیر خط فرمان به یک شیء تبدیل شوند، اما { $value } به دست آمد.

# پیام‌های خطای Clap.
clap-error-missing-argument = آرگومان الزامی ارائه نشده است: { $argument }
clap-error-missing-subcommand = زیرفرمان وجود ندارد. گزینه‌های در دسترس: { $valid_subcommands }
clap-error-unknown-argument = آرگومان ناشناخته: { $argument }
clap-error-invalid-value = مقدار نامعتبر برای { $argument }: { $value }
clap-error-invalid-subcommand = زیرفرمان ناشناخته: { $subcommand }
# یادداشت: عبارت value-validation از invalid-value متمایز است تا خطای
# اعتبارسنج‌های سفارشی (ErrorKind::ValueValidation) از ناسازگاری نوع
# (ErrorKind::InvalidValue) بازشناخته شود.
clap-error-value-validation = اعتبارسنجی { $argument } ناکام ماند: { $value }

# خطاها و بافتار زمان اجرا.
runner.manifest.not_found = مانیفست «{ $manifest_name }» در { $directory } یافت نشد.
runner.manifest.not_found.help = از وجود مانیفست مطمئن شوید یا `--file` را با مسیر درست بدهید.
runner.manifest.path_missing_name = مسیر مانیفست «{ $path }» نام پرونده ندارد.
runner.manifest.path_utf8 = مسیر مانیفست «{ $path }» ‏UTF-8 معتبر نیست.
runner.manifest.directory_utf8 = مسیر شاخهٔ مانیفست «{ $path }» ‏UTF-8 معتبر نیست.
runner.manifest.directory_label = شاخهٔ `{ $directory }`
runner.manifest.current_directory_label = شاخهٔ کنونی
runner.context.network_policy = ساخت سیاست شبکه ممکن نشد.
runner.context.load_manifest = بارگذاری مانیفست از { $path } ممکن نشد.
runner.context.serialise_manifest = تبدیل مانیفست به داده‌های پیاپی ممکن نشد.
runner.context.build_graph = ساخت گراف از روی مانیفست ممکن نشد.
runner.context.generate_ninja = تولید مانیفست Ninja ممکن نشد.
runner.context.render_graph = نمایش فرآوردهٔ گراف ممکن نشد.

runner.io.create_temp_file = ساخت پروندهٔ موقت Ninja ممکن نشد.
runner.io.write_temp_ninja = نوشتن در پروندهٔ موقت Ninja ممکن نشد.
runner.io.flush_temp_ninja = تخلیهٔ میان‌گیر پروندهٔ موقت Ninja ممکن نشد.
runner.io.sync_temp_ninja = همگام‌سازی پروندهٔ موقت Ninja ممکن نشد.
runner.io.create_parent_dir = ساخت شاخهٔ والد { $path } ممکن نشد.
runner.io.create_ninja_file = ساخت پروندهٔ Ninja در { $path } ممکن نشد.
runner.io.write_ninja_file = نوشتن در پروندهٔ Ninja در { $path } ممکن نشد.
runner.io.flush_ninja_file = تخلیهٔ میان‌گیر پروندهٔ Ninja در { $path } ممکن نشد.
runner.io.sync_ninja_file = همگام‌سازی پروندهٔ Ninja در { $path } ممکن نشد.
runner.io.open_ambient_dir = گشودن شاخهٔ پیرامون ممکن نشد.
runner.io.no_existing_ancestor = برای { $path } هیچ شاخهٔ والد موجودی نیست.
runner.io.derive_relative_path = استخراج مسیر نسبی Ninja ممکن نشد.
runner.io.non_utf8_path = مسیرهایی که UTF-8 نیستند پشتیبانی نمی‌شوند (مسیر: { $path }).
runner.io.write_stdout = نوشتن مانیفست Ninja در خروجی استاندارد ممکن نشد.
runner.io.flush_stdout = تخلیهٔ میان‌گیر خروجی استاندارد ممکن نشد.

# تشخیص‌های مانیفست.
manifest.parse = تجزیهٔ مانیفست ناکام ماند.
manifest.structure_error = خطای ساختاری مانیفست در { $name }: { $details }
manifest.yaml.parse = خطای تجزیهٔ YAML در سطر { $line }، ستون { $column }: { $details }
manifest.yaml.label = ‏YAML نامعتبر
manifest.yaml.hint.tabs = ‏YAML نویسهٔ تب را نمی‌پذیرد؛ برای تورفتگی از فاصله استفاده کنید.
manifest.yaml.hint.list_item = عضوهای فهرست YAML باید با «-» آغاز شوند و تورفتگی درست داشته باشند.
manifest.yaml.hint.expected_colon = این شبیه یک مدخل نگاشت است؛ «:» بعد از کلید جا افتاده است.
manifest.yaml.hint.mapping_values = نگاشت‌های YAML پس از «:» به یک مقدار (یا بلوک تودرتو) نیاز دارند.
manifest.yaml.hint.invalid_token = نشانهٔ YAML نامعتبر یا نابه‌جاست.
manifest.yaml.hint.escape = ممیزهای وارونه را بگریزانید یا دنباله‌های گریز نامعتبر را بردارید.
manifest.env.missing = یک متغیر محیطی الزامی تنظیم نشده است.
manifest.env.invalid_utf8 = یک متغیر محیطی دربردارندهٔ UTF-8 نامعتبر است.
manifest.vars.not_object = ‏`vars` در مانیفست باید نگاشت یا شیء باشد.
manifest.vars.reserved_name = کلید `vars` با نام '{ $name }' در مانیفست برای یک کمک‌کننده داخلی قالب رزرو شده است؛ نام متغیر را تغییر دهید.
manifest.read_failed = خواندن مانیفست از { $path } ممکن نشد.
manifest.resolve_workspace_root = تعیین ریشهٔ فضای کاری ممکن نشد.
manifest.workspace_non_utf8 = مسیر ریشهٔ فضای کاری «{ $path }» ‏UTF-8 معتبر نیست.
manifest.path_non_utf8 = مسیر مانیفست «{ $manifest }» ‏UTF-8 معتبر نیست: { $path }.
manifest.path_missing_name = مسیر مانیفست «{ $path }» نام پرونده ندارد.
manifest.open_workspace_failed = گشودن فضای کاری { $workspace } برای مانیفست { $manifest } ممکن نشد.
manifest.foreach.not_iterable = عبارت `foreach` پیمایش‌پذیر نیست.
manifest.foreach.serialise_item = تبدیل عضو `foreach` به داده‌های پیاپی ممکن نشد.
manifest.when.empty = عبارت `when` نباید تهی باشد.
manifest.when.eval_error = ارزیابی عبارت `when` «{ $expr }» ممکن نشد.
manifest.when.template_error = نمایش قالب `when` «{ $expr }» ممکن نشد.
manifest.target.vars_not_object = ‏`vars` هدف باید شیء باشد، اما { $value } به دست آمد.
manifest.vars.entry_not_object = مدخل `vars` مانیفست باید شیء باشد.
manifest.field_not_string = میدان «{ $field }» باید رشته باشد.
manifest.expression.parse_error = تجزیهٔ عبارت { $name } ممکن نشد.
manifest.expression.eval_error = ارزیابی عبارت { $name } ممکن نشد.

# تشخیص‌های ماکروهای مانیفست.
manifest.macro.signature_missing_identifier = امضای ماکرو شناسه ندارد.
manifest.macro.signature_missing_params = امضای ماکرو پارامتر ندارد.
manifest.macro.compile_failed = ترجمهٔ ماکروی { $name } ممکن نشد.
manifest.macro.sequence_invalid = ماکروها باید به شکل نگاشتی از نام‌ها به قالب‌ها تعریف شوند.
manifest.macro.register_failed = ثبت ماکروهای مانیفست ممکن نشد.
manifest.macro.not_initialised = محیط ماکروها راه‌اندازی نشده است.
manifest.macro.caller_invalid = فراخوانندهٔ ماکرو باید رشته باشد.
manifest.macro.template_load_failed = بارگذاری قالب ماکرو ممکن نشد.
manifest.macro.init_failed = راه‌اندازی محیط ماکروها ممکن نشد.
manifest.macro.missing = ماکروی { $name } وجود ندارد.

# خطاهای الگوهای glob در مانیفست.
manifest.glob.unmatched_brace = الگوی glob نامعتبر «{ $pattern }»: نویسهٔ «{ $character }» در جایگاه { $position } جفت ندارد.
manifest.glob.invalid_pattern = الگوی glob نامعتبر «{ $pattern }»: { $detail }.
manifest.glob.unknown_pattern_error = خطای الگوی ناشناخته.
manifest.glob.io_failed = ‏glob برای «{ $pattern }» ناکام ماند: { $detail }.
manifest.glob.unknown_io_error = خطای ورودی/خروجی ناشناخته.
manifest.command_list_empty = فیلد «command» نباید خالی باشد: یک رشتهٔ فرمان یا فهرستی ناتهی ارائه دهید.

# خطاهای بازنمایی میانی.
ir.rule_not_found = قاعدهٔ «{ $rule }» که هدف «{ $target }» به آن ارجاع می‌دهد یافت نشد.
ir.multiple_rules = هدف «{ $target }» باید تنها به یک قاعده ارجاع دهد، اما { $rules } به دست آمد.
ir.empty_rule = هدف «{ $target }» باید به یک قاعده ارجاع دهد.
ir.duplicate_outputs = خروجی‌های تکراری یافت شد: { $outputs }.
ir.circular_dependency = وابستگی چرخه‌ای یافت شد: { $cycle }.
ir.action_serialisation = تبدیل کنش به داده‌های پیاپی ممکن نشد: { $details }.
ir.invalid_command = درج نامعتبر در فرمان: { $snippet }.

# خطاهای تولید پرونده‌های Ninja.
ninja_gen.missing_action = کنش «{ $id }» که یک یال ساخت به آن ارجاع می‌دهد وجود ندارد.
ninja_gen.format = قالب‌بندی خروجی مانیفست Ninja ممکن نشد.

# اعتبارسنجی الگوهای میزبان.
host_pattern.empty = الگوی میزبان نباید تهی باشد.
host_pattern.contains_scheme = الگوی میزبان «{ $pattern }» نباید طرح URL داشته باشد.
host_pattern.contains_slash = الگوی میزبان «{ $pattern }» نباید «/» داشته باشد.
host_pattern.missing_suffix = الگوی میزبان «{ $pattern }» باید پس از «*.» پسوند داشته باشد.
host_pattern.empty_label = الگوی میزبان «{ $pattern }» برچسبی تهی دارد.
host_pattern.invalid_chars = الگوی میزبان «{ $pattern }» نویسه‌های نامعتبر دارد.
host_pattern.invalid_label_edge = برچسب‌های الگوی میزبان «{ $pattern }» نباید با «-» آغاز یا پایان یابند.
host_pattern.label_too_long = الگوی میزبان «{ $pattern }» برچسبی بلندتر از ۶۳ نویسه دارد.
host_pattern.too_long = الگوی میزبان «{ $pattern }» از مرز ۲۵۵ نویسه فراتر می‌رود.

# سیاست شبکه.
network_policy.scheme.empty = طرح نباید تهی باشد.
network_policy.scheme.invalid = طرح «{ $scheme }» نویسه‌های نامعتبر دارد.
network_policy.allowlist.empty = فهرست میزبان‌های مجاز نباید تهی باشد.
network_policy.scheme.not_allowed = طرح «{ $scheme }» مجاز نیست.
network_policy.missing_host = نشانی URL میزبان ندارد.
network_policy.host.blocked = میزبان «{ $host }» بر پایهٔ سیاست مسدود است.
network_policy.host.not_allowlisted = میزبان «{ $host }» در فهرست مجاز نیست.

# پیکربندی کتابخانهٔ استاندارد.
stdlib.config.default_fetch_cache_invalid = مسیر پیش‌فرض نهانگاه fetch باید نسبی باشد.
stdlib.config.default_which_cache_invalid = ظرفیت پیش‌فرض نهانگاه which باید مثبت باشد.
stdlib.config.workspace_root_absolute = مسیر ریشهٔ فضای کاری باید مطلق باشد.
stdlib.config.fetch_response_limit_positive = کران پاسخ fetch باید مثبت باشد.
stdlib.config.command_output_limit_positive = کران ضبط خروجی فرمان‌ها باید مثبت باشد.
stdlib.config.command_stream_limit_positive = کران جریان فرمان‌ها باید مثبت باشد.
stdlib.config.which_cache_capacity_positive = ظرفیت نهانگاه which باید مثبت باشد.
stdlib.config.skip_dir_empty = مدخل‌های شاخه‌های نادیده‌گرفته‌شده نباید تهی باشند.
stdlib.config.skip_dir_navigation = مدخل‌های شاخه‌های نادیده‌گرفته‌شده نباید «..» داشته باشند.
stdlib.config.skip_dir_separator = مدخل‌های شاخه‌های نادیده‌گرفته‌شده نباید جداکنندهٔ مسیر داشته باشند.
stdlib.config.fetch_cache_empty = مسیر نهانگاه fetch نباید تهی باشد.
stdlib.config.fetch_cache_not_relative = مسیر نهانگاه fetch باید نسبی باشد، اما { $path } به دست آمد.
stdlib.config.fetch_cache_escapes = مسیر نهانگاه fetch نباید از فضای کاری بیرون رود: { $path }.
stdlib.config.open_workspace_root = گشودن شاخهٔ کنونی به‌عنوان ریشهٔ فضای کاری stdlib ممکن نشد.
stdlib.config.resolve_cwd = تعیین شاخهٔ کنونی به‌عنوان ریشهٔ فضای کاری stdlib ممکن نشد.
stdlib.config.cwd_non_utf8 = شاخهٔ کنونی بخش‌هایی دارد که UTF-8 نیستند: { $path }.

# تشخیص‌های یاور fetch.
stdlib.fetch.url_invalid = نشانی URL نامعتبر «{ $url }»: { $details }.
stdlib.fetch.disallowed = نشانی URL «{ $url }» مجاز نیست: { $details }.
stdlib.fetch.failed = گرفتن «{ $url }» ممکن نشد: { $details }.
stdlib.fetch.cache_read_failed = خواندن مدخل نهانگاه «{ $name }» ممکن نشد: { $details }.
stdlib.fetch.cache_open_failed = گشودن مدخل نهانگاه «{ $name }» ممکن نشد: { $details }.
stdlib.fetch.response_read_failed = خواندن پاسخ از «{ $url }» ممکن نشد: { $details }.
stdlib.fetch.response_buffer_overflow = سرریز میان‌گیر هنگام خواندن «{ $url }».
stdlib.fetch.cache_write_failed = نوشتن نهانگاه برای «{ $url }» ممکن نشد: { $details }.
stdlib.fetch.response_limit_exceeded = پاسخ «{ $url }» از کران { $limit } بایت فراتر رفت.
stdlib.fetch.cache_limit_exceeded = پاسخ نهان‌شدهٔ «{ $name }» از کران { $limit } بایت فراتر رفت.
stdlib.fetch.io_failed = کنش «{ $action }» برای { $path } ناکام ماند: { $details }.
stdlib.fetch.action.sync_cache = همگام‌سازی نهانگاه fetch
stdlib.fetch.action.create_cache_dir = ساخت شاخهٔ نهانگاه fetch
stdlib.fetch.action.open_cache_dir = گشودن شاخهٔ نهانگاه fetch
stdlib.fetch.action.stat_cache = خواندن مشخصات مدخل نهانگاه fetch
stdlib.fetch.action.open_cache_entry = گشودن مدخل نهانگاه fetch

# تشخیص‌های یاور فرمان‌ها.
stdlib.command.location = فرمان «{ $command }» در قالب «{ $template }»
stdlib.command.spawn_failed = راه‌اندازی { $location } ممکن نشد: { $details }.
stdlib.command.io_failed = ‏{ $location } ناکام ماند: { $details }.
stdlib.command.closed_input_early = ورودی پیش از پایان نوشتن به فرمان بسته شد.
stdlib.command.broken_pipe = گسست لوله هنگام اجرای { $location }: { $details }.
stdlib.command.terminated_by_signal = ‏{ $location } با یک سیگنال پایان یافت.
stdlib.command.exited_with_status = ‏{ $location } با وضعیت { $status } پایان یافت.
stdlib.command.output_limit_exceeded = ‏{ $location } از کران { $mode } برابر { $limit } بایت برای { $stream } فراتر رفت.
stdlib.command.timeout = ‏{ $location } از مهلت { $seconds } ثانیه فراتر رفت.
stdlib.command.exit_status_suffix = ‏(وضعیت خروج { $status })
stdlib.command.signal_suffix = ‏(با سیگنال پایان یافت)
stdlib.command.shell.empty = فرمان پوسته نباید تهی باشد.
stdlib.command.grep.empty_pattern = الگوی grep نباید تهی باشد.
stdlib.command.grep.flags_not_string = پرچم‌های grep باید رشته باشند.
stdlib.command.quote.invalid = نهادن { $arg } میان گیومه ممکن نشد: { $details }.
stdlib.command.quote.line_break = آرگومان‌هایی که بازگشت به ابتدای خط یا شکست سطر دارند به‌شکل ایمن میان گیومه نمی‌گنجند.
stdlib.command.input_undefined = مقدار ورودی تعریف نشده است.
stdlib.command.tempfile.root_required = ساخت پرونده‌های موقت فرمان به ریشهٔ فضای کاری نیاز دارد.
stdlib.command.tempfile.create_failed = ساخت پروندهٔ موقت فرمان ممکن نشد: { $details }.
stdlib.command.options.invalid_utf8 = کلید گزینهٔ فرمان باید UTF-8 معتبر باشد.
stdlib.command.option.mode_not_string = حالت خروجی باید رشته باشد.
stdlib.command.options.invalid_type = گزینه‌های فرمان باید شیء باشند.
stdlib.command.output.mode_unsupported = حالت خروجی پشتیبانی‌نشده: «{ $mode }».
stdlib.command.output.mode.capture = ضبط
stdlib.command.output.mode.streaming = جریان
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# تشخیص‌های یاور مسیرها.
stdlib.path.io.failed = کنش «{ $action }» برای { $path } ناکام ماند ({ $label }).
stdlib.path.io.failed_with_detail = کنش «{ $action }» برای { $path } ناکام ماند: { $detail }.
stdlib.path.io.failed_with_label_and_detail = کنش «{ $action }» برای { $path } ناکام ماند ({ $label }): { $detail }.
stdlib.path.io.not_found = یافت نشد
stdlib.path.io.permission_denied = دسترسی رد شد
stdlib.path.io.already_exists = از پیش هست
stdlib.path.io.invalid_input = ورودی نامعتبر
stdlib.path.io.invalid_data = دادهٔ نامعتبر
stdlib.path.io.timed_out = مهلت به سر رسید
stdlib.path.io.interrupted = گسسته شد
stdlib.path.io.would_block = سبب انسداد می‌شد
stdlib.path.io.write_zero = صفر بایت نوشته شد
stdlib.path.io.unexpected_eof = پایان نابه‌هنگام پرونده
stdlib.path.io.broken_pipe = گسست لوله
stdlib.path.io.connection_refused = اتصال رد شد
stdlib.path.io.connection_reset = اتصال بازنشانی شد
stdlib.path.io.connection_aborted = اتصال لغو شد
stdlib.path.io.not_connected = بدون اتصال
stdlib.path.io.addr_in_use = نشانی در حال استفاده است
stdlib.path.io.addr_not_available = نشانی در دسترس نیست
stdlib.path.io.out_of_memory = حافظه به پایان رسید
stdlib.path.io.unsupported = پشتیبانی نمی‌شود
stdlib.path.io.file_too_large = پرونده بسیار بزرگ است
stdlib.path.io.resource_busy = منبع مشغول است
stdlib.path.io.executable_busy = پروندهٔ اجرایی مشغول است
stdlib.path.io.deadlock = بن‌بست
stdlib.path.io.crosses_devices = از مرز دستگاه‌ها می‌گذرد
stdlib.path.io.too_many_links = پیوندهای بیش از اندازه
stdlib.path.io.invalid_filename = نام پروندهٔ نامعتبر
stdlib.path.io.arg_list_too_long = فهرست آرگومان‌ها بیش از اندازه بلند است
stdlib.path.io.stale_handle = دستگیرهٔ پروندهٔ شبکه‌ای کهنه
stdlib.path.io.storage_full = فضای ذخیره‌سازی پر است
stdlib.path.io.not_seekable = جای‌گذاری در آن ممکن نیست
stdlib.path.io.network_down = شبکه از کار افتاده است
stdlib.path.io.network_unreachable = شبکه دسترس‌پذیر نیست
stdlib.path.io.host_unreachable = میزبان دسترس‌پذیر نیست
stdlib.path.io.other = خطای ورودی/خروجی
stdlib.path.action.canonicalize = متعارف‌سازی
stdlib.path.action.open_directory = گشودن شاخه
stdlib.path.action.stat = خواندن مشخصات
stdlib.path.action.read = خواندن
stdlib.path.action.open_file = گشودن پرونده
stdlib.path.with_suffix.empty_separator = ‏with_suffix به جداکننده‌ای ناتهی نیاز دارد.
stdlib.path.relative_to.mismatch = ‏{ $path } نسبت به { $root } نسبی نیست.
stdlib.path.expanduser.unsupported = گسترش ~ برای کاربری معین پشتیبانی نمی‌شود.
stdlib.path.expanduser.no_home = گسترش ~ ممکن نیست: هیچ متغیر محیطی برای شاخهٔ خانگی تنظیم نشده است.
stdlib.path.contents.unsupported_encoding = رمزگذاری پشتیبانی‌نشده: «{ $encoding }».
stdlib.path.hash.unsupported_algorithm = الگوریتم درهم‌سازی پشتیبانی‌نشده: «{ $algorithm }».
stdlib.path.hash.unsupported_algorithm_legacy = الگوریتم درهم‌سازی پشتیبانی‌نشده: «{ $algorithm }» (ویژگی «{ $feature }» را فعال کنید).

# تشخیص‌های یاورهای گردایه‌ها.
stdlib.collections.flatten.expected_sequence = ‏flatten عضوهای یک دنباله را انتظار داشت اما { $kind } یافت.
stdlib.collections.group_by.empty_attribute = ‏group_by به ویژگی‌ای ناتهی نیاز دارد.
stdlib.collections.group_by.unresolved = ‏group_by نتوانست «{ $attr }» را روی عضوی از گونهٔ { $kind } بیابد.

# تشخیص‌های یاورهای زمان.
stdlib.time.offset.invalid = اختلاف زمانی now «{ $offset }» نامعتبر است: «+HH:MM[:SS]» یا «Z» انتظار می‌رفت.
stdlib.time.timedelta.overflow = سرریز timedelta هنگام افزودن { $component }.
stdlib.time.label.weeks = هفته
stdlib.time.label.days = روز
stdlib.time.label.hours = ساعت
stdlib.time.label.minutes = دقیقه
stdlib.time.label.seconds = ثانیه
stdlib.time.label.milliseconds = میلی‌ثانیه
stdlib.time.label.microseconds = میکروثانیه
stdlib.time.label.nanoseconds = نانوثانیه

# تشخیص‌های یاور which.
stdlib.which.not_found = ‏[netsuke::jinja::which::not_found] فرمان «{ $command }» پس از بررسی { $count } مدخل PATH یافت نشد. پیش‌نمایش: { $preview }
stdlib.which.not_found.hint.cwd_auto = بخش‌های تهی PATH نادیده گرفته می‌شوند؛ برای دربرگرفتن شاخهٔ کاری از cwd_mode="auto" استفاده کنید.
stdlib.which.not_found.hint.cwd_always = برای دربرگرفتن شاخهٔ کنونی، cwd_mode="always" را تنظیم کنید.
stdlib.which.direct_not_found = ‏[netsuke::jinja::which::not_found] فرمان «{ $command }» در «{ $path }» وجود ندارد یا اجراشدنی نیست.
stdlib.which.args_error = ‏[netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = ‏<تهی>
stdlib.which.path_entry.non_utf8 = مدخل شمارهٔ { $index } در PATH نویسه‌هایی دارد که UTF-8 نیستند؛ ‏Netsuke به مسیرهای UTF-8 نیاز دارد.
stdlib.which.command.empty = ‏which به رشته‌ای ناتهی نیاز دارد.
stdlib.which.cwd_mode.invalid = ‏cwd_mode باید «auto»، «always» یا «never» باشد، اما «{ $mode }» به دست آمد.
stdlib.which.cwd.resolve_failed = تعیین شاخهٔ کنونی ممکن نشد: { $details }.
stdlib.which.cwd.non_utf8 = شاخهٔ کنونی بخش‌هایی دارد که UTF-8 نیستند.
stdlib.which.canonicalize_failed = متعارف‌سازی «{ $path }» ممکن نشد: { $details }.
stdlib.which.is_executable = بررسی اجراشدنی‌بودن «{ $path }» ممکن نشد: { $details }.
stdlib.which.canonicalize_non_utf8 = مسیر متعارف بخش‌هایی دارد که UTF-8 نیستند.
stdlib.which.workspace_non_utf8 = مسیر فضای کاری هنگام یافتن فرمان «{ $command }» بخش‌هایی دارد که UTF-8 نیستند: { $path }.
stdlib.which.walkdir_error = خطا هنگام پیمایش فضای کاری برای یافتن فرمان: { $details }.

# ثبت کتابخانهٔ استاندارد.
stdlib.register.open_dir = گشودن شاخهٔ کنونی برای ثبت stdlib ممکن نشد.
stdlib.register.resolve_dir = تعیین شاخهٔ کنونی برای ثبت stdlib ممکن نشد.
stdlib.register.dir_non_utf8 = شاخهٔ کنونی بخش‌هایی دارد که UTF-8 نیستند: { $path }.

# گزارش وضعیت برای حالت خروجی دسترس‌پذیر.
status.state.pending = در انتظار
status.state.running = در جریان
status.state.done = انجام شد
status.state.failed = ناکام
status.stage.label = مرحلهٔ { $current }/{ $total }: { $description }
status.stage.summary = ‏[{ $state }] { $label }
status.stage.summary_with_task = ‏[{ $state }] { $label } ({ $task_progress })
status.task.progress_label = کار { $current }/{ $total }
status.task.progress_update = ‏{ $task }: { $description }
status.stage.manifest_ingestion = خواندن پروندهٔ مانیفست
status.stage.initial_yaml_parsing = تجزیهٔ سند YAML
status.stage.template_expansion = گسترش دستورهای قالب
status.stage.final_rendering = بازگرداندن و نمایش مقادیر مانیفست
status.stage.ir_generation_validation = ساخت و بررسی گراف وابستگی‌ها
status.stage.ninja_synthesis = ترکیب طرح ساخت Ninja
status.stage.ninja_synthesis_execute = ترکیب طرح Ninja و اجرای { $tool }
status.stage.graph_rendering = نمایش فرآوردهٔ گراف
status.stage.graph_rendering_with_tool = نمایش { $tool }
status.complete = ‏{ $tool } به پایان رسید.
status.timing.summary_header = خلاصهٔ زمان به تفکیک مرحله:
status.timing.stage_line = ‏- { $label }: { $duration }
status.timing.total_line = زمان کل خط پردازش: { $duration }
status.tool.build = ساخت
status.tool.clean = پاک‌سازی
status.tool.graph = گراف
status.tool.graph_html = گراف (HTML)
status.tool.generate = تولید
status.tool.help_targets = راهنمای اهداف

# رشته‌های نمایش گراف به شکل HTML.
graph.html.title = گراف ساخت Netsuke
graph.html.heading = گراف ساخت Netsuke
graph.html.description = گراف ساختی که Netsuke نمایش داده است
graph.html.outline.summary = هدف‌ها و وابستگی‌ها (طرح متنی)
graph.html.outline.no_inputs = بدون ورودی
graph.html.noscript.notice = ‏JavaScript از کار افتاده است. طرح متنی بالا همان گراف کامل است؛ کد DOT در پی می‌آید.

# پیشوندهای معنایی برای خروجی دسترس‌پذیر.
semantic.prefix.error = خطا:
semantic.prefix.warning = هشدار:
semantic.prefix.success = موفق:
semantic.prefix.info = آگاهی:
semantic.prefix.timing = زمان:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# نمونه‌های صورت جمع برای مترجمان.
# فارسی در CLDR دو ردهٔ `one` و `other` دارد، ولی اسم پس از عدد مفرد می‌ماند.
example.files_processed = { $count ->
    [one] ‏{ $count } پرونده پردازش شد.
   *[other] ‏{ $count } پرونده پردازش شد.
}

example.errors_found = { $count ->
    [0] هیچ خطایی یافت نشد.
    [one] ‏{ $count } خطا یافت شد.
   *[other] ‏{ $count } خطا یافت شد.
}
