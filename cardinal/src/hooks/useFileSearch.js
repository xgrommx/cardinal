import { useReducer, useRef, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { SEARCH_DEBOUNCE_MS } from '../constants';

// Centralised search state management with debounced queries and Tauri integration
const initialSearchState = {
  results: [],
  isInitialized: false,
  scannedFiles: 0,
  processedEvents: 0,
  currentQuery: '',
  showLoadingUI: false,
  initialFetchCompleted: false,
  durationMs: null,
  resultCount: 0,
  searchError: null,
};

const initialSearchParams = {
  query: '',
  useRegex: false,
  caseSensitive: false,
};

const cancelTimer = (timerRef) => {
  if (timerRef.current) {
    clearTimeout(timerRef.current);
    timerRef.current = null;
  }
};

// Keep reducer pure and colocated so useReducer stays predictable
function reducer(state, action) {
  switch (action.type) {
    case 'STATUS_UPDATE':
      return {
        ...state,
        scannedFiles: action.payload.scannedFiles,
        processedEvents: action.payload.processedEvents,
      };
    case 'INIT_COMPLETED':
      return { ...state, isInitialized: true };
    case 'SEARCH_REQUEST':
      return {
        ...state,
        searchError: null,
        showLoadingUI: action.payload.immediate ? true : state.showLoadingUI,
      };
    case 'SEARCH_LOADING_DELAY':
      return {
        ...state,
        showLoadingUI: true,
      };
    case 'SEARCH_SUCCESS':
      return {
        ...state,
        results: action.payload.results,
        currentQuery: action.payload.query,
        showLoadingUI: false,
        initialFetchCompleted: true,
        durationMs: action.payload.duration,
        resultCount: action.payload.count,
        searchError: null,
      };
    case 'SEARCH_FAILURE':
      return {
        ...state,
        showLoadingUI: false,
        searchError: action.payload.error,
        initialFetchCompleted: true,
        durationMs: action.payload.duration,
        resultCount: 0,
      };
    default:
      return state;
  }
}

export function useFileSearch() {
  const [state, dispatch] = useReducer(reducer, initialSearchState);
  const latestSearchRef = useRef(initialSearchParams);
  const searchVersionRef = useRef(0);
  const hasInitialSearchRunRef = useRef(false);
  const debounceTimerRef = useRef(null);
  const loadingDelayTimerRef = useRef(null);

  const [searchParams, patchSearchParams] = useReducer((prev, patch) => {
    const next = { ...prev, ...patch };
    latestSearchRef.current = next;
    return next;
  }, initialSearchParams);

  const handleStatusUpdate = useCallback((scannedFiles, processedEvents) => {
    dispatch({
      type: 'STATUS_UPDATE',
      payload: { scannedFiles, processedEvents },
    });
  }, []);

  const markInitialized = useCallback(() => {
    dispatch({ type: 'INIT_COMPLETED' });
  }, []);

  const cancelPendingSearches = useCallback(() => {
    cancelTimer(debounceTimerRef);
    cancelTimer(loadingDelayTimerRef);
  }, []);

  const handleSearch = useCallback(async (overrides = {}) => {
    // Merge overrides with latest input to avoid racing with queued updates
    const nextSearch = { ...latestSearchRef.current, ...overrides };
    latestSearchRef.current = nextSearch;
    const requestVersion = searchVersionRef.current + 1;
    searchVersionRef.current = requestVersion;

    const { query, useRegex, caseSensitive } = nextSearch;
    const startTs = performance.now();
    const isInitial = !hasInitialSearchRunRef.current;
    const trimmedQuery = query.trim();

    dispatch({ type: 'SEARCH_REQUEST', payload: { immediate: isInitial } });

    if (!isInitial) {
      cancelTimer(loadingDelayTimerRef);
      loadingDelayTimerRef.current = setTimeout(() => {
        dispatch({ type: 'SEARCH_LOADING_DELAY' });
        loadingDelayTimerRef.current = null;
      }, 150);
    }

    try {
      const searchResults = await invoke('search', {
        query,
        options: {
          useRegex,
          caseInsensitive: !caseSensitive,
        },
      });

      if (searchVersionRef.current !== requestVersion) {
        return;
      }

      cancelTimer(loadingDelayTimerRef);

      const endTs = performance.now();
      const duration = endTs - startTs;

      dispatch({
        type: 'SEARCH_SUCCESS',
        payload: {
          results: searchResults,
          query: trimmedQuery,
          duration,
          count: Array.isArray(searchResults) ? searchResults.length : 0,
        },
      });
    } catch (error) {
      console.error('Search failed:', error);

      if (searchVersionRef.current !== requestVersion) {
        return;
      }

      cancelTimer(loadingDelayTimerRef);

      const endTs = performance.now();
      const duration = endTs - startTs;

      dispatch({
        type: 'SEARCH_FAILURE',
        payload: {
          error: error || 'An unknown error occurred.',
          duration,
        },
      });
    } finally {
      hasInitialSearchRunRef.current = true;
    }
  }, []);

  const queueSearch = useCallback(
    (query) => {
      patchSearchParams({ query });
      cancelTimer(debounceTimerRef);
      debounceTimerRef.current = setTimeout(() => {
        handleSearch({ query });
      }, SEARCH_DEBOUNCE_MS);
    },
    [handleSearch],
  );

  const resetSearchQuery = useCallback(() => {
    patchSearchParams({ query: '' });
    cancelPendingSearches();
  }, [cancelPendingSearches]);

  useEffect(() => {
    return () => {
      cancelPendingSearches();
    };
  }, [cancelPendingSearches]);

  useEffect(() => {
    if (!hasInitialSearchRunRef.current) {
      handleSearch({ query: '' });
      return;
    }

    // Only re-run if the current query is still non-empty; otherwise we wait for user input
    if (!(latestSearchRef.current.query || '').trim()) {
      return;
    }

    handleSearch();
  }, [handleSearch, searchParams.caseSensitive, searchParams.useRegex]);

  return {
    state,
    searchParams,
    updateSearchParams: patchSearchParams,
    queueSearch,
    handleSearch,
    resetSearchQuery,
    cancelPendingSearches,
    handleStatusUpdate,
    markInitialized,
  };
}
