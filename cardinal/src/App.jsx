import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { once, listen } from '@tauri-apps/api/event';
import { InfiniteLoader, List, AutoSizer } from 'react-virtualized';
import 'react-virtualized/styles.css';
import "./App.css";

class LRUCache {
  constructor(capacity) {
    this.capacity = capacity;
    this.cache = new Map();
  }

  get(key) {
    if (!this.cache.has(key)) {
      return undefined;
    }
    const value = this.cache.get(key);
    this.cache.delete(key);
    this.cache.set(key, value);
    return value;
  }

  put(key, value) {
    if (this.cache.has(key)) {
      this.cache.delete(key);
    } else if (this.cache.size >= this.capacity) {
      const oldestKey = this.cache.keys().next().value;
      this.cache.delete(oldestKey);
    }
    this.cache.set(key, value);
  }

  has(key) {
    return this.cache.has(key);
  }

  clear() {
    this.cache.clear();
  }
}

// Format bytes into KB with one decimal place, e.g., 12.3 KB
function formatKB(bytes) {
  if (bytes == null) return null;
  const kb = bytes / 1024;
  if (!isFinite(kb)) return null;
  return `${kb.toFixed(kb < 10 ? 1 : 0)} KB`;
}

function App() {
  const [results, setResults] = useState([]);
  // Column widths in px
  const [colWidths, setColWidths] = useState({ path: 600, modified: 180, created: 180, size: 120 });
  const resizingRef = useRef(null); // { key: 'path'|'modified'|'created'|'size', startX, startW }
  const lruCache = useRef(new LRUCache(1000));
  const infiniteLoaderRef = useRef(null);
  const debounceTimerRef = useRef(null);
  const [isInitialized, setIsInitialized] = useState(false);
  const [isStatusBarVisible, setIsStatusBarVisible] = useState(true);
  const [statusText, setStatusText] = useState("Walking filesystem...");
  const titlebarScrollRef = useRef(null);
  const virtualListScrollRef = useRef(null);

  useEffect(() => {
    listen('status_update', (event) => {
      setStatusText(event.payload);
    });
    once('init_completed', () => {
      setIsInitialized(true);
    });
  }, []);

  useEffect(() => {
    if (isInitialized) {
      const timer = setTimeout(() => {
        setIsStatusBarVisible(false);
      }, 2000);
      return () => clearTimeout(timer);
    }
  }, [isInitialized]);

  useEffect(() => {
    // console.log("results", results);
    if (infiniteLoaderRef.current) {
      // console.log("resetting load more rows cache");
      infiniteLoaderRef.current.resetLoadMoreRowsCache(true);
    }
  }, [results]);

  // Keep title bar and results horizontal scroll in sync
  useEffect(() => {
    const headerEl = titlebarScrollRef.current;
    const bodyEl = virtualListScrollRef.current;
    if (!headerEl || !bodyEl) return;
    let syncing = false;
    const onBodyScroll = () => {
      if (syncing) return;
      syncing = true;
      headerEl.scrollLeft = bodyEl.scrollLeft;
      syncing = false;
    };
    const onHeaderScroll = () => {
      if (syncing) return;
      syncing = true;
      bodyEl.scrollLeft = headerEl.scrollLeft;
      syncing = false;
    };
    bodyEl.addEventListener('scroll', onBodyScroll);
    headerEl.addEventListener('scroll', onHeaderScroll);
    return () => {
      bodyEl.removeEventListener('scroll', onBodyScroll);
      headerEl.removeEventListener('scroll', onHeaderScroll);
    };
  }, [results, colWidths]);

  const handleSearch = async (query) => {
    // console.log("handleSearch", query);
    let searchResults = [];
    if (query.trim() !== '') {
      searchResults = await invoke("search", { query });
    }
    // console.log("got query results, clearing lru cache", query, searchResults);
    lruCache.current.clear();
    setResults(searchResults);
  };

  const onQueryChange = (e) => {
    const currentQuery = e.target.value;
    clearTimeout(debounceTimerRef.current);
    debounceTimerRef.current = setTimeout(() => {
      handleSearch(currentQuery);
    }, 300);
  };

  // Column resizing handlers
  const onResizeStart = (key) => (e) => {
    e.preventDefault();
    e.stopPropagation();
    resizingRef.current = { key, startX: e.clientX, startW: colWidths[key] };
    window.addEventListener('mousemove', onResizing);
    window.addEventListener('mouseup', onResizeEnd, { once: true });
    document.body.style.userSelect = 'none';
    document.body.style.cursor = 'col-resize';
  };
  const onResizing = (e) => {
    const ctx = resizingRef.current;
    if (!ctx) return;
    const delta = e.clientX - ctx.startX;
    const nextW = Math.max(80, Math.min(1200, ctx.startW + delta));
    setColWidths((w) => ({ ...w, [ctx.key]: nextW }));
  };
  const onResizeEnd = () => {
    resizingRef.current = null;
    window.removeEventListener('mousemove', onResizing);
    document.body.style.userSelect = '';
    document.body.style.cursor = '';
  };

  const isRowLoaded = ({ index }) => {
    let loaded = lruCache.current.has(index);
    // console.log("isRowLoaded loading", index, loaded)
    return loaded;
  };

  const loadMoreRows = async ({ startIndex, stopIndex }) => {
    let rows = results.slice(startIndex, stopIndex + 1);
    // console.log("start loading more rows", startIndex, stopIndex, rows);
    const searchResults = await invoke("get_nodes_info", { results: rows });
    // console.log("loading more rows", startIndex, stopIndex, searchResults);
    for (let i = startIndex; i <= stopIndex; i++) {
      lruCache.current.put(i, searchResults[i - startIndex]);
    }
  };

  const rowRenderer = ({ key, index, style }) => {
    const item = lruCache.current.get(index);
    // console.log("rendering row", index, item);
    const path = typeof item === 'string' ? item : item?.path;
    // Prefer nested metadata.mtime, but also support top-level mtime if backend changed shape
    const mtimeSec =
      typeof item !== 'string'
        ? (item?.metadata?.mtime ?? item?.mtime)
        : undefined;
    const mtimeText =
      mtimeSec != null ? new Date(mtimeSec * 1000).toLocaleString() : null;
    const ctimeSec =
      typeof item !== 'string'
        ? (item?.metadata?.ctime ?? item?.ctime)
        : undefined;
    const ctimeText =
      ctimeSec != null ? new Date(ctimeSec * 1000).toLocaleString() : null;
    const sizeBytes =
      typeof item !== 'string'
        ? (item?.metadata?.size ?? item?.size)
        : undefined;
    const sizeText = formatKB(sizeBytes);
    return (
      <div
        key={key}
        style={style}
        className={`row ${index % 2 === 0 ? 'row-even' : 'row-odd'}`}
      >
        {item ? (
          <div
            className="columns row-inner"
            title={path}
          >
            <span className="path-text">{path}</span>
            {mtimeText ? (
              <span className="mtime-text">{mtimeText}</span>
            ) : (
              <span className="mtime-text muted">—</span>
            )}
            {ctimeText ? (
              <span className="ctime-text">{ctimeText}</span>
            ) : (
              <span className="ctime-text muted">—</span>
            )}
            {sizeText ? (
              <span className="size-text">{sizeText}</span>
            ) : (
              <span className="size-text muted">—</span>
            )}
          </div>
        ) : (
          <div />
        )}
      </div>
    );
  };

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
      {/* Title Bar (middle) */}
      <div
        className="titlebar-container"
        style={{
          ['--w-path']: `${colWidths.path}px`,
          ['--w-modified']: `${colWidths.modified}px`,
          ['--w-created']: `${colWidths.created}px`,
          ['--w-size']: `${colWidths.size}px`,
        }}
      >
        <div className="titlebar-scroll" ref={titlebarScrollRef}>
          <div className="header-row columns">
            <span className="path-text header header-cell">
              Path
              <span className="col-resizer" onMouseDown={onResizeStart('path')} />
            </span>
            <span className="mtime-text header header-cell">
              Modified
              <span className="col-resizer" onMouseDown={onResizeStart('modified')} />
            </span>
            <span className="ctime-text header header-cell">
              Created
              <span className="col-resizer" onMouseDown={onResizeStart('created')} />
            </span>
            <span className="size-text header header-cell">
              Size
              <span className="col-resizer" onMouseDown={onResizeStart('size')} />
            </span>
          </div>
        </div>
      </div>

      {/* Results (bottom) */}
      <div
        className="results-container"
        style={{
          ['--w-path']: `${colWidths.path}px`,
          ['--w-modified']: `${colWidths.modified}px`,
          ['--w-created']: `${colWidths.created}px`,
          ['--w-size']: `${colWidths.size}px`,
        }}
      >
        <div className="virtual-list" ref={virtualListScrollRef}>
          <InfiniteLoader
            ref={infiniteLoaderRef}
            isRowLoaded={isRowLoaded}
            loadMoreRows={loadMoreRows}
            rowCount={results.length}
          >
            {({ onRowsRendered, registerChild }) => (
              <AutoSizer>
                {({ height, width }) => {
                  const colGap = 12; // keep in sync with CSS --col-gap
                  const columnsTotal =
                    colWidths.path + colWidths.modified + colWidths.created + colWidths.size + (3 * colGap) + 20; // + paddings
                  return (
                  <List
                    ref={registerChild}
                    onRowsRendered={onRowsRendered}
                    width={Math.max(width, columnsTotal)}
                    height={height}
                    rowCount={results.length}
                    rowHeight={24}
                    rowRenderer={rowRenderer}
                  />
                  );
                }}
              </AutoSizer>
            )}
          </InfiniteLoader>
        </div>
      </div>
      {isStatusBarVisible && (
        <div className={`status-bar ${isInitialized ? 'fade-out' : ''}`}>
          {isInitialized ? 'Initialized' :
            <div className="initializing-container">
              <div className="spinner"></div>
              <span>{statusText}</span>
            </div>
          }
        </div>
      )}
    </main>
  );
}

export default App;
