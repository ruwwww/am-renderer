// hooks/usePreviewSocket.ts
// Manages the WebSocket connection to preview-service with automatic
// exponential-backoff reconnection on unexpected disconnection.

import { useEffect, useRef, useCallback } from 'react';
import { WsOutgoing } from '../types';

const MIN_RECONNECT_MS = 500;
const MAX_RECONNECT_MS = 16_000;

interface UsePreviewSocketOptions {
  /** Called with (frameNumber, webpBlobUrl) when a new frame arrives. */
  onFrame: (frame: number, blobUrl: string) => void;
  /** Called when the connection opens. */
  onOpen?: () => void;
  /** Called when the connection closes (code, reason). */
  onClose?: (code: number, reason: string) => void;
}

interface UsePreviewSocketReturn {
  send: (msg: WsOutgoing) => void;
  isConnected: () => boolean;
}

export function usePreviewSocket(
  activeProjectId: number | null,
  options: UsePreviewSocketOptions
): UsePreviewSocketReturn {
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const reconnectDelayRef = useRef<number>(MIN_RECONNECT_MS);
  // Track whether we intentionally closed the socket (project change / unmount)
  const intentionalCloseRef = useRef<boolean>(false);

  const { onFrame, onOpen, onClose } = options;

  const connect = useCallback(() => {
    if (!activeProjectId) return;

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/ws`;

    const socket = new WebSocket(wsUrl);
    socket.binaryType = 'arraybuffer';
    wsRef.current = socket;

    socket.onopen = () => {
      reconnectDelayRef.current = MIN_RECONNECT_MS;  // Reset backoff on success
      onOpen?.();
    };

    socket.onmessage = (event: MessageEvent) => {
      if (!(event.data instanceof ArrayBuffer)) return;
      const view = new DataView(event.data);
      const frameNum = view.getUint32(0, false); // big-endian
      const webpBytes = event.data.slice(4);
      const blob = new Blob([webpBytes], { type: 'image/webp' });
      const url = URL.createObjectURL(blob);
      onFrame(frameNum, url);
    };

    socket.onclose = (event: CloseEvent) => {
      onClose?.(event.code, event.reason);

      // Don't reconnect if we deliberately closed
      if (intentionalCloseRef.current) return;

      // Exponential backoff reconnect
      const delay = reconnectDelayRef.current;
      reconnectDelayRef.current = Math.min(delay * 2, MAX_RECONNECT_MS);
      reconnectTimeoutRef.current = setTimeout(() => {
        if (!intentionalCloseRef.current) connect();
      }, delay);
    };

    socket.onerror = () => {
      // onclose will fire after onerror, reconnection handled there
      socket.close();
    };
  }, [activeProjectId, onFrame, onOpen, onClose]);

  // Connect / reconnect when project changes
  useEffect(() => {
    intentionalCloseRef.current = false;
    reconnectDelayRef.current = MIN_RECONNECT_MS;

    // Clean up previous connection and pending reconnects
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }
    if (wsRef.current) {
      intentionalCloseRef.current = true;
      wsRef.current.close();
      wsRef.current = null;
      intentionalCloseRef.current = false;
    }

    if (activeProjectId !== null) connect();

    return () => {
      intentionalCloseRef.current = true;
      if (reconnectTimeoutRef.current) clearTimeout(reconnectTimeoutRef.current);
      wsRef.current?.close();
      wsRef.current = null;
    };
  }, [activeProjectId, connect]);

  const send = useCallback((msg: WsOutgoing) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(msg));
    }
  }, []);

  const isConnected = useCallback(
    () => wsRef.current?.readyState === WebSocket.OPEN,
    []
  );

  return { send, isConnected };
}
