import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, once } from '@tauri-apps/api/event';
import { SEARCH_DEBOUNCE_MS } from '../constants';

export function useAppState() {
  const [results, setResults] = useState([]);
  const [isInitialized, setIsInitialized] = useState(false);
  const [scannedFiles, setScannedFiles] = useState(0);
  const [processedEvents, setProcessedEvents] = useState(0);

  useEffect(() => {
    listen('status_bar_update', (event) => {
      const { scanned_files, processed_events } = event.payload;
      setScannedFiles(scanned_files);
      setProcessedEvents(processed_events);
    });
    once('init_completed', () => setIsInitialized(true));
  }, []);

  return {
    results,
    setResults,
    isInitialized,
    scannedFiles,
    processedEvents
  };
}

export function useSearch(setResults) {
  const debounceTimerRef = useRef(null);
  const loadingDelayTimerRef = useRef(null);
  const [currentQuery, setCurrentQuery] = useState('');
  const [hasInitialSearchRun, setHasInitialSearchRun] = useState(false);
  const [showLoadingUI, setShowLoadingUI] = useState(false);
  const [initialFetchCompleted, setInitialFetchCompleted] = useState(false);
  const [durationMs, setDurationMs] = useState(null);
  const [resultCount, setResultCount] = useState(0);
  const [searchError, setSearchError] = useState(null);

  const handleSearch = useCallback(async (query) => {
    const startTs = performance.now();
    const isInitial = !hasInitialSearchRun;
    setSearchError(null);
    // 初始搜索立即进入 loading；后续搜索使用延迟避免闪烁
    if (isInitial) {
      setShowLoadingUI(true);
    } else {
      loadingDelayTimerRef.current = setTimeout(() => {
        setResults([]);
        setShowLoadingUI(true);
      }, 150);
    }

    try {
      const searchResults = await invoke("search", { query });

      // 清除延迟timer
      if (loadingDelayTimerRef.current) {
        clearTimeout(loadingDelayTimerRef.current);
        loadingDelayTimerRef.current = null;
      }

      setResults(searchResults);
      setCurrentQuery(query.trim());
      setShowLoadingUI(false);
      if (!initialFetchCompleted) setInitialFetchCompleted(true);

      const endTs = performance.now();
      const dur = endTs - startTs;
      setDurationMs(dur);
      setResultCount(Array.isArray(searchResults) ? searchResults.length : 0);
    } catch (error) {
      console.error('Search failed:', error);
      if (loadingDelayTimerRef.current) {
        clearTimeout(loadingDelayTimerRef.current);
        loadingDelayTimerRef.current = null;
      }
      setSearchError(error || 'An unknown error occurred.');
      setShowLoadingUI(false);
      if (!initialFetchCompleted) setInitialFetchCompleted(true); // 即使失败也结束初始加载状态

      const endTs = performance.now();
      const dur = endTs - startTs;
      setDurationMs(dur);
      setResultCount(0);
    }
  }, [setResults, hasInitialSearchRun, initialFetchCompleted]);

  // 初始搜索
  useEffect(() => {
    if (!hasInitialSearchRun) {
      handleSearch("");
      setHasInitialSearchRun(true);
    }
  }, [handleSearch, hasInitialSearchRun]);

  const onQueryChange = useCallback((e) => {
    const inputValue = e.target.value;
    clearTimeout(debounceTimerRef.current);

    debounceTimerRef.current = setTimeout(() => {
      handleSearch(inputValue);
    }, SEARCH_DEBOUNCE_MS);
  }, [handleSearch]);

  // 清理定时器
  useEffect(() => {
    return () => {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }
      if (loadingDelayTimerRef.current) {
        clearTimeout(loadingDelayTimerRef.current);
      }
    };
  }, []);

  return { onQueryChange, currentQuery, showLoadingUI, initialFetchCompleted, durationMs, resultCount, searchError };
}

export { useRowData } from './useRowData';
export { useColumnResize } from './useColumnResize';
export { useContextMenu } from './useContextMenu';
