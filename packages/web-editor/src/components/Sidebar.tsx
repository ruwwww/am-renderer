// components/Sidebar.tsx
import React from 'react';
import { ProjectListItem } from '../types';

interface SidebarProps {
  projects: ProjectListItem[];
  activeProjectId: number | null;
  onSelectProject: (id: number) => void;
  onUndo: () => void;
  onRedo: () => void;
  wsStatus: 'connected' | 'disconnected' | 'reconnecting';
}

const STATUS_COLORS: Record<SidebarProps['wsStatus'], string> = {
  connected: '#22c55e',
  disconnected: '#ef4444',
  reconnecting: '#f59e0b',
};

export const Sidebar: React.FC<SidebarProps> = ({
  projects,
  activeProjectId,
  onSelectProject,
  onUndo,
  onRedo,
  wsStatus,
}) => {
  return (
    <div className="sidebar">
      <div className="sidebar-header">
        <h2>am-renderer</h2>
        <div className="ws-status" title={`WebSocket: ${wsStatus}`}>
          <span
            className="ws-dot"
            style={{ backgroundColor: STATUS_COLORS[wsStatus] }}
          />
          <span className="ws-label">{wsStatus}</span>
        </div>
      </div>

      <p className="sidebar-section-label">Projects</p>
      <ul className="project-list">
        {projects.map((p) => (
          <li
            key={p.id}
            className={`project-item ${p.id === activeProjectId ? 'active' : ''}`}
            onClick={() => onSelectProject(p.id)}
          >
            <div className="project-item-title">{p.title ?? `Project ${p.id}`}</div>
            <div className="project-item-meta">
              {p.width}×{p.height} &nbsp;|&nbsp; {p.duration_secs.toFixed(2)}s &nbsp;|&nbsp; {p.fps} fps
            </div>
          </li>
        ))}
      </ul>

      <div className="sidebar-actions">
        <button className="btn" onClick={onUndo} title="Undo (Ctrl+Z)">
          ↩ Undo
        </button>
        <button className="btn" onClick={onRedo} title="Redo (Ctrl+Y)">
          ↪ Redo
        </button>
      </div>
    </div>
  );
};
