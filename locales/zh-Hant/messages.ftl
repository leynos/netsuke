# Netsuke 命令列的在地化資源（繁體中文）。

runner.io.dyndep.retention = 無法套用 { $path } 處產生的 dyndep 保留設定。
cli.about = Netsuke 會將 YAML + Jinja 資訊清單編譯成 Ninja 建置計畫。
cli.long_about = Netsuke 把 YAML + Jinja 資訊清單轉換成可重現的 Ninja 圖，並以安全的預設值執行 Ninja。
cli.usage = { $usage }

# 全域選項的說明文字。
cli.flag.file.help = 要使用的 Netsuke 資訊清單檔案路徑。
cli.flag.directory.help = 以在此目錄中啟動的方式執行。
cli.flag.config.help = 設定檔的路徑，略過自動搜尋。
cli.flag.jobs.help = 設定平行建置工作的數量。
cli.flag.verbose.help = 啟用詳細的診斷記錄與完成時的耗時摘要。
cli.flag.locale.help = 命令列文字的地區設定標記（例如：en-US、zh-Hant）。
cli.flag.fetch_allow_scheme.help = fetch 輔助函式額外允許的 URL 通訊協定。
cli.flag.fetch_allow_host.help = 啟用預設拒絕時仍然允許的主機名稱。
cli.flag.fetch_block_host.help = 一律封鎖的主機名稱，即使在別處獲得允許。
cli.flag.fetch_default_deny.help = 預設拒絕所有主機；只放行所宣告的允許清單。
cli.flag.json.help = 輸出機器可讀的 JSON。
cli.flag.no_input.help = 絕不讀取互動式輸入。
cli.flag.color.help = 彩色輸出原則（auto、always、never）。
cli.flag.emoji.help = 表情符號原則（auto、always、never）。
cli.flag.progress.help = 進度顯示原則（auto、always、never）。
cli.flag.accessibility.help = 無障礙輸出原則（auto、on、off）。
cli.flag.default_targets.help = 未指定目標時採用的預設建置目標。

# 子命令說明。
cli.subcommand.build.about = 建置資訊清單中定義的目標（預設）。
cli.subcommand.build.long_about = 建置所要求的目標；若未指定，則採用資訊清單的預設目標。
cli.subcommand.clean.about = 透過 Ninja 移除建置產物。
cli.subcommand.clean.long_about = 產生暫存的 Ninja 檔案，接著執行 `ninja -t clean`。
cli.subcommand.graph.about = 輸出建置相依性圖。預設格式為 DOT。
cli.subcommand.graph.long_about = 將剖析後的 Netsuke 資訊清單投影為正規的建置圖，並寫成 Graphviz DOT；加上 `--html` 時則寫成自足的 HTML 頁面。使用 `--output <檔案>` 寫入檔案；`-` 會寫入標準輸出。
cli.subcommand.generate.about = 產生 Ninja 資訊清單但不執行 Ninja。
cli.subcommand.generate.long_about = 將產生的 Ninja 資訊清單寫入標準輸出，或寫入以 `--output` 選定的檔案。
cli.subcommand.help.about = 列印頂層說明，或列印指定主題的說明。
cli.subcommand.help.long_about = 沒有主題時，此命令等同於 `--help`。使用 `help targets` 列印所選資訊清單的目標和動作目錄。

# Help catalogue headings and markers.
cli.help.actions_heading = 動作：
cli.help.targets_heading = 目標：
cli.help.targets.about = 列出所選資訊清單中的目標和動作。
cli.help.default_marker = 預設
cli.help.conditional_marker = 條件式

# build 子命令選項的說明文字。
cli.subcommand.build.flag.targets.help = 要建置的目標（省略時採用資訊清單的預設值）。

# graph 子命令選項的說明文字。
cli.subcommand.graph.flag.html.help = 將圖算繪為自足的 HTML 頁面，而非 DOT。
cli.subcommand.graph.flag.output.help = 將圖產物寫入檔案；標準輸出請使用 `-`。

# generate 子命令選項的說明文字。
cli.subcommand.generate.flag.output.help = 將產生的 Ninja 資訊清單寫入檔案，而非標準輸出。

# 命令列驗證錯誤。
cli.validation.jobs.invalid_number = { $value } 不是有效的數字。
cli.validation.jobs.out_of_range = 工作數必須介於 { $min } 與 { $max } 之間。
cli.validation.scheme.empty = 通訊協定不得為空。
cli.validation.scheme.invalid_start = 通訊協定「{ $scheme }」必須以 ASCII 字母開頭。
cli.validation.scheme.invalid = 無效的通訊協定「{ $scheme }」。
cli.validation.locale.empty = 地區設定標記不得為空。
cli.validation.locale.invalid = 無效的地區設定標記「{ $locale }」。
cli.validation.color.invalid = 無效的色彩原則「{ $value }」。有效值：auto、always、never。
cli.validation.emoji.invalid = 無效的表情符號原則「{ $value }」。有效值：auto、always、never。
cli.validation.progress.invalid = 無效的進度原則「{ $value }」。有效值：auto、always、never。
cli.validation.accessibility.invalid = 無效的無障礙原則「{ $value }」。有效值：auto、on、off。
cli.validation.config.expected_object = 命令列的值本應序列化為物件，卻得到 { $value }。

# Clap 的錯誤訊息。
clap-error-missing-argument = 缺少必要的引數：{ $argument }
clap-error-missing-subcommand = 缺少子命令。可用的選項：{ $valid_subcommands }
clap-error-unknown-argument = 未知的引數：{ $argument }
clap-error-invalid-value = { $argument } 的值無效：{ $value }
clap-error-invalid-subcommand = 未知的子命令：{ $subcommand }
# 注意：value-validation 的措辭與 invalid-value 不同，以便區分自訂驗證器的
# 失敗（ErrorKind::ValueValidation）與型別不符（ErrorKind::InvalidValue）。
clap-error-value-validation = { $argument } 驗證失敗：{ $value }

# 執行期的錯誤與脈絡。
runner.manifest.not_found = 在 { $directory } 中找不到資訊清單「{ $manifest_name }」。
runner.manifest.not_found.help = 請確認資訊清單存在，或以正確的路徑指定 `--file`。
runner.manifest.path_missing_name = 資訊清單路徑「{ $path }」沒有檔名。
cli.file.non_utf8 = 資訊清單路徑「{ $path }」不是有效的 UTF-8。
runner.manifest.directory_label = 目錄 `{ $directory }`
runner.manifest.current_directory_label = 目前的目錄
runner.manifest.default_not_declared = 資訊清單預設值「{ $default }」未指定已宣告的動作或目標。
runner.context.network_policy = 無法建立網路原則。
runner.context.load_manifest = 無法載入 { $path } 的資訊清單。
runner.context.serialise_manifest = 無法序列化資訊清單。
runner.context.build_graph = 無法依資訊清單建立圖。
runner.context.generate_ninja = 無法產生 Ninja 資訊清單。
runner.context.render_graph = 無法算繪圖產物。

runner.io.create_temp_file = 無法建立暫存的 Ninja 檔案。
runner.io.write_temp_ninja = 無法寫入暫存的 Ninja 檔案。
runner.io.flush_temp_ninja = 無法清空暫存 Ninja 檔案的緩衝區。
runner.io.sync_temp_ninja = 無法同步暫存的 Ninja 檔案。
runner.io.create_parent_dir = 無法建立上層目錄 { $path }。
runner.io.create_ninja_file = 無法在 { $path } 建立 Ninja 檔案。
runner.io.write_ninja_file = 無法寫入 { $path } 的 Ninja 檔案。
runner.io.flush_ninja_file = 無法清空 { $path } Ninja 檔案的緩衝區。
runner.io.sync_ninja_file = 無法同步 { $path } 的 Ninja 檔案。
runner.io.open_ambient_dir = 無法開啟周邊目錄。
cli.directory.non_utf8 = 工作目錄路徑不是有效的 UTF-8。 ({ $path })
runner.io.no_existing_ancestor = { $path } 沒有既有的上層目錄。
runner.io.derive_relative_path = 無法推導 Ninja 的相對路徑。
runner.io.non_utf8_path = 不支援非 UTF-8 的路徑（路徑：{ $path }）。
runner.io.write_stdout = 無法將 Ninja 資訊清單寫入標準輸出。
runner.io.flush_stdout = 無法清空標準輸出的緩衝區。
runner.io.dyndep.create_dir = 無法建立 dyndep 目錄 { $path }。
runner.io.dyndep.read = 無法讀取 { $path } 處產生的 dyndep 檔案。
runner.io.dyndep.write = 無法寫入 { $path } 處產生的 dyndep 檔案。
runner.io.dyndep.rename = 無法重新命名在 { $path } 處產生的 dyndep 檔案。
runner.io.dyndep.corrupt = { $path } 處產生的 dyndep 檔案與預期內容不符；請只刪除該檔案後重試。
runner.io.dyndep.temp_collisions = 多次發生名稱衝突後，仍無法為 { $path } 建立唯一的暫存 dyndep 檔案。
runner.io.dyndep.too_large = { $path } 處產生的 dyndep 檔案超過 { $limit } 位元組的驗證上限。

# 資訊清單診斷。
manifest.parse = 資訊清單剖析失敗。
manifest.structure_error = 資訊清單在 { $name } 處有結構錯誤：{ $details }
manifest.yaml.parse = 第 { $line } 行第 { $column } 列發生 YAML 剖析錯誤：{ $details }
manifest.yaml.label = 無效的 YAML
manifest.yaml.hint.tabs = YAML 不允許定位字元；縮排請使用空白。
manifest.yaml.hint.list_item = YAML 清單項目必須以「-」開頭並正確縮排。
manifest.yaml.hint.expected_colon = 這看起來是對應項目；索引鍵後缺少「:」。
manifest.yaml.hint.mapping_values = YAML 對應在「:」之後需要一個值（或巢狀區塊）。
manifest.yaml.hint.invalid_token = YAML 記號無效或出乎意料。
manifest.yaml.hint.escape = 請逸出反斜線，或移除無效的逸出序列。
manifest.env.missing = 未設定必要的環境變數。
manifest.env.invalid_utf8 = 環境變數含有無效的 UTF-8。
manifest.vars.not_object = 資訊清單的 `vars` 必須是對應或物件。
manifest.vars.reserved_name = 清單的 `vars` 鍵 '{ $name }' 已保留給內建範本輔助函式；請重新命名該變數。
manifest.read_failed = 無法讀取 { $path } 的資訊清單。
manifest.resolve_workspace_root = 無法判定工作區的根目錄。
manifest.workspace_non_utf8 = 工作區根路徑「{ $path }」不是有效的 UTF-8。
manifest.path_non_utf8 = 資訊清單「{ $manifest }」的路徑不是有效的 UTF-8：{ $path }。
manifest.path_missing_name = 資訊清單路徑「{ $path }」沒有檔名。
manifest.open_workspace_failed = 無法為資訊清單 { $manifest } 開啟工作區 { $workspace }。
manifest.foreach.not_iterable = `foreach` 運算式無法逐一走訪。
manifest.foreach.serialise_item = 無法序列化 `foreach` 的元素。
manifest.when.empty = `when` 運算式不得為空。
manifest.when.eval_error = 無法求值 `when` 運算式「{ $expr }」。
manifest.when.template_error = 無法算繪 `when` 範本「{ $expr }」。
manifest.target.vars_not_object = 目標的 `vars` 必須是物件，卻得到 { $value }。
manifest.vars.entry_not_object = 資訊清單的 `vars` 項目必須是物件。
manifest.field_not_string = 欄位「{ $field }」必須是字串。
manifest.expression.parse_error = 無法剖析 { $name } 運算式。
manifest.expression.eval_error = 無法求值 { $name } 運算式。

# 資訊清單巨集的診斷。
manifest.macro.signature_missing_identifier = 巨集簽章缺少識別碼。
manifest.macro.signature_missing_params = 巨集簽章缺少參數。
manifest.macro.compile_failed = 無法編譯巨集 { $name }。
manifest.macro.sequence_invalid = 巨集必須定義為名稱對範本的對應。
manifest.macro.register_failed = 無法註冊資訊清單中的巨集。
manifest.macro.not_initialised = 巨集環境尚未初始化。
manifest.macro.caller_invalid = 巨集的呼叫端必須是字串。
manifest.macro.template_load_failed = 無法載入巨集範本。
manifest.macro.init_failed = 無法初始化巨集環境。
manifest.macro.missing = 缺少巨集 { $name }。

# 資訊清單的 glob 錯誤。
manifest.glob.unmatched_brace = 無效的 glob 樣式「{ $pattern }」：位置 { $position } 的「{ $character }」沒有配對。
manifest.glob.invalid_pattern = 無效的 glob 樣式「{ $pattern }」：{ $detail }。
manifest.glob.unknown_pattern_error = 未知的樣式錯誤。
manifest.glob.io_failed = 對「{ $pattern }」執行 glob 失敗：{ $detail }。
manifest.glob.unknown_io_error = 未知的輸入輸出錯誤。
manifest.command_list_empty = 「command」欄位不得為空：請提供命令字串或非空清單。

# 中介表示法的錯誤。
ir.rule_not_found = 找不到目標「{ $target }」所參照的規則「{ $rule }」。
ir.multiple_rules = 目標「{ $target }」必須只參照一條規則，卻得到 { $rules }。
ir.empty_rule = 目標「{ $target }」必須參照一條規則。
ir.duplicate_outputs = 偵測到重複的輸出：{ $outputs }。
ir.circular_dependency = 偵測到循環相依：{ $cycle }。
ir.action_serialisation = 無法序列化動作：{ $details }。
ir.invalid_command = 命令中的插值無效：{ $snippet }。

# Ninja 產生錯誤。
ninja_gen.missing_action = 缺少建置邊所參照的動作「{ $id }」。
ninja_gen.format = 無法格式化 Ninja 資訊清單的輸出。
ninja_gen.dyndep_files_required = 此操作需要產生的 Ninja dyndep 檔案；請使用 `netsuke build`、`netsuke clean` 或 `netsuke generate` 以產生這些檔案。
ninja_gen.reserved_output_path = 路徑「{ $path }」已保留給 Netsuke 的序列相依性狀態。
ninja_gen.unsupported_path_character = 路徑「{ $path }」包含不支援的 Ninja 路徑字元「{ $character }」。

# 主機樣式驗證。
host_pattern.empty = 主機樣式不得為空。
host_pattern.contains_scheme = 主機樣式「{ $pattern }」不得含有 URL 通訊協定。
host_pattern.contains_slash = 主機樣式「{ $pattern }」不得含有「/」。
host_pattern.missing_suffix = 主機樣式「{ $pattern }」必須在「*.」之後帶有字尾。
host_pattern.empty_label = 主機樣式「{ $pattern }」含有空白標籤。
host_pattern.invalid_chars = 主機樣式「{ $pattern }」含有無效字元。
host_pattern.invalid_label_edge = 主機樣式「{ $pattern }」的標籤不得以「-」開頭或結尾。
host_pattern.label_too_long = 主機樣式「{ $pattern }」含有超過 63 個字元的標籤。
host_pattern.too_long = 主機樣式「{ $pattern }」超出 255 個字元的上限。

# 網路原則。
network_policy.scheme.empty = 通訊協定不得為空。
network_policy.scheme.invalid = 通訊協定「{ $scheme }」含有無效字元。
network_policy.allowlist.empty = 主機允許清單不得為空。
network_policy.scheme.not_allowed = 不允許使用通訊協定「{ $scheme }」。
network_policy.missing_host = URL 缺少主機。
network_policy.host.blocked = 主機「{ $host }」已被原則封鎖。
network_policy.host.not_allowlisted = 主機「{ $host }」不在允許清單中。

# 標準函式庫設定。
stdlib.config.default_fetch_cache_invalid = fetch 快取的預設路徑必須是相對路徑。
stdlib.config.default_which_cache_invalid = which 快取的預設容量必須為正數。
stdlib.config.workspace_root_absolute = 工作區的根路徑必須是絕對路徑。
stdlib.config.fetch_response_limit_positive = fetch 的回應上限必須為正數。
stdlib.config.command_output_limit_positive = 命令輸出的擷取上限必須為正數。
stdlib.config.command_stream_limit_positive = 命令的串流上限必須為正數。
stdlib.config.which_cache_capacity_positive = which 快取的容量必須為正數。
stdlib.config.skip_dir_empty = 略過目錄的項目不得為空。
stdlib.config.skip_dir_navigation = 略過目錄的項目不得含有「..」。
stdlib.config.skip_dir_separator = 略過目錄的項目不得含有路徑分隔字元。
stdlib.config.fetch_cache_empty = fetch 快取的路徑不得為空。
stdlib.config.fetch_cache_not_relative = fetch 快取的路徑必須是相對路徑，卻得到 { $path }。
stdlib.config.fetch_cache_escapes = fetch 快取的路徑不得逸出工作區：{ $path }。
stdlib.config.open_workspace_root = 無法將目前的目錄開啟為 stdlib 工作區的根目錄。
stdlib.config.resolve_cwd = 無法將目前的目錄判定為 stdlib 工作區的根目錄。
stdlib.config.cwd_non_utf8 = 目前的目錄含有非 UTF-8 的部分：{ $path }。

# fetch 輔助函式的診斷。
stdlib.fetch.url_invalid = 無效的 URL「{ $url }」：{ $details }。
stdlib.fetch.disallowed = 不允許使用 URL「{ $url }」：{ $details }。
stdlib.fetch.failed = 無法取得「{ $url }」：{ $details }。
stdlib.fetch.redirect_loop = Redirect loop detected at '{ $url }'.
stdlib.fetch.redirect_limit_exceeded = Redirect limit of { $limit } exceeded while fetching '{ $url }'.
stdlib.fetch.redirect_location_invalid = Invalid redirect location '{ $location }' from '{ $url }': { $details }.
stdlib.fetch.redirect_disallowed = Redirect URL '{ $url }' is disallowed: { $details }.
stdlib.fetch.redirect_location_missing = Redirect response from '{ $url }' did not include a Location header.
stdlib.fetch.cache_read_failed = 無法讀取快取項目「{ $name }」：{ $details }。
stdlib.fetch.cache_open_failed = 無法開啟快取項目「{ $name }」：{ $details }。
stdlib.fetch.response_read_failed = 無法讀取來自「{ $url }」的回應：{ $details }。
stdlib.fetch.response_buffer_overflow = 讀取「{ $url }」時緩衝區溢位。
stdlib.fetch.cache_write_failed = 無法寫入「{ $url }」的快取：{ $details }。
stdlib.fetch.response_limit_exceeded = 來自「{ $url }」的回應超過 { $limit } 位元組的上限。
stdlib.fetch.cache_limit_exceeded = 快取的回應「{ $name }」超過 { $limit } 位元組的上限。
stdlib.fetch.io_failed = 對 { $path } 執行「{ $action }」失敗：{ $details }。
stdlib.fetch.action.sync_cache = 同步 fetch 快取
stdlib.fetch.action.create_cache_dir = 建立 fetch 快取目錄
stdlib.fetch.action.open_cache_dir = 開啟 fetch 快取目錄
stdlib.fetch.action.stat_cache = 查詢 fetch 快取項目資訊
stdlib.fetch.action.open_cache_entry = 開啟 fetch 快取項目

# 命令輔助函式的診斷。
stdlib.command.location = 範本「{ $template }」中的命令「{ $command }」
stdlib.command.spawn_failed = 無法啟動 { $location }：{ $details }。
stdlib.command.io_failed = { $location } 失敗：{ $details }。
stdlib.command.closed_input_early = 尚未寫完命令，輸入就已關閉。
stdlib.command.broken_pipe = 執行 { $location } 時管線中斷：{ $details }。
stdlib.command.terminated_by_signal = { $location } 被信號終止。
stdlib.command.exited_with_status = { $location } 以狀態 { $status } 結束。
stdlib.command.output_limit_exceeded = { $location } 對 { $stream } 超過了 { $mode } 的 { $limit } 位元組上限。
stdlib.command.timeout = { $location } 超過 { $seconds } 秒的時限。
stdlib.command.exit_status_suffix = （結束狀態 { $status }）
stdlib.command.signal_suffix = （被信號終止）
stdlib.command.shell.empty = shell 命令不得為空。
stdlib.command.grep.empty_pattern = grep 樣式不得為空。
stdlib.command.grep.flags_not_string = grep 的旗標必須是字串。
stdlib.command.quote.invalid = 無法為 { $arg } 加上引號：{ $details }。
stdlib.command.quote.line_break = 含有歸位或換行字元的引數無法安全地加上引號。
stdlib.command.input_undefined = 輸入值未定義。
stdlib.command.tempfile.root_required = 建立命令暫存檔需要工作區的根目錄。
stdlib.command.tempfile.create_failed = 無法建立命令暫存檔：{ $details }。
stdlib.command.options.invalid_utf8 = 命令選項的索引鍵必須是有效的 UTF-8。
stdlib.command.option.mode_not_string = 輸出模式必須是字串。
stdlib.command.options.invalid_type = 命令選項必須是物件。
stdlib.command.output.mode_unsupported = 不支援的輸出模式「{ $mode }」。
stdlib.command.output.mode.capture = 擷取
stdlib.command.output.mode.streaming = 串流
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# 路徑輔助函式的診斷。
stdlib.path.io.failed = 對 { $path } 執行「{ $action }」失敗（{ $label }）。
stdlib.path.io.failed_with_detail = 對 { $path } 執行「{ $action }」失敗：{ $detail }。
stdlib.path.io.failed_with_label_and_detail = 對 { $path } 執行「{ $action }」失敗（{ $label }）：{ $detail }。
stdlib.path.io.not_found = 找不到
stdlib.path.io.permission_denied = 權限遭拒
stdlib.path.io.already_exists = 已經存在
stdlib.path.io.invalid_input = 無效的輸入
stdlib.path.io.invalid_data = 無效的資料
stdlib.path.io.timed_out = 已逾時
stdlib.path.io.interrupted = 已中斷
stdlib.path.io.would_block = 將會阻擋
stdlib.path.io.write_zero = 寫入零位元組
stdlib.path.io.unexpected_eof = 非預期的檔案結尾
stdlib.path.io.broken_pipe = 管線中斷
stdlib.path.io.connection_refused = 連線遭拒
stdlib.path.io.connection_reset = 連線遭重設
stdlib.path.io.connection_aborted = 連線遭中止
stdlib.path.io.not_connected = 尚未連線
stdlib.path.io.addr_in_use = 位址已被占用
stdlib.path.io.addr_not_available = 位址無法使用
stdlib.path.io.out_of_memory = 記憶體不足
stdlib.path.io.unsupported = 不支援
stdlib.path.io.file_too_large = 檔案過大
stdlib.path.io.resource_busy = 資源忙碌
stdlib.path.io.executable_busy = 可執行檔忙碌
stdlib.path.io.deadlock = 死結
stdlib.path.io.crosses_devices = 跨越裝置
stdlib.path.io.too_many_links = 連結過多
stdlib.path.io.invalid_filename = 無效的檔名
stdlib.path.io.arg_list_too_long = 引數清單過長
stdlib.path.io.stale_handle = 失效的網路檔案控制代碼
stdlib.path.io.storage_full = 儲存空間已滿
stdlib.path.io.not_seekable = 無法定位
stdlib.path.io.network_down = 網路已中斷
stdlib.path.io.network_unreachable = 網路無法連線
stdlib.path.io.host_unreachable = 主機無法連線
stdlib.path.io.other = 輸入輸出錯誤
stdlib.path.action.canonicalize = 正規化
stdlib.path.action.open_directory = 開啟目錄
stdlib.path.action.stat = 查詢資訊
stdlib.path.action.read = 讀取
stdlib.path.action.open_file = 開啟檔案
stdlib.path.with_suffix.empty_separator = with_suffix 需要非空的分隔字元。
stdlib.path.relative_to.mismatch = { $path } 不是相對於 { $root } 的路徑。
stdlib.path.expanduser.unsupported = 不支援針對特定使用者展開 ~。
stdlib.path.expanduser.no_home = 無法展開 ~：未設定任何家目錄環境變數。
stdlib.path.contents.unsupported_encoding = 不支援的編碼「{ $encoding }」。
stdlib.path.hash.unsupported_algorithm = 不支援的雜湊演算法「{ $algorithm }」。
stdlib.path.hash.unsupported_algorithm_legacy = 不支援的雜湊演算法「{ $algorithm }」（請啟用特性「{ $feature }」）。

# 集合輔助函式的診斷。
stdlib.collections.flatten.expected_sequence = flatten 預期序列元素，卻發現 { $kind }。
stdlib.collections.group_by.empty_attribute = group_by 需要非空的屬性。
stdlib.collections.group_by.unresolved = group_by 無法在型別為 { $kind } 的元素上解析「{ $attr }」。

# 時間輔助函式的診斷。
stdlib.time.offset.invalid = now 的位移「{ $offset }」無效：應為「+HH:MM[:SS]」或「Z」。
stdlib.time.timedelta.overflow = 累加 { $component } 時 timedelta 溢位。
stdlib.time.label.weeks = 週
stdlib.time.label.days = 天
stdlib.time.label.hours = 小時
stdlib.time.label.minutes = 分鐘
stdlib.time.label.seconds = 秒
stdlib.time.label.milliseconds = 毫秒
stdlib.time.label.microseconds = 微秒
stdlib.time.label.nanoseconds = 奈秒

# which 輔助函式的診斷。
stdlib.which.not_found = [netsuke::jinja::which::not_found] 檢查了 { $count } 個 PATH 項目後仍找不到命令「{ $command }」。預覽：{ $preview }
stdlib.which.not_found.hint.cwd_auto = PATH 中的空白區段會被忽略；若要納入工作目錄，請使用 cwd_mode="auto"。
stdlib.which.not_found.hint.cwd_always = 若要納入目前的目錄，請設定 cwd_mode="always"。
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] 「{ $path }」中的命令「{ $command }」不存在或無法執行。
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <空白>
stdlib.which.path_entry.non_utf8 = 第 { $index } 個 PATH 項目含有非 UTF-8 字元；Netsuke 需要 UTF-8 路徑。
stdlib.which.command.empty = which 需要非空的字串。
stdlib.which.cwd_mode.invalid = cwd_mode 必須是「auto」、「always」或「never」，卻得到「{ $mode }」。
stdlib.which.cwd.resolve_failed = 無法判定目前的目錄：{ $details }。
stdlib.which.cwd.non_utf8 = 目前的目錄含有非 UTF-8 的部分。
stdlib.which.canonicalize_failed = 無法正規化「{ $path }」：{ $details }。
stdlib.which.is_executable = 無法判斷「{ $path }」是否可執行：{ $details }。
stdlib.which.canonicalize_non_utf8 = 正規路徑含有非 UTF-8 的部分。
stdlib.which.workspace_non_utf8 = 解析命令「{ $command }」時，工作區路徑含有非 UTF-8 的部分：{ $path }。
stdlib.which.walkdir_error = 解析命令時走訪工作區發生錯誤：{ $details }。

# 標準函式庫註冊。
stdlib.register.open_dir = 無法為註冊 stdlib 開啟目前的目錄。
stdlib.register.resolve_dir = 無法為註冊 stdlib 判定目前的目錄。
stdlib.register.dir_non_utf8 = 目前的目錄含有非 UTF-8 的部分：{ $path }。

# 無障礙輸出模式的狀態回報。
status.state.pending = 等待中
status.state.running = 進行中
status.state.done = 已完成
status.state.failed = 已失敗
status.stage.label = 階段 { $current }/{ $total }：{ $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label }（{ $task_progress }）
status.task.progress_label = 工作 { $current }/{ $total }
status.task.progress_update = { $task }：{ $description }
status.stage.manifest_ingestion = 正在讀取資訊清單檔案
status.stage.initial_yaml_parsing = 正在剖析 YAML 文件
status.stage.template_expansion = 正在展開範本指示詞
status.stage.final_rendering = 正在反序列化並算繪資訊清單的值
status.stage.ir_generation_validation = 正在建立並驗證相依性圖
status.stage.ninja_synthesis = 正在合成 Ninja 建置計畫
status.stage.ninja_synthesis_execute = 正在合成 Ninja 計畫並執行 { $tool }
status.stage.graph_rendering = 正在算繪圖產物
status.stage.graph_rendering_with_tool = 正在算繪 { $tool }
status.complete = { $tool } 已完成。
status.timing.summary_header = 各階段耗時摘要：
status.timing.stage_line = - { $label }：{ $duration }
status.timing.total_line = 管線總耗時：{ $duration }
status.tool.build = 建置
status.tool.clean = 清理
status.tool.graph = 圖
status.tool.graph_html = 圖（HTML）
status.tool.generate = 產生
status.tool.help_targets = 目標說明

# 圖的 HTML 算繪文字。
graph.html.title = Netsuke 建置圖
graph.html.heading = Netsuke 建置圖
graph.html.description = 由 Netsuke 算繪的建置圖
graph.html.outline.summary = 目標與相依性（文字大綱）
graph.html.outline.no_inputs = 沒有輸入
graph.html.noscript.notice = JavaScript 已停用。上方的文字大綱即完整的圖；其後為 DOT 原始碼。

# 無障礙輸出的語意前置詞。
semantic.prefix.error = 錯誤：
semantic.prefix.warning = 警告：
semantic.prefix.success = 成功：
semantic.prefix.info = 資訊：
semantic.prefix.timing = 耗時：
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# 供譯者參考的複數形式範例。
# 中文沒有文法上的複數變化，因此 CLDR 只有 `other` 一個類別。
example.files_processed = { $count ->
   *[other] 已處理 { $count } 個檔案。
}

example.errors_found = { $count ->
    [0] 未發現錯誤。
   *[other] 發現 { $count } 個錯誤。
}
