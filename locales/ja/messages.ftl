# Netsuke コマンドラインのローカライズリソース。

runner.io.dyndep.retention = { $path } 配下で生成済み dyndep の保持を適用できませんでした。
cli.about = Netsuke は YAML + Jinja のマニフェストを Ninja のビルド計画にコンパイルします。
cli.long_about = Netsuke は YAML + Jinja のマニフェストを再現可能な Ninja グラフに変換し、安全な既定値で Ninja を実行します。
cli.usage = { $usage }

# 全体オプションのヘルプ文。
cli.flag.file.help = 使用する Netsuke マニフェストファイルのパス。
cli.flag.directory.help = このディレクトリーで起動したものとして実行します。
cli.flag.config.help = 自動検出を行わずに使用する設定ファイルのパス。
cli.flag.jobs.help = 並列に実行するビルドジョブ数を指定します。
cli.flag.verbose.help = 詳細な診断ログと完了時の所要時間サマリーを有効にします。
cli.flag.locale.help = コマンドライン文言のロケールタグ（例: en-US、ja）。
cli.flag.fetch_allow_scheme.help = fetch ヘルパーで追加的に許可する URL スキーム。
cli.flag.fetch_allow_host.help = 既定の拒否が有効なときに許可するホスト名。
cli.flag.fetch_block_host.help = 他で許可されていても常に遮断するホスト名。
cli.flag.fetch_default_deny.help = 既定ですべてのホストを拒否し、宣言した許可リストのみを通します。
cli.flag.json.help = 機械可読な JSON を出力します。
cli.flag.no_input.help = 対話的な入力を一切読み取りません。
cli.flag.color.help = 色付き出力の方針（auto、always、never）。
cli.flag.emoji.help = 絵文字の方針（auto、always、never）。
cli.flag.progress.help = 進捗表示の方針（auto、always、never）。
cli.flag.accessibility.help = アクセシブル出力の方針（auto、on、off）。
cli.flag.default_targets.help = ターゲットが指定されない場合の既定のビルドターゲット。

# サブコマンドの説明。
cli.subcommand.build.about = マニフェストで定義したターゲットをビルドします（既定）。
cli.subcommand.build.long_about = 要求されたターゲットをビルドします。指定がない場合はマニフェストの既定ターゲットを使います。
cli.subcommand.clean.about = Ninja を介してビルド成果物を削除します。
cli.subcommand.clean.long_about = 一時的な Ninja ファイルを生成し、続いて `ninja -t clean` を実行します。
cli.subcommand.graph.about = ビルドの依存グラフを出力します。既定の形式は DOT です。
cli.subcommand.graph.long_about = 解析済みの Netsuke マニフェストを正準形のビルドグラフに射影し、Graphviz DOT として、または `--html` を指定した場合は自己完結型の HTML ページとして書き出します。ファイルへ書き出すには `--output <ファイル>` を使い、`-` を指定すると標準出力に書き出します。
cli.subcommand.generate.about = Ninja を実行せずに Ninja マニフェストを生成します。
cli.subcommand.generate.long_about = 生成した Ninja マニフェストを標準出力、または `--output` で選んだファイルに書き出します。
cli.subcommand.help.about = 最上位のヘルプ、または指定されたトピックのヘルプを表示します。
cli.subcommand.help.long_about = トピックなしの場合、これは `--help` と同じです。選択したファイルのターゲットとアクションのカタログを表示するには `help targets` を使用します。

# Help catalogue headings and markers.
cli.help.actions_heading = アクション:
cli.help.targets_heading = ターゲット:
cli.help.targets.about = 選択したファイルのターゲットとアクションを一覧表示します。
cli.help.default_marker = 既定
cli.help.conditional_marker = 条件付き

# build サブコマンドのオプションのヘルプ文。
cli.subcommand.build.flag.targets.help = ビルドするターゲット（省略時はマニフェストの既定値を使用）。

# graph サブコマンドのオプションのヘルプ文。
cli.subcommand.graph.flag.html.help = グラフを DOT ではなく自己完結型の HTML ページとして描画します。
cli.subcommand.graph.flag.output.help = グラフ成果物をファイルに書き出します。標準出力には `-` を使います。

# generate サブコマンドのオプションのヘルプ文。
cli.subcommand.generate.flag.output.help = 生成した Ninja マニフェストを標準出力ではなくファイルに書き出します。

# コマンドラインの検証エラー。
cli.validation.jobs.invalid_number = { $value } は有効な数値ではありません。
cli.validation.jobs.out_of_range = ジョブ数は { $min } から { $max } の範囲でなければなりません。
cli.validation.scheme.empty = スキームを空にすることはできません。
cli.validation.scheme.invalid_start = スキーム「{ $scheme }」は ASCII 文字で始まる必要があります。
cli.validation.scheme.invalid = 無効なスキーム「{ $scheme }」です。
cli.validation.locale.empty = ロケールタグを空にすることはできません。
cli.validation.locale.invalid = 無効なロケールタグ「{ $locale }」です。
cli.validation.color.invalid = 無効な色の方針「{ $value }」です。有効な選択肢: auto、always、never。
cli.validation.emoji.invalid = 無効な絵文字の方針「{ $value }」です。有効な選択肢: auto、always、never。
cli.validation.progress.invalid = 無効な進捗の方針「{ $value }」です。有効な選択肢: auto、always、never。
cli.validation.accessibility.invalid = 無効なアクセシビリティの方針「{ $value }」です。有効な選択肢: auto、on、off。
cli.validation.config.expected_object = コマンドラインの値はオブジェクトへ直列化されるはずでしたが、{ $value } が得られました。

# Clap のエラーメッセージ。
clap-error-missing-argument = 必須の引数がありません: { $argument }
clap-error-missing-subcommand = サブコマンドがありません。利用できる選択肢: { $valid_subcommands }
clap-error-unknown-argument = 不明な引数です: { $argument }
clap-error-invalid-value = { $argument } の値が無効です: { $value }
clap-error-invalid-subcommand = 不明なサブコマンドです: { $subcommand }
# 注記: value-validation は invalid-value とは異なる表現にして、独自バリデーター
# の失敗（ErrorKind::ValueValidation）と型の不一致（ErrorKind::InvalidValue）を
# 区別しています。
clap-error-value-validation = { $argument } の検証に失敗しました: { $value }

# 実行時のエラーと文脈。
runner.manifest.not_found = マニフェスト「{ $manifest_name }」が { $directory } に見つかりません。
runner.manifest.not_found.help = マニフェストが存在することを確認するか、正しいパスを指定して `--file` を渡してください。
runner.manifest.path_missing_name = マニフェストのパス「{ $path }」にファイル名がありません。
cli.file.non_utf8 = マニフェストのパス「{ $path }」は有効な UTF-8 ではありません。
runner.manifest.directory_label = ディレクトリー `{ $directory }`
runner.manifest.current_directory_label = 現在のディレクトリー
runner.manifest.default_not_declared = マニフェストの既定値 '{ $default }' は、宣言されたアクションまたはターゲットを指していません。
runner.context.network_policy = ネットワークポリシーを構築できませんでした。
runner.context.load_manifest = { $path } のマニフェストを読み込めませんでした。
runner.context.serialise_manifest = マニフェストを直列化できませんでした。
runner.context.build_graph = マニフェストからグラフを構築できませんでした。
runner.context.generate_ninja = Ninja マニフェストを生成できませんでした。
runner.context.render_graph = グラフ成果物を描画できませんでした。

runner.io.create_temp_file = 一時 Ninja ファイルを作成できませんでした。
runner.io.write_temp_ninja = 一時 Ninja ファイルに書き込めませんでした。
runner.io.flush_temp_ninja = 一時 Ninja ファイルのバッファーを書き出せませんでした。
runner.io.sync_temp_ninja = 一時 Ninja ファイルを同期できませんでした。
runner.io.create_parent_dir = 親ディレクトリー { $path } を作成できませんでした。
runner.io.create_ninja_file = { $path } に Ninja ファイルを作成できませんでした。
runner.io.write_ninja_file = { $path } の Ninja ファイルに書き込めませんでした。
runner.io.flush_ninja_file = { $path } の Ninja ファイルのバッファーを書き出せませんでした。
runner.io.sync_ninja_file = { $path } の Ninja ファイルを同期できませんでした。
runner.io.open_ambient_dir = 周囲のディレクトリーを開けませんでした。
cli.directory.non_utf8 = 作業ディレクトリーのパスは有効な UTF-8 ではありません。 ({ $path })
runner.io.no_existing_ancestor = { $path } に対応する既存の上位ディレクトリーがありません。
runner.io.derive_relative_path = Ninja の相対パスを導出できませんでした。
runner.io.non_utf8_path = UTF-8 でないパスには対応していません（パス: { $path }）。
runner.io.write_stdout = Ninja マニフェストを標準出力に書き込めませんでした。
runner.io.flush_stdout = 標準出力のバッファーを書き出せませんでした。
runner.io.dyndep.create_dir = dyndep ディレクトリ { $path } を作成できませんでした。
runner.io.dyndep.read = { $path } にある生成済み dyndep ファイルを読み取れませんでした。
runner.io.dyndep.write = { $path } に生成済み dyndep ファイルを書き込めませんでした。
runner.io.dyndep.rename = { $path } にある生成済み dyndep ファイルを確定できませんでした。
runner.io.dyndep.corrupt = { $path } にある生成済み dyndep ファイルが期待した内容と一致しません。このファイルだけを削除して再試行してください。
runner.io.dyndep.temp_collisions = 名前の衝突が繰り返されたため、{ $path } 用の一意な一時 dyndep ファイルを作成できませんでした。
runner.io.dyndep.too_large = { $path } にある生成済み dyndep ファイルが { $limit } バイトの検証上限を超えています。

# マニフェストの診断。
manifest.parse = マニフェストの解析に失敗しました。
manifest.structure_error = { $name } でマニフェストの構造エラー: { $details }
manifest.yaml.parse = { $line } 行 { $column } 桁で YAML の解析エラー: { $details }
manifest.yaml.label = 無効な YAML
manifest.yaml.hint.tabs = YAML はタブを許しません。字下げには空白を使ってください。
manifest.yaml.hint.list_item = YAML のリスト項目は「-」で始め、正しく字下げする必要があります。
manifest.yaml.hint.expected_colon = マッピングの項目のようです。キーの後に「:」がありません。
manifest.yaml.hint.mapping_values = YAML のマッピングは「:」の後に値（または入れ子のブロック）が必要です。
manifest.yaml.hint.invalid_token = YAML のトークンが無効か、予期しないものです。
manifest.yaml.hint.escape = 逆斜線をエスケープするか、無効なエスケープ列を取り除いてください。
manifest.env.missing = 必須の環境変数が設定されていません。
manifest.env.invalid_utf8 = 環境変数に無効な UTF-8 が含まれています。
manifest.vars.not_object = マニフェストの `vars` はマップまたはオブジェクトでなければなりません。
manifest.vars.reserved_name = マニフェストの `vars` キー '{ $name }' は組み込みのテンプレートヘルパー用に予約されています。変数名を変更してください。
manifest.read_failed = { $path } のマニフェストを読み取れませんでした。
manifest.resolve_workspace_root = ワークスペースのルートを特定できませんでした。
manifest.workspace_non_utf8 = ワークスペースのルートパス「{ $path }」は有効な UTF-8 ではありません。
manifest.path_non_utf8 = マニフェスト「{ $manifest }」のパスは有効な UTF-8 ではありません: { $path }。
manifest.path_missing_name = マニフェストのパス「{ $path }」にファイル名がありません。
manifest.open_workspace_failed = マニフェスト { $manifest } のためにワークスペース { $workspace } を開けませんでした。
manifest.foreach.not_iterable = `foreach` の式は反復できません。
manifest.foreach.serialise_item = `foreach` の要素を直列化できませんでした。
manifest.when.empty = `when` の式を空にすることはできません。
manifest.when.eval_error = `when` の式「{ $expr }」を評価できませんでした。
manifest.when.template_error = `when` のテンプレート「{ $expr }」を描画できませんでした。
manifest.target.vars_not_object = ターゲットの `vars` はオブジェクトでなければなりませんが、{ $value } が得られました。
manifest.vars.entry_not_object = マニフェストの `vars` の項目はオブジェクトでなければなりません。
manifest.field_not_string = フィールド「{ $field }」は文字列でなければなりません。
manifest.expression.parse_error = { $name } の式を解析できませんでした。
manifest.expression.eval_error = { $name } の式を評価できませんでした。

# マニフェストのマクロの診断。
manifest.macro.signature_missing_identifier = マクロのシグネチャーに識別子がありません。
manifest.macro.signature_missing_params = マクロのシグネチャーに引数がありません。
manifest.macro.compile_failed = マクロ { $name } をコンパイルできませんでした。
manifest.macro.sequence_invalid = マクロは名前からテンプレートへのマッピングとして定義する必要があります。
manifest.macro.register_failed = マニフェストのマクロを登録できませんでした。
manifest.macro.not_initialised = マクロ環境が初期化されていません。
manifest.macro.caller_invalid = マクロの呼び出し元は文字列でなければなりません。
manifest.macro.template_load_failed = マクロのテンプレートを読み込めませんでした。
manifest.macro.init_failed = マクロ環境を初期化できませんでした。
manifest.macro.missing = マクロ { $name } がありません。

# マニフェストの glob エラー。
manifest.glob.unmatched_brace = 無効な glob パターン「{ $pattern }」: 位置 { $position } の「{ $character }」に対応するものがありません。
manifest.glob.invalid_pattern = 無効な glob パターン「{ $pattern }」: { $detail }。
manifest.glob.unknown_pattern_error = 不明なパターンエラー。
manifest.glob.io_failed = 「{ $pattern }」の glob に失敗しました: { $detail }。
manifest.glob.unknown_io_error = 不明な入出力エラー。
manifest.command_list_empty = 「command」フィールドは空にできません: コマンド文字列または空でないリストを指定してください。

# 中間表現のエラー。
ir.rule_not_found = ターゲット「{ $target }」が参照する規則「{ $rule }」が見つかりません。
ir.multiple_rules = ターゲット「{ $target }」は規則をちょうど 1 つ参照しなければなりませんが、{ $rules } が得られました。
ir.empty_rule = ターゲット「{ $target }」は規則を参照しなければなりません。
ir.duplicate_outputs = 出力の重複を検出しました: { $outputs }。
ir.circular_dependency = 循環依存を検出しました: { $cycle }。
ir.action_serialisation = アクションを直列化できませんでした: { $details }。
ir.invalid_command = コマンドの補間が無効です: { $snippet }。

# Ninja 生成のエラー。
ninja_gen.missing_action = ビルド辺が参照するアクション「{ $id }」がありません。
ninja_gen.format = Ninja マニフェストの出力を整形できませんでした。
ninja_gen.dyndep_files_required = この操作には生成済み Ninja バンドルが必要です。dyndep ファイルを具体化するには `netsuke build`、`netsuke clean`、または `netsuke generate` を使用してください。
ninja_gen.reserved_output_path = パス '{ $path }' は Netsuke の直列依存関係の状態用に予約されています。
ninja_gen.unsupported_path_character = パス '{ $path }' にサポートされていない Ninja パス文字 '{ $character }' が含まれています。

# ホストパターンの検証。
host_pattern.empty = ホストパターンを空にすることはできません。
host_pattern.contains_scheme = ホストパターン「{ $pattern }」に URL スキームを含めることはできません。
host_pattern.contains_slash = ホストパターン「{ $pattern }」に「/」を含めることはできません。
host_pattern.missing_suffix = ホストパターン「{ $pattern }」には「*.」の後に接尾辞が必要です。
host_pattern.empty_label = ホストパターン「{ $pattern }」に空のラベルが含まれています。
host_pattern.invalid_chars = ホストパターン「{ $pattern }」に無効な文字が含まれています。
host_pattern.invalid_label_edge = ホストパターン「{ $pattern }」のラベルを「-」で始めたり終えたりすることはできません。
host_pattern.label_too_long = ホストパターン「{ $pattern }」に 63 文字を超えるラベルが含まれています。
host_pattern.too_long = ホストパターン「{ $pattern }」が 255 文字の上限を超えています。

# ネットワークポリシー。
network_policy.scheme.empty = スキームを空にすることはできません。
network_policy.scheme.invalid = スキーム「{ $scheme }」に無効な文字が含まれています。
network_policy.allowlist.empty = ホストの許可リストを空にすることはできません。
network_policy.scheme.not_allowed = スキーム「{ $scheme }」は許可されていません。
network_policy.missing_host = URL にホストがありません。
network_policy.host.blocked = ホスト「{ $host }」はポリシーにより遮断されています。
network_policy.host.not_allowlisted = ホスト「{ $host }」は許可リストにありません。

# 標準ライブラリーの設定。
stdlib.config.default_fetch_cache_invalid = fetch キャッシュの既定のパスは相対パスでなければなりません。
stdlib.config.default_which_cache_invalid = which キャッシュの既定の容量は正の値でなければなりません。
stdlib.config.workspace_root_absolute = ワークスペースのルートパスは絶対パスでなければなりません。
stdlib.config.fetch_response_limit_positive = fetch の応答上限は正の値でなければなりません。
stdlib.config.command_output_limit_positive = コマンド出力の取り込み上限は正の値でなければなりません。
stdlib.config.command_stream_limit_positive = コマンドのストリーム上限は正の値でなければなりません。
stdlib.config.which_cache_capacity_positive = which キャッシュの容量は正の値でなければなりません。
stdlib.config.skip_dir_empty = 読み飛ばすディレクトリーの項目を空にすることはできません。
stdlib.config.skip_dir_navigation = 読み飛ばすディレクトリーの項目に「..」を含めることはできません。
stdlib.config.skip_dir_separator = 読み飛ばすディレクトリーの項目にパス区切り文字を含めることはできません。
stdlib.config.fetch_cache_empty = fetch キャッシュのパスを空にすることはできません。
stdlib.config.fetch_cache_not_relative = fetch キャッシュのパスは相対パスでなければなりませんが、{ $path } が得られました。
stdlib.config.fetch_cache_escapes = fetch キャッシュのパスがワークスペースの外に出ることはできません: { $path }。
stdlib.config.open_workspace_root = 現在のディレクトリーを stdlib のワークスペースルートとして開けませんでした。
stdlib.config.resolve_cwd = 現在のディレクトリーを stdlib のワークスペースルートとして特定できませんでした。
stdlib.config.cwd_non_utf8 = 現在のディレクトリーに UTF-8 でない部分が含まれています: { $path }。

# fetch ヘルパーの診断。
stdlib.fetch.url_invalid = 無効な URL「{ $url }」: { $details }。
stdlib.fetch.disallowed = URL「{ $url }」は許可されていません: { $details }。
stdlib.fetch.failed = 「{ $url }」を取得できませんでした: { $details }。
stdlib.fetch.cache_read_failed = キャッシュ項目「{ $name }」を読み取れませんでした: { $details }。
stdlib.fetch.cache_open_failed = キャッシュ項目「{ $name }」を開けませんでした: { $details }。
stdlib.fetch.response_read_failed = 「{ $url }」からの応答を読み取れませんでした: { $details }。
stdlib.fetch.response_buffer_overflow = 「{ $url }」の読み取り中にバッファーがあふれました。
stdlib.fetch.cache_write_failed = 「{ $url }」のキャッシュを書き込めませんでした: { $details }。
stdlib.fetch.response_limit_exceeded = 「{ $url }」からの応答が { $limit } バイトの上限を超えました。
stdlib.fetch.cache_limit_exceeded = キャッシュ済みの応答「{ $name }」が { $limit } バイトの上限を超えました。
stdlib.fetch.io_failed = { $path } に対する「{ $action }」に失敗しました: { $details }。
stdlib.fetch.action.sync_cache = fetch キャッシュの同期
stdlib.fetch.action.create_cache_dir = fetch キャッシュディレクトリーの作成
stdlib.fetch.action.open_cache_dir = fetch キャッシュディレクトリーを開く操作
stdlib.fetch.action.stat_cache = fetch キャッシュ項目の情報取得
stdlib.fetch.action.open_cache_entry = fetch キャッシュ項目を開く操作

# コマンドヘルパーの診断。
stdlib.command.location = テンプレート「{ $template }」内のコマンド「{ $command }」
stdlib.command.spawn_failed = { $location } を起動できませんでした: { $details }。
stdlib.command.io_failed = { $location } が失敗しました: { $details }。
stdlib.command.closed_input_early = コマンドへの書き込みが終わる前に入力が閉じられました。
stdlib.command.broken_pipe = { $location } の実行中にパイプが切断されました: { $details }。
stdlib.command.terminated_by_signal = { $location } はシグナルにより終了しました。
stdlib.command.exited_with_status = { $location } は状態 { $status } で終了しました。
stdlib.command.output_limit_exceeded = { $location } は { $stream } について { $mode } の上限 { $limit } バイトを超えました。
stdlib.command.timeout = { $location } は { $seconds } 秒の制限時間を超えました。
stdlib.command.exit_status_suffix = （終了状態 { $status }）
stdlib.command.signal_suffix = （シグナルにより終了）
stdlib.command.shell.empty = シェルコマンドを空にすることはできません。
stdlib.command.grep.empty_pattern = grep のパターンを空にすることはできません。
stdlib.command.grep.flags_not_string = grep のフラグは文字列でなければなりません。
stdlib.command.quote.invalid = { $arg } を引用符で囲めませんでした: { $details }。
stdlib.command.quote.line_break = 復帰または改行を含む引数は安全に引用符で囲めません。
stdlib.command.input_undefined = 入力値が未定義です。
stdlib.command.tempfile.root_required = コマンドの一時ファイルを作成するにはワークスペースのルートが必要です。
stdlib.command.tempfile.create_failed = コマンドの一時ファイルを作成できませんでした: { $details }。
stdlib.command.options.invalid_utf8 = コマンドオプションのキーは有効な UTF-8 でなければなりません。
stdlib.command.option.mode_not_string = 出力モードは文字列でなければなりません。
stdlib.command.options.invalid_type = コマンドのオプションはオブジェクトでなければなりません。
stdlib.command.output.mode_unsupported = 対応していない出力モード「{ $mode }」です。
stdlib.command.output.mode.capture = 取り込み
stdlib.command.output.mode.streaming = ストリーミング
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# パスヘルパーの診断。
stdlib.path.io.failed = { $path } に対する「{ $action }」に失敗しました（{ $label }）。
stdlib.path.io.failed_with_detail = { $path } に対する「{ $action }」に失敗しました: { $detail }。
stdlib.path.io.failed_with_label_and_detail = { $path } に対する「{ $action }」に失敗しました（{ $label }）: { $detail }。
stdlib.path.io.not_found = 見つかりません
stdlib.path.io.permission_denied = アクセスが拒否されました
stdlib.path.io.already_exists = すでに存在します
stdlib.path.io.invalid_input = 無効な入力
stdlib.path.io.invalid_data = 無効なデータ
stdlib.path.io.timed_out = 時間切れ
stdlib.path.io.interrupted = 中断されました
stdlib.path.io.would_block = 処理が滞ります
stdlib.path.io.write_zero = 0 バイトの書き込み
stdlib.path.io.unexpected_eof = 予期しないファイル終端
stdlib.path.io.broken_pipe = パイプの切断
stdlib.path.io.connection_refused = 接続が拒否されました
stdlib.path.io.connection_reset = 接続がリセットされました
stdlib.path.io.connection_aborted = 接続が中断されました
stdlib.path.io.not_connected = 接続されていません
stdlib.path.io.addr_in_use = アドレスは使用中です
stdlib.path.io.addr_not_available = アドレスを利用できません
stdlib.path.io.out_of_memory = メモリー不足
stdlib.path.io.unsupported = 対応していません
stdlib.path.io.file_too_large = ファイルが大きすぎます
stdlib.path.io.resource_busy = リソースが使用中です
stdlib.path.io.executable_busy = 実行ファイルが使用中です
stdlib.path.io.deadlock = デッドロック
stdlib.path.io.crosses_devices = デバイスをまたいでいます
stdlib.path.io.too_many_links = リンクが多すぎます
stdlib.path.io.invalid_filename = 無効なファイル名
stdlib.path.io.arg_list_too_long = 引数リストが長すぎます
stdlib.path.io.stale_handle = 失効したネットワークファイルハンドル
stdlib.path.io.storage_full = 記憶領域がいっぱいです
stdlib.path.io.not_seekable = 位置指定ができません
stdlib.path.io.network_down = ネットワークが停止しています
stdlib.path.io.network_unreachable = ネットワークに到達できません
stdlib.path.io.host_unreachable = ホストに到達できません
stdlib.path.io.other = 入出力エラー
stdlib.path.action.canonicalize = 正準化
stdlib.path.action.open_directory = ディレクトリーを開く操作
stdlib.path.action.stat = 情報取得
stdlib.path.action.read = 読み取り
stdlib.path.action.open_file = ファイルを開く操作
stdlib.path.with_suffix.empty_separator = with_suffix には空でない区切り文字が必要です。
stdlib.path.relative_to.mismatch = { $path } は { $root } からの相対パスではありません。
stdlib.path.expanduser.unsupported = 特定ユーザーに対する ~ の展開には対応していません。
stdlib.path.expanduser.no_home = ~ を展開できません。ホームディレクトリーの環境変数が設定されていません。
stdlib.path.contents.unsupported_encoding = 対応していない文字符号化「{ $encoding }」です。
stdlib.path.hash.unsupported_algorithm = 対応していないハッシュアルゴリズム「{ $algorithm }」です。
stdlib.path.hash.unsupported_algorithm_legacy = 対応していないハッシュアルゴリズム「{ $algorithm }」です（機能「{ $feature }」を有効にしてください）。

# コレクションヘルパーの診断。
stdlib.collections.flatten.expected_sequence = flatten は列の要素を期待しましたが、{ $kind } が見つかりました。
stdlib.collections.group_by.empty_attribute = group_by には空でない属性が必要です。
stdlib.collections.group_by.unresolved = group_by は種別 { $kind } の要素で「{ $attr }」を解決できませんでした。

# 時刻ヘルパーの診断。
stdlib.time.offset.invalid = now のオフセット「{ $offset }」は無効です。「+HH:MM[:SS]」または「Z」が必要です。
stdlib.time.timedelta.overflow = { $component } の加算で timedelta があふれました。
stdlib.time.label.weeks = 週
stdlib.time.label.days = 日
stdlib.time.label.hours = 時間
stdlib.time.label.minutes = 分
stdlib.time.label.seconds = 秒
stdlib.time.label.milliseconds = ミリ秒
stdlib.time.label.microseconds = マイクロ秒
stdlib.time.label.nanoseconds = ナノ秒

# which ヘルパーの診断。
stdlib.which.not_found = [netsuke::jinja::which::not_found] PATH の項目を { $count } 件調べましたが、コマンド「{ $command }」は見つかりませんでした。プレビュー: { $preview }
stdlib.which.not_found.hint.cwd_auto = PATH の空の区間は無視されます。作業ディレクトリーを含めるには cwd_mode="auto" を使ってください。
stdlib.which.not_found.hint.cwd_always = 現在のディレクトリーを含めるには cwd_mode="always" を設定してください。
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] 「{ $path }」のコマンド「{ $command }」が存在しないか、実行できません。
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <空>
stdlib.which.path_entry.non_utf8 = PATH の { $index } 番目の項目に UTF-8 でない文字が含まれています。Netsuke は UTF-8 のパスを必要とします。
stdlib.which.command.empty = which には空でない文字列が必要です。
stdlib.which.cwd_mode.invalid = cwd_mode は「auto」「always」「never」のいずれかでなければなりませんが、「{ $mode }」が得られました。
stdlib.which.cwd.resolve_failed = 現在のディレクトリーを特定できませんでした: { $details }。
stdlib.which.cwd.non_utf8 = 現在のディレクトリーに UTF-8 でない部分が含まれています。
stdlib.which.canonicalize_failed = 「{ $path }」を正準化できませんでした: { $details }。
stdlib.which.is_executable = 「{ $path }」が実行可能かどうか調べられませんでした: { $details }。
stdlib.which.canonicalize_non_utf8 = 正準パスに UTF-8 でない部分が含まれています。
stdlib.which.workspace_non_utf8 = コマンド「{ $command }」の解決中、ワークスペースのパスに UTF-8 でない部分が含まれていました: { $path }。
stdlib.which.walkdir_error = コマンドの解決中にワークスペースの走査でエラーが発生しました: { $details }。

# 標準ライブラリーの登録。
stdlib.register.open_dir = stdlib の登録のために現在のディレクトリーを開けませんでした。
stdlib.register.resolve_dir = stdlib の登録のために現在のディレクトリーを特定できませんでした。
stdlib.register.dir_non_utf8 = 現在のディレクトリーに UTF-8 でない部分が含まれています: { $path }。

# アクセシブル出力モードの状態表示。
status.state.pending = 待機中
status.state.running = 進行中
status.state.done = 完了
status.state.failed = 失敗
status.stage.label = 段階 { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label }（{ $task_progress }）
status.task.progress_label = タスク { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = マニフェストファイルを読み取り中
status.stage.initial_yaml_parsing = YAML 文書を解析中
status.stage.template_expansion = テンプレート指令を展開中
status.stage.final_rendering = マニフェストの値を復元して描画中
status.stage.ir_generation_validation = 依存グラフを構築して検証中
status.stage.ninja_synthesis = Ninja のビルド計画を合成中
status.stage.ninja_synthesis_execute = Ninja の計画を合成し { $tool } を実行中
status.stage.graph_rendering = グラフ成果物を描画中
status.stage.graph_rendering_with_tool = { $tool } を描画中
status.complete = { $tool } が完了しました。
status.timing.summary_header = 段階ごとの所要時間:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = パイプライン全体の所要時間: { $duration }
status.tool.build = ビルド
status.tool.clean = クリーン
status.tool.graph = グラフ
status.tool.graph_html = グラフ（HTML）
status.tool.generate = 生成
status.tool.help_targets = ターゲットヘルプ

# グラフの HTML 描画に使う文言。
graph.html.title = Netsuke のビルドグラフ
graph.html.heading = Netsuke のビルドグラフ
graph.html.description = Netsuke が描画したビルドグラフ
graph.html.outline.summary = ターゲットと依存関係（テキストの概要）
graph.html.outline.no_inputs = 入力なし
graph.html.noscript.notice = JavaScript が無効です。上のテキスト概要がグラフ全体であり、続けて DOT のソースが表示されます。

# アクセシブル出力の意味づけ接頭辞。
semantic.prefix.error = エラー:
semantic.prefix.warning = 警告:
semantic.prefix.success = 成功:
semantic.prefix.info = 情報:
semantic.prefix.timing = 所要時間:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# 翻訳者向けの複数形の例。
# 日本語には文法上の複数形がないため、CLDR の分類は `other` だけです。
example.files_processed = { $count ->
   *[other] { $count } 件のファイルを処理しました。
}

example.errors_found = { $count ->
    [0] エラーは見つかりませんでした。
   *[other] { $count } 件のエラーが見つかりました。
}

# マニフェストの静的検査（`netsuke check`）。
cli.subcommand.check.about = ビルドを生成も実行もせずにマニフェストを検査します。
cli.subcommand.check.long_about = 選択したマニフェストを検査し、解析は通るものの誤り・危険・非可搬・キャッシュに不利と思われる記述を洗い出します。
cli.subcommand.check.flag.rule.help = 規則または分類の重大度を NAME=SEVERITY の形式で設定します。
cli.subcommand.check.flag.fail_on.help = 指摘がコマンドを失敗させる重大度です。
cli.subcommand.check.flag.limit.help = 報告する指摘の上限。0 はすべて報告します。
cli.subcommand.check.flag.explain.help = マニフェストを検査せず規則リファレンスを出力します。
check.threshold_exceeded = 指摘がしきい値 { $severity } に達しました: 報告 { $reported } 件のうち { $failing } 件。
check.threshold_exceeded.help = 報告された指摘を修正するか、--rule を調整するか、--fail-on を緩めてください。
check.summary.counts = 検査結果 — エラー: { $errors }、警告: { $warnings }、助言: { $advice }、抑制: { $suppressed }。
check.summary.clean = 指摘はありません。
check.summary.truncated = { $shown } 件を表示しています。--limit により { $omitted } 件を省略しました。
check.rule.malformed = セレクター { $selector } が NAME=SEVERITY の形式で書かれていません。
check.rule.unknown = セレクターが指定した { $name } は規則でも分類でもありません。
check.rule.severity = セレクター { $name } が指定した重大度 { $severity } は無効です。次のいずれかを指定してください: { $values }。
check.fail_on.invalid = 失敗しきい値 { $value } は不明です。次のいずれかを指定してください: { $values }。
check.source_index = 診断のために { $path } を { $line } 行目で索引付けできませんでした: { $reason }。
status.tool.check = 検査
