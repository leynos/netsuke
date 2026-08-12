# Tài nguyên bản địa hoá cho giao diện dòng lệnh Netsuke.

cli.about = Netsuke biên dịch tệp kê khai YAML + Jinja thành kế hoạch dựng Ninja.
cli.long_about = Netsuke chuyển tệp kê khai YAML + Jinja thành đồ thị Ninja có thể tái lập rồi chạy Ninja với các giá trị mặc định an toàn.
cli.usage = { $usage }

# Văn bản trợ giúp cho các tuỳ chọn chung.
cli.flag.file.help = Đường dẫn tới tệp kê khai Netsuke cần dùng.
cli.flag.directory.help = Chạy như thể đã khởi động trong thư mục này.
cli.flag.config.help = Đường dẫn tới tệp cấu hình, bỏ qua việc tìm kiếm tự động.
cli.flag.jobs.help = Đặt số lượng tác vụ dựng chạy song song.
cli.flag.verbose.help = Bật nhật ký chẩn đoán chi tiết và bản tóm tắt thời gian khi hoàn tất.
cli.flag.locale.help = Thẻ ngôn ngữ cho văn bản dòng lệnh (ví dụ: en-US, vi).
cli.flag.fetch_allow_scheme.help = Các lược đồ URL bổ sung được phép cho hàm trợ giúp fetch.
cli.flag.fetch_allow_host.help = Tên máy chủ được phép khi bật chế độ từ chối mặc định.
cli.flag.fetch_block_host.help = Tên máy chủ luôn bị chặn, kể cả khi được phép ở nơi khác.
cli.flag.fetch_default_deny.help = Mặc định từ chối mọi máy chủ; chỉ cho phép danh sách đã khai báo.
cli.flag.json.help = Xuất dữ liệu JSON máy đọc được.
cli.flag.no_input.help = Không bao giờ đọc dữ liệu nhập tương tác.
cli.flag.color.help = Chính sách xuất màu (auto, always, never).
cli.flag.emoji.help = Chính sách biểu tượng cảm xúc (auto, always, never).
cli.flag.progress.help = Chính sách hiển thị tiến trình (auto, always, never).
cli.flag.accessibility.help = Chính sách xuất dữ liệu dễ tiếp cận (auto, on, off).
cli.flag.default_targets.help = Đích dựng mặc định khi không chỉ định đích nào.

# Mô tả các lệnh con.
cli.subcommand.build.about = Dựng các đích được khai báo trong tệp kê khai (mặc định).
cli.subcommand.build.long_about = Dựng các đích được yêu cầu; nếu không có đích nào, dùng các đích mặc định của tệp kê khai.
cli.subcommand.clean.about = Xoá sản phẩm dựng thông qua Ninja.
cli.subcommand.clean.long_about = Tạo một tệp Ninja tạm rồi chạy `ninja -t clean`.
cli.subcommand.graph.about = Xuất đồ thị phụ thuộc của quá trình dựng. Định dạng mặc định là DOT.
cli.subcommand.graph.long_about = Chiếu tệp kê khai Netsuke đã phân tích thành đồ thị dựng chuẩn tắc rồi ghi ở định dạng Graphviz DOT, hoặc thành trang HTML độc lập với `--html`. Dùng `--output <TỆP>` để ghi ra tệp; `-` ghi ra đầu ra chuẩn.
cli.subcommand.generate.about = Tạo tệp kê khai Ninja mà không chạy Ninja.
cli.subcommand.generate.long_about = Ghi tệp kê khai Ninja đã tạo ra đầu ra chuẩn hoặc ra tệp được chọn bằng `--output`.
cli.subcommand.help.about = In trợ giúp cấp cao nhất hoặc trợ giúp cho một chủ đề cụ thể.
cli.subcommand.help.long_about = Không có chủ đề, lệnh này tương ứng với `--help`. Dùng `help targets` để in danh mục mục tiêu và hành động cho tệp đã chọn.

# Help catalogue headings and markers.
cli.help.actions_heading = Hành động:
cli.help.targets_heading = Mục tiêu:
cli.help.targets_about = Liệt kê mục tiêu và hành động trong tệp kê khai đã chọn.
cli.help.default_marker = mặc định

# Văn bản trợ giúp cho tuỳ chọn của lệnh con build.
cli.subcommand.build.flag.targets.help = Các đích cần dựng (nếu bỏ trống sẽ dùng đích mặc định của tệp kê khai).

# Văn bản trợ giúp cho tuỳ chọn của lệnh con graph.
cli.subcommand.graph.flag.html.help = Kết xuất đồ thị thành trang HTML độc lập thay cho định dạng DOT.
cli.subcommand.graph.flag.output.help = Ghi sản phẩm đồ thị ra TỆP; dùng `-` cho đầu ra chuẩn.

# Văn bản trợ giúp cho tuỳ chọn của lệnh con generate.
cli.subcommand.generate.flag.output.help = Ghi tệp kê khai Ninja đã tạo ra TỆP thay vì đầu ra chuẩn.

# Lỗi kiểm tra ở dòng lệnh.
cli.validation.jobs.invalid_number = { $value } không phải là số hợp lệ.
cli.validation.jobs.out_of_range = Số lượng tác vụ phải nằm trong khoảng từ { $min } đến { $max }.
cli.validation.scheme.empty = Lược đồ không được để trống.
cli.validation.scheme.invalid_start = Lược đồ “{ $scheme }” phải bắt đầu bằng một chữ cái ASCII.
cli.validation.scheme.invalid = Lược đồ không hợp lệ: “{ $scheme }”.
cli.validation.locale.empty = Thẻ ngôn ngữ không được để trống.
cli.validation.locale.invalid = Thẻ ngôn ngữ không hợp lệ: “{ $locale }”.
cli.validation.color.invalid = Chính sách màu không hợp lệ: “{ $value }”. Lựa chọn hợp lệ: auto, always, never.
cli.validation.emoji.invalid = Chính sách biểu tượng cảm xúc không hợp lệ: “{ $value }”. Lựa chọn hợp lệ: auto, always, never.
cli.validation.progress.invalid = Chính sách tiến trình không hợp lệ: “{ $value }”. Lựa chọn hợp lệ: auto, always, never.
cli.validation.accessibility.invalid = Chính sách trợ năng không hợp lệ: “{ $value }”. Lựa chọn hợp lệ: auto, on, off.
cli.validation.config.expected_object = Các giá trị dòng lệnh lẽ ra phải được tuần tự hoá thành đối tượng, nhưng nhận được { $value }.

# Thông báo lỗi của Clap.
clap-error-missing-argument = Thiếu đối số bắt buộc: { $argument }
clap-error-missing-subcommand = Thiếu lệnh con. Các lựa chọn sẵn có: { $valid_subcommands }
clap-error-unknown-argument = Đối số không xác định: { $argument }
clap-error-invalid-value = Giá trị không hợp lệ cho { $argument }: { $value }
clap-error-invalid-subcommand = Lệnh con không xác định: { $subcommand }
# Lưu ý: value-validation được diễn đạt khác invalid-value để phân biệt lỗi của
# bộ kiểm tra riêng (ErrorKind::ValueValidation) với lỗi sai kiểu
# (ErrorKind::InvalidValue).
clap-error-value-validation = Kiểm tra thất bại cho { $argument }: { $value }

# Lỗi và ngữ cảnh khi chạy.
runner.manifest.not_found = Không tìm thấy tệp kê khai “{ $manifest_name }” trong { $directory }.
runner.manifest.not_found.help = Hãy chắc chắn tệp kê khai tồn tại, hoặc truyền `--file` với đường dẫn đúng.
runner.manifest.path_missing_name = Đường dẫn tệp kê khai “{ $path }” không có tên tệp.
runner.manifest.path_utf8 = Đường dẫn tệp kê khai “{ $path }” không phải UTF-8 hợp lệ.
runner.manifest.directory_utf8 = Đường dẫn thư mục tệp kê khai “{ $path }” không phải UTF-8 hợp lệ.
runner.manifest.directory_label = thư mục `{ $directory }`
runner.manifest.current_directory_label = thư mục hiện tại
runner.context.network_policy = Không dựng được chính sách mạng.
runner.context.load_manifest = Không nạp được tệp kê khai tại { $path }.
runner.context.serialise_manifest = Không tuần tự hoá được tệp kê khai.
runner.context.build_graph = Không dựng được đồ thị từ tệp kê khai.
runner.context.generate_ninja = Không tạo được tệp kê khai Ninja.
runner.context.render_graph = Không kết xuất được sản phẩm đồ thị.

runner.io.create_temp_file = Không tạo được tệp Ninja tạm.
runner.io.write_temp_ninja = Không ghi được tệp Ninja tạm.
runner.io.flush_temp_ninja = Không xả được bộ đệm của tệp Ninja tạm.
runner.io.sync_temp_ninja = Không đồng bộ được tệp Ninja tạm.
runner.io.create_parent_dir = Không tạo được thư mục cha { $path }.
runner.io.create_ninja_file = Không tạo được tệp Ninja tại { $path }.
runner.io.write_ninja_file = Không ghi được tệp Ninja tại { $path }.
runner.io.flush_ninja_file = Không xả được bộ đệm của tệp Ninja tại { $path }.
runner.io.sync_ninja_file = Không đồng bộ được tệp Ninja tại { $path }.
runner.io.open_ambient_dir = Không mở được thư mục xung quanh.
runner.io.no_existing_ancestor = Không có thư mục cha nào tồn tại cho { $path }.
runner.io.derive_relative_path = Không suy ra được đường dẫn Ninja tương đối.
runner.io.non_utf8_path = Không hỗ trợ đường dẫn không phải UTF-8 (đường dẫn: { $path }).
runner.io.write_stdout = Không ghi được tệp kê khai Ninja ra đầu ra chuẩn.
runner.io.flush_stdout = Không xả được bộ đệm đầu ra chuẩn.

# Chẩn đoán tệp kê khai.
manifest.parse = Phân tích tệp kê khai thất bại.
manifest.structure_error = Lỗi cấu trúc tệp kê khai tại { $name }: { $details }
manifest.yaml.parse = Lỗi phân tích YAML tại dòng { $line }, cột { $column }: { $details }
manifest.yaml.label = YAML không hợp lệ
manifest.yaml.hint.tabs = YAML không cho phép ký tự tab; hãy dùng dấu cách để thụt lề.
manifest.yaml.hint.list_item = Mục danh sách YAML phải bắt đầu bằng “-” và được thụt lề đúng.
manifest.yaml.hint.expected_colon = Đây có vẻ là một mục ánh xạ; thiếu “:” sau khoá.
manifest.yaml.hint.mapping_values = Ánh xạ YAML cần một giá trị sau “:” (hoặc một khối lồng nhau).
manifest.yaml.hint.invalid_token = Thẻ từ YAML không hợp lệ hoặc bất ngờ.
manifest.yaml.hint.escape = Hãy thoát dấu gạch chéo ngược hoặc bỏ các chuỗi thoát không hợp lệ.
manifest.env.missing = Một biến môi trường bắt buộc chưa được đặt.
manifest.env.invalid_utf8 = Một biến môi trường chứa UTF-8 không hợp lệ.
manifest.vars.not_object = Trường `vars` của tệp kê khai phải là ánh xạ hoặc đối tượng.
manifest.vars.reserved_name = Khóa `vars` '{ $name }' của tệp kê khai được dành riêng cho hàm trợ giúp mẫu tích hợp; hãy đổi tên biến.
manifest.read_failed = Không đọc được tệp kê khai tại { $path }.
manifest.resolve_workspace_root = Không xác định được gốc của không gian làm việc.
manifest.workspace_non_utf8 = Đường dẫn gốc của không gian làm việc “{ $path }” không phải UTF-8 hợp lệ.
manifest.path_non_utf8 = Đường dẫn của tệp kê khai “{ $manifest }” không phải UTF-8 hợp lệ: { $path }.
manifest.path_missing_name = Đường dẫn tệp kê khai “{ $path }” không có tên tệp.
manifest.open_workspace_failed = Không mở được không gian làm việc { $workspace } cho tệp kê khai { $manifest }.
manifest.foreach.not_iterable = Biểu thức `foreach` không duyệt được.
manifest.foreach.serialise_item = Không tuần tự hoá được phần tử của `foreach`.
manifest.when.empty = Biểu thức `when` không được để trống.
manifest.when.eval_error = Không tính được biểu thức `when` “{ $expr }”.
manifest.when.template_error = Không kết xuất được mẫu `when` “{ $expr }”.
manifest.target.vars_not_object = Trường `vars` của đích phải là đối tượng, nhưng nhận được { $value }.
manifest.vars.entry_not_object = Mục `vars` của tệp kê khai phải là đối tượng.
manifest.field_not_string = Trường “{ $field }” phải là chuỗi.
manifest.expression.parse_error = Không phân tích được biểu thức { $name }.
manifest.expression.eval_error = Không tính được biểu thức { $name }.

# Chẩn đoán macro của tệp kê khai.
manifest.macro.signature_missing_identifier = Chữ ký macro thiếu định danh.
manifest.macro.signature_missing_params = Chữ ký macro thiếu tham số.
manifest.macro.compile_failed = Không biên dịch được macro { $name }.
manifest.macro.sequence_invalid = Macro phải được khai báo dưới dạng ánh xạ từ tên sang mẫu.
manifest.macro.register_failed = Không đăng ký được các macro của tệp kê khai.
manifest.macro.not_initialised = Môi trường macro chưa được khởi tạo.
manifest.macro.caller_invalid = Bên gọi macro phải là chuỗi.
manifest.macro.template_load_failed = Không nạp được mẫu của macro.
manifest.macro.init_failed = Không khởi tạo được môi trường macro.
manifest.macro.missing = Thiếu macro { $name }.

# Lỗi mẫu glob của tệp kê khai.
manifest.glob.unmatched_brace = Mẫu glob không hợp lệ “{ $pattern }”: “{ $character }” không có ký tự tương ứng tại vị trí { $position }.
manifest.glob.invalid_pattern = Mẫu glob không hợp lệ “{ $pattern }”: { $detail }.
manifest.glob.unknown_pattern_error = lỗi mẫu không xác định.
manifest.glob.io_failed = Glob thất bại với “{ $pattern }”: { $detail }.
manifest.glob.unknown_io_error = lỗi vào/ra không xác định.
manifest.command_list_empty = Trường “command” không được để trống: hãy cung cấp một chuỗi lệnh hoặc một danh sách không rỗng.

# Lỗi của biểu diễn trung gian.
ir.rule_not_found = Không tìm thấy quy tắc “{ $rule }” mà đích “{ $target }” tham chiếu.
ir.multiple_rules = Đích “{ $target }” phải tham chiếu đúng một quy tắc, nhưng nhận được { $rules }.
ir.empty_rule = Đích “{ $target }” phải tham chiếu một quy tắc.
ir.duplicate_outputs = Phát hiện đầu ra trùng lặp: { $outputs }.
ir.circular_dependency = Phát hiện phụ thuộc vòng: { $cycle }.
ir.action_serialisation = Không tuần tự hoá được hành động: { $details }.
ir.invalid_command = Nội suy không hợp lệ trong lệnh: { $snippet }.

# Lỗi khi tạo tệp Ninja.
ninja_gen.missing_action = Thiếu hành động “{ $id }” mà một cạnh dựng tham chiếu.
ninja_gen.format = Không định dạng được đầu ra của tệp kê khai Ninja.

# Kiểm tra mẫu máy chủ.
host_pattern.empty = Mẫu máy chủ không được để trống.
host_pattern.contains_scheme = Mẫu máy chủ “{ $pattern }” không được chứa lược đồ URL.
host_pattern.contains_slash = Mẫu máy chủ “{ $pattern }” không được chứa “/”.
host_pattern.missing_suffix = Mẫu máy chủ “{ $pattern }” phải có hậu tố sau “*.”.
host_pattern.empty_label = Mẫu máy chủ “{ $pattern }” chứa một nhãn rỗng.
host_pattern.invalid_chars = Mẫu máy chủ “{ $pattern }” chứa ký tự không hợp lệ.
host_pattern.invalid_label_edge = Nhãn của mẫu máy chủ “{ $pattern }” không được bắt đầu hoặc kết thúc bằng “-”.
host_pattern.label_too_long = Mẫu máy chủ “{ $pattern }” chứa nhãn dài hơn 63 ký tự.
host_pattern.too_long = Mẫu máy chủ “{ $pattern }” vượt quá giới hạn 255 ký tự.

# Chính sách mạng.
network_policy.scheme.empty = Lược đồ không được để trống.
network_policy.scheme.invalid = Lược đồ “{ $scheme }” chứa ký tự không hợp lệ.
network_policy.allowlist.empty = Danh sách máy chủ được phép không được để trống.
network_policy.scheme.not_allowed = Lược đồ “{ $scheme }” không được phép.
network_policy.missing_host = URL thiếu máy chủ.
network_policy.host.blocked = Máy chủ “{ $host }” bị chính sách chặn.
network_policy.host.not_allowlisted = Máy chủ “{ $host }” không nằm trong danh sách được phép.

# Cấu hình thư viện chuẩn.
stdlib.config.default_fetch_cache_invalid = Đường dẫn bộ nhớ đệm fetch mặc định phải là tương đối.
stdlib.config.default_which_cache_invalid = Dung lượng bộ nhớ đệm which mặc định phải là số dương.
stdlib.config.workspace_root_absolute = Đường dẫn gốc của không gian làm việc phải là tuyệt đối.
stdlib.config.fetch_response_limit_positive = Giới hạn phản hồi của fetch phải là số dương.
stdlib.config.command_output_limit_positive = Giới hạn thu nhận đầu ra lệnh phải là số dương.
stdlib.config.command_stream_limit_positive = Giới hạn luồng lệnh phải là số dương.
stdlib.config.which_cache_capacity_positive = Dung lượng bộ nhớ đệm which phải là số dương.
stdlib.config.skip_dir_empty = Mục thư mục bị bỏ qua không được để trống.
stdlib.config.skip_dir_navigation = Mục thư mục bị bỏ qua không được chứa “..”.
stdlib.config.skip_dir_separator = Mục thư mục bị bỏ qua không được chứa dấu phân tách đường dẫn.
stdlib.config.fetch_cache_empty = Đường dẫn bộ nhớ đệm fetch không được để trống.
stdlib.config.fetch_cache_not_relative = Đường dẫn bộ nhớ đệm fetch phải là tương đối, nhưng nhận được { $path }.
stdlib.config.fetch_cache_escapes = Đường dẫn bộ nhớ đệm fetch không được ra ngoài không gian làm việc: { $path }.
stdlib.config.open_workspace_root = Không mở được thư mục hiện tại làm gốc không gian làm việc của stdlib.
stdlib.config.resolve_cwd = Không xác định được thư mục hiện tại làm gốc không gian làm việc của stdlib.
stdlib.config.cwd_non_utf8 = Thư mục hiện tại chứa phần không phải UTF-8: { $path }.

# Chẩn đoán của hàm trợ giúp fetch.
stdlib.fetch.url_invalid = URL không hợp lệ “{ $url }”: { $details }.
stdlib.fetch.disallowed = URL “{ $url }” không được phép: { $details }.
stdlib.fetch.failed = Không tải được “{ $url }”: { $details }.
stdlib.fetch.cache_read_failed = Không đọc được mục bộ nhớ đệm “{ $name }”: { $details }.
stdlib.fetch.cache_open_failed = Không mở được mục bộ nhớ đệm “{ $name }”: { $details }.
stdlib.fetch.response_read_failed = Không đọc được phản hồi từ “{ $url }”: { $details }.
stdlib.fetch.response_buffer_overflow = Tràn bộ đệm khi đọc “{ $url }”.
stdlib.fetch.cache_write_failed = Không ghi được bộ nhớ đệm cho “{ $url }”: { $details }.
stdlib.fetch.response_limit_exceeded = Phản hồi từ “{ $url }” vượt quá giới hạn { $limit } byte.
stdlib.fetch.cache_limit_exceeded = Phản hồi đã lưu đệm “{ $name }” vượt quá giới hạn { $limit } byte.
stdlib.fetch.io_failed = Hành động “{ $action }” thất bại với { $path }: { $details }.
stdlib.fetch.action.sync_cache = đồng bộ bộ nhớ đệm fetch
stdlib.fetch.action.create_cache_dir = tạo thư mục bộ nhớ đệm fetch
stdlib.fetch.action.open_cache_dir = mở thư mục bộ nhớ đệm fetch
stdlib.fetch.action.stat_cache = đọc thông tin mục bộ nhớ đệm fetch
stdlib.fetch.action.open_cache_entry = mở mục bộ nhớ đệm fetch

# Chẩn đoán của hàm trợ giúp lệnh.
stdlib.command.location = lệnh “{ $command }” trong mẫu “{ $template }”
stdlib.command.spawn_failed = Không khởi chạy được { $location }: { $details }.
stdlib.command.io_failed = { $location } thất bại: { $details }.
stdlib.command.closed_input_early = Đầu vào đã đóng trước khi hoàn tất việc ghi sang lệnh.
stdlib.command.broken_pipe = Đứt ống dẫn khi chạy { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } bị tín hiệu chấm dứt.
stdlib.command.exited_with_status = { $location } kết thúc với trạng thái { $status }.
stdlib.command.output_limit_exceeded = { $location } vượt quá giới hạn { $mode } là { $limit } byte cho { $stream }.
stdlib.command.timeout = { $location } vượt quá thời hạn { $seconds } giây.
stdlib.command.exit_status_suffix = (trạng thái thoát { $status })
stdlib.command.signal_suffix = (bị tín hiệu chấm dứt)
stdlib.command.shell.empty = Lệnh shell không được để trống.
stdlib.command.grep.empty_pattern = Mẫu grep không được để trống.
stdlib.command.grep.flags_not_string = Cờ của grep phải là chuỗi.
stdlib.command.quote.invalid = Không đặt được { $arg } trong dấu nháy: { $details }.
stdlib.command.quote.line_break = Đối số chứa ký tự về đầu dòng hoặc xuống dòng không thể đặt trong dấu nháy một cách an toàn.
stdlib.command.input_undefined = Giá trị đầu vào chưa được xác định.
stdlib.command.tempfile.root_required = Cần gốc của không gian làm việc để tạo tệp lệnh tạm.
stdlib.command.tempfile.create_failed = Không tạo được tệp lệnh tạm: { $details }.
stdlib.command.options.invalid_utf8 = Khoá tuỳ chọn của lệnh phải là UTF-8 hợp lệ.
stdlib.command.option.mode_not_string = Chế độ đầu ra phải là chuỗi.
stdlib.command.options.invalid_type = Các tuỳ chọn của lệnh phải là một đối tượng.
stdlib.command.output.mode_unsupported = Chế độ đầu ra không được hỗ trợ: “{ $mode }”.
stdlib.command.output.mode.capture = thu nhận
stdlib.command.output.mode.streaming = truyền luồng
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Chẩn đoán của hàm trợ giúp đường dẫn.
stdlib.path.io.failed = Hành động “{ $action }” thất bại với { $path } ({ $label }).
stdlib.path.io.failed_with_detail = Hành động “{ $action }” thất bại với { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = Hành động “{ $action }” thất bại với { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = không tìm thấy
stdlib.path.io.permission_denied = bị từ chối quyền truy cập
stdlib.path.io.already_exists = đã tồn tại
stdlib.path.io.invalid_input = đầu vào không hợp lệ
stdlib.path.io.invalid_data = dữ liệu không hợp lệ
stdlib.path.io.timed_out = hết thời gian chờ
stdlib.path.io.interrupted = bị ngắt
stdlib.path.io.would_block = sẽ gây chặn
stdlib.path.io.write_zero = ghi được không byte
stdlib.path.io.unexpected_eof = kết thúc tệp ngoài dự kiến
stdlib.path.io.broken_pipe = đứt ống dẫn
stdlib.path.io.connection_refused = kết nối bị từ chối
stdlib.path.io.connection_reset = kết nối bị đặt lại
stdlib.path.io.connection_aborted = kết nối bị huỷ
stdlib.path.io.not_connected = chưa kết nối
stdlib.path.io.addr_in_use = địa chỉ đang được dùng
stdlib.path.io.addr_not_available = địa chỉ không khả dụng
stdlib.path.io.out_of_memory = hết bộ nhớ
stdlib.path.io.unsupported = không được hỗ trợ
stdlib.path.io.file_too_large = tệp quá lớn
stdlib.path.io.resource_busy = tài nguyên đang bận
stdlib.path.io.executable_busy = tệp thực thi đang bận
stdlib.path.io.deadlock = bế tắc
stdlib.path.io.crosses_devices = vượt qua ranh giới thiết bị
stdlib.path.io.too_many_links = quá nhiều liên kết
stdlib.path.io.invalid_filename = tên tệp không hợp lệ
stdlib.path.io.arg_list_too_long = danh sách đối số quá dài
stdlib.path.io.stale_handle = handle tệp mạng đã cũ
stdlib.path.io.storage_full = bộ lưu trữ đã đầy
stdlib.path.io.not_seekable = không định vị được
stdlib.path.io.network_down = mạng không hoạt động
stdlib.path.io.network_unreachable = không tới được mạng
stdlib.path.io.host_unreachable = không tới được máy chủ
stdlib.path.io.other = lỗi vào/ra
stdlib.path.action.canonicalize = chuẩn hoá đường dẫn
stdlib.path.action.open_directory = mở thư mục
stdlib.path.action.stat = đọc thông tin
stdlib.path.action.read = đọc
stdlib.path.action.open_file = mở tệp
stdlib.path.with_suffix.empty_separator = with_suffix cần một dấu phân tách không rỗng.
stdlib.path.relative_to.mismatch = { $path } không tương đối so với { $root }.
stdlib.path.expanduser.unsupported = Không hỗ trợ mở rộng ~ cho một người dùng cụ thể.
stdlib.path.expanduser.no_home = Không mở rộng được ~: chưa đặt biến môi trường nào cho thư mục cá nhân.
stdlib.path.contents.unsupported_encoding = Bảng mã không được hỗ trợ: “{ $encoding }”.
stdlib.path.hash.unsupported_algorithm = Thuật toán băm không được hỗ trợ: “{ $algorithm }”.
stdlib.path.hash.unsupported_algorithm_legacy = Thuật toán băm không được hỗ trợ: “{ $algorithm }” (hãy bật tính năng “{ $feature }”).

# Chẩn đoán của các hàm trợ giúp tập hợp.
stdlib.collections.flatten.expected_sequence = flatten mong đợi các phần tử của một dãy nhưng lại gặp { $kind }.
stdlib.collections.group_by.empty_attribute = group_by cần một thuộc tính không rỗng.
stdlib.collections.group_by.unresolved = group_by không tìm được “{ $attr }” trên phần tử kiểu { $kind }.

# Chẩn đoán của các hàm trợ giúp thời gian.
stdlib.time.offset.invalid = Độ lệch now “{ $offset }” không hợp lệ: cần “+HH:MM[:SS]” hoặc “Z”.
stdlib.time.timedelta.overflow = Tràn số trong timedelta khi cộng thêm { $component }.
stdlib.time.label.weeks = tuần
stdlib.time.label.days = ngày
stdlib.time.label.hours = giờ
stdlib.time.label.minutes = phút
stdlib.time.label.seconds = giây
stdlib.time.label.milliseconds = mili giây
stdlib.time.label.microseconds = micro giây
stdlib.time.label.nanoseconds = nano giây

# Chẩn đoán của hàm trợ giúp which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] không tìm thấy lệnh “{ $command }” sau khi kiểm tra { $count } mục PATH. Xem trước: { $preview }
stdlib.which.not_found.hint.cwd_auto = Các đoạn rỗng trong PATH bị bỏ qua; dùng cwd_mode="auto" để tính cả thư mục làm việc.
stdlib.which.not_found.hint.cwd_always = Đặt cwd_mode="always" để tính cả thư mục hiện tại.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] lệnh “{ $command }” tại “{ $path }” không tồn tại hoặc không thực thi được.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <trống>
stdlib.which.path_entry.non_utf8 = Mục PATH số { $index } chứa ký tự không phải UTF-8; Netsuke yêu cầu đường dẫn UTF-8.
stdlib.which.command.empty = which cần một chuỗi không rỗng.
stdlib.which.cwd_mode.invalid = cwd_mode phải là “auto”, “always” hoặc “never”, nhưng nhận được “{ $mode }”.
stdlib.which.cwd.resolve_failed = Không xác định được thư mục hiện tại: { $details }.
stdlib.which.cwd.non_utf8 = Thư mục hiện tại chứa phần không phải UTF-8.
stdlib.which.canonicalize_failed = Không chuẩn hoá được “{ $path }”: { $details }.
stdlib.which.is_executable = Không xác định được “{ $path }” có thực thi được hay không: { $details }.
stdlib.which.canonicalize_non_utf8 = Đường dẫn chuẩn tắc chứa phần không phải UTF-8.
stdlib.which.workspace_non_utf8 = Đường dẫn không gian làm việc chứa phần không phải UTF-8 khi phân giải lệnh “{ $command }”: { $path }.
stdlib.which.walkdir_error = Lỗi khi duyệt không gian làm việc trong lúc phân giải lệnh: { $details }.

# Đăng ký thư viện chuẩn.
stdlib.register.open_dir = Không mở được thư mục hiện tại để đăng ký stdlib.
stdlib.register.resolve_dir = Không xác định được thư mục hiện tại để đăng ký stdlib.
stdlib.register.dir_non_utf8 = Thư mục hiện tại chứa phần không phải UTF-8: { $path }.

# Báo cáo trạng thái cho chế độ đầu ra dễ tiếp cận.
status.state.pending = đang chờ
status.state.running = đang chạy
status.state.done = xong
status.state.failed = thất bại
status.stage.label = Giai đoạn { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Tác vụ { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Đang đọc tệp kê khai
status.stage.initial_yaml_parsing = Đang phân tích tài liệu YAML
status.stage.template_expansion = Đang mở rộng các chỉ thị mẫu
status.stage.final_rendering = Đang giải tuần tự và kết xuất giá trị của tệp kê khai
status.stage.ir_generation_validation = Đang dựng và kiểm tra đồ thị phụ thuộc
status.stage.ninja_synthesis = Đang tổng hợp kế hoạch dựng Ninja
status.stage.ninja_synthesis_execute = Đang tổng hợp kế hoạch Ninja và chạy { $tool }
status.stage.graph_rendering = Đang kết xuất sản phẩm đồ thị
status.stage.graph_rendering_with_tool = Đang kết xuất { $tool }
status.complete = { $tool } đã hoàn tất.
status.timing.summary_header = Tóm tắt thời gian theo giai đoạn:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Tổng thời gian của dây chuyền: { $duration }
status.tool.build = Dựng
status.tool.clean = Dọn dẹp
status.tool.graph = Đồ thị
status.tool.graph_html = Đồ thị (HTML)
status.tool.generate = Tạo
status.tool.help_targets = Trợ giúp mục tiêu

# Chuỗi của bộ kết xuất đồ thị sang HTML.
graph.html.title = Đồ thị dựng của Netsuke
graph.html.heading = Đồ thị dựng của Netsuke
graph.html.description = Đồ thị dựng do Netsuke kết xuất
graph.html.outline.summary = Đích và phụ thuộc (dàn ý văn bản)
graph.html.outline.no_inputs = Không có đầu vào
graph.html.noscript.notice = JavaScript đang tắt. Dàn ý văn bản ở trên chứa toàn bộ đồ thị; mã nguồn DOT nằm bên dưới.

# Tiền tố ngữ nghĩa cho đầu ra dễ tiếp cận.
semantic.prefix.error = Lỗi:
semantic.prefix.warning = Cảnh báo:
semantic.prefix.success = Thành công:
semantic.prefix.info = Thông tin:
semantic.prefix.timing = Thời gian:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Ví dụ về dạng số nhiều cho người dịch.
# Tiếng Việt chỉ dùng một hạng CLDR (`other`), vì danh từ không đổi theo số.
example.files_processed = { $count ->
   *[other] Đã xử lý { $count } tệp.
}

example.errors_found = { $count ->
    [0] Không tìm thấy lỗi nào.
   *[other] Tìm thấy { $count } lỗi.
}
