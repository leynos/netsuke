<!-- markdownlint-disable MD013 MD033 MD041 -->

<div align="center">

[English](README.md) | [Deutsch](README.de.md) | [Español](README.es.md) |
[Français](README.fr.md) | [日本語](README.ja.md) |
[Português do Brasil](README.pt-BR.md) | [简体中文](README.zh-CN.md)

</div>

<!-- markdownlint-enable MD013 MD033 MD041 -->

# 🧵 Netsuke

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](
https://deepwiki.com/leynos/netsuke)

*一个友好的构建系统编译器：输入YAML和Jinja，输出Ninja。*

Netsuke能将易读的`Netsukefile`转换为经过验证的静态Ninja构建图
，它把动态工作保留在更高层级的清单中，并将快速的增量执行交给
[Ninja](https://ninja-build.org/)完成。

网站：<https://df12.studio/netsuke>

______________________________________________________________________

## 为什么选择Netsuke？

- **可读的清单**：使用YAML描述规则、目标、依赖项和默认值，而非对制表符敏感的语言。
- **动态规划**：在Netsuke创建构建图之前，使用Jinja变量、宏、
  `foreach`、`when`和glob匹配。
- **静态执行**：在运行任何构建命令之前，检查生成的Ninja文件或渲染构建图。
- **实用的诊断信息**：获取具备源码感知能力的错误、本地化输出、进度报告，以及规范的
  `--json`机器可读命令输出。
- **不限定工具链**：对Rust、C、Python、Web项目，或任何可以通过命令构建的内容，都使用同一套清单模型。

______________________________________________________________________

## 快速开始

### 前提条件

Netsuke目前需要：

- 位于`PATH`中的[Ninja](https://ninja-build.org/)；
- 若从源码安装，则需要[`rust-toolchain.toml`](rust-toolchain.toml)
  中锁定的带日期Rust nightly工具链（在检出的仓库中`rustup`会自动安装它
  ）；Netsuke使用Polonius借用检查器构建，该检查器在nightly中默认启用，并在稳定之前将持续保持nightly专属，参见
  [ADR-006](docs/adr-006-adopt-polonius-nightly-toolchain.md)。

### 安装

最新发布的预发布版本是Netsuke v0.1.0-beta3，可从crates.io获取；若可以使用
[`cargo binstall`](https://github.com/cargo-bins/cargo-binstall)
，则优先使用它：它会获取预构建的发行版二进制文件，从而避免下文提到的工具链要求。

```sh
cargo binstall netsuke-build
```

改为从注册表构建时是在仓库检出之外运行的，因此不会自动应用锁定的工具链；需要显式选择该工具链：

```sh
rustup toolchain install nightly-2026-08-23
cargo +nightly-2026-08-23 install netsuke-build
```

预构建的安装程序可从
[v0.1.0-beta3 GitHub release](https://github.com/leynos/netsuke/releases/tag/v0.1.0-beta3)
获取：

| 平台    | 架构                                | 软件包                          |
| ------- | ----------------------------------- | ------------------------------- |
| Linux   | x86-64（`amd64`）和Arm64（`arm64`） | Debian（`.deb`）和RPM（`.rpm`） |
| macOS   | Intel x86-64和Apple silicon Arm64   | 安装包（`.pkg`）                |
| Windows | x64和Arm64                          | Windows Installer（`.msi`）     |

Linux软件包会安装`netsuke`的手册页，并声明`ninja-build`为依赖项
；使用macOS或Windows安装程序时，必须单独安装Ninja；Windows MSI会安装到
`C:\Program Files\netsuke`，且不会更新`PATH`
；SHA-256校验和文件会随附独立二进制文件以及配套的帮助文档和许可证文件；在v0.1.0-beta3中，安装程序包不附带校验和文件；关于平台专属命令和Windows设置，参见
[用户指南](docs/users-guide.md#install-netsuke)。

要使用Cargo安装当前的源码检出：

```sh
git clone https://github.com/leynos/netsuke.git
cd netsuke
cargo install --path .
```

### 你的第一次构建

创建一个新目录，并添加一个名为`Netsukefile`的文件：

```yaml
netsuke_version: "1.0.0"

targets:
  - name: hello.txt
    command: "echo 'Hello from Netsuke!' > hello.txt"

defaults:
  - hello.txt
```

运行Netsuke，然后查看结果：

```sh
netsuke
cat hello.txt
```

第二条命令会打印`Hello from Netsuke!`；关于变量、模板和`foreach`，参见
[快速入门指南](docs/quickstart.md)，然后可使用
[模板标准库指南](docs/stdlib-yaml-and-jinja-guide.md)
了解每一个路径、集合、文件系统、时间、命令、环境、glob和网络辅助函数。

______________________________________________________________________

## 目前已支持的功能

Netsuke v0.1.0-beta3的核心构建系统编译器提供以下功能：

- YAML 1.2清单解析，包含重复键检测和模式验证；
- Jinja变量、宏、`foreach`、`when`
  、glob匹配、环境辅助函数、可执行文件发现，以及可选启用的网络辅助函数；
- 可复用的规则、目标、操作、默认值，以及显式、隐式和仅顺序依赖项；
- 通过`netsuke help targets`进行目标和操作发现，包括不渲染配方的条件性条目；
- 具有确定性的中间构建图，包含重复输出、缺失规则和循环依赖检查；
- 默认通过Windows PowerShell执行Windows传统配方，并提供显式选用的Git
  Bash或MSYS2兼容路径；
- Ninja文件的生成与执行，以及`clean`和独立清单生成；
- 可复现的依赖图，格式为Graphviz DOT或自包含、无障碍的HTML；
- 分层配置、本地化输出、无障碍偏好设置、进度报告、阶段耗时，以及带版本号的JSON结果或诊断信息；
- 单元测试、行为测试、集成测试、属性测试、快照测试，以及初步的Kani验证覆盖。

beta3版本还支持仅依赖的操作和目标聚合：`deps`列表非空的节点可以省略配方。

______________________________________________________________________

## 安全与命令插值

`Netsukefile`会执行命令，也可以使用有副作用的模板辅助函数。请像对待
`Makefile`一样谨慎：运行不受信任的清单之前先进行审查。Netsuke可以减少
某些引号错误，但它不是沙箱。在POSIX shell路径中，你写入的反引号对
如果用于命令替换，就会由shell执行。

**Netsuke无法防护的内容。** 手写反引号和 `$( … )` 都可以执行命令。
在Unix上，Ninja会将命令文本传给 `sh -c`；Netsuke不会清理这些文本。

**模板中的值不会自动加引号。** 任意Jinja值、`raw` 块和手写shell片段
都会成为普通配方文本。不要将不受信任的值插入shell命令：路径占位符的
加引号处理无法保护这些值。

**Netsuke会改写什么。** 只有 `{{ ins }}` 和 `{{ outs }}` 是Netsuke 标记。
`command:` 和 `script:` 配方使用相同的标记集合。下表中的所有 美元符号形式以及
`$PATH` 在两种配方中仍然是shell变量。Netsuke会为
Ninja将美元符号加倍，使所选shell收到的内容保持不变；它不会暴露Ninja
自身的规则变量 `$in` 和 `$out`。

表1：会被改写为输入或输出路径的形式（`yes` 表示在配方的有效文本中改写；
请参阅下方说明）。

| 形式                                                | `command:`  | `script:`   |
| --------------------------------------------------- | ----------- | ----------- |
| `{{ ins }}`                                         | yes[^inert] | yes[^inert] |
| `{{ outs }}`                                        | yes[^inert] | yes[^inert] |
| `$in`, `$out`, `$ins`, `$outs`, `$input`, `$output` | no          | no          |

[^inert]: 在 POSIX 和 Bash 路径中，Netsuke 会将注释和 heredoc 正文中的标记原样
    复制为内部令牌，而不会展开；heredoc 分隔符中的标记会展开。包括 PowerShell 在内的
    `script:` 配方使用同一个兼容 POSIX 的扫描器；PowerShell `command:` 配方则遵循
    单独的插值规则。请勿在注释或 heredoc 正文中使用标记：内部令牌可能会保留在生成的
    配方中。

例如，若已有 `input.txt`，以下POSIX清单会将其内容复制到 `output.txt`，
并检查shell的 `PATH` 是否非空：

```yaml
netsuke_version: '1.0.0'
targets:
  - name: output.txt
    sources: input.txt
    command: 'cat {{ ins }} > {{ outs }} && test -n "$PATH"'
defaults: [output.txt]
```

**Netsuke会为哪些内容加引号。** 只有Netsuke自己的路径替换会自动进行
shell加引号。POSIX和Bash使用 `shell-quote` 及基于上下文的编码；
PowerShell使用自己的字面量编码，并拒绝带引号区域中的标记。若已有
`input file.txt`，以下POSIX示例会将输入路径作为一个参数传递，并生成
`output.txt`：

```yaml
netsuke_version: '1.0.0'
targets:
  - name: output.txt
    sources: input file.txt
    command: 'cat {{ ins }} > {{ outs }}'
defaults: [output.txt]
```

**反引号与命令替换。** POSIX和Bash中，Netsuke会拒绝位于反引号命令
替换中的标记；所有shell路径中也会拒绝位于 `$( … )` 中的标记，两条
规则均适用于两种配方。PowerShell使用反引号作为转义符，因此不受反引号
限制，但仍会拒绝 `$( … )` 中的标记。这一标记边界是明确保证的不变量。

此外，POSIX和Bash的 `command:` 配方目前会拒绝替换后反引号总数为奇数
的命令。这项保守计数也包括单引号内的反引号；它不是shell解析器。 `script:`
配方和PowerShell不执行此计数。未来版本可能在不造成破坏性
变更的情况下接受更多内容。作者写入且反引号成对的命令替换仍可使用。

以下POSIX清单会在Ninja运行前被拒绝。运行 `netsuke --json --locale en-GB`
可查看原因： `Invalid command interpolation:`，后面附有出错片段；默认的人类可读
输出可能只显示构建图失败这一外层错误：

```yaml
netsuke_version: '1.0.0'
targets:
  - name: output.txt
    sources: input.txt
    command: 'echo `cat {{ ins }}` > {{ outs }}'
defaults: [output.txt]
```

**`shlex` 检查。** POSIX和Bash的 `command:` 配方在替换后必须通过 `shlex::split`
，否则会在转换为中间表示时收到相同的本地化诊断。`script:`
配方和PowerShell跳过此检查。Netsuke不会执行返回的令牌。`shlex` 不会
进行展开，并将反引号视为普通字符：通过检查并不意味着命令安全。不要
将它用作注入检查。

拒绝行为属于可观察契约的一部分，但确切的可接受输入集合不属于稳定性
承诺：它取决于构建Netsuke时解析到的 `shlex` 版本。未来版本接受以前拒绝
的文本不属于破坏性变更；拒绝以前接受的文本则是缺陷，应记录在变更日志
中，而不是作为策略变更处理。

**稳定性与延伸阅读。** Netsuke尚未达到1.0；接口仍可能变化。上述有限
保证并不意味着任意配方都是安全的。shell路径的具体机制见
[用户指南中的安全边界](docs/users-guide.md#review-the-safety-boundary)，
相关决策见[ADR-027](docs/adr-027-command-placeholder-contract.md)。

______________________________________________________________________

## 发布与开发状态

v0.1.0-beta3版本是面向早期采用者的实用预览版，并不代表Netsuke已经完成，也不代表每个接口都已稳定；编译器管线和普通本地构建工作流已相当完善，但命令行界面、配置词汇和高级配方模型仍处于预稳定阶段。

在自动化流程中锁定Netsuke的版本号，并预期在1.0版本发布之前，部分命令名称、标志、诊断模式和清单细节可能发生变化。

以下限制适用于beta3版本。

已知限制包括：

- 配方仍然是shell字符串：Unix脚本使用`/bin/sh -e`
  ，Windows传统配方默认使用Windows
  PowerShell，Windows的Bash兼容路径需要显式选用；结构化的可执行文件参数和配方环境映射尚未实现；
- 编译器自动生成的依赖导入（例如GCC depfile）已在计划之中，但尚未纳入清单模型；
- `--json`会为每条命令精确输出一份带版本号的结果或诊断文档
  ，但其模式在1.0版本之前仍可能变化；
- 尚未实现彩色渲染；
- 无障碍功能仍需通过辅助技术进行验证。

beta3版本通过引入Ninja感知的转义，修复了beta2中shell美元符号（`$`
）的限制，因此可以正常编写普通的shell表达式；使用字面shell美元符号表达式的beta2清单需要迁移，参见
[用户指南中的安全边界](docs/users-guide.md#review-the-safety-boundary)。

详见[安全与命令插值](#安全与命令插值)以及
[用户指南中的安全边界](docs/users-guide.md#review-the-safety-boundary)。

______________________________________________________________________

## 未来规划

首个正式版本发布之后的工作将围绕三个优先事项展开：

1. **稳定命令行契约**：加固规范的命令与标志名称、非交互式安全保障、稳定的退出状态、有边界的输出，以及带版本号的
   `--json`文档。
2. **让配方更安全、更清晰**：加入结构化的可执行文件参数、环境映射、编译器依赖导入，以及更好的条件性操作反馈。
3. **增强可信度**：扩大Kani和属性测试的覆盖范围，使用辅助技术验证无障碍性，并为终端渲染添加回归测试覆盖。

更长期的工作将探索机器可读上下文、配置文件、运行历史、产物交付，以及面向人类和智能体工作流的本地优先反馈；详细顺序和当前进度参见
[路线图](docs/roadmap.md)。

______________________________________________________________________

## 了解更多

- [快速入门指南](docs/quickstart.md)：五分钟内构建出成果。
- [用户指南](docs/users-guide.md)：清单与命令参考。
- [设计文档](docs/netsuke-design.md)：架构与设计理念。
- [开发者指南](docs/developers-guide.md)：开发工作流与质量门禁。
- [路线图](docs/roadmap.md)：已完成的基础工作与计划中的工作。

______________________________________________________________________

## 许可证

ISC：详情参见[LICENSE](LICENSE)。

______________________________________________________________________

## 贡献指南

欢迎贡献；请先阅读[开发者指南](docs/developers-guide.md)，自动化贡献者还应遵循
[AGENTS.md](AGENTS.md)。
