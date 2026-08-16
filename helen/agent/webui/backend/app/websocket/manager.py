"""WebSocket 连接管理器

v6.1:广播模式。单会话架构下所有连接共享同一工作目录,
所有连接收到相同消息(支持多标签页同步)。
"""
from fastapi import WebSocket
from typing import List


class WebSocketManager:
    def __init__(self):
        self.active_connections: List[WebSocket] = []

    async def connect(self, websocket: WebSocket):
        """建立 WebSocket 连接"""
        await websocket.accept()
        self.active_connections.append(websocket)

    def disconnect(self, websocket: WebSocket):
        """断开连接"""
        if websocket in self.active_connections:
            self.active_connections.remove(websocket)

    async def broadcast(self, message: dict):
        """广播消息到所有连接(多标签页同步)"""
        disconnected = []
        for connection in self.active_connections:
            try:
                await connection.send_json(message)
            except Exception as e:
                print(f"Error sending to websocket: {e}")
                disconnected.append(connection)

        # 清理断开的连接
        for conn in disconnected:
            self.disconnect(conn)

    async def close_all(self):
        """关闭所有连接"""
        for connection in list(self.active_connections):
            try:
                await connection.close()
            except Exception:
                pass
        self.active_connections.clear()
