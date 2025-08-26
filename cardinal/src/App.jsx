import React, { useRef, useCallback, useEffect, useState } from 'react';
import './App.css';
import { ContextMenu } from './components/ContextMenu';
import { ColumnHeader } from './components/ColumnHeader';
import { FileRow } from './components/FileRow';
import StatusBar from './components/StatusBar';
import { useAppState, useSearch, useRowData } from './hooks';
import { useColumnResize } from './hooks/useColumnResize';
import { useContextMenu } from './hooks/useContextMenu';
import { ROW_HEIGHT, OVERSCAN_ROW_COUNT } from './constants';
import { VirtualList } from './components/VirtualList';
import { StateDisplay } from './components/StateDisplay';

function App() {
  const { results, setResults, isInitialized, scannedFiles, processedEvents } = useAppState();
  const { colWidths, onResizeStart, autoFitColumns } = useColumnResize();
  const { getItem, ensureRangeLoaded } = useRowData(results);
  const {
    contextMenu, showContextMenu, closeContextMenu, menuItems,
    headerContextMenu, showHeaderContextMenu, closeHeaderContextMenu, headerMenuItems
  } = useContextMenu(autoFitColumns);
  const { onQueryChange, currentQuery, showLoadingUI, initialFetchCompleted, durationMs, resultCount, searchError } = useSearch(setResults);

  const headerRef = useRef(null);
  const scrollAreaRef = useRef(null);
  const virtualListRef = useRef(null);
  const prevQueryRef = useRef('');
  const prevResultsLenRef = useRef(0);

  // 当搜索结果更新时，立即预加载前面的数据并重置滚动位置
  useEffect(() => {
    if (results.length > 0) {
      const isNewQuery = prevQueryRef.current !== currentQuery;
      const wasEmpty = prevResultsLenRef.current === 0;

      // 仅在新查询的第一次结果出现时滚动到顶部
      if (isNewQuery && virtualListRef.current) {
        virtualListRef.current.scrollToTop();
      }

      // 预加载：新查询第一次出现结果 或 之前为空（首次流式填充）
      if (isNewQuery || wasEmpty) {
        const estimatedViewportHeight = 600; // 默认估计高度, 无法测量时的备选
        const visibleRows = Math.ceil(estimatedViewportHeight / ROW_HEIGHT);
        const preloadCount = Math.min(visibleRows + OVERSCAN_ROW_COUNT * 2, results.length);
        ensureRangeLoaded(0, preloadCount - 1);
      }
    }
    prevQueryRef.current = currentQuery;
    prevResultsLenRef.current = results.length;
  }, [results, currentQuery, ensureRangeLoaded]);

  // 滚动同步处理 - 单向同步版本（Grid -> Header）
  const handleHorizontalSync = useCallback((scrollLeft) => {
    if (headerRef.current) headerRef.current.scrollLeft = scrollLeft;
  }, []);

  // 单元格渲染
  const renderRow = (rowIndex, rowStyle) => {
    const item = getItem(rowIndex);
    return (
      <FileRow
        key={rowIndex}
        item={item}
        rowIndex={rowIndex}
        style={{ ...rowStyle, width: 'var(--columns-total)' }}
        onContextMenu={showContextMenu}
        searchQuery={currentQuery}
      />
    );
  };

  const getDisplayState = () => {
    if (showLoadingUI || !initialFetchCompleted) return 'loading';
    if (searchError) return 'error';
    if (results.length === 0) return 'empty';
    return 'results';
  };

  const displayState = getDisplayState();

  return (
    <main className="container">
      <div className="search-container">
        <input
          id="search-input"
          onChange={onQueryChange}
          placeholder="Search for files and folders..."
          spellCheck={false}
          autoCorrect="off"
          autoComplete="off"
          autoCapitalize="off"
        />
      </div>
      <div
        className="results-container"
        style={{
          ['--w-filename']: `${colWidths.filename}px`,
          ['--w-path']: `${colWidths.path}px`,
          ['--w-size']: `${colWidths.size}px`,
          ['--w-modified']: `${colWidths.modified}px`,
          ['--w-created']: `${colWidths.created}px`,
        }}
      >
        <div className="scroll-area" ref={scrollAreaRef}>
          <ColumnHeader
            ref={headerRef}
            onResizeStart={onResizeStart}
            onContextMenu={showHeaderContextMenu}
          />
          <div className="flex-fill">
            {displayState !== 'results' ? (
              <StateDisplay state={displayState} message={searchError} query={currentQuery} />
            ) : (
              <VirtualList
                ref={virtualListRef}
                rowCount={results.length}
                rowHeight={ROW_HEIGHT}
                overscan={OVERSCAN_ROW_COUNT}
                renderRow={renderRow}
                onRangeChange={ensureRangeLoaded}
                onScrollSync={handleHorizontalSync}
                className="virtual-list"
              />
            )}
          </div>
        </div>
      </div>
      {contextMenu.visible && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={menuItems}
          onClose={closeContextMenu}
        />
      )}
      {headerContextMenu.visible && (
        <ContextMenu
          x={headerContextMenu.x}
          y={headerContextMenu.y}
          items={headerMenuItems}
          onClose={closeHeaderContextMenu}
        />
      )}
      <StatusBar
        scannedFiles={scannedFiles}
        processedEvents={processedEvents}
        isReady={isInitialized}
        searchDurationMs={durationMs}
        resultCount={resultCount}
      />
    </main>
  );
}

export default App;
