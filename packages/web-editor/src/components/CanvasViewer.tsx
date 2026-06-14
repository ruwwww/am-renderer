// components/CanvasViewer.tsx
import React, { useEffect, useRef } from 'react';
import { Project } from '../types';

interface CanvasViewerProps {
  project: Project | null;
  frameBlobUrl: string | null;
  scale: number;
  onScaleChange: (scale: number) => void;
  currentFrame: number;
  totalFrames: number;
  currentSeconds: number;
  isPlaying: boolean;
  onPlay: () => void;
  onPause: () => void;
  onStepBack: () => void;
  onStepForward: () => void;
  activeProjectId: number | null;
}

const SCALE_OPTIONS = [
  { label: '25%', value: 0.25 },
  { label: '50%', value: 0.5 },
  { label: '75%', value: 0.75 },
  { label: '100%', value: 1.0 },
];

export const CanvasViewer: React.FC<CanvasViewerProps> = ({
  project,
  frameBlobUrl,
  scale,
  onScaleChange,
  currentFrame,
  totalFrames,
  currentSeconds,
  isPlaying,
  onPlay,
  onPause,
  onStepBack,
  onStepForward,
  activeProjectId,
}) => {
  // Revoke stale blob URLs to prevent memory leaks
  const prevUrlRef = useRef<string | null>(null);
  useEffect(() => {
    if (prevUrlRef.current && prevUrlRef.current !== frameBlobUrl) {
      URL.revokeObjectURL(prevUrlRef.current);
    }
    prevUrlRef.current = frameBlobUrl;
  }, [frameBlobUrl]);

  const previewWidth = project ? Math.round(project.width * scale) : undefined;

  return (
    <>
      {/* Canvas Area */}
      <div className="canvas-area">
        {frameBlobUrl ? (
          <div className="canvas-wrapper">
            <img
              src={frameBlobUrl}
              alt="Frame render preview"
              className="preview-image"
              style={{ width: previewWidth }}
            />
          </div>
        ) : (
          <div className="canvas-placeholder">
            <div className="canvas-placeholder-icon">🎬</div>
            <div className="canvas-placeholder-text">
              {activeProjectId ? 'Waiting for first frame…' : 'Select a project to start'}
            </div>
          </div>
        )}
      </div>

      {/* Transport / Control Bar */}
      <div className="control-bar">
        <div className="transport-controls">
          <button
            id="btn-step-back"
            className="btn icon-btn"
            onClick={onStepBack}
            title="Step back one frame"
          >
            ⏮
          </button>
          <button
            id="btn-play-pause"
            className="btn btn-primary icon-btn"
            onClick={isPlaying ? onPause : onPlay}
            title={isPlaying ? 'Pause' : 'Play'}
          >
            {isPlaying ? '⏸' : '▶'}
          </button>
          <button
            id="btn-step-forward"
            className="btn icon-btn"
            onClick={onStepForward}
            title="Step forward one frame"
          >
            ⏭
          </button>
          <span className="timecode">
            {String(Math.floor(currentSeconds / 60)).padStart(2, '0')}:
            {(currentSeconds % 60).toFixed(2).padStart(5, '0')}
            <span className="timecode-frames"> [{currentFrame}/{totalFrames}]</span>
          </span>
        </div>

        <div className="scale-control">
          <label className="scale-label" htmlFor="scale-select">Preview</label>
          <select
            id="scale-select"
            className="scale-select"
            value={scale}
            onChange={(e) => onScaleChange(parseFloat(e.target.value))}
          >
            {SCALE_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>{o.label}</option>
            ))}
          </select>
        </div>
      </div>
    </>
  );
};
