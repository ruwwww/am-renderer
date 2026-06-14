// components/Timeline.tsx
import React, { useRef, useCallback } from 'react';
import { Project, Layer } from '../types';

interface TimelineDragState {
  layerId: number;
  action: 'move' | 'resize-start' | 'resize-end';
  startX: number;
  initialStartTime: number;
  initialEndTime: number;
  laneWidth: number;
}

interface TimelineProps {
  project: Project | null;
  currentFrame: number;
  totalFrames: number;
  selectedLayerId: number | null;
  onSelectLayer: (id: number) => void;
  onSeek: (frame: number) => void;
  onLayerTimingChange: (layerId: number, startTime: number, endTime: number) => void;
}

/** Renders second/frame tick marks across the ruler */
function RulerTicks({ project, totalFrames }: { project: Project; totalFrames: number }) {
  const durationSecs = project.total_time / 1000;
  const ticks: React.ReactNode[] = [];
  const stepSecs = durationSecs <= 5 ? 0.5 : durationSecs <= 30 ? 1 : 5;

  for (let t = 0; t <= durationSecs; t += stepSecs) {
    const pct = (t / durationSecs) * 100;
    const label = t % 1 === 0 ? `${t}s` : `${t.toFixed(1)}s`;
    ticks.push(
      <div
        key={t}
        className="ruler-tick"
        style={{ left: `${pct}%` }}
      >
        <div className="ruler-tick-line" />
        <span className="ruler-tick-label">{label}</span>
      </div>
    );
  }
  return <>{ticks}</>;
}

export const Timeline: React.FC<TimelineProps> = ({
  project,
  currentFrame,
  totalFrames,
  selectedLayerId,
  onSelectLayer,
  onSeek,
  onLayerTimingChange,
}) => {
  const rulerRef = useRef<HTMLDivElement>(null);
  const laneRef = useRef<HTMLDivElement>(null);

  // ---------- Playhead / ruler scrub ----------
  const computeFrameFromEvent = useCallback(
    (clientX: number): number => {
      if (!rulerRef.current || !project) return 0;
      const rect = rulerRef.current.getBoundingClientRect();
      const laneLeft = rect.left + 150; // 150px label column offset
      const laneWidth = rect.width - 150;
      const ratio = Math.max(0, Math.min((clientX - laneLeft) / laneWidth, 1));
      return Math.round(ratio * (totalFrames - 1));
    },
    [project, totalFrames]
  );

  const handleRulerMouseDown = useCallback(
    (e: React.MouseEvent) => {
      onSeek(computeFrameFromEvent(e.clientX));
      const move = (ev: MouseEvent) => onSeek(computeFrameFromEvent(ev.clientX));
      const up = () => {
        window.removeEventListener('mousemove', move);
        window.removeEventListener('mouseup', up);
      };
      window.addEventListener('mousemove', move);
      window.addEventListener('mouseup', up);
    },
    [computeFrameFromEvent, onSeek]
  );

  // ---------- Layer bar drag ----------
  const startLayerDrag = useCallback(
    (e: React.PointerEvent, layer: Layer, action: TimelineDragState['action']) => {
      e.stopPropagation();
      if (!laneRef.current || !project) return;

      const laneWidth = laneRef.current.getBoundingClientRect().width;
      const drag: TimelineDragState = {
        layerId: layer.id,
        action,
        startX: e.clientX,
        initialStartTime: layer.start_time,
        initialEndTime: layer.end_time,
        laneWidth,
      };

      const handleMove = (ev: PointerEvent) => {
        const deltaX = ev.clientX - drag.startX;
        const deltaTime = (deltaX / drag.laneWidth) * project.total_time;

        // optimistic preview: dispatch a DOM custom event so App can update state
        const detail = { ...drag, deltaTime };
        window.dispatchEvent(new CustomEvent('timeline-drag', { detail }));
      };

      const handleUp = () => {
        window.dispatchEvent(new CustomEvent('timeline-drag-commit', { detail: drag }));
        window.removeEventListener('pointermove', handleMove);
        window.removeEventListener('pointerup', handleUp);
      };

      window.addEventListener('pointermove', handleMove);
      window.addEventListener('pointerup', handleUp);
    },
    [project]
  );

  // Compute playhead position
  const playheadPct =
    totalFrames > 1 ? (currentFrame / (totalFrames - 1)) * 100 : 0;

  if (!project) {
    return (
      <div className="timeline-container">
        <div className="timeline-empty">No project loaded</div>
      </div>
    );
  }

  return (
    <div className="timeline-container">
      {/* Ruler */}
      <div
        className="timeline-header"
        ref={rulerRef}
        onMouseDown={handleRulerMouseDown}
      >
        {/* Fixed label column spacer */}
        <div className="track-label-spacer" />

        {/* Tick marks */}
        <div className="ruler-lane">
          <RulerTicks project={project} totalFrames={totalFrames} />
          {/* Playhead */}
          <div
            className="playhead"
            style={{ left: `${playheadPct}%` }}
          >
            <div className="playhead-handle" />
          </div>
        </div>
      </div>

      {/* Layer tracks */}
      <div className="timeline-tracks">
        {project.layers.map((layer) => {
          const leftPct = (layer.start_time / project.total_time) * 100;
          const widthPct = ((layer.end_time - layer.start_time) / project.total_time) * 100;
          const isSelected = layer.id === selectedLayerId;

          return (
            <div
              key={layer.id}
              className="timeline-track"
              onClick={() => onSelectLayer(layer.id)}
            >
              <div className="track-label" title={layer.label ?? `Layer ${layer.id}`}>
                {layer.label ?? `Layer ${layer.id}`}
              </div>
              <div className="track-lane" ref={laneRef}>
                <div
                  className={`layer-bar${isSelected ? ' selected' : ''}`}
                  style={{ left: `${leftPct}%`, width: `${widthPct}%` }}
                  onPointerDown={(e) => startLayerDrag(e, layer, 'move')}
                >
                  <div
                    className="drag-handle"
                    onPointerDown={(e) => startLayerDrag(e, layer, 'resize-start')}
                  />
                  <span className="layer-bar-title">{layer.label ?? `Layer ${layer.id}`}</span>
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
  );
};
