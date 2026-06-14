// App.tsx — Root orchestrator: connects state, hooks, and components.
import React, { useState, useEffect, useCallback } from 'react';
import { Project, ProjectListItem, Mutation, Layer } from './types';
import { usePreviewSocket } from './hooks/usePreviewSocket';
import { Sidebar } from './components/Sidebar';
import { CanvasViewer } from './components/CanvasViewer';
import { Timeline } from './components/Timeline';
import { Inspector } from './components/Inspector';

type WsStatus = 'connected' | 'disconnected' | 'reconnecting';

export default function App() {
  const [projects, setProjects] = useState<ProjectListItem[]>([]);
  const [activeProjectId, setActiveProjectId] = useState<number | null>(null);
  const [project, setProject] = useState<Project | null>(null);
  const [selectedLayerId, setSelectedLayerId] = useState<number | null>(null);
  const [currentFrame, setCurrentFrame] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [frameBlobUrl, setFrameBlobUrl] = useState<string | null>(null);
  const [scale, setScale] = useState(0.5);
  const [wsStatus, setWsStatus] = useState<WsStatus>('disconnected');

  // ---------- Computed values ----------
  const totalFrames = project ? Math.round((project.total_time / 1000) * project.fps) : 0;
  const currentSeconds = project ? currentFrame / project.fps : 0;
  const selectedLayer: Layer | undefined = project?.layers.find((l) => l.id === selectedLayerId);

  // ---------- REST helpers ----------
  const fetchProjects = useCallback(async () => {
    try {
      const res = await fetch('/api/projects');
      if (!res.ok) return;
      const data: ProjectListItem[] = await res.json();
      setProjects(data);
      if (data.length > 0) setActiveProjectId((prev) => prev ?? data[0].id);
    } catch (e) {
      console.error('Failed to fetch projects', e);
    }
  }, []);

  const fetchProject = useCallback(async (id: number) => {
    try {
      const res = await fetch(`/api/projects/${id}`);
      if (res.ok) setProject(await res.json());
    } catch (e) {
      console.error('Failed to fetch project details', e);
    }
  }, []);

  const mutate = useCallback(async (mutation: Mutation) => {
    if (activeProjectId === null) return;
    try {
      const res = await fetch(`/api/projects/${activeProjectId}/mutate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(mutation),
      });
      if (res.ok) setProject(await res.json());
    } catch (e) {
      console.error('Mutation failed', e);
    }
  }, [activeProjectId]);

  const undo = useCallback(async () => {
    if (activeProjectId === null) return;
    try {
      const res = await fetch(`/api/projects/${activeProjectId}/undo`, { method: 'POST' });
      if (res.ok) {
        setProject(await res.json());
        send({ type: 'seek', frame: currentFrame });
      }
    } catch (e) {
      console.error('Undo failed', e);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeProjectId, currentFrame]);

  const redo = useCallback(async () => {
    if (activeProjectId === null) return;
    try {
      const res = await fetch(`/api/projects/${activeProjectId}/redo`, { method: 'POST' });
      if (res.ok) {
        setProject(await res.json());
        send({ type: 'seek', frame: currentFrame });
      }
    } catch (e) {
      console.error('Redo failed', e);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeProjectId, currentFrame]);

  // ---------- WebSocket ----------
  const { send } = usePreviewSocket(activeProjectId, {
    onFrame: (frame, url) => {
      setCurrentFrame(frame);
      setFrameBlobUrl(url);
    },
    onOpen: () => {
      setWsStatus('connected');
      // Push current scale on reconnect
      send({ type: 'config', scale });
    },
    onClose: (_code, _reason) => {
      setWsStatus('reconnecting');
      setIsPlaying(false);
    },
  });

  // ---------- Transport ----------
  const play = useCallback(() => {
    send({ type: 'play', fps: project?.fps });
    setIsPlaying(true);
  }, [send, project]);

  const pause = useCallback(() => {
    send({ type: 'pause' });
    setIsPlaying(false);
  }, [send]);

  const seek = useCallback((frame: number) => {
    const clamped = Math.max(0, Math.min(frame, totalFrames - 1));
    send({ type: 'seek', frame: clamped });
    setCurrentFrame(clamped);
  }, [send, totalFrames]);

  const handleScaleChange = useCallback((newScale: number) => {
    setScale(newScale);
    send({ type: 'config', scale: newScale });
  }, [send]);

  // ---------- Project selection ----------
  const selectProject = useCallback((id: number) => {
    setActiveProjectId(id);
    setSelectedLayerId(null);
    setCurrentFrame(0);
    setIsPlaying(false);
    fetchProject(id);
  }, [fetchProject]);

  // ---------- Timeline drag events ----------
  useEffect(() => {
    const handleDrag = (e: Event) => {
      const { layerId, action, initialStartTime, initialEndTime, deltaTime } = (e as CustomEvent).detail;
      if (!project) return;
      setProject((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          layers: prev.layers.map((l) => {
            if (l.id !== layerId) return l;
            if (action === 'move') {
              const dur = initialEndTime - initialStartTime;
              const newStart = Math.max(0, Math.min(initialStartTime + deltaTime, prev.total_time - dur));
              return { ...l, start_time: newStart, end_time: newStart + dur };
            }
            if (action === 'resize-start') {
              return { ...l, start_time: Math.max(0, Math.min(initialStartTime + deltaTime, l.end_time - 100)) };
            }
            // resize-end
            return { ...l, end_time: Math.max(l.start_time + 100, Math.min(initialEndTime + deltaTime, prev.total_time)) };
          }),
        };
      });
    };

    const handleCommit = (e: Event) => {
      const { layerId, action, initialStartTime: _ist, initialEndTime: _iet } = (e as CustomEvent).detail;
      const layer = project?.layers.find((l) => l.id === layerId);
      if (!layer) return;
      if (action === 'move' || action === 'resize-start') {
        mutate({ type: 'update_layer_property', layer_id: layerId, property: 'start_time', value: layer.start_time });
      }
      if (action === 'move' || action === 'resize-end') {
        mutate({ type: 'update_layer_property', layer_id: layerId, property: 'end_time', value: layer.end_time });
      }
    };

    window.addEventListener('timeline-drag', handleDrag);
    window.addEventListener('timeline-drag-commit', handleCommit);
    return () => {
      window.removeEventListener('timeline-drag', handleDrag);
      window.removeEventListener('timeline-drag-commit', handleCommit);
    };
  }, [project, mutate]);

  // ---------- Keyboard shortcuts ----------
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA') return;
      if (e.code === 'Space') { e.preventDefault(); isPlaying ? pause() : play(); }
      if (e.code === 'ArrowLeft') { e.preventDefault(); seek(currentFrame - 1); }
      if (e.code === 'ArrowRight') { e.preventDefault(); seek(currentFrame + 1); }
      if ((e.ctrlKey || e.metaKey) && e.code === 'KeyZ') { e.preventDefault(); e.shiftKey ? redo() : undo(); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [isPlaying, play, pause, seek, currentFrame, undo, redo]);

  // ---------- Bootstrap ----------
  useEffect(() => { fetchProjects(); }, [fetchProjects]);
  useEffect(() => { if (activeProjectId !== null) fetchProject(activeProjectId); }, [activeProjectId, fetchProject]);

  // Disconnect status detection: if wsStatus is 'reconnecting' reset to 'disconnected' display after
  // some frames to prevent misleading status when fully offline.
  useEffect(() => {
    if (wsStatus === 'reconnecting') {
      const id = setTimeout(() => setWsStatus('disconnected'), 3000);
      return () => clearTimeout(id);
    }
  }, [wsStatus]);

  return (
    <div className="app-container">
      <Sidebar
        projects={projects}
        activeProjectId={activeProjectId}
        onSelectProject={selectProject}
        onUndo={undo}
        onRedo={redo}
        wsStatus={wsStatus}
      />

      <div className="main-content">
        <CanvasViewer
          project={project}
          frameBlobUrl={frameBlobUrl}
          scale={scale}
          onScaleChange={handleScaleChange}
          currentFrame={currentFrame}
          totalFrames={totalFrames}
          currentSeconds={currentSeconds}
          isPlaying={isPlaying}
          onPlay={play}
          onPause={pause}
          onStepBack={() => seek(currentFrame - 1)}
          onStepForward={() => seek(currentFrame + 1)}
          activeProjectId={activeProjectId}
        />

        <Timeline
          project={project}
          currentFrame={currentFrame}
          totalFrames={totalFrames}
          selectedLayerId={selectedLayerId}
          onSelectLayer={setSelectedLayerId}
          onSeek={seek}
          onLayerTimingChange={(layerId, startTime, endTime) => {
            mutate({ type: 'update_layer_property', layer_id: layerId, property: 'start_time', value: startTime });
            mutate({ type: 'update_layer_property', layer_id: layerId, property: 'end_time', value: endTime });
          }}
        />
      </div>

      <Inspector
        layer={selectedLayer ?? null}
        onMutate={mutate}
      />
    </div>
  );
}
