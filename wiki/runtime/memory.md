# 内存系统

> 模块 M7 | `helen/runtime/memory.py` | 测试: `tests/runtime/test_memory.py`

---

## MemoryProvider ABC

```python
class MemoryProvider(ABC):
    @abstractmethod
    def get(self, key: str) -> str | None: ...
    @abstractmethod
    def set(self, key: str, value: str) -> None: ...
    @abstractmethod
    def delete(self, key: str) -> None: ...
    @abstractmethod
    def list_keys(self) -> list[str]: ...
```

---

## FileMemoryProvider

JSON 文件持久化存储：

```python
class FileMemoryProvider(MemoryProvider):
    def __init__(self, path: str):
        self._path = path
        self._data: dict[str, str] = self._load()

    def _load(self) -> dict:
        if os.path.exists(self._path):
            with open(self._path) as f:
                return json.load(f)
        return {}

    def _save(self):
        with open(self._path, 'w') as f:
            json.dump(self._data, f, indent=2)

    def get(self, key: str) -> str | None:
        return self._data.get(key)

    def set(self, key: str, value: str) -> None:
        self._data[key] = value
        self._save()

    def delete(self, key: str) -> None:
        if key in self._data:
            del self._data[key]
            self._save()

    def list_keys(self) -> list[str]:
        return list(self._data.keys())
```

---

## InMemoryProvider

纯内存实现，用于测试：

```python
class InMemoryProvider(MemoryProvider):
    def __init__(self):
        self._data: dict[str, str] = {}

    def get(self, key): return self._data.get(key)
    def set(self, key, value): self._data[key] = value
    def delete(self, key): self._data.pop(key, None)
    def list_keys(self): return list(self._data.keys())
```

---

## 注册与使用

```python
runtime = HelenHermesRuntime()
file_provider = FileMemoryProvider("memories/agent.json")
runtime.register_memory_provider("file", file_provider)

# 通过 Runtime 接口访问
runtime.set_memory("user_name", "Alice")
name = runtime.get_memory("user_name")  # "Alice"
```

---

## v1.10 shared let 内存可见性

### 概述

`shared let` 变量在内存系统中的处理方式与普通变量不同，它们需要跨 agent 可见且可修改。

### Environment 扩展

`Environment` 类扩展以支持 shared let：

```python
class Environment:
    def __init__(self, parent: Environment | None = None):
        self.values: dict[str, Any] = {}
        self.shared: dict[str, Any] = {}  # v1.10: shared let 存储
        self.constants: dict[str, Any] = {}  # const 存储
        self.parent = parent
    
    def define_shared(self, name: str, value: Any):
        """定义 shared let 变量"""
        self.shared[name] = value
    
    def lookup_shared(self, name: str) -> Any | None:
        """查找 shared let（逐层向上）"""
        if name in self.shared:
            return self.shared[name]
        if self.parent:
            return self.parent.lookup_shared(name)
        return None
    
    def assign_shared(self, name: str, value: Any) -> bool:
        """修改 shared let（必须在某层已定义）"""
        if name in self.shared:
            self.shared[name] = value
            return True
        if self.parent:
            return self.parent.assign_shared(name, value)
        return False
```

### Agent Main 环境创建

创建 agent main 环境时，导入 shared let：

```python
def create_agent_main_env(self, global_env: Environment) -> Environment:
    main_env = Environment()  # 无 parent，完全隔离
    
    # 导入 const（只读）
    for name, value in global_env.constants.items():
        main_env.constants[name] = value
    
    # 导入 shared let（可读写）
    for name, value in global_env.shared.items():
        main_env.shared[name] = value
    
    # 注意：不导入普通 let
    
    return main_env
```

### 内存模型

```
Global Environment
├── let moduleVar = "模块级"        # ❌ agent main 不可见
├── const MODULE_CONST = "常量"     # ✅ agent main 只读
└── shared let sharedVar = 0        # ✅ agent main 可读写

Agent Main Environment (isolated)
├── constants: {MODULE_CONST: "常量"}
└── shared: {sharedVar: 0}
```

### 持久化

`shared let` 可以选择持久化到内存系统：

```helen
shared let counter = 0

// 手动持久化到文件内存
fn save_counter() {
  memory.set("counter", str(counter))
}

fn load_counter() {
  let saved = memory.get("counter")
  if saved != null {
    counter = int(saved)
  }
}
```

### 线程安全

多线程访问 `shared let` 时需要锁：

```python
class SharedStateManager:
    def __init__(self):
        self._lock = threading.Lock()
        self._shared: dict[str, Any] = {}
    
    def get(self, name: str) -> Any:
        with self._lock:
            return self._shared.get(name)
    
    def set(self, name: str, value: Any):
        with self._lock:
            self._shared[name] = value
```

---

**最后更新**: 2026-07-01  
**版本**: v1.10
