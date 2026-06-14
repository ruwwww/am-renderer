import React, { useState, useEffect, useRef } from 'react';

export default function App() {
  const [projects, setProjects] = useState([]);
  const [activeProjectId, setActiveProjectId] = useState(null);
  const [project, setProject] = useState(null);
  const [selectedLayerId, setSelectedLayerId] = useState(null);
  const [currentFrame, setCurrentFrame] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [frameBlobUrl, setFrameBlobUrl] = useState(null);
  const [scale, setScale] = useState(0.5);

  const wsRef = useRef(null);
  const timelineHeaderRef = useRef(null);
  const timelineLaneRef = useRef(null);

  // Drag state for layer bar
  const [dragInfo, setDragInfo] = useState(null);

  // Fetch project list on startup
  useEffect(() => {
    fetchProjects();
  }, []);

  const fetchProjects = async () => {
    try {
      const res = await fetch('/api/projects');
      if (res.ok) {
        const data = await res.json();
        setProjects(data);
        if (data.length > 0 && !activeProjectId) {
          selectProject(data[0].id);
        }
      }
    } catch (e) {
      console.error("Failed to fetch projects", e);
    }
  };

  const fetchProjectDetails = async (id) => {
    try {
      const res = await fetch(`/api/projects/${id}`);
      if (res.ok) {
        const data = await res.json();
        setProject(data);
      }
    } catch (e) {
      console.error("Failed to fetch project details", e);
    }
  };

  const selectProject = (id) => {
    setActiveProjectId(id);
    setSelectedLayerId(null);
    setCurrentFrame(0);
    setIsPlaying(false);
    fetchProjectDetails(id);
  };

  // Connect WebSocket when active project changes
  useEffect(() => {
    if (!activeProjectId) return;

    if (wsRef.current) {
      wsRef.current.close();
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/ws`;
    const socket = new WebSocket(wsUrl);
    socket.binaryType = 'arraybuffer';

    socket.onopen = () => {
      console.log("WebSocket connected to", wsUrl);
      // Send scale configuration
      socket.send(JSON.stringify({ type: 'config', scale }));
    };

    socket.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        const view = new DataView(event.data);
        const frameNum = view.getUint32(0, false); // big-endian
        const webpBytes = event.data.slice(4);
        const blob = new Blob([webpBytes], { type: 'image/webp' });
        const url = URL.createObjectURL(blob);

        setFrameBlobUrl(prev => {
          if (prev) URL.revokeObjectURL(prev);
          return url;
        });
        setCurrentFrame(frameNum);
      }
    };

    socket.onclose = () => {
      console.log("WebSocket disconnected");
    };

    wsRef.current = socket;

    return () => {
      socket.close();
    };
  }, [activeProjectId]);

  // Update scale on server when state changes
  const handleScaleChange = (newScale) => {
    setScale(newScale);
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: 'config', scale: newScale }));
    }
  };

  // Helper to get static value of Animated<T>
  const getStaticValue = (animatedObj, defaultValue) => {
    if (!animatedObj) return defaultValue;
    if (animatedObj.Static !== undefined) {
      return animatedObj.Static;
    }
    if (animatedObj.Keyframed !== undefined && animatedObj.Keyframed.length > 0) {
      return animatedObj.Keyframed[0].value;
    }
    return defaultValue;
  };

  // Transport actions
  const play = () => {
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: 'play', fps: project?.fps }));
      setIsPlaying(true);
    }
  };

  const pause = () => {
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: 'pause' }));
      setIsPlaying(false);
    }
  };

  const seek = (frame) => {
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      const targetFrame = Math.max(0, Math.min(frame, totalFrames - 1));
      wsRef.current.send(JSON.stringify({ type: 'seek', frame: targetFrame }));
      setCurrentFrame(targetFrame);
    }
  };

  const undo = async () => {
    if (!activeProjectId) return;
    try {
      const res = await fetch(`/api/projects/${activeProjectId}/undo`, { method: 'POST' });
      if (res.ok) {
        const updated = await res.json();
        setProject(updated);
        // Force seek to refresh preview
        seek(currentFrame);
      }
    } catch (e) {
      console.error("Undo failed", e);
    }
  };

  const redo = async () => {
    if (!activeProjectId) return;
    try {
      const res = await fetch(`/api/projects/${activeProjectId}/redo`, { method: 'POST' });
      if (res.ok) {
        const updated = await res.json();
        setProject(updated);
        seek(currentFrame);
      }
    } catch (e) {
      console.error("Redo failed", e);
    }
  };

  const mutate = async (mutation) => {
    if (!activeProjectId) return;
    try {
      const res = await fetch(`/api/projects/${activeProjectId}/mutate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(mutation),
      });
      if (res.ok) {
        const updated = await res.json();
        setProject(updated);
      }
    } catch (e) {
      console.error("Mutation failed", e);
    }
  };

  // Calculate project metrics
  const totalFrames = project ? Math.round((project.total_time / 1000.0) * project.fps) : 0;
  const projectDurationSecs = project ? project.total_time / 1000.0 : 0;
  const currentSeconds = project ? currentFrame / project.fps : 0;

  // Selected layer
  const selectedLayer = project?.layers.find(l => l.id === selectedLayerId);

  // Timeline scrub handler
  const handleTimelineScrub = (e) => {
    if (!timelineHeaderRef.current || !project) return;
    const rect = timelineHeaderRef.current.getBoundingClientRect();
    const laneWidth = rect.width - 150; // offset track label width
    const clientX = e.clientX;
    const relativeX = Math.max(0, Math.min(clientX - rect.left - 150, laneWidth));
    const ratio = relativeX / laneWidth;
    const targetFrame = Math.round(ratio * (totalFrames - 1));
    seek(targetFrame);
  };

  const handleTimelineMouseDown = (e) => {
    handleTimelineScrub(e);
    const handleMouseMove = (moveEvent) => {
      handleTimelineScrub(moveEvent);
    };
    const handleMouseUp = () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
  };

  // Layer drag handlers
  const startLayerDrag = (e, layer, actionType) => {
    e.stopPropagation();
    if (!timelineLaneRef.current) return;
    const rect = timelineLaneRef.current.getBoundingClientRect();
    setDragInfo({
      layerId: layer.id,
      action: actionType,
      startX: e.clientX,
      initialStartTime: layer.start_time,
      initialEndTime: layer.end_time,
      laneWidth: rect.width,
    });
  };

  useEffect(() => {
    if (!dragInfo) return;

    const handlePointerMove = (e) => {
      const deltaX = e.clientX - dragInfo.startX;
      const deltaTime = (deltaX / dragInfo.laneWidth) * project.total_time;

      setProject(prev => {
        if (!prev) return prev;
        const updatedLayers = prev.layers.map(layer => {
          if (layer.id !== dragInfo.layerId) return layer;

          let newStart = layer.start_time;
          let newEnd = layer.end_time;

          if (dragInfo.action === 'move') {
            const duration = dragInfo.initialEndTime - dragInfo.initialStartTime;
            newStart = Math.max(0, Math.min(dragInfo.initialStartTime + deltaTime, project.total_time - duration));
            newEnd = newStart + duration;
          } else if (dragInfo.action === 'resize-start') {
            newStart = Math.max(0, Math.min(dragInfo.initialStartTime + deltaTime, layer.end_time - 100));
          } else if (dragInfo.action === 'resize-end') {
            newEnd = Math.max(layer.start_time + 100, Math.min(dragInfo.initialEndTime + deltaTime, project.total_time));
          }

          return { ...layer, start_time: newStart, end_time: newEnd };
        });
        return { ...prev, layers: updatedLayers };
      });
    };

    const handlePointerUp = async () => {
      const targetLayer = project?.layers.find(l => l.id === dragInfo.layerId);
      if (targetLayer) {
        // Send mutation API requests
        if (dragInfo.action === 'move') {
          await mutate({
            type: 'update_layer_property',
            layer_id: dragInfo.layerId,
            property: 'start_time',
            value: targetLayer.start_time,
          });
          await mutate({
            type: 'update_layer_property',
            layer_id: dragInfo.layerId,
            property: 'end_time',
            value: targetLayer.end_time,
          });
        } else if (dragInfo.action === 'resize-start') {
          await mutate({
            type: 'update_layer_property',
            layer_id: dragInfo.layerId,
            property: 'start_time',
            value: targetLayer.start_time,
          });
        } else if (dragInfo.action === 'resize-end') {
          await mutate({
            type: 'update_layer_property',
            layer_id: dragInfo.layerId,
            property: 'end_time',
            value: targetLayer.end_time,
          });
        }
      }
      setDragInfo(null);
    };

    window.addEventListener('pointermove', handlePointerMove);
    window.addEventListener('pointerup', handlePointerUp);

    return () => {
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', handlePointerUp);
    };
  }, [dragInfo, project, activeProjectId]);

  return (
    <div className="app-container">
      {/* Sidebar: Projects Selector */}
      <div className="sidebar">
        <h2>Projects</h2>
        <ul className="project-list">
          {projects.map(p => (
            <li
              key={p.id}
              className={`project-item ${p.id === activeProjectId ? 'active' : ''}`}
              onClick={() => selectProject(p.id)}
            >
              <div style={{ fontWeight: 'bold' }}>{p.title || `Project ${p.id}`}</div>
              <div style={{ fontSize: '0.8rem', color: '#9ca3af', marginTop: '4px' }}>
                {p.width}x{p.height} | {(p.duration_secs).toFixed(2)}s | {p.fps} fps
              </div>
            </li>
          ))}
        </ul>

        {/* Undo/Redo Buttons */}
        <div style={{ marginTop: '20px', display: 'flex', gap: '10px' }}>
          <button className="btn" style={{ flex: 1 }} onClick={undo}>Undo</button>
          <button className="btn" style={{ flex: 1 }} onClick={redo}>Redo</button>
        </div>
      </div>

      {/* Main Panel */}
      <div className="main-content">
        {/* Top: Canvas Area */}
        <div className="canvas-area">
          {frameBlobUrl ? (
            <div className="canvas-wrapper">
              <img
                src={frameBlobUrl}
                alt="Frame render preview"
                className="preview-image"
                style={{ width: project ? `${project.width * scale}px` : 'auto' }}
              />
            </div>
          ) : (
            <div style={{ color: '#6b7280' }}>
              {activeProjectId ? 'Rendering preview...' : 'Select a project to start'}
            </div>
          )}
        </div>

        {/* Middle: Control / Transport bar */}
        <div className="control-bar">
          <div className="transport-controls">
            <button className="btn btn-primary" onClick={isPlaying ? pause : play}>
              {isPlaying ? 'Pause' : 'Play'}
            </button>
            <button className="btn" onClick={() => seek(currentFrame - 1)}>Step Back</button>
            <button className="btn" onClick={() => seek(currentFrame + 1)}>Step Fwd</button>
            <span style={{ marginLeft: '10px', fontSize: '0.9rem', color: '#9ca3af' }}>
              Frame: {currentFrame} / {totalFrames} ({(currentSeconds).toFixed(2)}s)
            </span>
          </div>

          <div>
            <label style={{ fontSize: '0.85rem', color: '#9ca3af', marginRight: '8px' }}>Scale:</label>
            <select
              value={scale}
              onChange={(e) => handleScaleChange(parseFloat(e.target.value))}
              style={{ backgroundColor: '#374151', color: 'white', border: '1px solid #4b5563', borderRadius: '4px', padding: '4px' }}
            >
              <option value={0.25}>25%</option>
              <option value={0.5}>50%</option>
              <option value={0.75}>75%</option>
              <option value={1.0}>100%</option>
            </select>
          </div>
        </div>

        {/* Bottom: Timeline */}
        <div className="timeline-container">
          {/* Timeline header ruler */}
          <div
            className="timeline-header"
            ref={timelineHeaderRef}
            onMouseDown={handleTimelineMouseDown}
          >
            <div className="timeline-ticks">
              {/* Playhead line representation */}
              {project && (
                <div
                  className="playhead"
                  style={{
                    left: `${150 + (currentFrame / (totalFrames - 1 || 1)) * (timelineHeaderRef.current?.getBoundingClientRect().width - 150 || 0)}px`
                  }}
                >
                  <div className="playhead-handle" />
                </div>
              )}
              <div style={{ width: '150px', height: '100%' }} />
              <div style={{ flex: 1, paddingLeft: '8px', fontSize: '0.75rem', color: '#6b7280', display: 'flex', alignItems: 'center' }}>
                Seek Timeline Ruler (0.0s - {projectDurationSecs.toFixed(2)}s)
              </div>
            </div>
          </div>

          {/* Timeline layer lanes */}
          <div className="timeline-tracks">
            {project?.layers.map(layer => {
              const leftPercent = (layer.start_time / project.total_time) * 100;
              const widthPercent = ((layer.end_time - layer.start_time) / project.total_time) * 100;

              return (
                <div
                  key={layer.id}
                  className="timeline-track"
                  onClick={() => setSelectedLayerId(layer.id)}
                >
                  <div className="track-label">{layer.label || `Layer ${layer.id}`}</div>
                  <div className="track-lane" ref={timelineLaneRef}>
                    <div
                      className={`layer-bar ${layer.id === selectedLayerId ? 'selected' : ''}`}
                      style={{
                        left: `${leftPercent}%`,
                        width: `${widthPercent}%`,
                      }}
                      onPointerDown={(e) => startLayerDrag(e, layer, 'move')}
                    >
                      <div
                        className="drag-handle"
                        onPointerDown={(e) => startLayerDrag(e, layer, 'resize-start')}
                      />
                      <span className="layer-bar-title">{layer.label || `Layer ${layer.id}`}</span>
                      <div
                        className="drag-handle"
                        onPointerDown={(e) => startLayerDrag(e, layer, 'resize-end')}
                      />
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      {/* Right Sidebar: Inspector */}
      <div className="inspector">
        <h2>Inspector</h2>
        {selectedLayer ? (
          <div>
            <div style={{ marginBottom: '15px', borderBottom: '1px solid #374151', paddingBottom: '10px' }}>
              <div style={{ fontWeight: 'bold' }}>{selectedLayer.label || `Layer ${selectedLayer.id}`}</div>
              <div style={{ fontSize: '0.8rem', color: '#9ca3af', marginTop: '4px' }}>
                ID: {selectedLayer.id} | Type: {selectedLayer.s || 'unknown'}
              </div>
            </div>

            {/* Location (X, Y, Z) */}
            <div className="inspector-group">
              <span className="inspector-label">Location (X, Y, Z)</span>
              <div className="inspector-input-row">
                <div className="inspector-input-col">
                  <input
                    type="number"
                    className="input"
                    value={getStaticValue(selectedLayer.transform.location, [0, 0, 0])[0].toFixed(1)}
                    onChange={(e) => {
                      const current = getStaticValue(selectedLayer.transform.location, [0, 0, 0]);
                      const val = parseFloat(e.target.value) || 0;
                      mutate({
                        type: 'update_layer_property',
                        layer_id: selectedLayer.id,
                        property: 'location',
                        value: [val, current[1], current[2]],
                      });
                    }}
                  />
                  <span className="inspector-input-sublabel">X</span>
                </div>
                <div className="inspector-input-col">
                  <input
                    type="number"
                    className="input"
                    value={getStaticValue(selectedLayer.transform.location, [0, 0, 0])[1].toFixed(1)}
                    onChange={(e) => {
                      const current = getStaticValue(selectedLayer.transform.location, [0, 0, 0]);
                      const val = parseFloat(e.target.value) || 0;
                      mutate({
                        type: 'update_layer_property',
                        layer_id: selectedLayer.id,
                        property: 'location',
                        value: [current[0], val, current[2]],
                      });
                    }}
                  />
                  <span className="inspector-input-sublabel">Y</span>
                </div>
              </div>
            </div>

            {/* Scale (X, Y) */}
            <div className="inspector-group">
              <span className="inspector-label">Scale (X, Y)</span>
              <div className="inspector-input-row">
                <div className="inspector-input-col">
                  <input
                    type="number"
                    step="0.1"
                    className="input"
                    value={getStaticValue(selectedLayer.transform.scale, [1, 1])[0].toFixed(2)}
                    onChange={(e) => {
                      const current = getStaticValue(selectedLayer.transform.scale, [1, 1]);
                      const val = parseFloat(e.target.value) || 1.0;
                      mutate({
                        type: 'update_layer_property',
                        layer_id: selectedLayer.id,
                        property: 'scale',
                        value: [val, current[1]],
                      });
                    }}
                  />
                  <span className="inspector-input-sublabel">X</span>
                </div>
                <div className="inspector-input-col">
                  <input
                    type="number"
                    step="0.1"
                    className="input"
                    value={getStaticValue(selectedLayer.transform.scale, [1, 1])[1].toFixed(2)}
                    onChange={(e) => {
                      const current = getStaticValue(selectedLayer.transform.scale, [1, 1]);
                      const val = parseFloat(e.target.value) || 1.0;
                      mutate({
                        type: 'update_layer_property',
                        layer_id: selectedLayer.id,
                        property: 'scale',
                        value: [current[0], val],
                      });
                    }}
                  />
                  <span className="inspector-input-sublabel">Y</span>
                </div>
              </div>
            </div>

            {/* Rotation */}
            <div className="inspector-group">
              <span className="inspector-label">Rotation: {getStaticValue(selectedLayer.transform.rotation, 0).toFixed(1)}°</span>
              <input
                type="range"
                min="0"
                max="360"
                className="slider"
                value={getStaticValue(selectedLayer.transform.rotation, 0)}
                onChange={(e) => {
                  const val = parseFloat(e.target.value);
                  mutate({
                    type: 'update_layer_property',
                    layer_id: selectedLayer.id,
                    property: 'rotation',
                    value: val,
                  });
                }}
              />
            </div>

            {/* Opacity */}
            <div className="inspector-group">
              <span className="inspector-label">Opacity: {(getStaticValue(selectedLayer.transform.opacity, 1.0) * 100).toFixed(0)}%</span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                className="slider"
                value={getStaticValue(selectedLayer.transform.opacity, 1.0)}
                onChange={(e) => {
                  const val = parseFloat(e.target.value);
                  mutate({
                    type: 'update_layer_property',
                    layer_id: selectedLayer.id,
                    property: 'opacity',
                    value: val,
                  });
                }}
              />
            </div>
          </div>
        ) : (
          <div style={{ color: '#6b7280', fontStyle: 'italic' }}>
            Select a layer from the timeline to edit properties.
          </div>
        )}
      </div>
    </div>
  );
}
