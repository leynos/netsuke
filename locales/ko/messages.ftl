# Netsuke 명령줄의 지역화 리소스.

cli.about = Netsuke는 YAML + Jinja 매니페스트를 Ninja 빌드 계획으로 컴파일합니다.
cli.long_about = Netsuke는 YAML + Jinja 매니페스트를 재현 가능한 Ninja 그래프로 변환한 뒤 안전한 기본값으로 Ninja를 실행합니다.
cli.usage = { $usage }

# 전역 옵션의 도움말.
cli.flag.file.help = 사용할 Netsuke 매니페스트 파일의 경로입니다.
cli.flag.directory.help = 이 디렉터리에서 시작한 것처럼 실행합니다.
cli.flag.config.help = 자동 검색을 건너뛰고 사용할 설정 파일의 경로입니다.
cli.flag.jobs.help = 병렬로 실행할 빌드 작업 수를 지정합니다.
cli.flag.verbose.help = 상세 진단 로그와 완료 시점의 소요 시간 요약을 켭니다.
cli.flag.locale.help = 명령줄 문구의 로케일 태그입니다(예: en-US, ko).
cli.flag.fetch_allow_scheme.help = fetch 도우미에 추가로 허용할 URL 스킴입니다.
cli.flag.fetch_allow_host.help = 기본 거부가 켜져 있을 때 허용할 호스트 이름입니다.
cli.flag.fetch_block_host.help = 다른 곳에서 허용되더라도 항상 차단할 호스트 이름입니다.
cli.flag.fetch_default_deny.help = 기본적으로 모든 호스트를 거부하고 선언한 허용 목록만 통과시킵니다.
cli.flag.json.help = 기계가 읽을 수 있는 JSON을 출력합니다.
cli.flag.no_input.help = 대화형 입력을 절대 읽지 않습니다.
cli.flag.color.help = 색상 출력 정책(auto, always, never).
cli.flag.emoji.help = 이모지 정책(auto, always, never).
cli.flag.progress.help = 진행 상황 표시 정책(auto, always, never).
cli.flag.accessibility.help = 접근성 출력 정책(auto, on, off).
cli.flag.default_targets.help = 대상을 지정하지 않았을 때 사용할 기본 빌드 대상입니다.

# 하위 명령 설명.
cli.subcommand.build.about = 매니페스트에 정의된 대상을 빌드합니다(기본값).
cli.subcommand.build.long_about = 요청한 대상을 빌드하며, 지정하지 않으면 매니페스트의 기본 대상을 사용합니다.
cli.subcommand.clean.about = Ninja를 통해 빌드 산출물을 제거합니다.
cli.subcommand.clean.long_about = 임시 Ninja 파일을 만든 뒤 `ninja -t clean`을 실행합니다.
cli.subcommand.graph.about = 빌드 의존성 그래프를 출력합니다. 기본 형식은 DOT입니다.
cli.subcommand.graph.long_about = 해석한 Netsuke 매니페스트를 정규 빌드 그래프로 투영해 Graphviz DOT으로, 또는 `--html`을 지정하면 자체 완결형 HTML 페이지로 씁니다. 파일로 쓰려면 `--output <파일>`을 사용하고, `-`는 표준 출력으로 씁니다.
cli.subcommand.generate.about = Ninja를 실행하지 않고 Ninja 매니페스트를 생성합니다.
cli.subcommand.generate.long_about = 생성한 Ninja 매니페스트를 표준 출력이나 `--output`으로 고른 파일에 씁니다.
cli.subcommand.help.about = 최상위 도움말 또는 지정된 주제에 대한 도움말을 출력합니다.
cli.subcommand.help.long_about = 주제가 없으면 `--help`와 동일합니다. 선택한 파일의 대상 및 작업 카탈로그를 출력하려면 `help targets`를 사용하세요.

# Help catalogue headings and markers.
cli.help.actions_heading = 작업:
cli.help.targets_heading = 대상:
cli.help.targets.about = 선택한 파일의 대상 및 작업을 나열합니다.
cli.help.default_marker = 기본값

# build 하위 명령 옵션의 도움말.
cli.subcommand.build.flag.targets.help = 빌드할 대상입니다(생략하면 매니페스트의 기본값을 사용).

# graph 하위 명령 옵션의 도움말.
cli.subcommand.graph.flag.html.help = 그래프를 DOT 대신 자체 완결형 HTML 페이지로 렌더링합니다.
cli.subcommand.graph.flag.output.help = 그래프 산출물을 파일에 씁니다. 표준 출력에는 `-`를 사용하세요.

# generate 하위 명령 옵션의 도움말.
cli.subcommand.generate.flag.output.help = 생성한 Ninja 매니페스트를 표준 출력 대신 파일에 씁니다.

# 명령줄 검증 오류.
cli.validation.jobs.invalid_number = { $value }은(는) 유효한 숫자가 아닙니다.
cli.validation.jobs.out_of_range = 작업 수는 { $min }에서 { $max } 사이여야 합니다.
cli.validation.scheme.empty = 스킴은 비어 있을 수 없습니다.
cli.validation.scheme.invalid_start = 스킴 '{ $scheme }'은(는) ASCII 문자로 시작해야 합니다.
cli.validation.scheme.invalid = 잘못된 스킴 '{ $scheme }'입니다.
cli.validation.locale.empty = 로케일 태그는 비어 있을 수 없습니다.
cli.validation.locale.invalid = 잘못된 로케일 태그 '{ $locale }'입니다.
cli.validation.color.invalid = 잘못된 색상 정책 '{ $value }'입니다. 유효한 값: auto, always, never.
cli.validation.emoji.invalid = 잘못된 이모지 정책 '{ $value }'입니다. 유효한 값: auto, always, never.
cli.validation.progress.invalid = 잘못된 진행 상황 정책 '{ $value }'입니다. 유효한 값: auto, always, never.
cli.validation.accessibility.invalid = 잘못된 접근성 정책 '{ $value }'입니다. 유효한 값: auto, on, off.
cli.validation.config.expected_object = 명령줄 값이 객체로 직렬화되어야 하지만 { $value }이(가) 나왔습니다.

# Clap 오류 메시지.
clap-error-missing-argument = 필수 인자가 없습니다: { $argument }
clap-error-missing-subcommand = 하위 명령이 없습니다. 사용할 수 있는 값: { $valid_subcommands }
clap-error-unknown-argument = 알 수 없는 인자입니다: { $argument }
clap-error-invalid-value = { $argument }의 값이 잘못되었습니다: { $value }
clap-error-invalid-subcommand = 알 수 없는 하위 명령입니다: { $subcommand }
# 참고: value-validation은 사용자 정의 검증기의 실패(ErrorKind::ValueValidation)를
# 형식 불일치(ErrorKind::InvalidValue)와 구분하기 위해 invalid-value와 다르게
# 표현했습니다.
clap-error-value-validation = { $argument }의 검증에 실패했습니다: { $value }

# 실행 중 오류와 맥락.
runner.manifest.not_found = 매니페스트 '{ $manifest_name }'을(를) { $directory }에서 찾을 수 없습니다.
runner.manifest.not_found.help = 매니페스트가 있는지 확인하거나 올바른 경로로 `--file`을 지정하세요.
runner.manifest.path_missing_name = 매니페스트 경로 '{ $path }'에 파일 이름이 없습니다.
runner.manifest.path_utf8 = 매니페스트 경로 '{ $path }'은(는) 올바른 UTF-8이 아닙니다.
runner.manifest.directory_utf8 = 매니페스트 디렉터리 경로 '{ $path }'은(는) 올바른 UTF-8이 아닙니다.
runner.manifest.directory_label = `{ $directory }` 디렉터리
runner.manifest.current_directory_label = 현재 디렉터리
runner.context.network_policy = 네트워크 정책을 구성하지 못했습니다.
runner.context.load_manifest = { $path }의 매니페스트를 불러오지 못했습니다.
runner.context.serialise_manifest = 매니페스트를 직렬화하지 못했습니다.
runner.context.build_graph = 매니페스트로 그래프를 구성하지 못했습니다.
runner.context.generate_ninja = Ninja 매니페스트를 생성하지 못했습니다.
runner.context.render_graph = 그래프 산출물을 렌더링하지 못했습니다.

runner.io.create_temp_file = 임시 Ninja 파일을 만들지 못했습니다.
runner.io.write_temp_ninja = 임시 Ninja 파일에 쓰지 못했습니다.
runner.io.flush_temp_ninja = 임시 Ninja 파일의 버퍼를 비우지 못했습니다.
runner.io.sync_temp_ninja = 임시 Ninja 파일을 동기화하지 못했습니다.
runner.io.create_parent_dir = 상위 디렉터리 { $path }을(를) 만들지 못했습니다.
runner.io.create_ninja_file = { $path }에 Ninja 파일을 만들지 못했습니다.
runner.io.write_ninja_file = { $path }의 Ninja 파일에 쓰지 못했습니다.
runner.io.flush_ninja_file = { $path }의 Ninja 파일 버퍼를 비우지 못했습니다.
runner.io.sync_ninja_file = { $path }의 Ninja 파일을 동기화하지 못했습니다.
runner.io.open_ambient_dir = 주변 디렉터리를 열지 못했습니다.
runner.io.no_existing_ancestor = { $path }에 해당하는 상위 디렉터리가 없습니다.
runner.io.derive_relative_path = 상대 Ninja 경로를 유도하지 못했습니다.
runner.io.non_utf8_path = UTF-8이 아닌 경로는 지원하지 않습니다(경로: { $path }).
runner.io.write_stdout = Ninja 매니페스트를 표준 출력에 쓰지 못했습니다.
runner.io.flush_stdout = 표준 출력의 버퍼를 비우지 못했습니다.

# 매니페스트 진단.
manifest.parse = 매니페스트 해석에 실패했습니다.
manifest.structure_error = { $name }에서 매니페스트 구조 오류: { $details }
manifest.yaml.parse = { $line }행 { $column }열에서 YAML 해석 오류: { $details }
manifest.yaml.label = 잘못된 YAML
manifest.yaml.hint.tabs = YAML은 탭을 허용하지 않습니다. 들여쓰기에는 공백을 사용하세요.
manifest.yaml.hint.list_item = YAML 목록 항목은 '-'로 시작하고 올바르게 들여써야 합니다.
manifest.yaml.hint.expected_colon = 매핑 항목으로 보입니다. 키 뒤에 ':'이 없습니다.
manifest.yaml.hint.mapping_values = YAML 매핑은 ':' 뒤에 값(또는 중첩 블록)이 필요합니다.
manifest.yaml.hint.invalid_token = YAML 토큰이 잘못되었거나 예상 밖입니다.
manifest.yaml.hint.escape = 역슬래시를 이스케이프하거나 잘못된 이스케이프 시퀀스를 제거하세요.
manifest.env.missing = 필수 환경 변수가 설정되지 않았습니다.
manifest.env.invalid_utf8 = 환경 변수에 잘못된 UTF-8이 들어 있습니다.
manifest.vars.not_object = 매니페스트의 `vars`는 매핑이나 객체여야 합니다.
manifest.vars.reserved_name = 매니페스트의 `vars` 키 '{ $name }'은(는) 내장 템플릿 헬퍼용으로 예약되어 있습니다. 변수 이름을 바꾸십시오.
manifest.read_failed = { $path }의 매니페스트를 읽지 못했습니다.
manifest.resolve_workspace_root = 작업 공간의 루트를 확인하지 못했습니다.
manifest.workspace_non_utf8 = 작업 공간의 루트 경로 '{ $path }'은(는) 올바른 UTF-8이 아닙니다.
manifest.path_non_utf8 = 매니페스트 '{ $manifest }'의 경로가 올바른 UTF-8이 아닙니다: { $path }.
manifest.path_missing_name = 매니페스트 경로 '{ $path }'에 파일 이름이 없습니다.
manifest.open_workspace_failed = 매니페스트 { $manifest }을(를) 위해 작업 공간 { $workspace }을(를) 열지 못했습니다.
manifest.foreach.not_iterable = `foreach` 식은 순회할 수 없습니다.
manifest.foreach.serialise_item = `foreach`의 항목을 직렬화하지 못했습니다.
manifest.when.empty = `when` 식은 비어 있을 수 없습니다.
manifest.when.eval_error = `when` 식 '{ $expr }'을(를) 평가하지 못했습니다.
manifest.when.template_error = `when` 템플릿 '{ $expr }'을(를) 렌더링하지 못했습니다.
manifest.target.vars_not_object = 대상의 `vars`는 객체여야 하지만 { $value }이(가) 나왔습니다.
manifest.vars.entry_not_object = 매니페스트의 `vars` 항목은 객체여야 합니다.
manifest.field_not_string = '{ $field }' 필드는 문자열이어야 합니다.
manifest.expression.parse_error = { $name } 식을 해석하지 못했습니다.
manifest.expression.eval_error = { $name } 식을 평가하지 못했습니다.

# 매니페스트 매크로 진단.
manifest.macro.signature_missing_identifier = 매크로 시그니처에 식별자가 없습니다.
manifest.macro.signature_missing_params = 매크로 시그니처에 매개변수가 없습니다.
manifest.macro.compile_failed = 매크로 { $name }을(를) 컴파일하지 못했습니다.
manifest.macro.sequence_invalid = 매크로는 이름에서 템플릿으로의 매핑으로 정의해야 합니다.
manifest.macro.register_failed = 매니페스트의 매크로를 등록하지 못했습니다.
manifest.macro.not_initialised = 매크로 환경이 초기화되지 않았습니다.
manifest.macro.caller_invalid = 매크로 호출자는 문자열이어야 합니다.
manifest.macro.template_load_failed = 매크로 템플릿을 불러오지 못했습니다.
manifest.macro.init_failed = 매크로 환경을 초기화하지 못했습니다.
manifest.macro.missing = 매크로 { $name }이(가) 없습니다.

# 매니페스트 glob 오류.
manifest.glob.unmatched_brace = 잘못된 glob 패턴 '{ $pattern }': { $position } 위치의 '{ $character }'에 짝이 없습니다.
manifest.glob.invalid_pattern = 잘못된 glob 패턴 '{ $pattern }': { $detail }.
manifest.glob.unknown_pattern_error = 알 수 없는 패턴 오류.
manifest.glob.io_failed = '{ $pattern }'에 대한 glob이 실패했습니다: { $detail }.
manifest.glob.unknown_io_error = 알 수 없는 입출력 오류.
manifest.command_list_empty = 'command' 필드는 비어 있을 수 없습니다: 명령 문자열 또는 비어 있지 않은 목록을 지정하십시오.

# 중간 표현 오류.
ir.rule_not_found = 대상 '{ $target }'이(가) 참조하는 규칙 '{ $rule }'을(를) 찾을 수 없습니다.
ir.multiple_rules = 대상 '{ $target }'은(는) 규칙 하나만 참조해야 하지만 { $rules }이(가) 나왔습니다.
ir.empty_rule = 대상 '{ $target }'은(는) 규칙을 참조해야 합니다.
ir.duplicate_outputs = 중복된 출력이 발견되었습니다: { $outputs }.
ir.circular_dependency = 순환 의존성이 발견되었습니다: { $cycle }.
ir.action_serialisation = 동작을 직렬화하지 못했습니다: { $details }.
ir.invalid_command = 명령의 보간이 잘못되었습니다: { $snippet }.

# Ninja 생성 오류.
ninja_gen.missing_action = 빌드 간선이 참조하는 동작 '{ $id }'이(가) 없습니다.
ninja_gen.format = Ninja 매니페스트 출력의 서식을 지정하지 못했습니다.

# 호스트 패턴 검증.
host_pattern.empty = 호스트 패턴은 비어 있을 수 없습니다.
host_pattern.contains_scheme = 호스트 패턴 '{ $pattern }'에는 URL 스킴이 들어갈 수 없습니다.
host_pattern.contains_slash = 호스트 패턴 '{ $pattern }'에는 '/'가 들어갈 수 없습니다.
host_pattern.missing_suffix = 호스트 패턴 '{ $pattern }'에는 '*.' 뒤에 접미사가 있어야 합니다.
host_pattern.empty_label = 호스트 패턴 '{ $pattern }'에 빈 레이블이 있습니다.
host_pattern.invalid_chars = 호스트 패턴 '{ $pattern }'에 잘못된 문자가 있습니다.
host_pattern.invalid_label_edge = 호스트 패턴 '{ $pattern }'의 레이블은 '-'로 시작하거나 끝날 수 없습니다.
host_pattern.label_too_long = 호스트 패턴 '{ $pattern }'에 63자를 넘는 레이블이 있습니다.
host_pattern.too_long = 호스트 패턴 '{ $pattern }'이(가) 255자 제한을 넘습니다.

# 네트워크 정책.
network_policy.scheme.empty = 스킴은 비어 있을 수 없습니다.
network_policy.scheme.invalid = 스킴 '{ $scheme }'에 잘못된 문자가 있습니다.
network_policy.allowlist.empty = 호스트 허용 목록은 비어 있을 수 없습니다.
network_policy.scheme.not_allowed = 스킴 '{ $scheme }'은(는) 허용되지 않습니다.
network_policy.missing_host = URL에 호스트가 없습니다.
network_policy.host.blocked = 호스트 '{ $host }'은(는) 정책에 의해 차단되었습니다.
network_policy.host.not_allowlisted = 호스트 '{ $host }'은(는) 허용 목록에 없습니다.

# 표준 라이브러리 설정.
stdlib.config.default_fetch_cache_invalid = fetch 캐시의 기본 경로는 상대 경로여야 합니다.
stdlib.config.default_which_cache_invalid = which 캐시의 기본 용량은 양수여야 합니다.
stdlib.config.workspace_root_absolute = 작업 공간의 루트 경로는 절대 경로여야 합니다.
stdlib.config.fetch_response_limit_positive = fetch의 응답 한도는 양수여야 합니다.
stdlib.config.command_output_limit_positive = 명령 출력의 수집 한도는 양수여야 합니다.
stdlib.config.command_stream_limit_positive = 명령의 스트림 한도는 양수여야 합니다.
stdlib.config.which_cache_capacity_positive = which 캐시의 용량은 양수여야 합니다.
stdlib.config.skip_dir_empty = 건너뛸 디렉터리 항목은 비어 있을 수 없습니다.
stdlib.config.skip_dir_navigation = 건너뛸 디렉터리 항목에는 '..'이 들어갈 수 없습니다.
stdlib.config.skip_dir_separator = 건너뛸 디렉터리 항목에는 경로 구분자가 들어갈 수 없습니다.
stdlib.config.fetch_cache_empty = fetch 캐시의 경로는 비어 있을 수 없습니다.
stdlib.config.fetch_cache_not_relative = fetch 캐시의 경로는 상대 경로여야 하지만 { $path }이(가) 나왔습니다.
stdlib.config.fetch_cache_escapes = fetch 캐시의 경로는 작업 공간을 벗어날 수 없습니다: { $path }.
stdlib.config.open_workspace_root = 현재 디렉터리를 stdlib 작업 공간의 루트로 열지 못했습니다.
stdlib.config.resolve_cwd = 현재 디렉터리를 stdlib 작업 공간의 루트로 확인하지 못했습니다.
stdlib.config.cwd_non_utf8 = 현재 디렉터리에 UTF-8이 아닌 부분이 있습니다: { $path }.

# fetch 도우미 진단.
stdlib.fetch.url_invalid = 잘못된 URL '{ $url }': { $details }.
stdlib.fetch.disallowed = URL '{ $url }'은(는) 허용되지 않습니다: { $details }.
stdlib.fetch.failed = '{ $url }'을(를) 가져오지 못했습니다: { $details }.
stdlib.fetch.cache_read_failed = 캐시 항목 '{ $name }'을(를) 읽지 못했습니다: { $details }.
stdlib.fetch.cache_open_failed = 캐시 항목 '{ $name }'을(를) 열지 못했습니다: { $details }.
stdlib.fetch.response_read_failed = '{ $url }'의 응답을 읽지 못했습니다: { $details }.
stdlib.fetch.response_buffer_overflow = '{ $url }'을(를) 읽는 중 버퍼가 넘쳤습니다.
stdlib.fetch.cache_write_failed = '{ $url }'의 캐시를 쓰지 못했습니다: { $details }.
stdlib.fetch.response_limit_exceeded = '{ $url }'의 응답이 { $limit }바이트 한도를 넘었습니다.
stdlib.fetch.cache_limit_exceeded = 캐시된 응답 '{ $name }'이(가) { $limit }바이트 한도를 넘었습니다.
stdlib.fetch.io_failed = { $path }에 대한 '{ $action }'에 실패했습니다: { $details }.
stdlib.fetch.action.sync_cache = fetch 캐시 동기화
stdlib.fetch.action.create_cache_dir = fetch 캐시 디렉터리 생성
stdlib.fetch.action.open_cache_dir = fetch 캐시 디렉터리 열기
stdlib.fetch.action.stat_cache = fetch 캐시 항목 정보 조회
stdlib.fetch.action.open_cache_entry = fetch 캐시 항목 열기

# 명령 도우미 진단.
stdlib.command.location = 템플릿 '{ $template }'의 명령 '{ $command }'
stdlib.command.spawn_failed = { $location }을(를) 시작하지 못했습니다: { $details }.
stdlib.command.io_failed = { $location }이(가) 실패했습니다: { $details }.
stdlib.command.closed_input_early = 명령에 쓰기가 끝나기 전에 입력이 닫혔습니다.
stdlib.command.broken_pipe = { $location } 실행 중 파이프가 끊겼습니다: { $details }.
stdlib.command.terminated_by_signal = { $location }이(가) 신호로 종료되었습니다.
stdlib.command.exited_with_status = { $location }이(가) 상태 { $status }(으)로 끝났습니다.
stdlib.command.output_limit_exceeded = { $location }이(가) { $stream }에 대한 { $mode } 한도 { $limit }바이트를 넘었습니다.
stdlib.command.timeout = { $location }이(가) { $seconds }초 제한 시간을 넘었습니다.
stdlib.command.exit_status_suffix = (종료 상태 { $status })
stdlib.command.signal_suffix = (신호로 종료됨)
stdlib.command.shell.empty = 셸 명령은 비어 있을 수 없습니다.
stdlib.command.grep.empty_pattern = grep 패턴은 비어 있을 수 없습니다.
stdlib.command.grep.flags_not_string = grep 플래그는 문자열이어야 합니다.
stdlib.command.quote.invalid = { $arg }을(를) 따옴표로 감싸지 못했습니다: { $details }.
stdlib.command.quote.line_break = 캐리지 리턴이나 줄바꿈이 들어 있는 인자는 안전하게 따옴표로 감쌀 수 없습니다.
stdlib.command.input_undefined = 입력 값이 정의되지 않았습니다.
stdlib.command.tempfile.root_required = 명령의 임시 파일을 만들려면 작업 공간의 루트가 필요합니다.
stdlib.command.tempfile.create_failed = 명령의 임시 파일을 만들지 못했습니다: { $details }.
stdlib.command.options.invalid_utf8 = 명령 옵션의 키는 올바른 UTF-8이어야 합니다.
stdlib.command.option.mode_not_string = 출력 모드는 문자열이어야 합니다.
stdlib.command.options.invalid_type = 명령 옵션은 객체여야 합니다.
stdlib.command.output.mode_unsupported = 지원하지 않는 출력 모드 '{ $mode }'입니다.
stdlib.command.output.mode.capture = 수집
stdlib.command.output.mode.streaming = 스트리밍
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# 경로 도우미 진단.
stdlib.path.io.failed = { $path }에 대한 '{ $action }'에 실패했습니다({ $label }).
stdlib.path.io.failed_with_detail = { $path }에 대한 '{ $action }'에 실패했습니다: { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $path }에 대한 '{ $action }'에 실패했습니다({ $label }): { $detail }.
stdlib.path.io.not_found = 찾을 수 없음
stdlib.path.io.permission_denied = 권한 거부됨
stdlib.path.io.already_exists = 이미 있음
stdlib.path.io.invalid_input = 잘못된 입력
stdlib.path.io.invalid_data = 잘못된 데이터
stdlib.path.io.timed_out = 시간 초과
stdlib.path.io.interrupted = 중단됨
stdlib.path.io.would_block = 차단될 수 있음
stdlib.path.io.write_zero = 0바이트 기록
stdlib.path.io.unexpected_eof = 예상치 못한 파일 끝
stdlib.path.io.broken_pipe = 파이프 끊김
stdlib.path.io.connection_refused = 연결 거부됨
stdlib.path.io.connection_reset = 연결 재설정됨
stdlib.path.io.connection_aborted = 연결 중단됨
stdlib.path.io.not_connected = 연결되지 않음
stdlib.path.io.addr_in_use = 주소가 사용 중
stdlib.path.io.addr_not_available = 주소를 사용할 수 없음
stdlib.path.io.out_of_memory = 메모리 부족
stdlib.path.io.unsupported = 지원하지 않음
stdlib.path.io.file_too_large = 파일이 너무 큼
stdlib.path.io.resource_busy = 자원이 사용 중
stdlib.path.io.executable_busy = 실행 파일이 사용 중
stdlib.path.io.deadlock = 교착 상태
stdlib.path.io.crosses_devices = 장치를 넘나듦
stdlib.path.io.too_many_links = 링크가 너무 많음
stdlib.path.io.invalid_filename = 잘못된 파일 이름
stdlib.path.io.arg_list_too_long = 인자 목록이 너무 김
stdlib.path.io.stale_handle = 오래된 네트워크 파일 핸들
stdlib.path.io.storage_full = 저장 공간이 가득 참
stdlib.path.io.not_seekable = 위치를 지정할 수 없음
stdlib.path.io.network_down = 네트워크가 작동하지 않음
stdlib.path.io.network_unreachable = 네트워크에 도달할 수 없음
stdlib.path.io.host_unreachable = 호스트에 도달할 수 없음
stdlib.path.io.other = 입출력 오류
stdlib.path.action.canonicalize = 정규화
stdlib.path.action.open_directory = 디렉터리 열기
stdlib.path.action.stat = 정보 조회
stdlib.path.action.read = 읽기
stdlib.path.action.open_file = 파일 열기
stdlib.path.with_suffix.empty_separator = with_suffix에는 비어 있지 않은 구분자가 필요합니다.
stdlib.path.relative_to.mismatch = { $path }은(는) { $root }에 대한 상대 경로가 아닙니다.
stdlib.path.expanduser.unsupported = 특정 사용자에 대한 ~ 확장은 지원하지 않습니다.
stdlib.path.expanduser.no_home = ~을(를) 확장할 수 없습니다. 홈 디렉터리 환경 변수가 설정되지 않았습니다.
stdlib.path.contents.unsupported_encoding = 지원하지 않는 인코딩 '{ $encoding }'입니다.
stdlib.path.hash.unsupported_algorithm = 지원하지 않는 해시 알고리즘 '{ $algorithm }'입니다.
stdlib.path.hash.unsupported_algorithm_legacy = 지원하지 않는 해시 알고리즘 '{ $algorithm }'입니다('{ $feature }' 기능을 켜세요).

# 컬렉션 도우미 진단.
stdlib.collections.flatten.expected_sequence = flatten은 열의 항목을 기대했지만 { $kind }을(를) 발견했습니다.
stdlib.collections.group_by.empty_attribute = group_by에는 비어 있지 않은 속성이 필요합니다.
stdlib.collections.group_by.unresolved = group_by가 { $kind } 형식의 항목에서 '{ $attr }'을(를) 찾지 못했습니다.

# 시간 도우미 진단.
stdlib.time.offset.invalid = now의 오프셋 '{ $offset }'이(가) 잘못되었습니다. '+HH:MM[:SS]' 또는 'Z'가 필요합니다.
stdlib.time.timedelta.overflow = { $component }을(를) 더하는 중 timedelta가 넘쳤습니다.
stdlib.time.label.weeks = 주
stdlib.time.label.days = 일
stdlib.time.label.hours = 시간
stdlib.time.label.minutes = 분
stdlib.time.label.seconds = 초
stdlib.time.label.milliseconds = 밀리초
stdlib.time.label.microseconds = 마이크로초
stdlib.time.label.nanoseconds = 나노초

# which 도우미 진단.
stdlib.which.not_found = [netsuke::jinja::which::not_found] PATH 항목 { $count }개를 확인했지만 명령 '{ $command }'을(를) 찾지 못했습니다. 미리 보기: { $preview }
stdlib.which.not_found.hint.cwd_auto = PATH의 빈 구간은 무시됩니다. 작업 디렉터리를 포함하려면 cwd_mode="auto"를 사용하세요.
stdlib.which.not_found.hint.cwd_always = 현재 디렉터리를 포함하려면 cwd_mode="always"로 설정하세요.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] '{ $path }'의 명령 '{ $command }'이(가) 없거나 실행할 수 없습니다.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <비어 있음>
stdlib.which.path_entry.non_utf8 = PATH의 { $index }번째 항목에 UTF-8이 아닌 문자가 있습니다. Netsuke는 UTF-8 경로가 필요합니다.
stdlib.which.command.empty = which에는 비어 있지 않은 문자열이 필요합니다.
stdlib.which.cwd_mode.invalid = cwd_mode는 'auto', 'always', 'never' 중 하나여야 하지만 '{ $mode }'이(가) 나왔습니다.
stdlib.which.cwd.resolve_failed = 현재 디렉터리를 확인하지 못했습니다: { $details }.
stdlib.which.cwd.non_utf8 = 현재 디렉터리에 UTF-8이 아닌 부분이 있습니다.
stdlib.which.canonicalize_failed = '{ $path }'을(를) 정규화하지 못했습니다: { $details }.
stdlib.which.is_executable = '{ $path }'이(가) 실행 가능한지 확인하지 못했습니다: { $details }.
stdlib.which.canonicalize_non_utf8 = 정규 경로에 UTF-8이 아닌 부분이 있습니다.
stdlib.which.workspace_non_utf8 = 명령 '{ $command }'을(를) 해석하는 중 작업 공간 경로에 UTF-8이 아닌 부분이 있습니다: { $path }.
stdlib.which.walkdir_error = 명령을 해석하는 중 작업 공간을 순회하다 오류가 발생했습니다: { $details }.

# 표준 라이브러리 등록.
stdlib.register.open_dir = stdlib 등록을 위해 현재 디렉터리를 열지 못했습니다.
stdlib.register.resolve_dir = stdlib 등록을 위해 현재 디렉터리를 확인하지 못했습니다.
stdlib.register.dir_non_utf8 = 현재 디렉터리에 UTF-8이 아닌 부분이 있습니다: { $path }.

# 접근성 출력 모드의 상태 보고.
status.state.pending = 대기 중
status.state.running = 진행 중
status.state.done = 완료
status.state.failed = 실패
status.stage.label = 단계 { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = 작업 { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = 매니페스트 파일 읽는 중
status.stage.initial_yaml_parsing = YAML 문서 해석 중
status.stage.template_expansion = 템플릿 지시문 확장 중
status.stage.final_rendering = 매니페스트 값 역직렬화 및 렌더링 중
status.stage.ir_generation_validation = 의존성 그래프 구성 및 검증 중
status.stage.ninja_synthesis = Ninja 빌드 계획 합성 중
status.stage.ninja_synthesis_execute = Ninja 계획 합성 및 { $tool } 실행 중
status.stage.graph_rendering = 그래프 산출물 렌더링 중
status.stage.graph_rendering_with_tool = { $tool } 렌더링 중
status.complete = { $tool } 작업이 완료되었습니다.
status.timing.summary_header = 단계별 소요 시간 요약:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = 파이프라인 전체 소요 시간: { $duration }
status.tool.build = 빌드
status.tool.clean = 정리
status.tool.graph = 그래프
status.tool.graph_html = 그래프(HTML)
status.tool.generate = 생성
status.tool.help_targets = 대상 도움말

# 그래프 HTML 렌더러의 문구.
graph.html.title = Netsuke 빌드 그래프
graph.html.heading = Netsuke 빌드 그래프
graph.html.description = Netsuke가 렌더링한 빌드 그래프
graph.html.outline.summary = 대상과 의존성(텍스트 개요)
graph.html.outline.no_inputs = 입력 없음
graph.html.noscript.notice = JavaScript가 꺼져 있습니다. 위의 텍스트 개요가 그래프 전체이며, 이어서 DOT 원본이 나옵니다.

# 접근성 출력의 의미 접두어.
semantic.prefix.error = 오류:
semantic.prefix.warning = 경고:
semantic.prefix.success = 성공:
semantic.prefix.info = 정보:
semantic.prefix.timing = 소요 시간:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# 번역자를 위한 복수형 예시.
# 한국어에는 문법적 복수 변화가 없으므로 CLDR 분류는 `other` 하나뿐입니다.
example.files_processed = { $count ->
   *[other] 파일 { $count }개를 처리했습니다.
}

example.errors_found = { $count ->
    [0] 오류를 찾지 못했습니다.
   *[other] 오류 { $count }개를 찾았습니다.
}
