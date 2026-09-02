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

最新发布的预发布版本是Netsuke
v0.1.0-beta3（此前为v0.1.0-beta2），可从crates.io获取；若可以使用
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

`Netsukefile`可以执行命令并使用非纯的模板辅助函数；应以对待
`Makefile`同样的谨慎态度对待它
：在运行不受信任的清单之前先进行审查；Netsuke会对受支持的路径替换加引号，但它并非沙箱。

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
