# משאבי לוקליזציה לשורת הפקודה של Netsuke.

cli.about = ‏Netsuke מהדר מניפסטים של YAML + Jinja לתוכניות בנייה של Ninja.
cli.long_about = ‏Netsuke ממיר מניפסטים של YAML + Jinja לגרפים ברי‑שחזור של Ninja ומריץ את Ninja עם ברירות מחדל בטוחות.
cli.usage = { $usage }

# טקסט העזרה של האפשרויות הכלליות.
cli.flag.file.help = הנתיב לקובץ המניפסט של Netsuke שיש להשתמש בו.
cli.flag.directory.help = הרצה כאילו ההפעלה התרחשה בספרייה זו.
cli.flag.config.help = הנתיב לקובץ תצורה, תוך עקיפת החיפוש האוטומטי.
cli.flag.jobs.help = קביעת מספר משימות הבנייה המקבילות.
cli.flag.verbose.help = הפעלת רישום אבחון מפורט וסיכומי זמן בסיום.
cli.flag.locale.help = תג השפה של טקסטי שורת הפקודה (למשל: en-US, he).
cli.flag.fetch_allow_scheme.help = סכימות URL נוספות המותרות לעוזר fetch.
cli.flag.fetch_allow_host.help = שמות מארחים המותרים כאשר הדחייה כברירת מחדל פעילה.
cli.flag.fetch_block_host.help = שמות מארחים החסומים תמיד, גם אם הותרו במקום אחר.
cli.flag.fetch_default_deny.help = דחיית כל המארחים כברירת מחדל; התרת הרשימה המוצהרת בלבד.
cli.flag.json.help = פלט JSON הניתן לקריאה במכונה.
cli.flag.no_input.help = לעולם לא לקרוא קלט אינטראקטיבי.
cli.flag.color.help = מדיניות הפלט הצבעוני (auto, always, never).
cli.flag.emoji.help = מדיניות האמוג׳י (auto, always, never).
cli.flag.progress.help = מדיניות הצגת ההתקדמות (auto, always, never).
cli.flag.accessibility.help = מדיניות הפלט הנגיש (auto, on, off).
cli.flag.default_targets.help = יעדי הבנייה המשמשים כברירת מחדל כאשר לא צוין יעד.

# תיאורי פקודות המשנה.
cli.subcommand.build.about = בניית היעדים המוגדרים במניפסט (ברירת מחדל).
cli.subcommand.build.long_about = בניית היעדים המבוקשים; אם לא צוין יעד, נעשה שימוש ביעדי ברירת המחדל של המניפסט.
cli.subcommand.clean.about = הסרת תוצרי הבנייה באמצעות Ninja.
cli.subcommand.clean.long_about = יצירת קובץ Ninja זמני ולאחר מכן הרצת `ninja -t clean`.
cli.subcommand.graph.about = פלט גרף התלויות של הבנייה. תבנית ברירת המחדל היא DOT.
cli.subcommand.graph.long_about = הטלת המניפסט המנותח של Netsuke לגרף בנייה קנוני וכתיבתו כ‑Graphviz DOT, או כדף HTML עצמאי עם `--html`. השתמשו ב‑`--output <קובץ>` לכתיבה לקובץ; `-` כותב לפלט התקני.
cli.subcommand.generate.about = יצירת מניפסט Ninja בלי להריץ את Ninja.
cli.subcommand.generate.long_about = כתיבת מניפסט Ninja שנוצר לפלט התקני או לקובץ שנבחר באמצעות `--output`.
cli.subcommand.help.about = הדפיסו את העזרה ברמה העליונה, או את העזרה עבור נושא בעל שם.
cli.subcommand.help.long_about = ללא נושא, זה תואם את `--help`. השתמשו ב-`help targets` כדי להדפיס את קטלוג היעדים והפעולות עבור הקובץ שנבחר.

# Help catalogue headings and markers.
cli.help.actions_heading = פעולות:
cli.help.targets_heading = יעדים:
cli.help.targets.about = הצגת רשימת היעדים והפעולות במניפסט שנבחר.
cli.help.default_marker = ברירת מחדל

# טקסט העזרה של אפשרויות פקודת המשנה build.
cli.subcommand.build.flag.targets.help = היעדים שיש לבנות (בהשמטה נעשה שימוש בברירות המחדל של המניפסט).

# טקסט העזרה של אפשרויות פקודת המשנה graph.
cli.subcommand.graph.flag.html.help = עיבוד הגרף כדף HTML עצמאי במקום כ‑DOT.
cli.subcommand.graph.flag.output.help = כתיבת תוצר הגרף לקובץ; לפלט התקני השתמשו ב‑`-`.

# טקסט העזרה של אפשרויות פקודת המשנה generate.
cli.subcommand.generate.flag.output.help = כתיבת מניפסט Ninja שנוצר לקובץ במקום לפלט התקני.

# שגיאות אימות בשורת הפקודה.
cli.validation.jobs.invalid_number = ‏{ $value } אינו מספר תקין.
cli.validation.jobs.out_of_range = מספר המשימות חייב להיות בין { $min } ל‑{ $max }.
cli.validation.scheme.empty = הסכימה אינה יכולה להיות ריקה.
cli.validation.scheme.invalid_start = הסכימה „{ $scheme }” חייבת להתחיל באות ASCII.
cli.validation.scheme.invalid = סכימה לא תקינה: „{ $scheme }”.
cli.validation.locale.empty = תג השפה אינו יכול להיות ריק.
cli.validation.locale.invalid = תג שפה לא תקין: „{ $locale }”.
cli.validation.color.invalid = מדיניות צבע לא תקינה: „{ $value }”. ערכים תקפים: auto, always, never.
cli.validation.emoji.invalid = מדיניות אמוג׳י לא תקינה: „{ $value }”. ערכים תקפים: auto, always, never.
cli.validation.progress.invalid = מדיניות התקדמות לא תקינה: „{ $value }”. ערכים תקפים: auto, always, never.
cli.validation.accessibility.invalid = מדיניות נגישות לא תקינה: „{ $value }”. ערכים תקפים: auto, on, off.
cli.validation.config.expected_object = ערכי שורת הפקודה היו אמורים לעבור סריאליזציה לאובייקט, אך התקבל { $value }.

# הודעות השגיאה של Clap.
clap-error-missing-argument = חסר ארגומנט נדרש: { $argument }
clap-error-missing-subcommand = חסרה פקודת משנה. האפשרויות הזמינות: { $valid_subcommands }
clap-error-unknown-argument = ארגומנט לא מוכר: { $argument }
clap-error-invalid-value = ערך לא תקין עבור { $argument }: { $value }
clap-error-invalid-subcommand = פקודת משנה לא מוכרת: { $subcommand }
# הערה: הניסוח של value-validation שונה מזה של invalid-value כדי להבחין בין
# כשלים של מאמתים מותאמים (ErrorKind::ValueValidation) לבין אי‑התאמת טיפוסים
# (ErrorKind::InvalidValue).
clap-error-value-validation = האימות של { $argument } נכשל: { $value }

# שגיאות והקשר בזמן ריצה.
runner.manifest.not_found = המניפסט „{ $manifest_name }” לא נמצא ב‑{ $directory }.
runner.manifest.not_found.help = ודאו שהמניפסט קיים או ציינו `--file` עם הנתיב הנכון.
runner.manifest.path_missing_name = לנתיב המניפסט „{ $path }” אין שם קובץ.
runner.manifest.path_utf8 = נתיב המניפסט „{ $path }” אינו UTF-8 תקין.
runner.manifest.directory_utf8 = נתיב ספריית המניפסט „{ $path }” אינו UTF-8 תקין.
runner.manifest.directory_label = הספרייה `{ $directory }`
runner.manifest.current_directory_label = הספרייה הנוכחית
runner.manifest.default_not_declared = ברירת המחדל של המניפסט '{ $default }' אינה מציינת פעולה או יעד מוצהרים.
runner.context.network_policy = לא ניתן היה לבנות את מדיניות הרשת.
runner.context.load_manifest = לא ניתן היה לטעון את המניפסט מ‑{ $path }.
runner.context.serialise_manifest = לא ניתן היה לבצע סריאליזציה למניפסט.
runner.context.build_graph = לא ניתן היה לבנות גרף מהמניפסט.
runner.context.generate_ninja = לא ניתן היה ליצור את מניפסט Ninja.
runner.context.render_graph = לא ניתן היה לעבד את תוצר הגרף.

runner.io.create_temp_file = לא ניתן היה ליצור את קובץ Ninja הזמני.
runner.io.write_temp_ninja = לא ניתן היה לכתוב לקובץ Ninja הזמני.
runner.io.flush_temp_ninja = לא ניתן היה לרוקן את החוצץ של קובץ Ninja הזמני.
runner.io.sync_temp_ninja = לא ניתן היה לסנכרן את קובץ Ninja הזמני.
runner.io.create_parent_dir = לא ניתן היה ליצור את ספריית האב { $path }.
runner.io.create_ninja_file = לא ניתן היה ליצור את קובץ Ninja ב‑{ $path }.
runner.io.write_ninja_file = לא ניתן היה לכתוב לקובץ Ninja ב‑{ $path }.
runner.io.flush_ninja_file = לא ניתן היה לרוקן את החוצץ של קובץ Ninja ב‑{ $path }.
runner.io.sync_ninja_file = לא ניתן היה לסנכרן את קובץ Ninja ב‑{ $path }.
runner.io.open_ambient_dir = לא ניתן היה לפתוח את הספרייה הסובבת.
runner.io.non_utf8_working_directory = ‏נתיב ספריית העבודה אינו UTF-8 חוקי.
runner.io.no_existing_ancestor = אין ספריית אב קיימת עבור { $path }.
runner.io.derive_relative_path = לא ניתן היה לגזור את נתיב Ninja היחסי.
runner.io.non_utf8_path = נתיבים שאינם UTF-8 אינם נתמכים (נתיב: { $path }).
runner.io.write_stdout = לא ניתן היה לכתוב את מניפסט Ninja לפלט התקני.
runner.io.flush_stdout = לא ניתן היה לרוקן את החוצץ של הפלט התקני.
runner.io.dyndep.create_dir = ‏לא ניתן ליצור את תיקיית dyndep ‏{ $path }.
runner.io.dyndep.read = ‏לא ניתן לקרוא את קובץ dyndep שנוצר ב־‏{ $path }.
runner.io.dyndep.write = ‏לא ניתן לכתוב את קובץ dyndep שנוצר ב־‏{ $path }.
runner.io.dyndep.rename = ‏לא ניתן להשלים את קובץ dyndep שנוצר ב־‏{ $path }.
runner.io.dyndep.corrupt = ‏קובץ dyndep שנוצר ב־‏{ $path } אינו תואם לתוכן הצפוי; הסירו קובץ זה בלבד ונסו שוב.
runner.io.dyndep.race = ‏תהליך אחר כתב את קובץ dyndep ‏{ $path }, אך לא ניתן לאמת את תוכנו.
runner.io.dyndep.temp_collisions = ‏לא ניתן ליצור קובץ dyndep זמני וייחודי עבור ‏{ $path } לאחר התנגשויות שמות חוזרות.
runner.io.dyndep.too_large = ‏קובץ dyndep שנוצר ב־‏{ $path } חורג ממגבלת האימות של ‏{ $limit } בתים.

# אבחון המניפסט.
manifest.parse = ניתוח המניפסט נכשל.
manifest.structure_error = שגיאת מבנה במניפסט ב‑{ $name }: { $details }
manifest.yaml.parse = שגיאת ניתוח YAML בשורה { $line }, בעמודה { $column }: { $details }
manifest.yaml.label = ‏YAML לא תקין
manifest.yaml.hint.tabs = ‏YAML אינו מתיר תווי טאב; השתמשו ברווחים להזחה.
manifest.yaml.hint.list_item = פריטי רשימה ב‑YAML חייבים להתחיל ב‑„-” ולהיות מוזחים כראוי.
manifest.yaml.hint.expected_colon = זה נראה כמו רשומת מיפוי; חסר „:” אחרי המפתח.
manifest.yaml.hint.mapping_values = מיפויים ב‑YAML דורשים ערך אחרי „:” (או בלוק מקונן).
manifest.yaml.hint.invalid_token = אסימון ה‑YAML אינו תקין או אינו צפוי.
manifest.yaml.hint.escape = בצעו מילוט ללוכסנים אחוריים או הסירו רצפי מילוט לא תקינים.
manifest.env.missing = משתנה סביבה נדרש אינו מוגדר.
manifest.env.invalid_utf8 = משתנה סביבה מכיל UTF-8 לא תקין.
manifest.vars.not_object = השדה `vars` של המניפסט חייב להיות מיפוי או אובייקט.
manifest.vars.reserved_name = מפתח `vars` בשם '{ $name }' במניפסט שמור לפונקציית עזר מובנית של תבניות; שנה את שם המשתנה.
manifest.read_failed = לא ניתן היה לקרוא את המניפסט מ‑{ $path }.
manifest.resolve_workspace_root = לא ניתן היה לקבוע את שורש סביבת העבודה.
manifest.workspace_non_utf8 = נתיב השורש של סביבת העבודה „{ $path }” אינו UTF-8 תקין.
manifest.path_non_utf8 = הנתיב של המניפסט „{ $manifest }” אינו UTF-8 תקין: { $path }.
manifest.path_missing_name = לנתיב המניפסט „{ $path }” אין שם קובץ.
manifest.open_workspace_failed = לא ניתן היה לפתוח את סביבת העבודה { $workspace } עבור המניפסט { $manifest }.
manifest.foreach.not_iterable = הביטוי `foreach` אינו ניתן למעבר.
manifest.foreach.serialise_item = לא ניתן היה לבצע סריאליזציה לפריט של `foreach`.
manifest.when.empty = הביטוי `when` אינו יכול להיות ריק.
manifest.when.eval_error = לא ניתן היה להעריך את הביטוי `when` „{ $expr }”.
manifest.when.template_error = לא ניתן היה לעבד את התבנית `when` „{ $expr }”.
manifest.target.vars_not_object = השדה `vars` של היעד חייב להיות אובייקט, אך התקבל { $value }.
manifest.vars.entry_not_object = רשומת `vars` של המניפסט חייבת להיות אובייקט.
manifest.field_not_string = השדה „{ $field }” חייב להיות מחרוזת.
manifest.expression.parse_error = לא ניתן היה לנתח את הביטוי { $name }.
manifest.expression.eval_error = לא ניתן היה להעריך את הביטוי { $name }.

# אבחון המאקרו של המניפסט.
manifest.macro.signature_missing_identifier = בחתימת המאקרו חסר מזהה.
manifest.macro.signature_missing_params = בחתימת המאקרו חסרים פרמטרים.
manifest.macro.compile_failed = לא ניתן היה להדר את המאקרו { $name }.
manifest.macro.sequence_invalid = יש להגדיר מאקרו כמיפוי משמות לתבניות.
manifest.macro.register_failed = לא ניתן היה לרשום את המאקרו של המניפסט.
manifest.macro.not_initialised = סביבת המאקרו אינה מאותחלת.
manifest.macro.caller_invalid = הקורא למאקרו חייב להיות מחרוזת.
manifest.macro.template_load_failed = לא ניתן היה לטעון את תבנית המאקרו.
manifest.macro.init_failed = לא ניתן היה לאתחל את סביבת המאקרו.
manifest.macro.missing = המאקרו { $name } חסר.

# שגיאות תבניות glob במניפסט.
manifest.glob.unmatched_brace = תבנית glob לא תקינה „{ $pattern }”: התו „{ $character }” בלא זוג במיקום { $position }.
manifest.glob.invalid_pattern = תבנית glob לא תקינה „{ $pattern }”: { $detail }.
manifest.glob.unknown_pattern_error = שגיאת תבנית לא ידועה.
manifest.glob.io_failed = ‏glob נכשל עבור „{ $pattern }”: { $detail }.
manifest.glob.unknown_io_error = שגיאת קלט/פלט לא ידועה.
manifest.command_list_empty = השדה „command” אינו יכול להיות ריק: יש לספק מחרוזת פקודה או רשימה שאינה ריקה.

# שגיאות הייצוג הביניימי.
ir.rule_not_found = הכלל „{ $rule }” שאליו מפנה היעד „{ $target }” לא נמצא.
ir.multiple_rules = היעד „{ $target }” חייב להפנות לכלל אחד בלבד, אך התקבל { $rules }.
ir.empty_rule = היעד „{ $target }” חייב להפנות לכלל.
ir.duplicate_outputs = זוהו פלטים כפולים: { $outputs }.
ir.circular_dependency = זוהתה תלות מעגלית: { $cycle }.
ir.action_serialisation = לא ניתן היה לבצע סריאליזציה לפעולה: { $details }.
ir.invalid_command = שיבוץ לא תקין בפקודה: { $snippet }.

# שגיאות ביצירת קובצי Ninja.
ninja_gen.missing_action = הפעולה „{ $id }” שאליה מפנה קשת בנייה חסרה.
ninja_gen.format = לא ניתן היה לעצב את פלט מניפסט Ninja.
ninja_gen.dyndep_files_required = ‏בנייה זו דורשת חבילת Ninja שנוצרה; השתמשו ב־`netsuke build`, ב־`netsuke clean` או ב־`netsuke generate` כדי ליצור את קובצי dyndep.
ninja_gen.reserved_output_path = ‏הנתיב '{ $path }' שמור למצב התלויות הסדרתיות של Netsuke.
ninja_gen.unsupported_path_character = ‏הנתיב '{ $path }' מכיל את תו הנתיב הלא נתמך של Ninja, '{ $character }'.

# אימות תבניות מארח.
host_pattern.empty = תבנית המארח אינה יכולה להיות ריקה.
host_pattern.contains_scheme = תבנית המארח „{ $pattern }” אינה יכולה לכלול סכימת URL.
host_pattern.contains_slash = תבנית המארח „{ $pattern }” אינה יכולה לכלול „/”.
host_pattern.missing_suffix = תבנית המארח „{ $pattern }” חייבת לכלול סיומת אחרי „*.”.
host_pattern.empty_label = תבנית המארח „{ $pattern }” מכילה תווית ריקה.
host_pattern.invalid_chars = תבנית המארח „{ $pattern }” מכילה תווים לא תקינים.
host_pattern.invalid_label_edge = תוויות של תבנית המארח „{ $pattern }” אינן יכולות להתחיל או להסתיים ב‑„-”.
host_pattern.label_too_long = תבנית המארח „{ $pattern }” מכילה תווית ארוכה מ‑63 תווים.
host_pattern.too_long = תבנית המארח „{ $pattern }” חורגת ממגבלת 255 התווים.

# מדיניות הרשת.
network_policy.scheme.empty = הסכימה אינה יכולה להיות ריקה.
network_policy.scheme.invalid = הסכימה „{ $scheme }” מכילה תווים לא תקינים.
network_policy.allowlist.empty = רשימת המארחים המותרים אינה יכולה להיות ריקה.
network_policy.scheme.not_allowed = הסכימה „{ $scheme }” אינה מותרת.
network_policy.missing_host = בכתובת ה‑URL חסר מארח.
network_policy.host.blocked = המארח „{ $host }” חסום על ידי המדיניות.
network_policy.host.not_allowlisted = המארח „{ $host }” אינו ברשימת המותרים.

# תצורת הספרייה התקנית.
stdlib.config.default_fetch_cache_invalid = נתיב ברירת המחדל של מטמון fetch חייב להיות יחסי.
stdlib.config.default_which_cache_invalid = קיבולת ברירת המחדל של מטמון which חייבת להיות חיובית.
stdlib.config.workspace_root_absolute = נתיב השורש של סביבת העבודה חייב להיות מוחלט.
stdlib.config.fetch_response_limit_positive = מגבלת התגובה של fetch חייבת להיות חיובית.
stdlib.config.command_output_limit_positive = מגבלת לכידת פלט הפקודות חייבת להיות חיובית.
stdlib.config.command_stream_limit_positive = מגבלת הזרימה של הפקודות חייבת להיות חיובית.
stdlib.config.which_cache_capacity_positive = קיבולת מטמון which חייבת להיות חיובית.
stdlib.config.skip_dir_empty = רשומות הספריות המדולגות אינן יכולות להיות ריקות.
stdlib.config.skip_dir_navigation = רשומות הספריות המדולגות אינן יכולות להכיל „..”.
stdlib.config.skip_dir_separator = רשומות הספריות המדולגות אינן יכולות להכיל מפרידי נתיב.
stdlib.config.fetch_cache_empty = נתיב מטמון fetch אינו יכול להיות ריק.
stdlib.config.fetch_cache_not_relative = נתיב מטמון fetch חייב להיות יחסי, אך התקבל { $path }.
stdlib.config.fetch_cache_escapes = נתיב מטמון fetch אינו יכול לצאת מסביבת העבודה: { $path }.
stdlib.config.open_workspace_root = לא ניתן היה לפתוח את הספרייה הנוכחית כשורש סביבת העבודה של stdlib.
stdlib.config.resolve_cwd = לא ניתן היה לקבוע את הספרייה הנוכחית כשורש סביבת העבודה של stdlib.
stdlib.config.cwd_non_utf8 = הספרייה הנוכחית מכילה חלקים שאינם UTF-8: { $path }.

# אבחון העוזר fetch.
stdlib.fetch.url_invalid = כתובת URL לא תקינה „{ $url }”: { $details }.
stdlib.fetch.disallowed = כתובת ה‑URL „{ $url }” אינה מותרת: { $details }.
stdlib.fetch.failed = לא ניתן היה להביא את „{ $url }”: { $details }.
stdlib.fetch.cache_read_failed = לא ניתן היה לקרוא את רשומת המטמון „{ $name }”: { $details }.
stdlib.fetch.cache_open_failed = לא ניתן היה לפתוח את רשומת המטמון „{ $name }”: { $details }.
stdlib.fetch.response_read_failed = לא ניתן היה לקרוא את התגובה מ‑„{ $url }”: { $details }.
stdlib.fetch.response_buffer_overflow = גלישת חוצץ בעת קריאת „{ $url }”.
stdlib.fetch.cache_write_failed = לא ניתן היה לכתוב את המטמון עבור „{ $url }”: { $details }.
stdlib.fetch.response_limit_exceeded = התגובה מ‑„{ $url }” חרגה ממגבלת { $limit } בתים.
stdlib.fetch.cache_limit_exceeded = התגובה שבמטמון „{ $name }” חרגה ממגבלת { $limit } בתים.
stdlib.fetch.io_failed = הפעולה „{ $action }” נכשלה עבור { $path }: { $details }.
stdlib.fetch.action.sync_cache = סנכרון מטמון fetch
stdlib.fetch.action.create_cache_dir = יצירת ספריית מטמון fetch
stdlib.fetch.action.open_cache_dir = פתיחת ספריית מטמון fetch
stdlib.fetch.action.stat_cache = קריאת נתוני רשומת מטמון fetch
stdlib.fetch.action.open_cache_entry = פתיחת רשומת מטמון fetch

# אבחון עוזר הפקודות.
stdlib.command.location = הפקודה „{ $command }” בתבנית „{ $template }”
stdlib.command.spawn_failed = לא ניתן היה להפעיל את { $location }: { $details }.
stdlib.command.io_failed = ‏{ $location } נכשל: { $details }.
stdlib.command.closed_input_early = הקלט נסגר לפני שהכתיבה אל הפקודה הושלמה.
stdlib.command.broken_pipe = הצינור נשבר בעת הרצת { $location }: { $details }.
stdlib.command.terminated_by_signal = ‏{ $location } הופסק על ידי אות.
stdlib.command.exited_with_status = ‏{ $location } הסתיים במצב { $status }.
stdlib.command.output_limit_exceeded = ‏{ $location } חרג ממגבלת { $mode } של { $limit } בתים עבור { $stream }.
stdlib.command.timeout = ‏{ $location } חרג ממגבלת הזמן של { $seconds } שניות.
stdlib.command.exit_status_suffix = ‏(מצב יציאה { $status })
stdlib.command.signal_suffix = ‏(הופסק על ידי אות)
stdlib.command.shell.empty = פקודת המעטפת אינה יכולה להיות ריקה.
stdlib.command.grep.empty_pattern = תבנית grep אינה יכולה להיות ריקה.
stdlib.command.grep.flags_not_string = דגלי grep חייבים להיות מחרוזות.
stdlib.command.quote.invalid = לא ניתן היה למקם את { $arg } בין מרכאות: { $details }.
stdlib.command.quote.line_break = ארגומנטים המכילים החזרת גרר או מעבר שורה אינם ניתנים למיקום בטוח בין מרכאות.
stdlib.command.input_undefined = ערך הקלט אינו מוגדר.
stdlib.command.tempfile.root_required = יצירת קובצי פקודה זמניים מחייבת את שורש סביבת העבודה.
stdlib.command.tempfile.create_failed = לא ניתן היה ליצור את קובץ הפקודה הזמני: { $details }.
stdlib.command.options.invalid_utf8 = מפתח אפשרות של פקודה חייב להיות UTF-8 תקין.
stdlib.command.option.mode_not_string = מצב הפלט חייב להיות מחרוזת.
stdlib.command.options.invalid_type = אפשרויות הפקודה חייבות להיות אובייקט.
stdlib.command.output.mode_unsupported = מצב פלט שאינו נתמך: „{ $mode }”.
stdlib.command.output.mode.capture = לכידה
stdlib.command.output.mode.streaming = הזרמה
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# אבחון עוזר הנתיבים.
stdlib.path.io.failed = הפעולה „{ $action }” נכשלה עבור { $path } ({ $label }).
stdlib.path.io.failed_with_detail = הפעולה „{ $action }” נכשלה עבור { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = הפעולה „{ $action }” נכשלה עבור { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = לא נמצא
stdlib.path.io.permission_denied = ההרשאה נדחתה
stdlib.path.io.already_exists = כבר קיים
stdlib.path.io.invalid_input = קלט לא תקין
stdlib.path.io.invalid_data = נתונים לא תקינים
stdlib.path.io.timed_out = תם הזמן
stdlib.path.io.interrupted = הופסק
stdlib.path.io.would_block = היה גורם לחסימה
stdlib.path.io.write_zero = נכתבו אפס בתים
stdlib.path.io.unexpected_eof = סוף קובץ בלתי צפוי
stdlib.path.io.broken_pipe = צינור שבור
stdlib.path.io.connection_refused = החיבור נדחה
stdlib.path.io.connection_reset = החיבור אופס
stdlib.path.io.connection_aborted = החיבור בוטל
stdlib.path.io.not_connected = אין חיבור
stdlib.path.io.addr_in_use = הכתובת בשימוש
stdlib.path.io.addr_not_available = הכתובת אינה זמינה
stdlib.path.io.out_of_memory = אין די זיכרון
stdlib.path.io.unsupported = אינו נתמך
stdlib.path.io.file_too_large = הקובץ גדול מדי
stdlib.path.io.resource_busy = המשאב תפוס
stdlib.path.io.executable_busy = קובץ ההרצה תפוס
stdlib.path.io.deadlock = קיפאון
stdlib.path.io.crosses_devices = חוצה התקנים
stdlib.path.io.too_many_links = יותר מדי קישורים
stdlib.path.io.invalid_filename = שם קובץ לא תקין
stdlib.path.io.arg_list_too_long = רשימת הארגומנטים ארוכה מדי
stdlib.path.io.stale_handle = ידית קובץ רשת מיושנת
stdlib.path.io.storage_full = שטח האחסון מלא
stdlib.path.io.not_seekable = אינו ניתן למיקום
stdlib.path.io.network_down = הרשת מושבתת
stdlib.path.io.network_unreachable = הרשת אינה נגישה
stdlib.path.io.host_unreachable = המארח אינו נגיש
stdlib.path.io.other = שגיאת קלט/פלט
stdlib.path.action.canonicalize = קנוניזציה
stdlib.path.action.open_directory = פתיחת ספרייה
stdlib.path.action.stat = קריאת נתונים
stdlib.path.action.read = קריאה
stdlib.path.action.open_file = פתיחת קובץ
stdlib.path.with_suffix.empty_separator = ‏with_suffix מחייב מפריד שאינו ריק.
stdlib.path.relative_to.mismatch = ‏{ $path } אינו יחסי אל { $root }.
stdlib.path.expanduser.unsupported = הרחבת ~ עבור משתמש מסוים אינה נתמכת.
stdlib.path.expanduser.no_home = לא ניתן להרחיב את ~: לא הוגדר אף משתנה סביבה לספריית הבית.
stdlib.path.contents.unsupported_encoding = קידוד שאינו נתמך: „{ $encoding }”.
stdlib.path.hash.unsupported_algorithm = אלגוריתם גיבוב שאינו נתמך: „{ $algorithm }”.
stdlib.path.hash.unsupported_algorithm_legacy = אלגוריתם גיבוב שאינו נתמך: „{ $algorithm }” (הפעילו את התכונה „{ $feature }”).

# אבחון עוזרי האוספים.
stdlib.collections.flatten.expected_sequence = ‏flatten ציפה לפריטים של סדרה אך מצא { $kind }.
stdlib.collections.group_by.empty_attribute = ‏group_by מחייב תכונה שאינה ריקה.
stdlib.collections.group_by.unresolved = ‏group_by לא הצליח לאתר את „{ $attr }” בפריט מסוג { $kind }.

# אבחון עוזרי הזמן.
stdlib.time.offset.invalid = ההיסט של now „{ $offset }” אינו תקין: נדרש „+HH:MM[:SS]” או „Z”.
stdlib.time.timedelta.overflow = גלישה ב‑timedelta בעת הוספת { $component }.
stdlib.time.label.weeks = שבועות
stdlib.time.label.days = ימים
stdlib.time.label.hours = שעות
stdlib.time.label.minutes = דקות
stdlib.time.label.seconds = שניות
stdlib.time.label.milliseconds = אלפיות שנייה
stdlib.time.label.microseconds = מיליוניות שנייה
stdlib.time.label.nanoseconds = מיליארדיות שנייה

# אבחון העוזר which.
stdlib.which.not_found = ‏[netsuke::jinja::which::not_found] הפקודה „{ $command }” לא נמצאה לאחר בדיקת { $count } רשומות ב‑PATH. תצוגה מקדימה: { $preview }
stdlib.which.not_found.hint.cwd_auto = מקטעים ריקים ב‑PATH מתעלמים מהם; השתמשו ב‑cwd_mode="auto" כדי לכלול את ספריית העבודה.
stdlib.which.not_found.hint.cwd_always = הגדירו cwd_mode="always" כדי לכלול את הספרייה הנוכחית.
stdlib.which.direct_not_found = ‏[netsuke::jinja::which::not_found] הפקודה „{ $command }” ב‑„{ $path }” חסרה או אינה ניתנת להרצה.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = ‏<ריק>
stdlib.which.path_entry.non_utf8 = הרשומה מס׳ { $index } ב‑PATH מכילה תווים שאינם UTF-8; ‏Netsuke מחייב נתיבים בקידוד UTF-8.
stdlib.which.command.empty = ‏which מחייב מחרוזת שאינה ריקה.
stdlib.which.cwd_mode.invalid = ‏cwd_mode חייב להיות „auto”, „always” או „never”, אך התקבל „{ $mode }”.
stdlib.which.cwd.resolve_failed = לא ניתן היה לקבוע את הספרייה הנוכחית: { $details }.
stdlib.which.cwd.non_utf8 = הספרייה הנוכחית מכילה חלקים שאינם UTF-8.
stdlib.which.canonicalize_failed = לא ניתן היה לבצע קנוניזציה ל‑„{ $path }”: { $details }.
stdlib.which.is_executable = לא ניתן היה לבדוק אם „{ $path }” ניתן להרצה: { $details }.
stdlib.which.canonicalize_non_utf8 = הנתיב הקנוני מכיל חלקים שאינם UTF-8.
stdlib.which.workspace_non_utf8 = נתיב סביבת העבודה מכיל חלקים שאינם UTF-8 בעת איתור הפקודה „{ $command }”: { $path }.
stdlib.which.walkdir_error = שגיאה במעבר על סביבת העבודה בעת איתור הפקודה: { $details }.

# רישום הספרייה התקנית.
stdlib.register.open_dir = לא ניתן היה לפתוח את הספרייה הנוכחית לצורך רישום stdlib.
stdlib.register.resolve_dir = לא ניתן היה לקבוע את הספרייה הנוכחית לצורך רישום stdlib.
stdlib.register.dir_non_utf8 = הספרייה הנוכחית מכילה חלקים שאינם UTF-8: { $path }.

# דיווח מצב עבור מצב הפלט הנגיש.
status.state.pending = ממתין
status.state.running = מתבצע
status.state.done = הושלם
status.state.failed = נכשל
status.stage.label = שלב { $current }/{ $total }: { $description }
status.stage.summary = ‏[{ $state }] { $label }
status.stage.summary_with_task = ‏[{ $state }] { $label } ({ $task_progress })
status.task.progress_label = משימה { $current }/{ $total }
status.task.progress_update = ‏{ $task }: { $description }
status.stage.manifest_ingestion = קריאת קובץ המניפסט
status.stage.initial_yaml_parsing = ניתוח מסמך ה‑YAML
status.stage.template_expansion = הרחבת הנחיות התבנית
status.stage.final_rendering = ביטול הסריאליזציה ועיבוד ערכי המניפסט
status.stage.ir_generation_validation = בניית גרף התלויות ואימותו
status.stage.ninja_synthesis = הרכבת תוכנית הבנייה של Ninja
status.stage.ninja_synthesis_execute = הרכבת תוכנית Ninja והרצת { $tool }
status.stage.graph_rendering = עיבוד תוצר הגרף
status.stage.graph_rendering_with_tool = עיבוד { $tool }
status.complete = ‏הפעולה הושלמה: { $tool }.
status.timing.summary_header = סיכום זמנים לפי שלב:
status.timing.stage_line = ‏- { $label }: { $duration }
status.timing.total_line = זמן כולל של הצינור: { $duration }
status.tool.build = בנייה
status.tool.clean = ניקוי
status.tool.graph = גרף
status.tool.graph_html = גרף (HTML)
status.tool.generate = יצירה
status.tool.help_targets = עזרת יעדים

# מחרוזות עיבוד הגרף ל‑HTML.
graph.html.title = גרף הבנייה של Netsuke
graph.html.heading = גרף הבנייה של Netsuke
graph.html.description = גרף בנייה שעובד על ידי Netsuke
graph.html.outline.summary = יעדים ותלויות (מתאר טקסטואלי)
graph.html.outline.no_inputs = אין קלטים
graph.html.noscript.notice = ‏JavaScript מושבת. המתאר הטקסטואלי שלמעלה הוא הגרף המלא; מקור ה‑DOT מופיע אחריו.

# קידומות סמנטיות לפלט הנגיש.
semantic.prefix.error = שגיאה:
semantic.prefix.warning = אזהרה:
semantic.prefix.success = הצלחה:
semantic.prefix.info = מידע:
semantic.prefix.timing = זמן:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# דוגמאות לצורות רבים עבור מתרגמים.
# העברית משתמשת בקטגוריות CLDR ‏`one`, ‏`two`, ‏`many` (עשרות עגולות מ‑20
# ומעלה) ו‑`other`.
example.files_processed = { $count ->
    [one] עובד קובץ אחד.
    [two] עובדו שני קבצים.
    [many] עובדו { $count } קבצים.
   *[other] עובדו { $count } קבצים.
}

example.errors_found = { $count ->
    [0] לא נמצאו שגיאות.
    [one] נמצאה שגיאה אחת.
    [two] נמצאו שתי שגיאות.
    [many] נמצאו { $count } שגיאות.
   *[other] נמצאו { $count } שגיאות.
}
