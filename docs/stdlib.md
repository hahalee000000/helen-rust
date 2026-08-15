# Helen API Documentation

Auto-generated from source code analysis.

## Built-in Functions (collection)

| Function | Signature | Description |
|----------|-----------|-------------|
| `chunk` <br><sup>aka `分块`</sup> | `chunk(lst, size)` | Split into chunks |
| `entries` <br><sup>aka `键值对`</sup> | `entries(dict)` | Get dict entries |
| `every` <br><sup>aka `全部满足`</sup> | `every(lst, fn)` | Check all elements |
| `filter` <br><sup>aka `过滤`</sup> | `filter(lst, fn)` | Filter list by predicate |
| `find_if` <br><sup>aka `条件查找`</sup> | `find_if(lst, fn)` | Find element by predicate |
| `flatten` <br><sup>aka `展平`</sup> | `flatten(lst)` | Flatten nested lists |
| `get` <br><sup>aka `获取`</sup> | `get(dict, key, default?)` | Get value with default |
| `has_key` <br><sup>aka `包含键`</sup> | `has_key(dict, key)` | Check key exists |
| `keys` <br><sup>aka `键`</sup> | `keys(dict)` | Get dict keys |
| `make_set` <br><sup>aka `构造集合`</sup> | `make_set(items)` | Create set |
| `map` <br><sup>aka `映射`</sup> | `map(lst, fn)` | Map function over list |
| `merge` <br><sup>aka `合并`</sup> | `merge(*dicts)` | Merge dicts |
| `omit` <br><sup>aka `剔除`</sup> | `omit(dict, keys)` | Omit dict keys |
| `pick` <br><sup>aka `选取`</sup> | `pick(dict, keys)` | Pick dict keys |
| `reduce` <br><sup>aka `归约`</sup> | `reduce(lst, fn, initial?)` | Reduce list to value |
| `remove_key` <br><sup>aka `删除键`</sup> | `remove_key(dict, key)` | Remove single key |
| `set_difference` <br><sup>aka `集合差`</sup> | `set_difference(s1, s2)` | Set difference |
| `set_has` <br><sup>aka `集合包含`</sup> | `set_has(set, item)` | Check set membership |
| `set_intersection` <br><sup>aka `集合交`</sup> | `set_intersection(s1, s2)` | Set intersection |
| `set_key` <br><sup>aka `设置键`</sup> | `set_key(dict, key, value)` | Set key-value pair |
| `set_union` <br><sup>aka `集合并`</sup> | `set_union(s1, s2)` | Set union |
| `some` <br><sup>aka `部分满足`</sup> | `some(lst, fn)` | Check any element |
| `sort` <br><sup>aka `排序`</sup> | `sort(lst, compare?)` | Sort list |
| `unique` <br><sup>aka `去重`</sup> | `unique(lst)` | Remove duplicates |
| `values` <br><sup>aka `值`</sup> | `values(dict)` | Get dict values |
| `zip` <br><sup>aka `压缩`</sup> | `zip(*lists)` | Zip lists |

## Built-in Functions (concurrency)

| Function | Signature | Description |
|----------|-----------|-------------|
| `mailbox_select` <br><sup>aka `邮箱选择`</sup> | `mailbox_select(channels, timeout?)` | Receive first available message from multiple channels |

## Built-in Functions (context)

| Function | Signature | Description |
|----------|-----------|-------------|
| `clear_context` <br><sup>aka `清除上下文`</sup> | `clear_context()` | Clear conversation context |
| `compress_context` <br><sup>aka `压缩上下文`</sup> | `compress_context(strategy?)` | Compress conversation context |
| `compress_context_target` <br><sup>aka `定向压缩`</sup> | `compress_context_target(target, keep_recent?)` | Compress context by target type |
| `context_slice` <br><sup>aka `上下文切片`</sup> | `context_slice(start?, end?, role?)` | Extract a slice of conversation history |
| `context_stats` <br><sup>aka `上下文统计`</sup> | `context_stats()` | Return detailed statistics about current context |
| `context_usage` <br><sup>aka `上下文占用率`</sup> | `context_usage()` | Return current context usage ratio (0.0-1.0) |
| `delete_message` <br><sup>aka `删除消息`</sup> | `delete_message(uuid)` | Delete a message by UUID |
| `export_context` <br><sup>aka `导出上下文`</sup> | `export_context()` | Export current context as serializable dict |
| `fork_context` <br><sup>aka `分叉上下文`</sup> | `fork_context()` | Create a deep-copy snapshot of current context |
| `get_context_config` <br><sup>aka `获取上下文配置`</sup> | `get_context_config()` | Get current context management config |
| `get_message` <br><sup>aka `获取消息`</sup> | `get_message(uuid)` | Retrieve a message by UUID |
| `import_context` <br><sup>aka `导入上下文`</sup> | `import_context(data)` | Import a previously exported context |
| `insert_message` <br><sup>aka `插入消息`</sup> | `insert_message(role, content, position?)` | Insert a new message into context |
| `list_pinned_messages` <br><sup>aka `已钉住消息`, `钉住列表`</sup> | `list_pinned_messages()` | List all pinned messages (uuid, role, snippet) |
| `on_compression` <br><sup>aka `压缩回调`</sup> | `on_compression(callback?)` | Register callback for compression events |
| `on_context_overflow` <br><sup>aka `溢出回调`</sup> | `on_context_overflow(callback?)` | Register callback for context overflow |
| `pin_message` <br><sup>aka `钉住消息`</sup> | `pin_message(uuid)` | Pin a message by UUID (immune to compression) |
| `replace_message` <br><sup>aka `替换消息`</sup> | `replace_message(uuid, new_content)` | Replace a message's content by UUID |
| `restore_context` <br><sup>aka `恢复上下文`</sup> | `restore_context(session_id, invocation_id?, agent?, last_only?, include_subtree?)` | Restore active context from a previous transcript session |
| `search_context` <br><sup>aka `搜索上下文`</sup> | `search_context(query, role?, limit?)` | Search context for messages matching query |
| `set_cache_aware` <br><sup>aka `设置缓存感知`</sup> | `set_cache_aware(enabled)` | Enable/disable cache-aware compression at runtime |
| `set_compression_strategy` <br><sup>aka `设置压缩策略`</sup> | `set_compression_strategy(strategy)` | Set compression strategy at runtime |
| `set_context_window` <br><sup>aka `设置上下文窗口`</sup> | `set_context_window(tokens)` | Set context window size at runtime |
| `set_working_memory_enabled` <br><sup>aka `设置工作记忆开关`</sup> | `set_working_memory_enabled(enabled)` | Enable/disable working memory at runtime |
| `unpin_message` <br><sup>aka `取消钉住`</sup> | `unpin_message(uuid)` | Unpin a previously pinned message |
| `working_memory_clear` <br><sup>aka `清空工作记忆`</sup> | `working_memory_clear()` | Clear all working memory |
| `working_memory_get` <br><sup>aka `获取工作记忆`</sup> | `working_memory_get(key?)` | Read working memory contents |
| `working_memory_remove` <br><sup>aka `移除工作记忆`</sup> | `working_memory_remove(key, item?)` | Remove a working memory entry |
| `working_memory_set` <br><sup>aka `设置工作记忆`</sup> | `working_memory_set(key, value)` | Set a working memory field |

## Built-in Functions (core)

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs` <br><sup>aka `绝对值`</sup> | `abs(value)` | Absolute value |
| `bool` <br><sup>aka `布尔`</sup> | `bool(value)` | Convert to boolean |
| `dict` <br><sup>aka `字典`</sup> | `dict(value?)` | Convert to dict or create empty dict |
| `float` <br><sup>aka `浮点`</sup> | `float(value)` | Convert to float |
| `input` <br><sup>aka `输入`</sup> | `input(prompt?)` | Read line from stdin |
| `int` <br><sup>aka `整数`</sup> | `int(value)` | Convert to integer |
| `isinstance` <br><sup>aka `类型判断`</sup> | `isinstance(value, type_name)` | Type check |
| `len` <br><sup>aka `长度`</sup> | `len(value)` | Return length of string/list/dict |
| `list` <br><sup>aka `列表`</sup> | `list(iterable?)` | Convert to list or create empty list |
| `max` <br><sup>aka `最大值`</sup> | `max(*args)` | Maximum value |
| `min` <br><sup>aka `最小值`</sup> | `min(*args)` | Minimum value |
| `multiline_input` <br><sup>aka `多行输入`</sup> | `multiline_input(prompt?)` | Read multiple lines (empty line ends) |
| `print` <br><sup>aka `打印`</sup> | `print(*args)` | Print values to stdout |
| `range` <br><sup>aka `范围`</sup> | `range(start, stop?, step?)` | Integer range |
| `read_file` <br><sup>aka `读文件`</sup> | `read_file(path)` | Read file content |
| `str` <br><sup>aka `字符串`</sup> | `str(value)` | Convert to string |
| `type` <br><sup>aka `类型`</sup> | `type(value)` | Type name |

## Built-in Functions (crypto)

| Function | Signature | Description |
|----------|-----------|-------------|
| `choice` <br><sup>aka `随机选择`</sup> | `choice(items)` | Choose random item |
| `hash_file` <br><sup>aka `文件哈希`</sup> | `hash_file(path, algorithm?)` | Calculate hash of file |
| `hmac_sha256` <br><sup>aka `hmac_sha256`</sup> | `hmac_sha256(key, message)` | Calculate HMAC-SHA256 |
| `md5` <br><sup>aka `md5`</sup> | `md5(text)` | Calculate MD5 hash |
| `randint` <br><sup>aka `随机整数`</sup> | `randint(min, max)` | Generate random integer |
| `random` <br><sup>aka `随机`</sup> | `random()` | Generate random float |
| `random_base64` <br><sup>aka `随机Base64`</sup> | `random_base64(n)` | Generate random base64 string |
| `random_bytes` <br><sup>aka `随机字节`</sup> | `random_bytes(n)` | Generate random bytes as hex |
| `random_hex` <br><sup>aka `随机十六进制`</sup> | `random_hex(n)` | Generate random hex string |
| `sample` <br><sup>aka `随机抽样`</sup> | `sample(items, k)` | Sample items randomly |
| `sha1` <br><sup>aka `sha1`</sup> | `sha1(text)` | Calculate SHA1 hash |
| `sha256` <br><sup>aka `sha256`</sup> | `sha256(text)` | Calculate SHA256 hash |
| `sha512` <br><sup>aka `sha512`</sup> | `sha512(text)` | Calculate SHA512 hash |
| `shuffle` <br><sup>aka `洗牌`</sup> | `shuffle(items)` | Shuffle list randomly |
| `uuid_from_string` <br><sup>aka `解析UUID`</sup> | `uuid_from_string(s)` | Parse UUID from string |
| `uuid_generate` <br><sup>aka `生成UUID`</sup> | `uuid_generate()` | Generate UUID |
| `uuid_nil` <br><sup>aka `空UUID`</sup> | `uuid_nil()` | Return nil UUID |

## Built-in Functions (data)

| Function | Signature | Description |
|----------|-----------|-------------|
| `csv_load` <br><sup>aka `csv加载`</sup> | `csv_load(path, delimiter?)` | Load CSV from file |
| `csv_parse` <br><sup>aka `csv解析`</sup> | `csv_parse(text, delimiter?)` | Parse CSV |
| `csv_save` <br><sup>aka `csv保存`</sup> | `csv_save(path, rows, delimiter?)` | Save CSV to file |
| `csv_stringify` <br><sup>aka `csv序列化`</sup> | `csv_stringify(rows, delimiter?)` | Stringify to CSV |
| `html_links` <br><sup>aka `html链接`</sup> | `html_links(html)` | Extract HTML links |
| `html_parse` <br><sup>aka `html解析`</sup> | `html_parse(text)` | Parse HTML |
| `html_select` <br><sup>aka `html选择`</sup> | `html_select(html, selector)` | CSS select elements |
| `html_text` <br><sup>aka `html文本`</sup> | `html_text(html)` | Extract HTML text |
| `json_load` <br><sup>aka `json加载`</sup> | `json_load(path)` | Load JSON from file |
| `json_parse` <br><sup>aka `json解析`</sup> | `json_parse(text)` | Parse JSON |
| `json_parse_lenient` | `json_parse_lenient(text)` | Parse JSON with markdown fence stripping |
| `json_save` <br><sup>aka `json保存`</sup> | `json_save(path, value, indent?)` | Save JSON to file |
| `json_stringify` <br><sup>aka `json序列化`</sup> | `json_stringify(value, indent?)` | Stringify to JSON |
| `markdown_extract_headings` <br><sup>aka `md提取标题`</sup> | `markdown_extract_headings(text)` | Extract Markdown headings |
| `markdown_parse` <br><sup>aka `md解析`</sup> | `markdown_parse(text)` | Parse Markdown to blocks |
| `markdown_to_html` <br><sup>aka `md转html`</sup> | `markdown_to_html(text)` | Convert Markdown to HTML |
| `toml_load` <br><sup>aka `toml加载`</sup> | `toml_load(path)` | Load TOML from file |
| `toml_parse` <br><sup>aka `toml解析`</sup> | `toml_parse(text)` | Parse TOML |
| `toml_save` <br><sup>aka `toml保存`</sup> | `toml_save(path, value)` | Save TOML to file |
| `toml_stringify` <br><sup>aka `toml序列化`</sup> | `toml_stringify(value)` | Stringify to TOML |
| `xml_load` <br><sup>aka `xml加载`</sup> | `xml_load(path)` | Load XML from file |
| `xml_parse` <br><sup>aka `xml解析`</sup> | `xml_parse(text)` | Parse XML |
| `xml_save` <br><sup>aka `xml保存`</sup> | `xml_save(path, value, root?)` | Save XML to file |
| `xml_stringify` <br><sup>aka `xml序列化`</sup> | `xml_stringify(value, root?)` | Stringify to XML |
| `yaml_load` <br><sup>aka `yaml加载`</sup> | `yaml_load(path)` | Load YAML from file |
| `yaml_parse` <br><sup>aka `yaml解析`</sup> | `yaml_parse(text)` | Parse YAML |
| `yaml_save` <br><sup>aka `yaml保存`</sup> | `yaml_save(path, value)` | Save YAML to file |
| `yaml_stringify` <br><sup>aka `yaml序列化`</sup> | `yaml_stringify(value)` | Stringify to YAML |

## Built-in Functions (debug)

| Function | Signature | Description |
|----------|-----------|-------------|
| `coverage_off` | `coverage_off()` | Disable coverage tracking |
| `coverage_on` | `coverage_on()` | Enable coverage tracking |
| `coverage_report` | `coverage_report(format?)` | Generate coverage report |
| `coverage_summary` | `coverage_summary()` | Get coverage summary |
| `debug` <br><sup>aka `调试`</sup> | `debug(message, data?)` | Output structured debug info |
| `error_category` | `error_category(err)` | Extract diagnostic category from error dict |
| `error_data_flow` | `error_data_flow(err)` | Extract data flow from error dict |
| `error_suggestion` | `error_suggestion(err)` | Extract suggestion from error dict |
| `get_call_stack` | `get_call_stack()` | Get current call stack |
| `get_data_lineage` | `get_data_lineage()` | Get the complete data lineage graph |
| `get_last_error` | `get_last_error()` | Get last error snapshot with context |
| `get_llm_log` | `get_llm_log(n?)` | Get recent LLM call audit log |
| `get_trace` <br><sup>aka `获取跟踪`</sup> | `get_trace(n?)` | Get recent execution trace |
| `last_error_detail` | `last_error_detail()` | Get detailed error with diagnostic category and suggestion |
| `record_data_flow` | `record_data_flow(producer_uuid, consumer_uuid, flow_type, metadata?)` | Manually record a data flow event |
| `record_session` | `record_session(cassette_path)` | Start recording LLM interactions to cassette |
| `replay_session` | `replay_session(cassette_path)` | Replay LLM interactions from cassette |
| `stop_recording` | `stop_recording()` | Stop recording LLM interactions |
| `trace_off` <br><sup>aka `关闭跟踪`</sup> | `trace_off()` | Disable execution tracing |
| `trace_on` <br><sup>aka `开启跟踪`</sup> | `trace_on()` | Enable execution tracing |
| `trace_value_consumers` | `trace_value_consumers(message_uuid)` | Trace the consumers of data produced by a message |
| `trace_value_origin` | `trace_value_origin(message_uuid)` | Trace the origin of data consumed by a message |
| `validate_output` | `validate_output(output, contract)` | Validate output against contract (json/text/schema) |

## Built-in Functions (file)

| Function | Signature | Description |
|----------|-----------|-------------|
| `copy_file` <br><sup>aka `复制文件`</sup> | `copy_file(src, dst)` | Copy file |
| `delete_dir` <br><sup>aka `删除目录`</sup> | `delete_dir(path, recursive?)` | Delete directory |
| `delete_file` <br><sup>aka `删除文件`</sup> | `delete_file(path)` | Delete file |
| `file_modified` <br><sup>aka `文件修改时间`</sup> | `file_modified(path)` | File modification time |
| `file_size` <br><sup>aka `文件大小`</sup> | `file_size(path)` | File size in bytes |
| `glob_files` <br><sup>aka `查找文件`</sup> | `glob_files(path, pattern?)` | Recursively find files matching glob pattern |
| `grep_files` <br><sup>aka `搜索内容`</sup> | `grep_files(path, pattern, regex?, case_sensitive?, max_results?)` | Search file contents for a pattern |
| `list_dir` <br><sup>aka `列出目录`</sup> | `list_dir(path, pattern?)` | List directory |
| `move_file` <br><sup>aka `移动文件`</sup> | `move_file(src, dst)` | Move file |
| `temp_dir` <br><sup>aka `临时目录`</sup> | `temp_dir(suffix?, prefix?, dir?)` | Create temp directory |
| `temp_file` <br><sup>aka `临时文件`</sup> | `temp_file(suffix?, prefix?, dir?)` | Create temp file |
| `walk_dir` <br><sup>aka `遍历目录`</sup> | `walk_dir(path)` | Walk directory tree |

## Built-in Functions (io)

| Function | Signature | Description |
|----------|-----------|-------------|
| `append_file` <br><sup>aka `追加文件`</sup> | `append_file(path, content)` | Append to file |
| `mkdir` <br><sup>aka `创建目录`</sup> | `mkdir(path)` | Create directory |
| `mkdir_p` <br><sup>aka `递归创建目录`</sup> | `mkdir_p(path)` | Create directory tree |
| `progress_bar` <br><sup>aka `进度条`</sup> | `progress_bar(current, total, width?)` | Display progress bar |
| `stream_clear` <br><sup>aka `流式清除`</sup> | `stream_clear()` | Clear current line |
| `stream_cursor_down` <br><sup>aka `光标下移`</sup> | `stream_cursor_down(n?)` | Move cursor down |
| `stream_cursor_up` <br><sup>aka `光标上移`</sup> | `stream_cursor_up(n?)` | Move cursor up |
| `stream_print` <br><sup>aka `流式打印`</sup> | `stream_print(text)` | Print without newline |
| `write_file` <br><sup>aka `写文件`</sup> | `write_file(path, content)` | Write to file |

## Built-in Functions (llm)

| Function | Signature | Description |
|----------|-----------|-------------|
| `cancel_all_llm_calls` <br><sup>aka `取消所有大模型调用`</sup> | `cancel_all_llm_calls()` | Cancel all active streaming LLM calls |
| `cancel_llm_call` <br><sup>aka `取消大模型调用`</sup> | `cancel_llm_call(call_id)` | Cancel an in-flight streaming LLM call |
| `current_llm_call_id` <br><sup>aka `当前大模型调用id`</sup> | `current_llm_call_id()` | Get the current active streaming call ID |
| `get_description` <br><sup>aka `获取描述`</sup> | `get_description()` | Get agent description (read-only) |
| `get_max_tokens` <br><sup>aka `获取最大tokens`</sup> | `get_max_tokens()` | Get effective max-tokens |
| `get_max_turns` <br><sup>aka `获取最大轮次`</sup> | `get_max_turns()` | Get effective max-turns |
| `get_model` <br><sup>aka `获取模型`</sup> | `get_model()` | Get current model (read-only) |
| `get_provider` <br><sup>aka `获取提供商`</sup> | `get_provider()` | Get current provider (read-only) |
| `get_reasoning_effort` <br><sup>aka `获取推理强度`</sup> | `get_reasoning_effort()` | Get effective reasoning-effort |
| `get_temperature` <br><sup>aka `获取温度`</sup> | `get_temperature()` | Get effective temperature |
| `get_thinking_mode` <br><sup>aka `获取思考模式`</sup> | `get_thinking_mode()` | Get effective thinking-mode |
| `set_max_tokens` <br><sup>aka `设置最大tokens`</sup> | `set_max_tokens(4000)` | Set max output tokens |
| `set_max_turns` <br><sup>aka `设置最大轮次`</sup> | `set_max_turns(5)` | Set max tool-calling turns |
| `set_reasoning_effort` <br><sup>aka `设置推理强度`</sup> | `set_reasoning_effort("high")` | Set reasoning effort level |
| `set_temperature` <br><sup>aka `设置温度`</sup> | `set_temperature(0.7)` | Set temperature for subsequent llm act calls |
| `set_thinking_mode` <br><sup>aka `设置思考模式`</sup> | `set_thinking_mode(true)` | Enable/disable thinking mode |

## Built-in Functions (math)

| Function | Signature | Description |
|----------|-----------|-------------|
| `acos` <br><sup>aka `反余弦`</sup> | `acos(x)` | Arc cosine |
| `asin` <br><sup>aka `反正弦`</sup> | `asin(x)` | Arc sine |
| `atan` <br><sup>aka `反正切`</sup> | `atan(x)` | Arc tangent |
| `atan2` <br><sup>aka `反正切二`</sup> | `atan2(y, x)` | Arc tangent of y/x |
| `bit_and` | `bit_and(a, b)` | Bitwise AND |
| `bit_not` | `bit_not(a)` | Bitwise NOT |
| `bit_or` | `bit_or(a, b)` | Bitwise OR |
| `bit_shift_left` | `bit_shift_left(a, n)` | Bitwise left shift |
| `bit_shift_right` | `bit_shift_right(a, n)` | Bitwise right shift |
| `bit_xor` | `bit_xor(a, b)` | Bitwise XOR |
| `ceil` <br><sup>aka `向上取整`</sup> | `ceil(value)` | Ceiling value |
| `correlation` <br><sup>aka `相关系数`</sup> | `correlation(x, y)` | Pearson correlation |
| `cos` <br><sup>aka `余弦`</sup> | `cos(radians)` | Cosine |
| `exp` <br><sup>aka `指数`</sup> | `exp(x)` | Exponential (e^x) |
| `floor` <br><sup>aka `向下取整`</sup> | `floor(value)` | Floor value |
| `log` <br><sup>aka `对数`</sup> | `log(x, base?)` | Logarithm |
| `log10` <br><sup>aka `对数十`</sup> | `log10(x)` | Base-10 logarithm |
| `log2` <br><sup>aka `对数二`</sup> | `log2(x)` | Base-2 logarithm |
| `mean` <br><sup>aka `平均值`</sup> | `mean(numbers)` | Arithmetic mean |
| `median` <br><sup>aka `中位数`</sup> | `median(numbers)` | Median value |
| `mode` <br><sup>aka `众数`</sup> | `mode(numbers)` | Most frequent values |
| `percentile` <br><sup>aka `百分位`</sup> | `percentile(numbers, p)` | Percentile value |
| `pow` <br><sup>aka `幂`</sup> | `pow(base, exponent)` | Power |
| `product` <br><sup>aka `求积`</sup> | `product(numbers)` | Product of numbers |
| `round` <br><sup>aka `四舍五入`</sup> | `round(value, ndigits?)` | Round number |
| `sin` <br><sup>aka `正弦`</sup> | `sin(radians)` | Sine |
| `sqrt` <br><sup>aka `平方根`</sup> | `sqrt(value)` | Square root |
| `stats_max` <br><sup>aka `统计最大`</sup> | `stats_max(numbers)` | Maximum value (stats) |
| `stats_min` <br><sup>aka `统计最小`</sup> | `stats_min(numbers)` | Minimum value (stats) |
| `stddev` <br><sup>aka `标准差`</sup> | `stddev(numbers, population?)` | Standard deviation |
| `sum` <br><sup>aka `求和`</sup> | `sum(numbers)` | Sum of numbers |
| `tan` <br><sup>aka `正切`</sup> | `tan(radians)` | Tangent |
| `variance` <br><sup>aka `方差`</sup> | `variance(numbers, population?)` | Variance |

## Built-in Functions (media)

| Function | Signature | Description |
|----------|-----------|-------------|
| `is_audio` <br><sup>aka `是音频`</sup> | `is_audio(value)` | Check if MediaPart is audio |
| `is_image` <br><sup>aka `是图片`</sup> | `is_image(value)` | Check if MediaPart is an image |
| `is_media` <br><sup>aka `是媒体`</sup> | `is_media(value)` | Check if value is MediaPart |
| `is_video` <br><sup>aka `是视频`</sup> | `is_video(value)` | Check if MediaPart is a video |
| `media` <br><sup>aka `媒体`</sup> | `media(source|MediaPart, ...) | media(source, type?)` | Create media from file/URL/MediaPart (passthrough + multi-arg) |
| `media_base64` <br><sup>aka `媒体base64`</sup> | `media_base64(data, mime, type?)` | Create media from base64 data |
| `media_to_base64` <br><sup>aka `媒体转base64`</sup> | `media_to_base64(part)` | Convert MediaPart content to base64 string |
| `media_type` <br><sup>aka `媒体类型`</sup> | `media_type(value)` | Get media type |
| `save_media` <br><sup>aka `保存媒体`</sup> | `save_media(part, path?)` | Save MediaPart to file |
| `to_claude_parts` <br><sup>aka `转Claude格式`</sup> | `to_claude_parts(parts)` | Convert MediaParts to Claude content format |
| `to_gemini_parts` <br><sup>aka `转Gemini格式`</sup> | `to_gemini_parts(parts)` | Convert MediaParts to Gemini content format |
| `to_openai_parts` <br><sup>aka `转OpenAI格式`</sup> | `to_openai_parts(parts)` | Convert MediaParts to OpenAI content format |

## Built-in Functions (network)

| Function | Signature | Description |
|----------|-----------|-------------|
| `http_delete` <br><sup>aka `http删除`</sup> | `http_delete(url, headers?)` | HTTP DELETE request |
| `http_download` <br><sup>aka `http下载`</sup> | `http_download(url, path)` | Download file from URL |
| `http_get` <br><sup>aka `http获取`</sup> | `http_get(url, headers?)` | HTTP GET request |
| `http_post` <br><sup>aka `http发布`</sup> | `http_post(url, data?, headers?)` | HTTP POST request |
| `http_put` <br><sup>aka `http提交`</sup> | `http_put(url, data?, headers?)` | HTTP PUT request |
| `url_build` <br><sup>aka `链接构建`</sup> | `url_build(scheme, host, path?, query?)` | Build URL |
| `url_decode` <br><sup>aka `链接解码`</sup> | `url_decode(s)` | URL decode |
| `url_encode` <br><sup>aka `链接编码`</sup> | `url_encode(s)` | URL encode |
| `url_parse` <br><sup>aka `链接解析`</sup> | `url_parse(url)` | Parse URL |

## Built-in Functions (path)

| Function | Signature | Description |
|----------|-----------|-------------|
| `path_basename` <br><sup>aka `路径基础名`</sup> | `path_basename(path)` | Base name |
| `path_dirname` <br><sup>aka `路径目录名`</sup> | `path_dirname(path)` | Directory name |
| `path_exists` <br><sup>aka `路径存在`</sup> | `path_exists(path)` | Check if path exists |
| `path_is_dir` <br><sup>aka `是否目录`</sup> | `path_is_dir(path)` | Check if path is directory |
| `path_is_file` <br><sup>aka `是否文件`</sup> | `path_is_file(path)` | Check if path is file |
| `path_join` <br><sup>aka `路径拼接`</sup> | `path_join(*parts)` | Join path components |

## Built-in Functions (quality)

| Function | Signature | Description |
|----------|-----------|-------------|
| `analyze_code` <br><sup>aka `分析代码`</sup> | `analyze_code(source, filename?)` | Analyze code metrics |
| `check_security` <br><sup>aka `安全检查`</sup> | `check_security(source)` | Check security issues |
| `quality_report` <br><sup>aka `质量报告`</sup> | `quality_report(source, filename?)` | Generate quality report |
| `quality_score` <br><sup>aka `质量评分`</sup> | `quality_score(source, file_path?)` | Calculate quality score |

## Built-in Functions (string)

| Function | Signature | Description |
|----------|-----------|-------------|
| `base64_decode` <br><sup>aka `base64解码`</sup> | `base64_decode(s)` | Base64 decode |
| `base64_encode` <br><sup>aka `base64编码`</sup> | `base64_encode(s)` | Base64 encode |
| `center` <br><sup>aka `居中`</sup> | `center(s, width, char?)` | Center string |
| `chr` <br><sup>aka `字符`</sup> | `chr(code)` | Unicode code point to character |
| `contains` <br><sup>aka `包含`</sup> | `contains(s, sub)` | Check if contains substring |
| `count` <br><sup>aka `计数`</sup> | `count(s, sub)` | Count substring |
| `endswith` <br><sup>aka `结尾是`</sup> | `endswith(s, suffix)` | Check suffix |
| `extract_emails` <br><sup>aka `提取邮箱`</sup> | `extract_emails(text)` | Extract emails |
| `extract_urls` <br><sup>aka `提取链接`</sup> | `extract_urls(text)` | Extract URLs |
| `find` <br><sup>aka `查找`</sup> | `find(s, sub)` | Find substring index |
| `find_from` <br><sup>aka `从位置查找`</sup> | `find_from(s, sub, start)` | Find substring from position |
| `format_float` <br><sup>aka `格式化浮点`</sup> | `format_float(value, decimals)` | Format float with decimals |
| `html_escape` <br><sup>aka `html转义`</sup> | `html_escape(s)` | HTML escape |
| `html_unescape` <br><sup>aka `html反转义`</sup> | `html_unescape(s)` | HTML unescape |
| `index` <br><sup>aka `查找索引`</sup> | `index(s, sub)` | Find substring index |
| `interpolate` <br><sup>aka `插值`</sup> | `interpolate(template, vars)` | Template string interpolation |
| `join` <br><sup>aka `连接`</sup> | `join(items, sep)` | Join strings |
| `levenshtein` <br><sup>aka `编辑距离`</sup> | `levenshtein(s1, s2)` | Edit distance |
| `lower` <br><sup>aka `转小写`</sup> | `lower(s)` | Lowercase string |
| `normalize_whitespace` <br><sup>aka `规范化空白`</sup> | `normalize_whitespace(text)` | Normalize whitespace |
| `ord` <br><sup>aka `码点`</sup> | `ord(char)` | Character to Unicode code point |
| `pad_left` <br><sup>aka `左填充`</sup> | `pad_left(s, width, char?)` | Pad left |
| `pad_right` <br><sup>aka `右填充`</sup> | `pad_right(s, width, char?)` | Pad right |
| `regex_findall` <br><sup>aka `正则查找全部`</sup> | `regex_findall(pattern, s)` | Regex find all |
| `regex_match` <br><sup>aka `正则匹配`</sup> | `regex_match(pattern, s)` | Regex match at start |
| `regex_replace` <br><sup>aka `正则替换`</sup> | `regex_replace(pattern, s, replacement)` | Regex replace |
| `regex_search` <br><sup>aka `正则搜索`</sup> | `regex_search(pattern, s)` | Regex search anywhere |
| `regex_split` <br><sup>aka `正则分割`</sup> | `regex_split(pattern, s)` | Regex split |
| `regex_test` <br><sup>aka `正则测试`</sup> | `regex_test(pattern, s)` | Regex test returns bool |
| `remove_punctuation` <br><sup>aka `去标点`</sup> | `remove_punctuation(text)` | Remove punctuation |
| `repeat` <br><sup>aka `重复`</sup> | `repeat(s, n)` | Repeat string |
| `replace` <br><sup>aka `替换`</sup> | `replace(s, old, new)` | Replace substring |
| `reverse` <br><sup>aka `反转`</sup> | `reverse(s)` | Reverse string |
| `similarity` <br><sup>aka `相似度`</sup> | `similarity(s1, s2)` | String similarity |
| `split` <br><sup>aka `分割`</sup> | `split(s, sep?)` | Split string |
| `startswith` <br><sup>aka `开头是`</sup> | `startswith(s, prefix)` | Check prefix |
| `strip` <br><sup>aka `去除空白`</sup> | `strip(s)` | Trim whitespace |
| `substring` <br><sup>aka `子串`</sup> | `substring(s, start, end?)` | Extract substring |
| `tokenize` <br><sup>aka `分词`</sup> | `tokenize(text)` | Tokenize text |
| `trim_prefix` <br><sup>aka `去前缀`</sup> | `trim_prefix(s, prefix)` | Remove prefix |
| `trim_suffix` <br><sup>aka `去后缀`</sup> | `trim_suffix(s, suffix)` | Remove suffix |
| `upper` <br><sup>aka `转大写`</sup> | `upper(s)` | Uppercase string |
| `word_count` <br><sup>aka `词频统计`</sup> | `word_count(text)` | Count word frequencies |

## Built-in Functions (system)

| Function | Signature | Description |
|----------|-----------|-------------|
| `cpu_count` <br><sup>aka `CPU核心数`</sup> | `cpu_count()` | Get CPU core count |
| `env_delete` <br><sup>aka `环境变量删除`</sup> | `env_delete(key)` | Delete environment variable |
| `env_get` <br><sup>aka `环境变量获取`</sup> | `env_get(key, default?)` | Get environment variable |
| `env_list` <br><sup>aka `环境变量列表`</sup> | `env_list()` | List all environment variables |
| `env_set` <br><sup>aka `环境变量设置`</sup> | `env_set(key, value)` | Set environment variable |
| `exec` <br><sup>aka `执行`</sup> | `exec(command, shell?, timeout?)` | Execute command |
| `exec_async` <br><sup>aka `异步执行`</sup> | `exec_async(command, shell?)` | Execute command asynchronously |
| `exit` <br><sup>aka `退出`</sup> | `exit(code?)` | Exit program |
| `get_cli_args` <br><sup>aka `命令行参数`</sup> | `get_cli_args()` | Get CLI arguments |
| `hostname` <br><sup>aka `主机名`</sup> | `hostname()` | Get hostname |
| `kill` <br><sup>aka `终止进程`</sup> | `kill(pid, signal?)` | Send signal to process |
| `log_critical` <br><sup>aka `日志严重`</sup> | `log_critical(message)` | Log critical message |
| `log_debug` <br><sup>aka `日志调试`</sup> | `log_debug(message)` | Log debug message |
| `log_error` <br><sup>aka `日志错误`</sup> | `log_error(message)` | Log error message |
| `log_info` <br><sup>aka `日志信息`</sup> | `log_info(message)` | Log info message |
| `log_set_level` <br><sup>aka `日志设置级别`</sup> | `log_set_level(level)` | Set logging level |
| `log_to_file` <br><sup>aka `日志写入文件`</sup> | `log_to_file(path)` | Set log output to file |
| `log_warn` <br><sup>aka `日志警告`</sup> | `log_warn(message)` | Log warning message |
| `memory_info` <br><sup>aka `内存信息`</sup> | `memory_info()` | Get memory information |
| `parse_cli_args` <br><sup>aka `解析命令行参数`</sup> | `parse_cli_args(spec?)` | Parse CLI arguments |
| `pid` <br><sup>aka `进程ID`</sup> | `pid()` | Get current process ID |
| `platform` <br><sup>aka `操作系统`</sup> | `platform()` | Get OS name |
| `platform_version` <br><sup>aka `系统版本`</sup> | `platform_version()` | Get detailed platform info |
| `python_version` <br><sup>aka `Python版本`</sup> | `python_version()` | Get Python version |

## Built-in Functions (test)

| Function | Signature | Description |
|----------|-----------|-------------|
| `after_all` <br><sup>aka `后置所有`</sup> | `after_all(fn)` | Register after-all hook |
| `after_each` <br><sup>aka `后置每个`</sup> | `after_each(fn)` | Register after-each hook |
| `assert_contains` <br><sup>aka `断言包含`</sup> | `assert_contains(container, item, message?)` | Assert container contains item |
| `assert_equal` <br><sup>aka `断言相等`</sup> | `assert_equal(actual, expected, message?)` | Assert equality |
| `assert_not_equal` <br><sup>aka `断言不等`</sup> | `assert_not_equal(actual, expected, message?)` | Assert inequality |
| `assert_throws` <br><sup>aka `断言抛出`</sup> | `assert_throws(fn, error_type?)` | Assert function throws |
| `assert_true` <br><sup>aka `断言为真`</sup> | `assert_true(condition, message?)` | Assert condition is truthy |
| `before_all` <br><sup>aka `前置所有`</sup> | `before_all(fn)` | Register before-all hook |
| `before_each` <br><sup>aka `前置每个`</sup> | `before_each(fn)` | Register before-each hook |
| `describe` <br><sup>aka `描述`</sup> | `describe(name, fn)` | Define a test suite |
| `expect` <br><sup>aka `期望`</sup> | `expect(value)` | Create chainable expectation |
| `fail` <br><sup>aka `失败`</sup> | `fail(message?)` | Explicitly fail a test |
| `it` <br><sup>aka `它`</sup> | `it(name, fn)` | Define a test case |
| `it_skip` <br><sup>aka `跳过它`</sup> | `it_skip(name, fn?)` | Define a skipped test case |
| `run_tests` <br><sup>aka `运行测试`</sup> | `run_tests(only?, suite?, filter?)` | Execute all tests and print report |
| `run_tests_json` <br><sup>aka `运行测试json`</sup> | `run_tests_json(only?, suite?, filter?)` | Execute tests and return JSON |
| `set_test_timeout` <br><sup>aka `设置测试超时`</sup> | `set_test_timeout(seconds)` | Set per-test timeout |
| `test_case` <br><sup>aka `测试用例`</sup> | `test_case(name, fn)` | Register a test case |
| `test_case_skip` <br><sup>aka `跳过测试用例`</sup> | `test_case_skip(name, fn?)` | Register a skipped test |
| `test_count` <br><sup>aka `测试计数`</sup> | `test_count()` | Count registered tests |
| `test_end_suite` <br><sup>aka `结束测试套件`</sup> | `test_end_suite()` | End current test suite |
| `test_reset` <br><sup>aka `重置测试`</sup> | `test_reset()` | Clear all registered tests |
| `test_suite` <br><sup>aka `测试套件`</sup> | `test_suite(name)` | Start a test suite |

## Built-in Functions (time)

| Function | Signature | Description |
|----------|-----------|-------------|
| `date` <br><sup>aka `日期`</sup> | `date(year?, month?, day?)` | Create/get date |
| `date_add` <br><sup>aka `日期相加`</sup> | `date_add(date_str, days?, hours?, minutes?, seconds?)` | Add to date |
| `date_day` <br><sup>aka `日`</sup> | `date_day(date_str)` | Extract day |
| `date_diff` <br><sup>aka `日期相减`</sup> | `date_diff(date1, date2, unit?)` | Date difference |
| `date_format` <br><sup>aka `日期格式化`</sup> | `date_format(date_str, format_str)` | Format date |
| `date_month` <br><sup>aka `月`</sup> | `date_month(date_str)` | Extract month |
| `date_parse` <br><sup>aka `日期解析`</sup> | `date_parse(date_str, format_str)` | Parse date |
| `date_weekday` <br><sup>aka `星期`</sup> | `date_weekday(date_str)` | Day of week |
| `date_year` <br><sup>aka `年`</sup> | `date_year(date_str)` | Extract year |
| `datetime` <br><sup>aka `日期时间`</sup> | `datetime(year?, month?, day?, hour?, minute?, second?)` | Create/get datetime |
| `fromtimestamp` | `fromtimestamp(timestamp)` | Unix timestamp to datetime |
| `now` <br><sup>aka `当前时间戳`</sup> | `now()` | Current datetime |
| `sleep` <br><sup>aka `休眠`</sup> | `sleep(seconds)` | Pause execution |
| `stopwatch_elapsed` <br><sup>aka `经过时间`</sup> | `stopwatch_elapsed(start_time)` | Get elapsed time |
| `stopwatch_lap` <br><sup>aka `计时分段`</sup> | `stopwatch_lap(start_time)` | Get lap time |
| `stopwatch_start` <br><sup>aka `开始计时`</sup> | `stopwatch_start()` | Start stopwatch |
| `time` <br><sup>aka `当前时间`</sup> | `time()` | Unix timestamp |

## Built-in Functions (tools)

| Function | Signature | Description |
|----------|-----------|-------------|
| `calculate` <br><sup>aka `计算表达式`</sup> | `calculate(expression)` | Evaluate math expression |
| `list_skill_references` <br><sup>aka `列出技能引用`</sup> | `list_skill_references(name)` | List reference documents for a skill |
| `load_skill` <br><sup>aka `加载技能`</sup> | `load_skill(name, include_references?)` | Load a skill by name |
| `patch_file` <br><sup>aka `修补文件`</sup> | `patch_file(path, old_string, new_string, replace_all?)` | Patch a file |
| `shell_exec` <br><sup>aka `执行命令`</sup> | `shell_exec(command, timeout?, shell?)` | Execute shell command |
| `web_fetch` <br><sup>aka `网页获取`</sup> | `web_fetch(url)` | Fetch web page content |
| `web_search` <br><sup>aka `网页搜索`</sup> | `web_search(query, limit?)` | Search the web |

## Built-in Functions (transcript)

| Function | Signature | Description |
|----------|-----------|-------------|
| `cleanup_sessions` <br><sup>aka `清理会话`</sup> | `cleanup_sessions(keep_count?, older_than_days?, cascade?)` | Clean up old sessions (v1.23.7: cascade=true deletes spawned) |
| `delete_current_session` <br><sup>aka `删除当前会话`</sup> | `delete_current_session(confirm?, cascade?)` | Permanently delete current session (v1.23.7: cascade=true deletes spawned) |
| `delete_session` <br><sup>aka `删除会话`</sup> | `delete_session(session_id, cascade?)` | Permanently delete a session (v1.23.7: cascade=true deletes spawned sessions) |
| `export_transcript` <br><sup>aka `导出会话`</sup> | `export_transcript(output_path, format?, session_id?, include_spawned?)` | Export transcript to file (v1.23.7: include_spawned) |
| `get_compression_audit` <br><sup>aka `压缩审计`</sup> | `get_compression_audit()` | Get compression event history |
| `get_invocation` <br><sup>aka `获取调用`</sup> | `get_invocation(invocation_id, session_id?)` | Get metadata for a specific invocation |
| `get_invocation_tree` <br><sup>aka `获取调用树`</sup> | `get_invocation_tree(session_id?)` | Get the full invocation tree for a session (agent calls) |
| `get_session_dir` <br><sup>aka `获取会话目录`</sup> | `get_session_dir()` | Get resolved transcript session directory |
| `get_session_id` <br><sup>aka `获取会话id`</sup> | `get_session_id()` | Get current transcript session ID |
| `get_session_meta` <br><sup>aka `获取会话元数据`</sup> | `get_session_meta(session_id?)` | Get session metadata (argv, timestamp, version) |
| `get_spawn_tree` <br><sup>aka `获取会话树`</sup> | `get_spawn_tree(session_id?)` | Get the full session spawn tree (nested spawns) |
| `get_spawned_sessions` <br><sup>aka `获取子会话`</sup> | `get_spawned_sessions(session_id?)` | Get sessions spawned by the given session |
| `invocation_path` <br><sup>aka `调用路径`</sup> | `invocation_path(invocation_id, session_id?)` | Get human-readable path string for an invocation |
| `list_invocations` <br><sup>aka `列出调用`</sup> | `list_invocations(session_id?, agent?, limit?, offset?)` | List invocations with optional filtering |
| `list_sessions` <br><sup>aka `列出会话`</sup> | `list_sessions()` | List all transcript sessions |
| `query_transcript` | `query_transcript(session_id?, role?, agent?, invocation_id?, since?, until?, content_regex?, message_type?, limit?, offset?)` | Query transcript with filtering and pagination (v1.40+) |
| `release_session_lock` <br><sup>aka `释放会话锁`</sup> | `release_session_lock(session_id)` | Release cross-process session lock (called on actor exit to prevent stale locks) |
| `replay_full_session` <br><sup>aka `回放完整会话`</sup> | `replay_full_session(session_id?)` | Replay transcript from session and all spawned sessions (v1.23.7+) |
| `replay_transcript` <br><sup>aka `回放会话`</sup> | `replay_transcript(session_id?, include_compressed?, agent?, invocation_id?, last_only?, include_subtree?)` | Replay transcript messages with optional invocation filtering |
| `resume_session` <br><sup>aka `恢复会话`</sup> | `resume_session(session_id)` | Resume a previous transcript session |
| `search_transcript` <br><sup>aka `搜索会话`</sup> | `search_transcript(query, session_id?, scope?, role?, regex?, limit?, include_spawned?)` | Search transcript messages by content (v1.23.7: include_spawned) |
| `set_session_dir` <br><sup>aka `设置会话目录`</sup> | `set_session_dir(path)` | Set transcript session directory at runtime |
