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

function App() {
  const [results, setResults] = useState([]);
  const lruCache = useRef(new LRUCache(1000));
  const infiniteLoaderRef = useRef(null);
  const debounceTimerRef = useRef(null);
  const [isInitialized, setIsInitialized] = useState(false);
  const [isStatusBarVisible, setIsStatusBarVisible] = useState(true);
  const [statusText, setStatusText] = useState("Walking filesystem...");

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
    return (
      <div key={key} style={style} className="row">
        {item ? (
          item
        ) : (
          <div/>
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
      <div className="results-container" style={{ flex: 1 }}>
        <InfiniteLoader
          ref={infiniteLoaderRef}
          isRowLoaded={isRowLoaded}
          loadMoreRows={loadMoreRows}
          rowCount={results.length}
        >
          {({ onRowsRendered, registerChild }) => (
            <AutoSizer>
              {({ height, width }) => (
                <List
                  ref={registerChild}
                  onRowsRendered={onRowsRendered}
                  width={width}
                  height={height}
                  rowCount={results.length}
                  rowHeight={30}
                  rowRenderer={rowRenderer}
                />
              )}
            </AutoSizer>
          )}
        </InfiniteLoader>
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
