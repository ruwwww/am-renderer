import http.server
import socketserver
import json
import subprocess
import os
import sys
import xml.etree.ElementTree as ET
from urllib.parse import urlparse, parse_qs
import webbrowser

PORT = 8080
XML_PATH = os.path.join("presets", "preset10.xml")
ASSETS_DIR = "assets"

def load_xml_layers(xml_path):
    if not os.path.exists(xml_path):
        raise FileNotFoundError(f"XML file not found at {xml_path}")
        
    tree = ET.parse(xml_path)
    root = tree.getroot()
    layers = []
    
    # We find shape and embedScene elements
    elements = root.findall('.//shape') + root.findall('.//embedScene')
    # Order layers bottom-to-top (as they appear in Alight Motion rendering)
    for element in elements:
        layer_id = element.attrib.get('id')
        label = element.attrib.get('label', layer_id)
        start_time = float(element.attrib.get('startTime', '0'))
        end_time = float(element.attrib.get('endTime', '0'))
        hidden = element.attrib.get('hidden', 'false') == 'true'
        blending = element.attrib.get('blending', 'normal')
        fill_type = element.attrib.get('fillType', 'none')
        
        # Parse transform
        transform_elem = element.find('transform')
        opacity = 1.0
        if transform_elem is not None:
            opacity_elem = transform_elem.find('opacity')
            if opacity_elem is not None:
                val_attr = opacity_elem.attrib.get('value')
                if val_attr:
                    try:
                        opacity = float(val_attr)
                    except ValueError:
                        pass
                    
        # Parse effects
        effects = []
        for effect_elem in element.findall('effect'):
            effect_id = effect_elem.attrib.get('id')
            properties = {}
            for prop in effect_elem.findall('property'):
                name = prop.attrib.get('name')
                prop_type = prop.attrib.get('type')
                val_str = prop.attrib.get('value')
                
                if val_str:
                    try:
                        if prop_type == 'float':
                            properties[name] = float(val_str)
                        elif prop_type == 'vec3':
                            properties[name] = [float(x) for x in val_str.split(',')]
                        elif prop_type == 'vec2':
                            properties[name] = [float(x) for x in val_str.split(',')]
                        elif prop_type == 'color':
                            properties[name] = val_str
                        elif prop_type == 'bool':
                            properties[name] = val_str == 'true'
                        else:
                            properties[name] = val_str
                    except ValueError:
                        properties[name] = val_str
            
            effects.append({
                'id': effect_id,
                'properties': properties,
                'disabled': effect_elem.attrib.get('disabled', 'false') == 'true'
            })
            
        layers.append({
            'id': layer_id,
            'label': label,
            'startTime': start_time,
            'endTime': end_time,
            'hidden': hidden,
            'blending': blending,
            'opacity': opacity,
            'fillType': fill_type,
            'effects': effects
        })
    return layers

def modify_and_render(xml_path, layers_mod, frame, proxy_scale):
    tree = ET.parse(xml_path)
    root = tree.getroot()
    
    # Index layers_mod by ID for quick lookup
    mod_dict = {str(l['id']): l for l in layers_mod}
    
    elements = root.findall('.//shape') + root.findall('.//embedScene')
    for elem in elements:
        eid = elem.attrib.get('id')
        if eid in mod_dict:
            mod = mod_dict[eid]
            
            # 1. Update visibility
            if mod.get('hidden', False):
                elem.set('hidden', 'true')
            else:
                if 'hidden' in elem.attrib:
                    del elem.attrib['hidden']
                    
            # 2. Update blending
            if 'blending' in mod:
                elem.set('blending', mod['blending'])
                
            # 3. Update opacity inside transform
            if 'opacity' in mod:
                trans = elem.find('transform')
                if trans is None:
                    trans = ET.SubElement(elem, 'transform')
                opac = trans.find('opacity')
                if opac is None:
                    opac = ET.SubElement(trans, 'opacity')
                val = mod['opacity']
                if isinstance(val, (int, float)):
                    opac.set('value', f"{val:.6f}")
                else:
                    opac.set('value', str(val))
                
            # 4. Update/Remove effects
            if 'effects' in mod:
                updated_effects = mod['effects']
                updated_eff_dict = {e['id']: e for e in updated_effects}
                
                for eff_elem in list(elem.findall('effect')):
                    eff_id = eff_elem.attrib.get('id')
                    if eff_id in updated_eff_dict:
                        eff_mod = updated_eff_dict[eff_id]
                        if eff_mod.get('disabled', False):
                            elem.remove(eff_elem)
                        else:
                            properties = eff_mod.get('properties', {})
                            for prop_elem in eff_elem.findall('property'):
                                prop_name = prop_elem.attrib.get('name')
                                if prop_name in properties:
                                    val = properties[prop_name]
                                    if isinstance(val, list):
                                        val_str = ','.join(f"{x:.6f}" if isinstance(x, (int, float)) else str(x) for x in val)
                                    elif isinstance(val, bool):
                                        val_str = 'true' if val else 'false'
                                    elif isinstance(val, (int, float)):
                                        val_str = f"{val:.6f}"
                                    else:
                                        val_str = str(val)
                                    prop_elem.set('value', val_str)
                                    
    # Write modified XML to a temporary file
    temp_dir = os.path.join(".temp_frames")
    os.makedirs(temp_dir, exist_ok=True)
    temp_xml_path = os.path.join(temp_dir, "temp_preset.xml")
    tree.write(temp_xml_path, encoding='utf-8', xml_declaration=True)
    
    # Use release binary if available, otherwise debug binary
    bin_path = os.path.join("target", "release", "am-renderer.exe")
    if not os.path.exists(bin_path):
        bin_path = os.path.join("target", "debug", "am-renderer.exe")
        
    cmd = [
        bin_path,
        'render',
        '-i', temp_xml_path,
        '-a', ASSETS_DIR,
        '-o', os.path.join(".temp_frames", "preview"),
        '--frame', str(frame),
        '--proxy-scale', f"{proxy_scale:.4f}"
    ]
    
    result = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode != 0:
        raise Exception(f"am-renderer failed:\n{result.stderr.decode('utf-8')}")
        
    rendered_img_path = os.path.join(".temp_frames", "preview", f"frame_{frame:06d}.png")
    if not os.path.exists(rendered_img_path):
        raise Exception(f"Rendered image not found at {rendered_img_path}")
        
    with open(rendered_img_path, 'rb') as f:
        img_bytes = f.read()
        
    return img_bytes

class DebugHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        # Suppress logging to keep output clean
        pass

    def do_GET(self):
        parsed = urlparse(self.path)
        if parsed.path == '/':
            self.send_response(200)
            self.send_header('Content-Type', 'text/html')
            self.end_headers()
            self.wfile.write(HTML_CONTENT.encode('utf-8'))
        elif parsed.path == '/api/load':
            try:
                layers = load_xml_layers(XML_PATH)
                self.send_json({'layers': layers})
            except Exception as e:
                self.send_response(500)
                self.send_header('Content-Type', 'text/plain')
                self.end_headers()
                self.wfile.write(str(e).encode('utf-8'))
        else:
            self.send_error(404, 'Not Found')

    def do_POST(self):
        parsed = urlparse(self.path)
        if parsed.path == '/api/render':
            content_length = int(self.headers['Content-Length'])
            post_data = self.rfile.read(content_length)
            try:
                data = json.loads(post_data.decode('utf-8'))
                frame = data.get('frame', 0)
                proxy_scale = data.get('proxy_scale', 1.0)
                layers_mod = data.get('layers', [])
                
                img_data = modify_and_render(XML_PATH, layers_mod, frame, proxy_scale)
                
                self.send_response(200)
                self.send_header('Content-Type', 'image/png')
                self.send_header('Content-Length', str(len(img_data)))
                self.end_headers()
                self.wfile.write(img_data)
            except Exception as e:
                import traceback
                traceback.print_exc()
                self.send_response(500)
                self.send_header('Content-Type', 'text/plain')
                self.end_headers()
                self.wfile.write(str(e).encode('utf-8'))
        else:
            self.send_error(404, 'Not Found')

    def send_json(self, data):
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(json.dumps(data).encode('utf-8'))

HTML_CONTENT = """<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>am-renderer — Interactive Debugger</title>
    <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;600;800&display=swap" rel="stylesheet">
    <style>
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: 'Outfit', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background-color: #0c0c0e;
            color: #e2e2e9;
            height: 100vh;
            overflow: hidden;
            display: flex;
            flex-direction: column;
        }
        header {
            background-color: #121216;
            border-bottom: 1px solid rgba(255, 255, 255, 0.08);
            padding: 15px 24px;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }
        header h1 {
            font-size: 1.25rem;
            font-weight: 800;
            letter-spacing: 0.5px;
            color: #fff;
            display: flex;
            align-items: center;
            gap: 10px;
        }
        header h1 span {
            background: linear-gradient(135deg, #a78bfa, #7c3aed);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }
        .container {
            display: flex;
            flex: 1;
            height: calc(100vh - 60px);
            overflow: hidden;
        }
        
        /* Sidebar (Left) */
        .sidebar {
            width: 320px;
            background-color: #121216;
            border-right: 1px solid rgba(255, 255, 255, 0.08);
            display: flex;
            flex-direction: column;
            overflow: hidden;
        }
        .sidebar-header {
            padding: 16px;
            border-bottom: 1px solid rgba(255, 255, 255, 0.05);
            font-weight: 600;
            font-size: 0.9rem;
            text-transform: uppercase;
            letter-spacing: 0.8px;
            color: #8e8e9f;
        }
        .layer-list {
            flex: 1;
            overflow-y: auto;
            padding: 8px;
        }
        .layer-item {
            display: flex;
            align-items: center;
            padding: 10px 12px;
            border-radius: 8px;
            cursor: pointer;
            margin-bottom: 4px;
            transition: all 0.2s ease;
            border: 1px solid transparent;
            background-color: rgba(255, 255, 255, 0.01);
        }
        .layer-item:hover {
            background-color: rgba(255, 255, 255, 0.04);
        }
        .layer-item.active {
            background-color: rgba(124, 58, 237, 0.15);
            border-color: rgba(124, 58, 237, 0.4);
        }
        .layer-item.inactive {
            opacity: 0.45;
            filter: grayscale(40%);
        }
        .layer-item.inactive:hover {
            opacity: 0.75;
            filter: none;
        }
        .layer-checkbox {
            margin-right: 12px;
            accent-color: #8b5cf6;
            cursor: pointer;
            width: 16px;
            height: 16px;
        }
        .layer-info {
            flex: 1;
            min-width: 0;
        }
        .layer-name {
            font-weight: 500;
            font-size: 0.875rem;
            color: #fff;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }
        .layer-meta {
            font-size: 0.75rem;
            color: #8e8e9f;
            margin-top: 2px;
            display: flex;
            justify-content: space-between;
        }
        
        /* Central Preview */
        .preview-area {
            flex: 1;
            display: flex;
            flex-direction: column;
            background-color: #08080a;
            position: relative;
        }
        .canvas-container {
            flex: 1;
            display: flex;
            align-items: center;
            justify-content: center;
            padding: 24px;
            position: relative;
            overflow: hidden;
        }
        .canvas-container img {
            width: 100%;
            height: 100%;
            box-shadow: 0 20px 40px rgba(0,0,0,0.5);
            border-radius: 4px;
            border: 1px solid rgba(255,255,255,0.05);
            object-fit: contain;
            background-image: linear-gradient(45deg, #18181b 25%, transparent 25%), 
                              linear-gradient(-45deg, #18181b 25%, transparent 25%), 
                              linear-gradient(45deg, transparent 75%, #18181b 75%), 
                              linear-gradient(-45deg, transparent 75%, #18181b 75%);
            background-size: 20px 20px;
            background-position: 0 0, 0 10px, 10px -10px, -10px 0px;
        }
        
        /* Controls & Timeline */
        .controls-panel {
            background-color: #121216;
            border-top: 1px solid rgba(255, 255, 255, 0.08);
            padding: 16px 24px;
            display: flex;
            flex-direction: column;
            gap: 12px;
        }
        .timeline-row {
            display: flex;
            align-items: center;
            gap: 16px;
        }
        .timeline-slider {
            flex: 1;
            accent-color: #8b5cf6;
            height: 6px;
            border-radius: 3px;
            cursor: pointer;
        }
        .frame-input-container {
            display: flex;
            align-items: center;
            gap: 8px;
        }
        .frame-input {
            width: 70px;
            background-color: #1c1c24;
            border: 1px solid rgba(255, 255, 255, 0.1);
            color: #fff;
            padding: 6px 10px;
            border-radius: 6px;
            font-family: inherit;
            text-align: center;
            font-size: 0.9rem;
        }
        .frame-input:focus {
            border-color: #8b5cf6;
            outline: none;
        }
        .settings-row {
            display: flex;
            justify-content: space-between;
            align-items: center;
            font-size: 0.875rem;
        }
        .settings-left {
            display: flex;
            gap: 20px;
        }
        .settings-item {
            display: flex;
            align-items: center;
            gap: 8px;
        }
        .settings-select {
            background-color: #1c1c24;
            border: 1px solid rgba(255, 255, 255, 0.1);
            color: #fff;
            padding: 4px 8px;
            border-radius: 6px;
            font-family: inherit;
            cursor: pointer;
        }
        .settings-select:focus {
            border-color: #8b5cf6;
            outline: none;
        }
        .latency-badge {
            font-size: 0.75rem;
            background-color: rgba(255, 255, 255, 0.05);
            padding: 4px 8px;
            border-radius: 4px;
            color: #a1a1aa;
        }
        
        /* Inspector (Right) */
        .inspector {
            width: 360px;
            background-color: #121216;
            border-left: 1px solid rgba(255, 255, 255, 0.08);
            display: flex;
            flex-direction: column;
            overflow: hidden;
        }
        .inspector-header {
            padding: 16px;
            border-bottom: 1px solid rgba(255, 255, 255, 0.05);
            font-weight: 600;
            color: #fff;
        }
        .inspector-content {
            flex: 1;
            overflow-y: auto;
            padding: 16px;
            display: flex;
            flex-direction: column;
            gap: 20px;
        }
        .section-title {
            font-size: 0.8rem;
            text-transform: uppercase;
            letter-spacing: 0.8px;
            color: #8e8e9f;
            margin-bottom: 12px;
            font-weight: 600;
        }
        .property-group {
            display: flex;
            flex-direction: column;
            gap: 12px;
            background-color: rgba(255, 255, 255, 0.02);
            padding: 12px;
            border-radius: 8px;
            border: 1px solid rgba(255,255,255,0.03);
        }
        .property-row {
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 12px;
        }
        .property-label {
            font-size: 0.875rem;
            color: #a1a1aa;
        }
        .property-value-slider {
            flex: 1;
            accent-color: #8b5cf6;
            max-width: 140px;
        }
        .property-value-text {
            font-size: 0.85rem;
            width: 50px;
            text-align: right;
            color: #fff;
            font-family: monospace;
        }
        
        /* Effects Card */
        .effect-card {
            background-color: rgba(255, 255, 255, 0.02);
            border: 1px solid rgba(255, 255, 255, 0.05);
            border-radius: 8px;
            overflow: hidden;
            margin-bottom: 10px;
        }
        .effect-card-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 10px 12px;
            background-color: rgba(255, 255, 255, 0.04);
            border-bottom: 1px solid rgba(255, 255, 255, 0.03);
        }
        .effect-name {
            font-size: 0.875rem;
            font-weight: 600;
            color: #fff;
        }
        .effect-card-content {
            padding: 12px;
            display: flex;
            flex-direction: column;
            gap: 10px;
        }
        
        /* Spinner & Overlay */
        .loader-overlay {
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background-color: rgba(0,0,0,0.4);
            display: none;
            justify-content: center;
            align-items: center;
            z-index: 10;
        }
        .spinner {
            width: 50px;
            height: 50px;
            border: 3px solid rgba(139, 92, 246, 0.2);
            border-top: 3px solid #8b5cf6;
            border-radius: 50%;
            animation: spin 0.8s linear infinite;
        }
        @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
        }
        .error-message {
            position: absolute;
            bottom: 20px;
            left: 50%;
            transform: translateX(-50%);
            background-color: #ef4444;
            color: #fff;
            padding: 10px 20px;
            border-radius: 6px;
            font-size: 0.9rem;
            box-shadow: 0 10px 20px rgba(0,0,0,0.3);
            display: none;
            max-width: 80%;
            z-index: 20;
        }
    </style>
</head>
<body>
    <header>
        <h1>am-renderer // <span>Interactive Debugger</span></h1>
        <div class="latency-badge" id="latencyDisplay">Render: --ms</div>
    </header>

    <div class="container">
        <!-- Sidebar (Left) -->
        <div class="sidebar">
            <div class="sidebar-header">Layers (Bottom-to-Top)</div>
            <div class="layer-list" id="layerList">
                <!-- Layers populated dynamically -->
            </div>
        </div>

        <!-- Preview Area (Center) -->
        <div class="preview-area">
            <div class="loader-overlay" id="loaderOverlay">
                <div class="spinner"></div>
            </div>
            <div class="error-message" id="errorMessage">Error text here</div>
            
            <div class="canvas-container">
                <img id="previewImage" src="" alt="Render Preview">
            </div>

            <!-- Controls (Bottom) -->
            <div class="controls-panel">
                <div class="timeline-row">
                    <span style="font-size: 0.85rem; color: #a1a1aa; width: 35px;">Frame</span>
                    <input type="range" class="timeline-slider" id="timelineSlider" min="0" max="510" value="211">
                    <div class="frame-input-container">
                        <input type="number" class="frame-input" id="frameInput" min="0" max="510" value="211">
                        <span style="font-size: 0.85rem; color: #a1a1aa;">/ 510</span>
                    </div>
                </div>
                <div class="settings-row">
                    <div class="settings-left">
                        <div class="settings-item">
                            <label for="proxyScale">Proxy Scale:</label>
                            <select class="settings-select" id="proxyScale">
                                <option value="0.10">0.10x (Ultra Fast)</option>
                                <option value="0.25" selected>0.25x (Recommended)</option>
                                <option value="0.50">0.50x (Medium)</option>
                                <option value="1.00">1.00x (Full Quality)</option>
                            </select>
                        </div>
                    </div>
                    <button class="settings-select" style="background-color:#8b5cf6; border:none; padding:6px 14px; font-weight:600; color:#fff;" id="renderBtn">Trigger Render</button>
                </div>
            </div>
        </div>

        <!-- Inspector (Right) -->
        <div class="inspector">
            <div class="inspector-header" id="selectedLayerTitle">No Layer Selected</div>
            <div class="inspector-content" id="inspectorContent">
                <!-- Controls populated dynamically -->
                <p style="color:#8e8e9f; font-size:0.9rem; text-align:center; margin-top:20px;">Select a layer from the list to inspect and edit its properties.</p>
            </div>
        </div>
    </div>

    <script>
        let allLayers = [];
        let selectedLayerId = null;
        let currentFrame = 211;
        let renderTimeout = null;

        // Elements
        const layerListContainer = document.getElementById('layerList');
        const inspectorContent = document.getElementById('inspectorContent');
        const selectedLayerTitle = document.getElementById('selectedLayerTitle');
        const timelineSlider = document.getElementById('timelineSlider');
        const frameInput = document.getElementById('frameInput');
        const proxyScaleSelect = document.getElementById('proxyScale');
        const renderBtn = document.getElementById('renderBtn');
        const previewImage = document.getElementById('previewImage');
        const loaderOverlay = document.getElementById('loaderOverlay');
        const errorMessage = document.getElementById('errorMessage');
        const latencyDisplay = document.getElementById('latencyDisplay');

        // Initial setup
        window.addEventListener('load', async () => {
            await loadLayers();
            updateTimelineBounds();
            triggerRender();
        });

        async function loadLayers() {
            try {
                const response = await fetch('/api/load');
                if (!response.ok) throw new Error(await response.text());
                const data = await response.json();
                allLayers = data.layers;
                
                renderLayerList();
                if (allLayers.length > 0) {
                    selectLayer(allLayers[allLayers.length - 1].id); // Select top layer by default
                }
            } catch (e) {
                showError("Failed to load project layers: " + e.message);
            }
        }

        function updateTimelineBounds() {
            // Find max bounds if preset has duration or total frames
            // Standard presets are 510 frames (17 seconds at 30 fps)
        }

        function renderLayerList() {
            layerListContainer.innerHTML = '';
            const currentTimeMs = (currentFrame / 30.0) * 1000.0;
            
            // Render from top to bottom in list (so index is reverse of render order, top layer on top)
            for (let i = allLayers.length - 1; i >= 0; i--) {
                const layer = allLayers[i];
                const isActive = currentTimeMs >= layer.startTime && currentTimeMs < layer.endTime;
                
                const item = document.createElement('div');
                let classes = 'layer-item';
                if (layer.id === selectedLayerId) classes += ' active';
                if (!isActive) classes += ' inactive';
                item.className = classes;
                item.dataset.id = layer.id;
                
                const checkbox = document.createElement('input');
                checkbox.type = 'checkbox';
                checkbox.className = 'layer-checkbox';
                checkbox.checked = !layer.hidden;
                checkbox.addEventListener('change', (e) => {
                    layer.hidden = !e.target.checked;
                    queueRender();
                });
                
                const info = document.createElement('div');
                info.className = 'layer-info';
                
                const name = document.createElement('div');
                name.className = 'layer-name';
                name.textContent = layer.label || `Layer ${layer.id}`;
                
                const meta = document.createElement('div');
                meta.className = 'layer-meta';
                const fillLabel = layer.fillType.charAt(0).toUpperCase() + layer.fillType.slice(1);
                
                let timeRange = '';
                if (!isActive) {
                    timeRange = `<span style="color: #f87171;">Inactive (${(layer.startTime/1000).toFixed(1)}s-${(layer.endTime/1000).toFixed(1)}s)</span>`;
                } else {
                    timeRange = `<span style="color: #34d399;">Active</span>`;
                }
                meta.innerHTML = `<span>Fill: ${fillLabel}</span> ${timeRange}`;
                
                info.appendChild(name);
                info.appendChild(meta);
                
                item.appendChild(checkbox);
                item.appendChild(info);
                
                item.addEventListener('click', (e) => {
                    if (e.target.className !== 'layer-checkbox') {
                        selectLayer(layer.id);
                    }
                });
                
                layerListContainer.appendChild(item);
            }
        }

        function selectLayer(id) {
            selectedLayerId = id;
            
            // Highlight selected item in sidebar
            const items = layerListContainer.querySelectorAll('.layer-item');
            items.forEach(item => {
                if (item.dataset.id === id) {
                    item.classList.add('active');
                } else {
                    item.classList.remove('active');
                }
            });
            
            const layer = allLayers.find(l => l.id === id);
            if (layer) {
                selectedLayerTitle.textContent = layer.label || `Layer ${layer.id}`;
                renderInspector(layer);
            }
        }

        function renderInspector(layer) {
            inspectorContent.innerHTML = '';

            // Section: General Properties
            const genSection = document.createElement('div');
            genSection.innerHTML = '<div class="section-title">General Properties</div>';
            
            const propGroup = document.createElement('div');
            propGroup.className = 'property-group';
            
            // Blend Mode
            const blendRow = document.createElement('div');
            blendRow.className = 'property-row';
            blendRow.innerHTML = `<span class="property-label">Blend Mode</span>`;
            const blendSelect = document.createElement('select');
            blendSelect.className = 'settings-select';
            const blendModes = ['normal', 'multiply', 'screen', 'overlay', 'darken', 'lighten', 'subtract', 'add', 'linear-dodge'];
            blendModes.forEach(m => {
                const opt = document.createElement('option');
                opt.value = m;
                opt.textContent = m.toUpperCase();
                if (m === layer.blending) opt.selected = true;
                blendSelect.appendChild(opt);
            });
            blendSelect.addEventListener('change', (e) => {
                layer.blending = e.target.value;
                renderLayerList(); // update label
                queueRender();
            });
            blendRow.appendChild(blendSelect);
            propGroup.appendChild(blendRow);

            // Opacity
            const opacityRow = document.createElement('div');
            opacityRow.className = 'property-row';
            opacityRow.innerHTML = `<span class="property-label">Opacity</span>`;
            const opacitySlider = document.createElement('input');
            opacitySlider.type = 'range';
            opacitySlider.className = 'property-value-slider';
            opacitySlider.min = '0';
            opacitySlider.max = '1';
            opacitySlider.step = '0.005';
            opacitySlider.value = layer.opacity;
            const opacityVal = document.createElement('span');
            opacityVal.className = 'property-value-text';
            opacityVal.textContent = parseFloat(layer.opacity).toFixed(2);
            opacitySlider.addEventListener('input', (e) => {
                layer.opacity = parseFloat(e.target.value);
                opacityVal.textContent = layer.opacity.toFixed(2);
                queueRender();
            });
            opacityRow.appendChild(opacitySlider);
            opacityRow.appendChild(opacityVal);
            propGroup.appendChild(opacityRow);
            
            genSection.appendChild(propGroup);
            inspectorContent.appendChild(genSection);

            // Section: Visual Effects
            const effSection = document.createElement('div');
            effSection.innerHTML = '<div class="section-title">Visual Effects</div>';
            
            if (!layer.effects || layer.effects.length === 0) {
                const p = document.createElement('p');
                p.style.color = '#8e8e9f';
                p.style.fontSize = '0.875rem';
                p.style.textAlign = 'center';
                p.style.padding = '12px';
                p.textContent = 'No effects on this layer.';
                effSection.appendChild(p);
            } else {
                layer.effects.forEach(eff => {
                    const card = document.createElement('div');
                    card.className = 'effect-card';
                    
                    const cardHeader = document.createElement('div');
                    cardHeader.className = 'effect-card-header';
                    
                    const name = document.createElement('span');
                    name.className = 'effect-name';
                    // Clean names: com.alightcreative.effects.colorize -> Colorize
                    const parts = eff.id.split('.');
                    const cleanName = parts[parts.length - 1].toUpperCase();
                    name.textContent = cleanName;
                    
                    const toggle = document.createElement('input');
                    toggle.type = 'checkbox';
                    toggle.checked = !eff.disabled;
                    toggle.addEventListener('change', (e) => {
                        eff.disabled = !e.target.checked;
                        queueRender();
                    });
                    
                    cardHeader.appendChild(name);
                    cardHeader.appendChild(toggle);
                    card.appendChild(cardHeader);
                    
                    const cardContent = document.createElement('div');
                    cardContent.className = 'effect-card-content';
                    
                    // Populate specific effect controls
                    if (eff.id === 'com.alightcreative.effects.colorize') {
                        // Colorize Tint: Tint Hue (-180 to 180 or 0 to 360) & Tint Strength (0 to 1)
                        // properties.tint is [hue, strength, 0.0]
                        const tint = eff.properties.tint || [0.0, 0.0, 0.0];
                        
                        // Hue Slider
                        const hueRow = document.createElement('div');
                        hueRow.className = 'property-row';
                        hueRow.innerHTML = '<span class="property-label">Tint Hue (Deg)</span>';
                        const hueSlider = document.createElement('input');
                        hueSlider.type = 'range';
                        hueSlider.className = 'property-value-slider';
                        hueSlider.min = '-180';
                        hueSlider.max = '180';
                        hueSlider.step = '1';
                        hueSlider.value = tint[0];
                        const hueVal = document.createElement('span');
                        hueVal.className = 'property-value-text';
                        hueVal.textContent = Math.round(tint[0]) + '°';
                        hueSlider.addEventListener('input', (e) => {
                            tint[0] = parseFloat(e.target.value);
                            hueVal.textContent = Math.round(tint[0]) + '°';
                            eff.properties.tint = tint;
                            queueRender();
                        });
                        hueRow.appendChild(hueSlider);
                        hueRow.appendChild(hueVal);
                        cardContent.appendChild(hueRow);
                        
                        // Strength Slider
                        const strRow = document.createElement('div');
                        strRow.className = 'property-row';
                        strRow.innerHTML = '<span class="property-label">Strength</span>';
                        const strSlider = document.createElement('input');
                        strSlider.type = 'range';
                        strSlider.className = 'property-value-slider';
                        strSlider.min = '0';
                        strSlider.max = '1';
                        strSlider.step = '0.01';
                        strSlider.value = tint[1];
                        const strVal = document.createElement('span');
                        strVal.className = 'property-value-text';
                        strVal.textContent = tint[1].toFixed(2);
                        strSlider.addEventListener('input', (e) => {
                            tint[1] = parseFloat(e.target.value);
                            strVal.textContent = tint[1].toFixed(2);
                            eff.properties.tint = tint;
                            queueRender();
                        });
                        strRow.appendChild(strSlider);
                        strRow.appendChild(strVal);
                        cardContent.appendChild(strRow);
                    }
                    else if (eff.id === 'com.alightcreative.effects.exposure') {
                        // properties: exposure (stops), gamma, offset
                        const exposure = eff.properties.exposure !== undefined ? eff.properties.exposure : 0.0;
                        const gamma = eff.properties.gamma !== undefined ? eff.properties.gamma : 1.0;
                        const offset = eff.properties.offset !== undefined ? eff.properties.offset : 0.0;
                        
                        // Exposure
                        const expRow = document.createElement('div');
                        expRow.className = 'property-row';
                        expRow.innerHTML = '<span class="property-label">Exposure (Stops)</span>';
                        const expSlider = document.createElement('input');
                        expSlider.type = 'range';
                        expSlider.className = 'property-value-slider';
                        expSlider.min = '-5.0';
                        expSlider.max = '5.0';
                        expSlider.step = '0.05';
                        expSlider.value = exposure;
                        const expVal = document.createElement('span');
                        expVal.className = 'property-value-text';
                        expVal.textContent = exposure.toFixed(2);
                        expSlider.addEventListener('input', (e) => {
                            eff.properties.exposure = parseFloat(e.target.value);
                            expVal.textContent = eff.properties.exposure.toFixed(2);
                            queueRender();
                        });
                        expRow.appendChild(expSlider);
                        expRow.appendChild(expVal);
                        cardContent.appendChild(expRow);
                        
                        // Gamma
                        const gamRow = document.createElement('div');
                        gamRow.className = 'property-row';
                        gamRow.innerHTML = '<span class="property-label">Gamma</span>';
                        const gamSlider = document.createElement('input');
                        gamSlider.type = 'range';
                        gamSlider.className = 'property-value-slider';
                        gamSlider.min = '0.1';
                        gamSlider.max = '3.0';
                        gamSlider.step = '0.05';
                        gamSlider.value = gamma;
                        const gamVal = document.createElement('span');
                        gamVal.className = 'property-value-text';
                        gamVal.textContent = gamma.toFixed(2);
                        gamSlider.addEventListener('input', (e) => {
                            eff.properties.gamma = parseFloat(e.target.value);
                            gamVal.textContent = eff.properties.gamma.toFixed(2);
                            queueRender();
                        });
                        gamRow.appendChild(gamSlider);
                        gamRow.appendChild(gamVal);
                        cardContent.appendChild(gamRow);
                        
                        // Offset
                        const offRow = document.createElement('div');
                        offRow.className = 'property-row';
                        offRow.innerHTML = '<span class="property-label">Offset</span>';
                        const offSlider = document.createElement('input');
                        offSlider.type = 'range';
                        offSlider.className = 'property-value-slider';
                        offSlider.min = '-1.0';
                        offSlider.max = '1.0';
                        offSlider.step = '0.01';
                        offSlider.value = offset;
                        const offVal = document.createElement('span');
                        offVal.className = 'property-value-text';
                        offVal.textContent = offset.toFixed(2);
                        offSlider.addEventListener('input', (e) => {
                            eff.properties.offset = parseFloat(e.target.value);
                            offVal.textContent = eff.properties.offset.toFixed(2);
                            queueRender();
                        });
                        offRow.appendChild(offSlider);
                        offRow.appendChild(offVal);
                        cardContent.appendChild(offRow);
                    }
                    else if (eff.id === 'com.alightcreative.effects.saturationvibrance' || eff.id === 'com.alightcreative.effects.satvib') {
                        // saturation
                        const sat = eff.properties.saturation !== undefined ? eff.properties.saturation : 0.0;
                        const satRow = document.createElement('div');
                        satRow.className = 'property-row';
                        satRow.innerHTML = '<span class="property-label">Saturation</span>';
                        const satSlider = document.createElement('input');
                        satSlider.type = 'range';
                        satSlider.className = 'property-value-slider';
                        satSlider.min = '-1.0';
                        satSlider.max = '1.0';
                        satSlider.step = '0.05';
                        satSlider.value = sat;
                        const satVal = document.createElement('span');
                        satVal.className = 'property-value-text';
                        satVal.textContent = sat.toFixed(2);
                        satSlider.addEventListener('input', (e) => {
                            eff.properties.saturation = parseFloat(e.target.value);
                            satVal.textContent = eff.properties.saturation.toFixed(2);
                            queueRender();
                        });
                        satRow.appendChild(satSlider);
                        satRow.appendChild(satVal);
                        cardContent.appendChild(satRow);
                    }
                    else if (eff.id === 'com.alightcreative.effects.lift') {
                        // fill
                        const fill = eff.properties.fill !== undefined ? eff.properties.fill : 0.0;
                        const fillRow = document.createElement('div');
                        fillRow.className = 'property-row';
                        fillRow.innerHTML = '<span class="property-label">Lift/Copy Fill</span>';
                        const fillSlider = document.createElement('input');
                        fillSlider.type = 'range';
                        fillSlider.className = 'property-value-slider';
                        fillSlider.min = '0.0';
                        fillSlider.max = '1.0';
                        fillSlider.step = '0.02';
                        fillSlider.value = fill;
                        const fillVal = document.createElement('span');
                        fillVal.className = 'property-value-text';
                        fillVal.textContent = fill.toFixed(2);
                        fillSlider.addEventListener('input', (e) => {
                            eff.properties.fill = parseFloat(e.target.value);
                            fillVal.textContent = eff.properties.fill.toFixed(2);
                            queueRender();
                        });
                        fillRow.appendChild(fillSlider);
                        fillRow.appendChild(fillVal);
                        cardContent.appendChild(fillRow);
                    }
                    else {
                        // Fallback for other effects (list properties simply)
                        if (Object.keys(eff.properties).length === 0) {
                            const p = document.createElement('p');
                            p.style.color = '#8e8e9f';
                            p.style.fontSize = '0.8rem';
                            p.textContent = 'No editable parameters.';
                            cardContent.appendChild(p);
                        } else {
                            Object.keys(eff.properties).forEach(propKey => {
                                const row = document.createElement('div');
                                row.className = 'property-row';
                                row.innerHTML = `<span class="property-label">${propKey}</span>`;
                                const valSpan = document.createElement('span');
                                valSpan.className = 'property-value-text';
                                valSpan.style.width = 'auto';
                                valSpan.textContent = JSON.stringify(eff.properties[propKey]);
                                row.appendChild(valSpan);
                                cardContent.appendChild(row);
                            });
                        }
                    }
                    
                    card.appendChild(cardContent);
                    effSection.appendChild(card);
                });
            }
            inspectorContent.appendChild(effSection);
        }

        // Timeline Scrubbing Event Handlers
        timelineSlider.addEventListener('input', (e) => {
            currentFrame = parseInt(e.target.value);
            frameInput.value = currentFrame;
            renderLayerList();
            queueRender();
        });
        
        frameInput.addEventListener('change', (e) => {
            let val = parseInt(e.target.value);
            if (isNaN(val)) val = 0;
            currentFrame = Math.max(0, Math.min(510, val));
            frameInput.value = currentFrame;
            timelineSlider.value = currentFrame;
            renderLayerList();
            queueRender();
        });

        proxyScaleSelect.addEventListener('change', () => {
            triggerRender();
        });
        
        renderBtn.addEventListener('click', () => {
            triggerRender();
        });

        function queueRender() {
            clearTimeout(renderTimeout);
            renderTimeout = setTimeout(triggerRender, 150); // debounce renders by 150ms
        }

        async function triggerRender() {
            showLoading(true);
            const startTime = performance.now();
            
            try {
                const response = await fetch('/api/render', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        frame: currentFrame,
                        proxy_scale: parseFloat(proxyScaleSelect.value),
                        layers: allLayers
                    })
                });
                
                if (!response.ok) {
                    const text = await response.text();
                    throw new Error(text || 'Failed to render frame');
                }
                
                const blob = await response.blob();
                const objectURL = URL.createObjectURL(blob);
                
                // Keep reference to clean memory
                if (previewImage.src && previewImage.src.startsWith('blob:')) {
                    URL.revokeObjectURL(previewImage.src);
                }
                
                previewImage.src = objectURL;
                
                const latency = (performance.now() - startTime).toFixed(0);
                latencyDisplay.textContent = `Render: ${latency}ms`;
                errorMessage.style.display = 'none';
            } catch (e) {
                showError("Rendering failed: " + e.message);
            } finally {
                showLoading(false);
            }
        }

        function showLoading(show) {
            loaderOverlay.style.display = show ? 'flex' : 'none';
        }

        function showError(msg) {
            errorMessage.textContent = msg;
            errorMessage.style.display = 'block';
            console.error(msg);
        }
    </script>
</body>
</html>
"""

def main():
    global XML_PATH, ASSETS_DIR, PORT
    import argparse
    parser = argparse.ArgumentParser(description="Alight Motion Renderer Interactive Debugger")
    parser.add_argument("--port", type=int, default=PORT, help="Port to run the HTTP server on")
    parser.add_argument("--xml", type=str, default=XML_PATH, help="Path to Alight Motion XML template file")
    parser.add_argument("--assets", type=str, default=ASSETS_DIR, help="Path to assets folder")
    
    args = parser.parse_args()
    
    XML_PATH = args.xml
    ASSETS_DIR = args.assets
    PORT = args.port

    # Verify input exists
    if not os.path.exists(XML_PATH):
        print(f"Error: XML file '{XML_PATH}' does not exist.")
        sys.exit(1)
        
    if not os.path.exists(ASSETS_DIR):
        print(f"Warning: Assets folder '{ASSETS_DIR}' does not exist.")

    # Try starting server
    socketserver.TCPServer.allow_reuse_address = True
    try:
        with socketserver.TCPServer(("", PORT), DebugHandler) as httpd:
            print("=================================================================")
            print(f"  Interactive Debugger Server running at http://localhost:{PORT}")
            print("=================================================================")
            print("  Press Ctrl+C to terminate the server.")
            print(f"  Watching: {XML_PATH}")
            print(f"  Assets: {ASSETS_DIR}")
            
            # Automatically launch web browser
            webbrowser.open(f"http://localhost:{PORT}")
            
            # Using poll_interval allows KeyboardInterrupt to be caught on Windows
            httpd.serve_forever(poll_interval=0.5)
    except KeyboardInterrupt:
        print("\nServer terminated by user.")
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    main()
