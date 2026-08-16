"""StreamRegistry 测试

覆盖 StreamRegistry 类的 register/unregister/is_processing 方法,
以及线程安全性和模块级单例。

运行: cd webui/backend && pytest tests/test_stream_registry.py -v
"""
import pytest
import threading
from app.services.stream_registry import StreamRegistry, stream_registry


@pytest.fixture
def registry():
    """Fresh StreamRegistry instance."""
    return StreamRegistry()


class TestStreamRegistry:
    def test_initial_state_not_processing(self, registry):
        """新 registry 没有活跃的 session"""
        assert registry.is_processing() is False

    def test_register_makes_processing_true(self, registry):
        """register 后 is_processing 返回 True"""
        registry.register("session-1")
        assert registry.is_processing() is True

    def test_unregister_makes_processing_false(self, registry):
        """所有 session unregister 后 is_processing 返回 False"""
        registry.register("session-1")
        registry.unregister("session-1")
        assert registry.is_processing() is False

    def test_multiple_sessions(self, registry):
        """多个 session 同时注册"""
        registry.register("session-1")
        registry.register("session-2")
        assert registry.is_processing() is True

        registry.unregister("session-1")
        # session-2 still active
        assert registry.is_processing() is True

        registry.unregister("session-2")
        assert registry.is_processing() is False

    def test_unregister_nonexistent_is_noop(self, registry):
        """unregister 不存在的 session 不会报错 (discard)"""
        registry.unregister("nonexistent")
        assert registry.is_processing() is False

    def test_register_same_session_idempotent(self, registry):
        """重复 register 同一 session 不会重复计数"""
        registry.register("session-1")
        registry.register("session-1")
        registry.unregister("session-1")
        # Set semantics: one unregister removes it
        assert registry.is_processing() is False


class TestStreamRegistryThreadSafety:
    def test_concurrent_register_unregister(self, registry):
        """并发 register/unregister 不崩溃"""
        errors = []

        def worker(session_id):
            try:
                for _ in range(100):
                    registry.register(session_id)
                    registry.unregister(session_id)
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=worker, args=(f"s{i}",)) for i in range(10)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert errors == []
        assert registry.is_processing() is False

    def test_concurrent_is_processing_reads(self, registry):
        """并发读写 is_processing 不崩溃"""
        errors = []
        stop = threading.Event()

        def writer():
            i = 0
            while not stop.is_set():
                registry.register(f"s{i % 5}")
                registry.unregister(f"s{i % 5}")
                i += 1

        def reader():
            while not stop.is_set():
                try:
                    registry.is_processing()
                except Exception as e:
                    errors.append(e)

        threads = [threading.Thread(target=writer) for _ in range(3)]
        threads += [threading.Thread(target=reader) for _ in range(3)]
        for t in threads:
            t.start()

        import time
        time.sleep(0.2)
        stop.set()
        for t in threads:
            t.join()

        assert errors == []


class TestModuleSingleton:
    def test_singleton_exists(self):
        """模块级 stream_registry 单例存在"""
        assert stream_registry is not None
        assert isinstance(stream_registry, StreamRegistry)

    def test_singleton_is_stream_registry(self):
        """单例是 StreamRegistry 实例"""
        assert isinstance(stream_registry, StreamRegistry)
