// components/Inspector.tsx
import React from 'react';
import { Layer, Mutation, getStaticValue } from '../types';

interface InspectorProps {
  layer: Layer | null;
  onMutate: (mutation: Mutation) => void;
}

interface NumberFieldProps {
  label: string;
  sublabel?: string;
  value: number;
  step?: number;
  min?: number;
  max?: number;
  onChange: (v: number) => void;
}

const NumberField: React.FC<NumberFieldProps> = ({ label, sublabel, value, step = 1, min, max, onChange }) => (
  <div className="inspector-input-col">
    <input
      type="number"
      className="input"
      value={value}
      step={step}
      min={min}
      max={max}
      onChange={(e) => onChange(parseFloat(e.target.value) || 0)}
      aria-label={label}
    />
    {sublabel && <span className="inspector-input-sublabel">{sublabel}</span>}
  </div>
);

interface SliderFieldProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  displayValue: string;
  onChange: (v: number) => void;
}

const SliderField: React.FC<SliderFieldProps> = ({ label, value, min, max, step = 0.01, displayValue, onChange }) => (
  <div className="inspector-group">
    <span className="inspector-label">{label}: <strong>{displayValue}</strong></span>
    <input
      type="range"
      className="slider"
      min={min}
      max={max}
      step={step}
      value={value}
      onChange={(e) => onChange(parseFloat(e.target.value))}
      aria-label={label}
    />
  </div>
);

export const Inspector: React.FC<InspectorProps> = ({ layer, onMutate }) => {
  if (!layer) {
    return (
      <div className="inspector">
        <h2>Inspector</h2>
        <div className="inspector-empty">
          <div className="inspector-empty-icon">🔍</div>
          <p>Select a layer from the timeline to inspect and edit its properties.</p>
        </div>
      </div>
    );
  }

  const loc = getStaticValue(layer.transform.location, [0, 0, 0] as [number, number, number]);
  const scale = getStaticValue(layer.transform.scale, [1, 1] as [number, number]);
  const rotation = getStaticValue(layer.transform.rotation, 0);
  const opacity = getStaticValue(layer.transform.opacity, 1);

  const mutateProperty = (property: string, value: unknown) =>
    onMutate({ type: 'update_layer_property', layer_id: layer.id, property, value });

  return (
    <div className="inspector">
      <h2>Inspector</h2>

      {/* Layer header */}
      <div className="inspector-layer-header">
        <div className="inspector-layer-name">{layer.label ?? `Layer ${layer.id}`}</div>
        <div className="inspector-layer-meta">
          ID: {layer.id} &nbsp;·&nbsp; {layer.s ?? layer.fill_type ?? 'unknown'}
        </div>
        <div className="inspector-layer-meta">
          Blend: <span className="inspector-tag">{layer.blend_mode}</span>
        </div>
      </div>

      {/* Position */}
      <div className="inspector-group">
        <span className="inspector-label">Position</span>
        <div className="inspector-input-row">
          <NumberField
            label="Position X"
            sublabel="X"
            value={parseFloat(loc[0].toFixed(1))}
            onChange={(v) => mutateProperty('location', [v, loc[1], loc[2]])}
          />
          <NumberField
            label="Position Y"
            sublabel="Y"
            value={parseFloat(loc[1].toFixed(1))}
            onChange={(v) => mutateProperty('location', [loc[0], v, loc[2]])}
          />
        </div>
      </div>

      {/* Scale */}
      <div className="inspector-group">
        <span className="inspector-label">Scale</span>
        <div className="inspector-input-row">
          <NumberField
            label="Scale X"
            sublabel="X"
            value={parseFloat(scale[0].toFixed(3))}
            step={0.01}
            onChange={(v) => mutateProperty('scale', [v, scale[1]])}
          />
          <NumberField
            label="Scale Y"
            sublabel="Y"
            value={parseFloat(scale[1].toFixed(3))}
            step={0.01}
            onChange={(v) => mutateProperty('scale', [scale[0], v])}
          />
        </div>
      </div>

      {/* Rotation slider */}
      <SliderField
        label="Rotation"
        value={rotation}
        min={-360}
        max={360}
        step={0.1}
        displayValue={`${rotation.toFixed(1)}°`}
        onChange={(v) => mutateProperty('rotation', v)}
      />

      {/* Opacity slider */}
      <SliderField
        label="Opacity"
        value={opacity}
        min={0}
        max={1}
        step={0.01}
        displayValue={`${Math.round(opacity * 100)}%`}
        onChange={(v) => mutateProperty('opacity', v)}
      />

      {/* Effects list (read-only display) */}
      {Array.isArray(layer.effects) && layer.effects.length > 0 && (
        <div className="inspector-group">
          <span className="inspector-label">Effects ({layer.effects.length})</span>
          <div className="inspector-effects-list">
            {layer.effects.map((fx, i) => (
              <div key={i} className="inspector-effect-chip">
                {typeof fx === 'object' && fx !== null
                  ? Object.keys(fx as object)[0] ?? 'Unknown'
                  : 'Unknown'}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};
