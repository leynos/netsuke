# Recursos de localização da CLI do Netsuke (português do Brasil).

cli.about = O Netsuke compila manifestos YAML + Jinja em planos de build do Ninja.
cli.long_about = O Netsuke transforma manifestos YAML + Jinja em grafos do Ninja reproduzíveis e executa o Ninja com padrões seguros.
cli.usage = { $usage }

# Texto de ajuda das opções globais.
cli.flag.file.help = Caminho do arquivo de manifesto do Netsuke a ser usado.
cli.flag.directory.help = Executar como se tivesse sido iniciado neste diretório.
cli.flag.config.help = Caminho de um arquivo de configuração, ignorando a descoberta automática.
cli.flag.jobs.help = Define a quantidade de tarefas de build em paralelo.
cli.flag.verbose.help = Habilita logs de diagnóstico detalhados e resumos de tempo ao concluir.
cli.flag.locale.help = Tag de idioma para os textos da CLI (por exemplo: en-US, pt-BR).
cli.flag.fetch_allow_scheme.help = Esquemas de URL adicionais permitidos para o auxiliar fetch.
cli.flag.fetch_allow_host.help = Nomes de host permitidos quando a negação padrão está ativa.
cli.flag.fetch_block_host.help = Nomes de host sempre bloqueados, mesmo que permitidos em outro lugar.
cli.flag.fetch_default_deny.help = Negar todos os hosts por padrão; permitir apenas a lista declarada.
cli.flag.json.help = Emitir saída JSON legível por máquina.
cli.flag.no_input.help = Nunca ler entrada interativa.
cli.flag.color.help = Política de cor na saída (auto, always, never).
cli.flag.emoji.help = Política de emojis (auto, always, never).
cli.flag.progress.help = Política de exibição do progresso (auto, always, never).
cli.flag.accessibility.help = Política de saída acessível (auto, on, off).
cli.flag.default_targets.help = Alvos de build padrão quando nenhum é informado.

# Descrições dos subcomandos.
cli.subcommand.build.about = Compilar os alvos definidos no manifesto (padrão).
cli.subcommand.build.long_about = Compilar os alvos solicitados; se nenhum for informado, usar os padrões do manifesto.
cli.subcommand.clean.about = Remover os artefatos de build por meio do Ninja.
cli.subcommand.clean.long_about = Gerar um arquivo Ninja temporário e depois executar `ninja -t clean`.
cli.subcommand.graph.about = Emitir o grafo de dependências do build. O formato padrão é DOT.
cli.subcommand.graph.long_about = Projetar o manifesto do Netsuke analisado em um grafo de build canônico e gravá-lo como Graphviz DOT ou como página HTML autocontida com `--html`. Use `--output <ARQUIVO>` para gravar em um arquivo; `-` grava na stdout.
cli.subcommand.generate.about = Gerar o manifesto do Ninja sem executar o Ninja.
cli.subcommand.generate.long_about = Gravar o manifesto do Ninja gerado na stdout ou no arquivo escolhido com `--output`.

# Texto de ajuda das opções do subcomando build.
cli.subcommand.build.flag.targets.help = Alvos a compilar (se omitido, usa os padrões do manifesto).

# Texto de ajuda das opções do subcomando graph.
cli.subcommand.graph.flag.html.help = Renderizar o grafo como página HTML autocontida em vez de DOT.
cli.subcommand.graph.flag.output.help = Gravar o artefato do grafo em ARQUIVO; use `-` para a stdout.

# Texto de ajuda das opções do subcomando generate.
cli.subcommand.generate.flag.output.help = Gravar o manifesto do Ninja gerado em ARQUIVO em vez da stdout.

# Erros de validação da CLI.
cli.validation.jobs.invalid_number = { $value } não é um número válido.
cli.validation.jobs.out_of_range = A quantidade de tarefas deve estar entre { $min } e { $max }.
cli.validation.scheme.empty = O esquema não pode estar vazio.
cli.validation.scheme.invalid_start = O esquema "{ $scheme }" deve começar com uma letra ASCII.
cli.validation.scheme.invalid = Esquema inválido "{ $scheme }".
cli.validation.locale.empty = A tag de idioma não pode estar vazia.
cli.validation.locale.invalid = Tag de idioma inválida "{ $locale }".
cli.validation.color.invalid = Política de cor inválida "{ $value }". Opções válidas: auto, always, never.
cli.validation.emoji.invalid = Política de emojis inválida "{ $value }". Opções válidas: auto, always, never.
cli.validation.progress.invalid = Política de progresso inválida "{ $value }". Opções válidas: auto, always, never.
cli.validation.accessibility.invalid = Política de acessibilidade inválida "{ $value }". Opções válidas: auto, on, off.
cli.validation.config.expected_object = Esperava-se que os valores da CLI fossem serializados como objeto, obteve-se { $value }.

# Mensagens de erro do Clap.
clap-error-missing-argument = Falta um argumento obrigatório: { $argument }
clap-error-missing-subcommand = Falta o subcomando. Opções disponíveis: { $valid_subcommands }
clap-error-unknown-argument = Argumento desconhecido: { $argument }
clap-error-invalid-value = Valor inválido para { $argument }: { $value }
clap-error-invalid-subcommand = Subcomando desconhecido: { $subcommand }
# Observação: value-validation usa uma redação distinta de invalid-value para
# diferenciar falhas de validadores personalizados
# (ErrorKind::ValueValidation) de incompatibilidades de tipo
# (ErrorKind::InvalidValue).
clap-error-value-validation = A validação falhou para { $argument }: { $value }

# Erros e contextos do executor.
runner.manifest.not_found = Manifesto "{ $manifest_name }" não encontrado em { $directory }.
runner.manifest.not_found.help = Verifique se o manifesto existe ou informe `--file` com o caminho correto.
runner.manifest.path_missing_name = O caminho do manifesto "{ $path }" não tem nome de arquivo.
runner.manifest.path_utf8 = O caminho do manifesto "{ $path }" não é UTF-8 válido.
runner.manifest.directory_utf8 = O caminho do diretório do manifesto "{ $path }" não é UTF-8 válido.
runner.manifest.directory_label = diretório `{ $directory }`
runner.manifest.current_directory_label = o diretório atual
runner.context.network_policy = Não foi possível construir a política de rede.
runner.context.load_manifest = Não foi possível carregar o manifesto em { $path }.
runner.context.serialise_manifest = Não foi possível serializar o manifesto.
runner.context.build_graph = Não foi possível construir o grafo a partir do manifesto.
runner.context.generate_ninja = Não foi possível gerar o manifesto do Ninja.
runner.context.render_graph = Não foi possível renderizar o artefato do grafo.

runner.io.create_temp_file = Não foi possível criar o arquivo Ninja temporário.
runner.io.write_temp_ninja = Não foi possível gravar o arquivo Ninja temporário.
runner.io.flush_temp_ninja = Não foi possível esvaziar o buffer do arquivo Ninja temporário.
runner.io.sync_temp_ninja = Não foi possível sincronizar o arquivo Ninja temporário.
runner.io.create_parent_dir = Não foi possível criar o diretório pai { $path }.
runner.io.create_ninja_file = Não foi possível criar o arquivo Ninja em { $path }.
runner.io.write_ninja_file = Não foi possível gravar o arquivo Ninja em { $path }.
runner.io.flush_ninja_file = Não foi possível esvaziar o buffer do arquivo Ninja em { $path }.
runner.io.sync_ninja_file = Não foi possível sincronizar o arquivo Ninja em { $path }.
runner.io.open_ambient_dir = Não foi possível abrir o diretório do ambiente.
runner.io.no_existing_ancestor = Não existe diretório ancestral para { $path }.
runner.io.derive_relative_path = Não foi possível derivar o caminho relativo do Ninja.
runner.io.non_utf8_path = Não há suporte para caminhos que não sejam UTF-8 (caminho: { $path }).
runner.io.write_stdout = Não foi possível gravar o manifesto do Ninja na stdout.
runner.io.flush_stdout = Não foi possível esvaziar o buffer da stdout.

# Diagnósticos do manifesto.
manifest.parse = A análise do manifesto falhou.
manifest.structure_error = Erro de estrutura do manifesto em { $name }: { $details }
manifest.yaml.parse = Erro de análise do YAML na linha { $line }, coluna { $column }: { $details }
manifest.yaml.label = YAML inválido
manifest.yaml.hint.tabs = O YAML não permite tabulações; use espaços na indentação.
manifest.yaml.hint.list_item = Itens de lista do YAML devem começar com "-" e estar corretamente indentados.
manifest.yaml.hint.expected_colon = Isto parece uma entrada de mapeamento; falta um ":" depois da chave.
manifest.yaml.hint.mapping_values = Mapeamentos do YAML exigem um valor depois de ":" (ou um bloco aninhado).
manifest.yaml.hint.invalid_token = O token do YAML é inválido ou inesperado.
manifest.yaml.hint.escape = Escape as barras invertidas ou remova as sequências de escape inválidas.
manifest.env.missing = Uma variável de ambiente obrigatória não está definida.
manifest.env.invalid_utf8 = Uma variável de ambiente contém UTF-8 inválido.
manifest.vars.not_object = `vars` do manifesto deve ser um mapa ou objeto.
manifest.vars.reserved_name = A chave `vars` '{ $name }' do manifesto está reservada para uma função auxiliar de modelo integrada; renomeie a variável.
manifest.read_failed = Não foi possível ler o manifesto em { $path }.
manifest.resolve_workspace_root = Não foi possível resolver a raiz do workspace.
manifest.workspace_non_utf8 = O caminho da raiz do workspace "{ $path }" não é UTF-8 válido.
manifest.path_non_utf8 = O caminho do manifesto "{ $manifest }" não é UTF-8 válido: { $path }.
manifest.path_missing_name = O caminho do manifesto "{ $path }" não tem nome de arquivo.
manifest.open_workspace_failed = Não foi possível abrir o workspace { $workspace } para o manifesto { $manifest }.
manifest.foreach.not_iterable = A expressão `foreach` não é iterável.
manifest.foreach.serialise_item = Não foi possível serializar o item de `foreach`.
manifest.when.empty = A expressão `when` não pode estar vazia.
manifest.when.eval_error = Não foi possível avaliar a expressão `when` "{ $expr }".
manifest.when.template_error = Não foi possível renderizar o template `when` "{ $expr }".
manifest.target.vars_not_object = `vars` do alvo deve ser um objeto, obteve-se { $value }.
manifest.vars.entry_not_object = Uma entrada `vars` do manifesto deve ser um objeto.
manifest.field_not_string = O campo "{ $field }" deve ser uma string.
manifest.expression.parse_error = Não foi possível analisar a expressão { $name }.
manifest.expression.eval_error = Não foi possível avaliar a expressão { $name }.

# Diagnósticos das macros do manifesto.
manifest.macro.signature_missing_identifier = Falta um identificador na assinatura da macro.
manifest.macro.signature_missing_params = Faltam parâmetros na assinatura da macro.
manifest.macro.compile_failed = Não foi possível compilar a macro { $name }.
manifest.macro.sequence_invalid = As macros devem ser definidas como um mapeamento de nomes para templates.
manifest.macro.register_failed = Não foi possível registrar as macros do manifesto.
manifest.macro.not_initialised = O ambiente de macros não está inicializado.
manifest.macro.caller_invalid = O chamador da macro deve ser uma string.
manifest.macro.template_load_failed = Não foi possível carregar o template da macro.
manifest.macro.init_failed = Não foi possível inicializar o ambiente de macros.
manifest.macro.missing = A macro { $name } está ausente.

# Erros de glob do manifesto.
manifest.glob.unmatched_brace = Padrão glob inválido "{ $pattern }": "{ $character }" sem correspondência na posição { $position }.
manifest.glob.invalid_pattern = Padrão glob inválido "{ $pattern }": { $detail }.
manifest.glob.unknown_pattern_error = erro de padrão desconhecido.
manifest.glob.io_failed = O glob falhou para "{ $pattern }": { $detail }.
manifest.glob.unknown_io_error = erro de E/S desconhecido.

# Erros da representação intermediária.
ir.rule_not_found = A regra "{ $rule }" referenciada pelo alvo "{ $target }" não foi encontrada.
ir.multiple_rules = O alvo "{ $target }" deve referenciar uma única regra, obteve-se { $rules }.
ir.empty_rule = O alvo "{ $target }" deve referenciar uma regra.
ir.duplicate_outputs = Saídas duplicadas detectadas: { $outputs }.
ir.circular_dependency = Dependência circular detectada: { $cycle }.
ir.action_serialisation = Não foi possível serializar a ação: { $details }.
ir.invalid_command = Interpolação de comando inválida: { $snippet }.

# Erros de geração do Ninja.
ninja_gen.missing_action = Falta a ação "{ $id }" referenciada por uma aresta de build.
ninja_gen.format = Não foi possível formatar a saída do manifesto do Ninja.

# Validação de padrões de host.
host_pattern.empty = O padrão de host não pode estar vazio.
host_pattern.contains_scheme = O padrão de host "{ $pattern }" não pode incluir um esquema de URL.
host_pattern.contains_slash = O padrão de host "{ $pattern }" não pode incluir "/".
host_pattern.missing_suffix = O padrão de host "{ $pattern }" deve incluir um sufixo depois de "*.".
host_pattern.empty_label = O padrão de host "{ $pattern }" contém um rótulo vazio.
host_pattern.invalid_chars = O padrão de host "{ $pattern }" contém caracteres inválidos.
host_pattern.invalid_label_edge = Os rótulos do padrão de host "{ $pattern }" não podem começar nem terminar com "-".
host_pattern.label_too_long = O padrão de host "{ $pattern }" contém um rótulo com mais de 63 caracteres.
host_pattern.too_long = O padrão de host "{ $pattern }" excede o limite de 255 caracteres.

# Política de rede.
network_policy.scheme.empty = O esquema não pode estar vazio.
network_policy.scheme.invalid = O esquema "{ $scheme }" contém caracteres inválidos.
network_policy.allowlist.empty = A lista de hosts permitidos não pode estar vazia.
network_policy.scheme.not_allowed = O esquema "{ $scheme }" não é permitido.
network_policy.missing_host = A URL não tem host.
network_policy.host.blocked = O host "{ $host }" está bloqueado pela política.
network_policy.host.not_allowlisted = O host "{ $host }" não está na lista de permitidos.

# Configuração da biblioteca padrão.
stdlib.config.default_fetch_cache_invalid = O caminho padrão do cache do fetch deve ser relativo.
stdlib.config.default_which_cache_invalid = A capacidade padrão do cache do which deve ser positiva.
stdlib.config.workspace_root_absolute = O caminho da raiz do workspace deve ser absoluto.
stdlib.config.fetch_response_limit_positive = O limite de resposta do fetch deve ser positivo.
stdlib.config.command_output_limit_positive = O limite de captura da saída dos comandos deve ser positivo.
stdlib.config.command_stream_limit_positive = O limite de streaming dos comandos deve ser positivo.
stdlib.config.which_cache_capacity_positive = A capacidade do cache do which deve ser positiva.
stdlib.config.skip_dir_empty = As entradas de diretórios ignorados não podem estar vazias.
stdlib.config.skip_dir_navigation = As entradas de diretórios ignorados não podem conter "..".
stdlib.config.skip_dir_separator = As entradas de diretórios ignorados não podem conter separadores de caminho.
stdlib.config.fetch_cache_empty = O caminho do cache do fetch não pode estar vazio.
stdlib.config.fetch_cache_not_relative = O caminho do cache do fetch deve ser relativo, obteve-se { $path }.
stdlib.config.fetch_cache_escapes = O caminho do cache do fetch não pode sair do workspace: { $path }.
stdlib.config.open_workspace_root = Não foi possível abrir o diretório atual como raiz do workspace da stdlib.
stdlib.config.resolve_cwd = Não foi possível resolver o diretório atual como raiz do workspace da stdlib.
stdlib.config.cwd_non_utf8 = O diretório atual contém componentes que não são UTF-8: { $path }.

# Diagnósticos do auxiliar fetch.
stdlib.fetch.url_invalid = URL inválida "{ $url }": { $details }.
stdlib.fetch.disallowed = A URL "{ $url }" não é permitida: { $details }.
stdlib.fetch.failed = Não foi possível baixar "{ $url }": { $details }.
stdlib.fetch.cache_read_failed = Não foi possível ler a entrada de cache "{ $name }": { $details }.
stdlib.fetch.cache_open_failed = Não foi possível abrir a entrada de cache "{ $name }": { $details }.
stdlib.fetch.response_read_failed = Não foi possível ler a resposta de "{ $url }": { $details }.
stdlib.fetch.response_buffer_overflow = Estouro do buffer ao ler "{ $url }".
stdlib.fetch.cache_write_failed = Não foi possível gravar o cache de "{ $url }": { $details }.
stdlib.fetch.response_limit_exceeded = A resposta de "{ $url }" excedeu o limite de { $limit } bytes.
stdlib.fetch.cache_limit_exceeded = A resposta em cache "{ $name }" excedeu o limite de { $limit } bytes.
stdlib.fetch.io_failed = { $action } falhou para { $path }: { $details }.
stdlib.fetch.action.sync_cache = sincronizar o cache do fetch
stdlib.fetch.action.create_cache_dir = criar o diretório de cache do fetch
stdlib.fetch.action.open_cache_dir = abrir o diretório de cache do fetch
stdlib.fetch.action.stat_cache = consultar a entrada de cache do fetch
stdlib.fetch.action.open_cache_entry = abrir a entrada de cache do fetch

# Diagnósticos do auxiliar de comandos.
stdlib.command.location = comando "{ $command }" no template "{ $template }"
stdlib.command.spawn_failed = Não foi possível iniciar { $location }: { $details }.
stdlib.command.io_failed = { $location } falhou: { $details }.
stdlib.command.closed_input_early = A entrada foi fechada antes de concluir a gravação para o comando.
stdlib.command.broken_pipe = Pipe quebrado ao executar { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } foi encerrado por um sinal.
stdlib.command.exited_with_status = { $location } terminou com status { $status }.
stdlib.command.output_limit_exceeded = { $location } excedeu o limite de { $mode } de { $limit } bytes para { $stream }.
stdlib.command.timeout = { $location } excedeu o tempo limite de { $seconds } segundos.
stdlib.command.exit_status_suffix = (status de saída { $status })
stdlib.command.signal_suffix = (encerrado por um sinal)
stdlib.command.shell.empty = O comando de shell não pode estar vazio.
stdlib.command.grep.empty_pattern = O padrão do grep não pode estar vazio.
stdlib.command.grep.flags_not_string = As flags do grep devem ser strings.
stdlib.command.quote.invalid = Não foi possível colocar { $arg } entre aspas: { $details }.
stdlib.command.quote.line_break = Argumentos com retornos de carro ou quebras de linha não podem ser protegidos com segurança.
stdlib.command.input_undefined = O valor de entrada não está definido.
stdlib.command.tempfile.root_required = A raiz do workspace é necessária para criar arquivos temporários de comandos.
stdlib.command.tempfile.create_failed = Não foi possível criar o arquivo temporário do comando: { $details }.
stdlib.command.options.invalid_utf8 = A chave de uma opção do comando deve ser UTF-8 válido.
stdlib.command.option.mode_not_string = O modo de saída deve ser uma string.
stdlib.command.options.invalid_type = As opções do comando devem ser um objeto.
stdlib.command.output.mode_unsupported = Modo de saída sem suporte "{ $mode }".
stdlib.command.output.mode.capture = captura
stdlib.command.output.mode.streaming = streaming
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnósticos do auxiliar de caminhos.
stdlib.path.io.failed = { $action } falhou para { $path } ({ $label }).
stdlib.path.io.failed_with_detail = { $action } falhou para { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $action } falhou para { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = não encontrado
stdlib.path.io.permission_denied = permissão negada
stdlib.path.io.already_exists = já existe
stdlib.path.io.invalid_input = entrada inválida
stdlib.path.io.invalid_data = dados inválidos
stdlib.path.io.timed_out = tempo esgotado
stdlib.path.io.interrupted = interrompido
stdlib.path.io.would_block = bloquearia
stdlib.path.io.write_zero = nenhum byte foi gravado
stdlib.path.io.unexpected_eof = fim de arquivo inesperado
stdlib.path.io.broken_pipe = pipe quebrado
stdlib.path.io.connection_refused = conexão recusada
stdlib.path.io.connection_reset = conexão redefinida
stdlib.path.io.connection_aborted = conexão abortada
stdlib.path.io.not_connected = sem conexão
stdlib.path.io.addr_in_use = endereço em uso
stdlib.path.io.addr_not_available = endereço indisponível
stdlib.path.io.out_of_memory = sem memória
stdlib.path.io.unsupported = sem suporte
stdlib.path.io.file_too_large = arquivo grande demais
stdlib.path.io.resource_busy = recurso ocupado
stdlib.path.io.executable_busy = executável ocupado
stdlib.path.io.deadlock = impasse
stdlib.path.io.crosses_devices = cruza dispositivos
stdlib.path.io.too_many_links = links em excesso
stdlib.path.io.invalid_filename = nome de arquivo inválido
stdlib.path.io.arg_list_too_long = lista de argumentos longa demais
stdlib.path.io.stale_handle = identificador de arquivo de rede obsoleto
stdlib.path.io.storage_full = armazenamento cheio
stdlib.path.io.not_seekable = não permite posicionamento
stdlib.path.io.network_down = rede fora do ar
stdlib.path.io.network_unreachable = rede inacessível
stdlib.path.io.host_unreachable = host inacessível
stdlib.path.io.other = erro de E/S
stdlib.path.action.canonicalize = canonizar
stdlib.path.action.open_directory = abrir o diretório
stdlib.path.action.stat = consultar
stdlib.path.action.read = ler
stdlib.path.action.open_file = abrir o arquivo
stdlib.path.with_suffix.empty_separator = with_suffix exige um separador não vazio.
stdlib.path.relative_to.mismatch = { $path } não é relativo a { $root }.
stdlib.path.expanduser.unsupported = A expansão de ~ para um usuário específico não tem suporte.
stdlib.path.expanduser.no_home = Não é possível expandir ~: nenhuma variável de ambiente do diretório pessoal está definida.
stdlib.path.contents.unsupported_encoding = Codificação sem suporte "{ $encoding }".
stdlib.path.hash.unsupported_algorithm = Algoritmo de hash sem suporte "{ $algorithm }".
stdlib.path.hash.unsupported_algorithm_legacy = Algoritmo de hash sem suporte "{ $algorithm }" (habilite o recurso "{ $feature }").

# Diagnósticos dos auxiliares de coleções.
stdlib.collections.flatten.expected_sequence = O flatten esperava itens de uma sequência, mas encontrou { $kind }.
stdlib.collections.group_by.empty_attribute = O group_by exige um atributo não vazio.
stdlib.collections.group_by.unresolved = O group_by não conseguiu resolver "{ $attr }" em um item do tipo { $kind }.

# Diagnósticos dos auxiliares de tempo.
stdlib.time.offset.invalid = O deslocamento de now "{ $offset }" é inválido: esperava-se "+HH:MM[:SS]" ou "Z".
stdlib.time.timedelta.overflow = Estouro de timedelta ao somar { $component }.
stdlib.time.label.weeks = semanas
stdlib.time.label.days = dias
stdlib.time.label.hours = horas
stdlib.time.label.minutes = minutos
stdlib.time.label.seconds = segundos
stdlib.time.label.milliseconds = milissegundos
stdlib.time.label.microseconds = microssegundos
stdlib.time.label.nanoseconds = nanossegundos

# Diagnósticos do auxiliar which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] comando "{ $command }" não encontrado após verificar { $count } entradas do PATH. Prévia: { $preview }
stdlib.which.not_found.hint.cwd_auto = Segmentos vazios do PATH são ignorados; use cwd_mode="auto" para incluir o diretório de trabalho.
stdlib.which.not_found.hint.cwd_always = Defina cwd_mode="always" para incluir o diretório atual.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] o comando "{ $command }" em "{ $path }" não existe ou não é executável.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <vazio>
stdlib.which.path_entry.non_utf8 = A entrada nº { $index } do PATH contém caracteres que não são UTF-8; o Netsuke exige caminhos UTF-8.
stdlib.which.command.empty = O which exige uma string não vazia.
stdlib.which.cwd_mode.invalid = cwd_mode deve ser "auto", "always" ou "never", obteve-se "{ $mode }".
stdlib.which.cwd.resolve_failed = Não foi possível resolver o diretório atual: { $details }.
stdlib.which.cwd.non_utf8 = O diretório atual contém componentes que não são UTF-8.
stdlib.which.canonicalize_failed = Não foi possível canonizar "{ $path }": { $details }.
stdlib.which.is_executable = Não foi possível verificar se "{ $path }" é executável: { $details }.
stdlib.which.canonicalize_non_utf8 = O caminho canônico contém componentes que não são UTF-8.
stdlib.which.workspace_non_utf8 = O caminho do workspace contém componentes que não são UTF-8 ao resolver o comando "{ $command }": { $path }.
stdlib.which.walkdir_error = Erro ao percorrer o workspace durante a resolução do comando: { $details }.

# Registro da biblioteca padrão.
stdlib.register.open_dir = Não foi possível abrir o diretório atual para o registro da stdlib.
stdlib.register.resolve_dir = Não foi possível resolver o diretório atual para o registro da stdlib.
stdlib.register.dir_non_utf8 = O diretório atual contém componentes que não são UTF-8: { $path }.

# Relatório de status para o modo de saída acessível.
status.state.pending = pendente
status.state.running = em andamento
status.state.done = concluída
status.state.failed = falhou
status.stage.label = Etapa { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Tarefa { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Lendo o arquivo de manifesto
status.stage.initial_yaml_parsing = Analisando o documento YAML
status.stage.template_expansion = Expandindo as diretivas dos templates
status.stage.final_rendering = Desserializando e renderizando os valores do manifesto
status.stage.ir_generation_validation = Construindo e validando o grafo de dependências
status.stage.ninja_synthesis = Sintetizando o plano de build do Ninja
status.stage.ninja_synthesis_execute = Sintetizando o plano do Ninja e executando { $tool }
status.stage.graph_rendering = Renderizando o artefato do grafo
status.stage.graph_rendering_with_tool = Renderizando { $tool }
status.complete = { $tool }: operação concluída.
status.timing.summary_header = Resumo de tempos por etapa:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Tempo total do pipeline: { $duration }
status.tool.build = Build
status.tool.clean = Limpeza
status.tool.graph = Grafo
status.tool.graph_html = Grafo (HTML)
status.tool.generate = Geração

# Textos do renderizador HTML do grafo.
graph.html.title = Grafo de build do Netsuke
graph.html.heading = Grafo de build do Netsuke
graph.html.description = Grafo de build renderizado pelo Netsuke
graph.html.outline.summary = Alvos e dependências (esboço em texto)
graph.html.outline.no_inputs = Sem entradas
graph.html.noscript.notice = O JavaScript está desativado. O esboço em texto acima contém o grafo completo; o código DOT vem a seguir.

# Prefixos semânticos para a saída acessível.
semantic.prefix.error = Erro:
semantic.prefix.warning = Aviso:
semantic.prefix.success = Sucesso:
semantic.prefix.info = Info:
semantic.prefix.timing = Tempos:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Exemplos de formas plurais para tradutores.
# O português usa as categorias CLDR `one` e `other`, assim como o idioma
# de origem.
example.files_processed = { $count ->
    [one] { $count } arquivo processado.
   *[other] { $count } arquivos processados.
}

example.errors_found = { $count ->
    [0] Nenhum erro encontrado.
    [one] { $count } erro encontrado.
   *[other] { $count } erros encontrados.
}
