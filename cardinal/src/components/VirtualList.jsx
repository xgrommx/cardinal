// React & libs
import React, {
  useRef,
  useState,
  useCallback,
  useMemo,
  useLayoutEffect,
  useEffect,
  forwardRef,
  useImperativeHandle,
} from 'react';
import { invoke } from '@tauri-apps/api/core';
import Scrollbar from './Scrollbar';
import { useDataLoader } from '../hooks/useDataLoader';

// Virtualized list with lazy row hydration and synchronized column scrolling
export const VirtualList = forwardRef(function VirtualList(
  { results = null, rowHeight = 24, overscan = 5, renderRow, onScrollSync, className = '' },
  ref,
) {
  // ----- refs -----
  const containerRef = useRef(null);
  const iconRequestIdRef = useRef(0);

  // ----- state -----
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);

  // ----- derived -----
  // Row count is inferred from the results array; explicit rowCount is no longer supported
  const rowCount = results?.length ?? 0;

  // ----- data loader -----
  const { cache, ensureRangeLoaded } = useDataLoader(results);

  // Virtualized height powers the scrollbar math
  const totalHeight = rowCount * rowHeight;
  const maxScrollTop = Math.max(0, totalHeight - viewportHeight);

  // ----- callbacks: pure calculations first -----
  // Compute visible window (with overscan) based on the current scroll offset
  const start =
    rowCount && viewportHeight ? Math.max(0, Math.floor(scrollTop / rowHeight) - overscan) : 0;
  const end =
    rowCount && viewportHeight
      ? Math.min(rowCount - 1, Math.ceil((scrollTop + viewportHeight) / rowHeight) + overscan - 1)
      : -1;

  // Clamp scroll updates so callers cannot push the viewport outside legal bounds
  const updateScrollAndRange = useCallback(
    (updater) => {
      setScrollTop((prev) => {
        const nextValue = updater(prev);
        const clamped = Math.max(0, Math.min(nextValue, maxScrollTop));
        return prev === clamped ? prev : clamped;
      });
    },
    [maxScrollTop],
  );

  // ----- event handlers -----
  // Normalise wheel deltas (line/page vs pixel) for consistent vertical scrolling
  const handleWheel = useCallback(
    (e) => {
      e.preventDefault();
      const { deltaMode, deltaY } = e;
      let delta = deltaY;
      if (deltaMode === 1) {
        delta = deltaY * rowHeight;
      } else if (deltaMode === 2) {
        const pageSize = viewportHeight || rowHeight * 10;
        delta = deltaY * pageSize;
      }
      updateScrollAndRange((prev) => prev + delta);
    },
    [rowHeight, viewportHeight, updateScrollAndRange],
  );

  // Propagate horizontal scroll offset to the parent (keeps column headers aligned)
  const handleHorizontalScroll = useCallback(
    (e) => {
      if (onScrollSync) onScrollSync(e.target.scrollLeft);
    },
    [onScrollSync],
  );

  // ----- effects -----
  const updateIconViewport = useCallback((viewport) => {
    const requestId = iconRequestIdRef.current + 1;
    iconRequestIdRef.current = requestId;
    // Notify the backend which rows are visible so icon thumbnails can stream lazily
    invoke('update_icon_viewport', { id: requestId, viewport }).catch((error) => {
      console.error('Failed to update icon viewport', error);
    });
  }, []);

  // Ensure the data cache stays warm for the active window
  useEffect(() => {
    if (end >= start) ensureRangeLoaded(start, end);
  }, [start, end, ensureRangeLoaded]);

  useEffect(() => {
    if (!Array.isArray(results) || results.length === 0 || end < start) {
      updateIconViewport([]);
      return;
    }

    const clampedStart = Math.max(0, start);
    const clampedEnd = Math.min(end, results.length - 1);
    if (clampedEnd < clampedStart) {
      updateIconViewport([]);
      return;
    }

    const viewport = results.slice(clampedStart, clampedEnd + 1);

    updateIconViewport(viewport);
  }, [results, start, end, updateIconViewport]);

  useEffect(
    () => () => {
      updateIconViewport([]);
    },
    [updateIconViewport],
  );

  // Track container height changes so virtualization recalculates the viewport
  useLayoutEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const updateViewport = () => setViewportHeight(container.clientHeight);
    const resizeObserver = new ResizeObserver(updateViewport);
    resizeObserver.observe(container);
    updateViewport();
    return () => resizeObserver.disconnect();
  }, []);

  // Re-clamp scrollTop whenever total height shrinks (e.g. due to a narrower result set)
  useEffect(() => {
    setScrollTop((prev) => {
      const clamped = Math.max(0, Math.min(prev, maxScrollTop));
      return clamped === prev ? prev : clamped;
    });
  }, [maxScrollTop]);

  // ----- imperative API -----
  // Imperative handle used by App.jsx to drive preloading and programmatic scroll
  useImperativeHandle(
    ref,
    () => ({
      scrollToTop: () => updateScrollAndRange(() => 0),
      ensureRangeLoaded,
    }),
    [updateScrollAndRange, ensureRangeLoaded],
  );

  // ----- rendered items memo -----
  // Memoize rendered rows so virtualization only re-renders what it must
  const renderedItems = useMemo(() => {
    if (end < start) return null;

    const baseTop = start * rowHeight - scrollTop;
    return Array.from({ length: end - start + 1 }, (_, i) => {
      const rowIndex = start + i;
      const item = cache.get(rowIndex);
      return renderRow(rowIndex, item, {
        position: 'absolute',
        top: baseTop + i * rowHeight,
        height: rowHeight,
        left: 0,
        right: 0,
      });
    });
  }, [start, end, scrollTop, rowHeight, cache, renderRow]);

  // ----- render -----
  return (
    <div
      ref={containerRef}
      className={`virtual-list ${className}`}
      onWheel={handleWheel}
      role="list"
      aria-rowcount={rowCount}
    >
      <div className="virtual-list-viewport" onScroll={handleHorizontalScroll}>
        <div className="virtual-list-items">{renderedItems}</div>
      </div>
      <Scrollbar
        totalHeight={totalHeight}
        viewportHeight={viewportHeight}
        maxScrollTop={maxScrollTop}
        scrollTop={scrollTop}
        onScrollUpdate={updateScrollAndRange}
      />
    </div>
  );
});

VirtualList.displayName = 'VirtualList';

export default VirtualList;
