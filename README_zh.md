# Git Enhancer

`git-enhancer` 是一个命令行工具，它通过 AI 功能增强您的 Git 工作流。它可以自动生成提交信息，并为 Git 命令提供 AI 驱动的解释。

## 功能特性

-   **AI 驱动的提交信息**：通过分析您暂存的 diff，使用大型语言模型 (LLM) 自动生成提交信息。
-   **AI 驱动的 Git 命令解释**：直接在您的终端中获取对 Git 命令及其选项的 AI 生成的解释。
-   **标准 Git Commit 传递**：与您现有的 `git commit` 工作流无缝集成。如果您不使用 AI 功能，它的行为与标准 `git commit` 相同。
-   **可配置**：允许自定义 AI 模型、API 端点、temperature (温度) 和系统提示。
-   **追踪/日志**：提供详细的日志用于调试和监控。

## 安装

1.  **先决条件**：
    *   Rust 和 Cargo：[安装 Rust](https://www.rust-lang.org/tools/install)
    *   Git：必须已安装并在您的 PATH 环境变量中。
    *   （可选）一个 OpenAI 兼容的 LLM API 端点（例如，本地运行的 Ollama 模型，或远程服务）。

2.  **从源码构建**：
    ```bash
    git clone <repository_url> # 请替换为实际的仓库 URL
    cd git-enhancer
    cargo build --release
    ```
    可执行文件将位于 `target/release/git-enhancer`。您可以将其复制到您 PATH 环境变量中的目录，例如 `~/.local/bin/` 或 `/usr/local/bin/`。

    ```bash
    # 示例：
    # mkdir -p ~/.local/bin
    # cp target/release/git-enhancer ~/.local/bin/
    # 确保 ~/.local/bin 在您的 PATH 中
    ```

## 配置

`git-enhancer` 在其根目录中使用 `config.json` 文件进行 AI 相关设置，并使用 `prompts/commit-prompt` 文件作为生成提交信息时使用的系统提示。

1.  **创建 `config.json`**：
    将示例配置文件 `config.example.json` 复制到 `git-enhancer` 项目的根目录下，并重命名为 `config.json`（如果它是全局安装并且期望在那里找到配置文件，则复制到运行可执行文件的目录——这可能需要针对全局安装进行调整）。

    ```bash
    cp config.example.json config.json
    ```

    编辑 `config.json` 并填入您的首选设置：
    ```json
    {
      "api_url": "http://localhost:11434/v1/chat/completions", // 您的 LLM API 端点
      "model_name": "qwen3:32b-q8_0",                        // 要使用的模型
      "temperature": 0.7,                                     // LLM temperature (温度)
      "api_key": "YOUR_API_KEY_IF_NEEDED"                   // API 密钥，如果您的端点需要
    }
    ```
    *   `api_url`: 您的 OpenAI 兼容的聊天补全端点的 URL。
    *   `model_name`: 您的 API 端点期望的特定模型标识符。
    *   `temperature`: 控制 AI 的创造力。较高的值意味着更具创造性/随机性，较低的值意味着更具确定性。
    *   `api_key`: 您的 API 密钥，如果服务需要。这是可选的。

2.  **自定义 `prompts/commit-prompt`**：
    `prompts/commit-prompt` 文件包含提供给 AI 的系统提示，以指导其生成提交信息。您可以编辑此文件以更改提交信息的风格、语气或特定要求。

    默认提示鼓励使用约定式提交 (conventional commit) 风格的信息。

    *注意：如果找不到 `config.json`，`git-enhancer` 将使用默认值，但如果缺少 `prompts/commit-prompt`，它将失败。*

## 使用方法

`git-enhancer` 主要作为 `git commit` 的包装器运行，但也引入了一个全局的 `--ai` 标志以提供更广泛的 AI 辅助。

### 1. AI 生成的提交信息

要让 AI 根据暂存的更改生成提交信息：

1.  像往常一样暂存您的更改：
    ```bash
    git add <file1> <file2> ...
    ```
2.  运行 `git-enhancer commit --ai`：
    ```bash
    git-enhancer commit --ai
    ```
    或者，如果您已为 `git enhancer` 或类似命令设置了别名：
    ```bash
    git enhancer commit --ai
    ```

    您还可以传递其他 `git commit` 参数：
    ```bash
    git-enhancer commit --ai -S  # 用于 GPG 签名
    ```

### 2. AI 驱动的命令解释与辅助

`git-enhancer` 使用 `--ai` 标志来激活多种 AI 辅助功能。其具体行为取决于 `--ai` 如何与其他命令和选项组合。核心原则是：
- `git-enhancer commit --ai` 用于生成提交信息。
- 在其他情况下，`--ai` 通常用于解释 Git 命令或其帮助文档。

以下是详细场景：

**a. 解释 Git 命令的帮助页面 (例如 `git-enhauser commit --help --ai`)**

当 `--ai` 标志与一个包含帮助选项 (`-h` 或 `--help`) 的 Git 命令一起使用时，`git-enhancer` 会获取该 Git 命令的标准帮助文档，并利用 AI 对其进行解释。

*   **预期行为**：AI 解释 Git 命令的帮助文档。
*   **示例**：
    ```bash
    # 场景：我想让 AI 解释 'git commit' 命令的帮助信息
    git-enhauser commit --help --ai

    # '--ai' 标志也可以在前面
    git-enhauser --ai commit --help

    # 解释 'git status --short' 的帮助信息
    git-enhauser status -s --help --ai
    ```

**b. 解释 Git 命令的实际功能 (例如 `git-enhauser --ai status -s`)**

如果使用了 `--ai` 标志，但命令中不包含帮助选项 (`-h` 或 `--help`)，并且该命令**不**是 `git-enhancer commit --ai`（用于生成提交信息），那么 AI 将会解释您指定的 Git 命令及其选项的用途和效果。

*   **预期行为**：AI 解释给定 Git 命令的功能。
*   **示例**：
    ```bash
    # 场景：我想让 AI 解释 'git status -s' 命令是做什么的
    git-enhauser --ai status -s

    # 获取 AI 对 'git log --oneline -n 5' 功能的解释
    git-enhauser --ai log --oneline -n 5

    # 解释 'git commit -m "..."' 命令本身的作用
    # (注意：这不会生成 AI commit message。要生成 AI commit message，请使用 'git-enhauser commit --ai')
    git-enhauser --ai commit -m "一条标准的提交信息"
    ```

**c. `--ai` 标志与 `git-enhancer commit --ai` (消息生成) 的特定交互**

`git-enhancer` 的核心功能之一是 `commit --ai` 用于自动生成提交信息。当 `--ai` 标志（可能意图用于解释）与这个特定的消息生成子命令组合时，**消息生成功能将优先执行**。

*   **预期行为**：即使存在多个 `--ai` 标志，或者 `--ai` 标志位置不同，只要最终可以解析为 `git-enhancer` 的 `commit` 子命令且其 `--ai` 选项被激活，就会执行 AI 提交信息生成。
*   **示例**：
    ```bash
    # 场景：明确使用 'commit --ai' 来生成 AI commit message (这是主要用法)
    git-enhauser commit --ai

    # 场景：即使存在一个可能被视为“全局解释”的 --ai 标志，
    # 但由于 'commit --ai' 是 git-enhancer 的一个特定 AI 功能（消息生成），
    # 因此消息生成优先。
    git-enhauser --ai commit --ai

    # 同样，以下命令也会触发 AI 提交信息生成，并包含 GPG 签名
    git-enhauser --ai commit --ai -S
    ```
    这种设计的目的是确保 `git-enhancer commit --ai` 作为生成提交信息的明确指令，其行为是稳定和可预测的，即使在与其他 `--ai` 用法组合时也是如此。

### 3. 标准提交信息 (无全局 `--ai` 时)

如果不使用全局 `--ai` 标志，`git-enhancer` 的行为如下：

-   **`git-enhancer` 特定子命令** (如 `commit`)：
    -   附带信息 (透传)：`git-enhancer commit -m "您的提交信息"`
    -   打开您配置的 Git 编辑器 (透传)：`git-enhancer commit`
    -   调用其自身的 AI 功能：`git-enhancer commit --ai` (如第1节所述)
-   **其他 Git 命令**：
    如果命令不是 `git-enhancer` 可识别的子命令，它将被直接传递给系统的 `git`。
    ```bash
    git-enhancer status -s  # 执行 'git status -s'
    git-enhancer branch my-feature # 执行 'git branch my-feature'
    ```

### 4. 日志记录

`git-enhancer` 使用 `tracing` 进行日志记录。默认情况下，日志会打印到标准错误输出。您可以使用 `RUST_LOG` 环境变量来控制日志级别。

示例：
```bash
RUST_LOG=debug git-enhancer commit --ai
```

## 工作流图 (AI 提交)

```mermaid
graph TD
    A[\"用户暂存更改: git add .\"] --> B{\"用户运行: git-enhancer commit --ai\"};
    B --> C{\"git-enhancer 启动\"};
    C --> D[\"加载 config.json 和 prompts/commit-prompt\"];
    D --> E[\"运行: git diff --staged\"];
    E --> F{\"有暂存的更改吗?\"};
    F -- \"否\" --> G[\"通知用户，退出或传递给 git commit\"];
    F -- \"是\" --> H[\"提取 diff 文本\"];
    H --> I[\"准备 AI 请求 (diff + 提示)\"];
    I --> J[\"发送请求到 LLM API\"];
    J --> K[\"接收 AI 生成的提交信息\"];
    K --> L{\"信息有效吗?\"};
    L -- \"否\" --> M[\"记录警告/错误，可能使用回退方案\"];
    L -- \"是\" --> N[\"构造: git commit -m \\\"<AI_MESSAGE>\\\"\"];
    N --> O[\"执行 git commit 命令\"];
    O --> P[\"记录成功/失败\"];
    P --> Q[\"退出\"];
```

## 开发

有关项目结构、贡献指南等更多详细信息，请参阅 `doc/DEVELOPMENT.md`。

### 开发者快速链接
- 构建: `cargo build`
- 运行测试: `cargo test` (添加测试后)
- 格式化: `cargo fmt`
- 代码检查: `cargo clippy`

## 许可证

本项目采用 [MIT 许可证](LICENSE)授权。（假设是 MIT，如果您选择此许可证，请添加一个 LICENSE 文件）
