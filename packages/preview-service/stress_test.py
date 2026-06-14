import urllib.request
import urllib.error
import json
import socket
import threading
import time
import sys

BASE_URL = "http://127.0.0.1:8080"
WS_URL = "ws://127.0.0.1:8080/ws"

def send_post(endpoint, data=None):
    url = f"{BASE_URL}{endpoint}"
    req = urllib.request.Request(url, method='POST')
    if data is not None:
        req.add_header('Content-Type', 'application/json')
        jsondata = json.dumps(data).encode('utf-8')
        req.data = jsondata
    try:
        with urllib.request.urlopen(req) as response:
            return response.status, json.loads(response.read().decode('utf-8'))
    except urllib.error.HTTPError as e:
        try:
            err_body = e.read().decode('utf-8')
        except Exception:
            err_body = str(e)
        return e.code, err_body
    except Exception as e:
        return 500, str(e)

def send_get(endpoint):
    url = f"{BASE_URL}{endpoint}"
    try:
        with urllib.request.urlopen(url) as response:
            return response.status, json.loads(response.read().decode('utf-8'))
    except urllib.error.HTTPError as e:
        try:
            err_body = e.read().decode('utf-8')
        except Exception:
            err_body = str(e)
        return e.code, err_body
    except Exception as e:
        return 500, str(e)

# Simple WebSocket client using raw TCP socket to avoid external dependencies
class RawWebSocketClient:
    def __init__(self, host="127.0.0.1", port=8080):
        self.host = host
        self.port = port
        self.sock = None

    def connect(self):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.connect((self.host, self.port))
        # Send WebSocket Handshake
        handshake = (
            "GET /ws HTTP/1.1\r\n"
            f"Host: {self.host}:{self.port}\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n"
        )
        self.sock.sendall(handshake.encode())
        # Read response headers (simple read until \r\n\r\n)
        response = b""
        while b"\r\n\r\n" not in response:
            chunk = self.sock.recv(1024)
            if not chunk:
                break
            response += chunk
        if b"101 Switching Protocols" not in response:
            raise Exception("Failed WS Handshake: " + response.decode(errors='ignore'))

    def send_text(self, text):
        payload = text.encode('utf-8')
        payload_len = len(payload)
        
        # Frame header: Fin=1, RSV1-3=0, Opcode=1 (Text) -> 0x81
        header = bytearray([0x81])
        
        # Mask is required for client-to-server frames. Mask bit = 1 -> 0x80
        mask_key = b"\x11\x22\x33\x44"
        
        if payload_len <= 125:
            header.append(0x80 | payload_len)
        elif payload_len <= 65535:
            header.append(0x80 | 126)
            header.extend(payload_len.to_bytes(2, byteorder='big'))
        else:
            header.append(0x80 | 127)
            header.extend(payload_len.to_bytes(8, byteorder='big'))
            
        header.extend(mask_key)
        
        # Mask payload
        masked_payload = bytearray(payload_len)
        for i in range(payload_len):
            masked_payload[i] = payload[i] ^ mask_key[i % 4]
            
        self.sock.sendall(header + masked_payload)

    def close(self):
        if self.sock:
            self.sock.close()


def test_invalid_fps_panic():
    print("\n--- Test 1: Play Command with Invalid FPS (Panic Trigger) ---")
    client = RawWebSocketClient()
    try:
        client.connect()
        print("Connected to WS server.")
        
        # Send a play command with FPS 0.0 (causes division by zero and panic in Duration::from_secs_f32)
        payload = json.dumps({"type": "play", "fps": 0.0})
        print(f"Sending play command: {payload}")
        client.send_text(payload)
        
        # Wait to see if connection drops (server task panic)
        time.sleep(1.0)
        # Try to send another seek command
        client.send_text(json.dumps({"type": "seek", "frame": 10}))
        print("Sent seek after play. checking connection...")
        
        # Try to connect a new socket to see if server is still responsive
        client2 = RawWebSocketClient()
        client2.connect()
        print("New connection succeeded. Server still running, but the previous WS session panicked.")
        client2.close()
    except Exception as e:
        print(f"Result: Connection failed/dropped as expected during panic. Details: {e}")
    finally:
        client.close()

def test_non_existent_project_id():
    print("\n--- Test 2: Non-Existent Project ID Undo/Redo ---")
    # Project ID 999999 does not exist
    code, body = send_post("/api/projects/999999/undo")
    print(f"Undo result: Status Code: {code}, Body: {body}")
    # Expected: 404 Not Found, Actual: 400 Bad Request (Undo stack empty)
    
    code, body = send_post("/api/projects/999999/redo")
    print(f"Redo result: Status Code: {code}, Body: {body}")
    # Expected: 404 Not Found, Actual: 400 Bad Request (Redo stack empty)

def test_duplicate_layer_ids():
    print("\n--- Test 3: Duplicate Layer IDs Vulnerability ---")
    # Fetch first project ID
    code, projects = send_get("/api/projects")
    if code != 200 or not projects:
        print("No projects available to test.")
        return
    project_id = projects[0]['id']
    print(f"Using Project ID: {project_id}")

    # Add a layer with a custom ID
    duplicate_id = 999
    layer_data = {
        "id": duplicate_id,
        "label": "Duplicate Layer A",
        "start_time": 0.0,
        "end_time": 5.0,
        "hidden": False,
        "transform": {
            "location": {"type": "static", "value": [0.0, 0.0, 0.0]},
            "scale": {"type": "static", "value": [1.0, 1.0]},
            "rotation": {"type": "static", "value": 0.0},
            "opacity": {"type": "static", "value": 1.0}
        },
        "fill_type": "Color",
        "fill_color": [1.0, 0.0, 0.0, 1.0],
        "blend_mode": "Normal",
        "effects": [],
        "size": [100.0, 100.0]
    }
    
    mutation_add_1 = {
        "type": "add_layer",
        "layer": layer_data
    }
    
    code, res = send_post(f"/api/projects/{project_id}/mutate", mutation_add_1)
    print(f"First AddLayer status: {code}")

    # Add second layer with the same ID
    layer_data2 = layer_data.copy()
    layer_data2["label"] = "Duplicate Layer B"
    mutation_add_2 = {
        "type": "add_layer",
        "layer": layer_data2
    }
    code, res = send_post(f"/api/projects/{project_id}/mutate", mutation_add_2)
    print(f"Second AddLayer with duplicate ID status: {code}")
    # Expected: Warning or error. Actual: 200 OK (both layers added with id 999)

    # Now mutate the layer property
    mutation_update = {
        "type": "update_layer_property",
        "layer_id": duplicate_id,
        "property": "opacity",
        "value": 0.5
    }
    code, res = send_post(f"/api/projects/{project_id}/mutate", mutation_update)
    print(f"Mutate duplicate layer status: {code}")
    # The mutation succeeded, but check how many layers actually updated:
    # It only updates the first one in the DB matching that layer_id.

def test_websocket_deadlock():
    print("\n--- Test 4: WebSocket Deadlock Simulation ---")
    print("This simulates rapid connections and seeks to trigger the lock order deadlock.")
    
    # We will spawn a thread that repeatedly performs WS connects/disconnects,
    # and another thread that triggers seeks/renders.
    stop_event = threading.Event()
    
    def connect_loop():
        while not stop_event.is_set():
            try:
                c = RawWebSocketClient()
                c.connect()
                # Send seek
                c.send_text(json.dumps({"type": "seek", "frame": 5}))
                time.sleep(0.01)
                c.close()
            except Exception:
                pass
            time.sleep(0.01)

    def mutate_loop():
        # Fetch first project
        code, projects = send_get("/api/projects")
        if code != 200 or not projects:
            return
        project_id = projects[0]['id']
        while not stop_event.is_set():
            # Trigger seek or config change via POST
            send_post(f"/api/projects/{project_id}/mutate", {
                "type": "update_layer_property",
                "layer_id": 1,
                "property": "opacity",
                "value": 0.9
            })
            time.sleep(0.02)

    threads = [
        threading.Thread(target=connect_loop),
        threading.Thread(target=connect_loop),
        threading.Thread(target=mutate_loop)
    ]
    
    for t in threads:
        t.start()
        
    print("Running deadlock stress test for 5 seconds...")
    time.sleep(5.0)
    stop_event.set()
    
    for t in threads:
        t.join()
        
    # Check if server is still responsive
    code, res = send_get("/api/projects")
    if code == 200:
        print("Server is still responsive. Deadlock did not trigger in this short window.")
    else:
        print(f"Server is HUNG or returned error! Status: {code}. Deadlock successfully triggered!")

if __name__ == "__main__":
    print("AM-Renderer Preview Service REST API & WS Stress Test")
    print("Ensure the preview server is running on http://localhost:8080 before starting.")
    
    # Check if server is running
    try:
        urllib.request.urlopen(BASE_URL, timeout=2)
    except Exception:
        print(f"Error: Preview-service is not running at {BASE_URL}. Please start it first.")
        sys.exit(1)
        
    test_invalid_fps_panic()
    test_non_existent_project_id()
    test_duplicate_layer_ids()
    test_websocket_deadlock()
