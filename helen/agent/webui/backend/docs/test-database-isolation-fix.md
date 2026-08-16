# 测试数据库隔离修复报告

## 问题描述

每次重启 `helen agent` 后，Web UI 的所有会话数据丢失。

## 根本原因

测试文件直接使用了生产数据库（`helen.db`），在测试结束后通过 `Base.metadata.drop_all(bind=engine)` 删除所有表，导致生产数据被清空。

### 受影响的测试文件

- `test_api.py`
- `test_transcript_endpoints.py`
- `test_slash_command_persistence.py`
- `test_slash_command_websocket.py`
- `test_websocket_disconnect.py`

## 解决方案

### 1. 创建独立的测试数据库配置

在 `tests/conftest.py` 中添加：

```python
# 测试数据库路径（与生产数据库分离）
TEST_DB_PATH = Path(__file__).parent / "test.db"
TEST_DATABASE_URL = f"sqlite:///{TEST_DB_PATH}"

@pytest.fixture(scope="function")
def test_db():
    """创建测试数据库，测试完成后清理"""
    engine = create_engine(TEST_DATABASE_URL, connect_args={"check_same_thread": False})
    Base.metadata.create_all(bind=engine)
    TestSessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)
    
    yield TestSessionLocal
    
    # 清理：删除所有表
    Base.metadata.drop_all(bind=engine)
    # 删除测试数据库文件
    if TEST_DB_PATH.exists():
        TEST_DB_PATH.unlink()
```

### 2. 修改所有测试文件

**修改前**：
```python
@pytest.fixture
def client():
    Base.metadata.create_all(bind=engine)  # 使用生产数据库
    with TestClient(app) as c:
        yield c
    Base.metadata.drop_all(bind=engine)  # 删除生产数据库表！
```

**修改后**：
```python
@pytest.fixture
def client(test_db):
    # 覆盖 FastAPI 的 get_db 依赖
    def _get_test_db():
        try:
            db = test_db()
            yield db
        finally:
            db.close()
    
    app.dependency_overrides[get_db] = _get_test_db
    
    with TestClient(app) as c:
        yield c
    
    app.dependency_overrides.clear()
```

### 3. 修改数据库查询辅助函数

**修改前**：
```python
def _get_db_messages(session_id):
    db = SessionLocal()  # 使用生产数据库
    try:
        messages = db.query(Message).filter(...).all()
        return [m.to_dict() for m in messages]
    finally:
        db.close()
```

**修改后**：
```python
def _get_db_messages(db, session_id):  # 添加 db 参数
    messages = db.query(Message).filter(...).all()
    return [m.to_dict() for m in messages]
```

所有调用处相应修改：
```python
# 修改前
messages = _get_db_messages(sid)

# 修改后
db = test_db()
messages = _get_db_messages(db, sid)
db.close()
```

## 修改的文件列表

1. **tests/conftest.py** - 添加 `test_db` 和 `override_get_db` fixtures
2. **tests/test_api.py** - 使用 `test_db` fixture
3. **tests/test_transcript_endpoints.py** - 使用 `test_db` fixture
4. **tests/test_slash_command_persistence.py** - 使用 `test_db` 和 `db_session` fixtures
5. **tests/test_slash_command_websocket.py** - 使用 `test_db` fixture，修改 `_get_db_messages` 函数
6. **tests/test_websocket_disconnect.py** - 使用 `test_db` fixture，修改 `_get_db_messages` 函数

## 验证结果

### 测试执行

```bash
$ pytest tests/ -v
======================= 70 passed, 7 warnings in 39.99s ========================
```

所有 70 个测试通过，无失败。

### 数据库隔离验证

1. **测试数据库**：
   - 位置：`webui/backend/test.db`
   - 生命周期：每个测试函数创建，测试结束后删除
   - 不影响生产数据

2. **生产数据库**：
   - 位置：`webui/backend/helen.db`
   - 测试期间保持不变
   - 重启 Web UI 后数据保留

### 验证命令

```bash
# 检查生产数据库
python -c "from app.database import SessionLocal; from app.models.session import Session; \
db = SessionLocal(); sessions = db.query(Session).all(); \
print(f'生产数据库会话数: {len(sessions)}'); db.close()"

# 检查测试数据库（应该不存在）
ls -lh test.db  # 应该显示 "没有那个文件或目录"
```

## 技术细节

### FastAPI 依赖注入覆盖

使用 `app.dependency_overrides[get_db]` 临时替换 FastAPI 的数据库依赖：

```python
# 在 fixture 中
app.dependency_overrides[get_db] = _get_test_db

# 在 fixture 清理时
app.dependency_overrides.clear()
```

这确保所有使用 `Depends(get_db)` 的路由都使用测试数据库。

### 数据库连接管理

- **测试数据库**：每个测试函数独立的连接和会话
- **自动清理**：`finally` 块确保连接关闭
- **文件清理**：测试结束后删除 `test.db` 文件

## 后续建议

1. **定期备份**：虽然测试不再影响生产数据，但仍建议定期备份 `helen.db`
2. **监控磁盘空间**：测试数据库会自动清理，不会积累
3. **CI/CD 集成**：测试数据库隔离机制适合 CI/CD 环境

## 总结

通过引入独立的测试数据库和 FastAPI 依赖注入覆盖机制，完全隔离了测试环境和生产环境。测试期间的数据操作不再影响生产数据库，重启 Web UI 后会话数据得以保留。
