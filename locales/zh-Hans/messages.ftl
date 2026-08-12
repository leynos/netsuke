# Netsuke 命令行的本地化资源（简体中文）。

cli.about = Netsuke 将 YAML + Jinja 清单编译为 Ninja 构建计划。
cli.long_about = Netsuke 把 YAML + Jinja 清单转换为可复现的 Ninja 图，并以安全的默认设置运行 Ninja。
cli.usage = { $usage }

# 全局选项的帮助文本。
cli.flag.file.help = 要使用的 Netsuke 清单文件路径。
cli.flag.directory.help = 按照在此目录中启动的方式运行。
cli.flag.config.help = 配置文件路径，跳过自动查找。
cli.flag.jobs.help = 设置并行构建任务的数量。
cli.flag.verbose.help = 启用详细的诊断日志和完成时的耗时摘要。
cli.flag.locale.help = 命令行文案的区域标记（例如：en-US、zh-Hans）。
cli.flag.fetch_allow_scheme.help = fetch 辅助函数额外允许的 URL 方案。
cli.flag.fetch_allow_host.help = 启用默认拒绝时仍然允许的主机名。
cli.flag.fetch_block_host.help = 始终阻止的主机名，即使在别处被允许。
cli.flag.fetch_default_deny.help = 默认拒绝所有主机；只放行声明的允许列表。
cli.flag.json.help = 输出机器可读的 JSON。
cli.flag.no_input.help = 绝不读取交互式输入。
cli.flag.color.help = 彩色输出策略（auto、always、never）。
cli.flag.emoji.help = 表情符号策略（auto、always、never）。
cli.flag.progress.help = 进度显示策略（auto、always、never）。
cli.flag.accessibility.help = 无障碍输出策略（auto、on、off）。
cli.flag.default_targets.help = 未指定目标时使用的默认构建目标。

# 子命令说明。
cli.subcommand.build.about = 构建清单中定义的目标（默认）。
cli.subcommand.build.long_about = 构建所请求的目标；若未指定，则使用清单中的默认目标。
cli.subcommand.clean.about = 通过 Ninja 删除构建产物。
cli.subcommand.clean.long_about = 生成临时 Ninja 文件，然后运行 `ninja -t clean`。
cli.subcommand.graph.about = 输出构建依赖图。默认格式为 DOT。
cli.subcommand.graph.long_about = 将解析后的 Netsuke 清单投影为规范的构建图，并写为 Graphviz DOT；使用 `--html` 时写为独立的 HTML 页面。使用 `--output <文件>` 写入文件；`-` 写入标准输出。
cli.subcommand.generate.about = 生成 Ninja 清单但不运行 Ninja。
cli.subcommand.generate.long_about = 将生成的 Ninja 清单写入标准输出，或写入用 `--output` 选定的文件。
cli.subcommand.help.about = 打印顶层帮助，或打印指定主题的帮助。
cli.subcommand.help.long_about = 没有主题时，此命令等价于 `--help`。使用 `help targets` 打印所选清单的目标和动作目录。

# Help catalogue headings and markers.
cli.help.actions_heading = 动作：
cli.help.targets_heading = 目标：
cli.help.targets.about = 列出所选清单中的目标和动作。
cli.help.default_marker = 默认

# build 子命令选项的帮助文本。
cli.subcommand.build.flag.targets.help = 要构建的目标（省略时使用清单中的默认目标）。

# graph 子命令选项的帮助文本。
cli.subcommand.graph.flag.html.help = 将图渲染为独立的 HTML 页面，而不是 DOT。
cli.subcommand.graph.flag.output.help = 将图产物写入文件；标准输出请使用 `-`。

# generate 子命令选项的帮助文本。
cli.subcommand.generate.flag.output.help = 将生成的 Ninja 清单写入文件，而不是标准输出。

# 命令行校验错误。
cli.validation.jobs.invalid_number = { $value } 不是有效的数字。
cli.validation.jobs.out_of_range = 任务数必须介于 { $min } 与 { $max } 之间。
cli.validation.scheme.empty = 方案不能为空。
cli.validation.scheme.invalid_start = 方案“{ $scheme }”必须以 ASCII 字母开头。
cli.validation.scheme.invalid = 无效的方案“{ $scheme }”。
cli.validation.locale.empty = 区域标记不能为空。
cli.validation.locale.invalid = 无效的区域标记“{ $locale }”。
cli.validation.color.invalid = 无效的颜色策略“{ $value }”。有效取值：auto、always、never。
cli.validation.emoji.invalid = 无效的表情符号策略“{ $value }”。有效取值：auto、always、never。
cli.validation.progress.invalid = 无效的进度策略“{ $value }”。有效取值：auto、always、never。
cli.validation.accessibility.invalid = 无效的无障碍策略“{ $value }”。有效取值：auto、on、off。
cli.validation.config.expected_object = 命令行的值本应序列化为对象，却得到 { $value }。

# Clap 的错误消息。
clap-error-missing-argument = 缺少必需的参数：{ $argument }
clap-error-missing-subcommand = 缺少子命令。可用选项：{ $valid_subcommands }
clap-error-unknown-argument = 未知参数：{ $argument }
clap-error-invalid-value = { $argument } 的取值无效：{ $value }
clap-error-invalid-subcommand = 未知子命令：{ $subcommand }
# 注意：value-validation 的措辞与 invalid-value 不同，以便区分自定义校验器的
# 失败（ErrorKind::ValueValidation）与类型不匹配（ErrorKind::InvalidValue）。
clap-error-value-validation = { $argument } 校验失败：{ $value }

# 运行时的错误与上下文。
runner.manifest.not_found = 在 { $directory } 中找不到清单“{ $manifest_name }”。
runner.manifest.not_found.help = 请确认清单存在，或用正确的路径指定 `--file`。
runner.manifest.path_missing_name = 清单路径“{ $path }”没有文件名。
runner.manifest.path_utf8 = 清单路径“{ $path }”不是有效的 UTF-8。
runner.manifest.directory_utf8 = 清单目录路径“{ $path }”不是有效的 UTF-8。
runner.manifest.directory_label = 目录 `{ $directory }`
runner.manifest.current_directory_label = 当前目录
runner.manifest.default_not_declared = 清单默认值“{ $default }”未指定已声明的动作或目标。
runner.context.network_policy = 无法构建网络策略。
runner.context.load_manifest = 无法加载 { $path } 处的清单。
runner.context.serialise_manifest = 无法序列化清单。
runner.context.build_graph = 无法根据清单构建图。
runner.context.generate_ninja = 无法生成 Ninja 清单。
runner.context.render_graph = 无法渲染图产物。

runner.io.create_temp_file = 无法创建临时 Ninja 文件。
runner.io.write_temp_ninja = 无法写入临时 Ninja 文件。
runner.io.flush_temp_ninja = 无法刷新临时 Ninja 文件的缓冲区。
runner.io.sync_temp_ninja = 无法同步临时 Ninja 文件。
runner.io.create_parent_dir = 无法创建父目录 { $path }。
runner.io.create_ninja_file = 无法在 { $path } 创建 Ninja 文件。
runner.io.write_ninja_file = 无法写入 { $path } 处的 Ninja 文件。
runner.io.flush_ninja_file = 无法刷新 { $path } 处 Ninja 文件的缓冲区。
runner.io.sync_ninja_file = 无法同步 { $path } 处的 Ninja 文件。
runner.io.open_ambient_dir = 无法打开周围目录。
runner.io.non_utf8_working_directory = 工作目录路径不是有效的 UTF-8。
runner.io.no_existing_ancestor = { $path } 没有已存在的上级目录。
runner.io.derive_relative_path = 无法推导 Ninja 的相对路径。
runner.io.non_utf8_path = 不支持非 UTF-8 路径（路径：{ $path }）。
runner.io.write_stdout = 无法将 Ninja 清单写入标准输出。
runner.io.flush_stdout = 无法刷新标准输出的缓冲区。
runner.io.dyndep.create_dir = 无法创建 dyndep 目录 { $path }。
runner.io.dyndep.read = 无法读取 { $path } 处生成的 dyndep 文件。
runner.io.dyndep.write = 无法写入 { $path } 处生成的 dyndep 文件。
runner.io.dyndep.rename = 无法完成 { $path } 处生成的 dyndep 文件。
runner.io.dyndep.corrupt = { $path } 处生成的 dyndep 文件与预期内容不匹配；请仅删除该文件后重试。
runner.io.dyndep.race = 另一个进程写入了 dyndep 文件 { $path }，但无法验证其内容。
runner.io.dyndep.temp_collisions = 多次发生名称冲突后，仍无法为 { $path } 创建唯一的临时 dyndep 文件。
runner.io.dyndep.too_large = { $path } 处生成的 dyndep 文件超过 { $limit } 字节的验证上限。

# 清单诊断。
manifest.parse = 清单解析失败。
manifest.structure_error = 清单在 { $name } 处存在结构错误：{ $details }
manifest.yaml.parse = 第 { $line } 行第 { $column } 列出现 YAML 解析错误：{ $details }
manifest.yaml.label = 无效的 YAML
manifest.yaml.hint.tabs = YAML 不允许制表符；缩进请使用空格。
manifest.yaml.hint.list_item = YAML 列表项必须以“-”开头并正确缩进。
manifest.yaml.hint.expected_colon = 这看起来是映射条目；键后缺少“:”。
manifest.yaml.hint.mapping_values = YAML 映射在“:”之后需要一个值（或嵌套块）。
manifest.yaml.hint.invalid_token = YAML 记号无效或出乎意料。
manifest.yaml.hint.escape = 请转义反斜杠，或删除无效的转义序列。
manifest.env.missing = 未设置必需的环境变量。
manifest.env.invalid_utf8 = 环境变量包含无效的 UTF-8。
manifest.vars.not_object = 清单的 `vars` 必须是映射或对象。
manifest.vars.reserved_name = 清单的 `vars` 键 '{ $name }' 已保留给内置模板辅助函数；请重命名该变量。
manifest.read_failed = 无法读取 { $path } 处的清单。
manifest.resolve_workspace_root = 无法确定工作区根目录。
manifest.workspace_non_utf8 = 工作区根路径“{ $path }”不是有效的 UTF-8。
manifest.path_non_utf8 = 清单“{ $manifest }”的路径不是有效的 UTF-8：{ $path }。
manifest.path_missing_name = 清单路径“{ $path }”没有文件名。
manifest.open_workspace_failed = 无法为清单 { $manifest } 打开工作区 { $workspace }。
manifest.foreach.not_iterable = `foreach` 表达式不可迭代。
manifest.foreach.serialise_item = 无法序列化 `foreach` 的元素。
manifest.when.empty = `when` 表达式不能为空。
manifest.when.eval_error = 无法求值 `when` 表达式“{ $expr }”。
manifest.when.template_error = 无法渲染 `when` 模板“{ $expr }”。
manifest.target.vars_not_object = 目标的 `vars` 必须是对象，却得到 { $value }。
manifest.vars.entry_not_object = 清单的 `vars` 条目必须是对象。
manifest.field_not_string = 字段“{ $field }”必须是字符串。
manifest.expression.parse_error = 无法解析 { $name } 表达式。
manifest.expression.eval_error = 无法求值 { $name } 表达式。

# 清单宏的诊断。
manifest.macro.signature_missing_identifier = 宏签名缺少标识符。
manifest.macro.signature_missing_params = 宏签名缺少参数。
manifest.macro.compile_failed = 无法编译宏 { $name }。
manifest.macro.sequence_invalid = 宏必须定义为从名称到模板的映射。
manifest.macro.register_failed = 无法注册清单中的宏。
manifest.macro.not_initialised = 宏环境尚未初始化。
manifest.macro.caller_invalid = 宏的调用方必须是字符串。
manifest.macro.template_load_failed = 无法加载宏模板。
manifest.macro.init_failed = 无法初始化宏环境。
manifest.macro.missing = 缺少宏 { $name }。

# 清单的 glob 错误。
manifest.glob.unmatched_brace = 无效的 glob 模式“{ $pattern }”：位置 { $position } 处的“{ $character }”没有配对。
manifest.glob.invalid_pattern = 无效的 glob 模式“{ $pattern }”：{ $detail }。
manifest.glob.unknown_pattern_error = 未知的模式错误。
manifest.glob.io_failed = 对“{ $pattern }”执行 glob 失败：{ $detail }。
manifest.glob.unknown_io_error = 未知的输入输出错误。
manifest.command_list_empty = “command”字段不能为空：请提供命令字符串或非空列表。

# 中间表示的错误。
ir.rule_not_found = 找不到目标“{ $target }”引用的规则“{ $rule }”。
ir.multiple_rules = 目标“{ $target }”必须只引用一条规则，却得到 { $rules }。
ir.empty_rule = 目标“{ $target }”必须引用一条规则。
ir.duplicate_outputs = 检测到重复输出：{ $outputs }。
ir.circular_dependency = 检测到循环依赖：{ $cycle }。
ir.action_serialisation = 无法序列化动作：{ $details }。
ir.invalid_command = 命令中的插值无效：{ $snippet }。

# Ninja 生成错误。
ninja_gen.missing_action = 缺少构建边引用的动作“{ $id }”。
ninja_gen.format = 无法格式化 Ninja 清单的输出。
ninja_gen.dyndep_files_required = 此构建需要生成的 Ninja 捆绑包；请使用 `netsuke build`、`netsuke clean` 或 `netsuke generate` 以生成 dyndep 文件。
ninja_gen.reserved_output_path = 路径“{ $path }”已保留给 Netsuke 的串行依赖状态。
ninja_gen.unsupported_path_character = 路径“{ $path }”包含不支持的 Ninja 路径字符“{ $character }”。

# 主机模式校验。
host_pattern.empty = 主机模式不能为空。
host_pattern.contains_scheme = 主机模式“{ $pattern }”不能包含 URL 方案。
host_pattern.contains_slash = 主机模式“{ $pattern }”不能包含“/”。
host_pattern.missing_suffix = 主机模式“{ $pattern }”必须在“*.”之后带有后缀。
host_pattern.empty_label = 主机模式“{ $pattern }”包含空标签。
host_pattern.invalid_chars = 主机模式“{ $pattern }”包含无效字符。
host_pattern.invalid_label_edge = 主机模式“{ $pattern }”的标签不能以“-”开头或结尾。
host_pattern.label_too_long = 主机模式“{ $pattern }”包含超过 63 个字符的标签。
host_pattern.too_long = 主机模式“{ $pattern }”超出 255 个字符的上限。

# 网络策略。
network_policy.scheme.empty = 方案不能为空。
network_policy.scheme.invalid = 方案“{ $scheme }”包含无效字符。
network_policy.allowlist.empty = 主机允许列表不能为空。
network_policy.scheme.not_allowed = 不允许使用方案“{ $scheme }”。
network_policy.missing_host = URL 缺少主机。
network_policy.host.blocked = 主机“{ $host }”已被策略阻止。
network_policy.host.not_allowlisted = 主机“{ $host }”不在允许列表中。

# 标准库配置。
stdlib.config.default_fetch_cache_invalid = fetch 缓存的默认路径必须是相对路径。
stdlib.config.default_which_cache_invalid = which 缓存的默认容量必须为正数。
stdlib.config.workspace_root_absolute = 工作区根路径必须是绝对路径。
stdlib.config.fetch_response_limit_positive = fetch 的响应上限必须为正数。
stdlib.config.command_output_limit_positive = 命令输出的捕获上限必须为正数。
stdlib.config.command_stream_limit_positive = 命令的流式上限必须为正数。
stdlib.config.which_cache_capacity_positive = which 缓存的容量必须为正数。
stdlib.config.skip_dir_empty = 跳过目录的条目不能为空。
stdlib.config.skip_dir_navigation = 跳过目录的条目不能包含“..”。
stdlib.config.skip_dir_separator = 跳过目录的条目不能包含路径分隔符。
stdlib.config.fetch_cache_empty = fetch 缓存的路径不能为空。
stdlib.config.fetch_cache_not_relative = fetch 缓存的路径必须是相对路径，却得到 { $path }。
stdlib.config.fetch_cache_escapes = fetch 缓存的路径不能越出工作区：{ $path }。
stdlib.config.open_workspace_root = 无法将当前目录作为 stdlib 工作区根目录打开。
stdlib.config.resolve_cwd = 无法将当前目录确定为 stdlib 工作区根目录。
stdlib.config.cwd_non_utf8 = 当前目录包含非 UTF-8 的部分：{ $path }。

# fetch 辅助函数的诊断。
stdlib.fetch.url_invalid = 无效的 URL“{ $url }”：{ $details }。
stdlib.fetch.disallowed = 不允许使用 URL“{ $url }”：{ $details }。
stdlib.fetch.failed = 无法获取“{ $url }”：{ $details }。
stdlib.fetch.cache_read_failed = 无法读取缓存条目“{ $name }”：{ $details }。
stdlib.fetch.cache_open_failed = 无法打开缓存条目“{ $name }”：{ $details }。
stdlib.fetch.response_read_failed = 无法读取来自“{ $url }”的响应：{ $details }。
stdlib.fetch.response_buffer_overflow = 读取“{ $url }”时缓冲区溢出。
stdlib.fetch.cache_write_failed = 无法写入“{ $url }”的缓存：{ $details }。
stdlib.fetch.response_limit_exceeded = 来自“{ $url }”的响应超过 { $limit } 字节的上限。
stdlib.fetch.cache_limit_exceeded = 缓存的响应“{ $name }”超过 { $limit } 字节的上限。
stdlib.fetch.io_failed = 对 { $path } 执行“{ $action }”失败：{ $details }。
stdlib.fetch.action.sync_cache = 同步 fetch 缓存
stdlib.fetch.action.create_cache_dir = 创建 fetch 缓存目录
stdlib.fetch.action.open_cache_dir = 打开 fetch 缓存目录
stdlib.fetch.action.stat_cache = 查询 fetch 缓存条目信息
stdlib.fetch.action.open_cache_entry = 打开 fetch 缓存条目

# 命令辅助函数的诊断。
stdlib.command.location = 模板“{ $template }”中的命令“{ $command }”
stdlib.command.spawn_failed = 无法启动 { $location }：{ $details }。
stdlib.command.io_failed = { $location } 失败：{ $details }。
stdlib.command.closed_input_early = 尚未写完命令，输入就已关闭。
stdlib.command.broken_pipe = 运行 { $location } 时管道中断：{ $details }。
stdlib.command.terminated_by_signal = { $location } 被信号终止。
stdlib.command.exited_with_status = { $location } 以状态 { $status } 退出。
stdlib.command.output_limit_exceeded = { $location } 对 { $stream } 超过了 { $mode } 的 { $limit } 字节上限。
stdlib.command.timeout = { $location } 超过 { $seconds } 秒的时限。
stdlib.command.exit_status_suffix = （退出状态 { $status }）
stdlib.command.signal_suffix = （被信号终止）
stdlib.command.shell.empty = shell 命令不能为空。
stdlib.command.grep.empty_pattern = grep 模式不能为空。
stdlib.command.grep.flags_not_string = grep 的标志必须是字符串。
stdlib.command.quote.invalid = 无法为 { $arg } 加引号：{ $details }。
stdlib.command.quote.line_break = 含有回车或换行的参数无法安全加引号。
stdlib.command.input_undefined = 输入值未定义。
stdlib.command.tempfile.root_required = 创建命令临时文件需要工作区根目录。
stdlib.command.tempfile.create_failed = 无法创建命令临时文件：{ $details }。
stdlib.command.options.invalid_utf8 = 命令选项的键必须是有效的 UTF-8。
stdlib.command.option.mode_not_string = 输出模式必须是字符串。
stdlib.command.options.invalid_type = 命令选项必须是对象。
stdlib.command.output.mode_unsupported = 不支持的输出模式“{ $mode }”。
stdlib.command.output.mode.capture = 捕获
stdlib.command.output.mode.streaming = 流式
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# 路径辅助函数的诊断。
stdlib.path.io.failed = 对 { $path } 执行“{ $action }”失败（{ $label }）。
stdlib.path.io.failed_with_detail = 对 { $path } 执行“{ $action }”失败：{ $detail }。
stdlib.path.io.failed_with_label_and_detail = 对 { $path } 执行“{ $action }”失败（{ $label }）：{ $detail }。
stdlib.path.io.not_found = 未找到
stdlib.path.io.permission_denied = 权限被拒绝
stdlib.path.io.already_exists = 已存在
stdlib.path.io.invalid_input = 无效输入
stdlib.path.io.invalid_data = 无效数据
stdlib.path.io.timed_out = 已超时
stdlib.path.io.interrupted = 已中断
stdlib.path.io.would_block = 将会阻塞
stdlib.path.io.write_zero = 写入零字节
stdlib.path.io.unexpected_eof = 意外的文件结尾
stdlib.path.io.broken_pipe = 管道中断
stdlib.path.io.connection_refused = 连接被拒绝
stdlib.path.io.connection_reset = 连接被重置
stdlib.path.io.connection_aborted = 连接被中止
stdlib.path.io.not_connected = 尚未连接
stdlib.path.io.addr_in_use = 地址已被占用
stdlib.path.io.addr_not_available = 地址不可用
stdlib.path.io.out_of_memory = 内存不足
stdlib.path.io.unsupported = 不受支持
stdlib.path.io.file_too_large = 文件过大
stdlib.path.io.resource_busy = 资源忙
stdlib.path.io.executable_busy = 可执行文件忙
stdlib.path.io.deadlock = 死锁
stdlib.path.io.crosses_devices = 跨越设备
stdlib.path.io.too_many_links = 链接过多
stdlib.path.io.invalid_filename = 无效的文件名
stdlib.path.io.arg_list_too_long = 参数列表过长
stdlib.path.io.stale_handle = 失效的网络文件句柄
stdlib.path.io.storage_full = 存储空间已满
stdlib.path.io.not_seekable = 无法定位
stdlib.path.io.network_down = 网络已中断
stdlib.path.io.network_unreachable = 网络不可达
stdlib.path.io.host_unreachable = 主机不可达
stdlib.path.io.other = 输入输出错误
stdlib.path.action.canonicalize = 规范化
stdlib.path.action.open_directory = 打开目录
stdlib.path.action.stat = 查询信息
stdlib.path.action.read = 读取
stdlib.path.action.open_file = 打开文件
stdlib.path.with_suffix.empty_separator = with_suffix 需要非空的分隔符。
stdlib.path.relative_to.mismatch = { $path } 不是相对于 { $root } 的路径。
stdlib.path.expanduser.unsupported = 不支持针对特定用户展开 ~。
stdlib.path.expanduser.no_home = 无法展开 ~：未设置任何主目录环境变量。
stdlib.path.contents.unsupported_encoding = 不支持的编码“{ $encoding }”。
stdlib.path.hash.unsupported_algorithm = 不支持的散列算法“{ $algorithm }”。
stdlib.path.hash.unsupported_algorithm_legacy = 不支持的散列算法“{ $algorithm }”（请启用特性“{ $feature }”）。

# 集合辅助函数的诊断。
stdlib.collections.flatten.expected_sequence = flatten 期望序列元素，却发现 { $kind }。
stdlib.collections.group_by.empty_attribute = group_by 需要非空的属性。
stdlib.collections.group_by.unresolved = group_by 无法在类型为 { $kind } 的元素上解析“{ $attr }”。

# 时间辅助函数的诊断。
stdlib.time.offset.invalid = now 的偏移“{ $offset }”无效：应为“+HH:MM[:SS]”或“Z”。
stdlib.time.timedelta.overflow = 累加 { $component } 时 timedelta 溢出。
stdlib.time.label.weeks = 周
stdlib.time.label.days = 天
stdlib.time.label.hours = 小时
stdlib.time.label.minutes = 分钟
stdlib.time.label.seconds = 秒
stdlib.time.label.milliseconds = 毫秒
stdlib.time.label.microseconds = 微秒
stdlib.time.label.nanoseconds = 纳秒

# which 辅助函数的诊断。
stdlib.which.not_found = [netsuke::jinja::which::not_found] 检查了 { $count } 个 PATH 条目后仍未找到命令“{ $command }”。预览：{ $preview }
stdlib.which.not_found.hint.cwd_auto = PATH 中的空段会被忽略；如需纳入工作目录，请使用 cwd_mode="auto"。
stdlib.which.not_found.hint.cwd_always = 如需纳入当前目录，请设置 cwd_mode="always"。
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] “{ $path }”中的命令“{ $command }”不存在或不可执行。
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <空>
stdlib.which.path_entry.non_utf8 = 第 { $index } 个 PATH 条目包含非 UTF-8 字符；Netsuke 需要 UTF-8 路径。
stdlib.which.command.empty = which 需要非空的字符串。
stdlib.which.cwd_mode.invalid = cwd_mode 必须是“auto”、“always”或“never”，却得到“{ $mode }”。
stdlib.which.cwd.resolve_failed = 无法确定当前目录：{ $details }。
stdlib.which.cwd.non_utf8 = 当前目录包含非 UTF-8 的部分。
stdlib.which.canonicalize_failed = 无法规范化“{ $path }”：{ $details }。
stdlib.which.is_executable = 无法判断“{ $path }”是否可执行：{ $details }。
stdlib.which.canonicalize_non_utf8 = 规范路径包含非 UTF-8 的部分。
stdlib.which.workspace_non_utf8 = 解析命令“{ $command }”时，工作区路径包含非 UTF-8 的部分：{ $path }。
stdlib.which.walkdir_error = 解析命令时遍历工作区出错：{ $details }。

# 标准库注册。
stdlib.register.open_dir = 无法为注册 stdlib 打开当前目录。
stdlib.register.resolve_dir = 无法为注册 stdlib 确定当前目录。
stdlib.register.dir_non_utf8 = 当前目录包含非 UTF-8 的部分：{ $path }。

# 无障碍输出模式的状态报告。
status.state.pending = 等待中
status.state.running = 进行中
status.state.done = 已完成
status.state.failed = 已失败
status.stage.label = 阶段 { $current }/{ $total }：{ $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label }（{ $task_progress }）
status.task.progress_label = 任务 { $current }/{ $total }
status.task.progress_update = { $task }：{ $description }
status.stage.manifest_ingestion = 正在读取清单文件
status.stage.initial_yaml_parsing = 正在解析 YAML 文档
status.stage.template_expansion = 正在展开模板指令
status.stage.final_rendering = 正在反序列化并渲染清单的取值
status.stage.ir_generation_validation = 正在构建并校验依赖图
status.stage.ninja_synthesis = 正在合成 Ninja 构建计划
status.stage.ninja_synthesis_execute = 正在合成 Ninja 计划并运行 { $tool }
status.stage.graph_rendering = 正在渲染图产物
status.stage.graph_rendering_with_tool = 正在渲染 { $tool }
status.complete = { $tool } 已完成。
status.timing.summary_header = 各阶段耗时汇总：
status.timing.stage_line = - { $label }：{ $duration }
status.timing.total_line = 流水线总耗时：{ $duration }
status.tool.build = 构建
status.tool.clean = 清理
status.tool.graph = 图
status.tool.graph_html = 图（HTML）
status.tool.generate = 生成
status.tool.help_targets = 目标帮助

# 图的 HTML 渲染文案。
graph.html.title = Netsuke 构建图
graph.html.heading = Netsuke 构建图
graph.html.description = 由 Netsuke 渲染的构建图
graph.html.outline.summary = 目标与依赖（文本大纲）
graph.html.outline.no_inputs = 无输入
graph.html.noscript.notice = JavaScript 已禁用。上面的文本大纲即完整的图；其后是 DOT 源码。

# 无障碍输出的语义前缀。
semantic.prefix.error = 错误：
semantic.prefix.warning = 警告：
semantic.prefix.success = 成功：
semantic.prefix.info = 信息：
semantic.prefix.timing = 耗时：
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# 供译者参考的复数形式示例。
# 中文没有语法上的复数变化，因此 CLDR 只有 `other` 一个类别。
example.files_processed = { $count ->
   *[other] 已处理 { $count } 个文件。
}

example.errors_found = { $count ->
    [0] 未发现错误。
   *[other] 发现 { $count } 个错误。
}
