//! Helen LSP constants — port of `helen/lsp/server.py` (HLD M12, v1.30.5).
//!
//! Keyword lists, types, snippet templates and hover descriptions.
//! Authoritative source: `helen/core/tokens.py` `_KEYWORD_MAP` (91 entries)
//! plus context keywords recognized by the parser as identifiers.

/// Formal keywords (from tokens.py `_KEYWORD_MAP`).
pub const HELLEN_KEYWORDS: &[&str] = &[
    // Agent keywords
    "agent",
    "main",
    "prompt",
    "description",
    "model",
    "temperature",
    "max-turns",
    "max-tokens",
    "tools",
    "streaming",
    // Variable declarations
    "let",
    "const",
    "shared",
    // Control flow
    "if",
    "else",
    "for",
    "in",
    "while",
    "break",
    "continue",
    "return",
    // Functions
    "fn",
    "call",
    "alias",
    // Error handling
    "try",
    "catch",
    "finally",
    "throw",
    "assert",
    // Pattern matching
    "match",
    "case",
    "branch",
    "default",
    // Imports
    "import",
    "as",
    // LLM keywords
    "llm",
    "act",
    // Concurrency (v1.18)
    "spawn",
    // Shared store (v1.12)
    "store",
    // Protocol/Interface (v1.7)
    "protocol",
    "impl",
    "is",
    // Agent functions block
    "functions",
    // Transcript (v1.29)
    "transcript",
    // Thinking mode (v1.36)
    "thinking-mode",
    "reasoning-effort",
    // Literals
    "true",
    "false",
    "null",
    // Chinese keywords (v1.10 — bilingual support)
    "设",
    "定义",
    "常量",
    "函数",
    "返回",
    "如果",
    "否则",
    "对于",
    "属于",
    "当",
    "中断",
    "继续",
    "匹配",
    "情况",
    "默认",
    "分支",
    "尝试",
    "捕获",
    "最终",
    "抛出",
    "断言",
    "且",
    "或", // v1.30.12: Chinese logical operators
    "真",
    "假",
    "空",
    "是",
    "智能体",
    "大模型",
    "执行",
    "分生",
    "提示词",
    "描述",
    "模型",
    "工具",
    "流式输出",
    "温度",
    "最大轮次",
    "最大tokens",
    "函数区",
    "主函",
    "导入",
    "作为",
    "协议",
    "实现",
    "共享",
    "别名",
    "仓库",
    "记录",
    "思考模式",
    "推理强度", // v1.36: thinking mode (formal keywords)
];

/// Context keywords: not in `_KEYWORD_MAP` (parsed as IDENTIFIER + context check).
pub const HELLEN_CONTEXT_KEYWORDS: &[&str] = &[
    // Async/concurrency
    "async",
    "await",
    // Channel (v1.18)
    "Channel",
    "send",
    "receive",
    "try_receive",
    "cancel",
    "close",
    "mailbox_select",
    // LLM callbacks (v1.21)
    "on_chunk",
    "on_complete",
    "on_tool_end",
    "on_media",
    "on_generate",
    // Multimodal (v1.17)
    "media",
    "provider",
    // Context management (v1.12, v1.19)
    "context",
    "memory",
    "persistent",
    "none",
    // Session resume (v1.27)
    "resume",
    // Test framework
    "expect",
    // Chinese context keywords
    "上下文",
    "记忆",
    "恢复会话",
    "逐块处理",
    "完成",
    "工具结束",
    "处理媒体",
    "生成",
    "媒体",
    "提供商", // v1.36: provider override (context keyword)
];

/// Agent property keywords (inside agent {} blocks).
pub const HELLEN_AGENT_PROPERTIES: &[&str] = &[
    "description",
    "model",
    "temperature",
    "max-turns",
    "max-tokens",
    "tools",
    "streaming",
    "prompt",
    "transcript",
    "描述",
    "模型",
    "温度",
    "最大轮次",
    "最大tokens",
    "工具",
    "流式输出",
    "提示词",
    "记录",
];

/// Built-in types.
pub const HELLEN_TYPES: &[&str] = &[
    "str", "int", "float", "bool", "list", "dict", "map", "any", "void", "number",
    // Union/Optional syntax hints
    "Optional", "Union", // Protocol/Agent types
    "Protocol", "Agent", // Literal type
    "Literal",
];

/// A snippet template (insertTextFormat=2, LSP snippet syntax).
pub struct Snippet {
    pub label: &'static str,
    pub detail: &'static str,
    pub insert_text: &'static str,
}

/// Snippet templates ($0 = final cursor, $1/$2 = tab stops).
pub const HELLEN_SNIPPETS: &[Snippet] = &[
    Snippet {
        label: "agent",
        detail: "Agent declaration block",
        insert_text: "agent ${1:AgentName} {\n    description \"${2:description}\"\n    model \"${3:model}\"\n    temperature ${4:0.7}\n    tools [${5}]\n    prompt {\n        {{${6:input}}}\n    }\n    functions {\n        ${7}\n    }\n    main {\n        $0\n    }\n}",
    },
    Snippet {
        label: "fn",
        detail: "Function declaration",
        insert_text: "fn ${1:name}(${2:args}): ${3:void} {\n    $0\n}",
    },
    Snippet {
        label: "llm act",
        detail: "LLM act with tool loop",
        insert_text: "llm act ${1:agent}(${2:prompt}) {\n    on_chunk {\n        ${3}\n    }\n    on_complete {\n        ${4}\n    }\n}$0",
    },
    Snippet {
        label: "llm if",
        detail: "LLM-routed conditional branch",
        insert_text: "llm if (${1:condition}) {\n    ${2:branch1}\n} else {\n    ${3:branch2}\n}$0",
    },
    Snippet {
        label: "shared store",
        detail: "Thread-safe shared store declaration",
        insert_text: "shared store ${1:StoreName} {\n    fields {\n        ${2}\n    }\n    methods {\n        ${3}\n    }\n}$0",
    },
    Snippet {
        label: "spawn",
        detail: "Spawn agent and return Channel",
        insert_text: "spawn ${1:Agent}(${2:args})$0",
    },
    Snippet {
        label: "match",
        detail: "Pattern matching block",
        insert_text: "match ${1:expression} {\n    case ${2:pattern} => {\n        ${3}\n    }\n    default => {\n        $0\n    }\n}",
    },
    Snippet {
        label: "try",
        detail: "Try/catch error handling",
        insert_text: "try {\n    ${1}\n} catch ${2:e} {\n    ${3}\n}$0",
    },
    Snippet {
        label: "if",
        detail: "If/else conditional",
        insert_text: "if ${1:condition} {\n    ${2}\n} else {\n    $0\n}",
    },
    Snippet {
        label: "for",
        detail: "For-in loop",
        insert_text: "for ${1:item} in ${2:collection} {\n    $0\n}",
    },
    Snippet {
        label: "while",
        detail: "While loop",
        insert_text: "while ${1:condition} {\n    $0\n}",
    },
    Snippet {
        label: "import",
        detail: "Import statement",
        insert_text: "import \"${1:path}\"${2: as ${3:alias}}$0",
    },
    Snippet {
        label: "protocol",
        detail: "Protocol declaration",
        insert_text: "protocol ${1:Name} {\n    ${2}\n}$0",
    },
    Snippet {
        label: "@sandbox",
        detail: "Sandbox agent decorator (tools=[])",
        insert_text: "@sandbox agent ${1:AgentName} {\n    $0\n}",
    },
    Snippet {
        label: "@open",
        detail: "Open agent decorator (can access module let)",
        insert_text: "@open agent ${1:AgentName} {\n    $0\n}",
    },
    Snippet {
        label: "@strict",
        detail: "Strict agent decorator (deep-copies shared let)",
        insert_text: "@strict agent ${1:AgentName} {\n    $0\n}",
    },
];

/// Keyword descriptions for hover.
pub const KEYWORD_DESCRIPTIONS: &[(&str, &str)] = &[
    ("agent", "Declare an agent (AI-native autonomous entity)"),
    ("fn", "Declare a function"),
    ("let", "Declare a mutable variable"),
    ("const", "Declare an immutable constant"),
    ("if", "Conditional branch"),
    ("else", "Alternative branch"),
    ("for", "Loop over a collection"),
    ("in", "Membership / iteration operator"),
    ("while", "Loop while condition is true"),
    (
        "match",
        "Pattern matching (range, type, wildcard, variable binding)",
    ),
    ("case", "A pattern match arm"),
    ("branch", "Branch arm (legacy)"),
    ("default", "Default match arm"),
    ("return", "Return a value from a function"),
    ("break", "Exit a loop"),
    ("continue", "Skip to next iteration"),
    ("try", "Try block for error handling"),
    ("catch", "Catch block for handling errors"),
    ("finally", "Block executed regardless of errors"),
    ("throw", "Raise an error"),
    ("assert", "Assert a condition (raises AssertionError)"),
    ("import", "Import a module"),
    ("as", "Alias an import"),
    ("llm", "LLM primitive (act / if)"),
    ("act", "LLM tool-calling loop"),
    ("spawn", "Spawn an agent and return a Channel (mailbox)"),
    (
        "Channel",
        "Inter-agent communication channel (spawn return type)",
    ),
    ("send", "Send a message through a Channel"),
    ("receive", "Blocking receive from a Channel"),
    ("try_receive", "Non-blocking receive from a Channel"),
    ("cancel", "Cancel a spawned agent"),
    ("close", "Close a Channel"),
    ("mailbox_select", "Multi-channel select (like Go select)"),
    ("shared", "Shared variable or shared store declaration"),
    ("store", "Thread-safe shared store (fields + methods)"),
    ("protocol", "Protocol declaration (structural typing)"),
    ("impl", "Protocol implementation"),
    ("is", "Type pattern in match"),
    ("alias", "Create a function alias"),
    ("functions", "Agent functions block (LLM-callable tools)"),
    ("main", "Agent main block (entry point)"),
    (
        "transcript",
        "Agent transcript control (none/memory/persistent)",
    ),
    ("prompt", "Agent prompt template"),
    ("description", "Agent description"),
    ("model", "Agent/model identifier"),
    ("temperature", "LLM sampling temperature"),
    ("max-turns", "Maximum LLM interaction turns"),
    ("max-tokens", "Maximum output tokens for LLM response"),
    ("thinking-mode", "Enable thinking/reasoning mode (v1.36)"),
    (
        "reasoning-effort",
        "Reasoning effort level: low/medium/high/max (v1.36)",
    ),
    ("provider", "Explicit provider override (v1.36)"),
    ("思考模式", "启用思考/推理模式 (v1.36)"),
    ("推理强度", "推理强度: low/medium/high/max (v1.36)"),
    ("提供商", "显式指定厂商 (v1.36)"),
    ("tools", "List of tools available to the agent"),
    ("streaming", "Enable streaming output"),
    ("async", "Async function marker"),
    ("await", "Await an async result"),
    ("call", "Explicit function call"),
    ("true", "Boolean true"),
    ("false", "Boolean false"),
    ("null", "Null / empty value"),
    (
        "context",
        "Context management (clear_context, compress_context)",
    ),
    ("memory", "In-memory transcript mode"),
    ("persistent", "Persistent (disk) transcript mode"),
    ("none", "No transcript recording (default)"),
    ("resume", "Resume a saved session"),
    ("expect", "Test expectation"),
    ("on_chunk", "Streaming callback: called for each text chunk"),
    (
        "on_complete",
        "Streaming callback: called when generation completes",
    ),
    ("on_tool_end", "Tool callback: called after a tool executes"),
    ("on_media", "Multimodal callback: called for media parts"),
    (
        "on_generate",
        "Generation callback: called before LLM request",
    ),
    ("media", "Multimodal media() function"),
    // Chinese keyword descriptions
    ("智能体", "声明一个智能体（AI 原生自主实体）"),
    ("函数", "声明一个函数"),
    ("设", "声明一个可变变量"),
    ("定义", "声明一个不可变常量（legacy alias for 设）"),
    ("常量", "声明一个不可变常量"),
    ("如果", "条件分支"),
    ("否则", "否则分支"),
    ("对于", "遍历集合"),
    ("属于", "成员/迭代运算符"),
    ("当", "当条件为真时循环"),
    ("返回", "从函数返回值"),
    ("中断", "退出循环"),
    ("继续", "跳到下一次迭代"),
    ("匹配", "模式匹配"),
    ("情况", "匹配分支"),
    ("默认", "默认匹配分支"),
    ("分支", "分支（legacy）"),
    ("尝试", "尝试块（错误处理）"),
    ("捕获", "捕获块"),
    ("最终", "最终块（无论是否出错都执行）"),
    ("抛出", "抛出错误"),
    ("断言", "断言条件"),
    ("大模型", "大模型原语（执行/如果）"),
    ("执行", "大模型工具调用循环"),
    ("分生", "分生（spawn）智能体并返回 Channel"),
    ("提示词", "智能体提示词模板"),
    ("描述", "智能体描述"),
    ("模型", "模型标识符"),
    ("温度", "LLM 采样温度"),
    ("最大轮次", "最大交互轮次"),
    ("最大tokens", "LLM 响应最大输出 token 数"),
    ("工具", "智能体可用工具列表"),
    ("流式输出", "启用流式输出"),
    ("函数区", "智能体函数区（LLM 可调用的工具）"),
    ("主函", "智能体主函数（入口）"),
    ("导入", "导入模块"),
    ("作为", "导入别名"),
    ("协议", "协议声明（结构化类型）"),
    ("实现", "协议实现"),
    ("共享", "共享变量或共享仓库"),
    ("别名", "函数别名"),
    ("仓库", "线程安全的共享仓库"),
    ("记录", "智能体 transcript 控制"),
    ("真", "布尔真"),
    ("假", "布尔假"),
    ("空", "空值"),
    ("是", "类型模式匹配"),
    ("且", "逻辑与（AND）"),
    ("或", "逻辑或（OR）"),
    ("上下文", "上下文管理"),
    ("记忆", "内存 transcript 模式"),
    ("恢复会话", "恢复已保存的会话"),
    ("逐块处理", "流式回调：处理每个文本块"),
    ("完成", "流式回调：生成完成时调用"),
    ("工具结束", "工具回调：工具执行后调用"),
    ("处理媒体", "多模态回调：处理媒体部分"),
    ("生成", "生成回调：LLM 请求前调用"),
    ("媒体", "多模态 media() 函数"),
];

/// Look up a keyword description (Python `_KEYWORD_DESCRIPTIONS.get`).
pub fn keyword_description(keyword: &str) -> Option<&'static str> {
    KEYWORD_DESCRIPTIONS
        .iter()
        .find(|(k, _)| *k == keyword)
        .map(|(_, v)| *v)
}

/// Server version string (Python uses `helen.__version__`).
pub const HELEN_LSP_VERSION: &str = "1.45.0";
