"""Mock MCP server for testing.

Implements a simple MCP server that provides an "echo" tool.
Used for testing MCP client integration.
"""

import sys
import json


def main():
    """Run the mock MCP server.

    Reads JSON-RPC requests from stdin, writes responses to stdout.
    """
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
            method = request.get("method")
            request_id = request.get("id")
            params = request.get("params", {})

            if method == "initialize":
                # Initialize connection
                response = {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {"tools": {}},
                        "serverInfo": {
                            "name": "mock-mcp-server",
                            "version": "1.0.0",
                        },
                    },
                }

            elif method == "tools/list":
                # List available tools
                response = {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": {
                        "tools": [
                            {
                                "name": "echo",
                                "description": "Echo the input message",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "message": {
                                            "type": "string",
                                            "description": "Message to echo",
                                        },
                                    },
                                    "required": ["message"],
                                },
                            },
                            {
                                "name": "add",
                                "description": "Add two numbers",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "a": {"type": "integer", "description": "First number"},
                                        "b": {"type": "integer", "description": "Second number"},
                                    },
                                    "required": ["a", "b"],
                                },
                            },
                        ]
                    },
                }

            elif method == "tools/call":
                # Call a tool
                tool_name = params.get("name")
                arguments = params.get("arguments", {})

                if tool_name == "echo":
                    message = arguments.get("message", "")
                    result = {"output": f"Echo: {message}"}
                elif tool_name == "add":
                    a = arguments.get("a", 0)
                    b = arguments.get("b", 0)
                    result = {"result": a + b}
                else:
                    result = {"error": f"Unknown tool: {tool_name}"}

                response = {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": result,
                }

            elif method == "shutdown":
                # Shutdown server
                response = {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": {},
                }
                # Send response and exit
                sys.stdout.write(json.dumps(response) + "\n")
                sys.stdout.flush()
                break

            else:
                # Unknown method
                response = {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "error": {
                        "code": -32601,
                        "message": f"Method not found: {method}",
                    },
                }

            # Send response
            sys.stdout.write(json.dumps(response, ensure_ascii=False) + "\n")
            sys.stdout.flush()

        except json.JSONDecodeError as e:
            # Invalid JSON, skip
            sys.stderr.write(f"Invalid JSON: {e}\n")
            continue
        except Exception as e:
            # Unexpected error
            sys.stderr.write(f"Error: {e}\n")
            continue


if __name__ == "__main__":
    main()
